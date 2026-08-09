#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferErrorCode {
    FilesystemUnsupported,
    ManifestInvalid,
    ChecksumMismatch,
    DecryptFailed,
    NonceReused,
}

