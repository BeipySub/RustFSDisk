use std::{
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    future::Future,
    io::Write,
    path::{Path, PathBuf},
    pin::Pin,
    process::Command,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

pub const PROTOCOL_ROOT: &str = "rustfs-transfer";
pub const DISK_INFO_FILE: &str = "disk_info.json";
pub const SUPPORTED_FILESYSTEM: &str = "ext4";
pub const SUPPORTED_PROTOCOL_VERSION: &str = "1.0";
const GIB: u64 = 1024 * 1024 * 1024;
const ESTIMATED_PROTOCOL_OVERHEAD_BYTES: u64 = 64 * 1024 * 1024;
const REPLACE_CURRENT_RUNTIME_SQL: &str = r#"
                DELETE FROM disk_runtime
                WHERE status <> 'COPYING'
                  AND (
                    ($1::uuid IS NOT NULL AND disk_id = $1)
                    OR (device_path = $2 AND mount_path IS NOT DISTINCT FROM $3)
                  )
                "#;
const MARK_MISSING_RUNTIME_REMOVED_SQL: &str = r#"
                UPDATE disk_runtime
                SET status = 'REMOVED',
                    free_bytes = 0,
                    reserve_bytes = 0,
                    object_budget_bytes = 0,
                    last_error_code = 'DISK_REMOVED',
                    error_message = 'transport disk is no longer detected by edge rescan',
                    last_seen_at = NOW() AT TIME ZONE 'UTC'
                WHERE id = $1
                "#;

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
    SignatureInvalid,
    RecoveryRequired,
    PartialFileFound,
    DiskRemoved,
    DiskWritePermissionDenied,
}

impl DiskErrorCode {
    pub fn as_db_value(&self) -> &'static str {
        match self {
            Self::FilesystemUnsupported => "FILESYSTEM_UNSUPPORTED",
            Self::HardwareSnUnavailable => "HARDWARE_SN_UNAVAILABLE",
            Self::ProtocolVersionUnsupported => "PROTOCOL_VERSION_UNSUPPORTED",
            Self::ManifestInvalid => "MANIFEST_INVALID",
            Self::SignatureInvalid => "SIGNATURE_INVALID",
            Self::RecoveryRequired => "RECOVERY_REQUIRED",
            Self::PartialFileFound => "PARTIAL_FILE_FOUND",
            Self::DiskRemoved => "DISK_REMOVED",
            Self::DiskWritePermissionDenied => "DISK_WRITE_PERMISSION_DENIED",
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
    #[serde(default)]
    pub center_signature: Option<String>,
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
    pub disk_presence_id: String,
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

#[derive(Debug, Error)]
pub enum DiskDetectionError {
    #[error("disk probe failed: {0}")]
    Probe(String),
    #[error("disk runtime ledger failed: {0}")]
    Ledger(String),
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

pub trait DiskRuntimeLedger: Send + Sync {
    fn record_disk_runtime<'a>(
        &'a self,
        record: DiskRuntimeRecord,
    ) -> BoxFuture<'a, Result<(), DiskDetectionError>>;

    fn mark_missing_disks_removed<'a>(
        &'a self,
        _current_records: &'a [DiskRuntimeRecord],
    ) -> BoxFuture<'a, Result<Vec<DiskRuntimeRecord>, DiskDetectionError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

pub trait DiskRuntimeEventPublisher: Clone + Send + Sync + 'static {
    fn publish_disk_runtime(&self, record: &DiskRuntimeRecord);
}

#[derive(Debug, Clone, Default)]
pub struct NoopDiskRuntimeEventPublisher;

impl DiskRuntimeEventPublisher for NoopDiskRuntimeEventPublisher {
    fn publish_disk_runtime(&self, _record: &DiskRuntimeRecord) {}
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

pub struct EdgeDiskDetector<P, L, E = NoopDiskRuntimeEventPublisher> {
    config: EdgeDiskDetectorConfig,
    probe: P,
    ledger: L,
    event_publisher: E,
    presence_by_location: Arc<Mutex<HashMap<(String, Option<String>), String>>>,
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
        let mount_paths = std::env::var("TRANSPORT_MOUNT_PATHS")
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
                let has_protocol_info = protocol_root.join(DISK_INFO_FILE).exists();

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
                if !should_include_mount_candidate(has_protocol_info, &filesystem, &device_metadata)
                {
                    continue;
                }

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
            append_unique_disks(&mut disks, discover_mounted_usb_partitions());
            disks.extend(discover_unmounted_unsupported_usb_partitions());
            Ok(disks)
        })
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
            sqlx::query(REPLACE_CURRENT_RUNTIME_SQL)
                .bind(disk_id)
                .bind(&record.device_path)
                .bind(&record.mount_path)
                .execute(&self.pool)
                .await
                .map_err(|err| DiskDetectionError::Ledger(err.to_string()))?;

