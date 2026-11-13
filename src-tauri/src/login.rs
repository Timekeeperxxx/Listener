use crate::{
    auth::Credentials,
    error::{AppError, Result},
    qq::QqMusic,
};
use base64::Engine;
use reqwest::{
    cookie::{CookieStore, Jar},
    Client,
};
use serde::Serialize;
use serde_json::json;
use std::{
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const QQ_APP_ID: &str = "716027609";
const MUSIC_APP_ID: &str = "100497308";
const LOGIN_JUMP: &str = "https://graph.qq.com/oauth2.0/login_jump";
const REDIRECT: &str =
    "https://y.qq.com/portal/wx_redirect.html?login_type=1&surl=https://y.qq.com/";

#[derive(Clone)]
pub struct QrSession {
    pub id: String,
    client: Client,
    jar: Arc<Jar>,
    qrsig: String,
    created: Instant,
    kind: String,
    mobile: Option<Arc<crate::mobile::MobileSession>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrImage {
    pub id: String,
    pub image: String,
    pub expires_in: u32,
}

#[derive(Clone)]
pub enum QrStatus {
    Waiting,
    Scanned,
    Expired,
    Refused,
    Authorized(Credentials),
}

impl QrSession {
    async fn request_qq_login(params: &[(&str, &str)]) -> Result<(Self, reqwest::Response)> {
        let jar = Arc::new(Jar::default());
        // A fresh cookie jar per QR attempt prevents two login attempts sharing tickets.
        let client = Client::builder().cookie_provider(jar.clone())
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/18.0 Safari/605.1.15")
            .connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(20)).build()?;
        let response = client
            .get("https://ssl.ptlogin2.qq.com/ptqrshow")
            .header("Referer", "https://xui.ptlogin2.qq.com/")
            .query(&[
                ("appid", QQ_APP_ID),
                ("t", &timestamp()),
                ("daid", "383"),
                ("pt_3rd_aid", MUSIC_APP_ID),
            ])
            .query(params)
            .send()
            .await?
            .error_for_status()?;
        let qrsig = response
            .cookies()
            .find(|c| c.name() == "qrsig")
            .map(|c| c.value().to_owned())
            .ok_or_else(|| AppError::protocol("qrsig"))?;
        Ok((
            Self {
                id: uuid::Uuid::new_v4().to_string(),
                client,
                jar,
                qrsig,
                created: Instant::now(),
                kind: "qq".into(),
                mobile: None,
            },
            response,
        ))
    }

    pub async fn create() -> Result<(Self, QrImage)> {
        let (session, response) =
            Self::request_qq_login(&[("e", "2"), ("l", "M"), ("s", "3"), ("d", "72"), ("v", "4")])
                .await?;
        let bytes = response.bytes().await?;
        if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(AppError::new(
                "QR_IMAGE_ERROR",
                "QQ 暂时未能生成二维码，请点击刷新。",
            ));
        }
        let image = QrImage {
            id: session.id.clone(),
            image: format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            ),
            expires_in: 180,
        };
        Ok((session, image))
    }

    pub async fn create_for(kind: &str, qq: &QqMusic) -> Result<(Self, QrImage)> {
        if kind == "qq" {
            return Self::create().await;
        }
        if !matches!(kind, "wx" | "mobile") {
            return Err(AppError::new("INVALID_LOGIN_TYPE", "不支持此扫码方式。"));
        }
        let jar = Arc::new(Jar::default());
        let client = Client::builder()
            .cookie_provider(jar.clone())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(35))
            .build()?;
        let (identifier, bytes, mime) = if kind == "wx" {
            let html = client.get("https://open.weixin.qq.com/connect/qrconnect").query(&[("appid","wx48db31d50e334801"),("redirect_uri","https://y.qq.com/portal/wx_redirect.html?login_type=2&surl=https://y.qq.com/"),("response_type","code"),("scope","snsapi_login"),("state","STATE"),("href","https://y.qq.com/mediastyle/music_v17/src/css/popup_wechat.css#wechat_redirect")]).send().await?.error_for_status()?.text().await?;
            let regex = regex::Regex::new(r#"uuid=([A-Za-z0-9_=-]+)"#).expect("uuid regex");
            let id = regex
                .captures(&html)
                .ok_or_else(|| AppError::protocol("wechat QR uuid"))?[1]
                .to_string();
            let bytes = client
                .get(format!("https://open.weixin.qq.com/connect/qrcode/{id}"))
                .header("Referer", "https://open.weixin.qq.com/connect/qrconnect")
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?;
            let mime = if bytes.starts_with(b"\xff\xd8") {
                "image/jpeg"
            } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                "image/png"
            } else {
                return Err(AppError::protocol("wechat QR image"));
            };
            (id, bytes.to_vec(), mime)
        } else {
            let data = qq
                .rpc(
                    None,
                    json!({"ct":23,"cv":0}),
                    "music.login.LoginServer",
                    "CreateQRCode",
                    json!({"tmeAppID":"qqmusic","ct":19,"cv":0}),
                )
                .await?;
            let id = data["qrcodeID"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or_else(|| AppError::protocol("qrcodeID"))?
                .to_owned();
            let raw = data["qrcode"]
                .as_str()
                .ok_or_else(|| AppError::protocol("qrcode"))?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(raw.rsplit(',').next().expect("QR data"))
                .map_err(|_| AppError::protocol("QR base64"))?;
            if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                return Err(AppError::protocol("mobile QR image"));
            }
            (id, bytes, "image/png")
        };
        let id = uuid::Uuid::new_v4().to_string();
        let image = QrImage {
            id: id.clone(),
            image: format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            ),
            expires_in: 180,
        };
        let mobile = if kind == "mobile" {
            Some(Arc::new(crate::mobile::MobileSession::start(
                identifier.clone(),
                qq.clone(),
            )))
        } else {
            None
        };
        Ok((
            Self {
                id,
                client,
                jar,
                qrsig: identifier,
                created: Instant::now(),
                kind: kind.into(),
                mobile,
            },
            image,
        ))
    }

    async fn poll_wechat(&self, qq: &QqMusic) -> Result<QrStatus> {
        let response = self
            .client
            .get("https://lp.open.weixin.qq.com/connect/l/qrconnect")
            .query(&[("uuid", self.qrsig.as_str()), ("_", &timestamp())])
            .send()
            .await;
        let text = match response {
            Err(error) if error.is_timeout() => return Ok(QrStatus::Waiting),
            result => result?.error_for_status()?.text().await?,
        };
        let regex = regex::Regex::new(r"wx_errcode=(\d+);\s*window.wx_code='([^']*)'")
            .expect("wechat status regex");
        let caps = regex
            .captures(&text)
            .ok_or_else(|| AppError::protocol("wechat QR status"))?;
        match &caps[1] {
            "408" => Ok(QrStatus::Waiting),
            "404" => Ok(QrStatus::Scanned),
            "402" => Ok(QrStatus::Expired),
            "403" => Ok(QrStatus::Refused),
            "405" => {
                let data = qq
                    .rpc(
                        None,
                        json!({"ct":24,"cv":0,"tmeLoginType":1}),
                        "music.login.LoginServer",
                        "Login",
                        json!({"code":&caps[2],"strAppid":"wx48db31d50e334801"}),
                    )
                    .await?;
                Ok(QrStatus::Authorized(Credentials::from_login(data, 1)?))
            }
            code => Err(AppError::new(
                "QR_LOGIN_FAILED",
                format!("微信扫码未完成（{code}）。"),
            )),
        }
    }

    pub async fn poll(&self, qq: &QqMusic) -> Result<QrStatus> {
        if self.created.elapsed() >= Duration::from_secs(180) {
            return Ok(QrStatus::Expired);
        }
        if self.kind == "wx" {
            return self.poll_wechat(qq).await;
        }
        if let Some(mobile) = &self.mobile {
            return mobile.poll();
        }
        let response = self
            .client
            .get("https://ssl.ptlogin2.qq.com/ptqrlogin")
            .header("Referer", "https://xui.ptlogin2.qq.com/")
            .query(&[
                ("u1", LOGIN_JUMP),
                ("ptqrtoken", &hash33(&self.qrsig, 0).to_string()),
                ("ptredirect", "0"),
                ("h", "1"),
                ("t", "1"),
                ("g", "1"),
                ("from_ui", "1"),
                ("ptlang", "2052"),
                ("action", &format!("0-0-{}", timestamp())),
                ("js_ver", "20102616"),
                ("js_type", "1"),
                ("pt_uistyle", "40"),
                ("aid", QQ_APP_ID),
                ("daid", "383"),
                ("pt_3rd_aid", MUSIC_APP_ID),
                ("has_onekey", "1"),
            ])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let (code, redirect) = parse_callback(&response)?;
        match code {
            66 => Ok(QrStatus::Waiting),
            67 => Ok(QrStatus::Scanned),
            65 => Ok(QrStatus::Expired),
            68 => Ok(QrStatus::Refused),
            0 => Ok(QrStatus::Authorized(self.authorize(qq, &redirect).await?)),
            _ => Err(AppError::new(
                "QR_LOGIN_FAILED",
                format!("QQ 授权未能完成（状态 {code}），请重新发起登录。"),
            )),
        }
    }

    async fn authorize(&self, qq: &QqMusic, redirect: &str) -> Result<Credentials> {
        let url = url::Url::parse(redirect).map_err(|_| AppError::protocol("QR redirect"))?;
        let uin = url
            .query_pairs()
            .find(|(key, _)| key == "uin")
            .map(|(_, v)| v.into_owned())
            .ok_or_else(|| AppError::protocol("QR uin"))?;
        let sigx = url
            .query_pairs()
            .find(|(key, _)| key == "ptsigx")
            .map(|(_, v)| v.into_owned())
            .ok_or_else(|| AppError::protocol("QR ptsigx"))?;
        self.client
            .get("https://ssl.ptlogin2.graph.qq.com/check_sig")
            .header("Referer", "https://xui.ptlogin2.qq.com/")
            .query(&[
                ("uin", uin.as_str()),
                ("pttype", "1"),
                ("service", "ptqrlogin"),
                ("nodirect", "0"),
                ("ptsigx", sigx.as_str()),
                ("s_url", LOGIN_JUMP),
                ("ptlang", "2052"),
                ("ptredirect", "100"),
                ("aid", QQ_APP_ID),
                ("daid", "383"),
                ("j_later", "0"),
                ("low_login_hour", "0"),
                ("regmaster", "0"),
                ("pt_login_type", "3"),
                ("pt_aid", "0"),
                ("pt_aaid", "16"),
                ("pt_light", "0"),
                ("pt_3rd_aid", MUSIC_APP_ID),
            ])
            .send()
            .await?
            .error_for_status()?;
        let graph = url::Url::parse("https://graph.qq.com/").expect("constant QQ URL");
        let cookies = self
            .jar
            .cookies(&graph)
            .ok_or_else(|| AppError::protocol("QQ authorization cookies"))?;
        let skey = cookies
            .to_str()
            .map_err(|_| AppError::protocol("QQ authorization cookies"))?
            .split(';')
            .filter_map(|p| p.trim().split_once('='))
            .find(|(k, _)| *k == "p_skey")
            .map(|(_, v)| v.to_string())
            .ok_or_else(|| AppError::protocol("p_skey"))?;
        let response = self
            .client
            .post("https://graph.qq.com/oauth2.0/authorize")
            .header("Referer", "https://graph.qq.com/")
            .form(&[
                ("response_type", "code"),
                ("client_id", MUSIC_APP_ID),
                ("redirect_uri", REDIRECT),
                ("scope", "get_user_info,get_app_friends"),
                ("state", "state"),
                ("switch", ""),
                ("from_ptlogin", "1"),
                ("src", "1"),
                ("update_auth", "1"),
                ("openapi", "1010_1030"),
                ("g_tk", &hash33(&skey, 5381).to_string()),
                ("auth_time", &timestamp()),
                ("ui", &uuid::Uuid::new_v4().to_string()),
            ])
            .send()
            .await?
            .error_for_status()?;
        let location = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                AppError::new(
                    "QQ_AUTH_REQUIRED",
                    "QQ 音乐尚未完成授权，请在手机 QQ 确认后重试。",
                )
            })?;
        let target = url::Url::parse(location).map_err(|_| AppError::protocol("OAuth redirect"))?;
        let code = target
            .query_pairs()
            .find(|(k, _)| k == "code")
            .map(|(_, v)| v.into_owned())
            .ok_or_else(|| AppError::protocol("OAuth code"))?;
        exchange_qq_code(qq, &code).await
    }
}

