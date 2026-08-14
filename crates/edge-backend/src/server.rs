use crate::{
    adapters::AdapterBundle,
    auto_export::{AutoExportOrchestrator, AutoExportRescanRunner},
    config::EdgeConfig,
    control::{
        missing_control_service, validate_export_job_id, ControlError, CreateExportJobRequest,
        EdgeControlService, ExportJobRecordsRequest, ProductionEdgeControlService,
        RecoverExportJobRequest, ScanTriggerRequest, StartExportJobRequest,
    },
    disk_detection::{
        ConfiguredMountProbe, EdgeDiskDetector, EdgeDiskDetectorConfig, PgDiskRuntimeLedger,
    },
    progress::{CopyProgressEvent, GlobalProgressSnapshot},
    realtime::EdgeRealtimeHub,
    rescan::{DiskRescanAccepted, DiskRescanCoordinator, DiskRescanTrigger},
    scanner::ScanProgressSnapshot,
};
use axum::response::IntoResponse;
use axum::{
    extract::ws::{Message, WebSocket},
    extract::{Path, Query, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{self, Duration};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<EdgeConfig>,
    pub adapters: AdapterBundle,
    pub disk_rescan: DiskRescanCoordinator,
    control: Arc<dyn EdgeControlService>,
    realtime: EdgeRealtimeHub,
}

impl AppState {
    pub async fn from_config(config: EdgeConfig) -> anyhow::Result<Self> {
        let adapters = AdapterBundle::from_config(&config).await?;
        let pg_pool = adapters
            .pg_pool
            .clone()
            .ok_or_else(|| anyhow::anyhow!("PostgreSQL pool is required for edge control API"))?;
        let s3_client = adapters
            .s3_client
            .clone()
            .ok_or_else(|| anyhow::anyhow!("RustFS S3 client is required for edge control API"))?;
        let config = Arc::new(config);
        let probe = ConfiguredMountProbe::new(
            config
                .paths
                .disk_mount_roots
                .iter()
                .map(Into::into)
                .collect(),
        );
        let ledger =
            PgDiskRuntimeLedger::connect(&config.database.url, config.database.max_connections)
                .await?;
        let control = Arc::new(ProductionEdgeControlService::new(
            config.clone(),
            pg_pool,
            s3_client,
        ));
        let realtime = EdgeRealtimeHub::new(config.center.edge_code.clone());
        let auto_export = AutoExportOrchestrator::new(config.auto_export.clone(), control.clone());
        let detector = EdgeDiskDetector::new_with_event_publisher(
            EdgeDiskDetectorConfig::new(config.center.edge_code.clone()),
            probe,
            ledger,
            realtime.clone(),
        );
        let disk_rescan = DiskRescanCoordinator::new(Arc::new(AutoExportRescanRunner::new(
            Arc::new(detector),
            auto_export,
        )));

        Ok(Self {
            config,
            adapters,
            disk_rescan,
            control,
            realtime,
        })
    }

    pub async fn request_startup_disk_scan(&self) {
        self.disk_rescan
            .request_rescan(DiskRescanTrigger::startup())
            .await;
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(liveness))
        .route("/readyz", get(readiness))
        .route("/internal/disk/rescan", post(request_disk_rescan))
        .route("/api/edge/dashboard/summary", get(edge_dashboard_summary))
        .route(
            "/api/edge/dashboard/export-jobs",
            get(edge_dashboard_export_jobs),
        )
        .route(
            "/api/edge/dashboard/export-jobs/{export_job_id}",
            get(edge_dashboard_get_export_job),
        )
        .route("/api/edge/summary", get(edge_summary))
        .route("/ws/edge/copy-progress", get(edge_copy_progress_ws))
        .route("/ws/edge/progress", get(edge_copy_progress_ws))
        .route("/api/edge/scan", post(trigger_scan))
        .route("/api/edge/export-jobs", post(create_export_job))
        .route("/api/edge/export-jobs/{export_job_id}", get(get_export_job))
        .route(
            "/api/edge/export-jobs/{export_job_id}/start",
            post(start_export_job),
        )
        .route(
            "/api/edge/export-jobs/{export_job_id}/recover",
            post(recover_export_job),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn liveness(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse::alive(&state))
}

async fn request_disk_rescan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DiskRescanRequest>,
) -> (StatusCode, Json<DiskRescanApiResponse>) {
    let Some(expected_token) = state.config.rescan_token() else {
        tracing::error!("edge disk rescan endpoint is disabled because no token is configured");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(DiskRescanApiResponse::error(
                "RESCAN_TOKEN_MISSING",
                "rescan token is not configured",
            )),
        );
    };

    let provided_token = headers
        .get("X-Rescan-Token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim);
    if provided_token != Some(expected_token) {
        tracing::warn!(
            trigger = request.trigger.as_deref(),
            device = request.device.as_deref(),
            "rejected unauthorized edge disk rescan request"
        );
        return (
            StatusCode::UNAUTHORIZED,
            Json(DiskRescanApiResponse::error(
                "UNAUTHORIZED",
                "invalid rescan token",
            )),
        );
    }

    let trigger = if request.trigger.as_deref() == Some("udev") {
        DiskRescanTrigger::udev(request.device)
    } else {
        DiskRescanTrigger::manual(request.device)
    };
    let accepted = state.disk_rescan.request_rescan(trigger).await;

    (
        StatusCode::ACCEPTED,
        Json(DiskRescanApiResponse::accepted(accepted)),
    )
}

async fn readiness(State(state): State<AppState>) -> (StatusCode, Json<ReadinessResponse>) {
    let database_ok = state.adapters.database.check().await.is_ok();
    let rustfs_ok = state.adapters.object_store.check().await.is_ok();
    let disk_mount_roots = state
        .adapters
        .disk
        .mount_roots()
        .into_iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    let ready = database_ok && rustfs_ok && !disk_mount_roots.is_empty();
    let code = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        code,
        Json(ReadinessResponse {
            ok: ready,
            service: "rustfs-transfer-edge",
            edge_code: state.config.center.edge_code.clone(),
            database_ok,
            rustfs_ok,
            disk_mount_roots,
        }),
    )
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    service: &'static str,
    edge_code: String,
}

