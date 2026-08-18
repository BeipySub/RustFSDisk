use std::{
    collections::BTreeMap,
    future::Future,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    pin::Pin,
};

use anyhow::{anyhow, Context};
use aws_sdk_s3::{primitives::DateTime as SmithyDateTime, Client};
use chrono::{DateTime, NaiveDateTime, Utc};
use rustfs_transfer_common::crypto::derive_disk_data_key_from_edge_key;
use sqlx::{PgPool, Row};
use tokio::{runtime::Handle, sync::RwLock, task};
use uuid::Uuid;

use crate::{
    config::EdgeConfig,
    disk_worker::{
        DiskWorker, DiskWorkerConfig, DiskWorkerError, ExportObjectRepository, ExportObjectTask,
        ExportedObjectUpdate, ObjectSource, SourceObjectHead,
    },
    progress::{ObjectInventorySnapshot, ProgressAggregator},
};

pub type ExportWorkerFuture<'a> =
    Pin<Box<dyn Future<Output = anyhow::Result<ExportWorkerReport>> + Send + 'a>>;

const MARK_DISK_RUNTIME_DONE_AFTER_SEAL_SQL: &str = r#"
    UPDATE disk_runtime
    SET status = 'DONE',
        last_error_code = NULL,
        error_message = NULL,
        last_seen_at = NOW() AT TIME ZONE 'UTC'
    WHERE disk_id = $1
    "#;

const LOCAL_OBJECT_INVENTORY_SQL: &str = r#"
    SELECT COUNT(*)::bigint AS total_count,
           COALESCE(SUM(size_bytes), 0)::bigint AS total_bytes
    FROM local_object_snapshot
    WHERE stable_status = 'STABLE'
    "#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportWorkerReport {
    pub disk_count: u64,
    pub worker_started_count: u64,
    pub worker_failed_count: u64,
}

pub trait ExportWorkerLauncher: Send + Sync {
    fn launch_workers<'a>(
        &'a self,
        export_job_id: Uuid,
        disk_ids: Vec<Uuid>,
    ) -> ExportWorkerFuture<'a>;
}

#[derive(Clone)]
pub struct ProductionExportWorkerLauncher {
    config: std::sync::Arc<EdgeConfig>,
    pool: PgPool,
    s3_client: Client,
    copy_progress: std::sync::Arc<RwLock<Option<ProgressAggregator>>>,
}

impl ProductionExportWorkerLauncher {
    pub fn new(
        config: std::sync::Arc<EdgeConfig>,
        pool: PgPool,
        s3_client: Client,
        copy_progress: std::sync::Arc<RwLock<Option<ProgressAggregator>>>,
    ) -> Self {
        Self {
            config,
            pool,
            s3_client,
            copy_progress,
        }
    }
}

