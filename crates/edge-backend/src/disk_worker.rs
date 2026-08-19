use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use aes_gcm::aead::{AeadInPlace, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::{DateTime, Utc};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::progress::ProgressAggregator;
use rustfs_transfer_common::crypto::{pack_object_aad, PackObjectAad};
use rustfs_transfer_common::protocol::{StorageMode, MANIFEST_VERSION};

const PROTOCOL_ROOT: &str = "rustfs-transfer";
const MANIFEST_PATH: &str = "manifests/export_manifest.json";
const MANIFEST_SHA256_PATH: &str = "manifests/export_manifest.sha256";
const ENCRYPTION_ALG: &str = "AES-256-GCM";
const COPY_BUFFER_BYTES: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum DiskWorkerError {
    #[error("manifest is invalid: {0}")]
    ManifestInvalid(String),
    #[error("checksum mismatch: {0}")]
    ChecksumMismatch(String),
    #[error("disk is full: {0}")]
    DiskFull(String),
    #[error("disk was removed: {0}")]
    DiskRemoved(String),
    #[error("partial cleanup failed: {0}")]
    PartialCleanFailed(String),
    #[error("source object changed: {0}")]
    SourceChanged(String),
    #[error("S3 operation failed: {0}")]
    S3OperationFailed(String),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl DiskWorkerError {
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::ManifestInvalid(_) => "MANIFEST_INVALID",
            Self::ChecksumMismatch(_) => "CHECKSUM_MISMATCH",
            Self::DiskFull(_) => "DISK_FULL",
            Self::DiskRemoved(_) => "DISK_REMOVED",
            Self::PartialCleanFailed(_) => "PARTIAL_CLEAN_FAILED",
            Self::SourceChanged(_) => "SOURCE_CHANGED",
            Self::S3OperationFailed(_) => "S3_OPERATION_FAILED",
            Self::Crypto(_) => "DECRYPT_FAILED",
            Self::Io(_) | Self::Json(_) => "MANIFEST_INVALID",
        }
    }
}

pub type Result<T> = std::result::Result<T, DiskWorkerError>;

