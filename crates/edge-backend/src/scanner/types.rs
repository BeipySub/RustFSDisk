use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StableStatus {
    Unknown,
    Stable,
    Unstable,
    SourceChanged,
}

impl StableStatus {
    pub fn as_db_value(self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Stable => "STABLE",
            Self::Unstable => "UNSTABLE",
            Self::SourceChanged => "SOURCE_CHANGED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectSummary {
    pub bucket: String,
    pub object_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectHead {
    pub bucket: String,
    pub object_key: String,
    pub etag: String,
    pub size_bytes: i64,
    pub last_modified: DateTime<Utc>,
    pub metadata: BTreeMap<String, String>,
}

impl ObjectHead {
    pub fn has_same_identity(&self, other: &Self) -> bool {
        self.bucket == other.bucket
            && self.object_key == other.object_key
            && self.etag == other.etag
            && self.size_bytes == other.size_bytes
            && self.last_modified == other.last_modified
    }
}

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("rustfs operation failed: {0}")]
    RustFs(String),
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("object metadata is invalid: {0}")]
    InvalidMetadata(String),
}
