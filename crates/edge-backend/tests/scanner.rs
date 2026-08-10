use chrono::{DateTime, TimeZone, Utc};
use rustfs_transfer_edge::scanner::{
    BoxFutureResult, ObjectBody, ObjectHead, ObjectScanner, ObjectSnapshotRepository,
    ObjectSummary, ProgressAggregator, RustFsReadClient, ScanError, ScanOptions, StableStatus,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Debug)]
struct SnapshotRecord {
    object: ObjectHead,
    stable_status: StableStatus,
}

#[derive(Debug)]
struct EnqueuedRecord {
    object: ObjectHead,
    object_status: &'static str,
    error_code: Option<&'static str>,
}

#[derive(Debug, Default)]
struct MemoryRepository {
    snapshots: Mutex<Vec<SnapshotRecord>>,
    enqueued: Mutex<Vec<EnqueuedRecord>>,
}

impl ObjectSnapshotRepository for MemoryRepository {
    fn save_snapshot<'a>(
        &'a self,
        object: &'a ObjectHead,
        stable_status: StableStatus,
        _scanned_at: DateTime<Utc>,
    ) -> rustfs_transfer_edge::scanner::BoxRepoFuture<'a, ()> {
        Box::pin(async move {
            self.snapshots
                .lock()
                .expect("snapshot lock poisoned")
                .push(SnapshotRecord {
                    object: object.clone(),
                    stable_status,
                });
            Ok(())
        })
    }

    fn enqueue_export_object<'a>(
        &'a self,
        _export_job_id: Uuid,
        object: &'a ObjectHead,
        object_status: &'static str,
        error_code: Option<&'static str>,
        _error_message: Option<&'static str>,
    ) -> rustfs_transfer_edge::scanner::BoxRepoFuture<'a, ()> {
        Box::pin(async move {
            self.enqueued
                .lock()
                .expect("enqueue lock poisoned")
                .push(EnqueuedRecord {
                    object: object.clone(),
                    object_status,
                    error_code,
                });
            Ok(())
        })
    }
}

#[derive(Debug)]
struct MemoryRustFs {
    buckets: Vec<String>,
    objects: HashMap<String, Vec<ObjectSummary>>,
    heads: Mutex<HashMap<(String, String), Vec<ObjectHead>>>,
}

impl RustFsReadClient for MemoryRustFs {
    fn list_buckets(&self) -> BoxFutureResult<'_, Vec<String>> {
        Box::pin(async { Ok(self.buckets.clone()) })
    }

    fn list_objects<'a>(&'a self, bucket: &'a str) -> BoxFutureResult<'a, Vec<ObjectSummary>> {
        Box::pin(async move { Ok(self.objects.get(bucket).cloned().unwrap_or_default()) })
    }

    fn head_object<'a>(
        &'a self,
        bucket: &'a str,
        object_key: &'a str,
    ) -> BoxFutureResult<'a, ObjectHead> {
        Box::pin(async move {
            let mut heads = self.heads.lock().expect("head lock poisoned");
            let key = (bucket.to_owned(), object_key.to_owned());
            let sequence = heads
                .get_mut(&key)
                .ok_or_else(|| ScanError::RustFs("missing test head".to_owned()))?;
            if sequence.len() > 1 {
                Ok(sequence.remove(0))
            } else {
                sequence
                    .first()
                    .cloned()
                    .ok_or_else(|| ScanError::RustFs("empty test head".to_owned()))
            }
        })
    }

    fn get_object<'a>(
        &'a self,
        _bucket: &'a str,
        _object_key: &'a str,
    ) -> BoxFutureResult<'a, ObjectBody> {
        Box::pin(async { Ok(ObjectBody { bytes: Vec::new() }) })
    }
}

