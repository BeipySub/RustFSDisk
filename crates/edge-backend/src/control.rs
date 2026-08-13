use crate::{
    config::EdgeConfig,
    export_planner::{
        assign_export_objects, create_export_plan_from_stable_snapshots, AssignedExportObject,
    },
    export_runtime::{ExportWorkerLauncher, ProductionExportWorkerLauncher},
    progress::{
        CopyProgressEvent, DashboardExportJobSnapshot, ObjectInventorySnapshot,
        ProgressAggregator as CopyProgressAggregator,
    },
    scanner::{
        AwsS3RustFsReadClient, ObjectScanner, PgObjectSnapshotRepository, ProgressAggregator,
        ScanOptions, ScanProgressSnapshot, ScanReport,
    },
};
use anyhow::{anyhow, Context};
use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::{
    collections::BTreeMap, future::Future, path::Path, pin::Pin, process::Command, sync::Arc,
};
use uuid::Uuid;

pub type ControlFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ControlError>> + Send + 'a>>;

const DEFAULT_MAX_BUDGET_SQL: &str =
    "SELECT MAX(object_budget_bytes) FROM disk_runtime WHERE status = 'READY'";
const READY_DISKS_SQL: &str = r#"
    SELECT disk_id, sn, mount_path, free_bytes, object_budget_bytes
    FROM disk_runtime
    WHERE status = 'READY'
      AND disk_id IS NOT NULL
      AND mount_path IS NOT NULL
      AND object_budget_bytes > 0
    ORDER BY id ASC
    "#;
const LOAD_DISK_RUNTIME_SQL: &str = r#"
    WITH latest AS (
        SELECT DISTINCT ON (
            COALESCE(disk_id::text, device_path || '|' || COALESCE(mount_path, ''))
        )
            id,
            sn,
            disk_id,
            device_path,
            mount_path,
            status,
            capacity_bytes,
            free_bytes,
            object_budget_bytes,
            last_error_code,
            error_message
        FROM disk_runtime
        ORDER BY
            COALESCE(disk_id::text, device_path || '|' || COALESCE(mount_path, '')),
            id DESC
    )
    SELECT sn, disk_id, device_path, mount_path, status, capacity_bytes, free_bytes, object_budget_bytes,
           last_error_code, error_message
    FROM latest
    WHERE status <> 'REMOVED'
    ORDER BY id ASC
    "#;
const DAILY_SCAN_WINDOW_HOURS: i64 = 24;

