use crate::{
    config::AutoExportConfig,
    control::{
        ControlError, CreateExportJobRequest, EdgeControlService, EdgeControlSummary,
        ExportJobRecordsRequest, ScanTriggerRequest, StartExportJobRequest,
    },
    disk_detection::{BoxFuture, DiskDetectionError},
    rescan::{DiskRescanRunner, DiskRescanSource, DiskRescanTrigger},
};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::sync::Arc;
use tokio::{sync::Mutex, time::Duration};
use uuid::Uuid;

const ACTIVE_EXPORT_JOB_STATUSES: [&str; 4] = ["PENDING", "SCANNING", "COPYING", "SEALING"];
const READY_RECHECK_ATTEMPTS: u8 = 5;
const READY_RECHECK_DELAY: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct AutoExportOrchestrator {
    config: AutoExportConfig,
    control: Arc<dyn EdgeControlService>,
    state: Arc<Mutex<AutoExportState>>,
}

#[derive(Debug, Default)]
struct AutoExportState {
    running: bool,
    last_attempt_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoExportTrigger {
    pub source: AutoExportTriggerSource,
    pub device: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoExportTriggerSource {
    Startup,
    Udev,
    Manual,
    Queued,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoExportDecision {
    pub outcome: AutoExportOutcome,
    pub export_job_id: Option<Uuid>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoExportOutcome {
    Disabled,
    StartOnReadyDisabled,
    Cooldown,
    AlreadyRunning,
    NoReadyDisk,
    ActiveExportJob,
    NoExportableObject,
    Started,
    Failed,
}

impl AutoExportOrchestrator {
    pub fn new(config: AutoExportConfig, control: Arc<dyn EdgeControlService>) -> Self {
        Self {
            config,
            control,
            state: Arc::new(Mutex::new(AutoExportState::default())),
        }
    }

    pub async fn on_transport_disks_refreshed(
        &self,
        trigger: AutoExportTrigger,
    ) -> AutoExportDecision {
        let begin = self.try_begin(&trigger).await;
        if begin.outcome != AutoExportOutcome::Started {
            log_decision(&trigger, &begin);
            return begin;
        }

        let decision = self.run_ready_flow(&trigger).await;
        let mut state = self.state.lock().await;
        state.running = false;
        // A hot-plug notification can arrive before Linux finishes mounting the partition.
        // Do not consume the cooldown until an initialized disk is actually ready.
        if decision.outcome == AutoExportOutcome::NoReadyDisk {
            state.last_attempt_at = None;
        }
        drop(state);

        log_decision(&trigger, &decision);
        decision
    }

    async fn try_begin(&self, trigger: &AutoExportTrigger) -> AutoExportDecision {
        if !self.config.enabled {
            return AutoExportDecision::new(
                AutoExportOutcome::Disabled,
                "auto_export.enabled is false",
            );
        }
        if !self.config.start_on_ready {
            return AutoExportDecision::new(
                AutoExportOutcome::StartOnReadyDisabled,
                "auto_export.start_on_ready is false",
            );
        }

        let now = Utc::now();
        let mut state = self.state.lock().await;
        if state.running {
            return AutoExportDecision::new(
                AutoExportOutcome::AlreadyRunning,
                "auto export orchestration is already running",
            );
        }
        if let Some(last_attempt_at) = state.last_attempt_at {
            let cooldown = ChronoDuration::seconds(self.config.cooldown_seconds as i64);
            if now - last_attempt_at < cooldown {
                return AutoExportDecision::new(
                    AutoExportOutcome::Cooldown,
                    format!(
                        "auto export cooldown is active for {} seconds",
                        self.config.cooldown_seconds
                    ),
                );
            }
        }

        state.running = true;
        state.last_attempt_at = Some(now);
        tracing::info!(
            trigger_source = ?trigger.source,
            device = trigger.device.as_deref(),
            "edge auto export orchestration started"
        );
        AutoExportDecision::new(
            AutoExportOutcome::Started,
            "auto export orchestration accepted",
        )
    }

    async fn run_ready_flow(&self, _trigger: &AutoExportTrigger) -> AutoExportDecision {
        let summary = match self.control.summary().await {
            Ok(summary) => summary,
            Err(error) => return failed("load edge control summary", error),
        };

        let ready_disk_count = ready_initialized_disk_count(&summary);
        if ready_disk_count < self.config.min_ready_disk_count {
            return AutoExportDecision::new(
                AutoExportOutcome::NoReadyDisk,
                format!(
                    "ready initialized disk count {ready_disk_count} is below required {}",
                    self.config.min_ready_disk_count
                ),
            );
        }

        match self.has_active_export_job().await {
            Ok(true) => {
                return AutoExportDecision::new(
                    AutoExportOutcome::ActiveExportJob,
                    "existing active export job blocks auto export",
                );
            }
            Ok(false) => {}
            Err(error) => return failed("check active export jobs", error),
        }

        let scan = match self.control.scan_once(ScanTriggerRequest::default()).await {
            Ok(scan) => scan,
            Err(error) => return failed("scan RustFS before auto export", error),
        };
        if scan.stable_object_count == 0 {
            return AutoExportDecision::new(
                AutoExportOutcome::NoExportableObject,
                "scan completed but no stable completed RustFS object is exportable",
            );
        }

        let export_job = match self
            .control
            .create_export_job(CreateExportJobRequest {
                run_scan: false,
                ..CreateExportJobRequest::default()
            })
            .await
        {
            Ok(export_job) => export_job,
            Err(error) => return failed("create export job after scan", error),
        };
        if export_job.object_count == 0 {
            return AutoExportDecision::new(
                AutoExportOutcome::NoExportableObject,
                "export plan has no object after latest successful scan",
            )
            .with_export_job_id(export_job.export_job_id);
        }

        match self
            .control
            .start_export_job(export_job.export_job_id, StartExportJobRequest::default())
            .await
        {
            Ok(start) => AutoExportDecision::new(
                AutoExportOutcome::Started,
                format!(
                    "auto export job started; assigned_objects={}, worker_started={}",
                    start.assigned_object_count, start.worker_started_count
                ),
            )
            .with_export_job_id(export_job.export_job_id),
            Err(error) => failed("start export job worker", error)
                .with_export_job_id(export_job.export_job_id),
        }
    }

    async fn has_active_export_job(&self) -> Result<bool, ControlError> {
        for export_job_status in ACTIVE_EXPORT_JOB_STATUSES {
            let records = self
                .control
                .export_jobs(ExportJobRecordsRequest {
                    page: 1,
                    page_size: 1,
                    export_job_status: Some(export_job_status.to_owned()),
                    started_from: None,
                    started_to: None,
                    q: None,
                })
                .await?;
            if records.total_count > 0 {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

pub struct AutoExportRescanRunner {
    inner: Arc<dyn DiskRescanRunner>,
    orchestrator: AutoExportOrchestrator,
}

impl AutoExportRescanRunner {
    pub fn new(inner: Arc<dyn DiskRescanRunner>, orchestrator: AutoExportOrchestrator) -> Self {
        Self {
            inner,
            orchestrator,
        }
    }
}

impl DiskRescanRunner for AutoExportRescanRunner {
    fn run_disk_rescan<'a>(
        &'a self,
        trigger: DiskRescanTrigger,
    ) -> BoxFuture<'a, Result<usize, DiskDetectionError>> {
        Box::pin(async move {
            let mut record_count = self.inner.run_disk_rescan(trigger.clone()).await?;
            if trigger.source == DiskRescanSource::ControlRefresh {
                return Ok(record_count);
            }

            let mut decision = self
                .orchestrator
                .on_transport_disks_refreshed(AutoExportTrigger::from(trigger.clone()))
                .await;

            if !matches!(
                trigger.source,
                DiskRescanSource::Startup | DiskRescanSource::Udev
            ) {
                return Ok(record_count);
            }

            for attempt in 1..=READY_RECHECK_ATTEMPTS {
                if decision.outcome != AutoExportOutcome::NoReadyDisk {
                    break;
                }
                tracing::info!(
                    source = ?trigger.source,
                    device = trigger.device.as_deref(),
                    attempt,
                    max_attempts = READY_RECHECK_ATTEMPTS,
                    "transport disk is not ready yet; retrying disk rescan before auto export"
                );
                tokio::time::sleep(READY_RECHECK_DELAY).await;
                record_count = self.inner.run_disk_rescan(trigger.clone()).await?;
                decision = self
                    .orchestrator
                    .on_transport_disks_refreshed(AutoExportTrigger::from(trigger.clone()))
                    .await;
            }
            Ok(record_count)
        })
    }
}

impl From<DiskRescanTrigger> for AutoExportTrigger {
    fn from(value: DiskRescanTrigger) -> Self {
        Self {
            source: match value.source {
                DiskRescanSource::Startup => AutoExportTriggerSource::Startup,
                DiskRescanSource::Udev => AutoExportTriggerSource::Udev,
                DiskRescanSource::Manual => AutoExportTriggerSource::Manual,
                DiskRescanSource::Queued => AutoExportTriggerSource::Queued,
                DiskRescanSource::ControlRefresh => AutoExportTriggerSource::Manual,
            },
            device: value.device,
        }
    }
}

impl AutoExportDecision {
    fn new(outcome: AutoExportOutcome, message: impl Into<String>) -> Self {
        Self {
            outcome,
            export_job_id: None,
            message: message.into(),
        }
    }

    fn with_export_job_id(mut self, export_job_id: Uuid) -> Self {
        self.export_job_id = Some(export_job_id);
        self
    }
}

fn ready_initialized_disk_count(summary: &EdgeControlSummary) -> usize {
    summary
        .disks
        .iter()
        .filter(|disk| {
            disk.runtime_status == "READY"
                && disk.disk_status_code == "INITIALIZED"
                && disk
                    .mount_path
                    .as_deref()
                    .is_some_and(|path| !path.trim().is_empty())
        })
        .count()
}

fn failed(step: &'static str, error: ControlError) -> AutoExportDecision {
    AutoExportDecision::new(
        AutoExportOutcome::Failed,
        format!("{step} failed: {}: {}", error.error_code, error.message),
    )
}

fn log_decision(trigger: &AutoExportTrigger, decision: &AutoExportDecision) {
    let is_failure = matches!(decision.outcome, AutoExportOutcome::Failed);
    if is_failure {
        tracing::warn!(
            trigger_source = ?trigger.source,
            device = trigger.device.as_deref(),
            outcome = ?decision.outcome,
            export_job_id = decision.export_job_id.map(|value| value.to_string()),
            message = decision.message,
            "edge auto export orchestration rejected or failed"
        );
    } else {
        tracing::info!(
            trigger_source = ?trigger.source,
            device = trigger.device.as_deref(),
            outcome = ?decision.outcome,
            export_job_id = decision.export_job_id.map(|value| value.to_string()),
            message = decision.message,
            "edge auto export orchestration decision"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        control::{
            ControlFuture, DashboardCurrentObject, EdgeDiskProgressSummary, EdgeGlobalSummary,
            ExportJobDiskSummary, ExportJobEvent, ExportJobRecord, ExportJobRecordsResponse,
            ExportJobResponse, RecoverExportJobRequest, RecoverExportJobResponse,
            ScanTriggerResponse, StartExportJobResponse,
        },
        disk_detection::DiskDetectionError,
        progress::CopyProgressEvent,
        scanner::ScanProgressSnapshot,
    };
    use axum::http::StatusCode;
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    struct NoopRescanRunner;

    impl DiskRescanRunner for NoopRescanRunner {
        fn run_disk_rescan<'a>(
            &'a self,
            _trigger: DiskRescanTrigger,
        ) -> BoxFuture<'a, Result<usize, DiskDetectionError>> {
            Box::pin(async { Ok(1) })
        }
    }

    #[derive(Clone)]
    struct FakeControl {
        disk_status_code: &'static str,
        runtime_status: &'static str,
        ready_after_summary_calls: Option<usize>,
        active_status: Option<&'static str>,
        stable_object_count: u64,
        export_object_count: u64,
        export_job_id: Uuid,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    impl Default for FakeControl {
        fn default() -> Self {
            Self {
                disk_status_code: "INITIALIZED",
                runtime_status: "READY",
                ready_after_summary_calls: None,
                active_status: None,
                stable_object_count: 1,
                export_object_count: 1,
                export_job_id: Uuid::new_v4(),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl EdgeControlService for FakeControl {
        fn scan_once<'a>(
            &'a self,
            _request: ScanTriggerRequest,
        ) -> ControlFuture<'a, ScanTriggerResponse> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("scan_once");
                Ok(ScanTriggerResponse {
                    scan_event_type: "SCAN_DONE".to_owned(),
                    scan_status: "DONE".to_owned(),
                    bucket_count: 1,
                    object_seen: self.stable_object_count,
                    stable_object_count: self.stable_object_count,
                    source_changed_count: 0,
                    total_bytes: self.stable_object_count * 10,
                    message: "scan done".to_owned(),
                })
            })
        }

        fn create_export_job<'a>(
            &'a self,
            request: CreateExportJobRequest,
        ) -> ControlFuture<'a, ExportJobResponse> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("create_export_job");
                assert!(!request.run_scan);
                Ok(export_job_response(
                    self.export_job_id,
                    "PENDING",
                    self.export_object_count,
                ))
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
                    export_job_status: "COPYING".to_owned(),
                    assigned_object_count: self.export_object_count,
                    assigned_bytes: self.export_object_count * 10,
                    disk_count: 1,
                    worker_started_count: 1,
                    worker_failed_count: 0,
                    message: "started".to_owned(),
                })
            })
        }

