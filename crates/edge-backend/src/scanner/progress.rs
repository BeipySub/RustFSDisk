use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgressSnapshot {
    pub event_type: &'static str,
    pub event_time: DateTime<Utc>,
    pub source: &'static str,
    pub scan_phase: &'static str,
    pub bucket_total: u64,
    pub bucket_done: u64,
    pub object_seen: u64,
    pub stable_object_count: u64,
    pub source_changed_count: u64,
    pub total_bytes: u64,
    pub current_bucket: Option<String>,
    pub current_object_key: Option<String>,
    pub last_error_code: Option<String>,
    pub message: Option<String>,
}

impl Default for ScanProgressSnapshot {
    fn default() -> Self {
        Self {
            event_type: "SCAN_PROGRESS",
            event_time: Utc::now(),
            source: "edge",
            scan_phase: "IDLE",
            bucket_total: 0,
            bucket_done: 0,
            object_seen: 0,
            stable_object_count: 0,
            source_changed_count: 0,
            total_bytes: 0,
            current_bucket: None,
            current_object_key: None,
            last_error_code: None,
            message: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProgressAggregator {
    snapshot: Arc<Mutex<ScanProgressSnapshot>>,
}

impl ProgressAggregator {
    pub fn snapshot(&self) -> ScanProgressSnapshot {
        self.snapshot
            .lock()
            .expect("scan progress lock poisoned")
            .clone()
    }

    pub fn start_scan(&self, bucket_total: u64) {
        self.update(|snapshot| {
            *snapshot = ScanProgressSnapshot {
                event_type: "SCAN_STARTED",
                event_time: Utc::now(),
                source: "edge",
                scan_phase: "SCANNING",
                bucket_total,
                ..ScanProgressSnapshot::default()
            };
        });
    }

    pub fn start_bucket(&self, bucket: &str) {
        self.update(|snapshot| {
            snapshot.event_type = "SCAN_PROGRESS";
            snapshot.event_time = Utc::now();
            snapshot.scan_phase = "SCANNING";
            snapshot.current_bucket = Some(bucket.to_owned());
            snapshot.current_object_key = None;
        });
    }

    pub fn observe_object(&self, bucket: &str, object_key: &str) {
        self.update(|snapshot| {
            snapshot.event_type = "SCAN_PROGRESS";
            snapshot.event_time = Utc::now();
            snapshot.scan_phase = "SCANNING";
            snapshot.current_bucket = Some(bucket.to_owned());
            snapshot.current_object_key = Some(object_key.to_owned());
            snapshot.object_seen += 1;
        });
    }

    pub fn record_stable_object(&self, size_bytes: i64) {
        self.update(|snapshot| {
            snapshot.event_time = Utc::now();
            snapshot.stable_object_count += 1;
            snapshot.total_bytes += size_bytes.max(0) as u64;
        });
    }

    pub fn record_source_changed(&self) {
        self.update(|snapshot| {
            snapshot.event_time = Utc::now();
            snapshot.source_changed_count += 1;
        });
    }

    pub fn finish_bucket(&self) {
        self.update(|snapshot| {
            snapshot.event_time = Utc::now();
            snapshot.bucket_done += 1;
            snapshot.current_object_key = None;
        });
    }

    pub fn finish_scan(&self) {
        self.update(|snapshot| {
            snapshot.event_type = "SCAN_DONE";
            snapshot.event_time = Utc::now();
            snapshot.scan_phase = "DONE";
            snapshot.current_bucket = None;
            snapshot.current_object_key = None;
            snapshot.message = Some("RustFS scan completed".to_owned());
        });
    }

    pub fn fail_scan(&self, error_code: &str, message: String) {
        self.update(|snapshot| {
            snapshot.event_type = "ERROR";
            snapshot.event_time = Utc::now();
            snapshot.scan_phase = "ERROR";
            snapshot.last_error_code = Some(error_code.to_owned());
            snapshot.message = Some(message);
        });
    }

    fn update(&self, update: impl FnOnce(&mut ScanProgressSnapshot)) {
        let mut snapshot = self.snapshot.lock().expect("scan progress lock poisoned");
        update(&mut snapshot);
    }
}
