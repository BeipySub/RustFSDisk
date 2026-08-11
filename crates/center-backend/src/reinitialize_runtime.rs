use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

use anyhow::{anyhow, Context};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::path::Component;
use tokio::{runtime::Handle, task};
use uuid::Uuid;

use crate::{
    center_security::CenterSecurity,
    reinitializer::{
        validate_center_signature_for_reinitialize, DiskInfoDocument, DiskInfoTemplate,
        DiskStatusCode, NewDataKey, PostImportReinitializer, PostImportRepository,
        RegisteredDiskIdentity, ReinitializeError, ReinitializedDisk, RuntimeStatus,
        DISK_INFO_FILE, PROTOCOL_ROOT,
    },
};

pub type CenterReinitializeFuture<'a> =
    Pin<Box<dyn Future<Output = anyhow::Result<CenterReinitializeResponse>> + Send + 'a>>;

pub trait CenterReinitializeControlService: Send + Sync {
    fn reinitialize_disk<'a>(
        &'a self,
        disk_id: Uuid,
        request: CenterReinitializeRequest,
    ) -> CenterReinitializeFuture<'a>;
}

#[derive(Debug, Clone, Deserialize)]
pub struct CenterReinitializeRequest {
    pub mount_path: PathBuf,
    pub seal_id: Uuid,
    pub expected_status_code: DiskStatusCode,
    pub operator_reason: String,
    pub confirm_reinitialize: bool,
    #[serde(default)]
    pub confirm_discard_sealed_export: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CenterReinitializeResponse {
    pub disk_id: Uuid,
    pub old_seal_id: Uuid,
    pub old_data_key_id: Uuid,
    pub new_data_key_id: Uuid,
    pub disk_status_code: DiskStatusCode,
    pub runtime_status: String,
    pub message: String,
}

pub trait FileSystemProbe: Send + Sync {
    fn filesystem_type(&self, mount_path: &Path) -> anyhow::Result<String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReinitializeMode {
    ImportedCleanup,
    DiscardSealedExport,
}

#[derive(Debug, Default)]
pub struct FindmntFileSystemProbe;

impl FileSystemProbe for FindmntFileSystemProbe {
    fn filesystem_type(&self, mount_path: &Path) -> anyhow::Result<String> {
        #[cfg(unix)]
        {
            use std::process::Command;

            let output = Command::new("findmnt")
                .args(["-no", "FSTYPE", "--target"])
                .arg(mount_path)
                .output()
                .context("probe transport disk filesystem with findmnt")?;
            return parse_findmnt_fstype_output(
                output.status.success(),
                &String::from_utf8_lossy(&output.stdout),
                &String::from_utf8_lossy(&output.stderr),
                mount_path,
            );
        }
        #[cfg(not(unix))]
        {
            let _ = mount_path;
            Ok("unknown".to_string())
        }
    }
}

#[cfg_attr(not(unix), allow(dead_code))]
fn parse_findmnt_fstype_output(
    command_success: bool,
    stdout: &str,
    stderr: &str,
    mount_path: &Path,
) -> anyhow::Result<String> {
    if !command_success {
        return Err(anyhow!(
            "findmnt failed for {}: {}",
            mount_path.display(),
            stderr.trim()
        ));
    }

    let filesystems = stdout.split_whitespace().collect::<Vec<_>>();
    if filesystems.is_empty() {
        return Err(anyhow!(
            "findmnt returned empty filesystem type for {}",
            mount_path.display()
        ));
    }
    if filesystems.iter().all(|value| *value == "ext4") {
        return Ok("ext4".to_string());
    }
    Err(anyhow!(
        "findmnt returned non-ext4 filesystem type for {}: {}",
        mount_path.display(),
        filesystems.join(",")
    ))
}

#[derive(Clone)]
pub struct ProductionCenterReinitializeControlService {
    pool: PgPool,
    template: DiskInfoTemplate,
    security: CenterSecurity,
    filesystem_probe: std::sync::Arc<dyn FileSystemProbe>,
}

impl ProductionCenterReinitializeControlService {
    pub fn new(pool: PgPool, template: DiskInfoTemplate, security: CenterSecurity) -> Self {
        Self {
            pool,
            template,
            security,
            filesystem_probe: std::sync::Arc::new(FindmntFileSystemProbe),
        }
    }
}

impl CenterReinitializeControlService for ProductionCenterReinitializeControlService {
    fn reinitialize_disk<'a>(
        &'a self,
        disk_id: Uuid,
        request: CenterReinitializeRequest,
    ) -> CenterReinitializeFuture<'a> {
        Box::pin(async move {
            validate_request(&request)?;
            let mount_path = normalize_mount_path(&request.mount_path)?;
            let filesystem_type = self.filesystem_probe.filesystem_type(&mount_path)?;
            let pool = self.pool.clone();
            let template = self.template.clone();
            let security = self.security.clone();
            let handle = Handle::current();

            task::spawn_blocking(move || {
                let repo = PgPostImportRepository::new(pool, handle);
                run_reinitialize_with_repo(
                    repo,
                    template,
                    security,
                    disk_id,
                    request,
                    mount_path,
                    filesystem_type,
                )
            })
            .await
            .context("join center reinitialize worker")?
        })
    }
}

fn validate_request(request: &CenterReinitializeRequest) -> anyhow::Result<ReinitializeMode> {
    if !request.confirm_reinitialize {
        return Err(anyhow!("confirm_reinitialize must be true"));
    }
    if request.operator_reason.trim().is_empty() {
        return Err(anyhow!("operator_reason is required for audit"));
    }
    match request.expected_status_code {
        DiskStatusCode::Imported => {
            if request.confirm_discard_sealed_export {
                return Err(anyhow!(
                    "confirm_discard_sealed_export is only valid with expected_status_code=SEALED"
                ));
            }
            Ok(ReinitializeMode::ImportedCleanup)
        }
        DiskStatusCode::Sealed => {
            if !request.confirm_discard_sealed_export {
                return Err(anyhow!(
                    "confirm_discard_sealed_export must be true to discard SEALED export payload"
                ));
            }
            Ok(ReinitializeMode::DiscardSealedExport)
        }
        other => Err(anyhow!(
            "expected_status_code must be IMPORTED or SEALED for cleanup/reinitialize, got {}",
            other.as_str()
        )),
    }
}

fn run_reinitialize_with_repo<R>(
    repo: R,
    template: DiskInfoTemplate,
    security: CenterSecurity,
    disk_id: Uuid,
    request: CenterReinitializeRequest,
    mount_path: PathBuf,
    filesystem_type: String,
) -> anyhow::Result<CenterReinitializeResponse>
where
    R: PostImportRepository,
{
    let mode = validate_request(&request)?;
    let disk_info_document = validate_reinitialize_preflight(
        &mount_path,
        disk_id,
        request.seal_id,
        request.expected_status_code,
        &filesystem_type,
        &security,
        mode,
    )?;

    let mut reinitializer = PostImportReinitializer::new(repo, template, security);
    let output = match mode {
        ReinitializeMode::ImportedCleanup => reinitializer
            .reinitialize_imported_disk_from_document(
                &mount_path,
                disk_id,
                request.seal_id,
                disk_info_document,
            )?,
        ReinitializeMode::DiscardSealedExport => reinitializer
            .discard_sealed_export_and_reinitialize_from_document(
                &mount_path,
                disk_id,
                request.seal_id,
                disk_info_document,
            )?,
    };
    Ok(response_from_output(output))
}

fn response_from_output(output: ReinitializedDisk) -> CenterReinitializeResponse {
    CenterReinitializeResponse {
        disk_id: output.disk_id,
        old_seal_id: output.old_seal_id,
        old_data_key_id: output.old_data_key_id,
        new_data_key_id: output.new_data_key_id,
        disk_status_code: DiskStatusCode::Initialized,
        runtime_status: RuntimeStatus::Done.as_str().to_string(),
        message: "imported disk cleaned and reinitialized".to_string(),
    }
}

