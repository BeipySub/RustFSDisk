use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt, fs,
    path::{Component, Path, PathBuf},
};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::center_security::{
    sign_disk_info_with_key, verify_disk_info_with_key, ENCRYPTION_ALG_AES_256_GCM,
    SIGNATURE_ALG_HMAC_SHA256,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportErrorCode {
    ManifestInvalid,
    ChecksumMismatch,
    DecryptFailed,
    NonceReused,
    SealIdManifestMismatch,
    SignatureInvalid,
}

impl ImportErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ManifestInvalid => "MANIFEST_INVALID",
            Self::ChecksumMismatch => "CHECKSUM_MISMATCH",
            Self::DecryptFailed => "DECRYPT_FAILED",
            Self::NonceReused => "NONCE_REUSED",
            Self::SealIdManifestMismatch => "SEAL_ID_MANIFEST_MISMATCH",
            Self::SignatureInvalid => "SIGNATURE_INVALID",
        }
    }
}

impl fmt::Display for ImportErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ImportError {
    pub code: ImportErrorCode,
    pub message: String,
}

impl ImportError {
    fn new(code: ImportErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub type ImportResult<T> = Result<T, ImportError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportOutcome {
    Imported { import_job_id: Uuid },
    SkippedAlreadyDone { import_job_id: Uuid },
    AlreadyImporting { import_job_id: Uuid },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportClaim {
    Acquired { import_job_id: Uuid },
    AlreadyImporting { import_job_id: Uuid },
    AlreadyDone { import_job_id: Uuid },
}

#[derive(Debug, Clone)]
pub struct ImportJobStart {
    pub disk_id: Uuid,
    pub seal_id: Uuid,
    pub export_job_id: Uuid,
    pub manifest_sha256: String,
    pub edge_code: String,
    pub object_count: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedDataKeyBinding {
    pub disk_id: Uuid,
    pub data_key_id: Uuid,
    pub export_job_id: Uuid,
    pub seal_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportCompletion {
    pub import_job_id: Uuid,
    pub imported_count: u64,
    pub imported_bytes: u64,
    pub data_key: ImportedDataKeyBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerIdentity {
    pub edge_code: String,
    pub source_bucket: String,
    pub source_key: String,
    pub source_etag: String,
    pub source_size_bytes: u64,
    pub source_last_modified: String,
}

#[derive(Debug, Clone)]
pub struct LedgerRecord {
    pub identity: LedgerIdentity,
    pub plaintext_sha256: String,
    pub ciphertext_sha256: Option<String>,
    pub chunk_group_id: Option<Uuid>,
    pub data_key_id: Option<Uuid>,
    pub nonce: Option<String>,
    pub import_bucket: String,
    pub import_key: String,
    pub export_job_id: Uuid,
    pub import_job_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ChunkPartRecord {
    pub chunk_group_id: Uuid,
    pub chunk_index: u32,
    pub chunk_total: u32,
    pub chunk_offset_bytes: u64,
    pub chunk_size_bytes: u64,
    pub disk_id: Uuid,
    pub seal_id: Uuid,
    pub import_job_id: Uuid,
    pub data_key_id: Uuid,
    pub nonce: String,
    pub ciphertext_sha256: String,
    pub plaintext: Vec<u8>,
}

pub trait ImportRepository {
    fn begin_import(&mut self, start: ImportJobStart) -> ImportResult<ImportClaim>;
    fn complete_import(&mut self, completion: ImportCompletion) -> ImportResult<()>;
    fn ensure_imported_data_key_sealed(
        &mut self,
        data_key: &ImportedDataKeyBinding,
    ) -> ImportResult<()>;
    fn fail_import(&mut self, import_job_id: Uuid, code: ImportErrorCode, message: &str);
    fn disk_registered(&self, disk_id: Uuid) -> bool;
    fn disk_enabled(&self, disk_id: Uuid) -> bool;
    fn data_key(&self, disk_id: Uuid, data_key_id: Uuid) -> Option<Vec<u8>>;
    fn identity_imported(&self, identity: &LedgerIdentity) -> bool;
    fn nonce_used(&self, data_key_id: Uuid, nonce: &str) -> bool;
    fn insert_ledger(&mut self, record: LedgerRecord);
    fn register_chunk_part(&mut self, part: ChunkPartRecord);
    fn chunk_parts(&self, chunk_group_id: Uuid) -> Vec<ChunkPartRecord>;
}

pub trait ArchiveStorage {
    fn ensure_bucket(&mut self, bucket: &str) -> ImportResult<()>;
    fn upload_object(&mut self, bucket: &str, key: &str, data: &[u8]) -> ImportResult<()>;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ImportProgressSnapshot {
    pub import_job_id: Option<Uuid>,
    pub disk_id: Option<Uuid>,
    pub seal_id: Option<Uuid>,
    pub import_job_status: String,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub object_total: u64,
    pub object_done: u64,
    pub current_object: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProgressAggregator {
    snapshot: ImportProgressSnapshot,
}

impl ProgressAggregator {
    pub fn start(
        &mut self,
        job: Uuid,
        disk_id: Uuid,
        seal_id: Uuid,
        total_bytes: u64,
        object_total: u64,
    ) {
        self.snapshot = ImportProgressSnapshot {
            import_job_id: Some(job),
            disk_id: Some(disk_id),
            seal_id: Some(seal_id),
            import_job_status: "IMPORTING".to_string(),
            total_bytes,
            object_total,
            ..ImportProgressSnapshot::default()
        };
    }

    pub fn current_object(&mut self, bucket: &str, key: &str) {
        self.snapshot.current_object = Some(format!("{bucket}/{key}"));
    }

    pub fn object_done(&mut self, bytes: u64) {
        self.snapshot.done_bytes = self.snapshot.done_bytes.saturating_add(bytes);
        self.snapshot.object_done = self.snapshot.object_done.saturating_add(1);
    }

    pub fn finish(&mut self) {
        self.snapshot.import_job_status = "DONE".to_string();
        self.snapshot.current_object = None;
    }

    pub fn fail(&mut self, code: ImportErrorCode) {
        self.snapshot.import_job_status = "FAILED".to_string();
        self.snapshot.error_code = Some(code.as_str().to_string());
    }

    pub fn snapshot(&self) -> ImportProgressSnapshot {
        self.snapshot.clone()
    }
}

pub struct ImportWorker<'a, R, S> {
    repo: &'a mut R,
    storage: &'a mut S,
    progress: &'a mut ProgressAggregator,
    center_signature_key: Vec<u8>,
}

impl<'a, R, S> ImportWorker<'a, R, S>
where
    R: ImportRepository,
    S: ArchiveStorage,
{
    pub fn new(
        repo: &'a mut R,
        storage: &'a mut S,
        progress: &'a mut ProgressAggregator,
        center_signature_key: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            repo,
            storage,
            progress,
            center_signature_key: center_signature_key.into(),
        }
    }

    pub fn import_sealed_disk(&mut self, protocol_root: &Path) -> ImportResult<ImportOutcome> {
        let disk_info = read_disk_info(protocol_root)?;
        validate_disk_info(&disk_info, &self.center_signature_key)?;

        let disk_id = parse_uuid("disk.disk_id", &disk_info.disk.disk_id)?;
        if !self.repo.disk_registered(disk_id) || !self.repo.disk_enabled(disk_id) {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "disk is not registered or enabled",
            ));
        }

        let manifest_path = safe_protocol_path(
            protocol_root,
            &disk_info.manifest.manifest_path,
            Some("manifests/"),
        )?;
        let manifest_sha_path = safe_protocol_path(
            protocol_root,
            &disk_info.manifest.manifest_sha256_path,
            Some("manifests/"),
        )?;
        let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
            ImportError::new(
                ImportErrorCode::ManifestInvalid,
                format!("failed to read manifest: {err}"),
            )
        })?;
        let actual_manifest_sha = sha256_hex(&manifest_bytes);
        let sha_file = fs::read_to_string(&manifest_sha_path).map_err(|err| {
            ImportError::new(
                ImportErrorCode::ManifestInvalid,
                format!("failed to read manifest sha256 file: {err}"),
            )
        })?;
        let sha_file = sha_file.trim();
        if sha_file != actual_manifest_sha
            || disk_info.manifest.manifest_sha256 != actual_manifest_sha
        {
            return Err(ImportError::new(
                ImportErrorCode::ChecksumMismatch,
                "manifest sha256 does not match disk_info or sha256 sidecar",
            ));
        }

        let manifest: ExportManifest = serde_json::from_slice(&manifest_bytes).map_err(|err| {
            ImportError::new(
                ImportErrorCode::ManifestInvalid,
                format!("failed to parse manifest: {err}"),
            )
        })?;
        let checked = validate_manifest(&disk_info, &manifest, &actual_manifest_sha)?;
        let claim = self.repo.begin_import(ImportJobStart {
            disk_id,
            seal_id: checked.seal_id,
            export_job_id: checked.export_job_id,
            manifest_sha256: actual_manifest_sha.clone(),
            edge_code: checked.edge_code.clone(),
            object_count: checked.object_count,
            total_bytes: checked.total_bytes,
        })?;

        let import_job_id = match claim {
            ImportClaim::Acquired { import_job_id } => import_job_id,
            ImportClaim::AlreadyDone { import_job_id } => {
                self.repo
                    .ensure_imported_data_key_sealed(&checked.data_key_binding())?;
                return Ok(ImportOutcome::SkippedAlreadyDone { import_job_id });
            }
            ImportClaim::AlreadyImporting { import_job_id } => {
                return Ok(ImportOutcome::AlreadyImporting { import_job_id });
            }
        };
        if disk_info.status.code != "SEALED" {
            let err = ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "disk must be SEALED for a new import",
            );
            self.repo
                .fail_import(import_job_id, err.code, err.message.as_str());
            return Err(err);
        }

        self.progress.start(
            import_job_id,
            disk_id,
            checked.seal_id,
            checked.total_bytes,
            checked.object_count,
        );

        match self.process_objects(
            protocol_root,
            &disk_info,
            &manifest,
            import_job_id,
            &checked,
        ) {
            Ok((count, bytes)) => {
                self.repo.complete_import(ImportCompletion {
                    import_job_id,
                    imported_count: count,
                    imported_bytes: bytes,
                    data_key: checked.data_key_binding(),
                })?;
                self.progress.finish();
                mark_disk_imported(
                    protocol_root,
                    disk_info,
                    import_job_id,
                    &self.center_signature_key,
                )?;
                Ok(ImportOutcome::Imported { import_job_id })
            }
            Err(err) => {
                self.repo.fail_import(import_job_id, err.code, &err.message);
                self.progress.fail(err.code);
                Err(err)
            }
        }
    }

    fn process_objects(
        &mut self,
        protocol_root: &Path,
        disk_info: &DiskInfo,
        manifest: &ExportManifest,
        import_job_id: Uuid,
        checked: &CheckedManifest,
    ) -> ImportResult<(u64, u64)> {
        let import_bucket = format!("archive-{}", checked.edge_code);
        self.storage.ensure_bucket(&import_bucket)?;

        let mut imported_count = 0_u64;
        let mut imported_bytes = 0_u64;

        for object in &manifest.objects {
            self.progress.current_object(&object.bucket, &object.key);
            let identity = object.identity(&checked.edge_code);
            if self.repo.identity_imported(&identity) {
                self.progress.object_done(object.progress_bytes());
                continue;
            }
            if self.repo.nonce_used(object.data_key_id, &object.nonce) {
                return Err(ImportError::new(
                    ImportErrorCode::NonceReused,
                    "data_key_id + nonce has already been imported",
                ));
            }

            let plaintext =
                self.decrypt_object(protocol_root, checked.disk_id, disk_info, object)?;
            if object.chunked {
                self.register_chunk(object, checked, import_job_id, plaintext)?;
                if self.try_merge_chunk_group(object, checked, import_job_id, &import_bucket)? {
                    imported_count += 1;
                    imported_bytes += object.size_bytes;
                    self.progress.object_done(object.progress_bytes());
                } else {
                    self.progress.object_done(object.progress_bytes());
                }
                continue;
            }

            let import_key = archive_key(&object.bucket, &object.key);
            self.storage
                .upload_object(&import_bucket, &import_key, &plaintext)?;
            self.repo.insert_ledger(LedgerRecord {
                identity,
                plaintext_sha256: object.plaintext_sha256.clone(),
                ciphertext_sha256: Some(object.ciphertext_sha256.clone()),
                chunk_group_id: None,
                data_key_id: Some(object.data_key_id),
                nonce: Some(object.nonce.clone()),
                import_bucket: import_bucket.clone(),
                import_key,
                export_job_id: checked.export_job_id,
                import_job_id,
            });
            imported_count += 1;
            imported_bytes += object.size_bytes;
            self.progress.object_done(object.progress_bytes());
        }

        Ok((imported_count, imported_bytes))
    }

    fn decrypt_object(
        &self,
        protocol_root: &Path,
        disk_id: Uuid,
        disk_info: &DiskInfo,
        object: &ManifestObject,
    ) -> ImportResult<Vec<u8>> {
        if object.data_key_id.to_string() != disk_info.security.data_key_id {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "object data_key_id does not match disk_info security.data_key_id",
            ));
        }
        let key = self
            .repo
            .data_key(disk_id, object.data_key_id)
            .ok_or_else(|| {
                ImportError::new(ImportErrorCode::DecryptFailed, "data key not found")
            })?;
        let ciphertext_path =
            safe_protocol_path(protocol_root, &object.relative_data_path, Some("data/"))?;
        let ciphertext = fs::read(&ciphertext_path).map_err(|err| {
            ImportError::new(
                ImportErrorCode::ManifestInvalid,
                format!("failed to read ciphertext: {err}"),
            )
        })?;
        let actual_ciphertext_sha = sha256_hex(&ciphertext);
        if actual_ciphertext_sha != object.ciphertext_sha256 {
            return Err(ImportError::new(
                ImportErrorCode::ChecksumMismatch,
                "ciphertext sha256 mismatch",
            ));
        }
        if ciphertext.len() as u64 != object.ciphertext_size_bytes {
            return Err(ImportError::new(
                ImportErrorCode::ChecksumMismatch,
                "ciphertext size mismatch",
            ));
        }

        let nonce = decode_b64_or_hex("nonce", &object.nonce)?;
        let tag = decode_b64_or_hex("tag", &object.tag)?;
        if nonce.len() != 12 || tag.len() != 16 || key.len() != 32 {
            return Err(ImportError::new(
                ImportErrorCode::DecryptFailed,
                "invalid AES-GCM key, nonce or tag length",
            ));
        }
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|_| ImportError::new(ImportErrorCode::DecryptFailed, "invalid AES key"))?;
        let mut payload = ciphertext;
        payload.extend_from_slice(&tag);
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                aes_gcm::aead::Payload {
                    msg: payload.as_ref(),
                    aad: object.aad.as_bytes(),
                },
            )
            .map_err(|_| {
                ImportError::new(
                    ImportErrorCode::DecryptFailed,
                    "AES-GCM authentication failed",
                )
            })?;
        if sha256_hex(&plaintext) != object.plaintext_sha256 && !object.chunked {
            return Err(ImportError::new(
                ImportErrorCode::ChecksumMismatch,
                "plaintext sha256 mismatch",
            ));
        }
        Ok(plaintext)
    }

    fn register_chunk(
        &mut self,
        object: &ManifestObject,
        checked: &CheckedManifest,
        import_job_id: Uuid,
        plaintext: Vec<u8>,
    ) -> ImportResult<()> {
        let chunk_group_id = object.chunk_group_id.ok_or_else(|| {
            ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "chunked object is missing chunk_group_id",
            )
        })?;
        self.repo.register_chunk_part(ChunkPartRecord {
            chunk_group_id,
            chunk_index: object.chunk_index,
            chunk_total: object.chunk_total,
            chunk_offset_bytes: object.chunk_offset_bytes,
            chunk_size_bytes: object.chunk_size_bytes,
            disk_id: checked.disk_id,
            seal_id: checked.seal_id,
            import_job_id,
            data_key_id: object.data_key_id,
            nonce: object.nonce.clone(),
            ciphertext_sha256: object.ciphertext_sha256.clone(),
            plaintext,
        });
        Ok(())
    }

    fn try_merge_chunk_group(
        &mut self,
        object: &ManifestObject,
        checked: &CheckedManifest,
        import_job_id: Uuid,
        import_bucket: &str,
    ) -> ImportResult<bool> {
        let chunk_group_id = object.chunk_group_id.ok_or_else(|| {
            ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "chunked object is missing chunk_group_id",
            )
        })?;
        let mut parts = self.repo.chunk_parts(chunk_group_id);
        if parts.len() != object.chunk_total as usize {
            return Ok(false);
        }
        parts.sort_by_key(|part| part.chunk_index);
        validate_registered_parts(&parts, object)?;
        let mut plaintext = Vec::with_capacity(object.size_bytes as usize);
        for part in &parts {
            plaintext.extend_from_slice(&part.plaintext);
        }
        if plaintext.len() as u64 != object.size_bytes
            || sha256_hex(&plaintext) != object.plaintext_sha256
        {
            return Err(ImportError::new(
                ImportErrorCode::ChecksumMismatch,
                "merged chunk plaintext sha256 mismatch",
            ));
        }
        let identity = object.identity(&checked.edge_code);
        if self.repo.identity_imported(&identity) {
            return Ok(true);
        }
        let import_key = archive_key(&object.bucket, &object.key);
        self.storage
            .upload_object(import_bucket, &import_key, &plaintext)?;
        self.repo.insert_ledger(LedgerRecord {
            identity,
            plaintext_sha256: object.plaintext_sha256.clone(),
            ciphertext_sha256: None,
            chunk_group_id: Some(chunk_group_id),
            data_key_id: None,
            nonce: None,
            import_bucket: import_bucket.to_string(),
            import_key,
            export_job_id: checked.export_job_id,
            import_job_id,
        });
        Ok(true)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DiskInfo {
    protocol: DiskProtocol,
    disk: DiskIdentity,
    status: DiskStatus,
    edge: DiskEdge,
    center: DiskCenter,
    manifest: DiskManifestRef,
    security: DiskSecurity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DiskProtocol {
    name: String,
    version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DiskIdentity {
    disk_id: String,
    #[serde(default)]
    sn: String,
    #[serde(default)]
    capacity_bytes: u64,
    #[serde(default)]
    last_init_time: String,
    #[serde(default)]
    initialized_by: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DiskStatus {
    code: String,
    #[serde(default)]
    sealed: bool,
    #[serde(default)]
    imported: bool,
    #[serde(default)]
    reusable: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DiskEdge {
    edge_code: String,
    seal_id: String,
    export_job_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DiskCenter {
    #[serde(default)]
    center_id: String,
    #[serde(default)]
    import_job_id: String,
    #[serde(default)]
    import_started_at: String,
    #[serde(default)]
    import_finished_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DiskManifestRef {
    manifest_path: String,
    manifest_sha256_path: String,
    object_count: u64,
    total_bytes: u64,
    manifest_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DiskSecurity {
    center_signature: String,
    signature_alg: String,
    center_key_id: String,
    encryption_alg: String,
    data_key_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ExportManifest {
    manifest_version: String,
    seal_id: Uuid,
    export_job_id: Uuid,
    disk_id: Uuid,
    edge_code: String,
    objects: Vec<ManifestObject>,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestObject {
    bucket: String,
    key: String,
    relative_data_path: String,
    #[serde(default)]
    encrypted: bool,
    encryption_alg: String,
    data_key_id: Uuid,
    nonce: String,
    tag: String,
    aad: String,
    ciphertext_size_bytes: u64,
    ciphertext_sha256: String,
    #[serde(default)]
    chunked: bool,
    #[serde(default, deserialize_with = "empty_uuid_as_none")]
    chunk_group_id: Option<Uuid>,
    #[serde(default)]
    chunk_index: u32,
    #[serde(default = "one")]
    chunk_total: u32,
    #[serde(default)]
    chunk_offset_bytes: u64,
    #[serde(default)]
    chunk_size_bytes: u64,
    #[serde(default)]
    chunk_sha256: String,
    relative_meta_path: String,
    size_bytes: u64,
    etag: String,
    last_modified: String,
    plaintext_sha256: String,
    object_status: String,
}

impl ManifestObject {
    fn identity(&self, edge_code: &str) -> LedgerIdentity {
        LedgerIdentity {
            edge_code: edge_code.to_string(),
            source_bucket: self.bucket.clone(),
            source_key: self.key.clone(),
            source_etag: self.etag.clone(),
            source_size_bytes: self.size_bytes,
            source_last_modified: self.last_modified.clone(),
        }
    }

    fn progress_bytes(&self) -> u64 {
        if self.chunked {
            self.chunk_size_bytes
        } else {
            self.size_bytes
        }
    }
}

#[derive(Debug, Clone)]
struct CheckedManifest {
    disk_id: Uuid,
    seal_id: Uuid,
    export_job_id: Uuid,
    data_key_id: Uuid,
    edge_code: String,
    object_count: u64,
    total_bytes: u64,
}

impl CheckedManifest {
    fn data_key_binding(&self) -> ImportedDataKeyBinding {
        ImportedDataKeyBinding {
            disk_id: self.disk_id,
            data_key_id: self.data_key_id,
            export_job_id: self.export_job_id,
            seal_id: self.seal_id,
        }
    }
}

fn one() -> u32 {
    1
}

fn empty_uuid_as_none<'de, D>(deserializer: D) -> Result<Option<Uuid>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    match value.as_deref() {
        None | Some("") => Ok(None),
        Some(raw) => Uuid::parse_str(raw)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

fn read_disk_info(protocol_root: &Path) -> ImportResult<DiskInfo> {
    let path = protocol_root.join("disk_info.json");
    let bytes = fs::read(&path).map_err(|err| {
        ImportError::new(
            ImportErrorCode::ManifestInvalid,
            format!("failed to read disk_info.json: {err}"),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|err| {
        ImportError::new(
            ImportErrorCode::ManifestInvalid,
            format!("failed to parse disk_info.json: {err}"),
        )
    })
}

fn validate_disk_info(disk_info: &DiskInfo, center_signature_key: &[u8]) -> ImportResult<()> {
    if disk_info.protocol.name != "rustfs-offline-transfer"
        || !disk_info.protocol.version.starts_with("1.")
    {
        return Err(ImportError::new(
            ImportErrorCode::ManifestInvalid,
            "unsupported disk protocol",
        ));
    }
    if !matches!(disk_info.status.code.as_str(), "SEALED" | "IMPORTED") || !disk_info.status.sealed
    {
        return Err(ImportError::new(
            ImportErrorCode::ManifestInvalid,
            "disk is not SEALED or already imported",
        ));
    }
    if disk_info.security.signature_alg != SIGNATURE_ALG_HMAC_SHA256 {
        return Err(ImportError::new(
            ImportErrorCode::SignatureInvalid,
            "unsupported disk_info signature algorithm",
        ));
    }
    verify_disk_info_with_key(disk_info, center_signature_key).map_err(|err| {
        ImportError::new(
            ImportErrorCode::SignatureInvalid,
            format!("disk_info center_signature verification failed: {err}"),
        )
    })?;
    if disk_info.security.encryption_alg != ENCRYPTION_ALG_AES_256_GCM {
        return Err(ImportError::new(
            ImportErrorCode::ManifestInvalid,
            "unsupported encryption algorithm",
        ));
    }
    Ok(())
}

fn validate_manifest(
    disk_info: &DiskInfo,
    manifest: &ExportManifest,
    manifest_sha256: &str,
) -> ImportResult<CheckedManifest> {
    if manifest.manifest_version != "1.0.0" {
        return Err(ImportError::new(
            ImportErrorCode::ManifestInvalid,
            "unsupported manifest version",
        ));
    }
    let disk_id = parse_uuid("disk.disk_id", &disk_info.disk.disk_id)?;
    let seal_id = parse_uuid("edge.seal_id", &disk_info.edge.seal_id)?;
    let export_job_id = parse_uuid("edge.export_job_id", &disk_info.edge.export_job_id)?;
    let data_key_id = parse_uuid("security.data_key_id", &disk_info.security.data_key_id)?;
    if manifest.disk_id != disk_id
        || manifest.seal_id != seal_id
        || manifest.export_job_id != export_job_id
        || manifest.edge_code != disk_info.edge.edge_code
    {
        return Err(ImportError::new(
            ImportErrorCode::ManifestInvalid,
            "manifest identity fields do not match disk_info",
        ));
    }
    if disk_info.manifest.manifest_sha256 != manifest_sha256 {
        return Err(ImportError::new(
            ImportErrorCode::ChecksumMismatch,
            "manifest sha256 mismatch",
        ));
    }
    if manifest
        .objects
        .iter()
        .any(|object| object.data_key_id != data_key_id)
    {
        return Err(ImportError::new(
            ImportErrorCode::ManifestInvalid,
            "manifest object data_key_id does not match disk_info security.data_key_id",
        ));
    }
    validate_objects(&manifest.objects)?;
    let object_count = manifest.objects.len() as u64;
    let total_bytes = manifest
        .objects
        .iter()
        .map(ManifestObject::progress_bytes)
        .sum::<u64>();
    if object_count != disk_info.manifest.object_count
        || total_bytes != disk_info.manifest.total_bytes
    {
        return Err(ImportError::new(
            ImportErrorCode::ManifestInvalid,
            "disk_info manifest counters do not match manifest objects",
        ));
    }

    Ok(CheckedManifest {
        disk_id,
        seal_id,
        export_job_id,
        data_key_id,
        edge_code: manifest.edge_code.clone(),
        object_count,
        total_bytes,
    })
}

fn validate_objects(objects: &[ManifestObject]) -> ImportResult<()> {
    let mut nonces = HashSet::new();
    let mut chunks: HashMap<Uuid, Vec<&ManifestObject>> = HashMap::new();
    for object in objects {
        if object.object_status != "EXPORTED"
            || !object.encrypted
            || object.encryption_alg != "AES-256-GCM"
            || object.bucket.is_empty()
            || object.key.is_empty()
            || object.plaintext_sha256.len() != 64
            || object.ciphertext_sha256.len() != 64
        {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "manifest object has invalid fields",
            ));
        }
        if !object.relative_data_path.starts_with("data/")
            || !object.relative_meta_path.starts_with("meta/")
        {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "object paths must be under data/ and meta/",
            ));
        }
        if !nonces.insert((object.data_key_id, object.nonce.clone())) {
            return Err(ImportError::new(
                ImportErrorCode::NonceReused,
                "manifest contains duplicate data_key_id + nonce",
            ));
        }
        if object.chunked {
            validate_chunk_object(object)?;
            chunks
                .entry(object.chunk_group_id.expect("validated chunk_group_id"))
                .or_default()
                .push(object);
        } else if object.chunk_index != 0
            || object.chunk_total != 1
            || object.chunk_offset_bytes != 0
            || object.chunk_size_bytes != object.size_bytes
        {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "plain object contains invalid chunk fields",
            ));
        }
    }
    for group in chunks.values() {
        validate_manifest_chunk_group(group)?;
    }
    Ok(())
}

fn validate_chunk_object(object: &ManifestObject) -> ImportResult<()> {
    if object.chunk_group_id.is_none()
        || object.chunk_total <= 1
        || object.chunk_total > 1_000_000
        || object.chunk_index >= object.chunk_total
        || object.chunk_size_bytes == 0
        || object.chunk_offset_bytes >= object.size_bytes
    {
        return Err(ImportError::new(
            ImportErrorCode::ManifestInvalid,
            "chunk object contains invalid chunk fields",
        ));
    }
    if !object.chunk_sha256.is_empty() && object.chunk_sha256 != object.ciphertext_sha256 {
        return Err(ImportError::new(
            ImportErrorCode::ChecksumMismatch,
            "chunk sha256 does not match ciphertext sha256",
        ));
    }
    Ok(())
}

fn validate_manifest_chunk_group(group: &[&ManifestObject]) -> ImportResult<()> {
    let first = group[0];
    let mut seen = HashSet::new();
    for object in group {
        if object.chunk_group_id != first.chunk_group_id
            || object.chunk_total != first.chunk_total
            || object.bucket != first.bucket
            || object.key != first.key
            || object.etag != first.etag
            || object.size_bytes != first.size_bytes
            || object.last_modified != first.last_modified
            || object.plaintext_sha256 != first.plaintext_sha256
        {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "chunk group source fields are inconsistent",
            ));
        }
        if !seen.insert(object.chunk_index) {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "duplicate chunk_index in manifest",
            ));
        }
    }
    if group.len() == first.chunk_total as usize {
        let mut sorted = group.to_vec();
        sorted.sort_by_key(|object| object.chunk_index);
        let mut offset = 0_u64;
        for (expected_index, object) in sorted.iter().enumerate() {
            if object.chunk_index != expected_index as u32 || object.chunk_offset_bytes != offset {
                return Err(ImportError::new(
                    ImportErrorCode::ManifestInvalid,
                    "chunk group is not continuous",
                ));
            }
            offset += object.chunk_size_bytes;
        }
        if offset != first.size_bytes {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "chunk sizes do not sum to source size",
            ));
        }
    }
    Ok(())
}