#[derive(Debug, Clone)]
pub struct DiskWorkerConfig {
    pub disk_id: Uuid,
    pub disk_sn: String,
    pub mount_path: PathBuf,
    pub edge_code: String,
    pub edge_name: String,
    pub export_job_id: Uuid,
    pub seal_id: Uuid,
    pub data_key_id: Uuid,
    pub disk_data_key: [u8; 32],
    pub free_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceObjectHead {
    pub etag: String,
    pub size_bytes: u64,
    pub last_modified: DateTime<Utc>,
    pub content_type: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

pub trait ObjectSource {
    fn head_object(&self, bucket: &str, key: &str) -> Result<SourceObjectHead>;
    fn open_object(
        &self,
        bucket: &str,
        key: &str,
        offset: u64,
        length: u64,
    ) -> Result<Box<dyn Read>>;
}

pub trait ExportObjectRepository {
    fn assigned_objects(&self, export_job_id: Uuid, disk_id: Uuid)
        -> Result<Vec<ExportObjectTask>>;
    fn mark_copying(&self, object_id: i64, partial_path: &str) -> Result<()>;
    fn mark_exported(&self, object_id: i64, exported: &ExportedObjectUpdate) -> Result<()>;
    fn mark_failed(&self, object_id: i64, error_code: &str, error_message: &str) -> Result<()>;
    fn load_exported_objects(
        &self,
        export_job_id: Uuid,
        disk_id: Uuid,
    ) -> Result<Vec<ExportedObjectUpdate>>;
    fn mark_disk_runtime(
        &self,
        disk_id: Uuid,
        runtime_status: &str,
        error_code: Option<&str>,
    ) -> Result<()>;
    fn mark_disk_runtime_done_after_seal(&self, disk_id: Uuid) -> Result<()>;
    fn mark_job_sealed_checkpoint(
        &self,
        export_job_id: Uuid,
        copied_count: u64,
        copied_bytes: u64,
    ) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct ExportObjectTask {
    pub id: i64,
    pub object_id: Uuid,
    pub bucket: String,
    pub object_key: String,
    pub etag: String,
    pub size_bytes: u64,
    pub last_modified: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedObjectUpdate {
    pub row_id: i64,
    pub object_id: Uuid,
    pub bucket: String,
    pub key: String,
    pub storage_mode: StorageMode,
    pub relative_meta_path: String,
    pub plaintext_sha256: String,
    pub data_key_id: Uuid,
    pub pack_path: String,
    pub pack_index_path: String,
    pub pack_offset_bytes: u64,
    pub pack_ciphertext_size_bytes: u64,
    pub pack_nonce: String,
    pub pack_tag: String,
    pub pack_aad: String,
    pub pack_ciphertext_sha256: String,
    pub frame_total: u32,
    pub estimated_landing_bytes: u64,
    pub size_bytes: u64,
    pub etag: String,
    pub last_modified: DateTime<Utc>,
    pub content_type: Option<String>,
    pub metadata: BTreeMap<String, String>,
    pub exported_at: DateTime<Utc>,
    pub object_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    pub manifest_version: String,
    pub seal_id: Uuid,
    pub export_job_id: Uuid,
    pub disk_id: Uuid,
    pub edge_code: String,
    pub create_time: DateTime<Utc>,
    pub objects: Vec<ManifestObject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestObject {
    pub object_id: Uuid,
    pub bucket: String,
    pub key: String,
    pub storage_mode: StorageMode,
    pub data_key_id: Uuid,
    pub pack_ref: ManifestPackRef,
    pub frames: Vec<ManifestFrameRef>,
    pub frame_total: u32,
    pub relative_meta_path: String,
    pub size_bytes: u64,
    pub estimated_landing_bytes: u64,
    pub etag: String,
    pub last_modified: DateTime<Utc>,
    pub content_type: Option<String>,
    pub metadata: BTreeMap<String, String>,
    pub plaintext_sha256: String,
    pub exported_at: DateTime<Utc>,
    pub object_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestPackRef {
    pub pack_path: String,
    pub pack_index_path: String,
    pub pack_offset_bytes: u64,
    pub ciphertext_size_bytes: u64,
    pub nonce: String,
    pub tag: String,
    pub aad: String,
    pub ciphertext_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFrameRef {
    pub frame_index: u32,
    pub frame_total: u32,
    pub frame_offset_bytes: u64,
    pub frame_size_bytes: u64,
    pub relative_frame_path: String,
    pub ciphertext_size_bytes: u64,
    pub nonce: String,
    pub tag: String,
    pub aad: String,
    pub ciphertext_sha256: String,
}

pub struct DiskWorker<'a, S, R> {
    config: DiskWorkerConfig,
    source: &'a S,
    repository: &'a R,
    progress: ProgressAggregator,
}

impl<'a, S, R> DiskWorker<'a, S, R>
where
    S: ObjectSource,
    R: ExportObjectRepository,
{
    pub fn new(
        config: DiskWorkerConfig,
        source: &'a S,
        repository: &'a R,
        progress: ProgressAggregator,
    ) -> Self {
        Self {
            config,
            source,
            repository,
            progress,
        }
    }

    pub fn run(&self) -> Result<ExportManifest> {
        self.begin_copying()?;

        let objects = self
            .repository
            .assigned_objects(self.config.export_job_id, self.config.disk_id)?;
        let total_bytes = objects.iter().map(|object| object.size_bytes).sum();
        self.progress.register_disk(
            self.config.disk_id.to_string(),
            "",
            self.config.disk_sn.clone(),
            self.config.mount_path.display().to_string(),
            0,
            total_bytes,
            objects.len() as u64,
            self.config.free_bytes,
        );

        for object in objects {
            if let Err(err) = self.export_one_object(&object) {
                self.repository
                    .mark_failed(object.id, err.error_code(), &err.to_string())?;
                if matches!(
                    err,
                    DiskWorkerError::DiskFull(_)
                        | DiskWorkerError::DiskRemoved(_)
                        | DiskWorkerError::PartialCleanFailed(_)
                ) {
                    self.progress
                        .fail_disk(&self.config.disk_id.to_string(), err.error_code());
                    self.repository.mark_disk_runtime(
                        self.config.disk_id,
                        "ERROR",
                        Some(err.error_code()),
                    )?;
                    return Err(err);
                }
            }
        }

        self.seal()
    }

    pub fn begin_copying(&self) -> Result<()> {
        self.ensure_protocol_dirs()?;
        self.cleanup_or_quarantine_partials()?;
        self.repository
            .mark_disk_runtime(self.config.disk_id, "COPYING", None)?;
        self.mark_disk_info_edge_copying()
    }

    pub fn register_single_disk_progress(&self, total_bytes: u64, object_total: u64) {
        self.progress.register_disk(
            self.config.disk_id.to_string(),
            "",
            self.config.disk_sn.clone(),
            self.config.mount_path.display().to_string(),
            0,
            total_bytes,
            object_total,
            self.config.free_bytes,
        );
    }

    pub fn config_export_job_id(&self) -> Uuid {
        self.config.export_job_id
    }

    pub fn config_disk_id(&self) -> Uuid {
        self.config.disk_id
    }

    pub fn config_free_bytes(&self) -> u64 {
        self.config.free_bytes
    }

    pub fn export_one_object(&self, object: &ExportObjectTask) -> Result<()> {
        let object_id = object.object_id;
        let pack_path = data_path(&self.config.export_job_id, object.id);
        let pack_index_path = pack_index_path(&self.config.export_job_id, object.id);
        let partial_path = format!("{pack_path}.partial");
        validate_relative_path(&pack_path, "packs/")?;
        validate_relative_path(&pack_index_path, "packs/")?;
        validate_relative_path(&partial_path, "packs/")?;

        self.progress.start_object(
            &self.config.disk_id.to_string(),
            &object.object_id.to_string(),
            &object.bucket,
            &object.object_key,
            StorageMode::Pack.as_str(),
            0,
            0,
            object.size_bytes,
        );
        self.repository.mark_copying(object.id, &partial_path)?;

        let before = self
            .source
            .head_object(&object.bucket, &object.object_key)?;
        ensure_head_matches_task(object, &before)?;

        let mut reader =
            self.source
                .open_object(&object.bucket, &object.object_key, 0, object.size_bytes)?;
        let mut plaintext =
            Vec::with_capacity(object.size_bytes.min(COPY_BUFFER_BYTES as u64) as usize);
        let mut plaintext_hasher = Sha256::new();
        let mut remaining = object.size_bytes;
        let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
        while remaining > 0 {
            let max_read = remaining.min(buffer.len() as u64) as usize;
            let read = reader
                .read(&mut buffer[..max_read])
                .map_err(classify_io_error)?;
            if read == 0 {
                return Err(DiskWorkerError::ChecksumMismatch(format!(
                    "source ended before {} bytes",
                    object.size_bytes
                )));
            }
            plaintext.extend_from_slice(&buffer[..read]);
            plaintext_hasher.update(&buffer[..read]);
            remaining -= read as u64;
            self.progress
                .add_bytes(&self.config.disk_id.to_string(), read as u64);
        }
        let plaintext_sha256 = hex::encode(plaintext_hasher.finalize());

        let after = self
            .source
            .head_object(&object.bucket, &object.object_key)?;
        if before != after {
            return Err(DiskWorkerError::SourceChanged(format!(
                "{}/{} changed during export",
                object.bucket, object.object_key
            )));
        }

        let mut nonce_bytes = [0_u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = BASE64.encode(nonce_bytes);
        let aad = build_pack_aad(
            &self.config,
            object,
            &object_id,
            &pack_path,
            &plaintext_sha256,
        );
        let mut ciphertext = plaintext;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.config.disk_data_key));
        let tag = cipher
            .encrypt_in_place_detached(
                Nonce::from_slice(&nonce_bytes),
                aad.as_bytes(),
                &mut ciphertext,
            )
            .map_err(|_| DiskWorkerError::Crypto("AES-256-GCM encryption failed".to_string()))?;
        let tag = BASE64.encode(tag);
        let ciphertext_sha256 = sha256_hex(&ciphertext);

        atomic_write_bytes(&self.root().join(&partial_path), &ciphertext)?;
        let final_path = self.root().join(&pack_path);
        fs::rename(self.root().join(&partial_path), &final_path).map_err(classify_io_error)?;
        fsync_file(&final_path)?;
        fsync_parent(&final_path)?;
        let pack_index = json!({
            "object_id": object_id,
            "pack_path": pack_path,
            "pack_offset_bytes": 0,
            "ciphertext_size_bytes": ciphertext.len(),
            "ciphertext_sha256": ciphertext_sha256,
        });
        atomic_write_json(&self.root().join(&pack_index_path), &pack_index)?;

        let relative_meta_path = meta_path(&self.config.export_job_id, object.id);
        validate_relative_path(&relative_meta_path, "meta/")?;
        let metadata_json = json!({
            "bucket": object.bucket,
            "key": object.object_key,
            "etag": object.etag,
            "size_bytes": object.size_bytes,
            "last_modified": object.last_modified,
            "content_type": after.content_type,
            "metadata": after.metadata,
        });
        atomic_write_json(&self.root().join(&relative_meta_path), &metadata_json)?;

        let exported = ExportedObjectUpdate {
            row_id: object.id,
            object_id,
            bucket: object.bucket.clone(),
            key: object.object_key.clone(),
            storage_mode: StorageMode::Pack,
            relative_meta_path,
            plaintext_sha256,
            data_key_id: self.config.data_key_id,
            pack_path,
            pack_index_path,
            pack_offset_bytes: 0,
            pack_ciphertext_size_bytes: ciphertext.len() as u64,
            pack_nonce: nonce,
            pack_tag: tag,
            pack_aad: aad,
            pack_ciphertext_sha256: ciphertext_sha256,
            frame_total: 0,
            estimated_landing_bytes: object.size_bytes.saturating_add(4096),
            size_bytes: object.size_bytes,
            etag: object.etag.clone(),
            last_modified: object.last_modified,
            content_type: after.content_type,
            metadata: after.metadata,
            exported_at: Utc::now(),
            object_status: "EXPORTED".to_string(),
        };
        self.repository.mark_exported(object.id, &exported)?;
        self.progress
            .complete_object(&self.config.disk_id.to_string());
        Ok(())
    }

    pub fn seal(&self) -> Result<ExportManifest> {
        self.cleanup_or_quarantine_partials()?;
        let exported = self
            .repository
            .load_exported_objects(self.config.export_job_id, self.config.disk_id)?;
        let objects: Vec<ManifestObject> = exported
            .into_iter()
            .filter(|object| object.object_status == "EXPORTED")
            .map(ManifestObject::from)
            .collect();
        for object in &objects {
            validate_relative_path(&object.pack_ref.pack_path, "packs/")?;
            validate_relative_path(&object.pack_ref.pack_index_path, "packs/")?;
            validate_relative_path(&object.relative_meta_path, "meta/")?;
        }

        let manifest = ExportManifest {
            manifest_version: MANIFEST_VERSION.to_string(),
            seal_id: self.config.seal_id,
            export_job_id: self.config.export_job_id,
            disk_id: self.config.disk_id,
            edge_code: self.config.edge_code.clone(),
            create_time: Utc::now(),
            objects,
        };
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
        let manifest_sha256 = sha256_hex(&manifest_bytes);
        atomic_write_bytes(&self.root().join(MANIFEST_PATH), &manifest_bytes)?;
        atomic_write_bytes(
            &self.root().join(MANIFEST_SHA256_PATH),
            manifest_sha256.as_bytes(),
        )?;
        self.mark_disk_info_sealed(
            manifest.objects.len() as u64,
            total_manifest_bytes(&manifest),
            &manifest_sha256,
        )?;
        self.repository.mark_job_sealed_checkpoint(
            self.config.export_job_id,
            manifest.objects.len() as u64,
            total_manifest_bytes(&manifest),
        )?;
        self.repository
            .mark_disk_runtime_done_after_seal(self.config.disk_id)?;
        self.progress
            .mark_disk_done(&self.config.disk_id.to_string());
        Ok(manifest)
    }

    fn mark_disk_info_edge_copying(&self) -> Result<()> {
        let mut disk_info = self.read_disk_info()?;
        set_json_path(&mut disk_info, &["status", "code"], json!("EDGE_COPYING"));
        set_json_path(&mut disk_info, &["status", "sealed"], json!(false));
        set_json_path(
            &mut disk_info,
            &["edge", "edge_name"],
            json!(self.config.edge_name),
        );
        set_json_path(
            &mut disk_info,
            &["edge", "edge_code"],
            json!(self.config.edge_code),
        );
        set_json_path(
            &mut disk_info,
            &["edge", "seal_id"],
            json!(self.config.seal_id),
        );
        set_json_path(
            &mut disk_info,
            &["edge", "export_job_id"],
            json!(self.config.export_job_id),
        );
        set_json_path(
            &mut disk_info,
            &["edge", "export_started_at"],
            json!(Utc::now()),
        );
        self.write_disk_info(&disk_info)
    }

    fn mark_disk_info_sealed(
        &self,
        object_count: u64,
        total_bytes: u64,
        manifest_sha256: &str,
    ) -> Result<()> {
        let mut disk_info = self.read_disk_info()?;
        set_json_path(&mut disk_info, &["status", "code"], json!("SEALED"));
        set_json_path(&mut disk_info, &["status", "sealed"], json!(true));
        set_json_path(&mut disk_info, &["status", "imported"], json!(false));
        set_json_path(&mut disk_info, &["status", "reusable"], json!(false));
        set_json_path(
            &mut disk_info,
            &["edge", "seal_id"],
            json!(self.config.seal_id),
        );
        set_json_path(
            &mut disk_info,
            &["edge", "export_job_id"],
            json!(self.config.export_job_id),
        );
        set_json_path(
            &mut disk_info,
            &["edge", "export_finished_at"],
            json!(Utc::now()),
        );
        set_json_path(
            &mut disk_info,
            &["manifest", "manifest_path"],
            json!(MANIFEST_PATH),
        );
        set_json_path(
            &mut disk_info,
            &["manifest", "manifest_sha256_path"],
            json!(MANIFEST_SHA256_PATH),
        );
        set_json_path(
            &mut disk_info,
            &["manifest", "object_count"],
            json!(object_count),
        );
        set_json_path(
            &mut disk_info,
            &["manifest", "total_bytes"],
            json!(total_bytes),
        );
        set_json_path(
            &mut disk_info,
            &["manifest", "manifest_sha256"],
            json!(manifest_sha256),
        );
        set_json_path(
            &mut disk_info,
            &["security", "encryption_alg"],
            json!(ENCRYPTION_ALG),
        );
        set_json_path(
            &mut disk_info,
            &["security", "data_key_id"],
            json!(self.config.data_key_id),
        );
        self.write_disk_info(&disk_info)
    }

    fn read_disk_info(&self) -> Result<Value> {
        let path = self.root().join("disk_info.json");
        let bytes = fs::read(&path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn write_disk_info(&self, disk_info: &Value) -> Result<()> {
        atomic_write_json(&self.root().join("disk_info.json"), disk_info)
    }

    fn cleanup_or_quarantine_partials(&self) -> Result<()> {
        let partials = find_partial_files(&self.root())?;
        if partials.is_empty() {
            return Ok(());
        }

        let quarantine_dir = self.root().join("quarantine").join("partial");
        fs::create_dir_all(&quarantine_dir)?;
        for partial in partials {
            if fs::remove_file(&partial).is_ok() {
                continue;
            }
            let file_name = partial
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("orphan.partial");
            let quarantine_path = quarantine_dir.join(format!("{}-{file_name}", Uuid::new_v4()));
            fs::rename(&partial, &quarantine_path).map_err(|err| {
                DiskWorkerError::PartialCleanFailed(format!(
                    "failed to remove or quarantine {}: {err}",
                    partial.display()
                ))
            })?;
        }
        Ok(())
    }

    fn ensure_protocol_dirs(&self) -> Result<()> {
        for relative in [
            "packs",
            "frames",
            "meta",
            "manifests",
            "logs",
            "quarantine",
            "quarantine/partial",
        ] {
            let path = self.root().join(relative);
            fs::create_dir_all(&path)?;
        }
        Ok(())
    }

    fn root(&self) -> PathBuf {
        self.config.mount_path.join(PROTOCOL_ROOT)
    }
}

impl From<ExportedObjectUpdate> for ManifestObject {
    fn from(value: ExportedObjectUpdate) -> Self {
        Self {
            object_id: value.object_id,
            bucket: value.bucket,
            key: value.key,
            storage_mode: value.storage_mode,
            data_key_id: value.data_key_id,
            pack_ref: ManifestPackRef {
                pack_path: value.pack_path,
                pack_index_path: value.pack_index_path,
                pack_offset_bytes: value.pack_offset_bytes,
                ciphertext_size_bytes: value.pack_ciphertext_size_bytes,
                nonce: value.pack_nonce,
                tag: value.pack_tag,
                aad: value.pack_aad,
                ciphertext_sha256: value.pack_ciphertext_sha256,
            },
            frames: Vec::new(),
            frame_total: value.frame_total,
            relative_meta_path: value.relative_meta_path,
            size_bytes: value.size_bytes,
            estimated_landing_bytes: value.estimated_landing_bytes,
            etag: value.etag,
            last_modified: value.last_modified,
            content_type: value.content_type,
            metadata: value.metadata,
            plaintext_sha256: value.plaintext_sha256,
            exported_at: value.exported_at,
            object_status: value.object_status,
        }
    }
}

fn ensure_head_matches_task(object: &ExportObjectTask, head: &SourceObjectHead) -> Result<()> {
    if object.etag != head.etag || object.size_bytes != head.size_bytes {
        return Err(DiskWorkerError::SourceChanged(format!(
            "{}/{} no longer matches assigned snapshot",
            object.bucket, object.object_key
        )));
    }
    Ok(())
}

fn build_pack_aad(
    config: &DiskWorkerConfig,
    object: &ExportObjectTask,
    object_id: &Uuid,
    pack_path: &str,
    plaintext_sha256: &str,
) -> String {
    let disk_id = config.disk_id.to_string();
    let seal_id = config.seal_id.to_string();
    let export_job_id = config.export_job_id.to_string();
    String::from_utf8(pack_object_aad(PackObjectAad {
        disk_id: &disk_id,
        seal_id: &seal_id,
        export_job_id: &export_job_id,
        object_id: &object_id.to_string(),
        bucket: &object.bucket,
        object_key: &object.object_key,
        pack_path,
        pack_offset_bytes: 0,
        plaintext_sha256,
    }))
    .expect("object AAD is formatted as UTF-8")
}

fn data_path(export_job_id: &Uuid, object_id: i64) -> String {
    format!("packs/{export_job_id}/pack-{object_id}.pack")
}

fn pack_index_path(export_job_id: &Uuid, object_id: i64) -> String {
    format!("packs/{export_job_id}/pack-{object_id}.idx")
}

fn meta_path(export_job_id: &Uuid, object_id: i64) -> String {
    format!("meta/{export_job_id}/{object_id}.json")
}

fn validate_relative_path(path: &str, required_prefix: &str) -> Result<()> {
    let candidate = Path::new(path);
    if candidate.is_absolute() || !path.starts_with(required_prefix) {
        return Err(DiskWorkerError::ManifestInvalid(format!(
            "invalid relative path {path}"
        )));
    }
    for component in candidate.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(DiskWorkerError::ManifestInvalid(format!(
                "path traversal is not allowed: {path}"
            )));
        }
    }
    Ok(())
}

fn atomic_write_json(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    atomic_write_bytes(path, &bytes)
}

fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(classify_io_error)?;
    }
    let tmp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("file")
    ));
    {
        let mut file = File::create(&tmp_path).map_err(classify_io_error)?;
        make_file_writable_by_all(&tmp_path)?;
        file.write_all(bytes).map_err(classify_io_error)?;
        file.sync_all().map_err(classify_io_error)?;
    }
    fs::rename(&tmp_path, path).map_err(classify_io_error)?;
    make_file_writable_by_all(path)?;
    fsync_parent(path)?;
    Ok(())
}

#[cfg(unix)]
fn make_file_writable_by_all(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o666)).map_err(classify_io_error)
}

