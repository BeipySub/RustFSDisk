use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    process::Command,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use thiserror::Error;
use uuid::Uuid;

pub const PROTOCOL_ROOT: &str = "rustfs-transfer";
pub const DISK_INFO_FILE: &str = "disk_info.json";
pub const SUPPORTED_FILESYSTEM: &str = "ext4";
pub const SUPPORTED_PROTOCOL_VERSION: &str = "1.0";
const GIB: u64 = 1024 * 1024 * 1024;
const ESTIMATED_PROTOCOL_OVERHEAD_BYTES: u64 = 64 * 1024 * 1024;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

impl RuntimeStatus {
    pub fn as_db_value(&self) -> &'static str {
        match self {
            Self::Detected => "DETECTED",
            Self::Checking => "CHECKING",
            Self::Ready => "READY",
            Self::Copying => "COPYING",
            Self::Cleaning => "CLEANING",
            Self::Reinitializing => "REINITIALIZING",
            Self::Done => "DONE",
            Self::Rejected => "REJECTED",
            Self::Removed => "REMOVED",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum DiskErrorCode {
    FilesystemUnsupported,
    HardwareSnUnavailable,
    ProtocolVersionUnsupported,
    ManifestInvalid,
    RecoveryRequired,
    PartialFileFound,
    CenterRejected,
}

impl DiskErrorCode {
    pub fn as_db_value(&self) -> &'static str {
        match self {
            Self::FilesystemUnsupported => "FILESYSTEM_UNSUPPORTED",
            Self::HardwareSnUnavailable => "HARDWARE_SN_UNAVAILABLE",
            Self::ProtocolVersionUnsupported => "PROTOCOL_VERSION_UNSUPPORTED",
            Self::ManifestInvalid => "MANIFEST_INVALID",
            Self::RecoveryRequired => "RECOVERY_REQUIRED",
            Self::PartialFileFound => "PARTIAL_FILE_FOUND",
            Self::CenterRejected => "CENTER_REJECTED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl DiskStatusCode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "UNREGISTERED" => Some(Self::Unregistered),
            "REGISTERED" => Some(Self::Registered),
            "INITIALIZED" => Some(Self::Initialized),
            "EDGE_COPYING" => Some(Self::EdgeCopying),
            "SEALED" => Some(Self::Sealed),
            "CENTER_IMPORTING" => Some(Self::CenterImporting),
            "IMPORTED" => Some(Self::Imported),
            "ERROR" => Some(Self::Error),
            _ => None,
        }
    }

    pub fn as_protocol_value(&self) -> &'static str {
        match self {
            Self::Unregistered => "UNREGISTERED",
            Self::Registered => "REGISTERED",
            Self::Initialized => "INITIALIZED",
            Self::EdgeCopying => "EDGE_COPYING",
            Self::Sealed => "SEALED",
            Self::CenterImporting => "CENTER_IMPORTING",
            Self::Imported => "IMPORTED",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DetectedDisk {
    pub sn: String,
    pub device_path: String,
    pub mount_path: PathBuf,
    pub filesystem: String,
    pub fs_uuid: Option<String>,
    pub label: Option<String>,
    pub id_serial: Option<String>,
    pub id_serial_short: Option<String>,
    pub capacity_bytes: u64,
    pub free_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DiskInfo {
    pub protocol: DiskInfoProtocol,
    pub disk: DiskInfoDisk,
    pub status: DiskInfoStatus,
    #[serde(default)]
    pub security: Option<DiskInfoSecurity>,
    #[serde(default)]
    pub edge: Option<DiskInfoEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DiskInfoProtocol {
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DiskInfoDisk {
    pub disk_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DiskInfoStatus {
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DiskInfoSecurity {
    pub data_key_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DiskInfoEdge {
    pub edge_code: Option<String>,
    pub export_job_id: Option<String>,
    pub seal_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialResidue {
    pub count: u32,
    pub bytes: u64,
    pub paths: Vec<PathBuf>,
}

impl PartialResidue {
    fn empty() -> Self {
        Self {
            count: 0,
            bytes: 0,
            paths: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiskRuntimeRecord {
    pub sn: String,
    pub fs_uuid: Option<String>,
    pub label: Option<String>,
    pub id_serial: Option<String>,
    pub id_serial_short: Option<String>,
    pub disk_id: Option<String>,
    pub device_path: String,
    pub mount_path: Option<String>,
    pub capacity_bytes: u64,
    pub free_bytes: u64,
    pub reserve_bytes: u64,
    pub object_budget_bytes: u64,
    pub runtime_status: String,
    pub last_error_code: Option<String>,
    pub error_message: Option<String>,
    pub partial_residue_count: u32,
    pub partial_residue_bytes: u64,
    pub last_seen_at: DateTime<Utc>,
    pub task_pool_eligible: bool,
    pub status_code: Option<String>,
    pub disk_enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiskVerifyRequest {
    pub edge_code: String,
    pub disk_id: String,
    pub sn: Option<String>,
    pub capacity_bytes: u64,
    pub free_bytes: u64,
    pub status_code: String,
    pub protocol_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskVerifyResponse {
    pub allowed: bool,
    pub disk_id: String,
    pub disk_enabled: bool,
    pub expected_status: String,
    pub action: VerifyAction,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyAction {
    AllowExport,
    Reject,
    NeedInit,
    NeedImportFirst,
}

#[derive(Debug, Error)]
pub enum DiskDetectionError {
    #[error("disk probe failed: {0}")]
    Probe(String),
    #[error("disk runtime ledger failed: {0}")]
    Ledger(String),
    #[error("center disk verify failed: {0}")]
    CenterVerify(String),
    #[error("failed to read disk_info.json at {path}: {source}")]
    ReadDiskInfo {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse disk_info.json at {path}: {source}")]
    ParseDiskInfo {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to scan partial files under {path}: {source}")]
    ScanPartial {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub trait DiskProbe: Send + Sync {
    fn scan_existing_disks<'a>(
        &'a self,
    ) -> BoxFuture<'a, Result<Vec<DetectedDisk>, DiskDetectionError>>;
}

pub trait CenterDiskVerifier: Send + Sync {
    fn verify_disk<'a>(
        &'a self,
        request: DiskVerifyRequest,
    ) -> BoxFuture<'a, Result<DiskVerifyResponse, DiskDetectionError>>;
}

pub trait DiskRuntimeLedger: Send + Sync {
    fn record_disk_runtime<'a>(
        &'a self,
        record: DiskRuntimeRecord,
    ) -> BoxFuture<'a, Result<(), DiskDetectionError>>;
}

#[derive(Debug, Clone)]
pub struct EdgeDiskDetectorConfig {
    pub edge_code: String,
    pub supported_protocol_version: String,
}

impl EdgeDiskDetectorConfig {
    pub fn new(edge_code: impl Into<String>) -> Self {
        Self {
            edge_code: edge_code.into(),
            supported_protocol_version: SUPPORTED_PROTOCOL_VERSION.to_string(),
        }
    }
}

pub struct EdgeDiskDetector<P, V, L> {
    config: EdgeDiskDetectorConfig,
    probe: P,
    verifier: V,
    ledger: L,
}

#[derive(Debug, Clone)]
pub struct ConfiguredMountProbe {
    mount_roots: Vec<PathBuf>,
}

impl ConfiguredMountProbe {
    pub fn new(mount_roots: Vec<PathBuf>) -> Self {
        Self { mount_roots }
    }

    pub fn from_env() -> Self {
        let mount_paths = std::env::var("RUSTFS_TRANSFER__TRANSPORT_MOUNT_PATHS")
            .unwrap_or_default()
            .split(';')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .collect();
        Self::new(mount_paths)
    }
}

impl DiskProbe for ConfiguredMountProbe {
    fn scan_existing_disks<'a>(
        &'a self,
    ) -> BoxFuture<'a, Result<Vec<DetectedDisk>, DiskDetectionError>> {
        Box::pin(async move {
            let mut disks = Vec::new();
            for mount_path in discover_transport_mounts(&self.mount_roots) {
                let protocol_root = mount_path.join(PROTOCOL_ROOT);
                if !protocol_root.join(DISK_INFO_FILE).exists() {
                    continue;
                }

                let (device_path, filesystem) = mount_info(&mount_path)
                    .unwrap_or_else(|| ("unknown".to_string(), "unknown".to_string()));
                let device_metadata = device_metadata(&device_path);
                let (capacity_bytes, free_bytes) =
                    disk_space_for_mount(&mount_path).unwrap_or((0, 0));
                let sn = device_metadata.hardware_sn().unwrap_or_default();
                let filesystem = if filesystem == "unknown" {
                    device_metadata.filesystem.clone().unwrap_or(filesystem)
                } else {
                    filesystem
                };

                disks.push(DetectedDisk {
                    sn,
                    device_path,
                    mount_path: mount_path.clone(),
                    filesystem,
                    fs_uuid: device_metadata.fs_uuid,
                    label: device_metadata.label,
                    id_serial: device_metadata.id_serial,
                    id_serial_short: device_metadata.id_serial_short,
                    capacity_bytes,
                    free_bytes,
                });
            }
            Ok(disks)
        })
    }
}

#[derive(Debug, Clone)]
pub struct StaticCenterDiskVerifier {
    response: DiskVerifyResponse,
}

impl StaticCenterDiskVerifier {
    pub fn reject_without_center_adapter() -> Self {
        Self {
            response: DiskVerifyResponse {
                allowed: false,
                disk_id: String::new(),
                disk_enabled: false,
                expected_status: DiskStatusCode::Initialized.as_protocol_value().to_string(),
                action: VerifyAction::Reject,
                message: Some(
                    "center /api/disk/verify adapter is not configured; export is blocked"
                        .to_string(),
                ),
            },
        }
    }
}

impl CenterDiskVerifier for StaticCenterDiskVerifier {
    fn verify_disk<'a>(
        &'a self,
        _request: DiskVerifyRequest,
    ) -> BoxFuture<'a, Result<DiskVerifyResponse, DiskDetectionError>> {
        Box::pin(async move { Ok(self.response.clone()) })
    }
}

#[derive(Debug, Clone, Default)]
pub struct LoggingDiskRuntimeLedger;

impl DiskRuntimeLedger for LoggingDiskRuntimeLedger {
    fn record_disk_runtime<'a>(
        &'a self,
        record: DiskRuntimeRecord,
    ) -> BoxFuture<'a, Result<(), DiskDetectionError>> {
        Box::pin(async move {
            tracing::info!(
                disk_sn = record.sn.as_str(),
                disk_id = record.disk_id.as_deref(),
                device_path = record.device_path.as_str(),
                mount_path = record.mount_path.as_deref(),
                fs_uuid = record.fs_uuid.as_deref(),
                label = record.label.as_deref(),
                id_serial_short = record.id_serial_short.as_deref(),
                id_serial = record.id_serial.as_deref(),
                runtime_status = record.runtime_status,
                status_code = record.status_code.as_deref(),
                disk_enabled = record.disk_enabled,
                last_error_code = record.last_error_code.as_deref(),
                task_pool_eligible = record.task_pool_eligible,
                "recorded edge disk runtime snapshot"
            );
            Ok(())
        })
    }
}

#[derive(Clone)]
pub struct PgDiskRuntimeLedger {
    pool: PgPool,
}

impl PgDiskRuntimeLedger {
    pub async fn connect(database_url: &str, max_connections: u32) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect_lazy(database_url)?;
        Ok(Self { pool })
    }
}

impl DiskRuntimeLedger for PgDiskRuntimeLedger {
    fn record_disk_runtime<'a>(
        &'a self,
        record: DiskRuntimeRecord,
    ) -> BoxFuture<'a, Result<(), DiskDetectionError>> {
        Box::pin(async move {
            let disk_id = record
                .disk_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok());
            sqlx::query(
                r#"
                INSERT INTO disk_runtime (
                    sn,
                    disk_id,
                    device_path,
                    mount_path,
                    capacity_bytes,
                    free_bytes,
                    reserve_bytes,
                    object_budget_bytes,
                    status,
                    last_error_code,
                    error_message,
                    partial_residue_count,
                    partial_residue_bytes,
                    last_seen_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                "#,
            )
            .bind(&record.sn)
            .bind(disk_id)
            .bind(&record.device_path)
            .bind(&record.mount_path)
            .bind(record.capacity_bytes as i64)
            .bind(record.free_bytes as i64)
            .bind(record.reserve_bytes as i64)
            .bind(record.object_budget_bytes as i64)
            .bind(&record.runtime_status)
            .bind(&record.last_error_code)
            .bind(&record.error_message)
            .bind(record.partial_residue_count as i32)
            .bind(record.partial_residue_bytes as i64)
            .bind(record.last_seen_at.naive_utc())
            .execute(&self.pool)
            .await
            .map_err(|err| DiskDetectionError::Ledger(err.to_string()))?;

            tracing::info!(
                disk_sn = record.sn.as_str(),
                disk_id = record.disk_id.as_deref(),
                device_path = record.device_path.as_str(),
                mount_path = record.mount_path.as_deref(),
                fs_uuid = record.fs_uuid.as_deref(),
                label = record.label.as_deref(),
                id_serial_short = record.id_serial_short.as_deref(),
                id_serial = record.id_serial.as_deref(),
                runtime_status = record.runtime_status,
                last_error_code = record.last_error_code.as_deref(),
                task_pool_eligible = record.task_pool_eligible,
                "persisted edge disk runtime snapshot"
            );
            Ok(())
        })
    }
}

impl<P, V, L> EdgeDiskDetector<P, V, L>
where
    P: DiskProbe,
    V: CenterDiskVerifier,
    L: DiskRuntimeLedger,
{
    pub fn new(config: EdgeDiskDetectorConfig, probe: P, verifier: V, ledger: L) -> Self {
        Self {
            config,
            probe,
            verifier,
            ledger,
        }
    }

    pub async fn scan_existing_transport_disks(
        &self,
    ) -> Result<Vec<DiskRuntimeRecord>, DiskDetectionError> {
        let disks = self.probe.scan_existing_disks().await?;
        let mut records = Vec::with_capacity(disks.len());

        for disk in disks {
            let record = self.evaluate_disk(disk).await?;
            self.ledger.record_disk_runtime(record.clone()).await?;
            records.push(record);
        }

        Ok(records)
    }

    pub async fn handle_udev_disk_change(
        &self,
    ) -> Result<Vec<DiskRuntimeRecord>, DiskDetectionError> {
        self.scan_existing_transport_disks().await
    }

    async fn evaluate_disk(
        &self,
        disk: DetectedDisk,
    ) -> Result<DiskRuntimeRecord, DiskDetectionError> {
        let mut record = base_record(&disk);

        if disk.filesystem != SUPPORTED_FILESYSTEM {
            reject(
                &mut record,
                RuntimeStatus::Rejected,
                DiskErrorCode::FilesystemUnsupported,
                format!(
                    "filesystem {} is not supported; transport disks must be ext4",
                    disk.filesystem
                ),
            );
            return Ok(record);
        }

        record.runtime_status = RuntimeStatus::Checking.as_db_value().to_string();

        let protocol_root = disk.mount_path.join(PROTOCOL_ROOT);
        let disk_info_path = protocol_root.join(DISK_INFO_FILE);
        let disk_info = read_disk_info(&disk_info_path)?;
        record.disk_id = Some(disk_info.disk.disk_id.clone());
        record.status_code = Some(disk_info.status.code.clone());

        if disk_info.protocol.version != self.config.supported_protocol_version {
            reject(
                &mut record,
                RuntimeStatus::Rejected,
                DiskErrorCode::ProtocolVersionUnsupported,
                format!(
                    "protocol version {} is not supported",
                    disk_info.protocol.version
                ),
            );
            return Ok(record);
        }

        if disk.sn.trim().is_empty() {
            reject(
                &mut record,
                RuntimeStatus::Rejected,
                DiskErrorCode::HardwareSnUnavailable,
                format!(
                    "hardware serial is unavailable for device {}; refusing to verify disk_id {} with center",
                    disk.device_path, disk_info.disk.disk_id
                ),
            );
            return Ok(record);
        }

        let Some(status_code) = DiskStatusCode::parse(&disk_info.status.code) else {
            reject(
                &mut record,
                RuntimeStatus::Rejected,
                DiskErrorCode::ManifestInvalid,
                format!("unknown disk status_code {}", disk_info.status.code),
            );
            return Ok(record);
        };

        let partial = scan_partial_residue(&protocol_root)?;
        record.partial_residue_count = partial.count;
        record.partial_residue_bytes = partial.bytes;

        if status_code == DiskStatusCode::EdgeCopying || partial.count > 0 {
            reject(
                &mut record,
                RuntimeStatus::Error,
                DiskErrorCode::RecoveryRequired,
                recovery_message(status_code, &partial),
            );
            return Ok(record);
        }

        let verify = self
            .verifier
            .verify_disk(DiskVerifyRequest {
                edge_code: self.config.edge_code.clone(),
                disk_id: disk_info.disk.disk_id,
                sn: if disk.sn.is_empty() {
                    None
                } else {
                    Some(disk.sn.clone())
                },
                capacity_bytes: disk.capacity_bytes,
                free_bytes: disk.free_bytes,
                status_code: disk_info.status.code,
                protocol_version: disk_info.protocol.version,
            })
            .await?;

        record.disk_enabled = Some(verify.disk_enabled);

        if verify.allowed
            && verify.disk_enabled
            && verify.action == VerifyAction::AllowExport
            && verify.expected_status == DiskStatusCode::Initialized.as_protocol_value()
            && status_code == DiskStatusCode::Initialized
        {
            record.runtime_status = RuntimeStatus::Ready.as_db_value().to_string();
            record.task_pool_eligible = true;
        } else {
            reject(
                &mut record,
                RuntimeStatus::Rejected,
                DiskErrorCode::CenterRejected,
                verify.message.unwrap_or_else(|| {
                    "center verify rejected disk; it must not enter the export task pool"
                        .to_string()
                }),
            );
        }

        Ok(record)
    }
}

fn base_record(disk: &DetectedDisk) -> DiskRuntimeRecord {
    let reserve_bytes = calculate_reserve_bytes(disk.free_bytes);
    let object_budget_bytes = calculate_object_budget_bytes(disk.free_bytes);

    DiskRuntimeRecord {
        sn: disk.sn.clone(),
        fs_uuid: disk.fs_uuid.clone(),
        label: disk.label.clone(),
        id_serial: disk.id_serial.clone(),
        id_serial_short: disk.id_serial_short.clone(),
        disk_id: None,
        device_path: disk.device_path.clone(),
        mount_path: Some(disk.mount_path.display().to_string()),
        capacity_bytes: disk.capacity_bytes,
        free_bytes: disk.free_bytes,
        reserve_bytes,
        object_budget_bytes,
        runtime_status: RuntimeStatus::Detected.as_db_value().to_string(),
        last_error_code: None,
        error_message: None,
        partial_residue_count: 0,
        partial_residue_bytes: 0,
        last_seen_at: Utc::now(),
        task_pool_eligible: false,
        status_code: None,
        disk_enabled: None,
    }
}

fn reject(
    record: &mut DiskRuntimeRecord,
    runtime_status: RuntimeStatus,
    error_code: DiskErrorCode,
    message: String,
) {
    record.runtime_status = runtime_status.as_db_value().to_string();
    record.last_error_code = Some(error_code.as_db_value().to_string());
    record.error_message = Some(message);
    record.task_pool_eligible = false;
}

fn read_disk_info(path: &Path) -> Result<DiskInfo, DiskDetectionError> {
    let bytes = std::fs::read(path).map_err(|source| DiskDetectionError::ReadDiskInfo {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| DiskDetectionError::ParseDiskInfo {
        path: path.to_path_buf(),
        source,
    })
}

fn scan_partial_residue(protocol_root: &Path) -> Result<PartialResidue, DiskDetectionError> {
    if !protocol_root.exists() {
        return Ok(PartialResidue::empty());
    }

    let mut residue = PartialResidue::empty();
    scan_partial_residue_inner(protocol_root, &mut residue).map_err(|source| {
        DiskDetectionError::ScanPartial {
            path: protocol_root.to_path_buf(),
            source,
        }
    })?;
    Ok(residue)
}

fn scan_partial_residue_inner(path: &Path, residue: &mut PartialResidue) -> std::io::Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            scan_partial_residue_inner(&path, residue)?;
        } else if file_type.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".partial"))
        {
            let bytes = entry.metadata()?.len();
            residue.count = residue.count.saturating_add(1);
            residue.bytes = residue.bytes.saturating_add(bytes);
            residue.paths.push(path);
        }
    }
    Ok(())
}

fn recovery_message(status_code: DiskStatusCode, partial: &PartialResidue) -> String {
    if partial.count > 0 {
        format!(
            "recovery required before export: status_code={}, partial_residue_count={}, partial_residue_bytes={}",
            status_code.as_protocol_value(),
            partial.count,
            partial.bytes
        )
    } else {
        format!(
            "recovery required before export: disk_info status_code={} is an unfinished edge copy",
            status_code.as_protocol_value()
        )
    }
}

pub fn calculate_reserve_bytes(free_bytes: u64) -> u64 {
    let two_percent = free_bytes.saturating_mul(2) / 100;
    two_percent.clamp(GIB, 8 * GIB)
}

pub fn calculate_object_budget_bytes(free_bytes: u64) -> u64 {
    free_bytes
        .saturating_sub(calculate_reserve_bytes(free_bytes))
        .saturating_sub(ESTIMATED_PROTOCOL_OVERHEAD_BYTES)
}

fn mount_info(mount_path: &Path) -> Option<(String, String)> {
    let output = Command::new("findmnt")
        .args(["-n", "-o", "SOURCE,FSTYPE", "--target"])
        .arg(mount_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut parts = stdout.split_whitespace();
    Some((parts.next()?.to_string(), parts.next()?.to_string()))
}

#[derive(Debug, Default)]
struct DeviceMetadata {
    id_serial_short: Option<String>,
    id_serial: Option<String>,
    fs_uuid: Option<String>,
    label: Option<String>,
    filesystem: Option<String>,
}

impl DeviceMetadata {
    fn hardware_sn(&self) -> Option<String> {
        self.id_serial_short
            .as_deref()
            .or(self.id_serial.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    }
}

fn device_metadata(device_path: &str) -> DeviceMetadata {
    let mut metadata = DeviceMetadata::default();
    if device_path == "unknown" || device_path.trim().is_empty() {
        return metadata;
    }

    if let Some(udev) = udev_properties(device_path) {
        metadata.id_serial_short = property(&udev, "ID_SERIAL_SHORT");
        metadata.id_serial = property(&udev, "ID_SERIAL");
        metadata.fs_uuid = property(&udev, "ID_FS_UUID");
        metadata.label = property(&udev, "ID_FS_LABEL");
        metadata.filesystem = property(&udev, "ID_FS_TYPE");
    }

    if let Some(blkid) = blkid_properties(device_path) {
        metadata.fs_uuid = metadata.fs_uuid.or_else(|| property(&blkid, "UUID"));
        metadata.label = metadata.label.or_else(|| property(&blkid, "LABEL"));
        metadata.filesystem = metadata.filesystem.or_else(|| property(&blkid, "TYPE"));
    }

    metadata
}

fn udev_properties(device_path: &str) -> Option<String> {
    let output = Command::new("udevadm")
        .args(["info", "--query=property", "--name"])
        .arg(device_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn blkid_properties(device_path: &str) -> Option<String> {
    let output = Command::new("blkid")
        .args(["-o", "export"])
        .arg(device_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn property(properties: &str, key: &str) -> Option<String> {
    properties.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name == key)
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn disk_space_for_mount(mount_path: &Path) -> Option<(u64, u64)> {
    let output = Command::new("df")
        .args(["-B1", "--output=size,avail"])
        .arg(mount_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines().skip(1);
    let line = lines.next()?;
    let mut parts = line.split_whitespace();
    let capacity_bytes = parts.next()?.parse().ok()?;
    let free_bytes = parts.next()?.parse().ok()?;
    Some((capacity_bytes, free_bytes))
}

fn discover_transport_mounts(mount_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut mounts = Vec::new();
    for root in mount_roots {
        if root.join(PROTOCOL_ROOT).join(DISK_INFO_FILE).exists() {
            mounts.push(root.clone());
            continue;
        }

        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.join(PROTOCOL_ROOT).join(DISK_INFO_FILE).exists() {
                mounts.push(path);
            }
        }
    }
    mounts.sort();
    mounts.dedup();
    mounts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::{Arc, Mutex},
    };

    #[derive(Clone)]
    struct MockProbe {
        disks: Vec<DetectedDisk>,
    }

    impl DiskProbe for MockProbe {
        fn scan_existing_disks<'a>(
            &'a self,
        ) -> BoxFuture<'a, Result<Vec<DetectedDisk>, DiskDetectionError>> {
            Box::pin(async move { Ok(self.disks.clone()) })
        }
    }

    #[derive(Clone)]
    struct MockVerifier {
        response: DiskVerifyResponse,
        requests: Arc<Mutex<Vec<DiskVerifyRequest>>>,
    }

    impl CenterDiskVerifier for MockVerifier {
        fn verify_disk<'a>(
            &'a self,
            request: DiskVerifyRequest,
        ) -> BoxFuture<'a, Result<DiskVerifyResponse, DiskDetectionError>> {
            Box::pin(async move {
                self.requests
                    .lock()
                    .expect("requests mutex poisoned")
                    .push(request);
                Ok(self.response.clone())
            })
        }
    }

    #[derive(Clone, Default)]
    struct MockLedger {
        records: Arc<Mutex<Vec<DiskRuntimeRecord>>>,
    }

    impl DiskRuntimeLedger for MockLedger {
        fn record_disk_runtime<'a>(
            &'a self,
            record: DiskRuntimeRecord,
        ) -> BoxFuture<'a, Result<(), DiskDetectionError>> {
            Box::pin(async move {
                self.records
                    .lock()
                    .expect("records mutex poisoned")
                    .push(record);
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn rejects_non_ext4_before_center_verify() {
        let mount = temp_mount("non-ext4");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let ledger = MockLedger::default();
        let detector = detector(
            vec![disk(&mount, "xfs")],
            verifier_response(true),
            requests.clone(),
            ledger.clone(),
        );

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "REJECTED");
        assert_eq!(
            records[0].last_error_code.as_deref(),
            Some("FILESYSTEM_UNSUPPORTED")
        );
        assert!(!records[0].task_pool_eligible);
        assert!(requests.lock().unwrap().is_empty());
        assert_eq!(ledger.records.lock().unwrap().len(), 1);
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn records_recovery_required_for_edge_copying_residue() {
        let mount = temp_mount("edge-copying");
        write_disk_info(&mount, "EDGE_COPYING");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let detector = detector(
            vec![disk(&mount, "ext4")],
            verifier_response(true),
            requests.clone(),
            MockLedger::default(),
        );

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "ERROR");
        assert_eq!(
            records[0].last_error_code.as_deref(),
            Some("RECOVERY_REQUIRED")
        );
        assert!(!records[0].task_pool_eligible);
        assert!(requests.lock().unwrap().is_empty());
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn records_recovery_required_for_partial_files() {
        let mount = temp_mount("partial");
        write_disk_info(&mount, "INITIALIZED");
        let partial_path = mount
            .join(PROTOCOL_ROOT)
            .join("data")
            .join("object.bin.partial");
        fs::create_dir_all(partial_path.parent().unwrap()).unwrap();
        fs::write(&partial_path, b"partial-bytes").unwrap();
        let detector = detector(
            vec![disk(&mount, "ext4")],
            verifier_response(true),
            Arc::new(Mutex::new(Vec::new())),
            MockLedger::default(),
        );

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "ERROR");
        assert_eq!(
            records[0].last_error_code.as_deref(),
            Some("RECOVERY_REQUIRED")
        );
        assert_eq!(records[0].partial_residue_count, 1);
        assert_eq!(records[0].partial_residue_bytes, 13);
        assert!(!records[0].task_pool_eligible);
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn ready_after_initialized_disk_passes_center_verify() {
        let mount = temp_mount("ready");
        write_disk_info(&mount, "INITIALIZED");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let detector = detector(
            vec![disk(&mount, "ext4")],
            verifier_response(true),
            requests.clone(),
            MockLedger::default(),
        );

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "READY");
        assert_eq!(records[0].status_code.as_deref(), Some("INITIALIZED"));
        assert_eq!(records[0].disk_enabled, Some(true));
        assert!(records[0].task_pool_eligible);
        let request = requests.lock().unwrap().first().cloned().unwrap();
        assert_eq!(request.edge_code, "edge-a");
        assert_eq!(request.disk_id, "11111111-1111-1111-1111-111111111111");
        assert_eq!(request.sn.as_deref(), Some("SN-001"));
        assert_eq!(request.status_code, "INITIALIZED");
        assert_eq!(request.protocol_version, "1.0");
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn rejects_missing_hardware_sn_before_center_verify() {
        let mount = temp_mount("missing-sn");
        write_disk_info(&mount, "INITIALIZED");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let mut detected = disk(&mount, "ext4");
        detected.sn.clear();
        detected.id_serial = None;
        detected.id_serial_short = None;
        let detector = detector(
            vec![detected],
            verifier_response(true),
            requests.clone(),
            MockLedger::default(),
        );

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "REJECTED");
        assert_eq!(
            records[0].last_error_code.as_deref(),
            Some("HARDWARE_SN_UNAVAILABLE")
        );
        assert!(!records[0].task_pool_eligible);
        assert!(requests.lock().unwrap().is_empty());
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn center_rejection_blocks_unregistered_disabled_or_sealed_disk() {
        let mount = temp_mount("center-rejected");
        write_disk_info(&mount, "SEALED");
        let detector = detector(
            vec![disk(&mount, "ext4")],
            DiskVerifyResponse {
                allowed: false,
                disk_id: "11111111-1111-1111-1111-111111111111".to_string(),
                disk_enabled: false,
                expected_status: "INITIALIZED".to_string(),
                action: VerifyAction::NeedImportFirst,
                message: Some("need import first".to_string()),
            },
            Arc::new(Mutex::new(Vec::new())),
            MockLedger::default(),
        );

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "REJECTED");
        assert_eq!(
            records[0].last_error_code.as_deref(),
            Some("CENTER_REJECTED")
        );
        assert_eq!(records[0].disk_enabled, Some(false));
        assert!(!records[0].task_pool_eligible);
        fs::remove_dir_all(mount).ok();
    }

    fn detector(
        disks: Vec<DetectedDisk>,
        response: DiskVerifyResponse,
        requests: Arc<Mutex<Vec<DiskVerifyRequest>>>,
        ledger: MockLedger,
    ) -> EdgeDiskDetector<MockProbe, MockVerifier, MockLedger> {
        EdgeDiskDetector::new(
            EdgeDiskDetectorConfig::new("edge-a"),
            MockProbe { disks },
            MockVerifier { response, requests },
            ledger,
        )
    }

    fn verifier_response(allowed: bool) -> DiskVerifyResponse {
        DiskVerifyResponse {
            allowed,
            disk_id: "11111111-1111-1111-1111-111111111111".to_string(),
            disk_enabled: allowed,
            expected_status: "INITIALIZED".to_string(),
            action: if allowed {
                VerifyAction::AllowExport
            } else {
                VerifyAction::Reject
            },
            message: None,
        }
    }

    fn disk(mount_path: &Path, filesystem: &str) -> DetectedDisk {
        DetectedDisk {
            sn: "SN-001".to_string(),
            device_path: "/dev/sdb1".to_string(),
            mount_path: mount_path.to_path_buf(),
            filesystem: filesystem.to_string(),
            fs_uuid: Some("fs-uuid-001".to_string()),
            label: Some("FUSTFS-TST-A".to_string()),
            id_serial: Some("USB_DISK_SN-001".to_string()),
            id_serial_short: Some("SN-001".to_string()),
            capacity_bytes: 100 * GIB,
            free_bytes: 80 * GIB,
        }
    }

    fn temp_mount(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rustfs-transfer-edge-test-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(path.join(PROTOCOL_ROOT)).unwrap();
        path
    }

    fn write_disk_info(mount: &Path, status_code: &str) {
        let payload = format!(
            r#"{{
  "protocol": {{ "version": "1.0" }},
  "disk": {{ "disk_id": "11111111-1111-1111-1111-111111111111" }},
  "status": {{ "code": "{status_code}" }},
  "security": {{ "data_key_id": "22222222-2222-2222-2222-222222222222" }}
}}"#
        );
        fs::write(mount.join(PROTOCOL_ROOT).join(DISK_INFO_FILE), payload).unwrap();
    }

    #[test]
    fn configured_probe_discovers_child_mounts_under_root() {
        let root = std::env::temp_dir().join(format!(
            "rustfs-transfer-edge-test-root-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let child = root.join("disk-a");
        fs::create_dir_all(child.join(PROTOCOL_ROOT)).unwrap();
        write_disk_info(&child, "INITIALIZED");
        let mounts = discover_transport_mounts(&[root.clone()]);

        assert_eq!(mounts, vec![child]);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn device_metadata_prefers_short_serial_and_preserves_fs_identity() {
        let udev = r#"
ID_SERIAL=USB_DISK_VENDOR_MODEL_LONG_SERIAL
ID_SERIAL_SHORT=323533394352343033383235
ID_FS_UUID=0878ee5b-3e86-4ae0-8d0f-461c2732ee42
ID_FS_LABEL=FUSTFS-TST-A
ID_FS_TYPE=ext4
"#;
        let metadata = DeviceMetadata {
            id_serial_short: property(udev, "ID_SERIAL_SHORT"),
            id_serial: property(udev, "ID_SERIAL"),
            fs_uuid: property(udev, "ID_FS_UUID"),
            label: property(udev, "ID_FS_LABEL"),
            filesystem: property(udev, "ID_FS_TYPE"),
        };

        assert_eq!(
            metadata.hardware_sn().as_deref(),
            Some("323533394352343033383235")
        );
        assert_eq!(
            metadata.id_serial.as_deref(),
            Some("USB_DISK_VENDOR_MODEL_LONG_SERIAL")
        );
        assert_eq!(metadata.label.as_deref(), Some("FUSTFS-TST-A"));
        assert_eq!(
            metadata.fs_uuid.as_deref(),
            Some("0878ee5b-3e86-4ae0-8d0f-461c2732ee42")
        );
        assert_eq!(metadata.filesystem.as_deref(), Some("ext4"));
    }

    #[tokio::test]
    async fn configured_probe_does_not_use_mount_label_as_hardware_sn() {
        let root = std::env::temp_dir().join(format!(
            "rustfs-transfer-edge-test-root-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let child = root.join("FUSTFS-TST-A");
        fs::create_dir_all(child.join(PROTOCOL_ROOT)).unwrap();
        write_disk_info(&child, "INITIALIZED");
        let probe = ConfiguredMountProbe::new(vec![root.clone()]);

        let disks = probe.scan_existing_disks().await.unwrap();

        assert_eq!(disks.len(), 1);
        assert_ne!(disks[0].sn, "FUSTFS-TST-A");
        fs::remove_dir_all(root).ok();
    }
}