pub trait EdgeControlService: Send + Sync {
    fn scan_once<'a>(
        &'a self,
        request: ScanTriggerRequest,
    ) -> ControlFuture<'a, ScanTriggerResponse>;

    fn create_export_job<'a>(
        &'a self,
        request: CreateExportJobRequest,
    ) -> ControlFuture<'a, ExportJobResponse>;

    fn start_export_job<'a>(
        &'a self,
        export_job_id: Uuid,
        request: StartExportJobRequest,
    ) -> ControlFuture<'a, StartExportJobResponse>;

    fn recover_export_job<'a>(
        &'a self,
        export_job_id: Uuid,
        request: RecoverExportJobRequest,
    ) -> ControlFuture<'a, RecoverExportJobResponse>;

    fn export_job<'a>(&'a self, export_job_id: Uuid) -> ControlFuture<'a, ExportJobResponse>;

    fn export_jobs<'a>(
        &'a self,
        request: ExportJobRecordsRequest,
    ) -> ControlFuture<'a, ExportJobRecordsResponse>;

    fn summary<'a>(&'a self) -> ControlFuture<'a, EdgeControlSummary>;

    fn scan_progress_snapshot<'a>(&'a self) -> ControlFuture<'a, ScanProgressSnapshot>;

    fn copy_progress_snapshot<'a>(&'a self) -> ControlFuture<'a, Option<CopyProgressEvent>>;
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScanTriggerRequest {
    #[serde(default)]
    pub export_job_id: Option<Uuid>,
    #[serde(default)]
    pub enqueue_stable_objects: bool,
    #[serde(default = "default_record_source_changed_objects")]
    pub record_source_changed_objects: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ScanTriggerResponse {
    pub scan_event_type: String,
    pub scan_status: String,
    pub bucket_count: u64,
    pub object_seen: u64,
    pub stable_object_count: u64,
    pub source_changed_count: u64,
    pub total_bytes: u64,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateExportJobRequest {
    #[serde(default)]
    pub export_job_id: Option<Uuid>,
    #[serde(default = "default_true")]
    pub run_scan: bool,
    #[serde(default)]
    pub max_single_disk_object_budget_bytes: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartExportJobRequest {
    #[serde(default = "default_candidate_limit")]
    pub candidate_limit: i64,
    #[serde(default = "default_batch_size")]
    pub batch_size: i64,
}

impl Default for ScanTriggerRequest {
    fn default() -> Self {
        Self {
            export_job_id: None,
            enqueue_stable_objects: false,
            record_source_changed_objects: default_record_source_changed_objects(),
        }
    }
}

impl Default for CreateExportJobRequest {
    fn default() -> Self {
        Self {
            export_job_id: None,
            run_scan: true,
            max_single_disk_object_budget_bytes: None,
        }
    }
}

impl Default for StartExportJobRequest {
    fn default() -> Self {
        Self {
            candidate_limit: default_candidate_limit(),
            batch_size: default_batch_size(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecoverExportJobRequest {
    pub recovery_reason: String,
    #[serde(default)]
    pub admin_confirm_write_before_zero_copy: bool,
    #[serde(default)]
    pub write_before_failure_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StartExportJobResponse {
    pub export_job_id: Uuid,
    pub export_job_status: String,
    pub assigned_object_count: u64,
    pub assigned_bytes: u64,
    pub disk_count: u64,
    pub worker_started_count: u64,
    pub worker_failed_count: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RecoverExportJobResponse {
    pub export_job_id: Uuid,
    pub export_job_status: String,
    pub recovered_disk_count: u64,
    pub worker_started_count: u64,
    pub worker_failed_count: u64,
    pub recovery_reason: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExportJobResponse {
    pub export_job_id: Uuid,
    pub edge_code: String,
    pub export_job_status: String,
    pub object_count: u64,
    pub copied_count: u64,
    pub total_bytes: u64,
    pub copied_bytes: u64,
    pub start_time: Option<DateTime<Utc>>,
    pub finish_time: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub object_status_counts: BTreeMap<String, u64>,
    pub disks: Vec<ExportJobDiskSummary>,
    pub events: Vec<ExportJobEvent>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExportJobDiskSummary {
    pub disk_id: Option<Uuid>,
    pub disk_sn: Option<String>,
    pub device_path: Option<String>,
    pub mount_path: Option<String>,
    pub disk_status_code: Option<String>,
    pub runtime_status: Option<String>,
    pub object_total: u64,
    pub object_done: u64,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub last_error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExportJobEvent {
    pub event_type: String,
    pub event_time: Option<DateTime<Utc>>,
    pub export_job_status: Option<String>,
    pub object_status: Option<String>,
    pub disk_id: Option<Uuid>,
    pub bucket: Option<String>,
    pub key: Option<String>,
    pub error_code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportJobRecordsRequest {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    #[serde(default)]
    pub export_job_status: Option<String>,
    #[serde(default)]
    pub started_from: Option<DateTime<Utc>>,
    #[serde(default)]
    pub started_to: Option<DateTime<Utc>>,
    #[serde(default)]
    pub q: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExportJobRecordsResponse {
    pub page: u32,
    pub page_size: u32,
    pub total_count: u64,
    pub records: Vec<ExportJobRecord>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExportJobRecord {
    pub export_job_id: Uuid,
    pub edge_code: String,
    pub export_job_status: String,
    pub object_count: u64,
    pub copied_count: u64,
    pub total_bytes: u64,
    pub copied_bytes: u64,
    pub start_time: Option<DateTime<Utc>>,
    pub finish_time: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DiskRuntimeSummary {
    pub hardware_serial: String,
    pub disk_sn: String,
    pub stable_hardware_id: String,
    pub disk_id: Option<Uuid>,
    pub device_path: String,
    pub mount_path: Option<String>,
    pub filesystem_type: Option<String>,
    pub filesystem: Option<String>,
    pub fs_uuid: Option<String>,
    pub filesystem_uuid: Option<String>,
    pub disk_status_code: String,
    pub runtime_status: String,
    pub task_pool_eligible: bool,
    pub capacity_bytes: u64,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub free_bytes: u64,
    pub object_budget_bytes: u64,
    pub export_job_id: Option<String>,
    pub seal_id: Option<String>,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
    pub progress: EdgeDiskProgressSummary,
    pub current_object: Option<DashboardCurrentObject>,
    pub current_file: Option<String>,
    pub current_file_size: u64,
    pub current_file_done: u64,
    pub last_error_code: Option<String>,
    pub error_message: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EdgeDiskProgressSummary {
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DashboardCurrentObject {
    pub bucket: String,
    pub key: String,
    pub display_name: String,
    pub relative_data_path: String,
    pub size_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_status: String,
}

impl EdgeDiskProgressSummary {
    fn idle() -> Self {
        Self {
            total_bytes: 0,
            done_bytes: 0,
            remaining_bytes: 0,
            speed_bytes_per_sec: 0,
            object_total: 0,
            object_done: 0,
            object_remaining: 0,
            percent: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeControlSummary {
    pub source: &'static str,
    pub edge_code: String,
    pub edge_name: String,
    pub object_inventory: ObjectInventorySnapshot,
    pub export_job: Option<DashboardExportJobSnapshot>,
    pub global: EdgeGlobalSummary,
    pub global_progress: EdgeGlobalSummary,
    pub disk_runtime: Vec<DiskRuntimeSummary>,
    pub disks: Vec<DiskRuntimeSummary>,
    pub ws_connected: bool,
    pub last_http_refresh_at: DateTime<Utc>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EdgeGlobalSummary {
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
    pub percent: f64,
}

#[derive(Debug)]
pub struct ControlError {
    pub http_status: axum::http::StatusCode,
    pub error_code: &'static str,
    pub message: String,
}

impl ControlError {
    pub fn bad_request(error_code: &'static str, message: impl Into<String>) -> Self {
        Self {
            http_status: axum::http::StatusCode::BAD_REQUEST,
            error_code,
            message: message.into(),
        }
    }

    pub fn conflict(error_code: &'static str, message: impl Into<String>) -> Self {
        Self {
            http_status: axum::http::StatusCode::CONFLICT,
            error_code,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            http_status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            error_code: "INTERNAL_ERROR",
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for ControlError {
    fn from(value: anyhow::Error) -> Self {
        Self::internal(value.to_string())
    }
}

#[derive(Clone)]
pub struct ProductionEdgeControlService {
    config: Arc<EdgeConfig>,
    pool: PgPool,
    s3_client: aws_sdk_s3::Client,
    scan_progress: ProgressAggregator,
    copy_progress: Arc<tokio::sync::RwLock<Option<CopyProgressAggregator>>>,
    worker_launcher: Arc<dyn ExportWorkerLauncher>,
}

impl ProductionEdgeControlService {
    pub fn new(config: Arc<EdgeConfig>, pool: PgPool, s3_client: aws_sdk_s3::Client) -> Self {
        let worker_launcher = Arc::new(ProductionExportWorkerLauncher::new(
            config.clone(),
            pool.clone(),
            s3_client.clone(),
        ));
        Self::new_with_worker_launcher(config, pool, s3_client, worker_launcher)
    }

    pub fn new_with_worker_launcher(
        config: Arc<EdgeConfig>,
        pool: PgPool,
        s3_client: aws_sdk_s3::Client,
        worker_launcher: Arc<dyn ExportWorkerLauncher>,
    ) -> Self {
        Self {
            config,
            pool,
            s3_client,
            scan_progress: ProgressAggregator::default(),
            copy_progress: Arc::new(tokio::sync::RwLock::new(None)),
            worker_launcher,
        }
    }

    async fn run_scan(&self, request: ScanTriggerRequest) -> Result<ScanReport, ControlError> {
        if request.enqueue_stable_objects && request.export_job_id.is_none() {
            return Err(ControlError::bad_request(
                "INVALID_REQUEST",
                "export_job_id is required when enqueue_stable_objects=true",
            ));
        }

        if let Some(report) = load_recent_successful_scan(&self.pool).await? {
            self.scan_progress.reuse_recent_scan(&report);
            return Ok(report);
        }

        let scan_run_id = Uuid::new_v4();
        begin_scan_run(&self.pool, scan_run_id).await?;

        let scanner = ObjectScanner::new(
            AwsS3RustFsReadClient::new(self.s3_client.clone()),
            PgObjectSnapshotRepository::new(self.pool.clone()),
            self.scan_progress.clone(),
        );

        let scan_result = scanner
            .scan_all_buckets(ScanOptions {
                export_job_id: request.export_job_id,
                enqueue_stable_objects: request.enqueue_stable_objects,
                record_source_changed_objects: request.record_source_changed_objects,
            })
            .await;

        match scan_result {
            Ok(report) => {
                finish_scan_run(&self.pool, scan_run_id, &report).await?;
                Ok(report)
            }
            Err(err) => {
                self.scan_progress.fail_scan("SCAN_FAILED", err.to_string());
                fail_scan_run(&self.pool, scan_run_id, "SCAN_FAILED", &err.to_string()).await?;
                Err(ControlError::internal(err.to_string()))
            }
        }
    }

    async fn default_max_budget(&self) -> Result<u64, ControlError> {
        let value: Option<i64> = sqlx::query_scalar(DEFAULT_MAX_BUDGET_SQL)
            .fetch_one(&self.pool)
            .await
            .context("query READY disk capacity budget")?;
        let budget = value.unwrap_or_default().max(0) as u64;
        if budget == 0 {
            return Err(ControlError::conflict(
                "INSUFFICIENT_SPACE",
                "no READY ext4 transport disk with object_budget_bytes is available",
            ));
        }
        Ok(budget)
    }

    async fn ready_disks(&self) -> Result<Vec<ReadyDisk>, ControlError> {
        let rows = sqlx::query(READY_DISKS_SQL)
            .fetch_all(&self.pool)
            .await
            .context("query READY transport disks")?;

        let disks = rows
            .into_iter()
            .map(|row| ReadyDisk {
                disk_id: row.get("disk_id"),
                disk_sn: row.get("sn"),
                mount_path: row.get("mount_path"),
                free_bytes: row.get::<i64, _>("free_bytes").max(0) as u64,
                object_budget_bytes: row.get::<i64, _>("object_budget_bytes").max(0) as u64,
            })
            .collect::<Vec<_>>();
        if disks.is_empty() {
            return Err(ControlError::conflict(
                "INSUFFICIENT_SPACE",
                "no READY ext4 transport disk is eligible for export assignment",
            ));
        }
        Ok(disks)
    }
}

impl EdgeControlService for ProductionEdgeControlService {
    fn scan_once<'a>(
        &'a self,
        request: ScanTriggerRequest,
    ) -> ControlFuture<'a, ScanTriggerResponse> {
        Box::pin(async move {
            let report = self.run_scan(request).await?;
            Ok(ScanTriggerResponse {
                scan_event_type: "SCAN_DONE".to_string(),
                scan_status: "DONE".to_string(),
                bucket_count: report.bucket_count,
                object_seen: report.object_seen,
                stable_object_count: report.stable_object_count,
                source_changed_count: report.source_changed_count,
                total_bytes: report.total_bytes,
                message: if report.reused_recent_scan {
                    "RustFS scan reused: last successful scan is within 24 hours".to_string()
                } else {
                    "RustFS scan completed".to_string()
                },
            })
        })
    }

    fn create_export_job<'a>(
        &'a self,
        request: CreateExportJobRequest,
    ) -> ControlFuture<'a, ExportJobResponse> {
        Box::pin(async move {
            let export_job_id = request.export_job_id.unwrap_or_else(Uuid::new_v4);
            if request.run_scan {
                self.run_scan(ScanTriggerRequest {
                    export_job_id: Some(export_job_id),
                    enqueue_stable_objects: false,
                    record_source_changed_objects: true,
                })
                .await?;
            }
            let max_budget = match request.max_single_disk_object_budget_bytes {
                Some(value) if value > 0 => value,
                _ => self.default_max_budget().await?,
            };

            create_export_plan_from_stable_snapshots(
                &self.pool,
                export_job_id,
                &self.config.center.edge_code,
                max_budget,
            )
            .await
            .context("create export plan from stable snapshots")?;

            self.export_job(export_job_id).await
        })
    }

    fn start_export_job<'a>(
        &'a self,
        export_job_id: Uuid,
        request: StartExportJobRequest,
    ) -> ControlFuture<'a, StartExportJobResponse> {
        Box::pin(async move {
            if request.candidate_limit <= 0 || request.batch_size <= 0 {
                return Err(ControlError::bad_request(
                    "INVALID_REQUEST",
                    "candidate_limit and batch_size must be positive",
                ));
            }

            sqlx::query(
                "UPDATE export_job SET status = 'COPYING' WHERE export_job_id = $1 AND status IN ('PENDING', 'SCANNING', 'COPYING')",
            )
            .bind(export_job_id)
            .execute(&self.pool)
            .await
            .context("mark export job COPYING")?;

            let mut assigned = Vec::new();
            let mut assigned_disk_ids = Vec::new();
            for disk in self.ready_disks().await? {
                let batch = assign_export_objects(
                    &self.pool,
                    export_job_id,
                    disk.disk_id,
                    disk.object_budget_bytes,
                    request.candidate_limit,
                    request.batch_size,
                )
                .await
                .with_context(|| format!("assign export objects to disk {}", disk.disk_id))?;
                if !batch.is_empty() {
                    assigned_disk_ids.push(disk.disk_id);
                }
                assigned_copy_progress_disk(
                    &mut assigned,
                    &disk,
                    &batch,
                    &self.config.center.edge_code,
                    export_job_id,
                    self.copy_progress.clone(),
                )
                .await;
                assigned.extend(batch);
            }
            let worker_report = self
                .worker_launcher
                .launch_workers(export_job_id, assigned_disk_ids)
                .await
                .context("launch DiskWorker instances for assigned READY disks")?;

            Ok(StartExportJobResponse {
                export_job_id,
                export_job_status: "COPYING".to_string(),
                assigned_object_count: assigned.len() as u64,
                assigned_bytes: assigned_bytes(&assigned),
                disk_count: worker_report.disk_count,
                worker_started_count: worker_report.worker_started_count,
                worker_failed_count: worker_report.worker_failed_count,
                message: "export objects assigned and DiskWorker instances launched through the controlled start API".to_string(),
            })
        })
    }

    fn recover_export_job<'a>(
        &'a self,
        export_job_id: Uuid,
        request: RecoverExportJobRequest,
    ) -> ControlFuture<'a, RecoverExportJobResponse> {
        Box::pin(async move {
            let reason = request.recovery_reason.trim();
            if reason.is_empty() {
                return Err(ControlError::bad_request(
                    "INVALID_REQUEST",
                    "recovery_reason is required for export recovery audit",
                ));
            }

            let disk_ids = validate_export_recovery(&self.pool, export_job_id, &request).await?;
            let recovery_note = recovery_audit_note(&request);
            let result = sqlx::query(
                r#"
                UPDATE export_job
                SET status = 'COPYING',
                    finish_time = NULL,
                    error_message = CONCAT_WS(E'\n', error_message, $2)
                WHERE export_job_id = $1
                  AND status = 'FAILED'
                "#,
            )
            .bind(export_job_id)
            .bind(&recovery_note)
            .execute(&self.pool)
            .await
            .context("mark failed export job as COPYING for controlled recovery")?;
            if result.rows_affected() != 1 {
                return Err(ControlError::conflict(
                    "EXPORT_JOB_NOT_FAILED",
                    "export_job_status changed before recovery could be started",
                ));
            }

            let worker_report = self
                .worker_launcher
                .launch_workers(export_job_id, disk_ids.clone())
                .await
                .context("launch DiskWorker instances for controlled export recovery")?;

            Ok(RecoverExportJobResponse {
                export_job_id,
                export_job_status: "COPYING".to_string(),
                recovered_disk_count: disk_ids.len() as u64,
                worker_started_count: worker_report.worker_started_count,
                worker_failed_count: worker_report.worker_failed_count,
                recovery_reason: reason.to_string(),
                message: "failed export job recovery accepted; original assignments were reused without creating a new job".to_string(),
            })
        })
    }

    fn export_job<'a>(&'a self, export_job_id: Uuid) -> ControlFuture<'a, ExportJobResponse> {
        Box::pin(async move { load_export_job(&self.pool, export_job_id).await })
    }

    fn export_jobs<'a>(
        &'a self,
        request: ExportJobRecordsRequest,
    ) -> ControlFuture<'a, ExportJobRecordsResponse> {
        Box::pin(async move { load_export_jobs(&self.pool, request).await })
    }

    fn summary<'a>(&'a self) -> ControlFuture<'a, EdgeControlSummary> {
        Box::pin(async move {
            let latest_export_job_id: Option<Uuid> =
                sqlx::query_scalar("SELECT export_job_id FROM export_job ORDER BY id DESC LIMIT 1")
                    .fetch_optional(&self.pool)
                    .await
                    .context("query latest export job")?
                    .flatten();
            let latest_export_job = match latest_export_job_id {
                Some(export_job_id) => Some(load_export_job(&self.pool, export_job_id).await?),
                None => None,
            };
            let object_inventory = load_object_inventory(&self.pool).await?;
            let copy_progress =
                self.copy_progress.read().await.as_ref().map(|progress| {
                    progress.snapshot("COPY_PROGRESS", "edge copy progress snapshot")
                });
            let mut disks = load_disk_runtime(&self.pool).await?;
            enrich_disks_from_copy_progress(&mut disks, copy_progress.as_ref());
            let global_progress = copy_progress
                .as_ref()
                .map(global_from_copy_progress)
                .unwrap_or_else(|| global_from_latest_job(latest_export_job.as_ref()));
            let export_job = latest_export_job
                .as_ref()
                .map(|job| export_job_snapshot(job, &global_progress));

            Ok(EdgeControlSummary {
                source: "edge",
                edge_code: self.config.center.edge_code.clone(),
                edge_name: self.config.center.edge_code.clone(),
                object_inventory,
                export_job,
                global: global_progress.clone(),
                global_progress,
                disk_runtime: disks.clone(),
                disks,
                ws_connected: false,
                last_http_refresh_at: Utc::now(),
                message: "edge controlled HTTP API summary".to_string(),
            })
        })
    }

    fn copy_progress_snapshot<'a>(&'a self) -> ControlFuture<'a, Option<CopyProgressEvent>> {
        Box::pin(async move {
            Ok(self
                .copy_progress
                .read()
                .await
                .as_ref()
                .map(|progress| progress.snapshot("COPY_PROGRESS", "edge copy progress snapshot")))
        })
    }

    fn scan_progress_snapshot<'a>(&'a self) -> ControlFuture<'a, ScanProgressSnapshot> {
        Box::pin(async move { Ok(self.scan_progress.snapshot()) })
    }
}

#[derive(Debug)]
struct ReadyDisk {
    disk_id: Uuid,
    disk_sn: String,
    mount_path: Option<String>,
    free_bytes: u64,
    object_budget_bytes: u64,
}

async fn load_export_job(
    pool: &PgPool,
    export_job_id: Uuid,
) -> Result<ExportJobResponse, ControlError> {
    let row = sqlx::query(
        r#"
        SELECT export_job_id, edge_code, status, object_count, copied_count, total_bytes,
               copied_bytes, start_time, finish_time, error_message
        FROM export_job
        WHERE export_job_id = $1
        "#,
    )
    .bind(export_job_id)
    .fetch_optional(pool)
    .await
    .context("load export job")?
    .ok_or_else(|| ControlError::bad_request("INVALID_REQUEST", "export_job_id was not found"))?;

    let object_status_counts = load_object_status_counts(pool, export_job_id).await?;
    let disks = load_export_job_disks(pool, export_job_id).await?;
    let events = load_export_job_events(
        pool,
        export_job_id,
        row.get("status"),
        naive_utc(row.get("start_time")),
        naive_utc(row.get("finish_time")),
        row.get("error_message"),
    )
    .await?;

    Ok(ExportJobResponse {
        export_job_id: row.get("export_job_id"),
        edge_code: row.get("edge_code"),
        export_job_status: row.get("status"),
        object_count: row.get::<i64, _>("object_count").max(0) as u64,
        copied_count: row.get::<i64, _>("copied_count").max(0) as u64,
        total_bytes: row.get::<i64, _>("total_bytes").max(0) as u64,
        copied_bytes: row.get::<i64, _>("copied_bytes").max(0) as u64,
        start_time: naive_utc(row.get("start_time")),
        finish_time: naive_utc(row.get("finish_time")),
        error_message: row.get("error_message"),
        object_status_counts,
        disks,
        events,
    })
}

async fn load_recent_successful_scan(pool: &PgPool) -> Result<Option<ScanReport>, ControlError> {
    let cutoff = Utc::now() - ChronoDuration::hours(DAILY_SCAN_WINDOW_HOURS);
    let row = sqlx::query(
        r#"
        SELECT bucket_count, object_seen, stable_object_count, source_changed_count, total_bytes
        FROM edge_scan_run
        WHERE scan_status = 'DONE'
          AND scan_finished_at IS NOT NULL
          AND scan_finished_at >= $1
        ORDER BY scan_finished_at DESC
        LIMIT 1
        "#,
    )
    .bind(cutoff.naive_utc())
    .fetch_optional(pool)
    .await
    .context("query recent successful RustFS scan")?;

    Ok(row.map(|row| ScanReport {
        bucket_count: row.get::<i64, _>("bucket_count").max(0) as u64,
        object_seen: row.get::<i64, _>("object_seen").max(0) as u64,
        stable_object_count: row.get::<i64, _>("stable_object_count").max(0) as u64,
        source_changed_count: row.get::<i64, _>("source_changed_count").max(0) as u64,
        total_bytes: row.get::<i64, _>("total_bytes").max(0) as u64,
        reused_recent_scan: true,
    }))
}

async fn begin_scan_run(pool: &PgPool, scan_run_id: Uuid) -> Result<(), ControlError> {
    sqlx::query(
        r#"
        INSERT INTO edge_scan_run(scan_run_id, scan_status, scan_started_at)
        VALUES ($1, 'SCANNING', NOW() AT TIME ZONE 'UTC')
        "#,
    )
    .bind(scan_run_id)
    .execute(pool)
    .await
    .context("insert RustFS scan run")?;

    Ok(())
}

async fn finish_scan_run(
    pool: &PgPool,
    scan_run_id: Uuid,
    report: &ScanReport,
) -> Result<(), ControlError> {
    sqlx::query(
        r#"
        UPDATE edge_scan_run
        SET scan_status = 'DONE',
            scan_finished_at = NOW() AT TIME ZONE 'UTC',
            bucket_count = $2,
            object_seen = $3,
            stable_object_count = $4,
            source_changed_count = $5,
            total_bytes = $6
        WHERE scan_run_id = $1
        "#,
    )
    .bind(scan_run_id)
    .bind(saturating_i64(report.bucket_count))
    .bind(saturating_i64(report.object_seen))
    .bind(saturating_i64(report.stable_object_count))
    .bind(saturating_i64(report.source_changed_count))
    .bind(saturating_i64(report.total_bytes))
    .execute(pool)
    .await
    .context("mark RustFS scan run DONE")?;

    Ok(())
}

async fn fail_scan_run(
    pool: &PgPool,
    scan_run_id: Uuid,
    error_code: &str,
    error_message: &str,
) -> Result<(), ControlError> {
    sqlx::query(
        r#"
        UPDATE edge_scan_run
        SET scan_status = 'FAILED',
            scan_finished_at = NOW() AT TIME ZONE 'UTC',
            error_code = $2,
            error_message = $3
        WHERE scan_run_id = $1
        "#,
    )
    .bind(scan_run_id)
    .bind(error_code)
    .bind(error_message)
    .execute(pool)
    .await
    .context("mark RustFS scan run FAILED")?;

    Ok(())
}

fn saturating_i64(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

async fn load_export_jobs(
    pool: &PgPool,
    request: ExportJobRecordsRequest,
) -> Result<ExportJobRecordsResponse, ControlError> {
    let page = request.page.max(1);
    let page_size = request.page_size.clamp(1, 100);
    let offset = ((page - 1) * page_size) as i64;
    let status = request
        .export_job_status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if status
        .as_deref()
        .is_some_and(|value| !is_valid_export_job_status(value))
    {
        return Err(ControlError::bad_request(
            "INVALID_REQUEST",
            "export_job_status filter is not valid",
        ));
    }
    let q = request
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"));

    let total_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM export_job
        WHERE ($1::text IS NULL OR status = $1)
          AND ($2::timestamp IS NULL OR start_time >= $2)
          AND ($3::timestamp IS NULL OR start_time <= $3)
          AND ($4::text IS NULL OR export_job_id::text ILIKE $4 OR edge_code ILIKE $4)
        "#,
    )
    .bind(status.as_deref())
    .bind(request.started_from.map(|value| value.naive_utc()))
    .bind(request.started_to.map(|value| value.naive_utc()))
    .bind(q.as_deref())
    .fetch_one(pool)
    .await
    .context("count export job records")?;

    let rows = sqlx::query(
        r#"
        SELECT export_job_id, edge_code, status, object_count, copied_count,
               total_bytes, copied_bytes, start_time, finish_time, error_message
        FROM export_job
        WHERE ($1::text IS NULL OR status = $1)
          AND ($2::timestamp IS NULL OR start_time >= $2)
          AND ($3::timestamp IS NULL OR start_time <= $3)
          AND ($4::text IS NULL OR export_job_id::text ILIKE $4 OR edge_code ILIKE $4)
        ORDER BY id DESC
        LIMIT $5 OFFSET $6
        "#,
    )
    .bind(status.as_deref())
    .bind(request.started_from.map(|value| value.naive_utc()))
    .bind(request.started_to.map(|value| value.naive_utc()))
    .bind(q.as_deref())
    .bind(page_size as i64)
    .bind(offset)
    .fetch_all(pool)
    .await
    .context("load export job records")?;

    Ok(ExportJobRecordsResponse {
        page,
        page_size,
        total_count: total_count.max(0) as u64,
        records: rows
            .into_iter()
            .map(|row| ExportJobRecord {
                export_job_id: row.get("export_job_id"),
                edge_code: row.get("edge_code"),
                export_job_status: row.get("status"),
                object_count: row.get::<i64, _>("object_count").max(0) as u64,
                copied_count: row.get::<i64, _>("copied_count").max(0) as u64,
                total_bytes: row.get::<i64, _>("total_bytes").max(0) as u64,
                copied_bytes: row.get::<i64, _>("copied_bytes").max(0) as u64,
                start_time: naive_utc(row.get("start_time")),
                finish_time: naive_utc(row.get("finish_time")),
                error_message: row.get("error_message"),
            })
            .collect(),
    })
}

async fn load_object_status_counts(
    pool: &PgPool,
    export_job_id: Uuid,
) -> Result<BTreeMap<String, u64>, ControlError> {
    let rows = sqlx::query(
        "SELECT status, COUNT(*) AS count FROM export_object WHERE export_job_id = $1 GROUP BY status",
    )
    .bind(export_job_id)
    .fetch_all(pool)
    .await
    .context("load export object status counts")?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("status"),
                row.get::<i64, _>("count").max(0) as u64,
            )
        })
        .collect())
}

async fn load_export_job_disks(
    pool: &PgPool,
    export_job_id: Uuid,
) -> Result<Vec<ExportJobDiskSummary>, ControlError> {
    let rows = sqlx::query(
        r#"
        SELECT
            eo.disk_id,
            dr.sn,
            dr.device_path,
            dr.mount_path,
            dr.status AS runtime_status,
            dr.last_error_code,
            dr.error_message,
            COUNT(*) AS object_total,
            COUNT(*) FILTER (WHERE eo.status = 'EXPORTED') AS object_done,
            COALESCE(SUM(eo.chunk_size_bytes), 0)::bigint AS total_bytes,
            COALESCE(SUM(CASE WHEN eo.status = 'EXPORTED' THEN eo.chunk_size_bytes ELSE 0 END), 0)::bigint AS done_bytes
        FROM export_object eo
        LEFT JOIN disk_runtime dr ON dr.disk_id = eo.disk_id
        WHERE eo.export_job_id = $1
        GROUP BY eo.disk_id, dr.sn, dr.device_path, dr.mount_path, dr.status,
                 dr.last_error_code, dr.error_message
        ORDER BY eo.disk_id NULLS LAST
        "#,
    )
    .bind(export_job_id)
    .fetch_all(pool)
    .await
    .context("load export job disk summaries")?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let mount_path: Option<String> = row.get("mount_path");
            ExportJobDiskSummary {
                disk_id: row.get("disk_id"),
                disk_sn: row.get("sn"),
                device_path: row.get("device_path"),
                disk_status_code: disk_status_code_from_mount(mount_path.as_deref()),
                mount_path,
                runtime_status: row.get("runtime_status"),
                object_total: row.get::<i64, _>("object_total").max(0) as u64,
                object_done: row.get::<i64, _>("object_done").max(0) as u64,
                total_bytes: row.get::<i64, _>("total_bytes").max(0) as u64,
                done_bytes: row.get::<i64, _>("done_bytes").max(0) as u64,
                last_error_code: row.get("last_error_code"),
                error_message: row.get("error_message"),
            }
        })
        .collect())
}