            sqlx::query(
                r#"
                INSERT INTO disk_runtime (
                    disk_presence_id,
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
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                "#,
            )
            .bind(Uuid::parse_str(&record.disk_presence_id).ok())
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

    fn mark_missing_disks_removed<'a>(
        &'a self,
        current_records: &'a [DiskRuntimeRecord],
    ) -> BoxFuture<'a, Result<Vec<DiskRuntimeRecord>, DiskDetectionError>> {
        Box::pin(async move {
            let current_disk_ids = current_records
                .iter()
                .filter_map(|record| record.disk_id.as_deref())
                .filter_map(|value| Uuid::parse_str(value).ok())
                .collect::<HashSet<_>>();
            let current_locations = current_records
                .iter()
                .map(|record| (record.device_path.clone(), record.mount_path.clone()))
                .collect::<HashSet<_>>();

            let rows = sqlx::query(
                r#"
                SELECT DISTINCT ON (
                    COALESCE(disk_id::text, device_path || '|' || COALESCE(mount_path, ''))
                )
                    id,
                    sn,
                    disk_presence_id,
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
                FROM disk_runtime
                ORDER BY
                    COALESCE(disk_id::text, device_path || '|' || COALESCE(mount_path, '')),
                    id DESC
                "#,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|err| DiskDetectionError::Ledger(err.to_string()))?;

            let mut removed = Vec::new();
            for row in rows {
                let status: String = row.get("status");
                if status == RuntimeStatus::Removed.as_db_value() {
                    continue;
                }

                let disk_id: Option<Uuid> = row.get("disk_id");
                let device_path: String = row.get("device_path");
                let mount_path: Option<String> = row.get("mount_path");
                let is_present = disk_id
                    .as_ref()
                    .is_some_and(|disk_id| current_disk_ids.contains(disk_id))
                    || current_locations.contains(&(device_path.clone(), mount_path.clone()));
                if is_present {
                    continue;
                }

                let row_id: i64 = row.get("id");
                let record = DiskRuntimeRecord {
                    disk_presence_id: row
                        .get::<Option<Uuid>, _>("disk_presence_id")
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| Uuid::new_v4().to_string()),
                    sn: row.get("sn"),
                    fs_uuid: None,
                    label: None,
                    id_serial: None,
                    id_serial_short: None,
                    disk_id: disk_id.map(|value| value.to_string()),
                    device_path,
                    mount_path,
                    capacity_bytes: row.get::<i64, _>("capacity_bytes").max(0) as u64,
                    free_bytes: 0,
                    reserve_bytes: 0,
                    object_budget_bytes: 0,
                    runtime_status: RuntimeStatus::Removed.as_db_value().to_string(),
                    last_error_code: Some(DiskErrorCode::DiskRemoved.as_db_value().to_string()),
                    error_message: Some(
                        "transport disk is no longer detected by edge rescan".to_string(),
                    ),
                    partial_residue_count: row.get::<i32, _>("partial_residue_count").max(0) as u32,
                    partial_residue_bytes: row.get::<i64, _>("partial_residue_bytes").max(0) as u64,
                    last_seen_at: Utc::now(),
                    task_pool_eligible: false,
                    status_code: None,
                    disk_enabled: None,
                };
                mark_missing_disk_runtime_removed(&self.pool, row_id).await?;
                tracing::info!(
                    disk_sn = record.sn.as_str(),
                    disk_id = record.disk_id.as_deref(),
                    device_path = record.device_path.as_str(),
                    mount_path = record.mount_path.as_deref(),
                    runtime_status = record.runtime_status,
                    last_error_code = record.last_error_code.as_deref(),
                    "marked missing edge disk runtime snapshot as removed"
                );
                removed.push(record);
            }

            Ok(removed)
        })
    }
}

async fn mark_missing_disk_runtime_removed(
    pool: &PgPool,
    row_id: i64,
) -> Result<(), DiskDetectionError> {
    sqlx::query(MARK_MISSING_RUNTIME_REMOVED_SQL)
        .bind(row_id)
        .execute(pool)
        .await
        .map_err(|err| DiskDetectionError::Ledger(err.to_string()))?;
    Ok(())
}

impl<P, L> EdgeDiskDetector<P, L>
where
    P: DiskProbe,
    L: DiskRuntimeLedger,
{
    pub fn new(config: EdgeDiskDetectorConfig, probe: P, ledger: L) -> Self {
        Self::new_with_event_publisher(config, probe, ledger, NoopDiskRuntimeEventPublisher)
    }
}