fn validate_reinitialize_preflight(
    mount_path: &Path,
    disk_id: Uuid,
    seal_id: Uuid,
    expected_status_code: DiskStatusCode,
    filesystem_type: &str,
    security: &CenterSecurity,
    mode: ReinitializeMode,
) -> anyhow::Result<DiskInfoDocument> {
    if filesystem_type != "ext4" {
        return Err(anyhow!(
            "filesystem unsupported for cleanup/reinitialize: expected ext4, got {filesystem_type}"
        ));
    }

    let disk_info_document = crate::reinitializer::read_disk_info_document(mount_path)?;
    let disk_info = &disk_info_document.disk_info;
    if disk_info.disk.disk_id != disk_id {
        return Err(anyhow!(
            "disk identity mismatch: path disk_id={} request disk_id={disk_id}",
            disk_info.disk.disk_id
        ));
    }
    if disk_info.status.code != expected_status_code {
        return Err(anyhow!(
            "disk_status_code must be {}, got {}",
            expected_status_code.as_str(),
            disk_info.status.code.as_str()
        ));
    }
    match mode {
        ReinitializeMode::ImportedCleanup if disk_info.status.code != DiskStatusCode::Imported => {
            return Err(anyhow!(
                "disk_status_code must be IMPORTED before cleanup/reinitialize"
            ));
        }
        ReinitializeMode::DiscardSealedExport
            if disk_info.status.code != DiskStatusCode::Sealed =>
        {
            return Err(anyhow!(
                "disk_status_code must be SEALED before discarding sealed export"
            ));
        }
        _ => {}
    }
    let actual_seal_id = disk_info
        .edge
        .as_ref()
        .and_then(crate::reinitializer::EdgeSealInfo::seal_id_uuid);
    if actual_seal_id != Some(seal_id) {
        return Err(anyhow!(
            "seal_id mismatch: expected {seal_id}, got {actual_seal_id:?}"
        ));
    }
    validate_center_signature_for_reinitialize(&disk_info_document, security)?;
    reject_partials(mount_path)?;
    if mode == ReinitializeMode::DiscardSealedExport {
        validate_sealed_payload_self_consistent(mount_path, disk_info)?;
    }
    Ok(disk_info_document)
}

fn validate_sealed_payload_self_consistent(
    mount_path: &Path,
    disk_info: &crate::reinitializer::DiskInfo,
) -> anyhow::Result<()> {
    if !disk_info.status.sealed || disk_info.status.imported || disk_info.status.reusable {
        return Err(anyhow!(
            "sealed discard requires SEALED lifecycle flags sealed=true imported=false reusable=false"
        ));
    }
    let manifest_ref = disk_info
        .manifest
        .as_ref()
        .ok_or_else(|| anyhow!("sealed discard requires disk_info manifest reference"))?;
    let edge = disk_info
        .edge
        .as_ref()
        .ok_or_else(|| anyhow!("sealed discard requires disk_info edge seal reference"))?;
    let protocol_root = mount_path.join(PROTOCOL_ROOT);
    let manifest_path = checked_protocol_path(&protocol_root, &manifest_ref.manifest_path)?;
    let manifest_sha_path =
        checked_protocol_path(&protocol_root, &manifest_ref.manifest_sha256_path)?;
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("read manifest {}", manifest_path.display()))?;
    let actual_manifest_sha = hex::encode(Sha256::digest(&manifest_bytes));
    let sidecar_sha = std::fs::read_to_string(&manifest_sha_path)
        .with_context(|| format!("read manifest sha256 {}", manifest_sha_path.display()))?
        .trim()
        .to_string();
    if sidecar_sha != actual_manifest_sha || manifest_ref.manifest_sha256 != actual_manifest_sha {
        return Err(anyhow!(
            "sealed discard manifest sha256 must match disk_info and sidecar"
        ));
    }
    let manifest: SealedExportManifest = serde_json::from_slice(&manifest_bytes)
        .context("parse sealed export manifest for discard gate")?;
    if manifest.manifest_version != "1.0.0"
        || manifest.disk_id != disk_info.disk.disk_id
        || manifest.seal_id.to_string() != edge.seal_id
        || manifest.export_job_id.to_string() != edge.export_job_id
        || manifest.edge_code != edge.edge_code
    {
        return Err(anyhow!(
            "sealed discard manifest identity fields must match disk_info"
        ));
    }
    let total_bytes = manifest
        .objects
        .iter()
        .map(SealedManifestObject::progress_bytes)
        .sum::<u64>();
    if manifest.objects.len() as u64 != manifest_ref.object_count
        || total_bytes != manifest_ref.total_bytes
    {
        return Err(anyhow!(
            "sealed discard manifest counters must match disk_info"
        ));
    }
    for object in &manifest.objects {
        checked_protocol_path(&protocol_root, &object.relative_data_path)?
            .try_exists()
            .with_context(|| format!("check data path {}", object.relative_data_path))?
            .then_some(())
            .ok_or_else(|| anyhow!("sealed discard manifest data path is missing"))?;
        checked_protocol_path(&protocol_root, &object.relative_meta_path)?
            .try_exists()
            .with_context(|| format!("check meta path {}", object.relative_meta_path))?
            .then_some(())
            .ok_or_else(|| anyhow!("sealed discard manifest meta path is missing"))?;
    }
    Ok(())
}

fn checked_protocol_path(root: &Path, relative: &str) -> anyhow::Result<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute() {
        return Err(anyhow!("protocol path must be relative"));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(anyhow!("protocol path must stay inside protocol root"));
    }
    Ok(root.join(path))
}

#[derive(Debug, Deserialize)]
struct SealedExportManifest {
    manifest_version: String,
    seal_id: Uuid,
    export_job_id: Uuid,
    disk_id: Uuid,
    edge_code: String,
    objects: Vec<SealedManifestObject>,
}

#[derive(Debug, Deserialize)]
struct SealedManifestObject {
    relative_data_path: String,
    #[serde(default)]
    chunked: bool,
    #[serde(default)]
    chunk_size_bytes: u64,
    size_bytes: u64,
    relative_meta_path: String,
}

impl SealedManifestObject {
    fn progress_bytes(&self) -> u64 {
        if self.chunked {
            self.chunk_size_bytes
        } else {
            self.size_bytes
        }
    }
}

fn reject_partials(mount_path: &Path) -> anyhow::Result<()> {
    let root = mount_path.join(PROTOCOL_ROOT);
    let mut count = 0_usize;
    scan_partials_in(&root, &mut count)?;
    if count > 0 {
        return Err(anyhow!(
            "partial residue blocks cleanup/reinitialize: {count} .partial files found"
        ));
    }
    Ok(())
}

fn scan_partials_in(path: &Path, count: &mut usize) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in
        std::fs::read_dir(path).with_context(|| format!("scan partials {}", path.display()))?
    {
        let entry = entry.with_context(|| format!("scan partials {}", path.display()))?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("scan partials {}", entry_path.display()))?;
        if file_type.is_dir() {
            scan_partials_in(&entry_path, count)?;
            continue;
        }
        if entry_path.extension().and_then(|value| value.to_str()) == Some("partial") {
            *count += 1;
        }
    }
    Ok(())
}

fn normalize_mount_path(input: &Path) -> anyhow::Result<PathBuf> {
    if input.join(PROTOCOL_ROOT).join(DISK_INFO_FILE).exists() {
        return Ok(input.to_path_buf());
    }
    if input.file_name().and_then(|value| value.to_str()) == Some(PROTOCOL_ROOT)
        && input.join(DISK_INFO_FILE).exists()
    {
        return input
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| anyhow!("protocol root has no mount parent"));
    }
    Ok(input.to_path_buf())
}