async fn load_export_job_events(
    pool: &PgPool,
    export_job_id: Uuid,
    export_job_status: String,
    start_time: Option<DateTime<Utc>>,
    finish_time: Option<DateTime<Utc>>,
    error_message: Option<String>,
) -> Result<Vec<ExportJobEvent>, ControlError> {
    let mut events = Vec::new();
    if start_time.is_some() {
        events.push(ExportJobEvent {
            event_type: "EXPORT_JOB_STARTED".to_string(),
            event_time: start_time,
            export_job_status: Some(export_job_status.clone()),
            object_status: None,
            disk_id: None,
            bucket: None,
            key: None,
            error_code: None,
            message: None,
        });
    }
    if finish_time.is_some() {
        events.push(ExportJobEvent {
            event_type: if export_job_status == "FAILED" {
                "EXPORT_JOB_FAILED".to_string()
            } else {
                "EXPORT_JOB_FINISHED".to_string()
            },
            event_time: finish_time,
            export_job_status: Some(export_job_status.clone()),
            object_status: None,
            disk_id: None,
            bucket: None,
            key: None,
            error_code: None,
            message: error_message.clone(),
        });
    }

    let rows = sqlx::query(
        r#"
        SELECT disk_id, bucket, object_key, status, error_code, error_message
        FROM export_object
        WHERE export_job_id = $1
          AND (status IN ('COPYING', 'FAILED', 'SOURCE_CHANGED', 'SKIPPED')
               OR error_code IS NOT NULL)
        ORDER BY id DESC
        LIMIT 50
        "#,
    )
    .bind(export_job_id)
    .fetch_all(pool)
    .await
    .context("load export job object events")?;

    events.extend(rows.into_iter().map(|row| {
        let object_status: String = row.get("status");
        ExportJobEvent {
            event_type: match object_status.as_str() {
                "COPYING" => "EXPORT_OBJECT_COPYING",
                "FAILED" => "EXPORT_OBJECT_FAILED",
                "SOURCE_CHANGED" => "EXPORT_OBJECT_SOURCE_CHANGED",
                "SKIPPED" => "EXPORT_OBJECT_SKIPPED",
                _ => "EXPORT_OBJECT_EVENT",
            }
            .to_string(),
            event_time: None,
            export_job_status: None,
            object_status: Some(object_status),
            disk_id: row.get("disk_id"),
            bucket: row.get("bucket"),
            key: row.get("object_key"),
            error_code: row.get("error_code"),
            message: row.get("error_message"),
        }
    }));

    Ok(events)
}

