use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, AppError>;

impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }
    pub fn protocol(field: &str) -> Self {
        Self::new(
            "QQ_RESPONSE_CHANGED",
            format!("QQ 音乐返回的数据缺少 {field}，请稍后重试。"),
        )
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for AppError {}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        // URLs can contain playback tokens. Never include them in errors or logs.
        if error.is_timeout() {
            Self::new("NETWORK_TIMEOUT", "连接 QQ 音乐超时，请检查网络后重试。")
        } else if let Some(status) = error.status() {
            Self::new("HTTP_ERROR", format!("QQ 音乐请求失败（HTTP {status}）。"))
        } else {
            Self::new("NETWORK_ERROR", "无法连接 QQ 音乐，请检查网络后重试。")
        }
    }
}
impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        Self::new("DATABASE_ERROR", format!("本地资料库读写失败：{error}"))
    }
}
impl From<serde_json::Error> for AppError {
    fn from(_: serde_json::Error) -> Self {
        Self::new("INVALID_DATA", "数据格式无法解析。")
    }
}
impl From<tauri::Error> for AppError {
    fn from(error: tauri::Error) -> Self {
        Self::new("WINDOW_ERROR", format!("窗口操作失败：{error}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::new("FILE_ERROR", format!("本地文件读写失败：{error}"))
    }
}
