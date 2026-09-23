use serde::Serialize;
use std::fmt::Display;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub detail: String,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: detail.into(),
        }
    }

    pub fn internal(detail: impl Display) -> Self {
        Self::new(
            "internal",
            "The operation could not be completed.",
            detail.to_string(),
        )
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new("validation", message, "")
    }

    pub fn not_found(entity: &str) -> Self {
        Self::new("notFound", format!("{entity} was not found."), "")
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.detail.is_empty() {
            write!(formatter, "{}: {}", self.code, self.message)
        } else {
            write!(
                formatter,
                "{}: {} ({})",
                self.code, self.message, self.detail
            )
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::internal(value)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::internal(value)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::internal(value)
    }
}
