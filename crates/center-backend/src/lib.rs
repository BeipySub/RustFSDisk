use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{anyhow, Context, Result};
use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket},
        Path as AxumPath, State, WebSocketUpgrade,
    },
    http::{HeaderMap, Request, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use tokio::{
    sync::RwLock,
    time::{self, Duration},
};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

pub mod center_auth;
pub mod center_security;
pub mod config;
pub mod import_runtime;
pub mod import_worker;
pub mod reinitialize_runtime;
pub mod reinitializer;

use center_security::{
    disk_data_key_base64, CenterSecurity, ENCRYPTION_ALG_AES_256_GCM,
    KEY_WRAP_ALG_LOCAL_MASTER_KEY, SIGNATURE_ALG_HMAC_SHA256,
};
pub use config::CenterConfig;

pub const PROTOCOL_VERSION: &str = "1.0";
const ENCRYPTION_ALG: &str = ENCRYPTION_ALG_AES_256_GCM;
const KEY_WRAP_ALG: &str = KEY_WRAP_ALG_LOCAL_MASTER_KEY;

#[derive(Clone)]
pub struct AppState {
    service: CenterService,
    control_api_token: Option<String>,
    import_control: Option<Arc<dyn import_runtime::CenterImportControlService>>,
    reinitialize_control: Option<Arc<dyn reinitialize_runtime::CenterReinitializeControlService>>,
}

impl AppState {
    pub fn new(service: CenterService) -> Self {
        Self {
            service,
            control_api_token: None,
            import_control: None,
            reinitialize_control: None,
        }
    }

    pub fn with_control_api_token(mut self, token: Option<String>) -> Self {
        self.control_api_token = token;
        self
    }

    pub fn with_import_control(
        mut self,
        import_control: Arc<dyn import_runtime::CenterImportControlService>,
    ) -> Self {
        self.import_control = Some(import_control);
        self
    }

    pub fn with_reinitialize_control(
        mut self,
        reinitialize_control: Arc<dyn reinitialize_runtime::CenterReinitializeControlService>,
    ) -> Self {
        self.reinitialize_control = Some(reinitialize_control);
        self
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health_handler))
        .route("/readyz", get(readiness_handler))
        .route("/api/center/summary", get(center_summary_handler))
        .route("/ws/center/import-progress", get(center_import_progress_ws))
        .route("/ws/center/progress", get(center_import_progress_ws))
        .route("/api/edge/auth", post(center_auth::edge_auth_handler))
        .route("/api/disk/register", post(register_disk_handler))
        .route("/api/disk/initialize", post(initialize_disk_handler))
        .route("/api/disk/verify", post(verify_disk_handler))
        .route("/api/disk/export-key", post(export_key_handler))
        .route("/api/center/import-jobs/start", post(start_import_handler))
        .route(
            "/api/center/disks/{disk_id}/reinitialize",
            post(reinitialize_disk_handler),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDiskRequest {
    pub sn: String,
    pub capacity_bytes: i64,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDiskResponse {
    pub disk_id: Uuid,
    pub disk_enabled: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeDiskRequest {
    pub disk_id: Uuid,
    pub sn: Option<String>,
    pub capacity_bytes: i64,
    pub mount_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeDiskResponse {
    pub disk_id: Uuid,
    pub data_key_id: Uuid,
    pub status_code: DiskStatusCode,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyDiskRequest {
    pub edge_code: String,
    pub disk_id: Uuid,
    pub sn: Option<String>,
    pub capacity_bytes: i64,
    pub free_bytes: i64,
    pub status_code: DiskStatusCode,
    pub protocol_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyDiskResponse {
    pub allowed: bool,
    pub disk_id: Uuid,
    pub disk_enabled: bool,
    pub expected_status: DiskStatusCode,
    pub action: VerifyAction,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportKeyRequest {
    pub edge_code: String,
    pub disk_id: Uuid,
    pub data_key_id: Uuid,
    pub export_job_id: Uuid,
    pub status_code: DiskStatusCode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportKeyResponse {
    pub allowed: bool,
    pub data_key_id: Uuid,
    pub encryption_alg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_data_key: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerifyAction {
    AllowExport,
    Reject,
    NeedInit,
    NeedImportFirst,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataKeyStatus {
    Active,
    Issued,
    SealedReadonly,
    Retired,
    Revoked,
}

impl DataKeyStatus {
    fn from_db(value: &str) -> Result<Self> {
        match value {
            "ACTIVE" => Ok(Self::Active),
            "ISSUED" => Ok(Self::Issued),
            "SEALED_READONLY" => Ok(Self::SealedReadonly),
            "RETIRED" => Ok(Self::Retired),
            "REVOKED" => Ok(Self::Revoked),
            other => Err(anyhow!("unsupported data_key.status: {other}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiskRecord {
    pub disk_id: Uuid,
    pub sn: String,
    pub capacity_bytes: i64,
    pub disk_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct EdgeRecord {
    pub edge_code: String,
    pub edge_name: String,
    pub auth_key_id: String,
    pub auth_secret: String,
    pub edge_status: String,
}

#[derive(Debug, Clone)]
pub struct DataKeyRecord {
    pub data_key_id: Uuid,
    pub disk_id: Uuid,
    pub edge_code: Option<String>,
    pub export_job_id: Option<Uuid>,
    pub encrypted_key: String,
    pub status: DataKeyStatus,
}

#[derive(Debug, Clone)]
pub struct CenterConfigRecord {
    pub center_id: Uuid,
    pub protocol_version: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CenterGlobalProgress {
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CenterImportObject {
    pub bucket: String,
    pub key: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CenterDiskSummary {
    pub disk_id: Uuid,
    pub disk_sn: String,
    pub edge_code: String,
    pub mount_path: Option<String>,
    pub device_path: Option<String>,
    pub filesystem: Option<String>,
    pub filesystem_uuid: Option<String>,
    pub disk_enabled: bool,
    pub registered: bool,
    pub can_initialize: bool,
    pub reusable: bool,
    pub imported_before: bool,
    pub disk_status_code: String,
    pub runtime_status: String,
    pub import_job_id: Option<Uuid>,
    pub import_job_status: Option<String>,
    pub seal_id: Option<Uuid>,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub speed_bytes_per_sec: u64,
    pub current_object: Option<CenterImportObject>,
    pub last_error_code: Option<String>,
    pub error_message: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CenterDashboardSummary {
    pub source: &'static str,
    pub center_id: Option<Uuid>,
    pub center_name: String,
    pub global_progress: CenterGlobalProgress,
    pub disks: Vec<CenterDiskSummary>,
    pub ws_connected: bool,
    pub last_http_refresh_at: DateTime<Utc>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CenterImportProgressEvent {
    pub event_type: String,
    pub event_time: DateTime<Utc>,
    pub source: &'static str,
    pub global_progress: CenterGlobalProgress,
    pub disks: Vec<CenterDiskSummary>,
    pub message: String,
}

#[derive(Clone)]
pub enum CenterStore {
    Pg(PgCenterStore),
    Memory(Arc<RwLock<MemoryCenterStore>>),
}

#[derive(Clone)]
pub struct PgCenterStore {
    pool: PgPool,
}

impl PgCenterStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, Default)]
pub struct MemoryCenterStore {
    pub center_config: Option<CenterConfigRecord>,
    pub disks: HashMap<Uuid, DiskRecord>,
    pub disks_by_sn: HashMap<String, Uuid>,
    pub edges: HashMap<String, EdgeRecord>,
    pub data_keys: HashMap<Uuid, DataKeyRecord>,
}

#[derive(Clone)]
pub struct CenterService {
    store: CenterStore,
    security: CenterSecurity,
}

impl CenterService {
    pub fn new(store: CenterStore, security: CenterSecurity) -> Self {
        Self { store, security }
    }

    pub fn memory(store: MemoryCenterStore) -> Self {
        Self::new(
            CenterStore::Memory(Arc::new(RwLock::new(store))),
            CenterSecurity::test(),
        )
    }

    pub async fn register_disk(&self, req: RegisterDiskRequest) -> Result<RegisterDiskResponse> {
        if req.sn.trim().is_empty() {
            return Err(anyhow!("sn is required for registration audit"));
        }
        if req.capacity_bytes <= 0 {
            return Err(anyhow!("capacity_bytes must be positive"));
        }

        if let Some(existing) = self.store.find_disk_by_sn(req.sn.trim()).await? {
            return Ok(RegisterDiskResponse {
                disk_id: existing.disk_id,
                disk_enabled: existing.disk_enabled,
                message: Some("disk already registered; disk_id remains stable".to_string()),
            });
        }

        let disk = DiskRecord {
            disk_id: Uuid::new_v4(),
            sn: req.sn.trim().to_string(),
            capacity_bytes: req.capacity_bytes,
            disk_enabled: true,
        };
        self.store.insert_disk(disk.clone(), req.remark).await?;

        Ok(RegisterDiskResponse {
            disk_id: disk.disk_id,
            disk_enabled: true,
            message: Some("disk registered".to_string()),
        })
    }

    pub async fn initialize_disk(
        &self,
        req: InitializeDiskRequest,
    ) -> Result<InitializeDiskResponse> {
        let disk = self
            .store
            .find_disk(req.disk_id)
            .await?
            .ok_or_else(|| anyhow!("disk is not registered"))?;

        if !disk.disk_enabled {
            return Err(anyhow!("disk is disabled"));
        }
        if let Some(sn) = &req.sn {
            if !sn.is_empty() && sn != &disk.sn {
                return Err(anyhow!("disk sn differs from registration record"));
            }
        }
        if req.capacity_bytes <= 0 {
            return Err(anyhow!("capacity_bytes must be positive"));
        }

        let center_config = self.store.center_config().await?;
        let data_key_id = Uuid::new_v4();
        let plaintext_key = self.security.generate_disk_data_key();
        let encrypted_key =
            self.security
                .wrap_disk_data_key(disk.disk_id, data_key_id, &plaintext_key)?;
        let key = DataKeyRecord {
            data_key_id,
            disk_id: disk.disk_id,
            edge_code: None,
            export_job_id: None,
            encrypted_key,
            status: DataKeyStatus::Revoked,
        };

        self.store.stage_initializing_data_key(key).await?;
        let disk_info = DiskInfoDocument::initialized(
            &center_config,
            &disk,
            req.capacity_bytes,
            data_key_id,
            &self.security,
        )?;
        write_initialized_disk_info(&req.mount_path, &disk_info)
            .with_context(|| format!("write disk_info.json under {}", req.mount_path.display()))?;
        if let Err(error) = self
            .store
            .activate_initialized_data_key(disk.disk_id, data_key_id, req.capacity_bytes)
            .await
        {
            let mut failed_disk_info = disk_info;
            failed_disk_info.status = DiskInfoStatus {
                code: DiskStatusCode::Error,
                sealed: false,
                imported: false,
                reusable: false,
                last_error: Some("center failed to activate initialized data key".to_string()),
            };
            let _ = write_initialized_disk_info(&req.mount_path, &failed_disk_info);
            return Err(error).context("activate initialized data key after disk_info write");
        }

        Ok(InitializeDiskResponse {
            disk_id: disk.disk_id,
            data_key_id,
            status_code: DiskStatusCode::Initialized,
            message: Some("disk initialized".to_string()),
        })
    }

    pub async fn verify_disk(&self, req: VerifyDiskRequest) -> Result<VerifyDiskResponse> {
        if self.store.active_edge(&req.edge_code).await?.is_none() {
            return Ok(VerifyDiskResponse::reject(
                req.disk_id,
                false,
                VerifyAction::Reject,
                "edge site is not active",
            ));
        }

        let Some(disk) = self.store.find_disk(req.disk_id).await? else {
            return Ok(VerifyDiskResponse::reject(
                req.disk_id,
                false,
                VerifyAction::NeedInit,
                "disk is not registered in center",
            ));
        };

        if !disk.disk_enabled {
            return Ok(VerifyDiskResponse::reject(
                disk.disk_id,
                false,
                VerifyAction::Reject,
                "disk is disabled",
            ));
        }
        if req.protocol_version != PROTOCOL_VERSION {
            return Ok(VerifyDiskResponse::reject(
                disk.disk_id,
                true,
                VerifyAction::Reject,
                "protocol version is unsupported",
            ));
        }
        if let Some(sn) = &req.sn {
            if !sn.is_empty() && sn != &disk.sn {
                return Ok(VerifyDiskResponse::reject(
                    disk.disk_id,
                    true,
                    VerifyAction::Reject,
                    "disk sn differs from registration record",
                ));
            }
        }

        match req.status_code {
            DiskStatusCode::Initialized => Ok(VerifyDiskResponse {
                allowed: true,
                disk_id: disk.disk_id,
                disk_enabled: true,
                expected_status: DiskStatusCode::Initialized,
                action: VerifyAction::AllowExport,
                message: Some("disk can be used for export".to_string()),
            }),
            DiskStatusCode::Sealed
            | DiskStatusCode::CenterImporting
            | DiskStatusCode::Imported => Ok(VerifyDiskResponse::reject(
                disk.disk_id,
                true,
                VerifyAction::NeedImportFirst,
                "disk contains sealed or imported lifecycle data and must return to center first",
            )),
            _ => Ok(VerifyDiskResponse::reject(
                disk.disk_id,
                true,
                VerifyAction::Reject,
                "disk lifecycle status is not allowed for edge export",
            )),
        }
    }

    pub async fn export_key(&self, req: ExportKeyRequest) -> Result<ExportKeyResponse> {
        if req.status_code != DiskStatusCode::Initialized {
            return Ok(ExportKeyResponse::denied(
                req.data_key_id,
                "disk status_code must be INITIALIZED before export key issuance",
            ));
        }
        if self.store.active_edge(&req.edge_code).await?.is_none() {
            return Ok(ExportKeyResponse::denied(
                req.data_key_id,
                "edge site is not active",
            ));
        }
        let Some(disk) = self.store.find_disk(req.disk_id).await? else {
            return Ok(ExportKeyResponse::denied(
                req.data_key_id,
                "disk is not registered",
            ));
        };
        if !disk.disk_enabled {
            return Ok(ExportKeyResponse::denied(
                req.data_key_id,
                "disk is disabled",
            ));
        }

        let Some(mut key) = self.store.find_data_key(req.data_key_id).await? else {
            return Ok(ExportKeyResponse::denied(
                req.data_key_id,
                "data key is not registered",
            ));
        };
        if key.disk_id != req.disk_id {
            return Ok(ExportKeyResponse::denied(
                req.data_key_id,
                "data key does not belong to disk_id",
            ));
        }

        match key.status {
            DataKeyStatus::Active => {
                key.status = DataKeyStatus::Issued;
                key.edge_code = Some(req.edge_code.clone());
                key.export_job_id = Some(req.export_job_id);
                self.store.issue_data_key(&key).await?;
                let disk_data_key = self.security.unwrap_disk_data_key(
                    key.disk_id,
                    key.data_key_id,
                    &key.encrypted_key,
                )?;
                Ok(ExportKeyResponse::allowed(
                    key.data_key_id,
                    disk_data_key_base64(&disk_data_key),
                ))
            }
            DataKeyStatus::Issued
                if key.edge_code.as_deref() == Some(req.edge_code.as_str())
                    && key.export_job_id == Some(req.export_job_id) =>
            {
                self.store.touch_data_key(req.data_key_id).await?;
                let disk_data_key = self.security.unwrap_disk_data_key(
                    key.disk_id,
                    key.data_key_id,
                    &key.encrypted_key,
                )?;
                Ok(ExportKeyResponse::allowed(
                    key.data_key_id,
                    disk_data_key_base64(&disk_data_key),
                ))
            }
            DataKeyStatus::Issued => Ok(ExportKeyResponse::denied(
                req.data_key_id,
                "data key is already issued to another edge export job",
            )),
            DataKeyStatus::SealedReadonly | DataKeyStatus::Retired | DataKeyStatus::Revoked => {
                Ok(ExportKeyResponse::denied(
                    req.data_key_id,
                    "data key is not writable for edge export",
                ))
            }
        }
    }

    pub async fn edge_for_auth(&self, edge_code: &str) -> Result<Option<EdgeRecord>> {
        self.store.edge_for_auth(edge_code).await
    }

    pub async fn ready(&self) -> bool {
        self.store.center_config().await.is_ok()
    }

    pub async fn dashboard_summary(&self) -> Result<CenterDashboardSummary> {
        let center = self.store.center_config().await.ok();
        let disks = self.store.center_dashboard_disks().await?;
        Ok(CenterDashboardSummary {
            source: "center",
            center_id: center.as_ref().map(|center| center.center_id),
            center_name: "RustFS Transfer Center".to_string(),
            global_progress: center_global_progress(&disks),
            disks,
            ws_connected: false,
            last_http_refresh_at: Utc::now(),
            message: "center HTTP dashboard summary".to_string(),
        })
    }
}

impl VerifyDiskResponse {
    fn reject(disk_id: Uuid, disk_enabled: bool, action: VerifyAction, message: &str) -> Self {
        Self {
            allowed: false,
            disk_id,
            disk_enabled,
            expected_status: DiskStatusCode::Initialized,
            action,
            message: Some(message.to_string()),
        }
    }
}

impl ExportKeyResponse {
    fn allowed(data_key_id: Uuid, disk_data_key: String) -> Self {
        Self {
            allowed: true,
            data_key_id,
            encryption_alg: ENCRYPTION_ALG.to_string(),
            disk_data_key: Some(disk_data_key),
            message: Some("data key issued".to_string()),
        }
    }

    fn denied(data_key_id: Uuid, message: &str) -> Self {
        Self {
            allowed: false,
            data_key_id,
            encryption_alg: ENCRYPTION_ALG.to_string(),
            disk_data_key: None,
            message: Some(message.to_string()),
        }
    }
}

impl CenterStore {
    async fn find_disk(&self, disk_id: Uuid) -> Result<Option<DiskRecord>> {
        match self {
            Self::Pg(pg) => pg.find_disk(disk_id).await,
            Self::Memory(mem) => Ok(mem.read().await.disks.get(&disk_id).cloned()),
        }
    }

    async fn find_disk_by_sn(&self, sn: &str) -> Result<Option<DiskRecord>> {
        match self {
            Self::Pg(pg) => pg.find_disk_by_sn(sn).await,
            Self::Memory(mem) => {
                let guard = mem.read().await;
                Ok(guard
                    .disks_by_sn
                    .get(sn)
                    .and_then(|disk_id| guard.disks.get(disk_id))
                    .cloned())
            }
        }
    }

    async fn insert_disk(&self, disk: DiskRecord, remark: Option<String>) -> Result<()> {
        match self {
            Self::Pg(pg) => pg.insert_disk(disk, remark).await,
            Self::Memory(mem) => {
                let mut guard = mem.write().await;
                guard.disks_by_sn.insert(disk.sn.clone(), disk.disk_id);
                guard.disks.insert(disk.disk_id, disk);
                Ok(())
            }
        }
    }

    async fn active_edge(&self, edge_code: &str) -> Result<Option<EdgeRecord>> {
        match self {
            Self::Pg(pg) => pg.active_edge(edge_code).await,
            Self::Memory(mem) => Ok(mem
                .read()
                .await
                .edges
                .get(edge_code)
                .filter(|edge| edge.edge_status == "ACTIVE")
                .cloned()),
        }
    }

    async fn edge_for_auth(&self, edge_code: &str) -> Result<Option<EdgeRecord>> {
        match self {
            Self::Pg(pg) => pg.edge_for_auth(edge_code).await,
            Self::Memory(mem) => Ok(mem.read().await.edges.get(edge_code).cloned()),
        }
    }

    async fn center_config(&self) -> Result<CenterConfigRecord> {
        match self {
            Self::Pg(pg) => pg.center_config().await,
            Self::Memory(mem) => mem
                .read()
                .await
                .center_config
                .clone()
                .ok_or_else(|| anyhow!("center_config is not initialized")),
        }
    }

    async fn center_dashboard_disks(&self) -> Result<Vec<CenterDiskSummary>> {
        match self {
            Self::Pg(pg) => pg.center_dashboard_disks().await,
            Self::Memory(mem) => {
                let guard = mem.read().await;
                Ok(guard
                    .disks
                    .values()
                    .cloned()
                    .map(memory_disk_summary)
                    .collect())
            }
        }
    }

    async fn stage_initializing_data_key(&self, key: DataKeyRecord) -> Result<()> {
        match self {
            Self::Pg(pg) => pg.stage_initializing_data_key(key).await,
            Self::Memory(mem) => {
                mem.write().await.data_keys.insert(key.data_key_id, key);
                Ok(())
            }
        }
    }

    async fn activate_initialized_data_key(
        &self,
        disk_id: Uuid,
        data_key_id: Uuid,
        capacity_bytes: i64,
    ) -> Result<()> {
        match self {
            Self::Pg(pg) => {
                pg.activate_initialized_data_key(disk_id, data_key_id, capacity_bytes)
                    .await
            }
            Self::Memory(mem) => {
                let mut guard = mem.write().await;
                let key = guard
                    .data_keys
                    .get_mut(&data_key_id)
                    .ok_or_else(|| anyhow!("staged data key is missing"))?;
                if key.disk_id != disk_id || key.status != DataKeyStatus::Revoked {
                    return Err(anyhow!("staged data key is not eligible for activation"));
                }
                key.status = DataKeyStatus::Active;
                for other_key in guard.data_keys.values_mut() {
                    if other_key.disk_id == disk_id
                        && other_key.data_key_id != data_key_id
                        && other_key.status == DataKeyStatus::Active
                        && other_key.edge_code.is_none()
                        && other_key.export_job_id.is_none()
                    {
                        other_key.status = DataKeyStatus::Revoked;
                    }
                }
                if let Some(disk) = guard.disks.get_mut(&disk_id) {
                    disk.capacity_bytes = capacity_bytes;
                }
                Ok(())
            }
        }
    }

    async fn find_data_key(&self, data_key_id: Uuid) -> Result<Option<DataKeyRecord>> {
        match self {
            Self::Pg(pg) => pg.find_data_key(data_key_id).await,
            Self::Memory(mem) => Ok(mem.read().await.data_keys.get(&data_key_id).cloned()),
        }
    }

    async fn issue_data_key(&self, key: &DataKeyRecord) -> Result<()> {
        match self {
            Self::Pg(pg) => pg.issue_data_key(key).await,
            Self::Memory(mem) => {
                mem.write()
                    .await
                    .data_keys
                    .insert(key.data_key_id, key.clone());
                Ok(())
            }
        }
    }

    async fn touch_data_key(&self, data_key_id: Uuid) -> Result<()> {
        match self {
            Self::Pg(pg) => pg.touch_data_key(data_key_id).await,
            Self::Memory(_) => Ok(()),
        }
    }
}

impl PgCenterStore {
    async fn find_disk(&self, disk_id: Uuid) -> Result<Option<DiskRecord>> {
        let row = sqlx::query(
            "SELECT disk_id, sn, capacity_bytes, status FROM disk_list WHERE disk_id = $1",
        )
        .bind(disk_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(disk_from_row).transpose()
    }

    async fn find_disk_by_sn(&self, sn: &str) -> Result<Option<DiskRecord>> {
        let row =
            sqlx::query("SELECT disk_id, sn, capacity_bytes, status FROM disk_list WHERE sn = $1")
                .bind(sn)
                .fetch_optional(&self.pool)
                .await?;
        row.map(disk_from_row).transpose()
    }

    async fn insert_disk(&self, disk: DiskRecord, remark: Option<String>) -> Result<()> {
        sqlx::query(
            "INSERT INTO disk_list (disk_id, sn, capacity_bytes, status, remark) VALUES ($1, $2, $3, TRUE, $4)",
        )
        .bind(disk.disk_id)
        .bind(disk.sn)
        .bind(disk.capacity_bytes)
        .bind(remark)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn active_edge(&self, edge_code: &str) -> Result<Option<EdgeRecord>> {
        let row = sqlx::query(
            "SELECT edge_code, edge_name, auth_key_id, auth_secret_ciphertext, status FROM edge_site WHERE edge_code = $1 AND status = 'ACTIVE'",
        )
        .bind(edge_code)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(edge_record_from_row))
    }

    async fn edge_for_auth(&self, edge_code: &str) -> Result<Option<EdgeRecord>> {
        let row = sqlx::query(
            "SELECT edge_code, edge_name, auth_key_id, auth_secret_ciphertext, status FROM edge_site WHERE edge_code = $1",
        )
        .bind(edge_code)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(edge_record_from_row))
    }

    async fn center_config(&self) -> Result<CenterConfigRecord> {
        let row = sqlx::query(
            "SELECT center_id, protocol_version FROM center_config ORDER BY id ASC LIMIT 1",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(CenterConfigRecord {
            center_id: row.get("center_id"),
            protocol_version: row.get("protocol_version"),
        })
    }

    async fn center_dashboard_disks(&self) -> Result<Vec<CenterDiskSummary>> {
        let rows = sqlx::query(
            r#"
            SELECT d.disk_id,
                   d.sn,
                   d.status AS disk_enabled,
                   ij.import_job_id,
                   ij.seal_id,
                   ij.edge_code,
                   ij.status AS import_job_status,
                   ij.object_count,
                   ij.imported_count,
                   ij.total_bytes,
                   ij.imported_bytes,
                   ij.error_message
            FROM disk_list AS d
            LEFT JOIN LATERAL (
              SELECT import_job_id, seal_id, edge_code, status, object_count, imported_count,
                     total_bytes, imported_bytes, error_message
              FROM import_job
              WHERE disk_id = d.disk_id
              ORDER BY id DESC
              LIMIT 1
            ) AS ij ON TRUE
            ORDER BY d.id ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(center_disk_summary_from_row).collect())
    }

    async fn stage_initializing_data_key(&self, key: DataKeyRecord) -> Result<()> {
        sqlx::query(
            "INSERT INTO data_key (data_key_id, disk_id, encryption_alg, encrypted_key, key_wrap_alg, status, remark) VALUES ($1, $2, $3, $4, $5, 'REVOKED', 'initialization staging: not issuable until disk_info write succeeds')",
        )
        .bind(key.data_key_id)
        .bind(key.disk_id)
        .bind(ENCRYPTION_ALG)
        .bind(key.encrypted_key)
        .bind(KEY_WRAP_ALG)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn activate_initialized_data_key(
        &self,
        disk_id: Uuid,
        data_key_id: Uuid,
        capacity_bytes: i64,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let activated = sqlx::query(
            "UPDATE data_key
             SET status = 'ACTIVE',
                 activate_time = (NOW() AT TIME ZONE 'UTC'),
                 remark = NULL
             WHERE data_key_id = $1
               AND disk_id = $2
               AND status = 'REVOKED'
               AND edge_code IS NULL
               AND export_job_id IS NULL
               AND seal_id IS NULL",
        )
        .bind(data_key_id)
        .bind(disk_id)
        .execute(&mut *tx)
        .await?;
        if activated.rows_affected() != 1 {
            return Err(anyhow!("staged data key is not eligible for activation"));
        }

        sqlx::query(
            "UPDATE data_key
             SET status = 'REVOKED',
                 remark = 'superseded by successful disk initialization'
             WHERE disk_id = $1
               AND data_key_id <> $2
               AND status = 'ACTIVE'
               AND edge_code IS NULL
               AND export_job_id IS NULL
               AND seal_id IS NULL",
        )
        .bind(disk_id)
        .bind(data_key_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE disk_list
             SET capacity_bytes = $2,
                 last_init_time = (NOW() AT TIME ZONE 'UTC')
             WHERE disk_id = $1",
        )
        .bind(disk_id)
        .bind(capacity_bytes)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn find_data_key(&self, data_key_id: Uuid) -> Result<Option<DataKeyRecord>> {
        let row = sqlx::query(
            "SELECT data_key_id, disk_id, edge_code, export_job_id, encrypted_key, status FROM data_key WHERE data_key_id = $1",
        )
        .bind(data_key_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok(DataKeyRecord {
                data_key_id: row.get("data_key_id"),
                disk_id: row.get("disk_id"),
                edge_code: row.get("edge_code"),
                export_job_id: row.get("export_job_id"),
                encrypted_key: row.get("encrypted_key"),
                status: DataKeyStatus::from_db(row.get::<String, _>("status").as_str())?,
            })
        })
        .transpose()
    }

    async fn issue_data_key(&self, key: &DataKeyRecord) -> Result<()> {
        let result = sqlx::query(
            "UPDATE data_key SET status = 'ISSUED', edge_code = $2, export_job_id = $3, issued_time = COALESCE(issued_time, (NOW() AT TIME ZONE 'UTC')), last_use_time = (NOW() AT TIME ZONE 'UTC') WHERE data_key_id = $1 AND status = 'ACTIVE'",
        )
        .bind(key.data_key_id)
        .bind(&key.edge_code)
        .bind(key.export_job_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(anyhow!(
                "data key was not ACTIVE during issuance; retry request"
            ));
        }
        Ok(())
    }

    async fn touch_data_key(&self, data_key_id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE data_key SET last_use_time = (NOW() AT TIME ZONE 'UTC') WHERE data_key_id = $1",
        )
        .bind(data_key_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn disk_from_row(row: sqlx::postgres::PgRow) -> Result<DiskRecord> {
    Ok(DiskRecord {
        disk_id: row.get("disk_id"),
        sn: row.get("sn"),
        capacity_bytes: row
            .get::<Option<i64>, _>("capacity_bytes")
            .unwrap_or_default(),
        disk_enabled: row.get("status"),
    })
}

fn edge_record_from_row(row: sqlx::postgres::PgRow) -> EdgeRecord {
    EdgeRecord {
        edge_code: row.get("edge_code"),
        edge_name: row.get("edge_name"),
        auth_key_id: row.get("auth_key_id"),
        auth_secret: row.get("auth_secret_ciphertext"),
        edge_status: row.get("status"),
    }
}

fn center_global_progress(disks: &[CenterDiskSummary]) -> CenterGlobalProgress {
    let total_bytes = disks.iter().map(|disk| disk.total_bytes).sum();
    let done_bytes = disks.iter().map(|disk| disk.done_bytes).sum();
    let object_total = disks.iter().map(|disk| disk.object_total).sum();
    let object_done = disks.iter().map(|disk| disk.object_done).sum();
    CenterGlobalProgress {
        total_bytes,
        done_bytes,
        remaining_bytes: total_bytes.saturating_sub(done_bytes),
        speed_bytes_per_sec: disks.iter().map(|disk| disk.speed_bytes_per_sec).sum(),
        object_total,
        object_done,
        object_remaining: object_total.saturating_sub(object_done),
    }
}

fn center_disk_summary_from_row(row: sqlx::postgres::PgRow) -> CenterDiskSummary {
    let import_job_status = row.get::<Option<String>, _>("import_job_status");
    let disk_enabled = row.get("disk_enabled");
    let disk_status_code = center_disk_status_code(import_job_status.as_deref(), disk_enabled);
    let runtime_status = center_runtime_status(import_job_status.as_deref(), &disk_status_code);
    let error_message = row.get::<Option<String>, _>("error_message");

    CenterDiskSummary {
        disk_id: row.get("disk_id"),
        disk_sn: row.get("sn"),
        edge_code: row
            .get::<Option<String>, _>("edge_code")
            .unwrap_or_else(|| "-".to_string()),
        mount_path: None,
        device_path: None,
        filesystem: Some("ext4".to_string()),
        filesystem_uuid: None,
        disk_enabled,
        registered: true,
        can_initialize: disk_enabled && import_job_status.is_none(),
        reusable: disk_status_code == "INITIALIZED",
        imported_before: import_job_status.as_deref() == Some("DONE"),
        disk_status_code: disk_status_code.clone(),
        runtime_status,
        import_job_id: row.get("import_job_id"),
        import_job_status,
        seal_id: row.get("seal_id"),
        total_bytes: row
            .get::<Option<i64>, _>("total_bytes")
            .unwrap_or_default()
            .max(0) as u64,
        done_bytes: row
            .get::<Option<i64>, _>("imported_bytes")
            .unwrap_or_default()
            .max(0) as u64,
        object_total: row
            .get::<Option<i64>, _>("object_count")
            .unwrap_or_default()
            .max(0) as u64,
        object_done: row
            .get::<Option<i64>, _>("imported_count")
            .unwrap_or_default()
            .max(0) as u64,
        speed_bytes_per_sec: 0,
        current_object: None,
        last_error_code: if error_message.is_some() {
            Some("IMPORT_FAILED".to_string())
        } else {
            None
        },
        error_message,
        message: center_disk_message(&disk_status_code).to_string(),
    }
}

fn memory_disk_summary(disk: DiskRecord) -> CenterDiskSummary {
    CenterDiskSummary {
        disk_id: disk.disk_id,
        disk_sn: disk.sn,
        edge_code: "-".to_string(),
        mount_path: None,
        device_path: None,
        filesystem: Some("ext4".to_string()),
        filesystem_uuid: None,
        disk_enabled: disk.disk_enabled,
        registered: true,
        can_initialize: disk.disk_enabled,
        reusable: false,
        imported_before: false,
        disk_status_code: "REGISTERED".to_string(),
        runtime_status: "DETECTED".to_string(),
        import_job_id: None,
        import_job_status: None,
        seal_id: None,
        total_bytes: 0,
        done_bytes: 0,
        object_total: 0,
        object_done: 0,
        speed_bytes_per_sec: 0,
        current_object: None,
        last_error_code: None,
        error_message: None,
        message: "registered center disk".to_string(),
    }
}

fn center_disk_status_code(import_job_status: Option<&str>, disk_enabled: bool) -> String {
    match import_job_status {
        Some("PENDING") => "SEALED",
        Some("IMPORTING") => "CENTER_IMPORTING",
        Some("DONE") => "IMPORTED",
        Some("FAILED") | Some("CANCELLED") => "ERROR",
        _ if disk_enabled => "REGISTERED",
        _ => "ERROR",
    }
    .to_string()
}

fn center_runtime_status(import_job_status: Option<&str>, disk_status_code: &str) -> String {
    match import_job_status {
        Some("IMPORTING") => "CHECKING",
        Some("DONE") if disk_status_code == "IMPORTED" => "DONE",
        Some("FAILED") | Some("CANCELLED") => "ERROR",
        Some("PENDING") => "READY",
        _ => "DETECTED",
    }
    .to_string()
}

fn center_disk_message(disk_status_code: &str) -> &'static str {
    match disk_status_code {
        "SEALED" => "sealed disk is waiting for center import",
        "CENTER_IMPORTING" => "center import is running",
        "IMPORTED" => "import is done; disk must be reinitialized before reuse",
        "ERROR" => "center disk needs operator attention",
        _ => "registered center disk",
    }
}

async fn health_handler(State(_state): State<AppState>) -> Json<HealthBody> {
    Json(HealthBody {
        ok: true,
        service: "rustfs-transfer-center",
    })
}

async fn readiness_handler(State(state): State<AppState>) -> (StatusCode, Json<HealthBody>) {
    let ok = state.service.ready().await;
    let status = if ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(HealthBody {
            ok,
            service: "rustfs-transfer-center",
        }),
    )
}

async fn center_summary_handler(
    State(state): State<AppState>,
) -> Result<Json<CenterDashboardSummary>, ApiError> {
    state
        .service
        .dashboard_summary()
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn center_import_progress_ws(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| async move {
        publish_center_import_progress(socket, state.service.clone()).await;
    })
}

async fn publish_center_import_progress(mut socket: WebSocket, service: CenterService) {
    let cached = service.dashboard_summary().await.ok();
    let mut interval = time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        let event = cached
            .as_ref()
            .map(center_progress_event_from_summary)
            .unwrap_or_else(idle_center_import_progress_event);
        let Ok(payload) = serde_json::to_string(&event) else {
            break;
        };
        if socket.send(Message::Text(payload.into())).await.is_err() {
            break;
        }
    }
}

fn center_progress_event_from_summary(
    summary: &CenterDashboardSummary,
) -> CenterImportProgressEvent {
    CenterImportProgressEvent {
        event_type: "IMPORT_PROGRESS".to_string(),
        event_time: Utc::now(),
        source: "center",
        global_progress: summary.global_progress.clone(),
        disks: summary.disks.clone(),
        message: "center import progress snapshot".to_string(),
    }
}

fn idle_center_import_progress_event() -> CenterImportProgressEvent {
    CenterImportProgressEvent {
        event_type: "IMPORT_PROGRESS".to_string(),
        event_time: Utc::now(),
        source: "center",
        global_progress: CenterGlobalProgress {
            total_bytes: 0,
            done_bytes: 0,
            remaining_bytes: 0,
            speed_bytes_per_sec: 0,
            object_total: 0,
            object_done: 0,
            object_remaining: 0,
        },
        disks: Vec::new(),
        message: "no center import progress snapshot".to_string(),
    }
}

#[derive(Debug, Serialize)]
struct HealthBody {
    ok: bool,
    service: &'static str,
}

#[derive(Debug, Serialize)]
struct DiskInfoDocument {
    protocol: DiskInfoProtocol,
    disk: DiskInfoDisk,
    edge: DiskInfoEdge,
    center: DiskInfoCenter,
    manifest: DiskInfoManifest,
    security: DiskInfoSecurity,
    status: DiskInfoStatus,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct DiskInfoProtocol {
    name: String,
    version: String,
}

#[derive(Debug, Serialize)]
struct DiskInfoDisk {
    disk_id: Uuid,
    sn: String,
    capacity_bytes: i64,
    last_init_time: String,
    initialized_by: String,
}

#[derive(Debug, Serialize)]
struct DiskInfoEdge {
    edge_name: String,
    edge_code: String,
    seal_id: String,
    export_job_id: String,
    export_started_at: String,
    export_finished_at: String,
}

#[derive(Debug, Serialize)]
struct DiskInfoCenter {
    center_id: Uuid,
    import_job_id: String,
    import_started_at: String,
    import_finished_at: String,
}

#[derive(Debug, Serialize)]
struct DiskInfoManifest {
    manifest_path: String,
    manifest_sha256_path: String,
    object_count: u64,
    total_bytes: u64,
    manifest_sha256: String,
}

#[derive(Debug, Serialize)]
struct DiskInfoSecurity {
    center_key_id: Uuid,
    data_key_id: Uuid,
    encryption_alg: String,
    signature_alg: String,
    center_signature: String,
}

#[derive(Debug, Serialize)]
struct DiskInfoStatus {
    code: DiskStatusCode,
    sealed: bool,
    imported: bool,
    reusable: bool,
    last_error: Option<String>,
}

impl DiskInfoDocument {
    fn initialized(
        center_config: &CenterConfigRecord,
        disk: &DiskRecord,
        capacity_bytes: i64,
        data_key_id: Uuid,
        security: &CenterSecurity,
    ) -> Result<Self> {
        let now = Utc::now();
        let mut document = Self {
            protocol: DiskInfoProtocol {
                name: "rustfs-offline-transfer".to_string(),
                version: center_config.protocol_version.clone(),
            },
            disk: DiskInfoDisk {
                disk_id: disk.disk_id,
                sn: disk.sn.clone(),
                capacity_bytes,
                last_init_time: now.to_rfc3339(),
                initialized_by: "center".to_string(),
            },
            edge: DiskInfoEdge {
                edge_name: String::new(),
                edge_code: String::new(),
                seal_id: String::new(),
                export_job_id: String::new(),
                export_started_at: String::new(),
                export_finished_at: String::new(),
            },
            center: DiskInfoCenter {
                center_id: center_config.center_id,
                import_job_id: String::new(),
                import_started_at: String::new(),
                import_finished_at: String::new(),
            },
            manifest: DiskInfoManifest {
                manifest_path: "manifests/export_manifest.json".to_string(),
                manifest_sha256_path: "manifests/export_manifest.sha256".to_string(),
                object_count: 0,
                total_bytes: 0,
                manifest_sha256: String::new(),
            },
            security: DiskInfoSecurity {
                center_key_id: security.center_key_id(),
                data_key_id,
                encryption_alg: ENCRYPTION_ALG.to_string(),
                signature_alg: SIGNATURE_ALG_HMAC_SHA256.to_string(),
                center_signature: String::new(),
            },
            status: DiskInfoStatus {
                code: DiskStatusCode::Initialized,
                sealed: false,
                imported: false,
                reusable: true,
                last_error: None,
            },
            created_at: now,
            updated_at: now,
        };
        document.security.center_signature = security.sign_disk_info(&document)?;
        Ok(document)
    }
}

fn write_initialized_disk_info(mount_path: &Path, document: &DiskInfoDocument) -> Result<()> {
    let root = mount_path.join("rustfs-transfer");
    fs::create_dir_all(root.join("data"))?;
    fs::create_dir_all(root.join("meta"))?;
    fs::create_dir_all(root.join("manifests"))?;
    fs::create_dir_all(root.join("logs"))?;
    fs::create_dir_all(root.join("quarantine").join("partial"))?;

    let disk_info_path = root.join("disk_info.json");
    let tmp_path = root.join("disk_info.json.tmp");
    let bytes = serde_json::to_vec_pretty(document)?;

    {
        let mut file = File::create(&tmp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp_path, &disk_info_path)?;
    sync_directory_best_effort(&root)?;
    Ok(())
}

fn sync_directory_best_effort(path: &Path) -> Result<()> {
    match File::open(path).and_then(|file| file.sync_all()) {
        Ok(()) => Ok(()),
        Err(err) if cfg!(windows) && err.kind() == std::io::ErrorKind::PermissionDenied => Ok(()),
        Err(err) => Err(err.into()),
    }
}

async fn register_disk_handler(
    State(state): State<AppState>,
    Json(req): Json<RegisterDiskRequest>,
) -> Result<Json<RegisterDiskResponse>, ApiError> {
    state
        .service
        .register_disk(req)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn initialize_disk_handler(
    State(state): State<AppState>,
    Json(req): Json<InitializeDiskRequest>,
) -> Result<Json<InitializeDiskResponse>, ApiError> {
    state
        .service
        .initialize_disk(req)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn verify_disk_handler(
    State(state): State<AppState>,
    request: Request<Body>,
) -> Result<Json<VerifyDiskResponse>, EdgeApiError> {
    let authenticated = center_auth::authenticate_edge_request(&state, request).await?;
    let req = serde_json::from_slice::<VerifyDiskRequest>(&authenticated.body)
        .map_err(|err| ApiError(anyhow!("invalid verify request json: {err}")))?;
    state
        .service
        .verify_disk(req)
        .await
        .map(Json)
        .map_err(ApiError::from)
        .map_err(Into::into)
}

async fn export_key_handler(
    State(state): State<AppState>,
    request: Request<Body>,
) -> Result<Json<ExportKeyResponse>, EdgeApiError> {
    let authenticated = center_auth::authenticate_edge_request(&state, request).await?;
    let req = serde_json::from_slice::<ExportKeyRequest>(&authenticated.body)
        .map_err(|err| ApiError(anyhow!("invalid export-key request json: {err}")))?;
    state
        .service
        .export_key(req)
        .await
        .map(Json)
        .map_err(ApiError::from)
        .map_err(Into::into)
}

async fn start_import_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<import_runtime::CenterImportRequest>,
) -> Result<Json<import_runtime::CenterImportResponse>, ApiError> {
    authorize_center_control_api(&state, &headers)?;
    let import_control = state
        .import_control
        .as_ref()
        .ok_or_else(|| anyhow!("center import control service is not configured"))?;
    import_control
        .import_disk(req)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn reinitialize_disk_handler(
    State(state): State<AppState>,
    AxumPath(disk_id): AxumPath<Uuid>,
    headers: HeaderMap,
    Json(req): Json<reinitialize_runtime::CenterReinitializeRequest>,
) -> Result<Json<reinitialize_runtime::CenterReinitializeResponse>, ApiError> {
    authorize_center_control_api(&state, &headers)?;
    let reinitialize_control = state
        .reinitialize_control
        .as_ref()
        .ok_or_else(|| anyhow!("center reinitialize control service is not configured"))?;
    reinitialize_control
        .reinitialize_disk(disk_id, req)
        .await
        .map(Json)
        .map_err(Into::into)
}

fn authorize_center_control_api(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let configured = state
        .control_api_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("center control API token is not configured"))?;
    let provided = headers
        .get("X-Center-Control-Token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if constant_time_eq(configured.as_bytes(), provided.as_bytes()) {
        Ok(())
    } else {
        Err(anyhow!("center control API token is missing or invalid").into())
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .fold(0_u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error_code: &'static str,
    message: String,
}

struct ApiError(anyhow::Error);

impl From<anyhow::Error> for ApiError {
    fn from(value: anyhow::Error) -> Self {
        Self(value)
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                error_code: "INVALID_REQUEST",
                message: self.0.to_string(),
            }),
        )
            .into_response()
    }
}

enum EdgeApiError {
    Auth(center_auth::AuthError),
    Api(ApiError),
}

impl From<center_auth::AuthError> for EdgeApiError {
    fn from(value: center_auth::AuthError) -> Self {
        Self::Auth(value)
    }
}

impl From<ApiError> for EdgeApiError {
    fn from(value: ApiError) -> Self {
        Self::Api(value)
    }
}

impl axum::response::IntoResponse for EdgeApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Auth(error) => error.into_response(),
            Self::Api(error) => error.into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use axum::{
        body::{to_bytes, Body},
        http::{Method, Request},
    };
    use tower::ServiceExt;

    fn memory_service() -> CenterService {
        let disk_id = Uuid::new_v4();
        let data_key_id = Uuid::new_v4();
        let export_job_id = Uuid::new_v4();
        let security = CenterSecurity::test();
        let mut store = MemoryCenterStore {
            center_config: Some(CenterConfigRecord {
                center_id: Uuid::new_v4(),
                protocol_version: PROTOCOL_VERSION.to_string(),
            }),
            ..Default::default()
        };
        store.disks.insert(
            disk_id,
            DiskRecord {
                disk_id,
                sn: "SN-C3".to_string(),
                capacity_bytes: 1024,
                disk_enabled: true,
            },
        );
        store.disks_by_sn.insert("SN-C3".to_string(), disk_id);
        store.edges.insert(
            "edge-a".to_string(),
            EdgeRecord {
                edge_code: "edge-a".to_string(),
                edge_name: "Edge A".to_string(),
                auth_key_id: "auth-key-a".to_string(),
                auth_secret: "edge-auth-secret".to_string(),
                edge_status: "ACTIVE".to_string(),
            },
        );
        store.data_keys.insert(
            data_key_id,
            DataKeyRecord {
                data_key_id,
                disk_id,
                edge_code: None,
                export_job_id: Some(export_job_id),
                encrypted_key: security
                    .wrap_disk_data_key(disk_id, data_key_id, &[3_u8; 32])
                    .unwrap(),
                status: DataKeyStatus::Active,
            },
        );
        CenterService::new(CenterStore::Memory(Arc::new(RwLock::new(store))), security)
    }

    async fn ids(service: &CenterService) -> (Uuid, Uuid) {
        match &service.store {
            CenterStore::Memory(mem) => {
                let guard = mem.read().await;
                (
                    *guard.disks.keys().next().unwrap(),
                    *guard.data_keys.keys().next().unwrap(),
                )
            }
            CenterStore::Pg(_) => unreachable!(),
        }
    }

    #[tokio::test]
    async fn verify_allows_initialized_enabled_disk() {
        let service = memory_service();
        let (disk_id, _) = ids(&service).await;

        let response = service
            .verify_disk(VerifyDiskRequest {
                edge_code: "edge-a".to_string(),
                disk_id,
                sn: Some("SN-C3".to_string()),
                capacity_bytes: 1024,
                free_bytes: 512,
                status_code: DiskStatusCode::Initialized,
                protocol_version: PROTOCOL_VERSION.to_string(),
            })
            .await
            .unwrap();

        assert!(response.allowed);
        assert_eq!(response.disk_enabled, true);
        assert_eq!(response.expected_status, DiskStatusCode::Initialized);
        assert_eq!(response.action, VerifyAction::AllowExport);
    }

    #[tokio::test]
    async fn verify_rejects_unregistered_disk_without_using_sn_as_identity() {
        let service = memory_service();

        let response = service
            .verify_disk(VerifyDiskRequest {
                edge_code: "edge-a".to_string(),
                disk_id: Uuid::new_v4(),
                sn: Some("SN-C3".to_string()),
                capacity_bytes: 1024,
                free_bytes: 512,
                status_code: DiskStatusCode::Initialized,
                protocol_version: PROTOCOL_VERSION.to_string(),
            })
            .await
            .unwrap();

        assert!(!response.allowed);
        assert_eq!(response.action, VerifyAction::NeedInit);
    }

    #[tokio::test]
    async fn export_key_is_idempotent_for_same_edge_and_export_job() {
        let service = memory_service();
        let (disk_id, data_key_id) = ids(&service).await;
        let export_job_id = Uuid::new_v4();

        let first = service
            .export_key(ExportKeyRequest {
                edge_code: "edge-a".to_string(),
                disk_id,
                data_key_id,
                export_job_id,
                status_code: DiskStatusCode::Initialized,
            })
            .await
            .unwrap();
        let second = service
            .export_key(ExportKeyRequest {
                edge_code: "edge-a".to_string(),
                disk_id,
                data_key_id,
                export_job_id,
                status_code: DiskStatusCode::Initialized,
            })
            .await
            .unwrap();

        assert!(first.allowed);
        assert!(second.allowed);
        assert_eq!(first.disk_data_key, second.disk_data_key);
    }

    #[tokio::test]
    async fn export_key_recovery_cannot_switch_export_job_for_issued_key() {
        let service = memory_service();
        let (disk_id, data_key_id) = ids(&service).await;
        let original_export_job_id = Uuid::new_v4();

        let issued = service
            .export_key(ExportKeyRequest {
                edge_code: "edge-a".to_string(),
                disk_id,
                data_key_id,
                export_job_id: original_export_job_id,
                status_code: DiskStatusCode::Initialized,
            })
            .await
            .unwrap();
        let replay_same_job = service
            .export_key(ExportKeyRequest {
                edge_code: "edge-a".to_string(),
                disk_id,
                data_key_id,
                export_job_id: original_export_job_id,
                status_code: DiskStatusCode::Initialized,
            })
            .await
            .unwrap();
        let different_job = service
            .export_key(ExportKeyRequest {
                edge_code: "edge-a".to_string(),
                disk_id,
                data_key_id,
                export_job_id: Uuid::new_v4(),
                status_code: DiskStatusCode::Initialized,
            })
            .await
            .unwrap();

        assert!(issued.allowed);
        assert!(replay_same_job.allowed);
        assert_eq!(issued.disk_data_key, replay_same_job.disk_data_key);
        assert!(!different_job.allowed);
        assert!(different_job.disk_data_key.is_none());
    }

    #[tokio::test]
    async fn export_key_denies_sealed_retired_and_revoked_keys() {
        for status in [
            DataKeyStatus::SealedReadonly,
            DataKeyStatus::Retired,
            DataKeyStatus::Revoked,
        ] {
            let service = memory_service();
            let (disk_id, data_key_id) = ids(&service).await;
            if let CenterStore::Memory(mem) = &service.store {
                mem.write()
                    .await
                    .data_keys
                    .get_mut(&data_key_id)
                    .unwrap()
                    .status = status;
            }

            let response = service
                .export_key(ExportKeyRequest {
                    edge_code: "edge-a".to_string(),
                    disk_id,
                    data_key_id,
                    export_job_id: Uuid::new_v4(),
                    status_code: DiskStatusCode::Initialized,
                })
                .await
                .unwrap();

            assert!(!response.allowed);
            assert!(response.disk_data_key.is_none());
        }
    }

    #[tokio::test]
    async fn export_key_response_does_not_serialize_expires_at() {
        let response = ExportKeyResponse::allowed(Uuid::new_v4(), "memory-only-key".to_string());
        let value = serde_json::to_value(response).unwrap();

        assert!(value.get("expires_at").is_none());
        assert_eq!(value["encryption_alg"], ENCRYPTION_ALG);
    }

    #[tokio::test]
    async fn initialize_writes_initialized_disk_info() {
        let service = memory_service();
        let (disk_id, _) = ids(&service).await;
        let mount_path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("test-disks")
            .join(format!("rustfs-center-c3-{disk_id}"));
        let _ = fs::remove_dir_all(&mount_path);

        let response = service
            .initialize_disk(InitializeDiskRequest {
                disk_id,
                sn: Some("SN-C3".to_string()),
                capacity_bytes: 2048,
                mount_path: mount_path.clone(),
            })
            .await
            .unwrap();

        let disk_info_path = mount_path.join("rustfs-transfer").join("disk_info.json");
        let disk_info: serde_json::Value =
            serde_json::from_slice(&fs::read(&disk_info_path).unwrap()).unwrap();

        assert_eq!(response.status_code, DiskStatusCode::Initialized);
        assert_eq!(disk_info["disk"]["disk_id"], disk_id.to_string());
        assert_eq!(
            disk_info["security"]["data_key_id"],
            response.data_key_id.to_string()
        );
        assert_eq!(disk_info["status"]["code"], "INITIALIZED");
        assert_eq!(disk_info["status"]["reusable"], true);
        assert!(chrono::DateTime::parse_from_rfc3339(
            disk_info["updated_at"]
                .as_str()
                .expect("initialized disk_info writes updated_at")
        )
        .is_ok());
        assert_ne!(
            disk_info["security"]["center_signature"],
            "mock-signature-adapter"
        );
        service
            .security
            .verify_disk_info(&disk_info)
            .expect("initialized disk_info has a real center signature");
        assert!(disk_info_path.exists());

        fs::remove_dir_all(&mount_path).unwrap();
    }

    #[tokio::test]
    async fn initialize_keeps_plaintext_data_key_out_of_db_and_disk_info() {
        let service = memory_service();
        let (disk_id, _) = ids(&service).await;
        let mount_path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("test-disks")
            .join(format!("rustfs-center-key-wrapping-{disk_id}"));
        let _ = fs::remove_dir_all(&mount_path);

        let response = service
            .initialize_disk(InitializeDiskRequest {
                disk_id,
                sn: Some("SN-C3".to_string()),
                capacity_bytes: 2048,
                mount_path: mount_path.clone(),
            })
            .await
            .unwrap();
        let export_key = service
            .export_key(ExportKeyRequest {
                edge_code: "edge-a".to_string(),
                disk_id,
                data_key_id: response.data_key_id,
                export_job_id: Uuid::new_v4(),
                status_code: DiskStatusCode::Initialized,
            })
            .await
            .unwrap()
            .disk_data_key
            .expect("issued key");

        let guard = match &service.store {
            CenterStore::Memory(mem) => mem.read().await,
            CenterStore::Pg(_) => unreachable!(),
        };
        let stored = guard.data_keys.get(&response.data_key_id).unwrap();
        assert!(stored.encrypted_key.starts_with("local-master-key:v1:"));
        assert!(!stored.encrypted_key.contains(&export_key));

        let disk_info: serde_json::Value = serde_json::from_slice(
            &fs::read(mount_path.join("rustfs-transfer").join("disk_info.json")).unwrap(),
        )
        .unwrap();
        assert!(disk_info
            .to_string()
            .contains(&response.data_key_id.to_string()));
        assert!(!disk_info.to_string().contains(&export_key));

        fs::remove_dir_all(&mount_path).unwrap();
    }

    #[tokio::test]
    async fn export_key_rejects_legacy_mock_wrapped_data_key() {
        let service = memory_service();
        let (disk_id, data_key_id) = ids(&service).await;
        if let CenterStore::Memory(mem) = &service.store {
            mem.write()
                .await
                .data_keys
                .get_mut(&data_key_id)
                .unwrap()
                .encrypted_key = "mock:v1:plaintext-equivalent-key".to_string();
        }

        let error = service
            .export_key(ExportKeyRequest {
                edge_code: "edge-a".to_string(),
                disk_id,
                data_key_id,
                export_job_id: Uuid::new_v4(),
                status_code: DiskStatusCode::Initialized,
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("old mock/plaintext-equivalent"));
    }

    #[tokio::test]
    async fn initialize_write_failure_does_not_leave_active_key_and_retry_uses_single_active_key() {
        let service = memory_service();
        let (disk_id, _) = ids(&service).await;
        if let CenterStore::Memory(mem) = &service.store {
            mem.write().await.data_keys.clear();
        }

        let base = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("test-disks");
        fs::create_dir_all(&base).unwrap();
        let blocked_mount_path = base.join(format!("rustfs-center-c3-blocked-{disk_id}"));
        let _ = fs::remove_file(&blocked_mount_path);
        let _ = fs::remove_dir_all(&blocked_mount_path);
        fs::write(&blocked_mount_path, b"not a directory").unwrap();

        let failed = service
            .initialize_disk(InitializeDiskRequest {
                disk_id,
                sn: Some("SN-C3".to_string()),
                capacity_bytes: 2048,
                mount_path: blocked_mount_path.clone(),
            })
            .await;
        assert!(failed.is_err());
        assert_eq!(
            active_data_key_ids_for_disk(&service, disk_id).await.len(),
            0
        );

        let retry_mount_path = base.join(format!("rustfs-center-c3-retry-{disk_id}"));
        let _ = fs::remove_dir_all(&retry_mount_path);
        let response = service
            .initialize_disk(InitializeDiskRequest {
                disk_id,
                sn: Some("SN-C3".to_string()),
                capacity_bytes: 2048,
                mount_path: retry_mount_path.clone(),
            })
            .await
            .unwrap();

        let active_keys = active_data_key_ids_for_disk(&service, disk_id).await;
        assert_eq!(active_keys, vec![response.data_key_id]);

        let disk_info_path = retry_mount_path
            .join("rustfs-transfer")
            .join("disk_info.json");
        let disk_info: serde_json::Value =
            serde_json::from_slice(&fs::read(&disk_info_path).unwrap()).unwrap();
        assert_eq!(
            disk_info["security"]["data_key_id"],
            response.data_key_id.to_string()
        );

        fs::remove_file(&blocked_mount_path).unwrap();
        fs::remove_dir_all(&retry_mount_path).unwrap();
    }

    #[tokio::test]
    async fn initialize_success_revokes_superseded_unissued_active_keys_for_same_disk() {
        let service = memory_service();
        let (disk_id, _) = ids(&service).await;
        let legacy_orphan_key_id = Uuid::new_v4();
        if let CenterStore::Memory(mem) = &service.store {
            let mut guard = mem.write().await;
            guard.data_keys.clear();
            guard.data_keys.insert(
                legacy_orphan_key_id,
                DataKeyRecord {
                    data_key_id: legacy_orphan_key_id,
                    disk_id,
                    edge_code: None,
                    export_job_id: None,
                    encrypted_key: CenterSecurity::test()
                        .wrap_disk_data_key(disk_id, legacy_orphan_key_id, &[5_u8; 32])
                        .unwrap(),
                    status: DataKeyStatus::Active,
                },
            );
        }

        let mount_path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("test-disks")
            .join(format!("rustfs-center-c3-supersede-{disk_id}"));
        let _ = fs::remove_dir_all(&mount_path);

        let response = service
            .initialize_disk(InitializeDiskRequest {
                disk_id,
                sn: Some("SN-C3".to_string()),
                capacity_bytes: 2048,
                mount_path: mount_path.clone(),
            })
            .await
            .unwrap();

        assert_eq!(
            active_data_key_ids_for_disk(&service, disk_id).await,
            vec![response.data_key_id]
        );
        if let CenterStore::Memory(mem) = &service.store {
            assert_eq!(
                mem.read()
                    .await
                    .data_keys
                    .get(&legacy_orphan_key_id)
                    .unwrap()
                    .status,
                DataKeyStatus::Revoked
            );
        }

        fs::remove_dir_all(&mount_path).unwrap();
    }

    #[tokio::test]
    async fn center_summary_route_uses_semantic_status_fields() {
        let app = router(AppState::new(memory_service()));

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/center/summary")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["source"], "center");
        assert!(body["disks"][0].get("disk_status_code").is_some());
        assert!(body["disks"][0].get("runtime_status").is_some());
        assert!(body["disks"][0].get("disk_enabled").is_some());
        assert!(body["disks"][0].get("status").is_none());
    }

    #[tokio::test]
    async fn center_import_progress_event_has_protocol_shape() {
        let summary = memory_service().dashboard_summary().await.unwrap();
        let event = center_progress_event_from_summary(&summary);
        let value = serde_json::to_value(event).unwrap();

        assert_eq!(value["event_type"], "IMPORT_PROGRESS");
        assert_eq!(value["source"], "center");
        assert!(value["global_progress"].is_object());
        assert!(value["disks"].is_array());
        assert!(value.get("status").is_none());
    }

    #[derive(Default)]
    struct FakeImportControl {
        calls: Mutex<Vec<PathBuf>>,
        fail: bool,
    }

    impl import_runtime::CenterImportControlService for FakeImportControl {
        fn import_disk<'a>(
            &'a self,
            request: import_runtime::CenterImportRequest,
        ) -> import_runtime::CenterImportFuture<'a> {
            Box::pin(async move {
                self.calls.lock().unwrap().push(request.mount_path);
                if self.fail {
                    anyhow::bail!("simulated import failure");
                }
                Ok(import_runtime::CenterImportResponse {
                    import_job_id: Some(Uuid::new_v4()),
                    import_job_status: "DONE".to_string(),
                    outcome: "IMPORTED".to_string(),
                    progress: import_worker::ImportProgressSnapshot::default(),
                    message: "ok".to_string(),
                })
            })
        }
    }

    #[tokio::test]
    async fn controlled_import_route_rejects_missing_token_without_calling_service() {
        let control = Arc::new(FakeImportControl::default());
        let app = router(
            AppState::new(memory_service())
                .with_control_api_token(Some("center-control-token".to_string()))
                .with_import_control(control.clone()),
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/center/import-jobs/start")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"mount_path":"C:\\transport\\disk-a"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(control.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn controlled_import_route_triggers_import_service_with_token() {
        let control = Arc::new(FakeImportControl::default());
        let app = router(
            AppState::new(memory_service())
                .with_control_api_token(Some("center-control-token".to_string()))
                .with_import_control(control.clone()),
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/center/import-jobs/start")
                    .header("content-type", "application/json")
                    .header("X-Center-Control-Token", "center-control-token")
                    .body(Body::from(r#"{"mount_path":"C:\\transport\\disk-a"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["import_job_status"], "DONE");
        assert!(body.get("status").is_none());
        assert_eq!(control.calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn controlled_import_route_propagates_failure_without_success_response() {
        let control = Arc::new(FakeImportControl {
            calls: Mutex::new(Vec::new()),
            fail: true,
        });
        let app = router(
            AppState::new(memory_service())
                .with_control_api_token(Some("center-control-token".to_string()))
                .with_import_control(control.clone()),
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/center/import-jobs/start")
                    .header("content-type", "application/json")
                    .header("X-Center-Control-Token", "center-control-token")
                    .body(Body::from(r#"{"mount_path":"C:\\transport\\disk-a"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(control.calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn controlled_reinitialize_route_triggers_service_with_token() {
        #[derive(Default)]
        struct FakeReinitializeControl {
            calls: Mutex<Vec<(Uuid, PathBuf)>>,
        }

        impl reinitialize_runtime::CenterReinitializeControlService for FakeReinitializeControl {
            fn reinitialize_disk<'a>(
                &'a self,
                disk_id: Uuid,
                request: reinitialize_runtime::CenterReinitializeRequest,
            ) -> reinitialize_runtime::CenterReinitializeFuture<'a> {
                Box::pin(async move {
                    self.calls
                        .lock()
                        .unwrap()
                        .push((disk_id, request.mount_path));
                    Ok(reinitialize_runtime::CenterReinitializeResponse {
                        disk_id,
                        old_seal_id: request.seal_id,
                        old_data_key_id: Uuid::new_v4(),
                        new_data_key_id: Uuid::new_v4(),
                        disk_status_code: reinitializer::DiskStatusCode::Initialized,
                        runtime_status: "DONE".to_string(),
                        message: "ok".to_string(),
                    })
                })
            }
        }

        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let control = Arc::new(FakeReinitializeControl::default());
        let app = router(
            AppState::new(memory_service())
                .with_control_api_token(Some("center-control-token".to_string()))
                .with_reinitialize_control(control.clone()),
        );
        let body = format!(
            r#"{{
                "mount_path":"C:\\transport\\FUSTFS-TST-A",
                "seal_id":"{seal_id}",
                "expected_status_code":"IMPORTED",
                "operator_reason":"VM acceptance cleanup for FUSTFS-TST-A",
                "confirm_reinitialize":true
            }}"#
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/center/disks/{disk_id}/reinitialize"))
                    .header("content-type", "application/json")
                    .header("X-Center-Control-Token", "center-control-token")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["disk_status_code"], "INITIALIZED");
        assert_eq!(body["runtime_status"], "DONE");
        assert!(body.get("status").is_none());
        assert_eq!(control.calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn controlled_reinitialize_route_rejects_missing_token() {
        #[derive(Default)]
        struct FakeReinitializeControl;

        impl reinitialize_runtime::CenterReinitializeControlService for FakeReinitializeControl {
            fn reinitialize_disk<'a>(
                &'a self,
                _disk_id: Uuid,
                _request: reinitialize_runtime::CenterReinitializeRequest,
            ) -> reinitialize_runtime::CenterReinitializeFuture<'a> {
                Box::pin(async { panic!("reinitialize service must not be called without token") })
            }
        }

        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let app = router(
            AppState::new(memory_service())
                .with_control_api_token(Some("center-control-token".to_string()))
                .with_reinitialize_control(Arc::new(FakeReinitializeControl)),
        );
        let body = format!(
            r#"{{
                "mount_path":"C:\\transport\\FUSTFS-TST-A",
                "seal_id":"{seal_id}",
                "expected_status_code":"IMPORTED",
                "operator_reason":"VM acceptance cleanup for FUSTFS-TST-A",
                "confirm_reinitialize":true
            }}"#
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/center/disks/{disk_id}/reinitialize"))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    async fn active_data_key_ids_for_disk(service: &CenterService, disk_id: Uuid) -> Vec<Uuid> {
        match &service.store {
            CenterStore::Memory(mem) => {
                let mut ids = mem
                    .read()
                    .await
                    .data_keys
                    .values()
                    .filter(|key| key.disk_id == disk_id && key.status == DataKeyStatus::Active)
                    .map(|key| key.data_key_id)
                    .collect::<Vec<_>>();
                ids.sort();
                ids
            }
            CenterStore::Pg(_) => unreachable!(),
        }
    }

    async fn json_body(response: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
}
