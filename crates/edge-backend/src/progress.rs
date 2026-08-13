use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CurrentObjectProgress {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiskProgressSnapshot {
    #[serde(default)]
    pub disk_presence_id: String,
    pub disk_id: String,
    pub disk_sn: String,
    #[serde(default)]
    pub hardware_serial: String,
    #[serde(default)]
    pub stable_hardware_id: String,
    #[serde(default)]
    pub device_path: String,
    pub mount_path: String,
    #[serde(default)]
    pub filesystem_type: Option<String>,
    #[serde(default)]
    pub filesystem: Option<String>,
    #[serde(default)]
    pub fs_uuid: Option<String>,
    #[serde(default)]
    pub filesystem_uuid: Option<String>,
    #[serde(default)]
    pub capacity_bytes: u64,
    pub runtime_status: String,
    #[serde(default)]
    pub disk_status_code: String,
    #[serde(default)]
    pub task_pool_eligible: bool,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub free_bytes: u64,
    #[serde(default)]
    pub object_budget_bytes: u64,
    #[serde(default)]
    pub export_job_id: Option<String>,
    #[serde(default)]
    pub seal_id: Option<String>,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
    #[serde(default)]
    pub progress: DiskProgressFields,
    pub current_object: Option<CurrentObjectProgress>,
    #[serde(default)]
    pub current_file: Option<String>,
    #[serde(default)]
    pub current_file_size: u64,
    #[serde(default)]
    pub current_file_done: u64,
    #[serde(default)]
    pub last_error_code: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DiskProgressFields {
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
    pub percent: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GlobalProgressSnapshot {
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
    #[serde(default)]
    pub percent: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectInventorySnapshot {
    pub total_bytes: u64,
    pub exported_bytes: u64,
    pub total_count: u64,
    pub exported_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DashboardExportJobSnapshot {
    pub export_job_id: String,
    pub export_job_status: String,
    pub start_time: Option<DateTime<Utc>>,
    pub finish_time: Option<DateTime<Utc>>,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
    #[serde(default)]
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyProgressEvent {
    #[serde(default)]
    pub protocol_version: String,
    #[serde(default)]
    pub event_id: String,
    pub event_type: String,
    pub event_time: DateTime<Utc>,
    pub source: String,
    pub edge_code: String,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub edge_name: String,
    #[serde(default)]
    pub scan: Option<Value>,
    #[serde(default)]
    pub object_inventory: ObjectInventorySnapshot,
    #[serde(default)]
    pub export_job: Option<DashboardExportJobSnapshot>,
    #[serde(default)]
    pub global: GlobalProgressSnapshot,
    pub global_progress: GlobalProgressSnapshot,
    #[serde(default)]
    pub disk_runtime: Vec<DiskProgressSnapshot>,
    pub disks: Vec<DiskProgressSnapshot>,
    #[serde(default)]
    pub ws_connected: bool,
    pub last_http_refresh_at: DateTime<Utc>,
    pub message: String,
}

#[derive(Clone)]
pub struct ProgressAggregator {
    inner: Arc<Mutex<ProgressState>>,
}

#[derive(Debug)]
struct ProgressState {
    edge_code: String,
    export_job_id: String,
    event_type: String,
    disk_status_code: String,
    export_job_status: String,
    disks: BTreeMap<String, DiskProgress>,
    started_at: Instant,
}

#[derive(Debug, Clone)]
struct DiskProgress {
    disk_id: String,
    disk_sn: String,
    mount_path: String,
    runtime_status: String,
    total_bytes: u64,
    done_bytes: u64,
    free_bytes: u64,
    object_total: u64,
    object_done: u64,
    current_object: Option<CurrentObjectProgress>,
    message: String,
}

impl ProgressAggregator {
    pub fn new(edge_code: impl Into<String>, export_job_id: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ProgressState {
                edge_code: edge_code.into(),
                export_job_id: export_job_id.into(),
                event_type: "COPY_STARTED".to_string(),
                disk_status_code: "EDGE_COPYING".to_string(),
                export_job_status: "COPYING".to_string(),
                disks: BTreeMap::new(),
                started_at: Instant::now(),
            })),
        }
    }

    pub fn register_disk(
        &self,
        disk_id: impl Into<String>,
        disk_sn: impl Into<String>,
        mount_path: impl Into<String>,
        total_bytes: u64,
        object_total: u64,
        free_bytes: u64,
    ) {
        let disk_id = disk_id.into();
        let mut state = self.inner.lock().expect("progress mutex poisoned");
        state.disks.insert(
            disk_id.clone(),
            DiskProgress {
                disk_id,
                disk_sn: disk_sn.into(),
                mount_path: mount_path.into(),
                runtime_status: "COPYING".to_string(),
                total_bytes,
                done_bytes: 0,
                free_bytes,
                object_total,
                object_done: 0,
                current_object: None,
                message: "copying".to_string(),
            },
        );
    }

    pub fn start_object(
        &self,
        disk_id: &str,
        bucket: impl Into<String>,
        key: impl Into<String>,
        relative_data_path: impl Into<String>,
        size_bytes: u64,
    ) {
        let key = key.into();
        let display_name = key.rsplit('/').next().unwrap_or(&key).to_string();
        let mut state = self.inner.lock().expect("progress mutex poisoned");
        state.event_type = "COPY_PROGRESS".to_string();
        if let Some(disk) = state.disks.get_mut(disk_id) {
            disk.current_object = Some(CurrentObjectProgress {
                bucket: bucket.into(),
                key,
                display_name,
                relative_data_path: relative_data_path.into(),
                size_bytes,
                done_bytes: 0,
                remaining_bytes: size_bytes,
                speed_bytes_per_sec: 0,
                object_status: "COPYING".to_string(),
            });
        }
    }

    pub fn add_bytes(&self, disk_id: &str, bytes: u64) {
        let mut state = self.inner.lock().expect("progress mutex poisoned");
        let elapsed = state.started_at.elapsed().as_secs().max(1);
        if let Some(disk) = state.disks.get_mut(disk_id) {
            disk.done_bytes = disk.done_bytes.saturating_add(bytes).min(disk.total_bytes);
            let disk_speed = disk.done_bytes / elapsed;
            if let Some(current) = disk.current_object.as_mut() {
                current.done_bytes = current
                    .done_bytes
                    .saturating_add(bytes)
                    .min(current.size_bytes);
                current.remaining_bytes = current.size_bytes.saturating_sub(current.done_bytes);
                current.speed_bytes_per_sec = disk_speed;
            }
        }
    }

    pub fn complete_object(&self, disk_id: &str) {
        let mut state = self.inner.lock().expect("progress mutex poisoned");
        if let Some(disk) = state.disks.get_mut(disk_id) {
            disk.object_done = disk.object_done.saturating_add(1).min(disk.object_total);
            if let Some(current) = disk.current_object.as_mut() {
                current.object_status = "EXPORTED".to_string();
                current.done_bytes = current.size_bytes;
                current.remaining_bytes = 0;
            }
        }
        if !state.disks.is_empty()
            && state
                .disks
                .values()
                .all(|disk| disk.object_done >= disk.object_total)
        {
            state.event_type = "COPY_DONE".to_string();
        }
    }

    pub fn fail_disk(&self, disk_id: &str, error_code: impl Into<String>) {
        let mut state = self.inner.lock().expect("progress mutex poisoned");
        state.event_type = "ERROR".to_string();
        state.export_job_status = "FAILED".to_string();
        state.disk_status_code = "ERROR".to_string();
        if let Some(disk) = state.disks.get_mut(disk_id) {
            disk.runtime_status = "ERROR".to_string();
            disk.message = error_code.into();
        }
    }

    pub fn mark_disk_done(&self, disk_id: &str) {
        let mut state = self.inner.lock().expect("progress mutex poisoned");
        if let Some(disk) = state.disks.get_mut(disk_id) {
            disk.runtime_status = "DONE".to_string();
            disk.current_object = None;
            disk.message = "sealed".to_string();
        }
        if !state.disks.is_empty()
            && state
                .disks
                .values()
                .all(|disk| disk.runtime_status == "DONE")
        {
            state.event_type = "SEAL_DONE".to_string();
            state.disk_status_code = "SEALED".to_string();
            state.export_job_status = "SEALED".to_string();
        }
    }

    pub fn snapshot(
        &self,
        event_type: impl Into<String>,
        message: impl Into<String>,
    ) -> CopyProgressEvent {
        let requested_event_type = event_type.into();
        let state = self.inner.lock().expect("progress mutex poisoned");
        let elapsed = state.started_at.elapsed().as_secs().max(1);
        let total_bytes = state.disks.values().map(|disk| disk.total_bytes).sum();
        let done_bytes = state.disks.values().map(|disk| disk.done_bytes).sum();
        let object_total = state.disks.values().map(|disk| disk.object_total).sum();
        let object_done = state.disks.values().map(|disk| disk.object_done).sum();
        let disks: Vec<DiskProgressSnapshot> = state
            .disks
            .values()
            .map(|disk| {
                let progress = DiskProgressFields {
                    total_bytes: disk.total_bytes,
                    done_bytes: disk.done_bytes,
                    remaining_bytes: disk.total_bytes.saturating_sub(disk.done_bytes),
                    speed_bytes_per_sec: disk.done_bytes / elapsed,
                    object_total: disk.object_total,
                    object_done: disk.object_done,
                    object_remaining: disk.object_total.saturating_sub(disk.object_done),
                    percent: percent(disk.done_bytes, disk.total_bytes),
                };
                let current_file = disk
                    .current_object
                    .as_ref()
                    .map(|object| object.display_name.clone());
                let current_file_size = disk
                    .current_object
                    .as_ref()
                    .map(|object| object.size_bytes)
                    .unwrap_or(0);
                let current_file_done = disk
                    .current_object
                    .as_ref()
                    .map(|object| object.done_bytes)
                    .unwrap_or(0);
                DiskProgressSnapshot {
                    disk_presence_id: String::new(),
                    disk_id: disk.disk_id.clone(),
                    disk_sn: disk.disk_sn.clone(),
                    hardware_serial: disk.disk_sn.clone(),
                    stable_hardware_id: disk.disk_sn.clone(),
                    device_path: String::new(),
                    mount_path: disk.mount_path.clone(),
                    filesystem_type: None,
                    filesystem: None,
                    fs_uuid: None,
                    filesystem_uuid: None,
                    capacity_bytes: 0,
                    runtime_status: disk.runtime_status.clone(),
                    disk_status_code: state.disk_status_code.clone(),
                    task_pool_eligible: false,
                    total_bytes: progress.total_bytes,
                    done_bytes: progress.done_bytes,
                    remaining_bytes: progress.remaining_bytes,
                    free_bytes: disk.free_bytes,
                    object_budget_bytes: 0,
                    export_job_id: Some(state.export_job_id.clone()),
                    seal_id: None,
                    speed_bytes_per_sec: progress.speed_bytes_per_sec,
                    object_total: progress.object_total,
                    object_done: progress.object_done,
                    object_remaining: progress.object_remaining,
                    progress,
                    current_object: disk.current_object.clone(),
                    current_file,
                    current_file_size,
                    current_file_done,
                    last_error_code: None,
                    error_message: None,
                    message: disk.message.clone(),
                }
            })
            .collect();
        let global_progress = GlobalProgressSnapshot {
            total_bytes,
            done_bytes,
            remaining_bytes: total_bytes.saturating_sub(done_bytes),
            speed_bytes_per_sec: done_bytes / elapsed,
            object_total,
            object_done,
            object_remaining: object_total.saturating_sub(object_done),
            percent: percent(done_bytes, total_bytes),
        };
        let export_job = DashboardExportJobSnapshot {
            export_job_id: state.export_job_id.clone(),
            export_job_status: state.export_job_status.clone(),
            start_time: None,
            finish_time: None,
            total_bytes: global_progress.total_bytes,
            done_bytes: global_progress.done_bytes,
            remaining_bytes: global_progress.remaining_bytes,
            speed_bytes_per_sec: global_progress.speed_bytes_per_sec,
            object_total: global_progress.object_total,
            object_done: global_progress.object_done,
            object_remaining: global_progress.object_remaining,
            percent: global_progress.percent,
        };

        CopyProgressEvent {
            protocol_version: "edge-ws-v2".to_string(),
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type: if requested_event_type == "COPY_PROGRESS" {
                state.event_type.clone()
            } else {
                requested_event_type
            },
            event_time: Utc::now(),
            source: "edge".to_string(),
            edge_code: state.edge_code.clone(),
            stage: Some(copy_stage(&state.event_type).to_string()),
            edge_name: state.edge_code.clone(),
            scan: None,
            object_inventory: ObjectInventorySnapshot::default(),
            export_job: Some(export_job),
            global: global_progress.clone(),
            global_progress,
            disk_runtime: disks.clone(),
            disks,
            ws_connected: true,
            last_http_refresh_at: Utc::now(),
            message: message.into(),
        }
    }
}

fn percent(done_bytes: u64, total_bytes: u64) -> f64 {
    if total_bytes == 0 {
        0.0
    } else {
        (done_bytes as f64 / total_bytes as f64) * 100.0
    }
}

fn copy_stage(event_type: &str) -> &'static str {
    match event_type {
        "COPY_DONE" => "SEALING",
        "SEAL_DONE" => "SEALED",
        "ERROR" => "FAILED",
        _ => "COPYING",
    }
}