#[tokio::test]
async fn scanner_records_stable_objects_as_pending_export_candidates() {
    let object = object_head("photos", "2026/a.jpg", "etag-a", 42);
    let rustfs = rustfs_with_heads(vec![object.clone(), object.clone()]);
    let repository = Arc::new(MemoryRepository::default());
    let progress = ProgressAggregator::default();
    let scanner = ObjectScanner::new(rustfs, Arc::clone(&repository), progress.clone());

    let report = scanner
        .scan_all_buckets(ScanOptions {
            export_job_id: Some(Uuid::new_v4()),
            enqueue_stable_objects: true,
            record_source_changed_objects: true,
        })
        .await
        .expect("scan should succeed");

    assert_eq!(report.object_seen, 1);
    assert_eq!(report.stable_object_count, 1);
    assert_eq!(report.source_changed_count, 0);
    assert_eq!(report.total_bytes, 42);

    let snapshot = progress.snapshot();
    assert_eq!(snapshot.event_type, "SCAN_DONE");
    assert_eq!(snapshot.scan_phase, "DONE");
    assert_eq!(snapshot.stable_object_count, 1);

    let snapshots = repository.snapshots.lock().expect("snapshot lock poisoned");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].stable_status, StableStatus::Stable);
    assert_eq!(snapshots[0].object.last_modified.offset(), &Utc);

    let enqueued = repository.enqueued.lock().expect("enqueue lock poisoned");
    assert_eq!(enqueued.len(), 1);
    assert_eq!(enqueued[0].object_status, "PENDING");
    assert_eq!(enqueued[0].object.object_key, object.object_key);
}

#[tokio::test]
async fn scanner_records_source_changed_without_pending_export() {
    let first = object_head("photos", "2026/a.jpg", "etag-a", 42);
    let second = object_head("photos", "2026/a.jpg", "etag-b", 43);
    let rustfs = rustfs_with_heads(vec![first, second]);
    let repository = Arc::new(MemoryRepository::default());
    let progress = ProgressAggregator::default();
    let scanner = ObjectScanner::new(rustfs, Arc::clone(&repository), progress);

    let report = scanner
        .scan_all_buckets(ScanOptions {
            export_job_id: Some(Uuid::new_v4()),
            enqueue_stable_objects: true,
            record_source_changed_objects: true,
        })
        .await
        .expect("scan should succeed");

    assert_eq!(report.object_seen, 1);
    assert_eq!(report.stable_object_count, 0);
    assert_eq!(report.source_changed_count, 1);

    let snapshots = repository.snapshots.lock().expect("snapshot lock poisoned");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].stable_status, StableStatus::SourceChanged);

    let enqueued = repository.enqueued.lock().expect("enqueue lock poisoned");
    assert_eq!(enqueued.len(), 1);
    assert_eq!(enqueued[0].object_status, "SOURCE_CHANGED");
    assert_eq!(enqueued[0].error_code, Some("SOURCE_CHANGED"));
}

fn rustfs_with_heads(heads: Vec<ObjectHead>) -> MemoryRustFs {
    let bucket = heads[0].bucket.clone();
    let object_key = heads[0].object_key.clone();
    let mut objects = HashMap::new();
    objects.insert(
        bucket.clone(),
        vec![ObjectSummary {
            bucket: bucket.clone(),
            object_key: object_key.clone(),
        }],
    );

    let mut head_map = HashMap::new();
    head_map.insert((bucket.clone(), object_key), heads);

    MemoryRustFs {
        buckets: vec![bucket],
        objects,
        heads: Mutex::new(head_map),
    }
}

fn object_head(bucket: &str, object_key: &str, etag: &str, size_bytes: i64) -> ObjectHead {
    ObjectHead {
        bucket: bucket.to_owned(),
        object_key: object_key.to_owned(),
        etag: etag.to_owned(),
        size_bytes,
        last_modified: Utc.with_ymd_and_hms(2026, 8, 9, 8, 0, 0).unwrap(),
        metadata: BTreeMap::from([("content-type".to_owned(), "image/jpeg".to_owned())]),
    }
}