        fn recover_export_job<'a>(
            &'a self,
            _export_job_id: Uuid,
            _request: RecoverExportJobRequest,
        ) -> ControlFuture<'a, RecoverExportJobResponse> {
            Box::pin(async { Err(control_error("UNUSED")) })
        }

        fn export_job<'a>(&'a self, _export_job_id: Uuid) -> ControlFuture<'a, ExportJobResponse> {
            Box::pin(async { Err(control_error("UNUSED")) })
        }

        fn export_jobs<'a>(
            &'a self,
            request: ExportJobRecordsRequest,
        ) -> ControlFuture<'a, ExportJobRecordsResponse> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("export_jobs");
                let total_count = if request.export_job_status.as_deref() == self.active_status {
                    1
                } else {
                    0
                };
                Ok(ExportJobRecordsResponse {
                    page: request.page,
                    page_size: request.page_size,
                    total_count,
                    records: if total_count == 0 {
                        Vec::new()
                    } else {
                        vec![ExportJobRecord {
                            export_job_id: self.export_job_id,
                            edge_code: "edge-a".to_owned(),
                            export_job_status: request.export_job_status.unwrap_or_default(),
                            object_count: 1,
                            copied_count: 0,
                            total_bytes: 10,
                            copied_bytes: 0,
                            start_time: None,
                            finish_time: None,
                            error_message: None,
                        }]
                    },
                })
            })
        }

        fn summary<'a>(&'a self) -> ControlFuture<'a, EdgeControlSummary> {
            Box::pin(async move {
                let summary_calls = {
                    let mut calls = self.calls.lock().unwrap();
                    calls.push("summary");
                    calls.iter().filter(|call| **call == "summary").count()
                };
                let is_ready = self
                    .ready_after_summary_calls
                    .is_none_or(|ready_after| summary_calls >= ready_after);
                let disk = disk_summary(
                    if is_ready {
                        self.disk_status_code
                    } else {
                        "INITIALIZED"
                    },
                    if is_ready {
                        self.runtime_status
                    } else {
                        "CHECKING"
                    },
                );
                let global_progress = EdgeGlobalSummary {
                    total_bytes: 0,
                    done_bytes: 0,
                    remaining_bytes: 0,
                    speed_bytes_per_sec: 0,
                    object_total: 0,
                    object_done: 0,
                    object_remaining: 0,
                    percent: 0.0,
                };
                Ok(EdgeControlSummary {
                    source: "edge",
                    edge_code: "edge-a".to_owned(),
                    edge_name: "edge-a".to_owned(),
                    object_inventory: crate::progress::ObjectInventorySnapshot::default(),
                    export_job: None,
                    global: global_progress.clone(),
                    global_progress,
                    disk_runtime: vec![disk.clone()],
                    disks: vec![disk],
                    ws_connected: false,
                    last_http_refresh_at: Utc::now(),
                    message: "summary".to_owned(),
                })
            })
        }

        fn copy_progress_snapshot<'a>(&'a self) -> ControlFuture<'a, Option<CopyProgressEvent>> {
            Box::pin(async { Ok(None) })
        }

        fn scan_progress_snapshot<'a>(&'a self) -> ControlFuture<'a, ScanProgressSnapshot> {
            Box::pin(async { Ok(ScanProgressSnapshot::default()) })
        }
    }

    #[tokio::test]
    async fn auto_export_is_disabled_by_default() {
        let control = Arc::new(FakeControl::default());
        let orchestrator =
            AutoExportOrchestrator::new(AutoExportConfig::default(), control.clone());

        let decision = orchestrator.on_transport_disks_refreshed(trigger()).await;

        assert_eq!(decision.outcome, AutoExportOutcome::Disabled);
        assert!(control.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejects_when_no_initialized_ready_disk_exists() {
        let control = Arc::new(FakeControl {
            disk_status_code: "SEALED",
            runtime_status: "READY",
            ..FakeControl::default()
        });
        let orchestrator = AutoExportOrchestrator::new(enabled_config(), control.clone());

        let decision = orchestrator.on_transport_disks_refreshed(trigger()).await;

        assert_eq!(decision.outcome, AutoExportOutcome::NoReadyDisk);
        assert_eq!(control.calls.lock().unwrap().as_slice(), &["summary"]);
    }

    #[tokio::test]
    async fn active_export_job_blocks_duplicate_auto_start() {
        let control = Arc::new(FakeControl {
            active_status: Some("COPYING"),
            ..FakeControl::default()
        });
        let orchestrator = AutoExportOrchestrator::new(enabled_config(), control.clone());

        let decision = orchestrator.on_transport_disks_refreshed(trigger()).await;

        assert_eq!(decision.outcome, AutoExportOutcome::ActiveExportJob);
        assert_eq!(
            control.calls.lock().unwrap().as_slice(),
            &["summary", "export_jobs", "export_jobs", "export_jobs"]
        );
    }

    #[tokio::test]
    async fn udev_rescan_rechecks_until_the_disk_is_ready_before_starting_auto_export() {
        let control = Arc::new(FakeControl {
            ready_after_summary_calls: Some(2),
            ..FakeControl::default()
        });
        let orchestrator = AutoExportOrchestrator::new(enabled_config(), control.clone());
        let runner = AutoExportRescanRunner::new(Arc::new(NoopRescanRunner), orchestrator);

        runner
            .run_disk_rescan(DiskRescanTrigger::udev(Some("/dev/sdb1".to_owned())))
            .await
            .unwrap();

        assert_eq!(
            control.calls.lock().unwrap().as_slice(),
            &[
                "summary",
                "summary",
                "export_jobs",
                "export_jobs",
                "export_jobs",
                "export_jobs",
                "scan_once",
                "create_export_job",
                "start_export_job"
            ]
        );
    }

    #[tokio::test]
    async fn sealing_export_job_blocks_duplicate_auto_start() {
        let control = Arc::new(FakeControl {
            active_status: Some("SEALING"),
            ..FakeControl::default()
        });
        let orchestrator = AutoExportOrchestrator::new(enabled_config(), control.clone());

        let decision = orchestrator.on_transport_disks_refreshed(trigger()).await;

        assert_eq!(decision.outcome, AutoExportOutcome::ActiveExportJob);
        assert_eq!(
            control.calls.lock().unwrap().as_slice(),
            &[
                "summary",
                "export_jobs",
                "export_jobs",
                "export_jobs",
                "export_jobs"
            ]
        );
    }

    #[tokio::test]
    async fn cooldown_blocks_repeated_udev_events_before_scan_or_export() {
        let control = Arc::new(FakeControl::default());
        let orchestrator = AutoExportOrchestrator::new(
            AutoExportConfig {
                cooldown_seconds: 60,
                ..enabled_config()
            },
            control.clone(),
        );

        let first = orchestrator.on_transport_disks_refreshed(trigger()).await;
        let second = orchestrator.on_transport_disks_refreshed(trigger()).await;

        assert_eq!(first.outcome, AutoExportOutcome::Started);
        assert_eq!(second.outcome, AutoExportOutcome::Cooldown);
        assert_eq!(
            control.calls.lock().unwrap().as_slice(),
            &[
                "summary",
                "export_jobs",
                "export_jobs",
                "export_jobs",
                "export_jobs",
                "scan_once",
                "create_export_job",
                "start_export_job"
            ]
        );
    }

    #[tokio::test]
    async fn starts_existing_edge_control_flow_after_ready_rescan() {
        let control = Arc::new(FakeControl::default());
        let export_job_id = control.export_job_id;
        let orchestrator = AutoExportOrchestrator::new(enabled_config(), control.clone());

        let decision = orchestrator.on_transport_disks_refreshed(trigger()).await;

        assert_eq!(decision.outcome, AutoExportOutcome::Started);
        assert_eq!(decision.export_job_id, Some(export_job_id));
        assert_eq!(
            control.calls.lock().unwrap().as_slice(),
            &[
                "summary",
                "export_jobs",
                "export_jobs",
                "export_jobs",
                "export_jobs",
                "scan_once",
                "create_export_job",
                "start_export_job"
            ]
        );
    }

    #[tokio::test]
    async fn no_stable_objects_do_not_create_export_job() {
        let control = Arc::new(FakeControl {
            stable_object_count: 0,
            ..FakeControl::default()
        });
        let orchestrator = AutoExportOrchestrator::new(enabled_config(), control.clone());

        let decision = orchestrator.on_transport_disks_refreshed(trigger()).await;

        assert_eq!(decision.outcome, AutoExportOutcome::NoExportableObject);
        assert_eq!(
            control.calls.lock().unwrap().as_slice(),
            &[
                "summary",
                "export_jobs",
                "export_jobs",
                "export_jobs",
                "export_jobs",
                "scan_once"
            ]
        );
    }

    #[tokio::test]
    async fn control_refresh_rescan_does_not_start_auto_export() {
        let control = Arc::new(FakeControl::default());
        let orchestrator = AutoExportOrchestrator::new(enabled_config(), control.clone());
        let runner = AutoExportRescanRunner::new(Arc::new(NoopRescanRunner), orchestrator);

        let record_count = runner
            .run_disk_rescan(DiskRescanTrigger::control_refresh())
            .await
            .unwrap();

        assert_eq!(record_count, 1);
        assert!(control.calls.lock().unwrap().is_empty());
    }

    fn enabled_config() -> AutoExportConfig {
        AutoExportConfig {
            enabled: true,
            start_on_ready: true,
            min_ready_disk_count: 1,
            cooldown_seconds: 0,
        }
    }

    fn trigger() -> AutoExportTrigger {
        AutoExportTrigger {
            source: AutoExportTriggerSource::Udev,
            device: Some("/dev/sdb".to_owned()),
        }
    }

    fn control_error(error_code: &'static str) -> ControlError {
        ControlError {
            http_status: StatusCode::CONFLICT,
            error_code,
            message: "unused".to_owned(),
        }
    }

    fn disk_summary(
        disk_status_code: &str,
        runtime_status: &str,
    ) -> crate::control::DiskRuntimeSummary {
        crate::control::DiskRuntimeSummary {
            disk_presence_id: Some(Uuid::new_v4().to_string()),
            hardware_serial: "SN-A".to_owned(),
            disk_sn: "SN-A".to_owned(),
            stable_hardware_id: "fs-uuid-a".to_owned(),
            disk_id: Some(Uuid::new_v4()),
            device_path: "/dev/sdb1".to_owned(),
            mount_path: Some("/mnt/rustfs-transfer/disk-a".to_owned()),
            filesystem_type: Some("ext4".to_owned()),
            filesystem: Some("ext4".to_owned()),
            fs_uuid: Some("fs-uuid-a".to_owned()),
            filesystem_uuid: Some("fs-uuid-a".to_owned()),
            disk_status_code: disk_status_code.to_owned(),
            runtime_status: runtime_status.to_owned(),
            task_pool_eligible: runtime_status == "READY" && disk_status_code == "INITIALIZED",
            capacity_bytes: 100,
            total_bytes: 0,
            done_bytes: 0,
            remaining_bytes: 0,
            free_bytes: 80,
            object_budget_bytes: 64,
            export_job_id: None,
            seal_id: None,
            speed_bytes_per_sec: 0,
            object_total: 0,
            object_done: 0,
            object_remaining: 0,
            progress: EdgeDiskProgressSummary {
                total_bytes: 0,
                done_bytes: 0,
                remaining_bytes: 0,
                speed_bytes_per_sec: 0,
                object_total: 0,
                object_done: 0,
                object_remaining: 0,
                percent: 0.0,
            },
            current_object: Some(DashboardCurrentObject {
                bucket: "bucket".to_owned(),
                key: "key".to_owned(),
                display_name: "key".to_owned(),
                relative_data_path: "key".to_owned(),
                size_bytes: 10,
                done_bytes: 0,
                remaining_bytes: 10,
                speed_bytes_per_sec: 0,
                object_status: "PENDING".to_owned(),
            }),
            current_file: Some("key".to_owned()),
            current_file_size: 10,
            current_file_done: 0,
            last_error_code: None,
            error_message: None,
            message: format!("disk runtime_status={runtime_status}"),
        }
    }

    fn export_job_response(
        export_job_id: Uuid,
        export_job_status: &str,
        object_count: u64,
    ) -> ExportJobResponse {
        ExportJobResponse {
            export_job_id,
            edge_code: "edge-a".to_owned(),
            export_job_status: export_job_status.to_owned(),
            object_count,
            copied_count: 0,
            total_bytes: object_count * 10,
            copied_bytes: 0,
            start_time: None,
            finish_time: None,
            error_message: None,
            object_status_counts: BTreeMap::from([("PENDING".to_owned(), object_count)]),
            disks: vec![ExportJobDiskSummary {
                disk_id: Some(Uuid::new_v4()),
                disk_sn: Some("SN-A".to_owned()),
                device_path: Some("/dev/sdb1".to_owned()),
                mount_path: Some("/mnt/rustfs-transfer/disk-a".to_owned()),
                disk_status_code: Some("INITIALIZED".to_owned()),
                runtime_status: Some("READY".to_owned()),
                object_total: object_count,
                object_done: 0,
                total_bytes: object_count * 10,
                done_bytes: 0,
                last_error_code: None,
                error_message: None,
            }],
            events: vec![ExportJobEvent {
                event_type: "EXPORT_JOB_CREATED".to_owned(),
                event_time: Some(Utc::now()),
                export_job_status: Some(export_job_status.to_owned()),
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
