use std::fmt;

use serde::Serialize;

/// An error the UI can show. `code` is stable and translated by the UI; `message`
/// is the English detail shown when the code has no translation.
#[derive(Debug, Clone, Serialize)]
pub struct SaveError {
    pub code: &'static str,
    pub message: String,
}

impl SaveError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn format(message: impl Into<String>) -> Self {
        Self::new("format", message)
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid", message)
    }

    pub fn incompatible(message: impl Into<String>) -> Self {
        Self::new("incompatible", message)
    }
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SaveError {}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        Self::new("io", e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SaveError>;
