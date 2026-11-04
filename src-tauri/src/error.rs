use serde::{Serialize, Serializer};

/// One error type for the whole backend. It serializes to a plain string so the UI
/// can show exactly what went wrong.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("data error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("app error: {0}")]
    Tauri(#[from] tauri::Error),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<String> for AppError {
    fn from(m: String) -> Self {
        AppError::Msg(m)
    }
}

impl From<&str> for AppError {
    fn from(m: &str) -> Self {
        AppError::Msg(m.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

pub fn fail<T>(m: impl Into<String>) -> Result<T> {
    Err(AppError::Msg(m.into()))
}
