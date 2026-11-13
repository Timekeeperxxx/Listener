use crate::error::{AppError, Result};
use serde_json::Value;

#[cfg(target_os = "ios")]
use crate::{
    auth::Credentials, begin_login, complete_login, models::Profile, AppState,
};
#[cfg(target_os = "ios")]
use serde_json::json;
#[cfg(target_os = "ios")]
use std::sync::atomic::Ordering;
use url::Url;

#[cfg(target_os = "ios")]
const LOGIN_SERVER: &str = "music.login.LoginServer";

#[cfg(target_os = "ios")]
#[derive(Clone)]
pub(crate) struct SmsLogin {
    phone: String,
    session_generation: u64,
    challenge: Option<Url>,
}

fn challenge_url(block: &Value) -> Option<Url> {
    let code = block["code"].as_i64()?;
    if !matches!(code, 20276 | 100001) {
        return None;
    }
    let url = Url::parse(block["data"]["securityURL"].as_str()?).ok()?;
    let host = url.host_str()?;
    let trusted = ["qq.com", "qcloud.com", "tencent.com"]
        .iter()
        .any(|domain| {
            host == *domain
                || host
                    .strip_suffix(*domain)
                    .is_some_and(|part| part.ends_with('.'))
        });
    (trusted
        && url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none())
    .then_some(url)
}

fn validate_phone(phone: &str) -> Result<()> {
    if phone.len() == 11 && phone.starts_with('1') && phone.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(());
    }
    Err(AppError::new(
        "INVALID_PHONE",
        "请输入 11 位中国大陆手机号。",
    ))
}

fn validate_code(code: &str) -> Result<()> {
    if code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(());
    }
    Err(AppError::new("INVALID_SMS_CODE", "请输入 6 位短信验证码。"))
}

fn check_sms_reply(block: &Value, sending: bool) -> Result<()> {
    let code = block["code"]
        .as_i64()
        .ok_or_else(|| AppError::protocol("code"))?;
    let has_challenge = block["data"]["securityURL"]
        .as_str()
        .is_some_and(|url| !url.is_empty());
    let (error, message) = match code {
        0 => return Ok(()),
        20276 | 100001 if has_challenge || code == 20276 => (
            "SMS_SECURITY_CHECK",
            "QQ 音乐要求先完成人机验证，但没有提供可打开的验证页面。",
        ),
        100001 | 100002 | 104604 => ("SMS_RATE_LIMIT", "请求过于频繁，请稍后再试。"),
        1000 | 104400 | 104401 if !sending => ("SMS_CODE_EXPIRED", "验证码已失效，请重新发送。"),
        20271 => ("SMS_WRONG_CODE", "验证码不正确，请检查后重试。"),
        20272 | 20274 => ("SMS_ACCOUNT_BINDING", "该手机号的 QQ 音乐账号绑定异常。"),
        20277 | 20278 | 20450 => ("SMS_ACCOUNT_RESTRICTED", "该 QQ 音乐账号的登录受限。"),
        20279 => ("SMS_DEVICE_LIMIT", "该 QQ 音乐账号的登录设备数已达上限。"),
        _ => {
            let action = if sending {
                "发送验证码"
            } else {
                "验证短信验证码"
            };
            return Err(AppError::new(
                "SMS_REQUEST_FAILED",
                format!("QQ 音乐{action}失败（代码 {code}）。"),
            ));
        }
    };
    Err(AppError::new(error, message))
}

#[cfg(target_os = "ios")]
fn cancelled() -> AppError {
    AppError::new("LOGIN_CANCELLED", "本次短信登录已取消，请重新发送验证码。")
}

#[cfg(target_os = "ios")]
#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub(crate) enum SmsSendResult {
    Sent { generation: u64 },
    Challenge { generation: u64 },
}

#[cfg(target_os = "ios")]
pub(crate) async fn send_login_sms_core(state: &AppState, phone: String) -> Result<SmsSendResult> {
    validate_phone(&phone)?;
    let generation = begin_login(state)?;
    let session_generation = state.session_lock()?.generation;
    let block = state
        .qq
        .rpc_block(
            None,
            json!({"ct": 11, "cv": 20090008, "tmeLoginMethod": 3}),
            LOGIN_SERVER,
            "SendPhoneAuthCode",
            json!({"tmeAppid": "qqmusic", "areaCode": "86", "phoneNo": phone}),
        )
        .await?;
    let challenge = challenge_url(&block);
    let outcome = if challenge.is_some() {
        SmsSendResult::Challenge { generation }
    } else {
        check_sms_reply(&block, true)?;
        SmsSendResult::Sent { generation }
    };
    let mut login = state.login_lock()?;
    if state.login_generation.load(Ordering::SeqCst) != generation {
        return Err(cancelled());
    }
    if state.session_lock()?.generation != session_generation {
        return Err(AppError::new("SESSION_CHANGED", "账户已变化，请重新连接。"));
    }
    login.sms = Some(SmsLogin {
        phone,
        session_generation,
        challenge,
    });
    Ok(outcome)
}

