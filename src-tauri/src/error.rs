//! Unified error type for the Feader backend.
//!
//! Tauri commands serialize their error to the frontend, which only ever does
//! `String(error)`. To preserve that contract, `FeaderError` serializes as its
//! `Display` string rather than as a tagged object.

use serde::{Serialize, Serializer};

/// Crate-wide result alias. Prefer this over `Result<T, String>`.
pub type Result<T> = std::result::Result<T, FeaderError>;

/// Every fallible backend operation funnels into one of these categories.
#[derive(Debug, thiserror::Error)]
pub enum FeaderError {
    /// A hand-written, user-facing message (validation, business rules).
    #[error("{0}")]
    Message(String),

    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("feed network error: {0}")]
    FeedHttp(#[from] wreq::Error),

    #[error("invalid data: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<String> for FeaderError {
    fn from(value: String) -> Self {
        FeaderError::Message(value)
    }
}

impl From<&str> for FeaderError {
    fn from(value: &str) -> Self {
        FeaderError::Message(value.to_string())
    }
}

/// Flatten back to a string at the boundaries that persist or display errors as
/// text (e.g. `SourceRefreshResult.error`, `record_source_error`).
impl From<FeaderError> for String {
    fn from(value: FeaderError) -> Self {
        value.to_string()
    }
}

impl FeaderError {
    pub fn contains(&self, other: &str) -> bool {
        self.to_string().contains(other)
    }
}

impl Serialize for FeaderError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_a_plain_string_for_the_frontend() {
        let error = FeaderError::Message("尚未设置该插件的 cookie".to_string());
        let json = serde_json::to_string(&error).expect("serializes");
        assert_eq!(json, "\"尚未设置该插件的 cookie\"");
    }

    #[test]
    fn wrapped_errors_keep_a_readable_message() {
        let parse: Result<()> = serde_json::from_str::<i32>("not json")
            .map(|_| ())
            .map_err(FeaderError::from);
        let json = serde_json::to_string(&parse.unwrap_err()).expect("serializes");
        assert!(json.starts_with("\"invalid data:"), "got {json}");
    }

    #[test]
    fn string_messages_round_trip_unchanged() {
        let error: FeaderError = "Feed URL is required".into();
        assert_eq!(error.to_string(), "Feed URL is required");
    }
}