fn validate_registered_parts(
    parts: &[ChunkPartRecord],
    object: &ManifestObject,
) -> ImportResult<()> {
    let mut offset = 0_u64;
    for (expected_index, part) in parts.iter().enumerate() {
        if part.chunk_index != expected_index as u32
            || part.chunk_total != object.chunk_total
            || part.chunk_offset_bytes != offset
            || part.chunk_size_bytes != part.plaintext.len() as u64
        {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "registered chunk parts are not continuous",
            ));
        }
        offset += part.chunk_size_bytes;
    }
    if offset != object.size_bytes {
        return Err(ImportError::new(
            ImportErrorCode::ManifestInvalid,
            "registered chunk parts do not cover source object",
        ));
    }
    Ok(())
}

fn mark_disk_imported(
    protocol_root: &Path,
    mut disk_info: DiskInfo,
    import_job_id: Uuid,
    center_signature_key: &[u8],
) -> ImportResult<()> {
    let now = Utc::now().to_rfc3339();
    disk_info.status.code = "IMPORTED".to_string();
    disk_info.status.sealed = true;
    disk_info.status.imported = true;
    disk_info.status.reusable = true;
    disk_info.center.import_job_id = import_job_id.to_string();
    disk_info.center.import_finished_at = now.clone();
    disk_info.updated_at = Some(now);
    disk_info.security.center_signature = String::new();
    disk_info.security.center_signature = sign_disk_info_with_key(&disk_info, center_signature_key)
        .map_err(|err| {
            ImportError::new(
                ImportErrorCode::SignatureInvalid,
                format!("failed to sign imported disk_info: {err}"),
            )
        })?;
    let path = protocol_root.join("disk_info.json");
    let tmp = protocol_root.join("disk_info.json.tmp");
    let bytes = serde_json::to_vec_pretty(&disk_info).map_err(|err| {
        ImportError::new(
            ImportErrorCode::ManifestInvalid,
            format!("failed to serialize disk_info.json: {err}"),
        )
    })?;
    fs::write(&tmp, bytes).map_err(|err| {
        ImportError::new(
            ImportErrorCode::ManifestInvalid,
            format!("failed to write disk_info temp file: {err}"),
        )
    })?;
    fs::rename(&tmp, &path).map_err(|err| {
        ImportError::new(
            ImportErrorCode::ManifestInvalid,
            format!("failed to replace disk_info.json: {err}"),
        )
    })
}

