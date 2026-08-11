use super::{
    ObjectHead, ObjectSnapshotRepository, ProgressAggregator, RustFsReadClient, ScanError,
    StableStatus,
};
use chrono::Utc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub export_job_id: Option<Uuid>,
    pub enqueue_stable_objects: bool,
    pub record_source_changed_objects: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            export_job_id: None,
            enqueue_stable_objects: false,
            record_source_changed_objects: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub bucket_count: u64,
    pub object_seen: u64,
    pub stable_object_count: u64,
    pub source_changed_count: u64,
    pub total_bytes: u64,
    pub reused_recent_scan: bool,
}

pub struct ObjectScanner<C, R> {
    rustfs: C,
    repository: R,
    progress: ProgressAggregator,
}

impl<C, R> ObjectScanner<C, R>
where
    C: RustFsReadClient,
    R: ObjectSnapshotRepository,
{
    pub fn new(rustfs: C, repository: R, progress: ProgressAggregator) -> Self {
        Self {
            rustfs,
            repository,
            progress,
        }
    }

    pub async fn scan_all_buckets(&self, options: ScanOptions) -> Result<ScanReport, ScanError> {
        let buckets = self.rustfs.list_buckets().await?;
        self.progress.start_scan(buckets.len() as u64);

        let mut report = ScanReport {
            bucket_count: buckets.len() as u64,
            ..ScanReport::default()
        };

        for bucket in buckets {
            self.progress.start_bucket(&bucket);
            let objects = self.rustfs.list_objects(&bucket).await?;

            for object in objects {
                self.progress
                    .observe_object(&object.bucket, &object.object_key);
                report.object_seen += 1;

                let before = self
                    .rustfs
                    .head_object(&object.bucket, &object.object_key)
                    .await?;
                let after = self
                    .rustfs
                    .head_object(&object.bucket, &object.object_key)
                    .await?;

                if before.has_same_identity(&after) {
                    self.record_stable(&options, &after).await?;
                    report.stable_object_count += 1;
                    report.total_bytes += after.size_bytes.max(0) as u64;
                    self.progress.record_stable_object(after.size_bytes);
                } else {
                    self.record_source_changed(&options, &after).await?;
                    report.source_changed_count += 1;
                    self.progress.record_source_changed();
                }
            }

            self.progress.finish_bucket();
        }

        self.progress.finish_scan();
        Ok(report)
    }

    async fn record_stable(
        &self,
        options: &ScanOptions,
        object: &ObjectHead,
    ) -> Result<(), ScanError> {
        let scanned_at = Utc::now();
        self.repository
            .save_snapshot(object, StableStatus::Stable, scanned_at)
            .await?;

        if options.enqueue_stable_objects {
            if let Some(export_job_id) = options.export_job_id {
                self.repository
                    .enqueue_export_object(export_job_id, object, "PENDING", None, None)
                    .await?;
            }
        }

        Ok(())
    }

    async fn record_source_changed(
        &self,
        options: &ScanOptions,
        object: &ObjectHead,
    ) -> Result<(), ScanError> {
        let scanned_at = Utc::now();
        self.repository
            .save_snapshot(object, StableStatus::SourceChanged, scanned_at)
            .await?;

        if options.record_source_changed_objects {
            if let Some(export_job_id) = options.export_job_id {
                self.repository
                    .enqueue_export_object(
                        export_job_id,
                        object,
                        "SOURCE_CHANGED",
                        Some("SOURCE_CHANGED"),
                        Some("source object changed between HEAD checks"),
                    )
                    .await?;
            }
        }

        Ok(())
    }
}