async fn load_disk_runtime(pool: &PgPool) -> Result<Vec<DiskRuntimeSummary>, ControlError> {
    let rows = sqlx::query(LOAD_DISK_RUNTIME_SQL)
        .fetch_all(pool)
        .await
        .context("load disk runtime summary")?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let disk_sn: String = row.get("sn");
            let device_path: String = row.get("device_path");
            let mount_path: Option<String> = row.get("mount_path");
            let runtime_status: String = row.get("status");
            let metadata = disk_runtime_filesystem_metadata(&device_path, mount_path.as_deref());
            let filesystem_type = metadata.filesystem_type;
            let fs_uuid = metadata.fs_uuid;
            let disk_status_code = disk_status_code_from_mount(mount_path.as_deref())
                .unwrap_or_else(|| disk_status_code_from_runtime(&runtime_status).to_string());
            let capacity_bytes = row.get::<i64, _>("capacity_bytes").max(0) as u64;
            let free_bytes = row.get::<i64, _>("free_bytes").max(0) as u64;
            let object_budget_bytes = row.get::<i64, _>("object_budget_bytes").max(0) as u64;
            let progress = EdgeDiskProgressSummary::idle();
            let last_error_code: Option<String> = row.get("last_error_code");
            let error_message: Option<String> = row.get("error_message");
            let message = disk_runtime_message(
                &runtime_status,
                last_error_code.as_deref(),
                error_message.as_deref(),
            );
            let task_pool_eligible = runtime_status == "READY" && disk_status_code == "INITIALIZED";
            DiskRuntimeSummary {
                hardware_serial: disk_sn.clone(),
                disk_sn: disk_sn.clone(),
                stable_hardware_id: fs_uuid.clone().unwrap_or_else(|| disk_sn.clone()),
                disk_id: row.get("disk_id"),
                device_path,
                mount_path: mount_path.clone(),
                filesystem_type: filesystem_type.clone(),
                filesystem: filesystem_type,
                fs_uuid: fs_uuid.clone(),
                filesystem_uuid: fs_uuid,
                disk_status_code,
                runtime_status,
                task_pool_eligible,
                capacity_bytes,
                total_bytes: progress.total_bytes,
                done_bytes: progress.done_bytes,
                remaining_bytes: progress.remaining_bytes,
                free_bytes,
                object_budget_bytes,
                export_job_id: None,
                seal_id: None,
                speed_bytes_per_sec: progress.speed_bytes_per_sec,
                object_total: progress.object_total,
                object_done: progress.object_done,
                object_remaining: progress.object_remaining,
                progress,
                current_object: None,
                current_file: None,
                current_file_size: 0,
                current_file_done: 0,
                last_error_code,
                error_message,
                message,
            }
        })
        .collect())
}