impl HealthResponse {
    fn alive(state: &AppState) -> Self {
        Self {
            ok: true,
            service: "rustfs-transfer-edge",
            edge_code: state.config.center.edge_code.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ReadinessResponse {
    ok: bool,
    service: &'static str,
    edge_code: String,
    database_ok: bool,
    rustfs_ok: bool,
    disk_mount_roots: Vec<String>,
}

async fn edge_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<crate::control::EdgeControlSummary>, ApiError> {
    authorize_control_api(&state, &headers)?;
    state.control.summary().await.map(Json).map_err(Into::into)
}

async fn edge_dashboard_summary(
    State(state): State<AppState>,
) -> Result<Json<crate::control::EdgeControlSummary>, ApiError> {
    if let Err(err) = state
        .disk_rescan
        .run_rescan_once(DiskRescanTrigger::control_refresh())
        .await
    {
        tracing::warn!(
            error = %err,
            "edge dashboard summary disk refresh failed; returning latest recorded summary"
        );
    }

    state
        .control
        .summary()
        .await
        .map(browser_safe_summary)
        .map(Json)
        .map_err(Into::into)
}

async fn edge_dashboard_export_jobs(
    State(state): State<AppState>,
    Query(request): Query<ExportJobRecordsRequest>,
) -> Result<Json<crate::control::ExportJobRecordsResponse>, ApiError> {
    state
        .control
        .export_jobs(request)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn edge_dashboard_get_export_job(
    State(state): State<AppState>,
    Path(export_job_id): Path<Uuid>,
) -> Result<Json<crate::control::ExportJobResponse>, ApiError> {
    validate_export_job_id(export_job_id)?;
    state
        .control
        .export_job(export_job_id)
        .await
        .map(browser_safe_export_job)
        .map(Json)
        .map_err(Into::into)
}

fn browser_safe_summary(
    summary: crate::control::EdgeControlSummary,
) -> crate::control::EdgeControlSummary {
    summary
}

fn browser_safe_export_job(
    mut job: crate::control::ExportJobResponse,
) -> crate::control::ExportJobResponse {
    for disk in &mut job.disks {
        if disk.disk_status_code.as_deref() == Some("IMPORTED") {
            disk.disk_status_code = Some("ERROR".to_string());
        }
    }
    job
}

async fn edge_copy_progress_ws(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| async move {
        publish_edge_copy_progress(
            socket,
            state.control.clone(),
            state.realtime.clone(),
            state.config.center.edge_code.clone(),
        )
        .await;
    })
}

async fn publish_edge_copy_progress(
    mut socket: WebSocket,
    control: Arc<dyn EdgeControlService>,
    realtime: EdgeRealtimeHub,
    edge_code: String,
) {
    let mut interval = time::interval(Duration::from_secs(1));
    let mut receiver = realtime.subscribe();
    loop {
        let event = tokio::select! {
            biased;
            received = receiver.recv() => {
                match received {
                    Ok(event) => event,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "edge websocket client lagged behind realtime events");
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = interval.tick() => {
                let copy_event = match control.copy_progress_snapshot().await {
                    Ok(event) => event.map(edge_ws_v2_copy_progress_event),
                    Err(error) => {
                        tracing::warn!(
                            error_code = error.error_code,
                            message = error.message,
                            "failed to load edge copy progress snapshot"
                        );
                        None
                    }
                };
                let scan_event = match control.scan_progress_snapshot().await {
                    Ok(snapshot) if snapshot.scan_phase != "IDLE" => {
                        Some(scan_progress_event(&edge_code, snapshot))
                    }
                    Ok(_) => None,
                    Err(error) => {
                        tracing::warn!(
                            error_code = error.error_code,
                            message = error.message,
                            "failed to load edge scan progress snapshot"
                        );
                        None
                    }
                };
                let Some(event) = copy_event.or(scan_event) else {
                    continue;
                };
                event
            }
        };
        let Ok(payload) = serde_json::to_string(&event) else {
            break;
        };
        if socket.send(Message::Text(payload.into())).await.is_err() {
            break;
        }
    }
}

fn scan_progress_event(edge_code: &str, snapshot: ScanProgressSnapshot) -> CopyProgressEvent {
    let global_progress = GlobalProgressSnapshot {
        total_bytes: snapshot.total_bytes,
        done_bytes: 0,
        remaining_bytes: snapshot.total_bytes,
        speed_bytes_per_sec: 0,
        object_total: snapshot.object_seen,
        object_done: snapshot.stable_object_count,
        object_remaining: snapshot
            .object_seen
            .saturating_sub(snapshot.stable_object_count),
        percent: 0.0,
    };
    CopyProgressEvent {
        protocol_version: "edge-ws-v2".to_string(),
        event_id: Uuid::new_v4().to_string(),
        event_type: "COPY_PROGRESS".to_string(),
        event_time: snapshot.event_time,
        source: snapshot.source.to_string(),
        edge_code: edge_code.to_string(),
        stage: Some("SCANNING_RUSTFS".to_string()),
        edge_name: edge_code.to_string(),
        scan: Some(serde_json::json!({
            "scan_status": snapshot.scan_phase,
            "bucket_count": snapshot.bucket_total,
            "object_seen": snapshot.object_seen,
            "stable_object_count": snapshot.stable_object_count,
            "source_changed_count": snapshot.source_changed_count,
            "total_bytes": snapshot.total_bytes,
            "current_bucket": snapshot.current_bucket,
            "current_object_key": snapshot.current_object_key,
            "last_error_code": snapshot.last_error_code,
        })),
        object_inventory: Default::default(),
        export_job: None,
        global: global_progress.clone(),
        global_progress,
        disk_runtime: Vec::new(),
        disks: Vec::new(),
        ws_connected: true,
        last_http_refresh_at: chrono::Utc::now(),
        message: snapshot
            .message
            .unwrap_or_else(|| format!("scan_phase={}", snapshot.scan_phase)),
    }
}

fn edge_ws_v2_copy_progress_event(mut event: CopyProgressEvent) -> CopyProgressEvent {
    event.protocol_version = "edge-ws-v2".to_string();
    if event.event_id.is_empty() {
        event.event_id = Uuid::new_v4().to_string();
    }
    event.event_type = "COPY_PROGRESS".to_string();
    event.stage = Some(
        match event
            .export_job
            .as_ref()
            .map(|job| job.export_job_status.as_str())
        {
            Some("SEALED") => "SEALED",
            Some("FAILED") => "FAILED",
            Some("SEALING") => "SEALING",
            Some("PENDING") | Some("SCANNING") => "PLANNING",
            _ if event.disks.iter().any(|disk| disk.runtime_status == "DONE")
                && event.global_progress.remaining_bytes == 0 =>
            {
                "SEALED"
            }
            _ => "COPYING",
        }
        .to_string(),
    );
    event.scan = None;
    event
}

async fn trigger_scan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ScanTriggerRequest>,
) -> Result<Json<crate::control::ScanTriggerResponse>, ApiError> {
    authorize_control_api(&state, &headers)?;
    refresh_transport_runtime_before_control(&state).await?;
    state
        .control
        .scan_once(request)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn create_export_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateExportJobRequest>,
) -> Result<Json<crate::control::ExportJobResponse>, ApiError> {
    authorize_control_api(&state, &headers)?;
    refresh_transport_runtime_before_control(&state).await?;
    state
        .control
        .create_export_job(request)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn get_export_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(export_job_id): Path<Uuid>,
) -> Result<Json<crate::control::ExportJobResponse>, ApiError> {
    authorize_control_api(&state, &headers)?;
    validate_export_job_id(export_job_id)?;
    state
        .control
        .export_job(export_job_id)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn start_export_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(export_job_id): Path<Uuid>,
    Json(request): Json<StartExportJobRequest>,
) -> Result<Json<crate::control::StartExportJobResponse>, ApiError> {
    authorize_control_api(&state, &headers)?;
    validate_export_job_id(export_job_id)?;
    refresh_transport_runtime_before_control(&state).await?;
    state
        .control
        .start_export_job(export_job_id, request)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn recover_export_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(export_job_id): Path<Uuid>,
    Json(request): Json<RecoverExportJobRequest>,
) -> Result<Json<crate::control::RecoverExportJobResponse>, ApiError> {
    authorize_control_api(&state, &headers)?;
    validate_export_job_id(export_job_id)?;
    state
        .control
        .recover_export_job(export_job_id, request)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn refresh_transport_runtime_before_control(state: &AppState) -> Result<(), ApiError> {
    state
        .disk_rescan
        .run_rescan_once(DiskRescanTrigger::control_refresh())
        .await
        .map(|_| ())
        .map_err(|err| {
            ApiError(ControlError {
                http_status: StatusCode::CONFLICT,
                error_code: "DISK_RESCAN_FAILED",
                message: format!("transport disk rescan failed before control operation: {err}"),
            })
        })
}

fn authorize_control_api(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let configured = state
        .config
        .server
        .control_api_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ApiError(ControlError {
                http_status: StatusCode::SERVICE_UNAVAILABLE,
                error_code: "CONTROL_API_DISABLED",
                message: "edge control API token is not configured".to_string(),
            })
        })?;
    let provided = headers
        .get("X-Edge-Control-Token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();

    if constant_time_eq(configured.as_bytes(), provided.as_bytes()) {
        Ok(())
    } else {
        Err(ApiError(ControlError {
            http_status: StatusCode::UNAUTHORIZED,
            error_code: "UNAUTHORIZED",
            message: "edge control API token is missing or invalid".to_string(),
        }))
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

#[derive(Debug, Deserialize)]
struct DiskRescanRequest {
    trigger: Option<String>,
    device: Option<String>,
}

#[derive(Debug, Serialize)]
struct DiskRescanApiResponse {
    accepted: bool,
    queued: bool,
    error_code: Option<&'static str>,
    message: String,
}

impl DiskRescanApiResponse {
    fn accepted(accepted: DiskRescanAccepted) -> Self {
        Self {
            accepted: accepted.accepted,
            queued: accepted.queued,
            error_code: None,
            message: accepted.message,
        }
    }

    fn error(error_code: &'static str, message: impl Into<String>) -> Self {
        Self {
            accepted: false,
            queued: false,
            error_code: Some(error_code),
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error_code: &'static str,
    message: String,
}

struct ApiError(ControlError);

impl From<ControlError> for ApiError {
    fn from(value: ControlError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.0.http_status,
            Json(ErrorResponse {
                error_code: self.0.error_code,
                message: self.0.message,
            }),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(value: anyhow::Error) -> Self {
        Self(missing_control_service().with_message(value.to_string()))
    }
}

trait ControlErrorExt {
    fn with_message(self, message: String) -> Self;
}

impl ControlErrorExt for ControlError {
    fn with_message(mut self, message: String) -> Self {
        self.message = message;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapters::{
            Clock, DatabaseAdapter, DiskAdapter, HealthFuture, IdGenerator, ObjectStoreAdapter,
        },
        control::{
            ControlFuture, DashboardCurrentObject, DiskRuntimeSummary, EdgeControlSummary,
            EdgeDiskProgressSummary, EdgeGlobalSummary, ExportJobDiskSummary, ExportJobEvent,
            ExportJobResponse, ScanTriggerResponse, StartExportJobResponse,
        },
        disk_detection::DiskDetectionError,
        progress::ObjectInventorySnapshot,
        rescan::{DiskRescanRunner, DiskRescanTrigger},
        scanner::ScanProgressSnapshot,
    };
    use axum::{
        body::{to_bytes, Body},
        http::{Method, Request},
    };
    use chrono::Utc;
    use std::{
        collections::BTreeMap,
        path::PathBuf,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };
    use tower::ServiceExt;

    #[derive(Default)]
    struct FakeHealth;

    impl DatabaseAdapter for FakeHealth {
        fn check<'a>(&'a self) -> HealthFuture<'a> {
            Box::pin(async { Ok(()) })
        }
    }

    impl ObjectStoreAdapter for FakeHealth {
        fn check<'a>(&'a self) -> HealthFuture<'a> {
            Box::pin(async { Ok(()) })
        }
    }

    impl DiskAdapter for FakeHealth {
        fn mount_roots(&self) -> Vec<PathBuf> {
            vec![PathBuf::from("/mnt/rustfs-transfer")]
        }
    }

    impl Clock for FakeHealth {
        fn now_utc(&self) -> chrono::DateTime<Utc> {
            Utc::now()
        }
    }

    impl IdGenerator for FakeHealth {
        fn new_uuid(&self) -> Uuid {
            Uuid::new_v4()
        }
    }

    #[derive(Default)]
    struct NoopRescanRunner;

    impl DiskRescanRunner for NoopRescanRunner {
        fn run_disk_rescan<'a>(
            &'a self,
            _trigger: DiskRescanTrigger,
        ) -> crate::disk_detection::BoxFuture<'a, Result<usize, DiskDetectionError>> {
            Box::pin(async { Ok(0) })
        }
    }

    struct CountingRescanRunner {
        calls: Arc<AtomicUsize>,
    }

    impl DiskRescanRunner for CountingRescanRunner {
        fn run_disk_rescan<'a>(
            &'a self,
            _trigger: DiskRescanTrigger,
        ) -> crate::disk_detection::BoxFuture<'a, Result<usize, DiskDetectionError>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                Ok(1)
            })
        }
    }

    #[derive(Default)]
    struct FakeControl {
        calls: Mutex<Vec<&'static str>>,
        export_job_id: Uuid,
    }

    impl EdgeControlService for FakeControl {
        fn scan_once<'a>(
            &'a self,
            _request: ScanTriggerRequest,
        ) -> ControlFuture<'a, ScanTriggerResponse> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("scan_once");
                Ok(ScanTriggerResponse {
                    scan_event_type: "SCAN_DONE".to_string(),
                    scan_status: "DONE".to_string(),
                    bucket_count: 1,
                    object_seen: 2,
                    stable_object_count: 2,
                    source_changed_count: 0,
                    total_bytes: 99,
                    message: "ok".to_string(),
                })
            })
        }

        fn create_export_job<'a>(
            &'a self,
            _request: CreateExportJobRequest,
        ) -> ControlFuture<'a, ExportJobResponse> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("create_export_job");
                Ok(export_job_response(self.export_job_id))
            })
        }

        fn start_export_job<'a>(
            &'a self,
            export_job_id: Uuid,
            _request: StartExportJobRequest,
        ) -> ControlFuture<'a, StartExportJobResponse> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("start_export_job");
                Ok(StartExportJobResponse {
                    export_job_id,
                    export_job_status: "COPYING".to_string(),
                    assigned_object_count: 2,
                    assigned_bytes: 99,
                    disk_count: 1,
                    worker_started_count: 1,
                    worker_failed_count: 0,
                    message: "assigned".to_string(),
                })
            })
        }

        fn recover_export_job<'a>(
            &'a self,
            export_job_id: Uuid,
            request: RecoverExportJobRequest,
        ) -> ControlFuture<'a, crate::control::RecoverExportJobResponse> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("recover_export_job");
                Ok(crate::control::RecoverExportJobResponse {
                    export_job_id,
                    export_job_status: "COPYING".to_string(),
                    recovered_disk_count: 1,
                    worker_started_count: 1,
                    worker_failed_count: 0,
                    recovery_reason: request.recovery_reason,
                    message: "recovering".to_string(),
                })
            })
        }

        fn export_job<'a>(&'a self, export_job_id: Uuid) -> ControlFuture<'a, ExportJobResponse> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("export_job");
                Ok(export_job_response(export_job_id))
            })
        }

        fn export_jobs<'a>(
            &'a self,
            request: ExportJobRecordsRequest,
        ) -> ControlFuture<'a, crate::control::ExportJobRecordsResponse> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("export_jobs");
                Ok(crate::control::ExportJobRecordsResponse {
                    page: request.page.max(1),
                    page_size: request.page_size.clamp(1, 100),
                    total_count: 1,
                    records: vec![crate::control::ExportJobRecord {
                        export_job_id: self.export_job_id,
                        edge_code: "edge-a".to_string(),
                        export_job_status: request
                            .export_job_status
                            .unwrap_or_else(|| "SEALED".to_string()),
                        object_count: 2,
                        copied_count: 2,
                        total_bytes: 99,
                        copied_bytes: 99,
                        start_time: None,
                        finish_time: None,
                        error_message: None,
                    }],
                })
            })
        }

        fn summary<'a>(&'a self) -> ControlFuture<'a, EdgeControlSummary> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("summary");
                let disk = DiskRuntimeSummary {
                    disk_presence_id: Some(Uuid::new_v4().to_string()),
                    hardware_serial: "SN-A".to_string(),
                    disk_sn: "SN-A".to_string(),
                    stable_hardware_id: "fs-uuid-a".to_string(),
                    disk_id: Some(Uuid::new_v4()),
                    device_path: "/dev/sdb1".to_string(),
                    mount_path: Some("/mnt/rustfs-transfer/disk-a".to_string()),
                    filesystem_type: Some("ext4".to_string()),
                    filesystem: Some("ext4".to_string()),
                    fs_uuid: Some("fs-uuid-a".to_string()),
                    filesystem_uuid: Some("fs-uuid-a".to_string()),
                    disk_status_code: "IMPORTED".to_string(),
                    runtime_status: "READY".to_string(),
                    task_pool_eligible: false,
                    capacity_bytes: 100,
                    total_bytes: 99,
                    done_bytes: 10,
                    remaining_bytes: 89,
                    free_bytes: 80,
                    object_budget_bytes: 64,
                    export_job_id: Some(self.export_job_id.to_string()),
                    seal_id: None,
                    speed_bytes_per_sec: 5,
                    object_total: 2,
                    object_done: 1,
                    object_remaining: 1,
                    progress: EdgeDiskProgressSummary {
                        total_bytes: 99,
                        done_bytes: 10,
                        remaining_bytes: 89,
                        speed_bytes_per_sec: 5,
                        object_total: 2,
                        object_done: 1,
                        object_remaining: 1,
                        percent: 10.1010101010101,
                    },
                    current_object: Some(DashboardCurrentObject {
                        bucket: "test".to_string(),
                        key: "alpha.bin".to_string(),
                        display_name: "alpha.bin".to_string(),
                        relative_data_path: "alpha.bin".to_string(),
                        size_bytes: 99,
                        done_bytes: 10,
                        remaining_bytes: 89,
                        speed_bytes_per_sec: 5,
                        object_status: "COPYING".to_string(),
                    }),
                    current_file: Some("alpha.bin".to_string()),
                    current_file_size: 99,
                    current_file_done: 10,
                    last_error_code: None,
                    error_message: None,
                    message: "disk runtime_status=READY".to_string(),
                };
                let global_progress = EdgeGlobalSummary {
                    total_bytes: 99,
                    done_bytes: 0,
                    remaining_bytes: 99,
                    speed_bytes_per_sec: 0,
                    object_total: 2,
                    object_done: 0,
                    object_remaining: 2,
                    percent: 0.0,
                };
                Ok(EdgeControlSummary {
                    source: "edge",
                    edge_code: "edge-a".to_string(),
                    edge_name: "edge-a".to_string(),
                    object_inventory: ObjectInventorySnapshot {
                        total_bytes: 99,
                        exported_bytes: 10,
                        total_count: 2,
                        exported_count: 1,
                    },
                    export_job: Some(crate::progress::DashboardExportJobSnapshot {
                        export_job_id: self.export_job_id.to_string(),
                        export_job_status: "PENDING".to_string(),
                        start_time: None,
                        finish_time: None,
                        total_bytes: 99,
                        done_bytes: 0,
                        remaining_bytes: 99,
                        speed_bytes_per_sec: 0,
                        object_total: 2,
                        object_done: 0,
                        object_remaining: 2,
                        percent: 0.0,
                    }),
                    global: global_progress.clone(),
                    global_progress,
                    disk_runtime: vec![disk.clone()],
                    disks: vec![disk],
                    ws_connected: false,
                    last_http_refresh_at: Utc::now(),
                    message: "summary".to_string(),
                })
            })
        }

        fn copy_progress_snapshot<'a>(
            &'a self,
        ) -> ControlFuture<'a, Option<crate::progress::CopyProgressEvent>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("copy_progress_snapshot");
                let progress = crate::progress::ProgressAggregator::new(
                    "edge-a",
                    self.export_job_id.to_string(),
                );
                progress.register_disk(
                    "11111111-1111-1111-1111-111111111111",
                    "presence-a",
                    "SN-A",
                    "/mnt/rustfs-transfer/disk-a",
                    100,
                    99,
                    2,
                    80,
                );
                Ok(Some(progress.snapshot("COPY_PROGRESS", "test snapshot")))
            })
        }

        fn scan_progress_snapshot<'a>(&'a self) -> ControlFuture<'a, ScanProgressSnapshot> {
            Box::pin(async move { Ok(ScanProgressSnapshot::default()) })
        }
    }

    #[tokio::test]
    async fn control_routes_require_local_token() {
        let router = app(test_state(Arc::new(FakeControl::default())));

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/edge/scan")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn controlled_summary_requires_token_but_dashboard_summary_is_public_readonly() {
        let control = Arc::new(FakeControl::default());
        let rescan_calls = Arc::new(AtomicUsize::new(0));
        let router = app(test_state_with_rescan(
            control.clone(),
            DiskRescanCoordinator::new(Arc::new(CountingRescanRunner {
                calls: rescan_calls.clone(),
            })),
        ));

        let controlled = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/edge/summary")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(controlled.status(), StatusCode::UNAUTHORIZED);

        let dashboard = router
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/edge/dashboard/summary")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(dashboard.status(), StatusCode::OK);
        let body = json_body(dashboard).await;
        assert_eq!(body["source"], "edge");
        assert_eq!(body["edge_code"], "edge-a");
        assert_eq!(body["edge_name"], "edge-a");
        assert!(body.get("export_job_id").is_none());
        assert!(body.get("export_job_status").is_none());
        assert!(body.get("disk_status_code").is_none());
        assert!(body.get("scan").is_none());
        assert!(body.get("latest_export_job").is_none());
        assert_eq!(body["object_inventory"]["total_bytes"], 99);
        assert_eq!(body["object_inventory"]["exported_count"], 1);
        assert_eq!(body["export_job"]["export_job_status"], "PENDING");
        assert_eq!(body["global"], body["global_progress"]);
        assert_eq!(body["global_progress"]["total_bytes"], 99);
        assert_eq!(body["global_progress"]["percent"], 0.0);
        assert_eq!(body["disk_runtime"], body["disks"]);
        assert_eq!(body["disks"][0]["disk_status_code"], "IMPORTED");
        assert_eq!(body["disks"][0]["hardware_serial"], "SN-A");
        assert_eq!(body["disks"][0]["device_path"], "/dev/sdb1");
        assert_eq!(
            body["disks"][0]["mount_path"],
            "/mnt/rustfs-transfer/disk-a"
        );
        assert_eq!(body["disks"][0]["filesystem_type"], "ext4");
        assert_eq!(body["disks"][0]["fs_uuid"], "fs-uuid-a");
        assert_eq!(body["disks"][0]["runtime_status"], "READY");
        assert_eq!(body["disks"][0]["progress"]["object_total"], 2);
        assert_eq!(body["disks"][0]["progress"]["percent"], 10.1010101010101);
        assert_eq!(
            body["disks"][0]["export_job_id"],
            control.export_job_id.to_string()
        );
        assert_eq!(body["disks"][0]["current_file"], "alpha.bin");
        assert_eq!(body["disks"][0]["current_file_size"], 99);
        assert_eq!(body["disks"][0]["current_file_done"], 10);
        assert_eq!(
            body["disks"][0]["current_object"]["object_status"],
            "COPYING"
        );
        assert!(body.get("status").is_none());
        assert!(body.get("disk_data_key").is_none());
        assert_eq!(control.calls.lock().unwrap().as_slice(), &["summary"]);
        assert_eq!(rescan_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dashboard_export_records_are_public_readonly_without_control_token() {
        let export_job_id = Uuid::new_v4();
        let control = Arc::new(FakeControl {
            export_job_id,
            ..Default::default()
        });
        let router = app(test_state(control.clone()));

        let list = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/edge/dashboard/export-jobs?page=1&page_size=20&export_job_status=SEALED")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list.status(), StatusCode::OK);
        let list_body = json_body(list).await;
        assert_eq!(list_body["records"][0]["export_job_status"], "SEALED");
        assert!(list_body.get("status").is_none());
        assert!(list_body.get("disk_data_key").is_none());

        let detail = router
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/api/edge/dashboard/export-jobs/{export_job_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(detail.status(), StatusCode::OK);
        let detail_body = json_body(detail).await;
        assert_eq!(detail_body["export_job_id"], export_job_id.to_string());
        assert_eq!(detail_body["disks"][0]["disk_status_code"], "ERROR");
        assert_eq!(detail_body["events"][0]["event_type"], "EXPORT_JOB_STARTED");
        assert!(detail_body.get("status").is_none());
        assert!(detail_body.get("disk_data_key").is_none());
        assert_eq!(
            control.calls.lock().unwrap().as_slice(),
            &["export_jobs", "export_job"]
        );
    }

    #[tokio::test]
    async fn scan_route_triggers_control_workflow() {
        let control = Arc::new(FakeControl::default());
        let router = app(test_state(control.clone()));

        let response = router
            .oneshot(authenticated_json(
                Method::POST,
                "/api/edge/scan",
                r#"{"enqueue_stable_objects":false}"#,
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["scan_status"], "DONE");
        assert!(body.get("status").is_none());
        assert_eq!(control.calls.lock().unwrap().as_slice(), &["scan_once"]);
    }

    #[tokio::test]
    async fn scan_and_export_routes_refresh_disk_runtime_before_control_workflow() {
        let export_job_id = Uuid::new_v4();
        let control = Arc::new(FakeControl {
            export_job_id,
            ..Default::default()
        });
        let rescan_calls = Arc::new(AtomicUsize::new(0));
        let router = app(test_state_with_rescan(
            control.clone(),
            DiskRescanCoordinator::new(Arc::new(CountingRescanRunner {
                calls: rescan_calls.clone(),
            })),
        ));

        let scan = router
            .clone()
            .oneshot(authenticated_json(
                Method::POST,
                "/api/edge/scan",
                r#"{"enqueue_stable_objects":false}"#,
            ))
            .await
            .unwrap();
        assert_eq!(scan.status(), StatusCode::OK);

        let create = router
            .clone()
            .oneshot(authenticated_json(
                Method::POST,
                "/api/edge/export-jobs",
                r#"{"run_scan":true}"#,
            ))
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::OK);

        let start = router
            .oneshot(authenticated_json(
                Method::POST,
                &format!("/api/edge/export-jobs/{export_job_id}/start"),
                "{}",
            ))
            .await
            .unwrap();
        assert_eq!(start.status(), StatusCode::OK);

        assert_eq!(rescan_calls.load(Ordering::SeqCst), 3);
        assert_eq!(
            control.calls.lock().unwrap().as_slice(),
            &["scan_once", "create_export_job", "start_export_job"]
        );
    }

    #[tokio::test]
    async fn export_job_routes_create_query_and_start() {
        let export_job_id = Uuid::new_v4();
        let control = Arc::new(FakeControl {
            export_job_id,
            ..Default::default()
        });
        let router = app(test_state(control.clone()));

        let create = router
            .clone()
            .oneshot(authenticated_json(
                Method::POST,
                "/api/edge/export-jobs",
                r#"{"run_scan":true}"#,
            ))
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::OK);
        let create_body = json_body(create).await;
        assert_eq!(create_body["export_job_status"], "PENDING");
        assert!(create_body.get("status").is_none());

        let query = router
            .clone()
            .oneshot(authenticated_empty(
                Method::GET,
                &format!("/api/edge/export-jobs/{export_job_id}"),
            ))
            .await
            .unwrap();
        assert_eq!(query.status(), StatusCode::OK);

        let start = router
            .oneshot(authenticated_json(
                Method::POST,
                &format!("/api/edge/export-jobs/{export_job_id}/start"),
                "{}",
            ))
            .await
            .unwrap();
        assert_eq!(start.status(), StatusCode::OK);
        let start_body = json_body(start).await;
        assert_eq!(start_body["export_job_status"], "COPYING");
        assert_eq!(start_body["assigned_object_count"], 2);

        assert_eq!(
            control.calls.lock().unwrap().as_slice(),
            &["create_export_job", "export_job", "start_export_job"]
        );
    }

    #[tokio::test]
    async fn recover_export_job_route_requires_token() {
        let export_job_id = Uuid::new_v4();
        let control = Arc::new(FakeControl {
            export_job_id,
            ..Default::default()
        });
        let router = app(test_state(control));

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/edge/export-jobs/{export_job_id}/recover"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"recovery_reason":"acl fixed"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn recover_export_job_route_triggers_control_without_naked_status() {
        let export_job_id = Uuid::new_v4();
        let control = Arc::new(FakeControl {
            export_job_id,
            ..Default::default()
        });
        let router = app(test_state(control.clone()));

        let response = router
            .oneshot(authenticated_json(
                Method::POST,
                &format!("/api/edge/export-jobs/{export_job_id}/recover"),
                r#"{"recovery_reason":"ACL repaired by operator","admin_confirm_write_before_zero_copy":true,"write_before_failure_code":"WRITE_BEFORE_PERMISSION_DENIED"}"#,
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["export_job_status"], "COPYING");
        assert_eq!(body["recovered_disk_count"], 1);
        assert!(body.get("status").is_none());
        assert_eq!(
            control.calls.lock().unwrap().as_slice(),
            &["recover_export_job"]
        );
    }

    #[tokio::test]
    async fn copy_progress_ws_requires_upgrade_handshake() {
        let control = Arc::new(FakeControl::default());
        let router = app(test_state(control));

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/ws/edge/copy-progress")
                    .header("host", "localhost")
                    .header("connection", "upgrade")
                    .header("upgrade", "websocket")
                    .header("sec-websocket-version", "13")
                    .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
    }

    #[tokio::test]
    async fn copy_progress_snapshot_has_protocol_shape() {
        let control = FakeControl::default();
        let event = control
            .copy_progress_snapshot()
            .await
            .unwrap()
            .expect("fake progress event");
        let value = serde_json::to_value(event).unwrap();

        assert_eq!(value["event_type"], "COPY_STARTED");
        assert_eq!(value["source"], "edge");
        assert!(value.get("export_job_id").is_none());
        assert!(value.get("export_job_status").is_none());
        assert!(value.get("disk_status_code").is_none());
        assert_eq!(value["edge_name"], "edge-a");
        assert!(value["object_inventory"].is_object());
        assert!(value["export_job"].is_object());
        assert_eq!(value["global"], value["global_progress"]);
        assert!(value["disks"].is_array());
        assert_eq!(value["disk_runtime"], value["disks"]);
        assert_eq!(value["disks"][0]["disk_status_code"], "EDGE_COPYING");
        assert!(value["disks"][0]["progress"].is_object());
        assert!(value["disks"][0]["progress"]["percent"].is_number());
        assert_eq!(
            value["disks"][0]["export_job_id"],
            control.export_job_id.to_string()
        );
        assert!(value["disks"][0].get("current_file").is_some());
        assert!(value["disks"][0].get("current_file_size").is_some());
        assert!(value["disks"][0].get("current_file_done").is_some());
        assert!(value["disks"][0].get("device_path").is_some());
        assert!(value["disks"][0].get("filesystem_type").is_some());
        assert!(value["disks"][0].get("task_pool_eligible").is_some());
        assert!(value.get("status").is_none());
    }

    #[tokio::test]
    async fn websocket_event_uses_summary_object_inventory_instead_of_default_zero() {
        let control = FakeControl::default();
        let event = edge_ws_v2_copy_progress_event(
            control
                .copy_progress_snapshot()
                .await
                .unwrap()
                .expect("fake progress event"),
        );

        assert_eq!(event.protocol_version, "edge-ws-v2");
        assert_eq!(event.event_type, "COPY_PROGRESS");
        assert_eq!(event.stage.as_deref(), Some("COPYING"));
        assert_eq!(event.object_inventory.total_count, 0);
        assert_eq!(
            control.calls.lock().unwrap().as_slice(),
            &["copy_progress_snapshot"]
        );
    }

    #[test]
    fn scan_progress_event_has_protocol_shape_without_naked_status() {
        let event = scan_progress_event(
            "edge-a",
            ScanProgressSnapshot {
                event_type: "SCAN_STARTED",
                event_time: Utc::now(),
                source: "edge",
                scan_phase: "SCANNING",
                bucket_total: 2,
                bucket_done: 0,
                object_seen: 0,
                stable_object_count: 0,
                source_changed_count: 0,
                total_bytes: 0,
                current_bucket: None,
                current_object_key: None,
                last_error_code: None,
                message: Some("scan started".to_string()),
            },
        );
        let value = serde_json::to_value(event).unwrap();

        assert_eq!(value["protocol_version"], "edge-ws-v2");
        assert!(value["event_id"].is_string());
        assert_eq!(value["event_type"], "COPY_PROGRESS");
        assert_eq!(value["stage"], "SCANNING_RUSTFS");
        assert_eq!(value["source"], "edge");
        assert_eq!(value["edge_code"], "edge-a");
        assert_eq!(value["scan"]["scan_status"], "SCANNING");
        assert!(value.get("export_job_id").is_none());
        assert!(value.get("export_job_status").is_none());
        assert!(value.get("disk_status_code").is_none());
        assert!(value["disks"].as_array().unwrap().is_empty());
        assert!(value.get("status").is_none());
    }

    #[test]
    fn copy_progress_aggregator_reports_copy_done_and_seal_done() {
        let progress = crate::progress::ProgressAggregator::new(
            "edge-a",
            "22222222-2222-2222-2222-222222222222",
        );
        progress.register_disk(
            "11111111-1111-1111-1111-111111111111",
            "presence-a",
            "SN-A",
            "/mnt/rustfs-transfer/disk-a",
            100,
            99,
            1,
            80,
        );
        progress.start_object(
            "11111111-1111-1111-1111-111111111111",
            "bucket-a",
            "alpha.bin",
            "data/alpha.bin",
            99,
        );
        progress.complete_object("11111111-1111-1111-1111-111111111111");
        let copy_done = progress.snapshot("COPY_PROGRESS", "copy done");
        assert_eq!(copy_done.event_type, "COPY_DONE");
        assert_eq!(
            copy_done.export_job.as_ref().unwrap().export_job_status,
            "COPYING"
        );

        progress.mark_disk_done("11111111-1111-1111-1111-111111111111");
        let seal_done = progress.snapshot("COPY_PROGRESS", "seal done");
        assert_eq!(seal_done.event_type, "SEAL_DONE");
        assert_eq!(seal_done.disks[0].disk_status_code, "SEALED");
        assert_eq!(
            seal_done.export_job.as_ref().unwrap().export_job_status,
            "SEALED"
        );
    }

    fn test_state(control: Arc<dyn EdgeControlService>) -> AppState {
        test_state_with_rescan(
            control,
            DiskRescanCoordinator::new(Arc::new(NoopRescanRunner)),
        )
    }

    fn test_state_with_rescan(
        control: Arc<dyn EdgeControlService>,
        disk_rescan: DiskRescanCoordinator,
    ) -> AppState {
        let raw = r#"
            [server]
            bind = "127.0.0.1:8081"
            control_api_token = "local-control-token"

            [database]
            url = "postgres://edge:edge@localhost/edge"

            [center]
            base_url = "http://center.local:8080"
            edge_code = "edge-a"
            auth_key_id = "auth-key"
            edge_auth_secret = "edge-secret"

            [rustfs]
            endpoint = "http://127.0.0.1:9000"
            access_key_id = "edge-access-key"
            secret_access_key = "edge-secret-key"

            [rescan]
            token = "local-rescan-token"
        "#;
        let config = EdgeConfig::from_toml(raw).unwrap();
        let fake = Arc::new(FakeHealth);
        let adapters = AdapterBundle {
            database: fake.clone(),
            object_store: fake.clone(),
            disk: fake.clone(),
            clock: fake.clone(),
            ids: fake,
            pg_pool: None,
            s3_client: None,
        };
        AppState {
            config: Arc::new(config),
            adapters,
            disk_rescan,
            control,
            realtime: EdgeRealtimeHub::new("edge-a"),
        }
    }

    fn authenticated_json(method: Method, uri: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .header("X-Edge-Control-Token", "local-control-token")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    fn authenticated_empty(method: Method, uri: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("X-Edge-Control-Token", "local-control-token")
            .body(Body::empty())
            .unwrap()
    }

    async fn json_body(response: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn export_job_response(export_job_id: Uuid) -> ExportJobResponse {
        ExportJobResponse {
            export_job_id,
            edge_code: "edge-a".to_string(),
            export_job_status: "PENDING".to_string(),
            object_count: 2,
            copied_count: 0,
            total_bytes: 99,
            copied_bytes: 0,
            start_time: None,
            finish_time: None,
            error_message: None,
            object_status_counts: BTreeMap::from([("PENDING".to_string(), 2)]),
            disks: vec![ExportJobDiskSummary {
                disk_id: Some(Uuid::new_v4()),
                disk_sn: Some("SN-A".to_string()),
                device_path: Some("/dev/sdb1".to_string()),
                mount_path: Some("/mnt/rustfs-transfer/disk-a".to_string()),
                disk_status_code: Some("IMPORTED".to_string()),
                runtime_status: Some("READY".to_string()),
                object_total: 2,
                object_done: 0,
                total_bytes: 99,
                done_bytes: 0,
                last_error_code: None,
                error_message: None,
            }],
            events: vec![ExportJobEvent {
                event_type: "EXPORT_JOB_STARTED".to_string(),
                event_time: Some(Utc::now()),
                export_job_status: Some("PENDING".to_string()),
                object_status: None,
                disk_id: None,
                bucket: None,
                key: None,
                error_code: None,
                message: None,
            }],
        }
    }
}
