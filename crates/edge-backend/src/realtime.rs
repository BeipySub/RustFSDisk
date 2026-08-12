use std::sync::Arc;

use chrono::Utc;
use tokio::sync::RwLock;

use crate::{
    disk_detection::{DiskRuntimeEventPublisher, DiskRuntimeRecord},
    progress::{
        CopyProgressEvent, DiskProgressFields, DiskProgressSnapshot, GlobalProgressSnapshot,
        ObjectInventorySnapshot,
    },
};

#[derive(Clone)]
pub struct EdgeRealtimeHub {
    edge_code: Arc<str>,
    latest_disk_event: Arc<RwLock<Option<CopyProgressEvent>>>,
    current_disks: Arc<RwLock<Vec<DiskProgressSnapshot>>>,
}

impl EdgeRealtimeHub {
    pub fn new(edge_code: impl Into<String>) -> Self {
        Self {
            edge_code: Arc::<str>::from(edge_code.into()),
            latest_disk_event: Arc::new(RwLock::new(None)),
            current_disks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn latest_disk_event(&self) -> Option<CopyProgressEvent> {
        self.latest_disk_event.read().await.clone()
    }
}

impl DiskRuntimeEventPublisher for EdgeRealtimeHub {
    fn publish_disk_runtime(&self, record: &DiskRuntimeRecord) {
        let hub = self.latest_disk_event.clone();
        let current_disks = self.current_disks.clone();
        let edge_code = self.edge_code.clone();
        let record = record.clone();
        tokio::spawn(async move {
            let message = disk_runtime_message(&record);
            let mut disks = current_disks.write().await;
            if record.runtime_status == "REMOVED" {
                disks.retain(|snapshot| !same_disk_snapshot(snapshot, &record));
            } else {
                let snapshot = disk_progress_snapshot(&record, message.clone());
                if let Some(existing) = disks
                    .iter_mut()
                    .find(|snapshot| same_disk_snapshot(snapshot, &record))
                {
                    *existing = snapshot;
                } else {
                    disks.push(snapshot);
                }
            }
            let event = disk_runtime_event(&edge_code, &record, disks.clone(), message);
            *hub.write().await = Some(event);
        });
    }
}

fn disk_runtime_event(
    edge_code: &str,
    record: &DiskRuntimeRecord,
    disks: Vec<DiskProgressSnapshot>,
    message: String,
) -> CopyProgressEvent {
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
    let global_progress = GlobalProgressSnapshot {
        total_bytes: 0,
        done_bytes: 0,
        remaining_bytes: 0,
        speed_bytes_per_sec: 0,
        object_total: 0,
        object_done: 0,
        object_remaining: 0,
        percent: 0.0,
    };
    CopyProgressEvent {
        event_type: event_type.to_string(),
        event_time: Utc::now(),
        source: "edge".to_string(),
        edge_code: edge_code.to_string(),
        edge_name: edge_code.to_string(),
        object_inventory: ObjectInventorySnapshot::default(),
        export_job_id: String::new(),
        export_job: None,
        disk_status_code: disk_status_code.clone(),
        export_job_status: "PENDING".to_string(),
        global: global_progress.clone(),
        global_progress,
        disk_runtime: disks.clone(),
        disks,
        ws_connected: true,
        last_http_refresh_at: Utc::now(),
        message,
    }
}

fn disk_runtime_message(record: &DiskRuntimeRecord) -> String {
    match (&record.last_error_code, &record.error_message) {
        (Some(code), Some(message)) => format!("{code}: {message}"),
        (Some(code), None) => code.clone(),
        (None, Some(message)) => message.clone(),
        (None, None) => format!("disk runtime_status={}", record.runtime_status),
    }
}

fn disk_progress_snapshot(record: &DiskRuntimeRecord, message: String) -> DiskProgressSnapshot {
    let progress = DiskProgressFields::default();
    let disk_status_code = record
        .status_code
        .clone()
        .unwrap_or_else(|| "UNREGISTERED".to_string());
    DiskProgressSnapshot {
        disk_id: record.disk_id.clone().unwrap_or_default(),
        disk_sn: record.sn.clone(),
        hardware_serial: record.sn.clone(),
        stable_hardware_id: stable_hardware_id(record),
        device_path: record.device_path.clone(),
        mount_path: record.mount_path.clone().unwrap_or_default(),
        filesystem_type: None,
        filesystem: None,
        fs_uuid: record.fs_uuid.clone(),
        filesystem_uuid: record.fs_uuid.clone(),
        capacity_bytes: record.capacity_bytes,
        runtime_status: record.runtime_status.clone(),
        disk_status_code,
        task_pool_eligible: record.task_pool_eligible,
        total_bytes: progress.total_bytes,
        done_bytes: progress.done_bytes,
        remaining_bytes: progress.remaining_bytes,
        free_bytes: record.free_bytes,
        object_budget_bytes: record.object_budget_bytes,
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
        last_error_code: record.last_error_code.clone(),
        error_message: record.error_message.clone(),
        message,
    }
}

fn stable_hardware_id(record: &DiskRuntimeRecord) -> String {
    record
        .fs_uuid
        .as_deref()
        .or(record.id_serial_short.as_deref())
        .or(record.id_serial.as_deref())
        .or(record.label.as_deref())
        .unwrap_or(&record.sn)
        .to_string()
}

fn same_disk_snapshot(snapshot: &DiskProgressSnapshot, record: &DiskRuntimeRecord) -> bool {
    match record.disk_id.as_deref().filter(|value| !value.is_empty()) {
        Some(disk_id) => {
            snapshot.disk_id == disk_id
                || (snapshot.disk_id.is_empty()
                    && record
                        .mount_path
                        .as_deref()
                        .is_some_and(|mount_path| snapshot.mount_path == mount_path))
        }
        None => {
            snapshot.disk_sn == record.sn
                && record
                    .mount_path
                    .as_deref()
                    .is_some_and(|mount_path| snapshot.mount_path == mount_path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disk_runtime_event_uses_semantic_fields_without_naked_status() {
        let hub = EdgeRealtimeHub::new("edge-a");
        hub.publish_disk_runtime(&runtime_record(
            "SN-A",
            "11111111-1111-1111-1111-111111111111",
            "/mnt/rustfs-transfer/disk-a",
            "READY",
        ));

        tokio::task::yield_now().await;
        let value = serde_json::to_value(hub.latest_disk_event().await.unwrap()).unwrap();

        assert_eq!(value["event_type"], "DISK_READY");
        assert_eq!(value["source"], "edge");
        assert_eq!(value["edge_name"], "edge-a");
        assert_eq!(value["disk_status_code"], "INITIALIZED");
        assert!(value["object_inventory"].is_object());
        assert!(value["global"].is_object());
        assert!(value["global_progress"].is_object());
        assert!(value["disk_runtime"].is_array());
        assert_eq!(value["disk_runtime"], value["disks"]);
        assert_eq!(value["ws_connected"], true);
        assert!(value.get("last_http_refresh_at").is_some());
        assert_eq!(value["disks"][0]["runtime_status"], "READY");
        assert_eq!(value["disks"][0]["disk_status_code"], "INITIALIZED");
        assert_eq!(value["disks"][0]["hardware_serial"], "SN-A");
        assert_eq!(value["disks"][0]["stable_hardware_id"], "fs-uuid-SN-A");
        assert_eq!(value["disks"][0]["device_path"], "/dev/sdb1");
        assert_eq!(
            value["disks"][0]["mount_path"],
            "/mnt/rustfs-transfer/disk-a"
        );
        assert_eq!(value["disks"][0]["fs_uuid"], "fs-uuid-SN-A");
        assert_eq!(value["disks"][0]["filesystem_uuid"], "fs-uuid-SN-A");
        assert_eq!(value["disks"][0]["capacity_bytes"], 100);
        assert_eq!(value["disks"][0]["object_budget_bytes"], 70);
        assert!(value["disks"][0].get("export_job_id").is_some());
        assert!(value["disks"][0].get("seal_id").is_some());
        assert_eq!(value["disks"][0]["task_pool_eligible"], true);
        assert!(value["disks"][0]["progress"].is_object());
        assert_eq!(value["disks"][0]["progress"]["percent"], 0.0);
        assert!(value["disks"][0].get("current_file").is_some());
        assert!(value["disks"][0].get("current_file_size").is_some());
        assert!(value["disks"][0].get("current_file_done").is_some());
        assert!(value["disks"][0].get("last_error_code").is_some());
        assert!(value["disks"][0].get("error_message").is_some());
        assert!(value.get("status").is_none());
        assert!(value.get("disk_data_key").is_none());
    }

    #[tokio::test]
    async fn rejected_disk_runtime_event_keeps_standard_error_code_visible() {
        let hub = EdgeRealtimeHub::new("edge-a");
        let mut record = runtime_record("SN-A", "", "/mnt/rustfs-transfer/disk-a", "REJECTED");
        record.disk_id = None;
        record.status_code = None;
        record.disk_enabled = None;
        record.task_pool_eligible = false;
        record.last_error_code = Some("FILESYSTEM_UNSUPPORTED".to_string());
        record.error_message = Some("transport disks must be ext4".to_string());
        hub.publish_disk_runtime(&record);

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

    #[tokio::test]
    async fn removed_disk_event_publishes_remaining_current_disk_snapshot() {
        let hub = EdgeRealtimeHub::new("edge-a");
        let disk_a = runtime_record(
            "SN-A",
            "11111111-1111-1111-1111-111111111111",
            "/mnt/rustfs-transfer/disk-a",
            "READY",
        );
        let disk_b = runtime_record(
            "SN-B",
            "22222222-2222-2222-2222-222222222222",
            "/mnt/rustfs-transfer/disk-b",
            "READY",
        );
        let mut removed_a = disk_a.clone();
        removed_a.runtime_status = "REMOVED".to_string();
        removed_a.last_error_code = Some("DISK_REMOVED".to_string());
        removed_a.error_message = Some("transport disk removed".to_string());

        hub.publish_disk_runtime(&disk_a);
        tokio::task::yield_now().await;
        hub.publish_disk_runtime(&disk_b);
        tokio::task::yield_now().await;
        hub.publish_disk_runtime(&removed_a);
        tokio::task::yield_now().await;

        let value = serde_json::to_value(hub.latest_disk_event().await.unwrap()).unwrap();

        assert_eq!(value["event_type"], "DISK_REMOVED");
        assert_eq!(value["disks"].as_array().unwrap().len(), 1);
        assert_eq!(
            value["disks"][0]["mount_path"],
            "/mnt/rustfs-transfer/disk-b"
        );
        assert_ne!(
            value["disks"][0]["mount_path"],
            "/mnt/rustfs-transfer/disk-a"
        );
    }

    #[tokio::test]
    async fn same_sn_unregistered_disks_are_matched_by_mount_path_for_current_snapshot() {
        let hub = EdgeRealtimeHub::new("edge-a");
        let mut disk_a = runtime_record(
            "DUPLICATE-SN",
            "",
            "/mnt/rustfs-transfer/disk-a",
            "REJECTED",
        );
        disk_a.disk_id = None;
        disk_a.status_code = Some("UNREGISTERED".to_string());
        disk_a.last_error_code = Some("MANIFEST_INVALID".to_string());
        let mut disk_b = runtime_record(
            "DUPLICATE-SN",
            "",
            "/mnt/rustfs-transfer/disk-b",
            "REJECTED",
        );
        disk_b.disk_id = None;
        disk_b.status_code = Some("UNREGISTERED".to_string());
        disk_b.last_error_code = Some("MANIFEST_INVALID".to_string());
        let mut removed_a = disk_a.clone();
        removed_a.runtime_status = "REMOVED".to_string();
        removed_a.last_error_code = Some("DISK_REMOVED".to_string());

        hub.publish_disk_runtime(&disk_a);
        tokio::task::yield_now().await;
        hub.publish_disk_runtime(&disk_b);
        tokio::task::yield_now().await;

        let value = serde_json::to_value(hub.latest_disk_event().await.unwrap()).unwrap();
        assert_eq!(value["event_type"], "DISK_REJECTED");
        assert_eq!(value["disks"].as_array().unwrap().len(), 2);
        assert_eq!(
            value["disks"][0]["mount_path"],
            "/mnt/rustfs-transfer/disk-a"
        );
        assert_eq!(
            value["disks"][1]["mount_path"],
            "/mnt/rustfs-transfer/disk-b"
        );

        hub.publish_disk_runtime(&removed_a);
        tokio::task::yield_now().await;
        let value = serde_json::to_value(hub.latest_disk_event().await.unwrap()).unwrap();
        assert_eq!(value["event_type"], "DISK_REMOVED");
        assert_eq!(value["disks"].as_array().unwrap().len(), 1);
        assert_eq!(
            value["disks"][0]["mount_path"],
            "/mnt/rustfs-transfer/disk-b"
        );
    }

    #[tokio::test]
    async fn protocol_disk_id_assignment_replaces_pre_admission_snapshot_and_remove_clears_it() {
        let hub = EdgeRealtimeHub::new("edge-a");
        let mut detected = runtime_record("SN-A", "", "/mnt/rustfs-transfer/disk-a", "CHECKING");
        detected.disk_id = None;
        detected.status_code = None;
        let rejected = runtime_record(
            "SN-A",
            "11111111-1111-1111-1111-111111111111",
            "/mnt/rustfs-transfer/disk-a",
            "REJECTED",
        );
        let mut removed = rejected.clone();
        removed.runtime_status = "REMOVED".to_string();
        removed.last_error_code = Some("DISK_REMOVED".to_string());
        removed.error_message = Some("transport disk removed".to_string());

        hub.publish_disk_runtime(&detected);
        tokio::task::yield_now().await;
        hub.publish_disk_runtime(&rejected);
        tokio::task::yield_now().await;

        let value = serde_json::to_value(hub.latest_disk_event().await.unwrap()).unwrap();
        assert_eq!(value["event_type"], "DISK_REJECTED");
        assert_eq!(value["disks"].as_array().unwrap().len(), 1);
        assert_eq!(
            value["disks"][0]["disk_id"],
            "11111111-1111-1111-1111-111111111111"
        );

        hub.publish_disk_runtime(&removed);
        tokio::task::yield_now().await;

        let value = serde_json::to_value(hub.latest_disk_event().await.unwrap()).unwrap();
        assert_eq!(value["event_type"], "DISK_REMOVED");
        assert_eq!(value["disks"].as_array().unwrap().len(), 0);
    }

    fn runtime_record(
        sn: &str,
        disk_id: &str,
        mount_path: &str,
        runtime_status: &str,
    ) -> DiskRuntimeRecord {
        DiskRuntimeRecord {
            sn: sn.to_string(),
            fs_uuid: Some(format!("fs-uuid-{sn}")),
            label: Some(format!("RUSTFS-{sn}")),
            id_serial: Some(format!("USB-{sn}")),
            id_serial_short: Some(sn.to_string()),
            disk_id: (!disk_id.is_empty()).then(|| disk_id.to_string()),
            device_path: "/dev/sdb1".to_string(),
            mount_path: Some(mount_path.to_string()),
            capacity_bytes: 100,
            free_bytes: 80,
            reserve_bytes: 10,
            object_budget_bytes: 70,
            runtime_status: runtime_status.to_string(),
            last_error_code: None,
            error_message: None,
            partial_residue_count: 0,
            partial_residue_bytes: 0,
            last_seen_at: Utc::now(),
            task_pool_eligible: runtime_status == "READY",
            status_code: Some("INITIALIZED".to_string()),
            disk_enabled: Some(true),
        }
    }
}
