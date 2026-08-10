use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

use anyhow::{anyhow, Context};
use aws_sdk_s3::{primitives::ByteStream, Client};
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{PgPool, Row};
use tokio::{runtime::Handle, task};
use uuid::Uuid;

use crate::{
    center_security::CenterSecurity,
    import_worker::{
        ArchiveStorage, ChunkPartRecord, ImportClaim, ImportError, ImportErrorCode, ImportJobStart,
        ImportOutcome, ImportProgressSnapshot, ImportRepository, ImportWorker, LedgerIdentity,
        LedgerRecord, ProgressAggregator,
    },
};

pub type CenterImportFuture<'a> =
    Pin<Box<dyn Future<Output = anyhow::Result<CenterImportResponse>> + Send + 'a>>;

pub trait CenterImportControlService: Send + Sync {
    fn import_disk<'a>(&'a self, request: CenterImportRequest) -> CenterImportFuture<'a>;
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CenterImportRequest {
    pub mount_path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CenterImportResponse {
    pub import_job_id: Option<Uuid>,
    pub import_job_status: String,
    pub outcome: String,
    pub progress: ImportProgressSnapshot,
    pub message: String,
}

#[derive(Clone)]
pub struct ProductionCenterImportControlService {
    pool: PgPool,
    s3_client: Client,
    security: CenterSecurity,
}

impl ProductionCenterImportControlService {
    pub fn new(pool: PgPool, s3_client: Client, security: CenterSecurity) -> Self {
        Self {
            pool,
            s3_client,
            security,
        }
    }
}

impl CenterImportControlService for ProductionCenterImportControlService {
    fn import_disk<'a>(&'a self, request: CenterImportRequest) -> CenterImportFuture<'a> {
        Box::pin(async move {
            let protocol_root = protocol_root(&request.mount_path)?;
            let pool = self.pool.clone();
            let s3_client = self.s3_client.clone();
            let security = self.security.clone();
            let signature_key = security.center_signature_key_bytes();
            let handle = Handle::current();

            task::spawn_blocking(move || {
                let mut repo = PgImportRepository::new(pool, handle.clone(), security);
                let mut storage = S3ArchiveStorage::new(s3_client, handle);
                let mut progress = ProgressAggregator::default();
                let outcome =
                    ImportWorker::new(&mut repo, &mut storage, &mut progress, signature_key)
                        .import_sealed_disk(&protocol_root);
                match outcome {
                    Ok(outcome) => {
                        let snapshot = progress.snapshot();
                        Ok(response_from_outcome(outcome, snapshot))
                    }
                    Err(err) => {
                        let snapshot = progress.snapshot();
                        Err(anyhow!("{}: {}", err.code.as_str(), err.message)
                            .context(format!("import_job_status={}", snapshot.import_job_status)))
                    }
                }
            })
            .await
            .context("join center ImportWorker")?
        })
    }
}

fn protocol_root(mount_path: &Path) -> anyhow::Result<PathBuf> {
    if mount_path.join("disk_info.json").exists() {
        return Ok(mount_path.to_path_buf());
    }
    let root = mount_path.join("rustfs-transfer");
    if root.join("disk_info.json").exists() {
        return Ok(root);
    }
    Err(anyhow!(
        "mount_path must point to a transport disk mount or rustfs-transfer protocol root"
    ))
}

fn response_from_outcome(
    outcome: ImportOutcome,
    progress: ImportProgressSnapshot,
) -> CenterImportResponse {
    match outcome {
        ImportOutcome::Imported { import_job_id } => CenterImportResponse {
            import_job_id: Some(import_job_id),
            import_job_status: "DONE".to_string(),
            outcome: "IMPORTED".to_string(),
            progress,
            message: "sealed disk imported".to_string(),
        },
        ImportOutcome::SkippedAlreadyDone { import_job_id } => CenterImportResponse {
            import_job_id: Some(import_job_id),
            import_job_status: "DONE".to_string(),
            outcome: "SKIPPED_ALREADY_DONE".to_string(),
            progress,
            message: "same disk_id + seal_id was already imported".to_string(),
        },
        ImportOutcome::AlreadyImporting { import_job_id } => CenterImportResponse {
            import_job_id: Some(import_job_id),
            import_job_status: "IMPORTING".to_string(),
            outcome: "ALREADY_IMPORTING".to_string(),
            progress,
            message: "same disk_id + seal_id is already importing".to_string(),
        },
    }
}