impl ExportWorkerLauncher for ProductionExportWorkerLauncher {
    fn launch_workers<'a>(
        &'a self,
        export_job_id: Uuid,
        disk_ids: Vec<Uuid>,
    ) -> ExportWorkerFuture<'a> {
        Box::pin(async move {
            let disks = load_worker_disks(&self.pool, &disk_ids).await?;
            let progress = install_copy_progress(
                &self.copy_progress,
                &self.config.edge.edge_code,
                export_job_id,
            )
            .await;
            progress.set_object_inventory(load_object_inventory(&self.pool).await?);
            let handle = Handle::current();
            let mut tasks = Vec::new();

            for disk in disks {
                let disk_info = DiskInfoForExport::read_from_mount(&disk.mount_path)
                    .with_context(|| format!("read disk_info for disk {}", disk.disk_id))?;
                if disk_info.status_code != "INITIALIZED" {
                    anyhow::bail!(
                        "disk {} status_code {} is not eligible for export worker",
                        disk.disk_id,
                        disk_info.status_code
                    );
                }

                let seal_id = Uuid::new_v4();
                let key = derive_disk_data_key_from_edge_key(
                    &self.config.edge.edge_key,
                    &self.config.edge.edge_code,
                    disk.disk_id,
                    disk_info.data_key_id,
                    export_job_id,
                    seal_id,
                )
                .map_err(|err| anyhow!(err.to_string()))?;

                let config = DiskWorkerConfig {
                    disk_id: disk.disk_id,
                    disk_sn: disk.sn,
                    mount_path: disk.mount_path,
                    edge_code: self.config.edge.edge_code.clone(),
                    edge_name: self.config.edge.edge_code.clone(),
                    export_job_id,
                    seal_id,
                    data_key_id: disk_info.data_key_id,
                    disk_data_key: key,
                    free_bytes: disk.free_bytes,
                };
                let source = S3ObjectSource::new(self.s3_client.clone(), handle.clone());
                let repo = PgExportObjectRepository::new(self.pool.clone(), handle.clone());
                let progress = progress.clone();
                tasks.push(task::spawn_blocking(move || {
                    let worker = DiskWorker::new(config, &source, &repo, progress);
                    worker.run().map(|_| ())
                }));
            }

            let disk_count = tasks.len() as u64;
            let mut worker_failed_count = 0_u64;
            let mut failure_audit_lines = Vec::new();
            for task in tasks {
                match task.await.context("join export disk worker")? {
                    Ok(()) => {}
                    Err(err) => {
                        worker_failed_count += 1;
                        failure_audit_lines.push(export_failure_audit_line(&err));
                        tracing::error!(error_code = err.error_code(), "export disk worker failed");
                    }
                }
            }

            if worker_failed_count == 0 && disk_count > 0 {
                seal_export_job_if_complete(&self.pool, export_job_id).await?;
            } else if worker_failed_count > 0 {
                sqlx::query(
                    r#"
                UPDATE export_job
                SET status = 'FAILED',
                    finish_time = NOW() AT TIME ZONE 'UTC',
                    error_message = CONCAT_WS(E'\n', error_message, $2)
                WHERE export_job_id = $1
                "#,
                )
                .bind(export_job_id)
                .bind(export_failure_summary(&failure_audit_lines))
                .execute(&self.pool)
                .await?;
            }

            Ok(ExportWorkerReport {
                disk_count,
                worker_started_count: disk_count,
                worker_failed_count,
            })
        })
    }
}

async fn install_copy_progress(
    slot: &std::sync::Arc<RwLock<Option<ProgressAggregator>>>,
    edge_code: &str,
    export_job_id: Uuid,
) -> ProgressAggregator {
    let mut slot = slot.write().await;
    if let Some(progress) = slot.as_ref() {
        if progress.matches_export_job(&export_job_id.to_string()) {
            return progress.clone();
        }
    }

    let progress = ProgressAggregator::new(edge_code, export_job_id);
    *slot = Some(progress.clone());
    progress
}

async fn load_object_inventory(pool: &PgPool) -> anyhow::Result<ObjectInventorySnapshot> {
    let local = sqlx::query(LOCAL_OBJECT_INVENTORY_SQL)
        .fetch_one(pool)
        .await?;
    let exported = sqlx::query(
        r#"
        SELECT COUNT(*)::bigint AS exported_count,
               COALESCE(SUM(size_bytes), 0)::bigint AS exported_bytes
        FROM local_object_snapshot AS current_snapshot
        WHERE current_snapshot.stable_status = 'STABLE'
          AND EXISTS (
              SELECT 1
              FROM export_object AS exported
              JOIN export_job AS exported_job
                ON exported_job.export_job_id = exported.export_job_id
              WHERE exported.bucket = current_snapshot.bucket
                AND exported.object_key = current_snapshot.object_key
                AND exported.etag = current_snapshot.etag
                AND exported.size_bytes = current_snapshot.size_bytes
                AND exported.status = 'EXPORTED'
                AND exported_job.status = 'SEALED'
          )
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(ObjectInventorySnapshot {
        total_bytes: local.get::<i64, _>("total_bytes").max(0) as u64,
        exported_bytes: exported.get::<i64, _>("exported_bytes").max(0) as u64,
        total_count: local.get::<i64, _>("total_count").max(0) as u64,
        exported_count: exported.get::<i64, _>("exported_count").max(0) as u64,
    })
}

#[derive(Debug)]
struct WorkerDisk {
    disk_id: Uuid,
    sn: String,
    mount_path: PathBuf,
    free_bytes: u64,
}

async fn load_worker_disks(pool: &PgPool, disk_ids: &[Uuid]) -> anyhow::Result<Vec<WorkerDisk>> {
    if disk_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT ON (disk_id) disk_id, sn, mount_path, free_bytes
        FROM disk_runtime
        WHERE disk_id = ANY($1)
          AND status = 'READY'
          AND mount_path IS NOT NULL
        ORDER BY disk_id, id DESC
        "#,
    )
    .bind(disk_ids)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let mount_path: Option<String> = row.get("mount_path");
            Ok(WorkerDisk {
                disk_id: row.get("disk_id"),
                sn: row.get("sn"),
                mount_path: PathBuf::from(
                    mount_path.ok_or_else(|| anyhow!("READY disk missing mount_path"))?,
                ),
                free_bytes: row.get::<i64, _>("free_bytes").max(0) as u64,
            })
        })
        .collect()
}