#[derive(Clone)]
struct PgPostImportRepository {
    pool: PgPool,
    handle: Handle,
}

impl PgPostImportRepository {
    fn new(pool: PgPool, handle: Handle) -> Self {
        Self { pool, handle }
    }
}

impl PostImportRepository for PgPostImportRepository {
    fn registered_disk(
        &mut self,
        disk_id: Uuid,
    ) -> Result<Option<RegisteredDiskIdentity>, ReinitializeError> {
        self.handle.block_on(async {
            let row = sqlx::query(
                r#"
                    SELECT disk_id, sn
                    FROM disk_list
                    WHERE disk_id = $1
                      AND status = TRUE
                    "#,
            )
            .bind(disk_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| ReinitializeError::Repository(err.to_string()))?;
            Ok(row.map(|row| RegisteredDiskIdentity {
                disk_id: row.get("disk_id"),
                sn: row.get("sn"),
            }))
        })
    }

    fn set_runtime(
        &mut self,
        disk_id: Uuid,
        runtime_status: RuntimeStatus,
        last_error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), ReinitializeError> {
        if runtime_status == RuntimeStatus::Error {
            tracing::error!(
                disk_id = %disk_id,
                runtime_status = runtime_status.as_str(),
                last_error_code = last_error_code,
                error_message = error_message,
                "center post-import reinitialize failed"
            );
        } else {
            tracing::info!(
                disk_id = %disk_id,
                runtime_status = runtime_status.as_str(),
                "center post-import reinitialize runtime update"
            );
        }
        Ok(())
    }

    fn stage_new_data_key(
        &mut self,
        disk_id: Uuid,
        data_key: &NewDataKey,
    ) -> Result<(), ReinitializeError> {
        self.handle.block_on(async {
            sqlx::query(
                r#"
                INSERT INTO data_key(
                    data_key_id, disk_id, encryption_alg, encrypted_key, key_wrap_alg,
                    status, create_time, remark
                )
                VALUES ($1, $2, $3, $4, $5, 'REVOKED', $6,
                    'post-import reinitialize staging: not issuable until disk_info write succeeds')
                "#,
            )
            .bind(data_key.data_key_id)
            .bind(disk_id)
            .bind(&data_key.encryption_alg)
            .bind(&data_key.encrypted_key)
            .bind(&data_key.key_wrap_alg)
            .bind(Utc::now().naive_utc())
            .execute(&self.pool)
            .await
            .map_err(|err| ReinitializeError::Repository(err.to_string()))?;
            Ok(())
        })
    }

    fn abort_staged_data_key(&mut self, data_key_id: Uuid) -> Result<(), ReinitializeError> {
        self.handle.block_on(async {
            sqlx::query(
                r#"
                UPDATE data_key
                SET status = 'REVOKED',
                    remark = 'post-import reinitialize aborted before activation'
                WHERE data_key_id = $1
                  AND status = 'REVOKED'
                "#,
            )
            .bind(data_key_id)
            .execute(&self.pool)
            .await
            .map_err(|err| ReinitializeError::Repository(err.to_string()))?;
            Ok(())
        })
    }