async fn assigned_copy_progress_disk(
    assigned: &mut [AssignedExportObject],
    disk: &ReadyDisk,
    batch: &[AssignedExportObject],
    edge_code: &str,
    export_job_id: Uuid,
    hub: Arc<tokio::sync::RwLock<Option<CopyProgressAggregator>>>,
) {
    if batch.is_empty() && assigned.is_empty() {
        return;
    }

    let mut guard = hub.write().await;
    if guard.is_none() {
        *guard = Some(CopyProgressAggregator::new(
            edge_code.to_string(),
            export_job_id.to_string(),
        ));
    }
    if let Some(progress) = guard.as_ref() {
        progress.register_disk(
            disk.disk_id.to_string(),
            disk.disk_sn.clone(),
            disk.mount_path.clone().unwrap_or_default(),
            batch
                .iter()
                .map(|object| object.chunk_size_bytes.max(0) as u64)
                .sum(),
            batch.len() as u64,
            disk.free_bytes,
        );
    }
}

fn global_from_latest_job(job: Option<&ExportJobResponse>) -> EdgeGlobalSummary {
    let Some(job) = job else {
        return EdgeGlobalSummary {
            total_bytes: 0,
            done_bytes: 0,
            remaining_bytes: 0,
            speed_bytes_per_sec: 0,
            object_total: 0,
            object_done: 0,
            object_remaining: 0,
            percent: 0.0,
        };
    };

    EdgeGlobalSummary {
        total_bytes: job.total_bytes,
        done_bytes: job.copied_bytes,
        remaining_bytes: job.total_bytes.saturating_sub(job.copied_bytes),
        speed_bytes_per_sec: 0,
        object_total: job.object_count,
        object_done: job.copied_count,
        object_remaining: job.object_count.saturating_sub(job.copied_count),
        percent: percent(job.copied_bytes, job.total_bytes),
    }
}

fn global_from_copy_progress(event: &CopyProgressEvent) -> EdgeGlobalSummary {
    EdgeGlobalSummary {
        total_bytes: event.global_progress.total_bytes,
        done_bytes: event.global_progress.done_bytes,
        remaining_bytes: event.global_progress.remaining_bytes,
        speed_bytes_per_sec: event.global_progress.speed_bytes_per_sec,
        object_total: event.global_progress.object_total,
        object_done: event.global_progress.object_done,
        object_remaining: event.global_progress.object_remaining,
        percent: event.global_progress.percent,
    }
}

async fn load_object_inventory(pool: &PgPool) -> Result<ObjectInventorySnapshot, ControlError> {
    let local = sqlx::query(
        r#"
        SELECT COUNT(*)::bigint AS total_count,
               COALESCE(SUM(size_bytes), 0)::bigint AS total_bytes
        FROM local_object_snapshot
        "#,
    )
    .fetch_one(pool)
    .await
    .context("load local object inventory")?;
    let exported = sqlx::query(
        r#"
        SELECT COUNT(*)::bigint AS exported_count,
               COALESCE(SUM(chunk_size_bytes), 0)::bigint AS exported_bytes
        FROM export_object
        WHERE status = 'EXPORTED'
        "#,
    )
    .fetch_one(pool)
    .await
    .context("load exported object inventory")?;

    Ok(ObjectInventorySnapshot {
        total_bytes: local.get::<i64, _>("total_bytes").max(0) as u64,
        exported_bytes: exported.get::<i64, _>("exported_bytes").max(0) as u64,
        total_count: local.get::<i64, _>("total_count").max(0) as u64,
        exported_count: exported.get::<i64, _>("exported_count").max(0) as u64,
    })
}

fn export_job_snapshot(
    job: &ExportJobResponse,
    progress: &EdgeGlobalSummary,
) -> DashboardExportJobSnapshot {
    DashboardExportJobSnapshot {
        export_job_id: job.export_job_id.to_string(),
        export_job_status: job.export_job_status.clone(),
        start_time: job.start_time,
        finish_time: job.finish_time,
        total_bytes: progress.total_bytes,
        done_bytes: progress.done_bytes,
        remaining_bytes: progress.remaining_bytes,
        speed_bytes_per_sec: progress.speed_bytes_per_sec,
        object_total: progress.object_total,
        object_done: progress.object_done,
        object_remaining: progress.object_remaining,
        percent: progress.percent,
    }
}

fn enrich_disks_from_copy_progress(
    disks: &mut [DiskRuntimeSummary],
    event: Option<&CopyProgressEvent>,
) {
    let Some(event) = event else {
        return;
    };
    for disk in disks {
        let disk_id = disk.disk_id.map(|id| id.to_string());
        let Some(progress) = event
            .disks
            .iter()
            .find(|progress| Some(progress.disk_id.as_str()) == disk_id.as_deref())
        else {
            continue;
        };
        disk.runtime_status = progress.runtime_status.clone();
        disk.progress = EdgeDiskProgressSummary {
            total_bytes: progress.total_bytes,
            done_bytes: progress.done_bytes,
            remaining_bytes: progress.remaining_bytes,
            speed_bytes_per_sec: progress.speed_bytes_per_sec,
            object_total: progress.object_total,
            object_done: progress.object_done,
            object_remaining: progress.object_remaining,
            percent: progress.progress.percent,
        };
        disk.total_bytes = disk.progress.total_bytes;
        disk.done_bytes = disk.progress.done_bytes;
        disk.remaining_bytes = disk.progress.remaining_bytes;
        disk.speed_bytes_per_sec = disk.progress.speed_bytes_per_sec;
        disk.object_total = disk.progress.object_total;
        disk.object_done = disk.progress.object_done;
        disk.object_remaining = disk.progress.object_remaining;
        disk.disk_status_code = progress.disk_status_code.clone();
        disk.export_job_id = progress.export_job_id.clone();
        disk.seal_id = progress.seal_id.clone();
        disk.task_pool_eligible = false;
        disk.message = progress.message.clone();
        disk.last_error_code = progress
            .last_error_code
            .clone()
            .or(disk.last_error_code.clone());
        disk.error_message = progress
            .error_message
            .clone()
            .or(disk.error_message.clone());
        disk.current_object =
            progress
                .current_object
                .as_ref()
                .map(|current| DashboardCurrentObject {
                    bucket: current.bucket.clone(),
                    key: current.key.clone(),
                    display_name: current.display_name.clone(),
                    relative_data_path: current.relative_data_path.clone(),
                    size_bytes: current.size_bytes,
                    done_bytes: current.done_bytes,
                    remaining_bytes: current.remaining_bytes,
                    speed_bytes_per_sec: current.speed_bytes_per_sec,
                    object_status: current.object_status.clone(),
                });
        disk.current_file = progress.current_file.clone();
        disk.current_file_size = progress.current_file_size;
        disk.current_file_done = progress.current_file_done;
    }
}