#[derive(Clone)]
struct PgImportRepository {
    pool: PgPool,
    handle: Handle,
    security: CenterSecurity,
}

impl PgImportRepository {
    fn new(pool: PgPool, handle: Handle, security: CenterSecurity) -> Self {
        Self {
            pool,
            handle,
            security,
        }
    }
}

impl ImportRepository for PgImportRepository {
    fn begin_import(&mut self, start: ImportJobStart) -> Result<ImportClaim, ImportError> {
        self.handle.block_on(async {
            let import_job_id = Uuid::new_v4();
            let result = sqlx::query(
                r#"
                INSERT INTO import_job(
                    import_job_id, disk_id, seal_id, export_job_id, manifest_sha256,
                    edge_code, status, object_count, imported_count, total_bytes,
                    imported_bytes, start_time
                )
                VALUES ($1, $2, $3, $4, $5, $6, 'IMPORTING', $7, 0, $8, 0, NOW() AT TIME ZONE 'UTC')
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(import_job_id)
            .bind(start.disk_id)
            .bind(start.seal_id)
            .bind(start.export_job_id)
            .bind(&start.manifest_sha256)
            .bind(&start.edge_code)
            .bind(start.object_count as i64)
            .bind(start.total_bytes as i64)
            .execute(&self.pool)
            .await
            .map_err(repo_err)?;

            if result.rows_affected() == 1 {
                return Ok(ImportClaim::Acquired { import_job_id });
            }
            existing_import_claim(&self.pool, &start).await
        })
    }

    fn complete_import(&mut self, import_job_id: Uuid, imported_count: u64, imported_bytes: u64) {
        let _ = self.handle.block_on(async {
            sqlx::query(
                "UPDATE import_job SET status = 'DONE', imported_count = $2, imported_bytes = $3, finish_time = NOW() AT TIME ZONE 'UTC' WHERE import_job_id = $1",
            )
            .bind(import_job_id)
            .bind(imported_count as i64)
            .bind(imported_bytes as i64)
            .execute(&self.pool)
            .await
        });
    }

    fn fail_import(&mut self, import_job_id: Uuid, code: ImportErrorCode, message: &str) {
        let _ = self.handle.block_on(async {
            sqlx::query(
                "UPDATE import_job SET status = 'FAILED', error_message = $2, finish_time = NOW() AT TIME ZONE 'UTC' WHERE import_job_id = $1",
            )
            .bind(import_job_id)
            .bind(format!("{}: {message}", code.as_str()))
            .execute(&self.pool)
            .await
        });
    }

    fn disk_registered(&self, disk_id: Uuid) -> bool {
        self.handle
            .block_on(async {
                sqlx::query_scalar::<_, bool>("SELECT status FROM disk_list WHERE disk_id = $1")
                    .bind(disk_id)
                    .fetch_optional(&self.pool)
                    .await
            })
            .ok()
            .flatten()
            .is_some()
    }

    fn disk_enabled(&self, disk_id: Uuid) -> bool {
        self.handle
            .block_on(async {
                sqlx::query_scalar::<_, bool>("SELECT status FROM disk_list WHERE disk_id = $1")
                    .bind(disk_id)
                    .fetch_optional(&self.pool)
                    .await
            })
            .ok()
            .flatten()
            .unwrap_or(false)
    }

    fn data_key(&self, disk_id: Uuid, data_key_id: Uuid) -> Option<Vec<u8>> {
        self.handle.block_on(async {
            let row = sqlx::query(
                r#"
                SELECT encrypted_key
                FROM data_key
                WHERE disk_id = $1
                  AND data_key_id = $2
                  AND status IN ('ACTIVE', 'ISSUED', 'SEALED_READONLY', 'RETIRED')
                "#,
            )
            .bind(disk_id)
            .bind(data_key_id)
            .fetch_optional(&self.pool)
            .await
            .ok()??;
            let encrypted_key: String = row.get("encrypted_key");
            self.security
                .unwrap_disk_data_key(disk_id, data_key_id, &encrypted_key)
                .ok()
                .map(Vec::from)
        })
    }

    fn identity_imported(&self, identity: &LedgerIdentity) -> bool {
        self.handle
            .block_on(async {
                let last_modified = parse_db_time(&identity.source_last_modified).ok()?;
                sqlx::query_scalar::<_, i64>(
                    r#"
                    SELECT COUNT(*)
                    FROM object_ledger
                    WHERE edge_code = $1
                      AND source_bucket = $2
                      AND source_key = $3
                      AND source_etag = $4
                      AND source_size_bytes = $5
                      AND source_last_modified = $6
                    "#,
                )
                .bind(&identity.edge_code)
                .bind(&identity.source_bucket)
                .bind(&identity.source_key)
                .bind(&identity.source_etag)
                .bind(identity.source_size_bytes as i64)
                .bind(last_modified)
                .fetch_one(&self.pool)
                .await
                .ok()
            })
            .unwrap_or_default()
            > 0
    }

    fn nonce_used(&self, data_key_id: Uuid, nonce: &str) -> bool {
        self.handle
            .block_on(async {
                sqlx::query_scalar::<_, i64>(
                    r#"
                    SELECT
                      (SELECT COUNT(*) FROM object_ledger WHERE data_key_id = $1 AND nonce = $2)
                      +
                      (SELECT COUNT(*) FROM chunk_import_part WHERE data_key_id = $1 AND nonce = $2)
                    "#,
                )
                .bind(data_key_id)
                .bind(nonce)
                .fetch_one(&self.pool)
                .await
                .ok()
            })
            .unwrap_or_default()
            > 0
    }

    fn insert_ledger(&mut self, record: LedgerRecord) {
        let _ = self.handle.block_on(async {
            let last_modified = parse_db_time(&record.identity.source_last_modified)?;
            sqlx::query(
                r#"
                INSERT INTO object_ledger(
                    edge_code, source_bucket, source_key, source_etag, source_size_bytes,
                    source_last_modified, plaintext_sha256, ciphertext_sha256, chunk_group_id,
                    data_key_id, nonce, import_bucket, import_key, export_job_id, import_job_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(&record.identity.edge_code)
            .bind(&record.identity.source_bucket)
            .bind(&record.identity.source_key)
            .bind(&record.identity.source_etag)
            .bind(record.identity.source_size_bytes as i64)
            .bind(last_modified)
            .bind(&record.plaintext_sha256)
            .bind(&record.ciphertext_sha256)
            .bind(record.chunk_group_id)
            .bind(record.data_key_id)
            .bind(&record.nonce)
            .bind(&record.import_bucket)
            .bind(&record.import_key)
            .bind(record.export_job_id)
            .bind(record.import_job_id)
            .execute(&self.pool)
            .await?;
            Ok::<_, anyhow::Error>(())
        });
    }

    fn register_chunk_part(&mut self, part: ChunkPartRecord) {
        let _ = self.handle.block_on(async {
            sqlx::query(
                r#"
                INSERT INTO chunk_import_part(
                    chunk_group_id, chunk_index, chunk_total, chunk_offset_bytes, chunk_size_bytes,
                    chunk_sha256, ciphertext_sha256, plaintext_sha256, data_key_id, nonce,
                    tag, aad, disk_id, seal_id, import_job_id, relative_data_path,
                    staged_ciphertext_path, staged_ciphertext_sha256, status, verified_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $6, $7, $8, $9, '', '', $10, $11, $12, '', '', $6, 'VERIFIED', NOW() AT TIME ZONE 'UTC')
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(part.chunk_group_id)
            .bind(part.chunk_index as i32)
            .bind(part.chunk_total as i32)
            .bind(part.chunk_offset_bytes as i64)
            .bind(part.chunk_size_bytes as i64)
            .bind(&part.ciphertext_sha256)
            .bind(sha256_hex(&part.plaintext))
            .bind(part.data_key_id)
            .bind(&part.nonce)
            .bind(part.disk_id)
            .bind(part.seal_id)
            .bind(part.import_job_id)
            .execute(&self.pool)
            .await
        });
    }

    fn chunk_parts(&self, _chunk_group_id: Uuid) -> Vec<ChunkPartRecord> {
        Vec::new()
    }
}

async fn existing_import_claim(
    pool: &PgPool,
    start: &ImportJobStart,
) -> Result<ImportClaim, ImportError> {
    let row = sqlx::query(
        r#"
        SELECT import_job_id, manifest_sha256, status
        FROM import_job
        WHERE disk_id = $1 AND seal_id = $2 AND status IN ('PENDING', 'IMPORTING', 'DONE')
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(start.disk_id)
    .bind(start.seal_id)
    .fetch_optional(pool)
    .await
    .map_err(repo_err)?
    .ok_or_else(|| ImportError {
        code: ImportErrorCode::ManifestInvalid,
        message: "import claim conflict but no active import_job was found".to_string(),
    })?;
    let manifest_sha256: String = row.get("manifest_sha256");
    if manifest_sha256 != start.manifest_sha256 {
        return Err(ImportError {
            code: ImportErrorCode::SealIdManifestMismatch,
            message: "same disk_id + seal_id has a different manifest sha256".to_string(),
        });
    }
    let import_job_id: Uuid = row.get("import_job_id");
    let status: String = row.get("status");
    if status == "DONE" {
        Ok(ImportClaim::AlreadyDone { import_job_id })
    } else {
        Ok(ImportClaim::AlreadyImporting { import_job_id })
    }
}

#[derive(Clone)]
struct S3ArchiveStorage {
    client: Client,
    handle: Handle,
}

impl S3ArchiveStorage {
    fn new(client: Client, handle: Handle) -> Self {
        Self { client, handle }
    }
}

impl ArchiveStorage for S3ArchiveStorage {
    fn ensure_bucket(&mut self, bucket: &str) -> Result<(), ImportError> {
        self.handle.block_on(async {
            if self
                .client
                .head_bucket()
                .bucket(bucket)
                .send()
                .await
                .is_err()
            {
                self.client
                    .create_bucket()
                    .bucket(bucket)
                    .send()
                    .await
                    .map_err(|err| ImportError {
                        code: ImportErrorCode::ManifestInvalid,
                        message: format!("failed to create archive bucket: {err}"),
                    })?;
            }
            Ok(())
        })
    }

    fn upload_object(&mut self, bucket: &str, key: &str, data: &[u8]) -> Result<(), ImportError> {
        self.handle.block_on(async {
            self.client
                .put_object()
                .bucket(bucket)
                .key(key)
                .body(ByteStream::from(data.to_vec()))
                .send()
                .await
                .map_err(|err| ImportError {
                    code: ImportErrorCode::ManifestInvalid,
                    message: format!("failed to upload archive object: {err}"),
                })?;
            Ok(())
        })
    }
}

fn repo_err(err: sqlx::Error) -> ImportError {
    ImportError {
        code: ImportErrorCode::ManifestInvalid,
        message: err.to_string(),
    }
}

fn parse_db_time(value: &str) -> anyhow::Result<NaiveDateTime> {
    Ok(DateTime::parse_from_rfc3339(value)?
        .with_timezone(&Utc)
        .naive_utc())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_worker::{ImportOutcome, ImportProgressSnapshot};

    #[test]
    fn import_response_uses_prefixed_status_field() {
        let import_job_id = Uuid::new_v4();
        let response = response_from_outcome(
            ImportOutcome::Imported { import_job_id },
            ImportProgressSnapshot::default(),
        );
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["import_job_status"], "DONE");
        assert!(value.get("status").is_none());
    }
}