#[derive(Debug)]
struct DiskInfoForExport {
    status_code: String,
    data_key_id: Uuid,
}

impl DiskInfoForExport {
    fn read_from_mount(mount_path: &Path) -> anyhow::Result<Self> {
        let path = mount_path.join("rustfs-transfer").join("disk_info.json");
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        let status_code = value
            .pointer("/status/code")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("disk_info.status.code is missing"))?
            .to_string();
        let data_key_id = value
            .pointer("/security/data_key_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("disk_info.security.data_key_id is missing"))
            .and_then(|value| Uuid::parse_str(value).map_err(Into::into))?;
        Ok(Self {
            status_code,
            data_key_id,
        })
    }
}

async fn seal_export_job_if_complete(pool: &PgPool, export_job_id: Uuid) -> anyhow::Result<()> {
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM export_object WHERE export_job_id = $1 AND status IN ('PENDING', 'ASSIGNED', 'COPYING')",
    )
    .bind(export_job_id)
    .fetch_one(pool)
    .await?;
    if remaining == 0 {
        sqlx::query(
            "UPDATE export_job SET status = 'SEALED', finish_time = NOW() AT TIME ZONE 'UTC' WHERE export_job_id = $1 AND status = 'COPYING'",
        )
        .bind(export_job_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

#[derive(Clone)]
struct S3ObjectSource {
    client: Client,
    handle: Handle,
}

impl S3ObjectSource {
    fn new(client: Client, handle: Handle) -> Self {
        Self { client, handle }
    }
}

impl ObjectSource for S3ObjectSource {
    fn head_object(&self, bucket: &str, key: &str) -> Result<SourceObjectHead, DiskWorkerError> {
        self.handle.block_on(async {
            let output = self
                .client
                .head_object()
                .bucket(bucket)
                .key(key)
                .send()
                .await
                .map_err(|err| DiskWorkerError::Io(std::io::Error::other(err.to_string())))?;
            let last_modified = output
                .last_modified()
                .ok_or_else(|| {
                    DiskWorkerError::ManifestInvalid("HEAD missing last_modified".to_string())
                })
                .and_then(smithy_time_to_utc)?;
            Ok(SourceObjectHead {
                etag: output.e_tag().unwrap_or_default().to_string(),
                size_bytes: output.content_length().unwrap_or_default().max(0) as u64,
                last_modified,
                content_type: output.content_type().map(str::to_string),
                metadata: output
                    .metadata()
                    .map(|metadata| {
                        metadata
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect::<BTreeMap<_, _>>()
                    })
                    .unwrap_or_default(),
            })
        })
    }

    fn open_object(
        &self,
        bucket: &str,
        key: &str,
        offset: u64,
        length: u64,
    ) -> Result<Box<dyn Read>, DiskWorkerError> {
        self.handle.block_on(async {
            let end = offset.saturating_add(length).saturating_sub(1);
            let output = self
                .client
                .get_object()
                .bucket(bucket)
                .key(key)
                .range(format!("bytes={offset}-{end}"))
                .send()
                .await
                .map_err(|err| DiskWorkerError::Io(std::io::Error::other(err.to_string())))?;
            let bytes = output
                .body
                .collect()
                .await
                .map_err(|err| DiskWorkerError::Io(std::io::Error::other(err.to_string())))?
                .into_bytes()
                .to_vec();
            Ok(Box::new(Cursor::new(bytes)) as Box<dyn Read>)
        })
    }
}

fn smithy_time_to_utc(value: &SmithyDateTime) -> Result<DateTime<Utc>, DiskWorkerError> {
    let system_time = std::time::SystemTime::try_from(*value)
        .map_err(|err| DiskWorkerError::ManifestInvalid(format!("invalid last_modified: {err}")))?;
    Ok(DateTime::<Utc>::from(system_time))
}

#[derive(Clone)]
struct PgExportObjectRepository {
    pool: PgPool,
    handle: Handle,
}

impl PgExportObjectRepository {
    fn new(pool: PgPool, handle: Handle) -> Self {
        Self { pool, handle }
    }
}

impl ExportObjectRepository for PgExportObjectRepository {
    fn assigned_objects(
        &self,
        export_job_id: Uuid,
        disk_id: Uuid,
    ) -> Result<Vec<ExportObjectTask>, DiskWorkerError> {
        self.handle.block_on(async {
            let rows = sqlx::query(
                r#"
                SELECT id, object_id, bucket, object_key, etag, size_bytes, last_modified
                FROM export_object
                WHERE export_job_id = $1 AND disk_id = $2 AND status = 'ASSIGNED'
                ORDER BY id ASC
                "#,
            )
            .bind(export_job_id)
            .bind(disk_id)
            .fetch_all(&self.pool)
            .await
            .map_err(sqlx_err)?;

            rows.into_iter()
                .map(|row| {
                    Ok(ExportObjectTask {
                        id: row.get("id"),
                        object_id: row.get("object_id"),
                        bucket: row.get("bucket"),
                        object_key: row.get("object_key"),
                        etag: row.get("etag"),
                        size_bytes: row.get::<i64, _>("size_bytes").max(0) as u64,
                        last_modified: naive_utc(row.get("last_modified")),
                    })
                })
                .collect()
        })
    }

    fn mark_copying(&self, object_id: i64, partial_path: &str) -> Result<(), DiskWorkerError> {
        self.handle.block_on(async {
            sqlx::query(
                "UPDATE export_object SET status = 'COPYING', partial_path = $2 WHERE id = $1 AND status = 'ASSIGNED'",
            )
            .bind(object_id)
            .bind(partial_path)
            .execute(&self.pool)
            .await
            .map_err(sqlx_err)?;
            Ok(())
        })
    }

    fn mark_exported(
        &self,
        object_id: i64,
        exported: &ExportedObjectUpdate,
    ) -> Result<(), DiskWorkerError> {
        self.handle.block_on(async {
            sqlx::query(
                r#"
                UPDATE export_object
                SET status = 'EXPORTED',
                    plaintext_sha256 = $2,
                    data_key_id = $3,
                    pack_path = $4,
                    pack_index_path = $5,
                    pack_offset_bytes = $6,
                    pack_ciphertext_size_bytes = $7,
                    pack_nonce = $8,
                    pack_tag = $9,
                    pack_aad = $10,
                    pack_ciphertext_sha256 = $11,
                    frame_total = 0,
                    relative_meta_path = $12,
                    partial_path = NULL,
                    error_code = NULL,
                    error_message = NULL
                WHERE id = $1
                "#,
            )
            .bind(object_id)
            .bind(&exported.plaintext_sha256)
            .bind(exported.data_key_id)
            .bind(&exported.pack_path)
            .bind(&exported.pack_index_path)
            .bind(exported.pack_offset_bytes as i64)
            .bind(exported.pack_ciphertext_size_bytes as i64)
            .bind(&exported.pack_nonce)
            .bind(&exported.pack_tag)
            .bind(&exported.pack_aad)
            .bind(&exported.pack_ciphertext_sha256)
            .bind(&exported.relative_meta_path)
            .execute(&self.pool)
            .await
            .map_err(sqlx_err)?;
            Ok(())
        })
    }

    fn mark_failed(
        &self,
        object_id: i64,
        error_code: &str,
        error_message: &str,
    ) -> Result<(), DiskWorkerError> {
        let object_status = if error_code == "SOURCE_CHANGED" {
            "SOURCE_CHANGED"
        } else {
            "FAILED"
        };
        self.handle.block_on(async {
            sqlx::query(
                "UPDATE export_object SET status = $2, error_code = $3, error_message = $4 WHERE id = $1",
            )
            .bind(object_id)
            .bind(object_status)
            .bind(error_code)
            .bind(error_message)
            .execute(&self.pool)
            .await
            .map_err(sqlx_err)?;
            Ok(())
        })
    }

    fn load_exported_objects(
        &self,
        export_job_id: Uuid,
        disk_id: Uuid,
    ) -> Result<Vec<ExportedObjectUpdate>, DiskWorkerError> {
        self.handle.block_on(async {
            let rows = sqlx::query(
                r#"
                SELECT id, object_id, bucket, object_key, storage_mode, relative_meta_path,
                       plaintext_sha256, data_key_id, pack_path, pack_index_path,
                       pack_offset_bytes, pack_ciphertext_size_bytes, pack_nonce, pack_tag,
                       pack_aad, pack_ciphertext_sha256, frame_total, estimated_landing_bytes,
                       size_bytes, etag, last_modified, status
                FROM export_object
                WHERE export_job_id = $1 AND disk_id = $2 AND status = 'EXPORTED'
                ORDER BY id ASC
                "#,
            )
            .bind(export_job_id)
            .bind(disk_id)
            .fetch_all(&self.pool)
            .await
            .map_err(sqlx_err)?;

            rows.into_iter()
                .map(|row| {
                    Ok(ExportedObjectUpdate {
                        row_id: row.get("id"),
                        object_id: row.get("object_id"),
                        bucket: row.get("bucket"),
                        key: row.get("object_key"),
                        storage_mode: match row.get::<String, _>("storage_mode").as_str() {
                            "FRAMES" => rustfs_transfer_common::protocol::StorageMode::Frames,
                            _ => rustfs_transfer_common::protocol::StorageMode::Pack,
                        },
                        relative_meta_path: row.get("relative_meta_path"),
                        plaintext_sha256: row.get("plaintext_sha256"),
                        data_key_id: row.get("data_key_id"),
                        pack_path: row.get("pack_path"),
                        pack_index_path: row.get("pack_index_path"),
                        pack_offset_bytes: row.get::<i64, _>("pack_offset_bytes").max(0) as u64,
                        pack_ciphertext_size_bytes: row
                            .get::<i64, _>("pack_ciphertext_size_bytes")
                            .max(0) as u64,
                        pack_nonce: row.get("pack_nonce"),
                        pack_tag: row.get("pack_tag"),
                        pack_aad: row.get("pack_aad"),
                        pack_ciphertext_sha256: row.get("pack_ciphertext_sha256"),
                        frame_total: row.get::<i32, _>("frame_total").max(0) as u32,
                        estimated_landing_bytes: row.get::<i64, _>("estimated_landing_bytes").max(0)
                            as u64,
                        size_bytes: row.get::<i64, _>("size_bytes").max(0) as u64,
                        etag: row.get("etag"),
                        last_modified: naive_utc(row.get("last_modified")),
                        content_type: None,
                        metadata: BTreeMap::new(),
                        exported_at: Utc::now(),
                        object_status: row.get("status"),
                    })
                })
                .collect()
        })
    }

    fn mark_disk_runtime(
        &self,
        disk_id: Uuid,
        runtime_status: &str,
        error_code: Option<&str>,
    ) -> Result<(), DiskWorkerError> {
        self.handle.block_on(async {
            sqlx::query(
                "UPDATE disk_runtime SET status = $2, last_error_code = $3, last_seen_at = NOW() AT TIME ZONE 'UTC' WHERE disk_id = $1",
            )
            .bind(disk_id)
            .bind(runtime_status)
            .bind(error_code)
            .execute(&self.pool)
            .await
            .map_err(sqlx_err)?;
            Ok(())
        })
    }

    fn mark_disk_runtime_done_after_seal(&self, disk_id: Uuid) -> Result<(), DiskWorkerError> {
        self.handle.block_on(async {
            let result = sqlx::query(MARK_DISK_RUNTIME_DONE_AFTER_SEAL_SQL)
                .bind(disk_id)
                .execute(&self.pool)
                .await
                .map_err(sqlx_err)?;
            tracing::info!(
                disk_id = %disk_id,
                updated_runtime_rows = result.rows_affected(),
                "marked edge disk runtime DONE after successful seal"
            );
            Ok(())
        })
    }

    fn mark_job_sealed_checkpoint(
        &self,
        export_job_id: Uuid,
        _copied_count: u64,
        _copied_bytes: u64,
    ) -> Result<(), DiskWorkerError> {
        self.handle.block_on(async {
            sqlx::query(
                r#"
                UPDATE export_job
                SET copied_count = (
                        SELECT COUNT(*)
                        FROM export_object
                        WHERE export_job_id = $1
                          AND status = 'EXPORTED'
                    ),
                    copied_bytes = COALESCE((
                        SELECT SUM(size_bytes)
                        FROM export_object
                        WHERE export_job_id = $1
                          AND status = 'EXPORTED'
                    ), 0)
                WHERE export_job_id = $1
                "#,
            )
            .bind(export_job_id)
            .execute(&self.pool)
            .await
            .map_err(sqlx_err)?;
            Ok(())
        })
    }
}

fn naive_utc(value: NaiveDateTime) -> DateTime<Utc> {
    DateTime::from_naive_utc_and_offset(value, Utc)
}

fn sqlx_err(err: sqlx::Error) -> DiskWorkerError {
    DiskWorkerError::Io(std::io::Error::other(err.to_string()))
}

fn export_failure_summary(failure_audit_lines: &[String]) -> String {
    let mut lines = vec![
        "one or more DiskWorker instances failed; disk was not sealed for failed workers"
            .to_string(),
    ];
    lines.extend(failure_audit_lines.iter().cloned());
    lines.join("\n")
}

fn export_failure_audit_line(err: &DiskWorkerError) -> String {
    format!(
        "export_failure_code={}; export_failure_stage={}; worker_error_code={}; worker_error_message={}",
        export_failure_code(err),
        export_failure_stage(err),
        err.error_code(),
        sanitize_audit_value(&err.to_string())
    )
}

fn export_failure_code(err: &DiskWorkerError) -> &'static str {
    if is_permission_denied_error(err) {
        return "WRITE_BEFORE_PERMISSION_DENIED";
    }
    err.error_code()
}

fn export_failure_stage(err: &DiskWorkerError) -> &'static str {
    match err {
        DiskWorkerError::PartialCleanFailed(_) => "PARTIAL_RECOVERY",
        DiskWorkerError::DiskFull(_)
        | DiskWorkerError::DiskRemoved(_)
        | DiskWorkerError::SourceChanged(_)
        | DiskWorkerError::ChecksumMismatch(_)
        | DiskWorkerError::Crypto(_) => "OBJECT_WRITE",
        DiskWorkerError::ManifestInvalid(_) | DiskWorkerError::Io(_) | DiskWorkerError::Json(_) => {
            if is_permission_denied_error(err) {
                "WRITE_BEFORE"
            } else {
                "UNKNOWN"
            }
        }
    }
}