struct DiskRuntimeFilesystemMetadata {
    filesystem_type: Option<String>,
    fs_uuid: Option<String>,
}

fn disk_runtime_filesystem_metadata(
    device_path: &str,
    mount_path: Option<&str>,
) -> DiskRuntimeFilesystemMetadata {
    DiskRuntimeFilesystemMetadata {
        filesystem_type: mount_path
            .and_then(filesystem_type_from_mount)
            .or_else(|| blkid_value(device_path, "TYPE")),
        fs_uuid: blkid_value(device_path, "UUID"),
    }
}

fn filesystem_type_from_mount(mount_path: &str) -> Option<String> {
    command_stdout("findmnt", &["-nro", "FSTYPE", "--target", mount_path])
}

fn blkid_value(device_path: &str, field: &str) -> Option<String> {
    if device_path.trim().is_empty() || device_path == "unknown" || !Path::new(device_path).exists()
    {
        return None;
    }
    command_stdout("blkid", &["-o", "value", "-s", field, device_path])
}

fn command_stdout(command: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(command).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout)
        .ok()?
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?
        .to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn disk_status_code_from_mount(mount_path: Option<&str>) -> Option<String> {
    let root = std::path::Path::new(mount_path?).join("rustfs-transfer");
    let bytes = std::fs::read(root.join("disk_info.json")).ok()?;
    let value = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    value
        .get("status")
        .and_then(|status| status.get("code"))
        .and_then(|code| code.as_str())
        .map(str::to_string)
}

fn disk_status_code_from_runtime(runtime_status: &str) -> &'static str {
    match runtime_status {
        "COPYING" => "EDGE_COPYING",
        "REJECTED" => "UNREGISTERED",
        "ERROR" | "REMOVED" => "ERROR",
        _ => "INITIALIZED",
    }
}

fn disk_runtime_message(
    runtime_status: &str,
    last_error_code: Option<&str>,
    error_message: Option<&str>,
) -> String {
    match (last_error_code, error_message) {
        (Some(code), Some(message)) => format!("{code}: {message}"),
        (Some(code), None) => code.to_string(),
        (None, Some(message)) => message.to_string(),
        (None, None) => format!("disk runtime_status={runtime_status}"),
    }
}

fn assigned_bytes(objects: &[AssignedExportObject]) -> u64 {
    objects
        .iter()
        .map(|object| object.chunk_size_bytes.max(0) as u64)
        .sum()
}

fn percent(done_bytes: u64, total_bytes: u64) -> f64 {
    if total_bytes == 0 {
        0.0
    } else {
        (done_bytes as f64 / total_bytes as f64) * 100.0
    }
}

async fn validate_export_recovery(
    pool: &PgPool,
    export_job_id: Uuid,
    request: &RecoverExportJobRequest,
) -> Result<Vec<Uuid>, ControlError> {
    let job = load_export_recovery_job(pool, export_job_id).await?;
    ensure_recovery_job_guard(&job)?;

    let object_guard = load_recovery_object_guard(pool, export_job_id).await?;
    ensure_recovery_object_guard(&object_guard)?;
    if !is_recoverable_write_before_failure(job.error_message.as_deref(), request) {
        return Err(ControlError::conflict(
            "EXPORT_FAILURE_NOT_RECOVERABLE",
            "failed export job is not classified as a write-before recoverable failure",
        ));
    }

    let disk_ids = load_assigned_disk_ids(pool, export_job_id).await?;
    validate_recovery_disks(pool, &disk_ids).await?;
    Ok(disk_ids)
}

