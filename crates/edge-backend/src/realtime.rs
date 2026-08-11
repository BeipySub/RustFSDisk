use std::sync::Arc;

use chrono::Utc;
use tokio::sync::RwLock;

use crate::{
    disk_detection::{DiskRuntimeEventPublisher, DiskRuntimeRecord},
    progress::{CopyProgressEvent, DiskProgressSnapshot, GlobalProgressSnapshot},
};

#[derive(Clone)]
pub struct EdgeRealtimeHub {
    edge_code: Arc<str>,
    latest_disk_event: Arc<RwLock<Option<CopyProgressEvent>>>,
}

impl EdgeRealtimeHub {
    pub fn new(edge_code: impl Into<String>) -> Self {
        Self {
            edge_code: Arc::<str>::from(edge_code.into()),
            latest_disk_event: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn latest_disk_event(&self) -> Option<CopyProgressEvent> {
        self.latest_disk_event.read().await.clone()
    }
}

impl DiskRuntimeEventPublisher for EdgeRealtimeHub {
    fn publish_disk_runtime(&self, record: &DiskRuntimeRecord) {
        let hub = self.latest_disk_event.clone();
        let event = disk_runtime_event(&self.edge_code, record);
        tokio::spawn(async move {
            *hub.write().await = Some(event);
        });
    }
}

fn disk_runtime_event(edge_code: &str, record: &DiskRuntimeRecord) -> CopyProgressEvent {
    let event_type = match record.runtime_status.as_str() {
        "DETECTED" => "DISK_DETECTED",
        "CHECKING" => "DISK_CHECKING",
        "READY" => "DISK_READY",
        "REJECTED" => "DISK_REJECTED",
        "REMOVED" => "DISK_REMOVED",
        "ERROR" => "ERROR",
        "COPYING" => "COPY_PROGRESS",
        "DONE" => "COPY_DONE",
        _ => "DISK_RUNTIME_CHANGED",
    };
    let disk_status_code = record
        .status_code
        .clone()
        .unwrap_or_else(|| "UNREGISTERED".to_string());
    let message = match (&record.last_error_code, &record.error_message) {
        (Some(code), Some(message)) => format!("{code}: {message}"),
        (Some(code), None) => code.clone(),
        (None, Some(message)) => message.clone(),
        (None, None) => format!("disk runtime_status={}", record.runtime_status),
    };

    CopyProgressEvent {
        event_type: event_type.to_string(),
        event_time: Utc::now(),
        source: "edge".to_string(),
        edge_code: edge_code.to_string(),
        export_job_id: String::new(),
        disk_status_code: disk_status_code.clone(),
        export_job_status: "PENDING".to_string(),
        global_progress: GlobalProgressSnapshot {
            total_bytes: 0,
            done_bytes: 0,
            remaining_bytes: 0,
            speed_bytes_per_sec: 0,
            object_total: 0,
            object_done: 0,
            object_remaining: 0,
        },
        disks: vec![DiskProgressSnapshot {
            disk_id: record.disk_id.clone().unwrap_or_default(),
            disk_sn: record.sn.clone(),
            mount_path: record.mount_path.clone().unwrap_or_default(),
            runtime_status: record.runtime_status.clone(),
            total_bytes: 0,
            done_bytes: 0,
            remaining_bytes: 0,
            free_bytes: record.free_bytes,
            speed_bytes_per_sec: 0,
            object_total: 0,
            object_done: 0,
            object_remaining: 0,
            current_object: None,
            message: message.clone(),
        }],
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disk_runtime_event_uses_semantic_fields_without_naked_status() {
        let hub = EdgeRealtimeHub::new("edge-a");
        hub.publish_disk_runtime(&DiskRuntimeRecord {
            sn: "SN-A".to_string(),
            fs_uuid: Some("fs-uuid-a".to_string()),
            label: Some("RUSTFS-A".to_string()),
            id_serial: Some("USB-SN-A".to_string()),
            id_serial_short: Some("SN-A".to_string()),
            disk_id: Some("11111111-1111-1111-1111-111111111111".to_string()),
            device_path: "/dev/sdb1".to_string(),
            mount_path: Some("/mnt/rustfs-transfer/disk-a".to_string()),
            capacity_bytes: 100,
            free_bytes: 80,
            reserve_bytes: 10,
            object_budget_bytes: 70,
            runtime_status: "READY".to_string(),
            last_error_code: None,
            error_message: None,
            partial_residue_count: 0,
            partial_residue_bytes: 0,
            last_seen_at: Utc::now(),
            task_pool_eligible: true,
            status_code: Some("INITIALIZED".to_string()),
            disk_enabled: Some(true),
        });

        tokio::task::yield_now().await;
        let value = serde_json::to_value(hub.latest_disk_event().await.unwrap()).unwrap();

        assert_eq!(value["event_type"], "DISK_READY");
        assert_eq!(value["source"], "edge");
        assert_eq!(value["disk_status_code"], "INITIALIZED");
        assert_eq!(value["disks"][0]["runtime_status"], "READY");
        assert!(value.get("status").is_none());
        assert!(value.get("disk_data_key").is_none());
    }

    #[tokio::test]
    async fn rejected_disk_runtime_event_keeps_standard_error_code_visible() {
        let hub = EdgeRealtimeHub::new("edge-a");
        hub.publish_disk_runtime(&DiskRuntimeRecord {
            sn: "SN-A".to_string(),
            fs_uuid: Some("fs-uuid-a".to_string()),
            label: None,
            id_serial: None,
            id_serial_short: Some("SN-A".to_string()),
            disk_id: None,
            device_path: "/dev/sdb1".to_string(),
            mount_path: Some("/mnt/rustfs-transfer/disk-a".to_string()),
            capacity_bytes: 100,
            free_bytes: 80,
            reserve_bytes: 10,
            object_budget_bytes: 70,
            runtime_status: "REJECTED".to_string(),
            last_error_code: Some("FILESYSTEM_UNSUPPORTED".to_string()),
            error_message: Some("transport disks must be ext4".to_string()),
            partial_residue_count: 0,
            partial_residue_bytes: 0,
            last_seen_at: Utc::now(),
            task_pool_eligible: false,
            status_code: None,
            disk_enabled: None,
        });

        tokio::task::yield_now().await;
        let value = serde_json::to_value(hub.latest_disk_event().await.unwrap()).unwrap();

        assert_eq!(value["event_type"], "DISK_REJECTED");
        assert_eq!(value["disk_status_code"], "UNREGISTERED");
        assert_eq!(value["disks"][0]["runtime_status"], "REJECTED");
        assert!(value["message"]
            .as_str()
            .unwrap()
            .contains("FILESYSTEM_UNSUPPORTED"));
        assert!(value.get("status").is_none());
    }
}