#[cfg(target_os = "ios")]
fn pending_sms(state: &AppState, generation: u64) -> Result<SmsLogin> {
    let login = state.login_lock()?;
    if state.login_generation.load(Ordering::SeqCst) != generation {
        return Err(cancelled());
    }
    let sms = login.sms.clone().ok_or_else(cancelled)?;
    if state.session_lock()?.generation != sms.session_generation {
        return Err(AppError::new("SESSION_CHANGED", "账户已变化，请重新连接。"));
    }
    Ok(sms)
}

#[cfg(target_os = "ios")]
pub(crate) fn sms_challenge(state: &AppState, generation: u64) -> Result<url::Url> {
    pending_sms(state, generation)?.challenge.ok_or_else(cancelled)
}

#[cfg(target_os = "ios")]
pub(crate) async fn verify_login_sms_core(state: &AppState, generation: u64, code: String) -> Result<Profile> {
    validate_code(&code)?;
    let sms = pending_sms(state, generation)?;
    let mut block = state
        .qq
        .rpc_block(
            None,
            json!({"ct": 11, "cv": 20090008, "tmeLoginMethod": 3, "tmeLoginType": 0}),
            LOGIN_SERVER,
            "Login",
            json!({"code": code, "loginMode": 1, "phoneNo": sms.phone}),
        )
        .await?;
    check_sms_reply(&block, false)?;
    let data = block["data"]
        .as_object_mut()
        .ok_or_else(|| AppError::protocol("data"))?;
    let login_type = data
        .get("loginType")
        .and_then(Value::as_u64)
        .and_then(|kind| u8::try_from(kind).ok())
        .ok_or_else(|| AppError::protocol("loginType"))?;
    for field in [
        "phoneNo",
        "encryptedPhoneNo",
        "otherPhoneNo",
        "otherAccounts",
    ] {
        data.remove(field);
    }
    let credentials = Credentials::from_login(block["data"].take(), login_type)?;
    complete_login(
        state,
        generation,
        sms.session_generation,
        None,
        credentials,
    )
    .await?
    .ok_or_else(cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_phone_and_preserves_leading_zero_in_code() {
        assert!(validate_phone("13000000000").is_ok());
        assert!(validate_phone("０1300000000").is_err());
        assert!(validate_phone("+8613000000000").is_err());
        assert!(validate_code("012345").is_ok());
        assert!(validate_code("12345").is_err());
    }

    #[test]
    fn opens_only_https_tencent_challenge_urls() {
        for (url, accepted) in [
            ("https://captcha.qq.com/verify?context=abc", true),
            ("https://captcha.qcloud.com/verify", true),
            ("https://qq.com.attacker.test/verify", false),
            ("https://attacker.test@captcha.qq.com/verify", false),
            ("http://captcha.qq.com/verify", false),
        ] {
            let reply = json!({"code": 20276, "data": {"securityURL": url}});
            assert_eq!(challenge_url(&reply).is_some(), accepted);
        }
    }

    #[test]
    fn requires_explicit_success_and_distinguishes_challenges_from_rate_limits() {
        assert!(check_sms_reply(&json!({"code": 0}), true).is_ok());
        assert_eq!(
            check_sms_reply(&json!({}), true).unwrap_err().code,
            "QQ_RESPONSE_CHANGED"
        );
        assert_eq!(
            check_sms_reply(
                &json!({"code": 100001, "data": {"securityURL": "https://example.test/"}}),
                true
            )
            .unwrap_err()
            .code,
            "SMS_SECURITY_CHECK"
        );
        assert_eq!(
            check_sms_reply(&json!({"code": 100001}), true)
                .unwrap_err()
                .code,
            "SMS_RATE_LIMIT"
        );
        assert_eq!(
            check_sms_reply(&json!({"code": 20271}), false)
                .unwrap_err()
                .code,
            "SMS_WRONG_CODE"
        );
        assert_eq!(
            check_sms_reply(&json!({"code": 1000}), false)
                .unwrap_err()
                .code,
            "SMS_CODE_EXPIRED"
        );
    }
}