fn ensure_recovery_job_guard(job: &RecoveryJobGuard) -> Result<(), ControlError> {
    if job.export_job_status != "FAILED" {
        return Err(ControlError::conflict(
            "EXPORT_JOB_NOT_FAILED",
            format!(
                "export_job_status must be FAILED for recovery, got {}",
                job.export_job_status
            ),
        ));
    }
    if job.copied_count != 0 || job.copied_bytes != 0 {
        return Err(ControlError::conflict(
            "EXPORT_RECOVERY_NOT_CLEAN",
            "export job already has copied_count or copied_bytes; use manual recovery, not write-before retry",
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct RecoveryJobGuard {
    export_job_status: String,
    copied_count: u64,
    copied_bytes: u64,
    error_message: Option<String>,
}

async fn load_export_recovery_job(
    pool: &PgPool,
    export_job_id: Uuid,
) -> Result<RecoveryJobGuard, ControlError> {
    let row = sqlx::query(
        r#"
        SELECT status, copied_count, copied_bytes, error_message
        FROM export_job
        WHERE export_job_id = $1
        "#,
    )
    .bind(export_job_id)
    .fetch_optional(pool)
    .await
    .context("load export job for recovery")?
    .ok_or_else(|| ControlError::bad_request("INVALID_REQUEST", "export_job_id was not found"))?;

    Ok(RecoveryJobGuard {
        export_job_status: row.get("status"),
        copied_count: row.get::<i64, _>("copied_count").max(0) as u64,
        copied_bytes: row.get::<i64, _>("copied_bytes").max(0) as u64,
        error_message: row.get("error_message"),
    })
}

fn ensure_recovery_object_guard(object_guard: &RecoveryObjectGuard) -> Result<(), ControlError> {
    if object_guard.total_count == 0 {
        return Err(ControlError::conflict(
            "EXPORT_RECOVERY_NO_OBJECTS",
            "failed export job has no assigned objects to recover",
        ));
    }
    if object_guard.assigned_count != object_guard.total_count {
        return Err(ControlError::conflict(
            "EXPORT_OBJECT_STATE_NOT_RECOVERABLE",
            "all export objects must remain ASSIGNED before controlled recovery",
        ));
    }
    if object_guard.null_disk_count != 0 {
        return Err(ControlError::conflict(
            "EXPORT_OBJECT_DISK_MISSING",
            "all assigned export objects must remain bound to their original disk_id",
        ));
    }
    if object_guard.dirty_count != 0 {
        return Err(ControlError::conflict(
            "EXPORT_OBJECT_ALREADY_WRITTEN",
            "one or more export objects already has hash, nonce, partial, data, or metadata fields",
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct RecoveryObjectGuard {
    total_count: i64,
    assigned_count: i64,
    null_disk_count: i64,
    dirty_count: i64,
}

async fn load_recovery_object_guard(
    pool: &PgPool,
    export_job_id: Uuid,
) -> Result<RecoveryObjectGuard, ControlError> {
    let row = sqlx::query(
        r#"
        SELECT
          COUNT(*) AS total_count,
          COUNT(*) FILTER (WHERE status = 'ASSIGNED') AS assigned_count,
          COUNT(*) FILTER (WHERE disk_id IS NULL) AS null_disk_count,
          COUNT(*) FILTER (
            WHERE plaintext_sha256 IS NOT NULL
               OR ciphertext_sha256 IS NOT NULL
               OR ciphertext_size_bytes IS NOT NULL
               OR encryption_alg IS NOT NULL
               OR data_key_id IS NOT NULL
               OR nonce IS NOT NULL
               OR tag IS NOT NULL
               OR aad IS NOT NULL
               OR chunk_sha256 IS NOT NULL
               OR partial_path IS NOT NULL
               OR relative_data_path IS NOT NULL
               OR relative_meta_path IS NOT NULL
          ) AS dirty_count
        FROM export_object
        WHERE export_job_id = $1
        "#,
    )
    .bind(export_job_id)
    .fetch_one(pool)
    .await
    .context("load export object recovery guard")?;

    Ok(RecoveryObjectGuard {
        total_count: row.get("total_count"),
        assigned_count: row.get("assigned_count"),
        null_disk_count: row.get("null_disk_count"),
        dirty_count: row.get("dirty_count"),
    })
}

async fn load_assigned_disk_ids(
    pool: &PgPool,
    export_job_id: Uuid,
) -> Result<Vec<Uuid>, ControlError> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT disk_id
        FROM export_object
        WHERE export_job_id = $1
          AND status = 'ASSIGNED'
          AND disk_id IS NOT NULL
        ORDER BY disk_id
        "#,
    )
    .bind(export_job_id)
    .fetch_all(pool)
    .await
    .context("load assigned recovery disk ids")?;

    Ok(rows.into_iter().map(|row| row.get("disk_id")).collect())
}

async fn validate_recovery_disks(pool: &PgPool, disk_ids: &[Uuid]) -> Result<(), ControlError> {
    if disk_ids.is_empty() {
        return Err(ControlError::conflict(
            "EXPORT_RECOVERY_NO_DISKS",
            "failed export job has no original disk assignments",
        ));
    }

    let rows = sqlx::query(
        r#"
        SELECT DISTINCT ON (disk_id) disk_id, mount_path, status, partial_residue_count,
               partial_residue_bytes, last_error_code
        FROM disk_runtime
        WHERE disk_id = ANY($1)
        ORDER BY disk_id, id DESC
        "#,
    )
    .bind(disk_ids)
    .fetch_all(pool)
    .await
    .context("load latest disk runtime records for recovery")?;
    if rows.len() != disk_ids.len() {
        return Err(ControlError::conflict(
            "EXPORT_RECOVERY_DISK_MISSING",
            "one or more originally assigned disks is not currently detected",
        ));
    }

    for row in rows {
        let disk_id: Uuid = row.get("disk_id");
        let runtime_status: String = row.get("status");
        let mount_path: Option<String> = row.get("mount_path");
        let partial_count: i32 = row.get("partial_residue_count");
        let partial_bytes: i64 = row.get("partial_residue_bytes");
        let last_error_code: Option<String> = row.get("last_error_code");
        if runtime_status != "READY" {
            return Err(ControlError::conflict(
                "EXPORT_RECOVERY_DISK_NOT_READY",
                format!(
                    "disk {disk_id} runtime_status must be READY for recovery, got {runtime_status}"
                ),
            ));
        }
        if partial_count != 0 || partial_bytes != 0 {
            return Err(ControlError::conflict(
                "PARTIAL_FILE_FOUND",
                format!("disk {disk_id} has recorded partial residue and cannot be recovered"),
            ));
        }
        if matches!(
            last_error_code.as_deref(),
            Some("RECOVERY_REQUIRED" | "PARTIAL_FILE_FOUND" | "PARTIAL_CLEAN_FAILED")
        ) {
            return Err(ControlError::conflict(
                "EXPORT_RECOVERY_DISK_DIRTY",
                format!("disk {disk_id} still records a recovery-required error"),
            ));
        }
        let mount_path = mount_path.ok_or_else(|| {
            ControlError::conflict(
                "EXPORT_RECOVERY_DISK_MISSING",
                format!("disk {disk_id} is READY but has no mount_path"),
            )
        })?;
        validate_recovery_protocol_root(disk_id, std::path::Path::new(&mount_path))?;
    }

    Ok(())
}

fn validate_recovery_protocol_root(
    disk_id: Uuid,
    mount_path: &std::path::Path,
) -> Result<(), ControlError> {
    let root = mount_path.join("rustfs-transfer");
    let disk_info_path = root.join("disk_info.json");
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&disk_info_path).map_err(|err| {
            ControlError::conflict(
                "DISK_INFO_INVALID",
                format!("failed to read disk_info.json for disk {disk_id}: {err}"),
            )
        })?)
        .map_err(|err| {
            ControlError::conflict(
                "DISK_INFO_INVALID",
                format!("failed to parse disk_info.json for disk {disk_id}: {err}"),
            )
        })?;
    let status_code = value
        .pointer("/status/code")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if status_code != "INITIALIZED" {
        return Err(ControlError::conflict(
            "DISK_STATUS_NOT_INITIALIZED",
            format!(
                "disk {disk_id} status_code must be INITIALIZED for recovery, got {status_code}"
            ),
        ));
    }
    let disk_info_disk_id = value
        .pointer("/disk/disk_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    if disk_info_disk_id != Some(disk_id) {
        return Err(ControlError::conflict(
            "DISK_ID_MISMATCH",
            format!("disk_info.json disk_id does not match assigned disk {disk_id}"),
        ));
    }
    ensure_no_recovery_residue(&root)?;
    Ok(())
}

fn ensure_no_recovery_residue(root: &std::path::Path) -> Result<(), ControlError> {
    if contains_partial_file(root).map_err(|err| {
        ControlError::conflict(
            "PARTIAL_SCAN_FAILED",
            format!("failed to scan protocol root for partial files: {err}"),
        )
    })? {
        return Err(ControlError::conflict(
            "PARTIAL_FILE_FOUND",
            "protocol root contains .partial residue",
        ));
    }
    ensure_dir_has_no_files(&root.join("manifests"), "MANIFEST_RESIDUE_FOUND")?;
    ensure_dir_has_no_files(&root.join("data"), "EXPORT_DATA_RESIDUE_FOUND")?;
    ensure_dir_has_no_files(&root.join("meta"), "EXPORT_META_RESIDUE_FOUND")?;
    Ok(())
}

fn ensure_dir_has_no_files(
    path: &std::path::Path,
    error_code: &'static str,
) -> Result<(), ControlError> {
    if !path.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path).map_err(|err| {
        ControlError::conflict(
            error_code,
            format!("failed to scan {}: {err}", path.display()),
        )
    })? {
        let entry = entry.map_err(|err| {
            ControlError::conflict(
                error_code,
                format!("failed to scan {}: {err}", path.display()),
            )
        })?;
        let child = entry.path();
        if child.is_dir() {
            ensure_dir_has_no_files(&child, error_code)?;
        } else {
            return Err(ControlError::conflict(
                error_code,
                format!("{} contains recovery residue", path.display()),
            ));
        }
    }
    Ok(())
}

fn contains_partial_file(path: &std::path::Path) -> std::io::Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            if contains_partial_file(&child)? {
                return Ok(true);
            }
        } else if child
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".partial"))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_recoverable_write_before_failure(
    error_message: Option<&str>,
    request: &RecoverExportJobRequest,
) -> bool {
    let Some(message) = error_message else {
        return false;
    };

    if has_non_recoverable_failure_marker(message) || has_legacy_non_recoverable_text(message) {
        return false;
    }

    if persisted_write_before_marker(message) {
        return true;
    }

    request.admin_confirm_write_before_zero_copy
        && request
            .write_before_failure_code
            .as_deref()
            .is_some_and(is_recoverable_write_before_code)
}

fn persisted_write_before_marker(message: &str) -> bool {
    message.lines().any(|line| {
        line.contains("export_failure_stage=WRITE_BEFORE")
            && extract_marker_value(line, "export_failure_code")
                .is_some_and(is_recoverable_write_before_code)
    })
}

fn has_non_recoverable_failure_marker(message: &str) -> bool {
    message.lines().any(|line| {
        extract_marker_value(line, "export_failure_code").is_some_and(|code| {
            matches!(
                code,
                "SOURCE_CHANGED"
                    | "CHECKSUM_MISMATCH"
                    | "DECRYPT_FAILED"
                    | "PARTIAL_CLEAN_FAILED"
                    | "DISK_FULL"
                    | "DISK_REMOVED"
            )
        }) || extract_marker_value(line, "export_failure_stage")
            .is_some_and(|stage| matches!(stage, "OBJECT_WRITE" | "SEAL" | "PARTIAL_RECOVERY"))
    })
}

fn has_legacy_non_recoverable_text(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    [
        "source_changed",
        "source changed",
        "checksum",
        "decrypt",
        "partial",
        "disk full",
        "disk removed",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn extract_marker_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    line.split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix(&prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn is_recoverable_write_before_code(code: &str) -> bool {
    matches!(
        code.trim(),
        "WRITE_BEFORE_PERMISSION_DENIED"
            | "PROTOCOL_ROOT_PERMISSION_DENIED"
            | "MANIFEST_PREWRITE_PERMISSION_DENIED"
    )
}

fn recovery_audit_note(request: &RecoverExportJobRequest) -> String {
    let code = request
        .write_before_failure_code
        .as_deref()
        .unwrap_or("UNSPECIFIED")
        .replace(['\n', ';'], " ");
    format!(
        "export_recovery_requested_at={}; admin_confirm_write_before_zero_copy={}; write_before_failure_code={}; recovery_reason={}",
        Utc::now().to_rfc3339(),
        request.admin_confirm_write_before_zero_copy,
        code,
        request.recovery_reason.trim().replace(['\n', ';'], " ")
    )
}

fn naive_utc(value: Option<NaiveDateTime>) -> Option<DateTime<Utc>> {
    value.map(|value| DateTime::from_naive_utc_and_offset(value, Utc))
}

fn is_valid_export_job_status(value: &str) -> bool {
    matches!(
        value,
        "PENDING" | "SCANNING" | "COPYING" | "SEALED" | "FAILED" | "CANCELLED"
    )
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

fn default_record_source_changed_objects() -> bool {
    true
}

fn default_true() -> bool {
    true
}

fn default_candidate_limit() -> i64 {
    500
}

fn default_batch_size() -> i64 {
    100
}

impl<T> EdgeControlService for Arc<T>
where
    T: EdgeControlService + ?Sized,
{
    fn scan_once<'a>(
        &'a self,
        request: ScanTriggerRequest,
    ) -> ControlFuture<'a, ScanTriggerResponse> {
        (**self).scan_once(request)
    }

    fn create_export_job<'a>(
        &'a self,
        request: CreateExportJobRequest,
    ) -> ControlFuture<'a, ExportJobResponse> {
        (**self).create_export_job(request)
    }

    fn start_export_job<'a>(
        &'a self,
        export_job_id: Uuid,
        request: StartExportJobRequest,
    ) -> ControlFuture<'a, StartExportJobResponse> {
        (**self).start_export_job(export_job_id, request)
    }

    fn recover_export_job<'a>(
        &'a self,
        export_job_id: Uuid,
        request: RecoverExportJobRequest,
    ) -> ControlFuture<'a, RecoverExportJobResponse> {
        (**self).recover_export_job(export_job_id, request)
    }

    fn export_job<'a>(&'a self, export_job_id: Uuid) -> ControlFuture<'a, ExportJobResponse> {
        (**self).export_job(export_job_id)
    }

    fn export_jobs<'a>(
        &'a self,
        request: ExportJobRecordsRequest,
    ) -> ControlFuture<'a, ExportJobRecordsResponse> {
        (**self).export_jobs(request)
    }

    fn summary<'a>(&'a self) -> ControlFuture<'a, EdgeControlSummary> {
        (**self).summary()
    }

    fn scan_progress_snapshot<'a>(&'a self) -> ControlFuture<'a, ScanProgressSnapshot> {
        (**self).scan_progress_snapshot()
    }

    fn copy_progress_snapshot<'a>(&'a self) -> ControlFuture<'a, Option<CopyProgressEvent>> {
        (**self).copy_progress_snapshot()
    }
}

