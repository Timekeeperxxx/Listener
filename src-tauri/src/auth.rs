use crate::error::{AppError, Result};
use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashMap;

const SERVICE: &str = "com.listener.music.qqmusic";
const ACCOUNT: &str = "session";

#[derive(Clone)]
pub struct Credentials {
    pub cookie: String,
    pub uin: String,
    pub music_key: String,
    pub login_type: u8,
    pub refresh: Option<Value>,
}

impl Credentials {
    pub fn parse(cookie: String) -> Result<Self> {
        let pairs: HashMap<&str, &str> = cookie
            .split(';')
            .filter_map(|pair| pair.trim().split_once('='))
            .collect();
        let uin_key = if pairs.get("login_type") == Some(&"2") {
            "wxuin"
        } else {
            "uin"
        };
        let raw_uin = pairs
            .get(uin_key)
            .or_else(|| pairs.get("qqmusic_uin"))
            .ok_or_else(|| AppError::new("LOGIN_INCOMPLETE", "请先在 QQ 音乐窗口中完成登录。"))?;
        let uin = raw_uin
            .trim_start_matches('o')
            .trim_start_matches('0')
            .to_string();
        if uin.is_empty() || !uin.bytes().all(|b| b.is_ascii_digit()) {
            return Err(AppError::new(
                "LOGIN_INCOMPLETE",
                "QQ 音乐尚未返回有效账户。",
            ));
        }
        let music_key = ["qm_keyst", "qqmusic_key", "music_key", "wxskey"]
            .iter()
            .find_map(|key| pairs.get(key).filter(|v| !v.is_empty()))
            .ok_or_else(|| AppError::new("LOGIN_INCOMPLETE", "正在等待 QQ 音乐完成账户授权。"))?
            .to_string();
        let login_type = pairs
            .get("tmeLoginType")
            .map(|value| value.parse::<u8>())
            .transpose()
            .map_err(|_| AppError::new("INVALID_CREDENTIAL", "登录类型无效，请重新扫码。"))?
            .unwrap_or(2);
        let refresh = pairs
            .get("listener_refresh")
            .map(|value| {
                let raw = base64::engine::general_purpose::STANDARD
                    .decode(value)
                    .map_err(|_| {
                        AppError::new("INVALID_CREDENTIAL", "续期凭据损坏，请重新扫码。")
                    })?;
                Ok::<Value, AppError>(serde_json::from_slice(&raw)?)
            })
            .transpose()?;
        Ok(Self {
            cookie,
            uin,
            music_key,
            login_type,
            refresh,
        })
    }
    pub fn from_login(data: Value, login_type: u8) -> Result<Self> {
        let uin = data
            .get("musicid")
            .or_else(|| data.get("str_musicid"))
            .and_then(|value| {
                value
                    .as_u64()
                    .map(|n| n.to_string())
                    .or_else(|| value.as_str().map(str::to_string))
            })
            .ok_or_else(|| AppError::protocol("musicid"))?;
        let key = data["musickey"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::protocol("musickey"))?;
        let payload = base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&data)?);
        Self::parse(format!("uin={uin}; qqmusic_uin={uin}; qm_keyst={key}; qqmusic_key={key}; tmeLoginType={login_type}; listener_refresh={payload}"))
    }
    pub fn refresh_param(&self) -> Result<Value> {
        let data = self.refresh.as_ref().ok_or_else(|| {
            AppError::new("AUTH_EXPIRED", "此旧登录尚无续期凭据，请重新扫码一次。")
        })?;
        if !["refresh_key", "refresh_token"]
            .iter()
            .any(|key| data[*key].as_str().is_some_and(|value| !value.is_empty()))
        {
            return Err(AppError::new(
                "AUTH_EXPIRED",
                "QQ 未提供可用的续期凭据，请重新扫码。",
            ));
        }
        let mut param = json!({"musicid":self.uin.parse::<u64>().map_err(|_| AppError::protocol("musicid"))?,"str_musicid":self.uin,"musickey":self.music_key,"loginMode":2});
        for key in [
            "openid",
            "access_token",
            "refresh_token",
            "expired_in",
            "unionid",
            "refresh_key",
        ] {
            let source = if key == "expired_in" {
                "expired_at"
            } else {
                key
            };
            if let Some(value) = data.get(source).or_else(|| data.get(key)) {
                param[key] = value.clone();
            }
        }
        Ok(param)
    }
    pub fn expires_soon(&self) -> bool {
        let Some(data) = &self.refresh else {
            return false;
        };
        let Some(created) = data["musickeyCreateTime"].as_u64() else {
            return false;
        };
        let Some(lifetime) = data["keyExpiresIn"].as_u64().filter(|n| *n > 0) else {
            return false;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock before 1970")
            .as_secs();
        now.saturating_add(300) >= created.saturating_add(lifetime)
    }
    pub fn comm(&self) -> serde_json::Value {
        serde_json::json!({ "uin": self.uin, "authst": self.music_key, "format": "json", "ct": 19, "cv": 0, "tmeLoginType": self.login_type })
    }
    pub fn cookie_header(&self) -> String {
        self.cookie
            .split(';')
            .map(str::trim)
            .filter(|pair| !pair.starts_with("listener_refresh="))
            .collect::<Vec<_>>()
            .join("; ")
    }
    /// Encrypted UIN used by the musicasset read APIs. The value arrives with
    /// the login response and is kept in the refresh payload; older imported
    /// cookies may carry it as `euin`.
    pub fn encrypt_uin(&self) -> String {
        self.refresh
            .as_ref()
            .and_then(|value| value["encryptUin"].as_str())
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                self.cookie.split(';').find_map(|pair| {
                    let (key, value) = pair.trim().split_once('=')?;
                    (key == "euin" && !value.is_empty()).then(|| value.to_owned())
                })
            })
            .unwrap_or_default()
    }
    pub fn csrf_token(&self) -> String {
        (self.music_key.bytes().fold(5381u32, |hash, c| {
            hash.wrapping_mul(33).wrapping_add(c as u32)
        }) & 0x7fffffff)
            .to_string()
    }
}

fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|_| AppError::new("KEYCHAIN_ERROR", "无法访问 macOS 钥匙串。"))
}

pub fn load() -> Result<Option<Credentials>> {
    match entry()?.get_password() {
        Ok(cookie) => Credentials::parse(cookie).map(Some),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(AppError::new(
            "KEYCHAIN_ERROR",
            "无法读取 macOS 钥匙串中的登录信息。",
        )),
    }
}

pub fn save(credentials: &Credentials) -> Result<()> {
    entry()?.set_password(&credentials.cookie).map_err(|_| {
        AppError::new(
            "KEYCHAIN_ERROR",
            "登录信息未能保存到 macOS 钥匙串，请重新连接。",
        )
    })
}

pub fn delete() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(AppError::new(
            "KEYCHAIN_ERROR",
            "无法从 macOS 钥匙串移除登录信息。",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renewal_metadata_round_trips_only_through_keychain_payload() {
        let credentials=Credentials::from_login(json!({"musicid":123,"musickey":"test-key","refresh_key":"test-refresh","openid":"test-open"}),2).unwrap();
        let loaded = Credentials::parse(credentials.cookie.clone()).unwrap();
        assert_eq!(
            loaded.refresh_param().unwrap()["refresh_key"],
            "test-refresh"
        );
        assert!(!loaded.cookie_header().contains("listener_refresh"));
        assert!(!loaded.cookie_header().contains("test-refresh"));
        assert_eq!(loaded.uin, "123");
    }
    #[test]
    fn encrypted_uin_stays_in_the_keychain_payload() {
        let credentials = Credentials::from_login(
            json!({"musicid":123,"musickey":"test-key","encryptUin":"encrypted-uin"}),
            2,
        )
        .unwrap();
        assert_eq!(credentials.encrypt_uin(), "encrypted-uin");
        let imported = Credentials::parse("uin=123; qm_keyst=test-key; euin=cookie-uin".into())
            .unwrap();
        assert_eq!(imported.encrypt_uin(), "cookie-uin");
    }
    #[test]
    fn legacy_sessions_need_a_new_scan_instead_of_invented_refresh_tokens() {
        let credentials = Credentials::parse("uin=123;qm_keyst=old-key".into()).unwrap();
        assert_eq!(
            credentials.refresh_param().unwrap_err().code,
            "AUTH_EXPIRED"
        );
    }
    #[test]
    fn requires_music_session_not_just_qq_login() {
        assert!(Credentials::parse("uin=o0123456; p_skey=qq-only".into()).is_err());
        let parsed = Credentials::parse("uin=o0123456; qm_keyst=music-ticket".into()).unwrap();
        assert_eq!(parsed.uin, "123456");
    }
    #[test]
    fn wechat_identity_is_not_the_qq_uin() {
        let parsed =
            Credentials::parse("uin=123; wxuin=456; login_type=2; wxskey=music-ticket".into())
                .unwrap();
        assert_eq!(parsed.uin, "456");
    }
}
