use super::{ObjectHead, ScanError, StableStatus};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::{future::Future, pin::Pin, sync::Arc};
use uuid::Uuid;

pub type BoxRepoFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ScanError>> + Send + 'a>>;

pub trait ObjectSnapshotRepository: Send + Sync {
    fn save_snapshot<'a>(
        &'a self,
        object: &'a ObjectHead,
        stable_status: StableStatus,
        scanned_at: DateTime<Utc>,
    ) -> BoxRepoFuture<'a, ()>;

    fn enqueue_export_object<'a>(
        &'a self,
        export_job_id: Uuid,
        object: &'a ObjectHead,
        object_status: &'static str,
        error_code: Option<&'static str>,
        error_message: Option<&'static str>,
    ) -> BoxRepoFuture<'a, ()>;
}

#[derive(Debug, Clone)]
pub struct PgObjectSnapshotRepository {
    pool: PgPool,
}

impl PgObjectSnapshotRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl ObjectSnapshotRepository for PgObjectSnapshotRepository {
    fn save_snapshot<'a>(
        &'a self,
        object: &'a ObjectHead,
        stable_status: StableStatus,
        scanned_at: DateTime<Utc>,
    ) -> BoxRepoFuture<'a, ()> {
        Box::pin(async move {
            let metadata_json = serde_json::to_value(&object.metadata)
                .map_err(|err| ScanError::InvalidMetadata(err.to_string()))?;

            sqlx::query(
                r#"
                INSERT INTO local_object_snapshot (
                    bucket,
                    object_key,
                    etag,
                    size_bytes,
                    last_modified,
                    metadata_json,
                    scanned_at,
                    stable_status
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(&object.bucket)
            .bind(&object.object_key)
            .bind(&object.etag)
            .bind(object.size_bytes)
            .bind(object.last_modified.naive_utc())
            .bind(metadata_json)
            .bind(scanned_at.naive_utc())
            .bind(stable_status.as_db_value())
            .execute(&self.pool)
            .await?;

            Ok(())
        })
    }

    fn enqueue_export_object<'a>(
        &'a self,
        export_job_id: Uuid,
        object: &'a ObjectHead,
        object_status: &'static str,
        error_code: Option<&'static str>,
        error_message: Option<&'static str>,
    ) -> BoxRepoFuture<'a, ()> {
        Box::pin(async move {
            let error_code_value: Option<String> = error_code.map(str::to_owned);
            let error_message_value: Option<String> = error_message.map(str::to_owned);

            sqlx::query(
                r#"
                INSERT INTO export_object (
                    object_id,
                    export_job_id,
                    bucket,
                    object_key,
                    storage_mode,
                    etag,
                    size_bytes,
                    estimated_landing_bytes,
                    last_modified,
                    frame_total,
                    status,
                    error_code,
                    error_message
                )
                SELECT $1, $2, $3, $4, 'PACK', $5, $6, $7, $8, 0, $9, $10, $11
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM export_object AS exported
                    JOIN export_job AS exported_job
                      ON exported_job.export_job_id = exported.export_job_id
                    WHERE exported.bucket = $3
                      AND exported.object_key = $4
                      AND exported.etag = $5
                      AND exported.size_bytes = $6
                      AND exported.last_modified = $8
                      AND exported.status = 'EXPORTED'
                      AND exported_job.status = 'SEALED'
                )
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(uuid::Uuid::new_v4())
            .bind(export_job_id)
            .bind(&object.bucket)
            .bind(&object.object_key)
            .bind(&object.etag)
            .bind(object.size_bytes)
            .bind(object.size_bytes.saturating_add(4096))
            .bind(object.last_modified.naive_utc())
            .bind(object_status)
            .bind(error_code_value)
            .bind(error_message_value)
            .execute(&self.pool)
            .await?;

            Ok(())
        })
    }
}

impl<T> ObjectSnapshotRepository for Arc<T>
where
    T: ObjectSnapshotRepository + ?Sized,
{
    fn save_snapshot<'a>(
        &'a self,
        object: &'a ObjectHead,
        stable_status: StableStatus,
        scanned_at: DateTime<Utc>,
    ) -> BoxRepoFuture<'a, ()> {
        (**self).save_snapshot(object, stable_status, scanned_at)
    }

    fn enqueue_export_object<'a>(
        &'a self,
        export_job_id: Uuid,
        object: &'a ObjectHead,
        object_status: &'static str,
        error_code: Option<&'static str>,
        error_message: Option<&'static str>,
    ) -> BoxRepoFuture<'a, ()> {
        (**self).enqueue_export_object(
            export_job_id,
            object,
            object_status,
            error_code,
            error_message,
        )
    }
}
