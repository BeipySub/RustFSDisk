//! Shared protocol structures for disk files, center HTTP APIs, and local WebSocket events.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{TransferError, TransferErrorCode, TransferResult};

#[cfg(unix)]
use std::fs::File;

pub const PROTOCOL_NAME: &str = "rustfs-offline-transfer";
pub const PROTOCOL_VERSION: &str = "1.0.0";
pub const MANIFEST_VERSION: &str = "1.0.0";
pub const TRANSFER_ROOT: &str = "/rustfs-transfer/";
pub const PROTOCOL_ROOT_DIR: &str = "rustfs-transfer";
pub const DISK_INFO_PATH: &str = "disk_info.json";
pub const EXPORT_MANIFEST_PATH: &str = "manifests/export_manifest.json";
pub const EXPORT_MANIFEST_SHA256_PATH: &str = "manifests/export_manifest.sha256";
pub const MANIFEST_PATH: &str = EXPORT_MANIFEST_PATH;
pub const MANIFEST_SHA256_PATH: &str = EXPORT_MANIFEST_SHA256_PATH;
pub const DATA_DIR: &str = "data";
pub const META_DIR: &str = "meta";
pub const MANIFESTS_DIR: &str = "manifests";
pub const LOGS_DIR: &str = "logs";
pub const QUARANTINE_PARTIAL_DIR: &str = "quarantine/partial";
pub const ENCRYPTION_ALG_AES_256_GCM: &str = "AES-256-GCM";
pub const SIGNATURE_ALG_HMAC_SHA256: &str = "HMAC-SHA256";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiskStatusCode {
    Unregistered,
    Registered,
    Initialized,
    EdgeCopying,
    Sealed,
    CenterImporting,
    Imported,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ObjectStatus {
    Pending,
    Assigned,
    Copying,
    Exported,
    Failed,
    SourceChanged,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeStatus {
    Detected,
    Checking,
    Ready,
    Copying,
    Cleaning,
    Reinitializing,
    Done,
    Rejected,
    Removed,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DataKeyStatus {
    Active,
    Issued,
    SealedReadonly,
    Retired,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EdgeStatus {
    Active,
    Disabled,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExportJobStatus {
    Pending,
    Scanning,
    Copying,
    Sealing,
    Sealed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ImportJobStatus {
    Pending,
    Importing,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChunkImportGroupStatus {
    WaitingParts,
    ReadyToMerge,
    Merging,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChunkImportPartStatus {
    Registered,
    Verified,
    Merged,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebSocketEventType {
    DiskDetected,
    DiskRemoved,
    DiskChecking,
    DiskReady,
    DiskRejected,
    ScanStarted,
    ScanProgress,
    ScanDone,
    CopyStarted,
    CopyProgress,
    CopyDone,
    SealDone,
    ImportStarted,
    ImportProgress,
    ImportDone,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventSource {
    Edge,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiskVerifyAction {
    AllowExport,
    Reject,
    NeedInit,
    NeedImportFirst,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskInfo {
    pub protocol: DiskInfoProtocol,
    pub disk: DiskInfoDisk,
    pub status: DiskInfoStatus,
    pub edge: DiskInfoEdge,
    pub center: DiskInfoCenter,
    pub manifest: DiskInfoManifest,
    pub security: DiskInfoSecurity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskInfoProtocol {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskInfoDisk {
    pub sn: String,
    pub disk_id: String,
    pub capacity_bytes: u64,
    pub last_init_time: String,
    pub initialized_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskInfoStatus {
    pub code: DiskStatusCode,
    pub sealed: bool,
    pub imported: bool,
    pub reusable: bool,
    pub last_error: String,
}

impl DiskInfoStatus {
    pub fn from_code(code: DiskStatusCode, last_error: impl Into<String>) -> Self {
        Self {
            code,
            sealed: matches!(code, DiskStatusCode::Sealed),
            imported: matches!(code, DiskStatusCode::Imported),
            reusable: matches!(code, DiskStatusCode::Imported),
            last_error: last_error.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskInfoEdge {
    pub edge_name: String,
    pub edge_code: String,
    pub seal_id: String,
    pub export_job_id: String,
    pub export_started_at: String,
    pub export_finished_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskInfoCenter {
    pub center_id: String,
    pub import_job_id: String,
    pub import_started_at: String,
    pub import_finished_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskInfoManifest {
    pub manifest_path: String,
    pub manifest_sha256_path: String,
    pub object_count: u64,
    pub total_bytes: u64,
    pub manifest_sha256: String,
}

impl Default for DiskInfoManifest {
    fn default() -> Self {
        Self {
            manifest_path: EXPORT_MANIFEST_PATH.to_owned(),
            manifest_sha256_path: EXPORT_MANIFEST_SHA256_PATH.to_owned(),
            object_count: 0,
            total_bytes: 0,
            manifest_sha256: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskInfoSecurity {
    pub center_signature: String,
    pub signature_alg: String,
    pub center_key_id: String,
    pub encryption_alg: String,
    pub data_key_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportManifest {
    pub manifest_version: String,
    pub seal_id: String,
    pub export_job_id: String,
    pub disk_id: String,
    pub edge_code: String,
    pub create_time: String,
    pub objects: Vec<ManifestObject>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestObject {
    pub bucket: String,
    pub key: String,
    pub relative_data_path: String,
    pub encrypted: bool,
    pub encryption_alg: String,
    pub data_key_id: String,
    pub nonce: String,
    pub tag: String,
    pub aad: String,
    pub ciphertext_size_bytes: u64,
    pub ciphertext_sha256: String,
    pub chunked: bool,
    pub chunk_group_id: String,
    pub chunk_index: u32,
    pub chunk_total: u32,
    pub chunk_offset_bytes: u64,
    pub chunk_size_bytes: u64,
    pub chunk_sha256: String,
    pub relative_meta_path: String,
    pub size_bytes: u64,
    pub etag: String,
    pub last_modified: String,
    pub content_type: String,
    pub metadata: BTreeMap<String, String>,
    pub plaintext_sha256: String,
    pub exported_at: String,
    pub object_status: ObjectStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeAuthRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeAuthResponse {
    pub allowed: bool,
    pub edge_code: String,
    pub edge_name: String,
    pub edge_status: EdgeStatus,
    pub server_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskVerifyRequest {
    pub edge_code: String,
    pub disk_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sn: Option<String>,
    pub capacity_bytes: u64,
    pub free_bytes: u64,
    pub status_code: DiskStatusCode,
    pub protocol_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskVerifyResponse {
    pub allowed: bool,
    pub disk_id: String,
    pub disk_enabled: bool,
    pub expected_status: DiskStatusCode,
    pub action: DiskVerifyAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskExportKeyRequest {
    pub edge_code: String,
    pub disk_id: String,
    pub data_key_id: String,
    pub export_job_id: String,
    pub status_code: DiskStatusCode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskExportKeyResponse {
    pub allowed: bool,
    pub data_key_id: String,
    pub encryption_alg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_data_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskDetailResponse {
    pub disk_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sn: Option<String>,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_init_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remark: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub error_code: crate::error::TransferErrorCode,
    pub message: String,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyProgressEvent {
    pub event_type: WebSocketEventType,
    pub event_time: String,
    pub source: EventSource,
    pub edge_code: String,
    pub export_job_id: String,
    pub disk_status_code: DiskStatusCode,
    pub export_job_status: ExportJobStatus,
    pub global_progress: ProgressSummary,
    pub disks: Vec<DiskCopyProgress>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportProgressEvent {
    pub event_type: WebSocketEventType,
    pub event_time: String,
    pub source: EventSource,
    pub edge_code: String,
    pub import_job_id: String,
    pub disk_status_code: DiskStatusCode,
    pub import_job_status: ImportJobStatus,
    pub global_progress: ProgressSummary,
    pub disks: Vec<DiskImportProgress>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressSummary {
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskCopyProgress {
    pub disk_id: String,
    pub disk_sn: String,
    pub mount_path: String,
    pub runtime_status: RuntimeStatus,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub free_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_object: Option<CurrentObjectProgress>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskImportProgress {
    pub disk_id: String,
    pub disk_sn: String,
    pub mount_path: String,
    pub runtime_status: RuntimeStatus,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_object: Option<CurrentObjectProgress>,
    pub reusable: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentObjectProgress {
    pub bucket: String,
    pub key: String,
    pub display_name: String,
    pub relative_data_path: String,
    pub size_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_status: ObjectStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialScan {
    pub count: usize,
    pub bytes: u64,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TransferDisk {
    root: PathBuf,
}

impl TransferDisk {
    pub fn new(mount_path: impl AsRef<Path>) -> Self {
        Self {
            root: mount_path.as_ref().join(PROTOCOL_ROOT_DIR),
        }
    }

    pub fn from_protocol_root(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ensure_layout(&self) -> TransferResult<()> {
        for relative in [
            DATA_DIR,
            META_DIR,
            MANIFESTS_DIR,
            LOGS_DIR,
            QUARANTINE_PARTIAL_DIR,
        ] {
            fs::create_dir_all(self.resolve_relative(relative)?)?;
        }
        sync_dir(&self.root)?;
        Ok(())
    }

    pub fn read_disk_info(&self) -> TransferResult<DiskInfo> {
        let bytes = fs::read(self.resolve_relative(DISK_INFO_PATH)?)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn write_disk_info(&self, disk_info: &DiskInfo) -> TransferResult<()> {
        self.write_json_atomic(DISK_INFO_PATH, disk_info)
    }

    pub fn read_manifest(&self) -> TransferResult<ExportManifest> {
        let manifest_path = self.resolve_relative(MANIFEST_PATH)?;
        let expected = fs::read_to_string(self.resolve_relative(MANIFEST_SHA256_PATH)?)?;
        let bytes = fs::read(&manifest_path)?;
        let actual = sha256_hex(&bytes);

        if expected.trim().to_ascii_lowercase() != actual {
            return Err(TransferError::new(
                TransferErrorCode::ChecksumMismatch,
                "manifest SHA256 does not match export_manifest.sha256",
            ));
        }

        let manifest: ExportManifest = serde_json::from_slice(&bytes)?;
        validate_manifest(&manifest)?;
        Ok(manifest)
    }

    pub fn write_manifest(&self, manifest: &ExportManifest) -> TransferResult<String> {
        validate_manifest(manifest)?;
        let bytes = serde_json::to_vec_pretty(manifest)?;
        let sha256 = sha256_hex(&bytes);
        self.write_bytes_atomic(MANIFEST_PATH, &bytes, TempFileKind::Protocol)?;
        self.write_bytes_atomic(
            MANIFEST_SHA256_PATH,
            format!("{sha256}\n").as_bytes(),
            TempFileKind::Protocol,
        )?;
        Ok(sha256)
    }

    pub fn read_metadata<T>(&self, relative_path: &str) -> TransferResult<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        require_under(relative_path, META_DIR)?;
        let bytes = fs::read(self.resolve_relative(relative_path)?)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn write_metadata<T>(&self, relative_path: &str, metadata: &T) -> TransferResult<()>
    where
        T: Serialize,
    {
        require_under(relative_path, META_DIR)?;
        self.write_json_atomic(relative_path, metadata)
    }

    pub fn write_object_atomic(
        &self,
        relative_data_path: &str,
        bytes: &[u8],
    ) -> TransferResult<()> {
        require_under(relative_data_path, DATA_DIR)?;
        self.write_bytes_atomic(relative_data_path, bytes, TempFileKind::ObjectPartial)
    }

    pub fn scan_partials(&self) -> TransferResult<PartialScan> {
        let mut scan = PartialScan {
            count: 0,
            bytes: 0,
            paths: Vec::new(),
        };
        if !self.root.exists() {
            return Ok(scan);
        }
        self.scan_partials_in(&self.root, &mut scan)?;
        scan.paths.sort();
        Ok(scan)
    }

    fn write_json_atomic<T>(&self, relative_path: &str, value: &T) -> TransferResult<()>
    where
        T: Serialize,
    {
        let bytes = serde_json::to_vec_pretty(value)?;
        self.write_bytes_atomic(relative_path, &bytes, TempFileKind::Protocol)
    }

    fn write_bytes_atomic(
        &self,
        relative_path: &str,
        bytes: &[u8],
        kind: TempFileKind,
    ) -> TransferResult<()> {
        let final_path = self.resolve_relative(relative_path)?;
        let parent = final_path.parent().ok_or_else(|| {
            TransferError::new(
                TransferErrorCode::ManifestInvalid,
                "protocol file path has no parent directory",
            )
        })?;
        fs::create_dir_all(parent)?;

        let temp_path = temp_path_for(&final_path, kind);
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);

        fs::rename(&temp_path, &final_path)?;
        sync_file(&final_path)?;
        sync_dir(parent)?;
        Ok(())
    }

    fn resolve_relative(&self, relative_path: &str) -> TransferResult<PathBuf> {
        validate_relative_path(relative_path)?;
        Ok(self.root.join(relative_path))
    }

    fn scan_partials_in(&self, dir: &Path, scan: &mut PartialScan) -> TransferResult<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                self.scan_partials_in(&path, scan)?;
                continue;
            }

            if path.extension() == Some(OsStr::new("partial")) {
                let metadata = entry.metadata()?;
                scan.count += 1;
                scan.bytes += metadata.len();
                scan.paths.push(relative_string(&self.root, &path)?);
            }
        }
        Ok(())
    }
}

pub fn validate_relative_path(relative_path: &str) -> TransferResult<()> {
    if relative_path.is_empty() {
        return Err(manifest_invalid("relative path must not be empty"));
    }
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err(manifest_invalid("relative path must not be absolute"));
    }

    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(manifest_invalid(
                    "relative path must not escape protocol root",
                ));
            }
        }
    }

    if relative_path
        .replace('\\', "/")
        .split('/')
        .any(|part| part == "..")
    {
        return Err(manifest_invalid("relative path must not contain '..'"));
    }

    Ok(())
}

pub fn validate_manifest(manifest: &ExportManifest) -> TransferResult<()> {
    for object in &manifest.objects {
        if object.object_status != ObjectStatus::Exported {
            return Err(manifest_invalid(
                "only EXPORTED objects are allowed in export_manifest.json",
            ));
        }
        require_under(&object.relative_data_path, DATA_DIR)?;
        require_under(&object.relative_meta_path, META_DIR)?;
        reject_partial_path(&object.relative_data_path)?;
        reject_partial_path(&object.relative_meta_path)?;
    }
    Ok(())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn require_under(relative_path: &str, expected_dir: &str) -> TransferResult<()> {
    validate_relative_path(relative_path)?;
    let normalized = relative_path.replace('\\', "/");
    let prefix = format!("{expected_dir}/");
    if normalized == expected_dir || !normalized.starts_with(&prefix) {
        return Err(manifest_invalid(format!(
            "relative path must be under {expected_dir}/"
        )));
    }
    Ok(())
}

fn reject_partial_path(relative_path: &str) -> TransferResult<()> {
    if relative_path.ends_with(".partial") {
        return Err(manifest_invalid(".partial paths must not enter manifest"));
    }
    Ok(())
}

fn manifest_invalid(message: impl Into<String>) -> TransferError {
    TransferError::new(TransferErrorCode::ManifestInvalid, message)
}

fn temp_path_for(final_path: &Path, kind: TempFileKind) -> PathBuf {
    match kind {
        TempFileKind::Protocol => final_path.with_file_name(format!(
            ".{}.{}.tmp",
            final_path
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("protocol"),
            Uuid::new_v4()
        )),
        TempFileKind::ObjectPartial => final_path.with_extension(format!(
            "{}.partial",
            final_path
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or("rustfs-transfer")
        )),
    }
}

#[cfg(unix)]
fn sync_file(path: &Path) -> TransferResult<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_file(_path: &Path) -> TransferResult<()> {
    Ok(())
}

#[cfg(unix)]
fn sync_dir(path: &Path) -> TransferResult<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_dir(_path: &Path) -> TransferResult<()> {
    Ok(())
}

fn relative_string(root: &Path, path: &Path) -> TransferResult<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|error| manifest_invalid(error.to_string()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

#[derive(Debug, Clone, Copy)]
enum TempFileKind {
    Protocol,
    ObjectPartial,
}