#[cfg(not(unix))]
fn make_file_writable_by_all(_path: &Path) -> Result<()> {
    Ok(())
}

fn fsync_file(path: &Path) -> Result<()> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(classify_io_error)?
        .sync_all()
        .map_err(classify_io_error)?;
    Ok(())
}

#[cfg(unix)]
fn fsync_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn fsync_parent(_path: &Path) -> Result<()> {
    Ok(())
}

fn find_partial_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut partials = Vec::new();
    if !root.exists() {
        return Ok(partials);
    }
    collect_partial_files(root, &mut partials)?;
    Ok(partials)
}

fn collect_partial_files(path: &Path, partials: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_partial_files(&path, partials)?;
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.ends_with(".partial"))
            .unwrap_or(false)
        {
            partials.push(path);
        }
    }
    Ok(())
}

fn set_json_path(root: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() {
        *root = value;
        return;
    }

    let mut cursor = root;
    for key in &path[..path.len() - 1] {
        if !cursor.get(key).is_some_and(Value::is_object) {
            cursor[key] = json!({});
        }
        cursor = &mut cursor[key];
    }
    cursor[path[path.len() - 1]] = value;
}

fn total_manifest_bytes(manifest: &ExportManifest) -> u64 {
    manifest
        .objects
        .iter()
        .map(|object| object.size_bytes)
        .sum()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn classify_io_error(err: io::Error) -> DiskWorkerError {
    match err.kind() {
        io::ErrorKind::NotFound => DiskWorkerError::DiskRemoved(err.to_string()),
        io::ErrorKind::StorageFull => DiskWorkerError::DiskFull(err.to_string()),
        _ if err.raw_os_error() == Some(28) => DiskWorkerError::DiskFull(err.to_string()),
        _ => DiskWorkerError::Io(err),
    }
}