impl<P, L, E> EdgeDiskDetector<P, L, E>
where
    P: DiskProbe,
    L: DiskRuntimeLedger,
    E: DiskRuntimeEventPublisher,
{
    pub fn new_with_event_publisher(
        config: EdgeDiskDetectorConfig,
        probe: P,
        ledger: L,
        event_publisher: E,
    ) -> Self {
        Self {
            config,
            probe,
            ledger,
            event_publisher,
            presence_by_location: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn scan_existing_transport_disks(
        &self,
    ) -> Result<Vec<DiskRuntimeRecord>, DiskDetectionError> {
        let disks = self.probe.scan_existing_disks().await?;
        let mut records = Vec::with_capacity(disks.len());

        for disk in disks {
            let detected = self.base_record(&disk);
            let record = self.evaluate_disk(disk, detected).await?;
            self.ledger.record_disk_runtime(record.clone()).await?;
            self.event_publisher.publish_disk_runtime(&record);
            records.push(record);
        }

        let removed_records = self.ledger.mark_missing_disks_removed(&records).await?;
        for record in &removed_records {
            self.presence_by_location
                .lock()
                .expect("disk presence mutex poisoned")
                .remove(&(record.device_path.clone(), record.mount_path.clone()));
            self.event_publisher.publish_disk_runtime(record);
        }
        records.extend(removed_records);

        Ok(records)
    }

    pub async fn handle_disk_change(&self) -> Result<Vec<DiskRuntimeRecord>, DiskDetectionError> {
        self.scan_existing_transport_disks().await
    }

    async fn evaluate_disk(
        &self,
        disk: DetectedDisk,
        mut record: DiskRuntimeRecord,
    ) -> Result<DiskRuntimeRecord, DiskDetectionError> {
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
        if !disk_info_path.exists() {
            record.status_code = Some(DiskStatusCode::Unregistered.as_protocol_value().to_string());
            record.disk_enabled = Some(false);
            reject(
                &mut record,
                RuntimeStatus::Rejected,
                DiskErrorCode::ManifestInvalid,
                "transport candidate is not initialized by Center: missing rustfs-transfer/disk_info.json".to_string(),
            );
            return Ok(record);
        }

        let disk_info = match read_disk_info(&disk_info_path) {
            Ok(disk_info) => disk_info,
            Err(err) => {
                reject(
                    &mut record,
                    RuntimeStatus::Rejected,
                    DiskErrorCode::ManifestInvalid,
                    err.to_string(),
                );
                return Ok(record);
            }
        };
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

        if !has_center_signature(&disk_info) {
            reject(
                &mut record,
                RuntimeStatus::Rejected,
                DiskErrorCode::SignatureInvalid,
                "disk_info security.center_signature is missing".to_string(),
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

        let partial = match scan_partial_residue(&protocol_root) {
            Ok(partial) => partial,
            Err(err) => {
                reject(
                    &mut record,
                    RuntimeStatus::Rejected,
                    DiskErrorCode::RecoveryRequired,
                    err.to_string(),
                );
                return Ok(record);
            }
        };
        record.partial_residue_count = partial.count;
        record.partial_residue_bytes = partial.bytes;

        if status_code == DiskStatusCode::EdgeCopying || partial.count > 0 {
            reject(
                &mut record,
                RuntimeStatus::Rejected,
                DiskErrorCode::RecoveryRequired,
                recovery_message(status_code, &partial),
            );
            return Ok(record);
        }

        if status_code == DiskStatusCode::Initialized {
            if let Err(message) = verify_protocol_root_writable(&protocol_root) {
                reject(
                    &mut record,
                    RuntimeStatus::Rejected,
                    DiskErrorCode::DiskWritePermissionDenied,
                    message,
                );
                return Ok(record);
            }
        }

        record.disk_enabled = Some(true);
        match status_code {
            DiskStatusCode::Initialized => {
                record.runtime_status = RuntimeStatus::Ready.as_db_value().to_string();
                record.task_pool_eligible = true;
            }
            DiskStatusCode::Sealed => {
                // SEALED is a valid terminal state on Edge, not a malformed transport disk.
                record.runtime_status = RuntimeStatus::Done.as_db_value().to_string();
            }
            _ => {
                reject(
                    &mut record,
                    RuntimeStatus::Rejected,
                    DiskErrorCode::ManifestInvalid,
                    format!(
                        "disk status_code {} is not eligible for offline edge export; expected INITIALIZED",
                        status_code.as_protocol_value()
                    ),
                );
            }
        }

        Ok(record)
    }

    fn base_record(&self, disk: &DetectedDisk) -> DiskRuntimeRecord {
        let location = (
            disk.device_path.clone(),
            Some(disk.mount_path.display().to_string()),
        );
        let disk_presence_id = self
            .presence_by_location
            .lock()
            .expect("disk presence mutex poisoned")
            .entry(location)
            .or_insert_with(|| Uuid::new_v4().to_string())
            .clone();
        base_record(disk, disk_presence_id)
    }
}

fn has_center_signature(disk_info: &DiskInfo) -> bool {
    disk_info
        .security
        .as_ref()
        .and_then(|security| security.center_signature.as_deref())
        .is_some_and(|value| !value.trim().is_empty())
}

fn base_record(disk: &DetectedDisk, disk_presence_id: String) -> DiskRuntimeRecord {
    let reserve_bytes = calculate_reserve_bytes(disk.free_bytes);
    let object_budget_bytes = calculate_object_budget_bytes(disk.free_bytes);

    DiskRuntimeRecord {
        disk_presence_id,
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

fn verify_protocol_root_writable(protocol_root: &Path) -> Result<(), String> {
    let probe_path = protocol_root.join(format!(".edge-write-probe-{}", Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe_path)
            .map_err(|err| {
                format!(
                    "edge process cannot create write probe under {}: {err}",
                    protocol_root.display()
                )
            })?;
        file.write_all(b"rustfs-transfer-edge-write-probe")
            .map_err(|err| {
                format!(
                    "edge process cannot write probe file {}: {err}",
                    probe_path.display()
                )
            })?;
        file.sync_all().map_err(|err| {
            format!(
                "edge process cannot fsync probe file {}: {err}",
                probe_path.display()
            )
        })?;
        Ok(())
    })();

    match std::fs::remove_file(&probe_path) {
        Ok(()) => {}
        Err(err) if probe_path.exists() => {
            return Err(format!(
                "edge process cannot remove write probe file {}: {err}",
                probe_path.display()
            ));
        }
        Err(_) => {}
    }

    result
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
        .args(["-n", "-o", "SOURCE,FSTYPE", "--mountpoint"])
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
    bus: Option<String>,
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

fn is_usb_block_device(metadata: &DeviceMetadata) -> bool {
    metadata.bus.as_deref() == Some("usb")
}

fn should_include_mount_candidate(
    has_protocol_info: bool,
    filesystem: &str,
    metadata: &DeviceMetadata,
) -> bool {
    has_protocol_info || filesystem == SUPPORTED_FILESYSTEM || is_usb_block_device(metadata)
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
        metadata.bus = property(&udev, "ID_BUS");
    }

    if let Some(blkid) = blkid_properties(device_path) {
        metadata.fs_uuid = metadata.fs_uuid.or_else(|| property(&blkid, "UUID"));
        metadata.label = metadata.label.or_else(|| property(&blkid, "LABEL"));
        metadata.filesystem = metadata.filesystem.or_else(|| property(&blkid, "TYPE"));
    }

    metadata
}

fn discover_mounted_usb_partitions() -> Vec<DetectedDisk> {
    let output = Command::new("lsblk")
        .args([
            "-P",
            "-p",
            "-n",
            "-o",
            "NAME,PKNAME,TYPE,MOUNTPOINT,FSTYPE,TRAN",
        ])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_mounted_usb_partition)
        .collect()
}

fn parse_mounted_usb_partition(line: &str) -> Option<DetectedDisk> {
    let fields = parse_lsblk_pairs(line);
    if fields.get("TYPE").map(String::as_str) != Some("part") {
        return None;
    }

    let mount_path = fields.get("MOUNTPOINT")?.trim();
    if mount_path.is_empty() {
        return None;
    }

    let filesystem = fields.get("FSTYPE")?.trim().to_string();
    if filesystem.is_empty() {
        return None;
    }

    let device_path = fields.get("NAME")?.trim().to_string();
    let parent_device_path = fields
        .get("PKNAME")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let transport = fields
        .get("TRAN")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let device_metadata = device_metadata_with_parent(&device_path, parent_device_path);
    if !is_usb_lsblk_candidate(&device_metadata, transport) {
        return None;
    }

    let mount_path = PathBuf::from(mount_path);
    let (capacity_bytes, free_bytes) = disk_space_for_mount(&mount_path).unwrap_or((0, 0));
    Some(DetectedDisk {
        sn: device_metadata.hardware_sn().unwrap_or_default(),
        device_path,
        mount_path,
        filesystem,
        fs_uuid: device_metadata.fs_uuid,
        label: device_metadata.label,
        id_serial: device_metadata.id_serial,
        id_serial_short: device_metadata.id_serial_short,
        capacity_bytes,
        free_bytes,
    })
}

fn discover_unmounted_unsupported_usb_partitions() -> Vec<DetectedDisk> {
    let output = Command::new("lsblk")
        .args([
            "-P",
            "-p",
            "-n",
            "-o",
            "NAME,PKNAME,TYPE,MOUNTPOINT,FSTYPE,TRAN",
        ])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_unmounted_unsupported_usb_partition)
        .collect()
}

fn parse_unmounted_unsupported_usb_partition(line: &str) -> Option<DetectedDisk> {
    let fields = parse_lsblk_pairs(line);
    if fields.get("TYPE").map(String::as_str) != Some("part") {
        return None;
    }
    if fields
        .get("MOUNTPOINT")
        .is_some_and(|value| !value.trim().is_empty())
    {
        return None;
    }

    let filesystem = fields.get("FSTYPE")?.trim().to_string();
    if filesystem.is_empty() || filesystem == SUPPORTED_FILESYSTEM {
        return None;
    }

    let device_path = fields.get("NAME")?.trim().to_string();
    let parent_device_path = fields
        .get("PKNAME")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let transport = fields
        .get("TRAN")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let device_metadata = device_metadata_with_parent(&device_path, parent_device_path);
    if !is_usb_lsblk_candidate(&device_metadata, transport) {
        return None;
    }

    let capacity_bytes = block_device_size(&device_path).unwrap_or_default();
    Some(DetectedDisk {
        sn: device_metadata.hardware_sn().unwrap_or_default(),
        device_path: device_path.clone(),
        mount_path: PathBuf::from(device_path),
        filesystem,
        fs_uuid: device_metadata.fs_uuid,
        label: device_metadata.label,
        id_serial: device_metadata.id_serial,
        id_serial_short: device_metadata.id_serial_short,
        capacity_bytes,
        free_bytes: 0,
    })
}

fn device_metadata_with_parent(
    device_path: &str,
    parent_device_path: Option<&str>,
) -> DeviceMetadata {
    let mut metadata = device_metadata(device_path);
    if metadata.bus.as_deref() == Some("usb") {
        return metadata;
    }

    if let Some(parent_device_path) = parent_device_path {
        let parent = device_metadata(parent_device_path);
        metadata.id_serial_short = metadata.id_serial_short.or(parent.id_serial_short);
        metadata.id_serial = metadata.id_serial.or(parent.id_serial);
        metadata.bus = metadata.bus.or(parent.bus);
    }
    metadata
}

fn is_usb_lsblk_candidate(metadata: &DeviceMetadata, transport: Option<&str>) -> bool {
    is_usb_block_device(metadata) || transport == Some("usb")
}

fn parse_lsblk_pairs(line: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    let mut rest = line.trim();
    while let Some((key, after_key)) = rest.split_once('=') {
        let key = key
            .rsplit_once(char::is_whitespace)
            .map(|(_, key)| key)
            .unwrap_or(key)
            .trim();
        let mut chars = after_key.chars();
        if chars.next() != Some('"') {
            break;
        }

        let mut value = String::new();
        let mut escaped = false;
        let mut consumed = key.len() + 2;
        for ch in chars {
            consumed += ch.len_utf8();
            if escaped {
                value.push(ch);
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => break,
                _ => value.push(ch),
            }
        }
        fields.insert(key.to_string(), value);
        rest = rest.get(consumed..).unwrap_or_default().trim_start();
    }
    fields
}

fn append_unique_disks(disks: &mut Vec<DetectedDisk>, candidates: Vec<DetectedDisk>) {
    let mut seen = disks
        .iter()
        .map(|disk| (disk.device_path.clone(), disk.mount_path.clone()))
        .collect::<HashSet<_>>();
    for candidate in candidates {
        let key = (candidate.device_path.clone(), candidate.mount_path.clone());
        if seen.insert(key) {
            disks.push(candidate);
        }
    }
}

fn block_device_size(device_path: &str) -> Option<u64> {
    let output = Command::new("blockdev")
        .arg("--getsize64")
        .arg(device_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
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
        discover_transport_mounts_inner(root, 0, &mut mounts);
    }
    mounts.sort();
    mounts.dedup();
    mounts
}

fn discover_transport_mounts_inner(path: &Path, depth: usize, mounts: &mut Vec<PathBuf>) {
    if !path.is_dir() {
        return;
    }
    mounts.push(path.to_path_buf());
    if depth >= 2 {
        return;
    }

    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let child = entry.path();
        if child.is_dir() && child.file_name().and_then(|name| name.to_str()) != Some(PROTOCOL_ROOT)
        {
            discover_transport_mounts_inner(&child, depth + 1, mounts);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
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

    #[derive(Clone, Default)]
    struct MockLedger {
        records: Arc<Mutex<Vec<DiskRuntimeRecord>>>,
        existing: Arc<Mutex<Vec<DiskRuntimeRecord>>>,
    }

    impl MockLedger {
        fn with_existing(existing: Vec<DiskRuntimeRecord>) -> Self {
            Self {
                records: Arc::new(Mutex::new(Vec::new())),
                existing: Arc::new(Mutex::new(existing)),
            }
        }
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

        fn mark_missing_disks_removed<'a>(
            &'a self,
            current_records: &'a [DiskRuntimeRecord],
        ) -> BoxFuture<'a, Result<Vec<DiskRuntimeRecord>, DiskDetectionError>> {
            Box::pin(async move {
                let current_ids = current_records
                    .iter()
                    .filter_map(|record| record.disk_id.clone())
                    .collect::<HashSet<_>>();
                let current_locations = current_records
                    .iter()
                    .map(|record| (record.device_path.clone(), record.mount_path.clone()))
                    .collect::<HashSet<_>>();
                let removed = self
                    .existing
                    .lock()
                    .expect("existing mutex poisoned")
                    .iter()
                    .filter(|record| record.runtime_status != "REMOVED")
                    .filter(|record| {
                        !record
                            .disk_id
                            .as_ref()
                            .is_some_and(|disk_id| current_ids.contains(disk_id))
                            && !current_locations
                                .contains(&(record.device_path.clone(), record.mount_path.clone()))
                    })
                    .map(|record| {
                        let mut removed = record.clone();
                        removed.runtime_status = RuntimeStatus::Removed.as_db_value().to_string();
                        removed.last_error_code =
                            Some(DiskErrorCode::DiskRemoved.as_db_value().to_string());
                        removed.error_message =
                            Some("transport disk is no longer detected by edge rescan".to_string());
                        removed.task_pool_eligible = false;
                        removed
                    })
                    .collect::<Vec<_>>();
                self.records
                    .lock()
                    .expect("records mutex poisoned")
                    .extend(removed.clone());
                Ok(removed)
            })
        }
    }

    #[derive(Clone, Default)]
    struct MockEventPublisher {
        runtime_statuses: Arc<Mutex<Vec<String>>>,
        disk_presence_ids: Arc<Mutex<Vec<String>>>,
    }

    impl DiskRuntimeEventPublisher for MockEventPublisher {
        fn publish_disk_runtime(&self, record: &DiskRuntimeRecord) {
            self.runtime_statuses
                .lock()
                .expect("runtime status mutex poisoned")
                .push(record.runtime_status.clone());
            self.disk_presence_ids
                .lock()
                .expect("disk presence mutex poisoned")
                .push(record.disk_presence_id.clone());
        }
    }

    #[tokio::test]
    async fn rejects_non_ext4_before_center_verify() {
        let mount = temp_mount("non-ext4");
        let ledger = MockLedger::default();
        let detector = detector(vec![disk(&mount, "xfs")], ledger.clone());

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "REJECTED");
        assert_eq!(
            records[0].last_error_code.as_deref(),
            Some("FILESYSTEM_UNSUPPORTED")
        );
        assert!(!records[0].task_pool_eligible);
        assert_eq!(ledger.records.lock().unwrap().len(), 1);
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn records_recovery_required_for_edge_copying_residue() {
        let mount = temp_mount("edge-copying");
        write_disk_info(&mount, "EDGE_COPYING");
        let detector = detector(vec![disk(&mount, "ext4")], MockLedger::default());

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "REJECTED");
        assert_eq!(
            records[0].last_error_code.as_deref(),
            Some("RECOVERY_REQUIRED")
        );
        assert!(!records[0].task_pool_eligible);
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
        let detector = detector(vec![disk(&mount, "ext4")], MockLedger::default());

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "REJECTED");
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
    async fn rejects_uninitialized_ext4_candidate_without_protocol_marker() {
        let mount = temp_mount("uninitialized-candidate");
        fs::remove_file(mount.join(PROTOCOL_ROOT).join(DISK_INFO_FILE)).ok();
        let ledger = MockLedger::default();
        let detector = detector(vec![disk(&mount, "ext4")], ledger.clone());

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].runtime_status, "REJECTED");
        assert_eq!(records[0].status_code.as_deref(), Some("UNREGISTERED"));
        assert_eq!(
            records[0].last_error_code.as_deref(),
            Some("MANIFEST_INVALID")
        );
        assert!(records[0]
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("missing rustfs-transfer/disk_info.json"));
        assert!(!records[0].task_pool_eligible);
        assert_eq!(records[0].disk_enabled, Some(false));
        assert!(records[0].disk_id.is_none());
        assert_eq!(ledger.records.lock().unwrap().len(), 1);
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn mixed_protocol_and_uninitialized_candidates_keep_distinct_identity() {
        let protocol_mount = temp_mount("mixed-protocol-imported");
        write_disk_info(&protocol_mount, "IMPORTED");
        let fresh_mount = temp_mount("mixed-uninitialized");
        fs::remove_file(fresh_mount.join(PROTOCOL_ROOT).join(DISK_INFO_FILE)).ok();

        let mut protocol_disk = disk(&protocol_mount, "ext4");
        protocol_disk.sn = "SN-PROTOCOL".to_string();
        protocol_disk.device_path = "/dev/sdb1".to_string();
        protocol_disk.capacity_bytes = 100 * GIB;
        protocol_disk.free_bytes = 40 * GIB;

        let mut fresh_disk = disk(&fresh_mount, "ext4");
        fresh_disk.sn = "SN-FRESH".to_string();
        fresh_disk.device_path = "/dev/sdc1".to_string();
        fresh_disk.capacity_bytes = 200 * GIB;
        fresh_disk.free_bytes = 120 * GIB;
        fresh_disk.fs_uuid = Some("fresh-fs-uuid".to_string());
        fresh_disk.label = Some("FRESH-NOT-INIT".to_string());

        let detector = detector(
            vec![protocol_disk.clone(), fresh_disk.clone()],
            MockLedger::default(),
        );

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records.len(), 2);
        let protocol = records
            .iter()
            .find(|record| record.device_path == "/dev/sdb1")
            .unwrap();
        let fresh = records
            .iter()
            .find(|record| record.device_path == "/dev/sdc1")
            .unwrap();

        assert_eq!(protocol.sn, "SN-PROTOCOL");
        assert_eq!(
            protocol.disk_id.as_deref(),
            Some("11111111-1111-1111-1111-111111111111")
        );
        assert_eq!(protocol.status_code.as_deref(), Some("IMPORTED"));
        assert_eq!(protocol.capacity_bytes, 100 * GIB);
        assert_eq!(protocol.runtime_status, "REJECTED");

        assert_eq!(fresh.sn, "SN-FRESH");
        assert!(fresh.disk_id.is_none());
        assert_eq!(fresh.status_code.as_deref(), Some("UNREGISTERED"));
        assert_eq!(fresh.capacity_bytes, 200 * GIB);
        assert_eq!(
            fresh.mount_path.as_deref(),
            Some(fresh_mount.to_str().unwrap())
        );
        assert_eq!(fresh.runtime_status, "REJECTED");
        assert_eq!(fresh.last_error_code.as_deref(), Some("MANIFEST_INVALID"));

        fs::remove_dir_all(protocol_mount).ok();
        fs::remove_dir_all(fresh_mount).ok();
    }

    #[tokio::test]
    async fn ready_after_initialized_disk_passes_local_checks_without_center_verify() {
        let mount = temp_mount("ready");
        write_disk_info(&mount, "INITIALIZED");
        let ledger = MockLedger::default();
        let detector = detector(vec![disk(&mount, "ext4")], ledger.clone());

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "READY");
        assert_eq!(records[0].status_code.as_deref(), Some("INITIALIZED"));
        assert_eq!(records[0].disk_enabled, Some(true));
        assert!(records[0].task_pool_eligible);
        let persisted = ledger.records.lock().unwrap().first().cloned().unwrap();
        assert_eq!(
            persisted.disk_id.as_deref(),
            Some("11111111-1111-1111-1111-111111111111")
        );
        assert_eq!(persisted.runtime_status, "READY");
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn ready_write_probe_does_not_leave_probe_files() {
        let mount = temp_mount("ready-write-probe-cleanup");
        write_disk_info(&mount, "INITIALIZED");
        let detector = detector(vec![disk(&mount, "ext4")], MockLedger::default());

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "READY");
        let protocol_root = mount.join(PROTOCOL_ROOT);
        let probe_left = fs::read_dir(&protocol_root)
            .unwrap()
            .flatten()
            .any(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(".edge-write-probe-"))
            });
        assert!(!probe_left);
        fs::remove_dir_all(mount).ok();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_initialized_disk_when_protocol_root_is_not_writable() {
        let mount = temp_mount("ready-write-probe-denied");
        write_disk_info(&mount, "INITIALIZED");
        let protocol_root = mount.join(PROTOCOL_ROOT);
        let original_permissions = fs::metadata(&protocol_root).unwrap().permissions();
        fs::set_permissions(&protocol_root, fs::Permissions::from_mode(0o555)).unwrap();
        let detector = detector(vec![disk(&mount, "ext4")], MockLedger::default());

        let records = detector.scan_existing_transport_disks().await.unwrap();

        fs::set_permissions(&protocol_root, original_permissions).unwrap();
        assert_eq!(records[0].runtime_status, "REJECTED");
        assert_eq!(
            records[0].last_error_code.as_deref(),
            Some("DISK_WRITE_PERMISSION_DENIED")
        );
        assert!(!records[0].task_pool_eligible);
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn publishes_only_final_ready_runtime_event() {
        let mount = temp_mount("runtime-events-ready");
        write_disk_info(&mount, "INITIALIZED");
        let publisher = MockEventPublisher::default();
        let detector = EdgeDiskDetector::new_with_event_publisher(
            EdgeDiskDetectorConfig::new("edge-a"),
            MockProbe {
                disks: vec![disk(&mount, "ext4")],
            },
            MockLedger::default(),
            publisher.clone(),
        );

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "READY");
        assert_eq!(
            publisher.runtime_statuses.lock().unwrap().as_slice(),
            &["READY"]
        );
        let disk_presence_ids = publisher.disk_presence_ids.lock().unwrap();
        assert_eq!(disk_presence_ids.len(), 1);
        assert!(disk_presence_ids
            .iter()
            .all(|value| value == &disk_presence_ids[0]));
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn publishes_only_final_rejected_runtime_event() {
        let mount = temp_mount("runtime-events-rejected");
        fs::remove_file(mount.join(PROTOCOL_ROOT).join(DISK_INFO_FILE)).ok();
        let publisher = MockEventPublisher::default();
        let detector = EdgeDiskDetector::new_with_event_publisher(
            EdgeDiskDetectorConfig::new("edge-a"),
            MockProbe {
                disks: vec![disk(&mount, "ext4")],
            },
            MockLedger::default(),
            publisher.clone(),
        );

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "REJECTED");
        assert_eq!(
            records[0].status_code.as_deref(),
            Some(DiskStatusCode::Unregistered.as_protocol_value())
        );
        assert_eq!(
            publisher.runtime_statuses.lock().unwrap().as_slice(),
            &["REJECTED"]
        );
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn empty_rescan_marks_previous_runtime_removed_and_publishes_event() {
        let mount = temp_mount("removed-rescan");
        write_disk_info(&mount, "INITIALIZED");
        let mut previous = base_record(&disk(&mount, "ext4"), Uuid::new_v4().to_string());
        previous.disk_id = Some("11111111-1111-1111-1111-111111111111".to_string());
        previous.runtime_status = RuntimeStatus::Ready.as_db_value().to_string();
        previous.status_code = Some("INITIALIZED".to_string());
        previous.task_pool_eligible = true;
        let ledger = MockLedger::with_existing(vec![previous]);
        let publisher = MockEventPublisher::default();
        let detector = EdgeDiskDetector::new_with_event_publisher(
            EdgeDiskDetectorConfig::new("edge-a"),
            MockProbe { disks: Vec::new() },
            ledger.clone(),
            publisher.clone(),
        );

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].runtime_status, "REMOVED");
        assert_eq!(records[0].last_error_code.as_deref(), Some("DISK_REMOVED"));
        assert!(!records[0].task_pool_eligible);
        assert_eq!(ledger.records.lock().unwrap().len(), 1);
        assert_eq!(
            publisher.runtime_statuses.lock().unwrap().as_slice(),
            &["REMOVED"]
        );
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn empty_rescan_marks_previous_uninitialized_candidate_removed_by_location() {
        let mount = temp_mount("removed-uninitialized-candidate");
        let mut previous = base_record(&disk(&mount, "ext4"), Uuid::new_v4().to_string());
        previous.runtime_status = RuntimeStatus::Rejected.as_db_value().to_string();
        previous.status_code = Some("UNREGISTERED".to_string());
        previous.last_error_code = Some(DiskErrorCode::ManifestInvalid.as_db_value().to_string());
        let ledger = MockLedger::with_existing(vec![previous]);
        let detector = EdgeDiskDetector::new(
            EdgeDiskDetectorConfig::new("edge-a"),
            MockProbe { disks: Vec::new() },
            ledger.clone(),
        );

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].runtime_status, "REMOVED");
        assert_eq!(records[0].last_error_code.as_deref(), Some("DISK_REMOVED"));
        assert!(records[0].disk_id.is_none());
        assert_eq!(ledger.records.lock().unwrap().len(), 1);
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn refreshed_disk_info_id_becomes_runtime_identity_without_cache_requirement() {
        let mount = temp_mount("refreshed-disk-id");
        write_disk_info_with_disk_id(
            &mount,
            "INITIALIZED",
            "33333333-3333-3333-3333-333333333333",
        );
        let ledger = MockLedger::default();
        let detector = detector(vec![disk(&mount, "ext4")], ledger.clone());

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "READY");
        assert_eq!(
            records[0].disk_id.as_deref(),
            Some("33333333-3333-3333-3333-333333333333")
        );
        assert!(records[0].task_pool_eligible);
        assert_eq!(
            ledger.records.lock().unwrap()[0].disk_id.as_deref(),
            Some("33333333-3333-3333-3333-333333333333")
        );
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn rejects_missing_hardware_sn_before_center_verify() {
        let mount = temp_mount("missing-sn");
        write_disk_info(&mount, "INITIALIZED");
        let mut detected = disk(&mount, "ext4");
        detected.sn.clear();
        detected.id_serial = None;
        detected.id_serial_short = None;
        let detector = detector(vec![detected], MockLedger::default());

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "REJECTED");
        assert_eq!(
            records[0].last_error_code.as_deref(),
            Some("HARDWARE_SN_UNAVAILABLE")
        );
        assert!(!records[0].task_pool_eligible);
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn recognizes_sealed_disk_as_completed_without_center_verify() {
        let mount = temp_mount("not-initialized");
        write_disk_info(&mount, "SEALED");
        let detector = detector(vec![disk(&mount, "ext4")], MockLedger::default());

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "DONE");
        assert!(records[0].last_error_code.is_none());
        assert_eq!(records[0].status_code.as_deref(), Some("SEALED"));
        assert_eq!(records[0].disk_enabled, Some(true));
        assert!(!records[0].task_pool_eligible);
        fs::remove_dir_all(mount).ok();
    }

    #[tokio::test]
    async fn missing_center_signature_rejects_initialized_disk_locally() {
        let mount = temp_mount("signature-missing");
        write_disk_info_without_center_signature(&mount, "INITIALIZED");
        let detector = detector(vec![disk(&mount, "ext4")], MockLedger::default());

        let records = detector.scan_existing_transport_disks().await.unwrap();

        assert_eq!(records[0].runtime_status, "REJECTED");
        assert_eq!(
            records[0].last_error_code.as_deref(),
            Some("SIGNATURE_INVALID")
        );
        assert!(!records[0].task_pool_eligible);
        fs::remove_dir_all(mount).ok();
    }

    #[test]
    fn runtime_refresh_sql_replaces_only_current_runtime_snapshots() {
        assert!(REPLACE_CURRENT_RUNTIME_SQL.contains("DELETE FROM disk_runtime"));
        assert!(REPLACE_CURRENT_RUNTIME_SQL.contains("status <> 'COPYING'"));
        assert!(REPLACE_CURRENT_RUNTIME_SQL.contains("device_path = $2"));
        assert!(REPLACE_CURRENT_RUNTIME_SQL.contains("mount_path IS NOT DISTINCT FROM $3"));
        assert!(!REPLACE_CURRENT_RUNTIME_SQL.contains("export_job"));
        assert!(!REPLACE_CURRENT_RUNTIME_SQL.contains("export_object"));
        assert!(!REPLACE_CURRENT_RUNTIME_SQL.contains("manifest"));
    }

    #[test]
    fn missing_disk_removal_updates_existing_runtime_row_instead_of_inserting_duplicate() {
        assert!(MARK_MISSING_RUNTIME_REMOVED_SQL.contains("UPDATE disk_runtime"));
        assert!(MARK_MISSING_RUNTIME_REMOVED_SQL.contains("status = 'REMOVED'"));
        assert!(MARK_MISSING_RUNTIME_REMOVED_SQL.contains("WHERE id = $1"));
        assert!(!MARK_MISSING_RUNTIME_REMOVED_SQL.contains("INSERT INTO disk_runtime"));
    }

    fn detector(
        disks: Vec<DetectedDisk>,
        ledger: MockLedger,
    ) -> EdgeDiskDetector<MockProbe, MockLedger> {
        EdgeDiskDetector::new(
            EdgeDiskDetectorConfig::new("edge-a"),
            MockProbe { disks },
            ledger,
        )
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
        write_disk_info_with_disk_id(mount, status_code, "11111111-1111-1111-1111-111111111111");
    }

    fn write_disk_info_with_disk_id(mount: &Path, status_code: &str, disk_id: &str) {
        let payload = format!(
            r#"{{
  "protocol": {{ "version": "1.0" }},
  "disk": {{ "disk_id": "{disk_id}" }},
  "status": {{ "code": "{status_code}" }},
  "security": {{
    "data_key_id": "22222222-2222-2222-2222-222222222222",
    "center_signature": "test-center-signature"
  }}
}}"#
        );
        fs::write(mount.join(PROTOCOL_ROOT).join(DISK_INFO_FILE), payload).unwrap();
    }

    fn write_disk_info_without_center_signature(mount: &Path, status_code: &str) {
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

        assert_eq!(mounts, vec![root.clone(), child]);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn configured_probe_discovers_child_candidates_without_protocol_marker() {
        let root = std::env::temp_dir().join(format!(
            "rustfs-transfer-edge-test-root-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let child = root.join("fresh-center-init-needed");
        fs::create_dir_all(&child).unwrap();

        let mounts = discover_transport_mounts(&[root.clone()]);

        assert_eq!(mounts, vec![root.clone(), child]);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn configured_probe_discovers_desktop_user_mounts_two_levels_deep() {
        let root = std::env::temp_dir().join(format!(
            "rustfs-transfer-edge-test-root-{}",
            uuid::Uuid::new_v4()
        ));
        let user = root.join("alice");
        let disk = user.join("RFS-A");
        fs::create_dir_all(&disk).unwrap();

        let mounts = discover_transport_mounts(&[root.clone()]);

        assert_eq!(mounts, vec![root.clone(), user, disk]);
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
            bus: property(udev, "ID_BUS"),
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

    #[test]
    fn parses_lsblk_pairs_for_unmounted_usb_partition_candidates() {
        let fields = parse_lsblk_pairs(
            r#"NAME="/dev/sdb1" PKNAME="/dev/sdb" TYPE="part" MOUNTPOINT="" FSTYPE="ntfs" TRAN="usb""#,
        );

        assert_eq!(fields.get("NAME").map(String::as_str), Some("/dev/sdb1"));
        assert_eq!(fields.get("PKNAME").map(String::as_str), Some("/dev/sdb"));
        assert_eq!(fields.get("TYPE").map(String::as_str), Some("part"));
        assert_eq!(fields.get("MOUNTPOINT").map(String::as_str), Some(""));
        assert_eq!(fields.get("FSTYPE").map(String::as_str), Some("ntfs"));
        assert_eq!(fields.get("TRAN").map(String::as_str), Some("usb"));
    }

    #[test]
    fn mounted_usb_partition_without_protocol_is_kept_for_rejection() {
        let usb_ntfs = DeviceMetadata {
            bus: Some("usb".to_string()),
            filesystem: Some("ntfs".to_string()),
            ..DeviceMetadata::default()
        };
        let local_ntfs = DeviceMetadata {
            bus: None,
            filesystem: Some("ntfs".to_string()),
            ..DeviceMetadata::default()
        };

        assert!(should_include_mount_candidate(false, "ntfs", &usb_ntfs));
        assert!(!should_include_mount_candidate(false, "ntfs", &local_ntfs));
        assert!(should_include_mount_candidate(false, "ext4", &local_ntfs));
        assert!(should_include_mount_candidate(true, "ntfs", &local_ntfs));
    }

    #[test]
    fn mounted_usb_partition_is_discovered_outside_configured_roots() {
        let candidate = parse_mounted_usb_partition(
            r#"NAME="/dev/sdb1" PKNAME="/dev/sdb" TYPE="part" MOUNTPOINT="/media/alice/RFS-A" FSTYPE="ext4" TRAN="usb""#,
        )
        .expect("mounted usb partition should be detected");

        assert_eq!(candidate.device_path, "/dev/sdb1");
        assert_eq!(candidate.mount_path, PathBuf::from("/media/alice/RFS-A"));
        assert_eq!(candidate.filesystem, "ext4");
    }

    #[test]
    fn mounted_non_usb_partition_is_not_discovered_from_system_mount_table() {
        let candidate = parse_mounted_usb_partition(
            r#"NAME="/dev/sda2" PKNAME="/dev/sda" TYPE="part" MOUNTPOINT="/" FSTYPE="ext4" TRAN="sata""#,
        );

        assert!(candidate.is_none());
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