async fn exchange_qq_code(qq: &QqMusic, code: &str) -> Result<Credentials> {
    let data = qq
        .rpc(
            None,
            json!({"ct":24,"cv":0,"tmeLoginType":2}),
            "QQConnectLogin.LoginServer",
            "QQLogin",
            json!({"code":code}),
        )
        .await?;
    Credentials::from_login(data, 2)
}

#[cfg(target_os = "ios")]
pub fn qq_authorize_url(state: &str) -> url::Url {
    let mut url =
        url::Url::parse("https://graph.qq.com/oauth2.0/authorize").expect("constant QQ URL");
    url.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", MUSIC_APP_ID),
        ("redirect_uri", REDIRECT),
        ("scope", "get_user_info,get_app_friends"),
        ("state", state),
        ("display", "mobile"),
    ]);
    url
}

#[cfg(any(target_os = "ios", test))]
pub fn qq_resume_url(callback: &url::Url) -> Option<url::Url> {
    if callback.scheme() != "listener-qq-login" || callback.host_str() != Some("return") {
        return None;
    }
    let raw = callback.query()?.strip_prefix("url=")?;
    // QQ appends the browser destination to schemacallback. It can include its own query.
    let destination = if raw.starts_with("https://") {
        url::Url::parse(raw).ok()?
    } else {
        let (_, decoded) = callback.query_pairs().next()?;
        url::Url::parse(&decoded).ok()?
    };
    if destination.scheme() != "https"
        || !destination.username().is_empty()
        || destination.password().is_some()
        || destination.port().is_some()
    {
        return None;
    }
    match (destination.host_str()?, destination.path()) {
        ("ssl.ptlogin2.qq.com" | "ssl.ptlogin2.graph.qq.com", "/jump" | "/check_sig")
        | ("graph.qq.com", "/oauth2.0/authorize" | "/oauth2.0/login_jump")
        | ("connect.qq.com", "/")
        | ("y.qq.com", "/portal/wx_redirect.html") => Some(destination),
        _ => None,
    }
}