fn is_permission_denied_error(err: &DiskWorkerError) -> bool {
    let message = err.to_string().to_ascii_lowercase();
    message.contains("permission denied")
        || message.contains("access is denied")
        || message.contains("access denied")
        || message.contains("os error 13")
}

fn sanitize_audit_value(value: &str) -> String {
    value.replace(['\n', '\r', ';'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[tokio::test]
    async fn installed_copy_progress_is_the_snapshot_shared_with_websocket() {
        let slot = std::sync::Arc::new(RwLock::new(None));
        let export_job_id = Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();
        let worker_progress = install_copy_progress(&slot, "edge-a", export_job_id).await;
        worker_progress.set_object_inventory(ObjectInventorySnapshot {
            total_bytes: 500,
            exported_bytes: 0,
            total_count: 3,
            exported_count: 0,
        });

        worker_progress.register_disk(
            "disk-a",
            "presence-a",
            "sn-a",
            "/media/edge/disk-a",
            200,
            100,
            1,
            50,
        );
        worker_progress.set_disk_filesystem_type("disk-a", Some("ext4".to_string()));
        let reused_worker_progress = install_copy_progress(&slot, "edge-a", export_job_id).await;
        assert!(reused_worker_progress.matches_export_job(&export_job_id.to_string()));
        worker_progress.start_object(
            "disk-a",
            Uuid::new_v4().to_string(),
            "source",
            "objects/a.bin",
            "PACK",
            0,
            0,
            100,
        );
        worker_progress.add_bytes("disk-a", 100);
        worker_progress.complete_object("disk-a");

        let websocket_snapshot = slot
            .read()
            .await
            .as_ref()
            .expect("copy progress must be installed for websocket snapshots")
            .snapshot("COPY_PROGRESS", "edge copy progress snapshot");

        assert_eq!(
            websocket_snapshot.export_job.unwrap().export_job_id,
            export_job_id.to_string()
        );
        assert_eq!(websocket_snapshot.disks[0].done_bytes, 100);
        assert_eq!(websocket_snapshot.disks[0].disk_presence_id, "presence-a");
        assert_eq!(websocket_snapshot.disks[0].capacity_bytes, 200);
        assert_eq!(
            websocket_snapshot.disks[0].filesystem_type.as_deref(),
            Some("ext4")
        );
        assert_eq!(websocket_snapshot.object_inventory.total_count, 3);
        assert_eq!(websocket_snapshot.object_inventory.total_bytes, 500);
        assert_eq!(websocket_snapshot.object_inventory.exported_count, 1);
        assert_eq!(websocket_snapshot.object_inventory.exported_bytes, 100);
        assert_eq!(
            websocket_snapshot.disks[0].current_file.as_deref(),
            Some("a.bin")
        );
    }

    #[test]
    fn derives_offline_disk_data_key_from_fixed_hmac_message() {
        let disk_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let data_key_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        let export_job_id = Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();
        let seal_id = Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap();

        let key = derive_disk_data_key_from_edge_key(
            "edge-secret",
            "edge-a",
            disk_id,
            data_key_id,
            export_job_id,
            seal_id,
        )
        .unwrap();

        assert_eq!(key.len(), 32);
        assert_eq!(
            hex::encode(key),
            "58961baf139e4fcf657dbc1ffff395aacb9986f1b5e864ec90d5016b51fe5775"
        );
    }

    #[test]
    fn rejects_missing_edge_key_for_offline_export() {
        let err = derive_disk_data_key_from_edge_key(
            " ",
            "edge-a",
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("edge_key"));
    }

    #[test]
    fn failure_audit_marks_manifest_permission_denied_as_write_before() {
        let err = DiskWorkerError::ManifestInvalid(
            "io error: Permission denied while creating rustfs-transfer/manifests".to_string(),
        );

        assert_eq!(
            export_failure_audit_line(&err),
            "export_failure_code=WRITE_BEFORE_PERMISSION_DENIED; export_failure_stage=WRITE_BEFORE; worker_error_code=MANIFEST_INVALID; worker_error_message=manifest is invalid: io error: Permission denied while creating rustfs-transfer/manifests"
        );
    }

    #[test]
    fn failure_audit_keeps_generic_summary_but_includes_machine_markers() {
        let err = DiskWorkerError::Io(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Access is denied",
        ));
        let line = export_failure_audit_line(&err);
        let summary = export_failure_summary(&[line]);

        assert!(summary.contains("one or more DiskWorker instances failed"));
        assert!(summary.contains("export_failure_code=WRITE_BEFORE_PERMISSION_DENIED"));
        assert!(summary.contains("export_failure_stage=WRITE_BEFORE"));
        assert!(summary.contains("worker_error_code=MANIFEST_INVALID"));
    }

    #[test]
    fn seal_success_runtime_marks_done_without_deleting_summary_source() {
        assert!(MARK_DISK_RUNTIME_DONE_AFTER_SEAL_SQL.contains("UPDATE disk_runtime"));
        assert!(MARK_DISK_RUNTIME_DONE_AFTER_SEAL_SQL.contains("status = 'DONE'"));
        assert!(!MARK_DISK_RUNTIME_DONE_AFTER_SEAL_SQL.contains("DELETE FROM disk_runtime"));
        assert!(!MARK_DISK_RUNTIME_DONE_AFTER_SEAL_SQL.contains("export_job"));
        assert!(!MARK_DISK_RUNTIME_DONE_AFTER_SEAL_SQL.contains("export_object"));
        assert!(!MARK_DISK_RUNTIME_DONE_AFTER_SEAL_SQL.contains("manifest"));
    }

    #[test]
    fn copy_progress_inventory_uses_current_stable_snapshot() {
        assert!(LOCAL_OBJECT_INVENTORY_SQL.contains("FROM local_object_snapshot"));
        assert!(LOCAL_OBJECT_INVENTORY_SQL.contains("stable_status = 'STABLE'"));
        assert!(!LOCAL_OBJECT_INVENTORY_SQL.contains("latest_scan"));
    }
}
