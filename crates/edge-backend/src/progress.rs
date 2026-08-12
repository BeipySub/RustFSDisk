use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiskProgressSnapshot {
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
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
    #[serde(default)]
    pub progress: DiskProgressFields,
    pub current_object: Option<CurrentObjectProgress>,
    #[serde(default)]
    pub last_error_code: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiskProgressFields {
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalProgressSnapshot {
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub remaining_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub object_remaining: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CopyProgressEvent {
    pub event_type: String,
    pub event_time: DateTime<Utc>,
    pub source: String,
    pub edge_code: String,
    pub export_job_id: String,
    pub disk_status_code: String,
    pub export_job_status: String,
    pub global_progress: GlobalProgressSnapshot,
    pub disks: Vec<DiskProgressSnapshot>,
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
        let disks = state
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
                };
                DiskProgressSnapshot {
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
                    speed_bytes_per_sec: progress.speed_bytes_per_sec,
                    object_total: progress.object_total,
                    object_done: progress.object_done,
                    object_remaining: progress.object_remaining,
                    progress,
                    current_object: disk.current_object.clone(),
                    last_error_code: None,
                    error_message: None,
                    message: disk.message.clone(),
                }
            })
            .collect();

        CopyProgressEvent {
            event_type: if requested_event_type == "COPY_PROGRESS" {
                state.event_type.clone()
            } else {
                requested_event_type
            },
            event_time: Utc::now(),
            source: "edge".to_string(),
            edge_code: state.edge_code.clone(),
            export_job_id: state.export_job_id.clone(),
            disk_status_code: state.disk_status_code.clone(),
            export_job_status: state.export_job_status.clone(),
            global_progress: GlobalProgressSnapshot {
                total_bytes,
                done_bytes,
                remaining_bytes: total_bytes.saturating_sub(done_bytes),
                speed_bytes_per_sec: done_bytes / elapsed,
                object_total,
                object_done,
                object_remaining: object_total.saturating_sub(object_done),
            },
            disks,
            message: message.into(),
        }
    }
}