    fn activate_new_key(
        &mut self,
        disk_id: Uuid,
        new_data_key_id: Uuid,
    ) -> Result<(), ReinitializeError> {
        self.handle.block_on(async {
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|err| ReinitializeError::Repository(err.to_string()))?;

            let activated = sqlx::query(
                r#"
                UPDATE data_key
                SET status = 'ACTIVE',
                    activate_time = (NOW() AT TIME ZONE 'UTC'),
                    remark = NULL
                WHERE data_key_id = $1
                  AND disk_id = $2
                  AND status = 'REVOKED'
                  AND edge_code IS NULL
                  AND export_job_id IS NULL
                  AND seal_id IS NULL
                "#,
            )
            .bind(new_data_key_id)
            .bind(disk_id)
            .execute(&mut *tx)
            .await
            .map_err(|err| ReinitializeError::Repository(err.to_string()))?;
            if activated.rows_affected() != 1 {
                return Err(ReinitializeError::Repository(
                    "new data key was not eligible for activation".to_string(),
                ));
            }

            tx.commit()
                .await
                .map_err(|err| ReinitializeError::Repository(err.to_string()))?;
            Ok(())
        })
    }

    fn rollback_new_key_activation(
        &mut self,
        disk_id: Uuid,
        new_data_key_id: Uuid,
    ) -> Result<(), ReinitializeError> {
        self.handle.block_on(async {
            sqlx::query(
                r#"
                UPDATE data_key
                SET status = 'REVOKED',
                    activate_time = NULL,
                    remark = 'post-import reinitialize disk_info write failed after activation'
                WHERE data_key_id = $1
                  AND disk_id = $2
                  AND status = 'ACTIVE'
                  AND edge_code IS NULL
                  AND export_job_id IS NULL
                  AND seal_id IS NULL
                "#,
            )
            .bind(new_data_key_id)
            .bind(disk_id)
            .execute(&self.pool)
            .await
            .map_err(|err| ReinitializeError::Repository(err.to_string()))?;
            Ok(())
        })
    }

    fn retire_old_key(
        &mut self,
        disk_id: Uuid,
        old_data_key_id: Uuid,
    ) -> Result<(), ReinitializeError> {
        self.handle.block_on(async {
            sqlx::query(
                r#"
                UPDATE disk_list
                SET last_init_time = (NOW() AT TIME ZONE 'UTC')
                WHERE disk_id = $1
                "#,
            )
            .bind(disk_id)
            .execute(&self.pool)
            .await
            .map_err(|err| ReinitializeError::Repository(err.to_string()))?;

            let retired = sqlx::query(
                r#"
                UPDATE data_key
                SET status = 'RETIRED',
                    retire_time = (NOW() AT TIME ZONE 'UTC'),
                    remark = 'retired after successful post-import reinitialize'
                WHERE data_key_id = $1
                  AND disk_id = $2
                  AND status IN ('ISSUED', 'SEALED_READONLY')
                "#,
            )
            .bind(old_data_key_id)
            .bind(disk_id)
            .execute(&self.pool)
            .await
            .map_err(|err| ReinitializeError::Repository(err.to_string()))?;
            if retired.rows_affected() != 1 {
                return Err(ReinitializeError::Repository(
                    "old data key was not eligible for retirement".to_string(),
                ));
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::center_security::SIGNATURE_ALG_HMAC_SHA256;
    use crate::reinitializer::{
        CenterInfo, DiskIdentity, DiskInfo, EdgeSealInfo, ManifestInfo, ProtocolInfo, SecurityInfo,
    };
    use sha2::{Digest, Sha256};
    use std::{
        collections::HashMap,
        fs,
        sync::{Arc, Mutex},
    };

    const EXPORT_MANIFEST: &str = "manifests/export_manifest.json";
    const EXPORT_MANIFEST_SHA256: &str = "manifests/export_manifest.sha256";

    #[derive(Default)]
    struct MemoryRepo {
        registered_disk: Option<RegisteredDiskIdentity>,
        runtime: Vec<RuntimeStatus>,
        staged_keys: HashMap<Uuid, NewDataKey>,
        active_key: Option<Uuid>,
        retired_key: Option<Uuid>,
        fail_activate: bool,
        fail_retire_old_key: bool,
    }

    #[derive(Clone, Default)]
    struct SharedRepo(Arc<Mutex<MemoryRepo>>);

    impl PostImportRepository for SharedRepo {
        fn registered_disk(
            &mut self,
            disk_id: Uuid,
        ) -> Result<Option<RegisteredDiskIdentity>, ReinitializeError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .registered_disk
                .clone()
                .filter(|registered| registered.disk_id == disk_id))
        }

        fn set_runtime(
            &mut self,
            _disk_id: Uuid,
            runtime_status: RuntimeStatus,
            _last_error_code: Option<&str>,
            _error_message: Option<&str>,
        ) -> Result<(), ReinitializeError> {
            self.0.lock().unwrap().runtime.push(runtime_status);
            Ok(())
        }

        fn stage_new_data_key(
            &mut self,
            _disk_id: Uuid,
            data_key: &NewDataKey,
        ) -> Result<(), ReinitializeError> {
            self.0
                .lock()
                .unwrap()
                .staged_keys
                .insert(data_key.data_key_id, data_key.clone());
            Ok(())
        }

        fn abort_staged_data_key(&mut self, data_key_id: Uuid) -> Result<(), ReinitializeError> {
            self.0.lock().unwrap().staged_keys.remove(&data_key_id);
            Ok(())
        }

        fn activate_new_key(
            &mut self,
            _disk_id: Uuid,
            new_data_key_id: Uuid,
        ) -> Result<(), ReinitializeError> {
            if self.0.lock().unwrap().fail_activate {
                return Err(ReinitializeError::Repository("activate failed".to_string()));
            }
            let mut guard = self.0.lock().unwrap();
            guard.active_key = Some(new_data_key_id);
            Ok(())
        }

        fn rollback_new_key_activation(
            &mut self,
            _disk_id: Uuid,
            new_data_key_id: Uuid,
        ) -> Result<(), ReinitializeError> {
            let mut guard = self.0.lock().unwrap();
            if guard.active_key == Some(new_data_key_id) {
                guard.active_key = None;
            }
            Ok(())
        }

        fn retire_old_key(
            &mut self,
            _disk_id: Uuid,
            old_data_key_id: Uuid,
        ) -> Result<(), ReinitializeError> {
            if self.0.lock().unwrap().fail_retire_old_key {
                return Err(ReinitializeError::Repository("retire failed".to_string()));
            }
            let mut guard = self.0.lock().unwrap();
            guard.retired_key = Some(old_data_key_id);
            Ok(())
        }
    }

    #[test]
    fn findmnt_fstype_parser_accepts_single_ext4() {
        let fs_type =
            parse_findmnt_fstype_output(true, "ext4\n", "", Path::new("/media/control/disk"))
                .unwrap();

        assert_eq!(fs_type, "ext4");
    }

    #[test]
    fn findmnt_fstype_parser_accepts_repeated_ext4_lines() {
        let fs_type =
            parse_findmnt_fstype_output(true, "ext4\next4\n", "", Path::new("/media/control/disk"))
                .unwrap();

        assert_eq!(fs_type, "ext4");
    }

    #[test]
    fn findmnt_fstype_parser_accepts_multiline_whitespace_ext4() {
        let fs_type = parse_findmnt_fstype_output(
            true,
            "\n  ext4  \n\n\t ext4 \r\n",
            "",
            Path::new("/media/control/disk"),
        )
        .unwrap();

        assert_eq!(fs_type, "ext4");
    }

    #[test]
    fn findmnt_fstype_parser_rejects_mixed_filesystems() {
        let error =
            parse_findmnt_fstype_output(true, "ext4\nxfs\n", "", Path::new("/media/control/disk"))
                .unwrap_err();

        assert!(error.to_string().contains("non-ext4"));
    }

    #[test]
    fn findmnt_fstype_parser_rejects_empty_output() {
        let error =
            parse_findmnt_fstype_output(true, " \n\t\n", "", Path::new("/media/control/disk"))
                .unwrap_err();

        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn findmnt_fstype_parser_rejects_command_failure() {
        let error = parse_findmnt_fstype_output(
            false,
            "ext4\n",
            "not mounted",
            Path::new("/media/control/disk"),
        )
        .unwrap_err();

        assert!(error.to_string().contains("findmnt failed"));
    }

    #[test]
    fn current_center_signature_allows_reinitialize_preflight() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, old_key);

        validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Imported,
            "ext4",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap();
    }

    #[test]
    fn legacy_raw_imported_signature_without_updated_at_allows_reinitialize_preflight() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, old_key);
        write_legacy_raw_imported_missing_updated_at_signature(&temp);

        validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Imported,
            "ext4",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap();
    }

    #[test]
    fn full_protocol_imported_signature_allows_reinitialize_preflight() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        let disk_info = write_full_protocol_imported_disk(&temp, disk_id, seal_id, old_key);

        crate::center_security::verify_disk_info_with_key(
            &disk_info,
            &security().center_signature_key_bytes(),
        )
        .unwrap();
        validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Imported,
            "ext4",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap();
    }

    #[test]
    fn signature_rejects_missing_wrong_signature_and_wrong_key_before_writing() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, old_key);
        let disk_info_path = temp.root().join(DISK_INFO_FILE);
        let original: serde_json::Value =
            serde_json::from_slice(&fs::read(&disk_info_path).unwrap()).unwrap();

        for signature in ["", "wrong-signature"] {
            let mut value = original.clone();
            value["security"]["center_signature"] =
                serde_json::Value::String(signature.to_string());
            fs::write(&disk_info_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

            let error = validate_reinitialize_preflight(
                &temp.path,
                disk_id,
                seal_id,
                DiskStatusCode::Imported,
                "ext4",
                &security(),
                ReinitializeMode::ImportedCleanup,
            )
            .unwrap_err();

            assert!(error.to_string().contains("center_signature"));
            assert_eq!(
                crate::reinitializer::read_disk_info(&temp.path)
                    .unwrap()
                    .status
                    .code,
                DiskStatusCode::Imported
            );
        }

        let mut value = original.clone();
        let stale_signature = value["security"]["center_signature"]
            .as_str()
            .unwrap()
            .to_string();
        value["security"]["center_key_id"] = serde_json::Value::String(Uuid::new_v4().to_string());
        value["security"]["center_signature"] = serde_json::Value::String(stale_signature);
        fs::write(&disk_info_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let error = validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Imported,
            "ext4",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("center_signature verification failed"));
    }

    #[test]
    fn legacy_signature_ignores_updated_at_but_requires_imported_shape() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, old_key);
        let legacy_value = write_legacy_raw_imported_missing_updated_at_signature(&temp);
        let disk_info_path = temp.root().join(DISK_INFO_FILE);

        let mut with_updated_at = legacy_value.clone();
        with_updated_at["updated_at"] = serde_json::Value::String(Utc::now().to_rfc3339());
        fs::write(
            &disk_info_path,
            serde_json::to_vec_pretty(&with_updated_at).unwrap(),
        )
        .unwrap();
        validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Imported,
            "ext4",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap();

        let mut sealed = legacy_value;
        sealed["status"]["code"] = serde_json::Value::String("SEALED".to_string());
        fs::write(&disk_info_path, serde_json::to_vec_pretty(&sealed).unwrap()).unwrap();
        let error = validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Imported,
            "ext4",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap_err();
        assert!(error.to_string().contains("updated_at") || error.to_string().contains("json"));
    }

    #[test]
    fn successful_cleanup_reinitialize_cleans_payload_and_rotates_key() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, old_key);
        fs::create_dir_all(temp.root().join("data")).unwrap();
        fs::write(temp.root().join("data/object.enc"), b"ciphertext").unwrap();
        fs::create_dir_all(temp.root().join("meta")).unwrap();
        fs::write(temp.root().join("meta/object.json"), b"{}").unwrap();

        let repo = SharedRepo::default();
        repo.0.lock().unwrap().registered_disk = Some(registered_disk(disk_id));

        let response = run_reinitialize_with_repo(
            repo.clone(),
            template(),
            security(),
            disk_id,
            request(&temp.path, seal_id),
            temp.path.clone(),
            "ext4".to_string(),
        )
        .unwrap();

        assert_eq!(response.disk_status_code, DiskStatusCode::Initialized);
        assert_eq!(response.runtime_status, "DONE");
        let disk_info = crate::reinitializer::read_disk_info(&temp.path).unwrap();
        assert_eq!(disk_info.status.code, DiskStatusCode::Initialized);
        let edge = disk_info
            .edge
            .as_ref()
            .expect("initialized disk_info uses full edge object");
        assert!(edge.seal_id.is_empty());
        assert!(edge.export_job_id.is_empty());
        let manifest = disk_info
            .manifest
            .as_ref()
            .expect("initialized disk_info uses full manifest object");
        assert_eq!(manifest.object_count, 0);
        assert_eq!(manifest.total_bytes, 0);
        assert!(!temp.root().join("data/object.enc").exists());
        assert!(!temp.root().join("meta/object.json").exists());
        assert!(temp.root().join("quarantine/partial").is_dir());

        let guard = repo.0.lock().unwrap();
        assert_eq!(guard.retired_key, Some(old_key));
        assert_eq!(guard.active_key, Some(response.new_data_key_id));
        let staged = guard.staged_keys.get(&response.new_data_key_id).unwrap();
        assert!(staged.encrypted_key.starts_with("local-master-key:v1:"));
    }

    #[test]
    fn minimal_admission_reinitializes_without_manifest_or_import_job_gate() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, old_key);
        fs::remove_dir_all(temp.root().join("manifests")).unwrap();
        fs::create_dir_all(temp.root().join("data")).unwrap();
        fs::write(temp.root().join("data/object.enc"), b"ciphertext").unwrap();

        let repo = SharedRepo::default();
        repo.0.lock().unwrap().registered_disk = Some(registered_disk(disk_id));

        let response = run_reinitialize_with_repo(
            repo.clone(),
            template(),
            security(),
            disk_id,
            request(&temp.path, seal_id),
            temp.path.clone(),
            "ext4".to_string(),
        )
        .unwrap();

        assert_eq!(response.disk_status_code, DiskStatusCode::Initialized);
        let disk_info = crate::reinitializer::read_disk_info(&temp.path).unwrap();
        assert_eq!(disk_info.status.code, DiskStatusCode::Initialized);
        assert_eq!(
            disk_info
                .manifest
                .as_ref()
                .expect("initialized disk_info uses full manifest object")
                .object_count,
            0
        );
        assert!(!temp.root().join("data/object.enc").exists());
        let guard = repo.0.lock().unwrap();
        assert_eq!(guard.active_key, Some(response.new_data_key_id));
        assert_eq!(guard.retired_key, Some(old_key));
    }

    #[test]
    fn legacy_missing_updated_at_signature_reinitializes_through_runtime_and_core() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, old_key);
        write_legacy_raw_imported_missing_updated_at_signature(&temp);
        fs::create_dir_all(temp.root().join("data")).unwrap();
        fs::write(temp.root().join("data/object.enc"), b"ciphertext").unwrap();

        let repo = SharedRepo::default();
        repo.0.lock().unwrap().registered_disk = Some(registered_disk(disk_id));

        let response = run_reinitialize_with_repo(
            repo.clone(),
            template(),
            security(),
            disk_id,
            request(&temp.path, seal_id),
            temp.path.clone(),
            "ext4".to_string(),
        )
        .unwrap();

        assert_eq!(response.disk_status_code, DiskStatusCode::Initialized);
        let disk_info = crate::reinitializer::read_disk_info(&temp.path).unwrap();
        assert_eq!(disk_info.status.code, DiskStatusCode::Initialized);
        assert!(security().verify_disk_info(&disk_info).is_ok());
        let raw: serde_json::Value =
            serde_json::from_slice(&fs::read(temp.root().join(DISK_INFO_FILE)).unwrap()).unwrap();
        assert!(raw.get("updated_at").is_some());
        assert!(!temp.root().join("data/object.enc").exists());

        let guard = repo.0.lock().unwrap();
        assert_eq!(
            guard.runtime,
            vec![
                RuntimeStatus::Cleaning,
                RuntimeStatus::Reinitializing,
                RuntimeStatus::Done,
            ]
        );
        assert_eq!(guard.retired_key, Some(old_key));
        assert_eq!(guard.active_key, Some(response.new_data_key_id));
    }

    #[test]
    fn imported_status_change_without_resigning_passes_outer_and_inner_verifiers() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        let mut disk_info = imported_disk_info(disk_id, seal_id, old_key);
        disk_info.edge = None;
        disk_info.manifest = None;
        disk_info.status = crate::reinitializer::DiskStatus {
            code: DiskStatusCode::Initialized,
            sealed: false,
            imported: false,
            reusable: true,
            last_error: None,
        };
        disk_info.security.center_signature = String::new();
        disk_info.security.center_signature = security().sign_disk_info(&disk_info).unwrap();

        disk_info.edge = Some(EdgeSealInfo {
            edge_name: "edge-a".to_string(),
            edge_code: "edge-a".to_string(),
            seal_id: seal_id.to_string(),
            export_job_id: Uuid::new_v4().to_string(),
            export_started_at: "2026-08-11T00:00:00Z".to_string(),
            export_finished_at: "2026-08-11T00:01:00Z".to_string(),
        });
        disk_info.manifest = Some(ManifestInfo {
            manifest_path: EXPORT_MANIFEST.to_string(),
            manifest_sha256_path: EXPORT_MANIFEST_SHA256.to_string(),
            object_count: 1,
            total_bytes: 1024,
            manifest_sha256: "manifest-after-import".to_string(),
        });
        disk_info.status = crate::reinitializer::DiskStatus {
            code: DiskStatusCode::Imported,
            sealed: true,
            imported: true,
            reusable: true,
            last_error: None,
        };
        crate::reinitializer::write_disk_info(&temp.path, &disk_info).unwrap();
        fs::create_dir_all(temp.root().join("data")).unwrap();
        fs::write(temp.root().join("data/object.enc"), b"ciphertext").unwrap();

        validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Imported,
            "ext4",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap();

        let repo = SharedRepo::default();
        repo.0.lock().unwrap().registered_disk = Some(registered_disk(disk_id));
        let response = run_reinitialize_with_repo(
            repo.clone(),
            template(),
            security(),
            disk_id,
            request(&temp.path, seal_id),
            temp.path.clone(),
            "ext4".to_string(),
        )
        .unwrap();

        assert_eq!(response.disk_status_code, DiskStatusCode::Initialized);
        assert!(!temp.root().join("data/object.enc").exists());
        let guard = repo.0.lock().unwrap();
        assert_eq!(guard.retired_key, Some(old_key));
        assert_eq!(guard.active_key, Some(response.new_data_key_id));
    }

    #[test]
    fn runtime_bad_signature_rejects_before_core_writes_or_repo_updates() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, old_key);
        fs::create_dir_all(temp.root().join("data")).unwrap();
        fs::write(temp.root().join("data/object.enc"), b"ciphertext").unwrap();

        let disk_info_path = temp.root().join(DISK_INFO_FILE);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&disk_info_path).unwrap()).unwrap();
        value["security"]["center_signature"] =
            serde_json::Value::String("wrong-signature".to_string());
        fs::write(&disk_info_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let original_disk_info_bytes = fs::read(&disk_info_path).unwrap();

        let repo = SharedRepo::default();
        repo.0.lock().unwrap().registered_disk = Some(registered_disk(disk_id));

        let error = run_reinitialize_with_repo(
            repo.clone(),
            template(),
            security(),
            disk_id,
            request(&temp.path, seal_id),
            temp.path.clone(),
            "ext4".to_string(),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("center_signature verification failed"));
        assert_eq!(fs::read(&disk_info_path).unwrap(), original_disk_info_bytes);
        assert!(temp.root().join("data/object.enc").exists());
        let guard = repo.0.lock().unwrap();
        assert!(guard.runtime.is_empty());
        assert!(guard.staged_keys.is_empty());
        assert_eq!(guard.active_key, None);
        assert_eq!(guard.retired_key, None);
    }

    #[test]
    fn partial_residue_rejects_before_cleanup() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, Uuid::new_v4());
        fs::create_dir_all(temp.root().join("data")).unwrap();
        fs::write(temp.root().join("data/object.enc"), b"ciphertext").unwrap();
        fs::write(temp.root().join("data/object.enc.partial"), b"incomplete").unwrap();

        let error = validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Imported,
            "ext4",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap_err();

        assert!(error.to_string().contains("partial residue blocks"));
        assert!(temp.root().join("data/object.enc").exists());
        assert!(temp.root().join("data/object.enc.partial").exists());
    }

    #[test]
    fn legacy_missing_updated_at_ignores_manifest_as_admission_gate() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, old_key);

        let disk_info_path = temp.root().join(DISK_INFO_FILE);
        write_legacy_raw_imported_missing_updated_at_signature(&temp);
        fs::write(temp.root().join(EXPORT_MANIFEST_SHA256), "tampered-sidecar").unwrap();

        validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Imported,
            "ext4",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap();

        let current: serde_json::Value =
            serde_json::from_slice(&fs::read(&disk_info_path).unwrap()).unwrap();
        assert_eq!(current["status"]["code"], "IMPORTED");
        assert!(current.get("updated_at").is_none());
    }

    #[test]
    fn invalid_status_and_signature_are_rejected() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();

        let mut disk_info = imported_disk_info(disk_id, seal_id, old_key);
        disk_info.status.code = DiskStatusCode::Sealed;
        crate::reinitializer::write_disk_info(&temp.path, &disk_info).unwrap();
        write_manifest(&temp, disk_id, seal_id, "{}");
        let error = validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Imported,
            "ext4",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap_err();
        assert!(error.to_string().contains("disk_status_code"));

        disk_info.status.code = DiskStatusCode::Imported;
        disk_info.security.center_signature.clear();
        crate::reinitializer::write_disk_info(&temp.path, &disk_info).unwrap();
        let error = validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Imported,
            "ext4",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap_err();
        assert!(error.to_string().contains("center_signature"));
    }

    #[test]
    fn non_ext4_rejected_before_disk_mutation() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, old_key);

        let error = validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Imported,
            "xfs",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap_err();

        assert!(error.to_string().contains("expected ext4"));
        assert_eq!(
            crate::reinitializer::read_disk_info(&temp.path)
                .unwrap()
                .status
                .code,
            DiskStatusCode::Imported
        );
    }

    #[test]
    fn request_cannot_reinitialize_a_different_disk_identity() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, old_key);

        let error = validate_reinitialize_preflight(
            &temp.path,
            Uuid::new_v4(),
            seal_id,
            DiskStatusCode::Imported,
            "ext4",
            &security(),
            ReinitializeMode::ImportedCleanup,
        )
        .unwrap_err();

        assert!(error.to_string().contains("disk identity mismatch"));
        assert_eq!(
            crate::reinitializer::read_disk_info(&temp.path)
                .unwrap()
                .security
                .data_key_id,
            old_key
        );
    }

    #[test]
    fn response_does_not_serialize_naked_status() {
        let response = response_from_output(ReinitializedDisk {
            disk_id: Uuid::new_v4(),
            old_seal_id: Uuid::new_v4(),
            old_data_key_id: Uuid::new_v4(),
            new_data_key_id: Uuid::new_v4(),
        });
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["disk_status_code"], "INITIALIZED");
        assert_eq!(value["runtime_status"], "DONE");
        assert!(value.get("status").is_none());
    }

    #[test]
    fn runtime_old_key_retirement_failure_does_not_block_reinitialize() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_imported_disk(&temp, disk_id, seal_id, old_key);

        let repo = SharedRepo::default();
        {
            let mut guard = repo.0.lock().unwrap();
            guard.registered_disk = Some(registered_disk(disk_id));
            guard.fail_retire_old_key = true;
        }

        let response = run_reinitialize_with_repo(
            repo.clone(),
            template(),
            security(),
            disk_id,
            request(&temp.path, seal_id),
            temp.path.clone(),
            "ext4".to_string(),
        )
        .unwrap();

        let disk_info = crate::reinitializer::read_disk_info(&temp.path).unwrap();
        assert_eq!(disk_info.status.code, DiskStatusCode::Initialized);
        let guard = repo.0.lock().unwrap();
        assert_eq!(guard.active_key, Some(response.new_data_key_id));
        assert_eq!(guard.retired_key, None);
        assert_eq!(
            guard.runtime,
            vec![
                RuntimeStatus::Cleaning,
                RuntimeStatus::Reinitializing,
                RuntimeStatus::Done,
            ]
        );
    }

    #[test]
    fn sealed_discard_requires_explicit_confirmation() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_sealed_disk(&temp, disk_id, seal_id, old_key);
        let before = fs::read(temp.root().join(DISK_INFO_FILE)).unwrap();

        let mut req = sealed_discard_request(&temp.path, seal_id);
        req.confirm_discard_sealed_export = false;
        let repo = SharedRepo::default();
        repo.0.lock().unwrap().registered_disk = Some(registered_disk(disk_id));

        let error = run_reinitialize_with_repo(
            repo.clone(),
            template(),
            security(),
            disk_id,
            req,
            temp.path.clone(),
            "ext4".to_string(),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("confirm_discard_sealed_export must be true"));
        assert_eq!(fs::read(temp.root().join(DISK_INFO_FILE)).unwrap(), before);
        assert!(temp.root().join("data/object.enc").exists());
        assert!(repo.0.lock().unwrap().runtime.is_empty());
    }

    #[test]
    fn sealed_discard_reinitializes_after_manifest_gate() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_sealed_disk(&temp, disk_id, seal_id, old_key);

        validate_reinitialize_preflight(
            &temp.path,
            disk_id,
            seal_id,
            DiskStatusCode::Sealed,
            "ext4",
            &security(),
            ReinitializeMode::DiscardSealedExport,
        )
        .unwrap();

        let repo = SharedRepo::default();
        repo.0.lock().unwrap().registered_disk = Some(registered_disk(disk_id));

        let response = run_reinitialize_with_repo(
            repo.clone(),
            template(),
            security(),
            disk_id,
            sealed_discard_request(&temp.path, seal_id),
            temp.path.clone(),
            "ext4".to_string(),
        )
        .unwrap();

        assert_eq!(response.disk_status_code, DiskStatusCode::Initialized);
        let disk_info = crate::reinitializer::read_disk_info(&temp.path).unwrap();
        assert_eq!(disk_info.status.code, DiskStatusCode::Initialized);
        assert!(security().verify_disk_info(&disk_info).is_ok());
        assert_eq!(
            disk_info
                .manifest
                .as_ref()
                .expect("initialized disk_info has manifest")
                .object_count,
            0
        );
        assert!(!temp.root().join("data/object.enc").exists());
        assert!(!temp.root().join("meta/object.json").exists());
        assert!(!temp.root().join("manifests/export_manifest.json").exists());
        assert!(temp.root().join("quarantine/partial").is_dir());

        let guard = repo.0.lock().unwrap();
        assert_eq!(
            guard.runtime,
            vec![
                RuntimeStatus::Cleaning,
                RuntimeStatus::Reinitializing,
                RuntimeStatus::Done,
            ]
        );
        assert_eq!(guard.active_key, Some(response.new_data_key_id));
        assert_eq!(guard.retired_key, Some(old_key));
    }

    #[test]
    fn sealed_discard_manifest_mismatch_rejects_before_runtime_or_payload_writes() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_sealed_disk(&temp, disk_id, seal_id, old_key);
        fs::write(temp.root().join(EXPORT_MANIFEST_SHA256), "tampered").unwrap();
        let before = fs::read(temp.root().join(DISK_INFO_FILE)).unwrap();

        let repo = SharedRepo::default();
        repo.0.lock().unwrap().registered_disk = Some(registered_disk(disk_id));
        let error = run_reinitialize_with_repo(
            repo.clone(),
            template(),
            security(),
            disk_id,
            sealed_discard_request(&temp.path, seal_id),
            temp.path.clone(),
            "ext4".to_string(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("manifest sha256"));
        assert_eq!(fs::read(temp.root().join(DISK_INFO_FILE)).unwrap(), before);
        assert!(temp.root().join("data/object.enc").exists());
        assert!(repo.0.lock().unwrap().runtime.is_empty());
    }

    #[test]
    fn sealed_discard_accepts_legacy_missing_protocol_name_with_raw_signature() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_sealed_disk(&temp, disk_id, seal_id, old_key);
        let disk_info_path = temp.root().join(DISK_INFO_FILE);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&disk_info_path).unwrap()).unwrap();
        value["protocol"].as_object_mut().unwrap().remove("name");
        value["security"]["center_signature"] = serde_json::Value::String(String::new());
        value["security"]["center_signature"] =
            serde_json::Value::String(security().sign_disk_info(&value).unwrap());
        fs::write(&disk_info_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let repo = SharedRepo::default();
        repo.0.lock().unwrap().registered_disk = Some(registered_disk(disk_id));
        let response = run_reinitialize_with_repo(
            repo,
            template(),
            security(),
            disk_id,
            sealed_discard_request(&temp.path, seal_id),
            temp.path.clone(),
            "ext4".to_string(),
        )
        .unwrap();

        assert_eq!(response.disk_status_code, DiskStatusCode::Initialized);
        let disk_info = crate::reinitializer::read_disk_info(&temp.path).unwrap();
        assert_eq!(disk_info.status.code, DiskStatusCode::Initialized);
        assert_eq!(
            disk_info.protocol.name,
            crate::disk_info_document::PROTOCOL_NAME
        );
        assert!(security().verify_disk_info(&disk_info).is_ok());
    }

    #[test]
    fn sealed_discard_restores_payload_when_key_activation_fails() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_sealed_disk(&temp, disk_id, seal_id, old_key);
        let before = fs::read(temp.root().join(DISK_INFO_FILE)).unwrap();

        let repo = SharedRepo::default();
        {
            let mut guard = repo.0.lock().unwrap();
            guard.registered_disk = Some(registered_disk(disk_id));
            guard.fail_activate = true;
        }

        let error = run_reinitialize_with_repo(
            repo.clone(),
            template(),
            security(),
            disk_id,
            sealed_discard_request(&temp.path, seal_id),
            temp.path.clone(),
            "ext4".to_string(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("activate failed"));
        assert_eq!(fs::read(temp.root().join(DISK_INFO_FILE)).unwrap(), before);
        assert_eq!(
            crate::reinitializer::read_disk_info(&temp.path)
                .unwrap()
                .status
                .code,
            DiskStatusCode::Sealed
        );
        assert!(temp.root().join("data/object.enc").exists());
        assert!(temp.root().join("meta/object.json").exists());
        assert!(temp.root().join("manifests/export_manifest.json").exists());
        let guard = repo.0.lock().unwrap();
        assert_eq!(guard.active_key, None);
        assert_eq!(guard.retired_key, None);
        assert!(guard.runtime.contains(&RuntimeStatus::Error));
    }

    fn request(mount_path: &Path, seal_id: Uuid) -> CenterReinitializeRequest {
        CenterReinitializeRequest {
            mount_path: mount_path.to_path_buf(),
            seal_id,
            expected_status_code: DiskStatusCode::Imported,
            operator_reason: "vm acceptance cleanup for authorized disk".to_string(),
            confirm_reinitialize: true,
            confirm_discard_sealed_export: false,
        }
    }

    fn sealed_discard_request(mount_path: &Path, seal_id: Uuid) -> CenterReinitializeRequest {
        CenterReinitializeRequest {
            mount_path: mount_path.to_path_buf(),
            seal_id,
            expected_status_code: DiskStatusCode::Sealed,
            operator_reason: "discard sealed test export for fresh end-to-end run".to_string(),
            confirm_reinitialize: true,
            confirm_discard_sealed_export: true,
        }
    }

    fn registered_disk(disk_id: Uuid) -> RegisteredDiskIdentity {
        RegisteredDiskIdentity {
            disk_id,
            sn: Some("SN001".to_string()),
        }
    }

    fn write_imported_disk(temp: &TempDisk, disk_id: Uuid, seal_id: Uuid, old_key: Uuid) {
        crate::reinitializer::write_disk_info(
            &temp.path,
            &imported_disk_info(disk_id, seal_id, old_key),
        )
        .unwrap();
        write_manifest(temp, disk_id, seal_id, "");
    }

    fn write_manifest(temp: &TempDisk, disk_id: Uuid, seal_id: Uuid, sidecar_override: &str) {
        fs::create_dir_all(temp.root().join("manifests")).unwrap();
        let manifest = serde_json::json!({
            "manifest_version": "1.0.0",
            "disk_id": disk_id.to_string(),
            "seal_id": seal_id.to_string(),
            "export_job_id": Uuid::new_v4().to_string(),
            "edge_code": "edge-a",
            "objects": []
        });
        let bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        let sha = sha256_hex(&bytes);
        fs::write(temp.root().join(EXPORT_MANIFEST), bytes).unwrap();
        fs::write(
            temp.root().join(EXPORT_MANIFEST_SHA256),
            if sidecar_override.is_empty() {
                sha.clone()
            } else {
                sidecar_override.to_string()
            },
        )
        .unwrap();
        let mut disk_info = crate::reinitializer::read_disk_info(&temp.path).unwrap();
        disk_info.manifest = Some(ManifestInfo {
            manifest_path: EXPORT_MANIFEST.to_string(),
            manifest_sha256_path: EXPORT_MANIFEST_SHA256.to_string(),
            object_count: 1,
            total_bytes: 1024,
            manifest_sha256: sha,
        });
        disk_info.security.center_signature = String::new();
        disk_info.security.center_signature = security().sign_disk_info(&disk_info).unwrap();
        crate::reinitializer::write_disk_info(&temp.path, &disk_info).unwrap();
    }

    fn write_sealed_disk(temp: &TempDisk, disk_id: Uuid, seal_id: Uuid, old_key: Uuid) {
        let export_job_id = Uuid::new_v4();
        fs::create_dir_all(temp.root().join("data")).unwrap();
        fs::create_dir_all(temp.root().join("meta")).unwrap();
        fs::create_dir_all(temp.root().join("manifests")).unwrap();
        fs::write(temp.root().join("data/object.enc"), b"ciphertext").unwrap();
        fs::write(temp.root().join("meta/object.json"), b"{}").unwrap();

        let manifest = serde_json::json!({
            "manifest_version": "1.0.0",
            "disk_id": disk_id,
            "seal_id": seal_id,
            "export_job_id": export_job_id,
            "edge_code": "edge-a",
            "objects": [{
                "bucket": "test",
                "key": "object.bin",
                "relative_data_path": "data/object.enc",
                "encrypted": true,
                "encryption_alg": "AES-256-GCM",
                "data_key_id": old_key,
                "nonce": "nonce",
                "tag": "tag",
                "aad": "aad",
                "ciphertext_size_bytes": 10,
                "ciphertext_sha256": "ciphertext-sha",
                "chunked": false,
                "chunk_group_id": "",
                "chunk_index": 0,
                "chunk_total": 1,
                "chunk_offset_bytes": 0,
                "chunk_size_bytes": 0,
                "chunk_sha256": "",
                "relative_meta_path": "meta/object.json",
                "size_bytes": 10,
                "etag": "etag",
                "last_modified": "2026-08-11T00:00:00Z",
                "plaintext_sha256": "plaintext-sha",
                "object_status": "EXPORTED"
            }]
        });
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        let manifest_sha = sha256_hex(&manifest_bytes);
        fs::write(temp.root().join(EXPORT_MANIFEST), manifest_bytes).unwrap();
        fs::write(temp.root().join(EXPORT_MANIFEST_SHA256), &manifest_sha).unwrap();

        let signing = security();
        let mut disk_info = imported_disk_info(disk_id, seal_id, old_key);
        disk_info.edge = Some(EdgeSealInfo {
            edge_name: "edge-a".to_string(),
            edge_code: "edge-a".to_string(),
            seal_id: seal_id.to_string(),
            export_job_id: export_job_id.to_string(),
            export_started_at: "2026-08-11T00:00:00Z".to_string(),
            export_finished_at: "2026-08-11T00:01:00Z".to_string(),
        });
        disk_info.manifest = Some(ManifestInfo {
            manifest_path: EXPORT_MANIFEST.to_string(),
            manifest_sha256_path: EXPORT_MANIFEST_SHA256.to_string(),
            object_count: 1,
            total_bytes: 10,
            manifest_sha256: manifest_sha,
        });
        disk_info.status = crate::reinitializer::DiskStatus {
            code: DiskStatusCode::Sealed,
            sealed: true,
            imported: false,
            reusable: false,
            last_error: None,
        };
        disk_info.security.center_signature = String::new();
        disk_info.security.center_signature = signing.sign_disk_info(&disk_info).unwrap();
        crate::reinitializer::write_disk_info(&temp.path, &disk_info).unwrap();
    }

    fn write_full_protocol_imported_disk(
        temp: &TempDisk,
        disk_id: Uuid,
        seal_id: Uuid,
        old_key: Uuid,
    ) -> serde_json::Value {
        let export_job_id = Uuid::new_v4();
        let center_id = Uuid::new_v4();
        let mut disk_info = serde_json::json!({
            "protocol": {
                "name": "rustfs-offline-transfer",
                "version": "1.0.0"
            },
            "disk": {
                "disk_id": disk_id,
                "sn": "SN001",
                "capacity_bytes": 1024,
                "last_init_time": "2026-08-11T00:00:00Z",
                "initialized_by": "center"
            },
            "center": {
                "center_id": center_id,
                "center_name": "center",
                "import_job_id": Uuid::new_v4().to_string(),
                "import_started_at": "2026-08-11T01:00:00Z",
                "import_finished_at": "2026-08-11T01:01:00Z"
            },
            "edge": {
                "edge_code": "edge-a",
                "export_job_id": export_job_id,
                "seal_id": seal_id
            },
            "manifest": {
                "manifest_path": "manifests/export_manifest.json",
                "manifest_sha256_path": "manifests/export_manifest.sha256",
                "object_count": 0,
                "total_bytes": 0,
                "manifest_sha256": "manifest-sha"
            },
            "security": {
                "center_key_id": security().center_key_id(),
                "data_key_id": old_key,
                "encryption_alg": "AES-256-GCM",
                "signature_alg": SIGNATURE_ALG_HMAC_SHA256,
                "center_signature": ""
            },
            "status": {
                "code": "IMPORTED",
                "sealed": true,
                "imported": true,
                "reusable": true,
                "last_error": null
            },
            "updated_at": "2026-08-11T01:01:00Z"
        });
        disk_info["security"]["center_signature"] =
            serde_json::Value::String(security().sign_disk_info(&disk_info).unwrap());
        fs::write(
            temp.root().join(DISK_INFO_FILE),
            serde_json::to_vec_pretty(&disk_info).unwrap(),
        )
        .unwrap();
        disk_info
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn write_legacy_raw_imported_missing_updated_at_signature(
        temp: &TempDisk,
    ) -> serde_json::Value {
        let disk_info_path = temp.root().join(DISK_INFO_FILE);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&disk_info_path).unwrap()).unwrap();
        value["protocol"]["name"] =
            serde_json::Value::String("rustfs-offline-transfer".to_string());
        value.as_object_mut().unwrap().remove("updated_at");
        value["security"]["center_signature"] = serde_json::Value::String(String::new());
        value["security"]["center_signature"] =
            serde_json::Value::String(security().sign_disk_info(&value).unwrap());
        fs::write(&disk_info_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        value
    }

    fn template() -> DiskInfoTemplate {
        let security = security();
        DiskInfoTemplate {
            protocol_version: "1.0".to_string(),
            center_id: Uuid::new_v4(),
            center_name: Some("center".to_string()),
            center_key_id: security.center_key_id(),
            signature_alg: SIGNATURE_ALG_HMAC_SHA256.to_string(),
        }
    }

    fn imported_disk_info(disk_id: Uuid, seal_id: Uuid, data_key_id: Uuid) -> DiskInfo {
        let signing = security();
        let mut disk_info = DiskInfo {
            protocol: ProtocolInfo {
                name: crate::disk_info_document::PROTOCOL_NAME.to_string(),
                version: "1.0".to_string(),
            },
            disk: DiskIdentity {
                disk_id,
                sn: Some("SN001".to_string()),
                capacity_bytes: 1024,
                last_init_time: "2026-08-11T00:00:00Z".to_string(),
                initialized_by: "center".to_string(),
            },
            center: CenterInfo {
                center_id: Uuid::new_v4(),
                center_name: Some("center".to_string()),
            },
            edge: Some(EdgeSealInfo {
                edge_name: "edge-a".to_string(),
                edge_code: "edge-a".to_string(),
                seal_id: seal_id.to_string(),
                export_job_id: Uuid::new_v4().to_string(),
                export_started_at: "2026-08-11T00:00:00Z".to_string(),
                export_finished_at: "2026-08-11T00:01:00Z".to_string(),
            }),
            manifest: Some(ManifestInfo {
                manifest_path: EXPORT_MANIFEST.to_string(),
                manifest_sha256_path: EXPORT_MANIFEST_SHA256.to_string(),
                object_count: 1,
                total_bytes: 1024,
                manifest_sha256: String::new(),
            }),
            security: SecurityInfo {
                center_key_id: signing.center_key_id(),
                data_key_id,
                encryption_alg: "AES-256-GCM".to_string(),
                signature_alg: SIGNATURE_ALG_HMAC_SHA256.to_string(),
                center_signature: String::new(),
            },
            status: crate::reinitializer::DiskStatus {
                code: DiskStatusCode::Imported,
                sealed: true,
                imported: true,
                reusable: false,
                last_error: None,
            },
            updated_at: Utc::now(),
        };
        disk_info.security.center_signature = signing.sign_disk_info(&disk_info).unwrap();
        disk_info
    }

    fn security() -> CenterSecurity {
        CenterSecurity::test()
    }

    struct TempDisk {
        path: PathBuf,
    }

    impl TempDisk {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("rustfs-center-reinit-{}", Uuid::new_v4()));
            fs::create_dir_all(path.join(PROTOCOL_ROOT)).unwrap();
            Self { path }
        }

        fn root(&self) -> PathBuf {
            self.path.join(PROTOCOL_ROOT)
        }
    }

    impl Drop for TempDisk {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
