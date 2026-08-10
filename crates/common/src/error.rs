use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferErrorCode {
    Unauthorized,
    InvalidRequest,
    DiskNotFound,
    DiskDisabled,
    DiskRemoved,
    InvalidStatus,
    RecoveryRequired,
    ProtocolVersionUnsupported,
    FilesystemUnsupported,
    KeyRevoked,
    SignatureInvalid,
    ManifestInvalid,
    ChecksumMismatch,
    DecryptFailed,
    NonceReused,
    SourceChanged,
    ChunkIndexOverflow,
    SealIdManifestMismatch,
    OrphanEdgeCopying,
    OrphanCenterImporting,
    PartialFileFound,
    PartialCleaned,
    PartialCleanFailed,
    InsufficientSpace,
    DiskFull,
    ReinitFailed,
}

impl TransferErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unauthorized => "UNAUTHORIZED",
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::DiskNotFound => "DISK_NOT_FOUND",
            Self::DiskDisabled => "DISK_DISABLED",
            Self::DiskRemoved => "DISK_REMOVED",
            Self::InvalidStatus => "INVALID_STATUS",
            Self::RecoveryRequired => "RECOVERY_REQUIRED",
            Self::ProtocolVersionUnsupported => "PROTOCOL_VERSION_UNSUPPORTED",
            Self::FilesystemUnsupported => "FILESYSTEM_UNSUPPORTED",
            Self::KeyRevoked => "KEY_REVOKED",
            Self::SignatureInvalid => "SIGNATURE_INVALID",
            Self::ManifestInvalid => "MANIFEST_INVALID",
            Self::ChecksumMismatch => "CHECKSUM_MISMATCH",
            Self::DecryptFailed => "DECRYPT_FAILED",
            Self::NonceReused => "NONCE_REUSED",
            Self::SourceChanged => "SOURCE_CHANGED",
            Self::ChunkIndexOverflow => "CHUNK_INDEX_OVERFLOW",
            Self::SealIdManifestMismatch => "SEAL_ID_MANIFEST_MISMATCH",
            Self::OrphanEdgeCopying => "ORPHAN_EDGE_COPYING",
            Self::OrphanCenterImporting => "ORPHAN_CENTER_IMPORTING",
            Self::PartialFileFound => "PARTIAL_FILE_FOUND",
            Self::PartialCleaned => "PARTIAL_CLEANED",
            Self::PartialCleanFailed => "PARTIAL_CLEAN_FAILED",
            Self::InsufficientSpace => "INSUFFICIENT_SPACE",
            Self::DiskFull => "DISK_FULL",
            Self::ReinitFailed => "REINIT_FAILED",
        }
    }
}

impl std::fmt::Display for TransferErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct TransferError {
    pub code: TransferErrorCode,
    pub message: String,
}

impl TransferError {
    pub fn new(code: TransferErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub type TransferResult<T> = Result<T, TransferError>;

impl From<std::io::Error> for TransferError {
    fn from(error: std::io::Error) -> Self {
        Self::new(TransferErrorCode::ManifestInvalid, error.to_string())
    }
}

impl From<serde_json::Error> for TransferError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(TransferErrorCode::ManifestInvalid, error.to_string())
    }
}