pub fn validate_export_job_id(export_job_id: Uuid) -> Result<(), ControlError> {
    if export_job_id == Uuid::nil() {
        return Err(ControlError::bad_request(
            "INVALID_REQUEST",
            "export_job_id must not be nil",
        ));
    }
    Ok(())
}

pub fn missing_control_service() -> ControlError {
    ControlError::internal(anyhow!("edge control service is not configured").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{fs, path::Path};

    #[test]
    fn recovery_job_guard_rejects_non_failed_job() {
        let err = ensure_recovery_job_guard(&RecoveryJobGuard {
            export_job_status: "COPYING".to_string(),
            copied_count: 0,
            copied_bytes: 0,
            error_message: Some("one or more DiskWorker instances failed".to_string()),
        })
        .unwrap_err();

        assert_eq!(err.error_code, "EXPORT_JOB_NOT_FAILED");
    }

    #[test]
    fn recovery_object_guard_rejects_already_written_fields() {
        let err = ensure_recovery_object_guard(&RecoveryObjectGuard {
            total_count: 1,
            assigned_count: 1,
            null_disk_count: 0,
            dirty_count: 1,
        })
        .unwrap_err();

        assert_eq!(err.error_code, "EXPORT_OBJECT_ALREADY_WRITTEN");
    }

    #[test]
    fn rejected_runtime_without_disk_info_uses_unregistered_disk_status_code() {
        assert_eq!(disk_status_code_from_runtime("REJECTED"), "UNREGISTERED");
    }

    #[test]
    fn recovery_protocol_root_accepts_clean_initialized_original_disk() {
        let disk_id = Uuid::new_v4();
        let mount = test_mount("clean", disk_id);
        write_initialized_disk_info(&mount, disk_id);

        validate_recovery_protocol_root(disk_id, &mount).unwrap();

        let _ = fs::remove_dir_all(&mount);
    }

    #[test]
    fn recovery_protocol_root_rejects_data_meta_manifest_and_partial_residue() {
        for (case, relative, expected_error) in [
            (
                "data",
                "rustfs-transfer/data/object.bin",
                "EXPORT_DATA_RESIDUE_FOUND",
            ),
            (
                "meta",
                "rustfs-transfer/meta/object.json",
                "EXPORT_META_RESIDUE_FOUND",
            ),
            (
                "manifest",
                "rustfs-transfer/manifests/export_manifest.json",
                "MANIFEST_RESIDUE_FOUND",
            ),
            (
                "partial",
                "rustfs-transfer/data/object.bin.partial",
                "PARTIAL_FILE_FOUND",
            ),
        ] {
            let disk_id = Uuid::new_v4();
            let mount = test_mount(case, disk_id);
            write_initialized_disk_info(&mount, disk_id);
            let path = mount.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"residue").unwrap();

            let err = validate_recovery_protocol_root(disk_id, &mount).unwrap_err();
            assert_eq!(err.error_code, expected_error);

            let _ = fs::remove_dir_all(&mount);
        }
    }

    #[test]
    fn recovery_failure_classifier_allows_persisted_write_before_marker() {
        let request = recover_request(false, None);

        assert!(is_recoverable_write_before_failure(
            Some(
                "one or more DiskWorker instances failed\nexport_failure_code=WRITE_BEFORE_PERMISSION_DENIED; export_failure_stage=WRITE_BEFORE; worker_error_code=MANIFEST_INVALID"
            ),
            &request
        ));
    }

    #[test]
    fn recovery_failure_classifier_rejects_unknown_generic_worker_failure() {
        let request = recover_request(false, None);

        assert!(!is_recoverable_write_before_failure(
            Some("one or more DiskWorker instances failed; disk was not sealed for failed workers"),
            &request
        ));
    }

    #[test]
    fn recovery_failure_classifier_allows_legacy_generic_only_with_admin_whitelist() {
        let denied = recover_request(false, Some("WRITE_BEFORE_PERMISSION_DENIED"));
        let allowed = recover_request(true, Some("WRITE_BEFORE_PERMISSION_DENIED"));
        let bad_code = recover_request(true, Some("MANIFEST_INVALID"));
        let generic =
            Some("one or more DiskWorker instances failed; disk was not sealed for failed workers");

        assert!(!is_recoverable_write_before_failure(generic, &denied));
        assert!(is_recoverable_write_before_failure(generic, &allowed));
        assert!(!is_recoverable_write_before_failure(generic, &bad_code));
    }

    #[test]
    fn recovery_failure_classifier_rejects_non_recoverable_marker_even_with_admin() {
        let request = recover_request(true, Some("WRITE_BEFORE_PERMISSION_DENIED"));

        assert!(!is_recoverable_write_before_failure(
            Some(
                "export_failure_code=PARTIAL_CLEAN_FAILED; export_failure_stage=PARTIAL_RECOVERY; worker_error_code=PARTIAL_CLEAN_FAILED"
            ),
            &request
        ));
        assert!(!is_recoverable_write_before_failure(
            Some("export_failure_code=SOURCE_CHANGED; export_failure_stage=OBJECT_WRITE"),
            &request
        ));
        assert!(!is_recoverable_write_before_failure(
            Some("one or more DiskWorker instances failed; partial cleanup failed"),
            &request
        ));
    }

    #[test]
    fn ready_queries_use_current_rebuildable_runtime_only() {
        for sql in [DEFAULT_MAX_BUDGET_SQL, READY_DISKS_SQL] {
            assert!(!sql.contains("DISTINCT ON"));
            assert!(!sql.contains("DELETE FROM export_job"));
            assert!(!sql.contains("DELETE FROM export_object"));
        }
        assert!(READY_DISKS_SQL.contains("WHERE status = 'READY'"));
        assert!(READY_DISKS_SQL.contains("ORDER BY id ASC"));
        assert!(LOAD_DISK_RUNTIME_SQL.contains("DISTINCT ON"));
        assert!(LOAD_DISK_RUNTIME_SQL.contains("WHERE status <> 'REMOVED'"));
        assert!(LOAD_DISK_RUNTIME_SQL.contains("ORDER BY id ASC"));
        assert!(!LOAD_DISK_RUNTIME_SQL.contains("DELETE FROM export_job"));
        assert!(!LOAD_DISK_RUNTIME_SQL.contains("DELETE FROM export_object"));
    }

    fn test_mount(case: &str, disk_id: Uuid) -> std::path::PathBuf {
        let path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("test-disks")
            .join(format!("edge-recover-{case}-{disk_id}"));
        let _ = fs::remove_dir_all(&path);
        path
    }

    fn recover_request(
        admin_confirm_write_before_zero_copy: bool,
        write_before_failure_code: Option<&str>,
    ) -> RecoverExportJobRequest {
        RecoverExportJobRequest {
            recovery_reason: "operator confirmed ACL fix after zero-write failure".to_string(),
            admin_confirm_write_before_zero_copy,
            write_before_failure_code: write_before_failure_code.map(str::to_string),
        }
    }

    fn write_initialized_disk_info(mount: &Path, disk_id: Uuid) {
        let root = mount.join("rustfs-transfer");
        for relative in ["data", "meta", "manifests", "logs", "quarantine/partial"] {
            fs::create_dir_all(root.join(relative)).unwrap();
        }
        let disk_info = json!({
            "disk": { "disk_id": disk_id },
            "status": { "code": "INITIALIZED" },
            "security": { "data_key_id": Uuid::new_v4() }
        });
        fs::write(
            root.join("disk_info.json"),
            serde_json::to_vec_pretty(&disk_info).unwrap(),
        )
        .unwrap();
    }
}