fn safe_protocol_path(
    root: &Path,
    relative: &str,
    required_prefix: Option<&str>,
) -> ImportResult<PathBuf> {
    if relative.is_empty() || relative.starts_with('/') || relative.starts_with('\\') {
        return Err(ImportError::new(
            ImportErrorCode::ManifestInvalid,
            "protocol path must be relative",
        ));
    }
    if let Some(prefix) = required_prefix {
        if !relative.starts_with(prefix) {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "protocol path is outside required directory",
            ));
        }
    }
    let path = Path::new(relative);
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "protocol path contains traversal or prefix",
            ));
        }
    }
    Ok(root.join(path))
}

fn parse_uuid(field: &str, value: &str) -> ImportResult<Uuid> {
    Uuid::parse_str(value).map_err(|_| {
        ImportError::new(
            ImportErrorCode::ManifestInvalid,
            format!("{field} is not a valid UUID"),
        )
    })
}

fn decode_b64_or_hex(field: &str, value: &str) -> ImportResult<Vec<u8>> {
    general_purpose::STANDARD
        .decode(value)
        .or_else(|_| hex::decode(value))
        .map_err(|_| {
            ImportError::new(
                ImportErrorCode::ManifestInvalid,
                format!("{field} must be base64 or hex"),
            )
        })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn archive_key(bucket: &str, key: &str) -> String {
    format!("{bucket}/{key}")
}

#[derive(Debug, Clone)]
pub struct MemoryImportJob {
    pub import_job_id: Uuid,
    pub disk_id: Uuid,
    pub seal_id: Uuid,
    pub export_job_id: Uuid,
    pub manifest_sha256: String,
    pub status: String,
    pub error_code: Option<String>,
}

pub const DATA_KEY_STATUS_ACTIVE: &str = "ACTIVE";
pub const DATA_KEY_STATUS_ISSUED: &str = "ISSUED";
pub const DATA_KEY_STATUS_SEALED_READONLY: &str = "SEALED_READONLY";
pub const DATA_KEY_STATUS_RETIRED: &str = "RETIRED";
pub const DATA_KEY_STATUS_REVOKED: &str = "REVOKED";

#[derive(Debug, Clone)]
struct MemoryDataKey {
    key: Vec<u8>,
    status: String,
    export_job_id: Option<Uuid>,
    seal_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryDataKeyState {
    pub status: String,
    pub export_job_id: Option<Uuid>,
    pub seal_id: Option<Uuid>,
}

#[derive(Debug, Default)]
pub struct MemoryRepository {
    registered_disks: HashSet<Uuid>,
    disabled_disks: HashSet<Uuid>,
    data_keys: HashMap<(Uuid, Uuid), MemoryDataKey>,
    jobs: HashMap<(Uuid, Uuid), MemoryImportJob>,
    ledger: Vec<LedgerRecord>,
    chunks: HashMap<(Uuid, u32), ChunkPartRecord>,
}

impl MemoryRepository {
    pub fn register_disk(&mut self, disk_id: Uuid) {
        self.registered_disks.insert(disk_id);
    }

    pub fn put_data_key(&mut self, disk_id: Uuid, data_key_id: Uuid, key: Vec<u8>) {
        self.put_issued_data_key(disk_id, data_key_id, Uuid::nil(), key);
        if let Some(stored) = self.data_keys.get_mut(&(disk_id, data_key_id)) {
            stored.export_job_id = None;
        }
    }

    pub fn put_issued_data_key(
        &mut self,
        disk_id: Uuid,
        data_key_id: Uuid,
        export_job_id: Uuid,
        key: Vec<u8>,
    ) {
        self.data_keys.insert(
            (disk_id, data_key_id),
            MemoryDataKey {
                key,
                status: DATA_KEY_STATUS_ISSUED.to_string(),
                export_job_id: Some(export_job_id),
                seal_id: None,
            },
        );
    }

    pub fn data_key_state(&self, disk_id: Uuid, data_key_id: Uuid) -> Option<MemoryDataKeyState> {
        self.data_keys
            .get(&(disk_id, data_key_id))
            .map(|key| MemoryDataKeyState {
                status: key.status.clone(),
                export_job_id: key.export_job_id,
                seal_id: key.seal_id,
            })
    }

    pub fn set_data_key_lifecycle_for_test(
        &mut self,
        disk_id: Uuid,
        data_key_id: Uuid,
        status: impl Into<String>,
        seal_id: Option<Uuid>,
    ) {
        if let Some(key) = self.data_keys.get_mut(&(disk_id, data_key_id)) {
            key.status = status.into();
            key.seal_id = seal_id;
        }
    }

    pub fn ledger(&self) -> &[LedgerRecord] {
        &self.ledger
    }

    pub fn jobs(&self) -> Vec<MemoryImportJob> {
        self.jobs.values().cloned().collect()
    }

    fn validate_data_key_binding(&self, data_key: &ImportedDataKeyBinding) -> ImportResult<()> {
        let Some(key) = self
            .data_keys
            .get(&(data_key.disk_id, data_key.data_key_id))
        else {
            return Err(ImportError::new(
                ImportErrorCode::DecryptFailed,
                "data key for imported disk was not found",
            ));
        };
        if let Some(export_job_id) = key.export_job_id {
            if export_job_id != data_key.export_job_id {
                return Err(ImportError::new(
                    ImportErrorCode::ManifestInvalid,
                    "data key export_job_id does not match imported manifest",
                ));
            }
        }
        if let Some(seal_id) = key.seal_id {
            if seal_id != data_key.seal_id {
                return Err(ImportError::new(
                    ImportErrorCode::ManifestInvalid,
                    "data key seal_id does not match imported manifest",
                ));
            }
        }
        match key.status.as_str() {
            DATA_KEY_STATUS_ACTIVE
            | DATA_KEY_STATUS_ISSUED
            | DATA_KEY_STATUS_SEALED_READONLY
            | DATA_KEY_STATUS_RETIRED => Ok(()),
            DATA_KEY_STATUS_REVOKED => Err(ImportError::new(
                ImportErrorCode::DecryptFailed,
                "revoked data key cannot be finalized for import",
            )),
            _ => Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "data key has unsupported lifecycle status",
            )),
        }
    }

    fn bind_imported_data_key(&mut self, data_key: &ImportedDataKeyBinding) {
        let key = self
            .data_keys
            .get_mut(&(data_key.disk_id, data_key.data_key_id))
            .expect("binding validated before mutation");
        key.export_job_id = Some(data_key.export_job_id);
        key.seal_id = Some(data_key.seal_id);
        if key.status != DATA_KEY_STATUS_RETIRED {
            key.status = DATA_KEY_STATUS_SEALED_READONLY.to_string();
        }
    }
}