#[cfg(any(target_os = "ios", test))]
fn qq_callback_code(callback: &str, state: &str) -> Result<String> {
    let invalid = || AppError::new("QQ_AUTH_CALLBACK", "QQ 返回的授权结果无效，请重新登录。");
    let url = url::Url::parse(callback).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || url.host_str() != Some("y.qq.com")
        || url.path() != "/portal/wx_redirect.html"
    {
        return Err(invalid());
    }
    let mut codes = url.query_pairs().filter(|(key, _)| key == "code");
    let code = codes
        .next()
        .map(|(_, value)| value)
        .filter(|code| !code.is_empty())
        .ok_or_else(invalid)?;
    let mut states = url.query_pairs().filter(|(key, _)| key == "state");
    if states.next().as_ref().map(|(_, value)| value.as_ref()) != Some(state)
        || states.next().is_some()
        || codes.next().is_some()
    {
        return Err(invalid());
    }
    Ok(code.into_owned())
}

#[cfg(target_os = "ios")]
pub async fn authorize_from_callback(
    qq: &QqMusic,
    callback: &str,
    state: &str,
) -> Result<Credentials> {
    let code = qq_callback_code(callback, state)?;
    exchange_qq_code(qq, &code).await
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before 1970")
        .as_millis()
        .to_string()
}
fn hash33(value: &str, seed: u32) -> u32 {
    value
        .bytes()
        .fold(seed, |hash, c| hash.wrapping_mul(33).wrapping_add(c as u32))
        & 0x7fffffff
}
fn parse_callback(text: &str) -> Result<(u32, String)> {
    if !text.trim_start().starts_with("ptuiCB(") {
        return Err(AppError::protocol("QR callback"));
    }
    let regex = regex::Regex::new(r"'((?:\\.|[^'])*)'").expect("constant QQ callback expression");
    let args: Vec<String> = regex
        .captures_iter(text)
        .map(|c| c[1].to_string())
        .collect();
    let code = args
        .first()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::protocol("QR status"))?;
    let redirect = args
        .get(2)
        .ok_or_else(|| AppError::protocol("QR callback arguments"))?
        .to_owned();
    Ok((code, redirect))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quick_login_resumes_only_tencent_urls_without_losing_the_query() {
        let destination = "https://ssl.ptlogin2.qq.com/jump?x=one&y=two%2Bthree";
        let callback =
            url::Url::parse(&format!("listener-qq-login://return?url={destination}")).unwrap();
        assert_eq!(qq_resume_url(&callback).unwrap().as_str(), destination);

        let mut encoded = url::Url::parse("listener-qq-login://return").unwrap();
        encoded.query_pairs_mut().append_pair("url", destination);
        assert_eq!(qq_resume_url(&encoded).unwrap().as_str(), destination);

        let unrelated =
            url::Url::parse("listener-qq-login://return?url=https://example.com/jump").unwrap();
        assert!(qq_resume_url(&unrelated).is_none());
    }
    #[test]
    fn quick_login_accepts_only_the_matching_qq_music_callback() {
        let callback =
            "https://y.qq.com/portal/wx_redirect.html?login_type=1&code=one-time&state=attempt";
        assert_eq!(qq_callback_code(callback, "attempt").unwrap(), "one-time");
        assert!(qq_callback_code(callback, "other-attempt").is_err());
        assert!(
            qq_callback_code(&callback.replace("y.qq.com", "other.example"), "attempt").is_err()
        );
        assert!(qq_callback_code(&format!("{callback}&code=other"), "attempt").is_err());
    }
    #[test]
    fn parses_qr_states_without_evaluating_javascript() {
        assert_eq!(
            parse_callback("ptuiCB('66','0','','0','二维码未失效。', '');")
                .unwrap()
                .0,
            66
        );
        assert_eq!(
            parse_callback("ptuiCB('67','0','','0','正在验证二维码。', '');")
                .unwrap()
                .0,
            67
        );
        assert!(parse_callback("window.alert('0')").is_err());
        assert!(parse_callback("ptuiCB('oops')").is_err());
    }
    #[test]
    fn qq_hash_uses_unsigned_wrapping_arithmetic() {
        assert_eq!(hash33("abc", 0), 108966);
        assert_eq!(hash33("abc", 5381), 193485963);
    }
    #[tokio::test]
    #[ignore = "live QQ request; generates a new QR without signing in"]
    async fn live_qr_is_a_real_png_and_can_be_polled() {
        let (session, image) = QrSession::create().await.unwrap();
        assert!(image.image.starts_with("data:image/png;base64,iVBOR"));
        assert!(matches!(
            session.poll(&QqMusic::new().unwrap()).await.unwrap(),
            QrStatus::Waiting
        ));
    }
    #[tokio::test]
    #[ignore = "live QQ/WeChat requests; obtains QR images without authorizing an account"]
    async fn live_wechat_and_mobile_qrs() {
        let qq = QqMusic::new().unwrap();
        for kind in ["wx", "mobile"] {
            let (session, image) = QrSession::create_for(kind, &qq).await.unwrap();
            assert!(image.image.starts_with("data:image/"));
            if kind == "mobile" {
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
            assert!(matches!(
                session.poll(&qq).await.unwrap(),
                QrStatus::Waiting | QrStatus::Scanned
            ));
        }
    }
}