impl ImportRepository for MemoryRepository {
    fn begin_import(&mut self, start: ImportJobStart) -> ImportResult<ImportClaim> {
        let key = (start.disk_id, start.seal_id);
        if let Some(job) = self.jobs.get(&key) {
            if job.manifest_sha256 != start.manifest_sha256 {
                return Err(ImportError::new(
                    ImportErrorCode::SealIdManifestMismatch,
                    "same disk_id + seal_id has a different manifest sha256",
                ));
            }
            return if job.status == "DONE" {
                Ok(ImportClaim::AlreadyDone {
                    import_job_id: job.import_job_id,
                })
            } else {
                Ok(ImportClaim::AlreadyImporting {
                    import_job_id: job.import_job_id,
                })
            };
        }
        let import_job_id = Uuid::new_v4();
        self.jobs.insert(
            key,
            MemoryImportJob {
                import_job_id,
                disk_id: start.disk_id,
                seal_id: start.seal_id,
                export_job_id: start.export_job_id,
                manifest_sha256: start.manifest_sha256,
                status: "IMPORTING".to_string(),
                error_code: None,
            },
        );
        Ok(ImportClaim::Acquired { import_job_id })
    }

    fn complete_import(&mut self, completion: ImportCompletion) -> ImportResult<()> {
        let Some(job_key) = self
            .jobs
            .iter()
            .find_map(|(key, job)| (job.import_job_id == completion.import_job_id).then_some(*key))
        else {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "import job was not found for completion",
            ));
        };
        let job = self
            .jobs
            .get(&job_key)
            .expect("job key was selected from jobs");
        if job.disk_id != completion.data_key.disk_id
            || job.seal_id != completion.data_key.seal_id
            || job.export_job_id != completion.data_key.export_job_id
        {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "import job does not match imported data key binding",
            ));
        }
        if !matches!(job.status.as_str(), "IMPORTING" | "DONE") {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "import job is not in a completable state",
            ));
        }
        self.validate_data_key_binding(&completion.data_key)?;
        self.bind_imported_data_key(&completion.data_key);
        self.jobs
            .get_mut(&job_key)
            .expect("job key was validated before mutation")
            .status = "DONE".to_string();
        Ok(())
    }

    fn ensure_imported_data_key_sealed(
        &mut self,
        data_key: &ImportedDataKeyBinding,
    ) -> ImportResult<()> {
        let done = self.jobs.values().any(|job| {
            job.disk_id == data_key.disk_id
                && job.seal_id == data_key.seal_id
                && job.export_job_id == data_key.export_job_id
                && job.status == "DONE"
        });
        if !done {
            return Err(ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "completed import job was not found for data key sealing",
            ));
        }
        self.validate_data_key_binding(data_key)?;
        self.bind_imported_data_key(data_key);
        Ok(())
    }

    fn fail_import(&mut self, import_job_id: Uuid, code: ImportErrorCode, _message: &str) {
        if let Some(job) = self
            .jobs
            .values_mut()
            .find(|job| job.import_job_id == import_job_id)
        {
            job.status = "FAILED".to_string();
            job.error_code = Some(code.as_str().to_string());
        }
    }

    fn disk_registered(&self, disk_id: Uuid) -> bool {
        self.registered_disks.contains(&disk_id)
    }

    fn disk_enabled(&self, disk_id: Uuid) -> bool {
        !self.disabled_disks.contains(&disk_id)
    }

    fn data_key(&self, disk_id: Uuid, data_key_id: Uuid) -> Option<Vec<u8>> {
        self.data_keys
            .get(&(disk_id, data_key_id))
            .filter(|key| key.status != DATA_KEY_STATUS_REVOKED)
            .map(|key| key.key.clone())
    }

    fn identity_imported(&self, identity: &LedgerIdentity) -> bool {
        self.ledger
            .iter()
            .any(|record| &record.identity == identity)
    }

    fn nonce_used(&self, data_key_id: Uuid, nonce: &str) -> bool {
        self.ledger.iter().any(|record| {
            record.data_key_id == Some(data_key_id) && record.nonce.as_deref() == Some(nonce)
        }) || self
            .chunks
            .values()
            .any(|part| part.data_key_id == data_key_id && part.nonce == nonce)
    }

    fn insert_ledger(&mut self, record: LedgerRecord) {
        self.ledger.push(record);
    }

    fn register_chunk_part(&mut self, part: ChunkPartRecord) {
        self.chunks
            .entry((part.chunk_group_id, part.chunk_index))
            .or_insert(part);
    }

    fn chunk_parts(&self, chunk_group_id: Uuid) -> Vec<ChunkPartRecord> {
        let mut parts = self
            .chunks
            .values()
            .filter(|part| part.chunk_group_id == chunk_group_id)
            .cloned()
            .collect::<Vec<_>>();
        parts.sort_by_key(|part| part.chunk_index);
        parts
    }
}

#[derive(Debug, Default)]
pub struct MemoryArchiveStorage {
    objects: BTreeMap<(String, String), Vec<u8>>,
}

impl MemoryArchiveStorage {
    pub fn objects(&self) -> &BTreeMap<(String, String), Vec<u8>> {
        &self.objects
    }
}

impl ArchiveStorage for MemoryArchiveStorage {
    fn ensure_bucket(&mut self, _bucket: &str) -> ImportResult<()> {
        Ok(())
    }

    fn upload_object(&mut self, bucket: &str, key: &str, data: &[u8]) -> ImportResult<()> {
        self.objects
            .insert((bucket.to_string(), key.to_string()), data.to_vec());
        Ok(())
    }
}

#[allow(dead_code)]
fn parse_rfc3339(value: &str) -> ImportResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| {
            ImportError::new(
                ImportErrorCode::ManifestInvalid,
                "invalid RFC3339 timestamp",
            )
        })
}
