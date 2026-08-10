use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

use crate::center_security::{
    CenterSecurity, ENCRYPTION_ALG_AES_256_GCM, SIGNATURE_ALG_HMAC_SHA256,
};

pub const PROTOCOL_ROOT: &str = "rustfs-transfer";
pub const DISK_INFO_FILE: &str = "disk_info.json";
pub const REINIT_FAILED: &str = "REINIT_FAILED";
pub const LEGACY_UPDATED_AT_SENTINEL: &str = "1970-01-01T00:00:00Z";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStatus {
    Cleaning,
    Reinitializing,
    Done,
    Error,
}

impl RuntimeStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cleaning => "CLEANING",
            Self::Reinitializing => "REINITIALIZING",
            Self::Done => "DONE",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredDiskIdentity {
    pub disk_id: Uuid,
    pub sn: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReinitializeAdmission {
    disk_id: Uuid,
    seal_id: Uuid,
    old_data_key_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDataKey {
    pub data_key_id: Uuid,
    pub encrypted_key: String,
    pub encryption_alg: String,
    pub key_wrap_alg: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinitializedDisk {
    pub disk_id: Uuid,
    pub old_seal_id: Uuid,
    pub old_data_key_id: Uuid,
    pub new_data_key_id: Uuid,
}

pub trait PostImportRepository {
    fn registered_disk(
        &mut self,
        disk_id: Uuid,
    ) -> Result<Option<RegisteredDiskIdentity>, ReinitializeError>;

    fn set_runtime(
        &mut self,
        disk_id: Uuid,
        runtime_status: RuntimeStatus,
        last_error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), ReinitializeError>;

    fn stage_new_data_key(
        &mut self,
        disk_id: Uuid,
        data_key: &NewDataKey,
    ) -> Result<(), ReinitializeError>;

    fn abort_staged_data_key(&mut self, data_key_id: Uuid) -> Result<(), ReinitializeError>;

    fn activate_new_key(
        &mut self,
        disk_id: Uuid,
        new_data_key_id: Uuid,
    ) -> Result<(), ReinitializeError>;

    fn rollback_new_key_activation(
        &mut self,
        disk_id: Uuid,
        new_data_key_id: Uuid,
    ) -> Result<(), ReinitializeError>;

    fn retire_old_key(
        &mut self,
        disk_id: Uuid,
        old_data_key_id: Uuid,
    ) -> Result<(), ReinitializeError>;
}

#[derive(Debug, Clone)]
pub struct DiskInfoTemplate {
    pub protocol_version: String,
    pub center_id: Uuid,
    pub center_name: Option<String>,
    pub center_key_id: Uuid,
    pub signature_alg: String,
}

pub struct PostImportReinitializer<R> {
    repo: R,
    template: DiskInfoTemplate,
    security: CenterSecurity,
}

impl<R> PostImportReinitializer<R>
where
    R: PostImportRepository,
{
    pub fn new(repo: R, template: DiskInfoTemplate, security: CenterSecurity) -> Self {
        Self {
            repo,
            template,
            security,
        }
    }

    pub fn reinitialize_imported_disk(
        &mut self,
        mount_path: &Path,
        disk_id: Uuid,
        seal_id: Uuid,
    ) -> Result<ReinitializedDisk, ReinitializeError> {
        let document = read_disk_info_document(mount_path)?;
        self.reinitialize_imported_disk_from_document(mount_path, disk_id, seal_id, document)
    }

    pub(crate) fn reinitialize_imported_disk_from_document(
        &mut self,
        mount_path: &Path,
        disk_id: Uuid,
        seal_id: Uuid,
        document: DiskInfoDocument,
    ) -> Result<ReinitializedDisk, ReinitializeError> {
        validate_center_signature_for_reinitialize(&document, &self.security)?;
        let previous_disk_info = document.disk_info;
        validate_imported_disk_info(&previous_disk_info, disk_id, seal_id)?;
        let registered_disk = self
            .repo
            .registered_disk(disk_id)?
            .ok_or(ReinitializeError::DiskNotRegistered { disk_id })?;
        validate_registered_disk_identity(&previous_disk_info, &registered_disk)?;
        let old_data_key_id = previous_disk_info.security.data_key_id;
        let admission = ReinitializeAdmission {
            disk_id,
            seal_id,
            old_data_key_id,
        };

        let result = self.reinitialize_after_admission(mount_path, admission, &previous_disk_info);
        if let Err(error) = &result {
            let _ = self.repo.set_runtime(
                disk_id,
                RuntimeStatus::Error,
                Some(REINIT_FAILED),
                Some(&error.to_string()),
            );
        }
        result
    }

    fn reinitialize_after_admission(
        &mut self,
        mount_path: &Path,
        admission: ReinitializeAdmission,
        previous_disk_info: &DiskInfo,
    ) -> Result<ReinitializedDisk, ReinitializeError> {
        ensure_no_partial_residue(mount_path)?;
        self.repo
            .set_runtime(admission.disk_id, RuntimeStatus::Cleaning, None, None)?;
        clean_sealed_payload(mount_path)?;

        self.repo
            .set_runtime(admission.disk_id, RuntimeStatus::Reinitializing, None, None)?;
        create_protocol_dirs(mount_path)?;

        let new_key = generate_data_key(admission.disk_id, &self.security)?;
        self.repo.stage_new_data_key(admission.disk_id, &new_key)?;

        let initialized_disk_info = self.initialized_disk_info(previous_disk_info, &new_key);
        if let Err(error) = self
            .repo
            .activate_new_key(admission.disk_id, new_key.data_key_id)
        {
            let _ = self.repo.abort_staged_data_key(new_key.data_key_id);
            return Err(error);
        }
        if let Err(error) = write_disk_info(mount_path, &initialized_disk_info) {
            let _ = self
                .repo
                .rollback_new_key_activation(admission.disk_id, new_key.data_key_id);
            let _ = self.repo.abort_staged_data_key(new_key.data_key_id);
            return Err(error);
        }
        if let Err(error) = self
            .repo
            .retire_old_key(admission.disk_id, admission.old_data_key_id)
        {
            tracing::warn!(
                disk_id = %admission.disk_id,
                old_data_key_id = %admission.old_data_key_id,
                error = %error,
                "old data key retirement failed after successful reinitialize"
            );
        }

        self.repo
            .set_runtime(admission.disk_id, RuntimeStatus::Done, None, None)?;

        Ok(ReinitializedDisk {
            disk_id: admission.disk_id,
            old_seal_id: admission.seal_id,
            old_data_key_id: admission.old_data_key_id,
            new_data_key_id: new_key.data_key_id,
        })
    }

    fn initialized_disk_info(
        &self,
        previous_disk_info: &DiskInfo,
        new_key: &NewDataKey,
    ) -> DiskInfo {
        let mut next = previous_disk_info.clone();
        next.protocol.version = self.template.protocol_version.clone();
        next.center.center_id = self.template.center_id;
        next.center.center_name = self.template.center_name.clone();
        next.edge = None;
        next.manifest = None;
        next.status = DiskStatus {
            code: DiskStatusCode::Initialized,
            sealed: false,
            imported: false,
            reusable: true,
            last_error: None,
        };
        next.security.center_key_id = self.template.center_key_id;
        next.security.data_key_id = new_key.data_key_id;
        next.security.encryption_alg = new_key.encryption_alg.clone();
        next.security.signature_alg = self.template.signature_alg.clone();
        next.updated_at = Utc::now();
        next.security.center_signature = String::new();
        next.security.center_signature = self
            .security
            .sign_disk_info(&next)
            .expect("disk_info signing uses validated center security key");
        next
    }
}

fn generate_data_key(
    disk_id: Uuid,
    security: &CenterSecurity,
) -> Result<NewDataKey, ReinitializeError> {
    let data_key_id = Uuid::new_v4();
    let plaintext_key = security.generate_disk_data_key();
    let encrypted_key = security
        .wrap_disk_data_key(disk_id, data_key_id, &plaintext_key)
        .map_err(|error| ReinitializeError::Repository(error.to_string()))?;
    Ok(NewDataKey {
        data_key_id,
        encrypted_key,
        encryption_alg: ENCRYPTION_ALG_AES_256_GCM.to_string(),
        key_wrap_alg: "LOCAL-MASTER-KEY".to_string(),
    })
}

fn validate_imported_disk_info(
    disk_info: &DiskInfo,
    disk_id: Uuid,
    seal_id: Uuid,
) -> Result<(), ReinitializeError> {
    if disk_info.disk.disk_id != disk_id {
        return Err(ReinitializeError::DiskIdentityMismatch {
            expected: disk_id,
            actual: disk_info.disk.disk_id,
        });
    }
    if disk_info.status.code != DiskStatusCode::Imported {
        return Err(ReinitializeError::DiskNotImported {
            actual: disk_info.status.code.as_str().to_string(),
        });
    }
    let actual_seal_id = disk_info.edge.as_ref().and_then(|edge| edge.seal_id);
    if actual_seal_id != Some(seal_id) {
        return Err(ReinitializeError::SealMismatch {
            expected: seal_id,
            actual: actual_seal_id,
        });
    }
    Ok(())
}

fn validate_registered_disk_identity(
    disk_info: &DiskInfo,
    registered_disk: &RegisteredDiskIdentity,
) -> Result<(), ReinitializeError> {
    if disk_info.disk.disk_id != registered_disk.disk_id {
        return Err(ReinitializeError::DiskIdentityMismatch {
            expected: registered_disk.disk_id,
            actual: disk_info.disk.disk_id,
        });
    }
    let registered_sn = registered_disk.sn.as_deref().map(str::trim);
    let disk_sn = disk_info.disk.sn.as_deref().map(str::trim);
    if let (Some(expected), Some(actual)) = (registered_sn, disk_sn) {
        if !expected.is_empty() && !actual.is_empty() && expected != actual {
            return Err(ReinitializeError::DiskRegistrationMismatch {
                field: "sn",
                expected: expected.to_string(),
                actual: actual.to_string(),
            });
        }
    }
    Ok(())
}

fn clean_sealed_payload(mount_path: &Path) -> Result<(), ReinitializeError> {
    let root = protocol_root(mount_path);
    for dir_name in ["data", "meta", "manifests", "logs", "quarantine"] {
        let path = root.join(dir_name);
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|source| ReinitializeError::Fs {
                action: format!("remove {}", path.display()),
                source,
            })?;
        }
    }
    sync_dir(&root)?;
    Ok(())
}

fn create_protocol_dirs(mount_path: &Path) -> Result<(), ReinitializeError> {
    let root = protocol_root(mount_path);
    for dir_name in ["data", "meta", "manifests", "logs", "quarantine/partial"] {
        let path = root.join(dir_name);
        fs::create_dir_all(&path).map_err(|source| ReinitializeError::Fs {
            action: format!("create {}", path.display()),
            source,
        })?;
    }
    sync_dir(&root)?;
    Ok(())
}

fn ensure_no_partial_residue(mount_path: &Path) -> Result<(), ReinitializeError> {
    let root = protocol_root(mount_path);
    let mut count = 0_usize;
    count_partial_files(&root, &mut count)?;
    if count > 0 {
        return Err(ReinitializeError::PartialResidueFound { count });
    }
    Ok(())
}

fn count_partial_files(path: &Path, count: &mut usize) -> Result<(), ReinitializeError> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|source| ReinitializeError::Fs {
        action: format!("scan partials {}", path.display()),
        source,
    })? {
        let entry = entry.map_err(|source| ReinitializeError::Fs {
            action: format!("scan partials {}", path.display()),
            source,
        })?;
        let entry_path = entry.path();
        let file_type = entry.file_type().map_err(|source| ReinitializeError::Fs {
            action: format!("scan partials {}", entry_path.display()),
            source,
        })?;
        if file_type.is_dir() {
            count_partial_files(&entry_path, count)?;
        } else if entry_path.extension().and_then(|value| value.to_str()) == Some("partial") {
            *count += 1;
        }
    }
    Ok(())
}

pub fn read_disk_info(mount_path: &Path) -> Result<DiskInfo, ReinitializeError> {
    Ok(read_disk_info_document(mount_path)?.disk_info)
}

#[derive(Debug, Clone)]
pub struct DiskInfoDocument {
    pub disk_info: DiskInfo,
    pub raw_value: Value,
    pub has_top_level_updated_at: bool,
}

pub fn read_disk_info_document(mount_path: &Path) -> Result<DiskInfoDocument, ReinitializeError> {
    let path = protocol_root(mount_path).join(DISK_INFO_FILE);
    let bytes = fs::read(&path).map_err(|source| ReinitializeError::Fs {
        action: format!("read {}", path.display()),
        source,
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(ReinitializeError::Json)?;
    let has_top_level_updated_at = value
        .as_object()
        .map(|object| object.contains_key("updated_at"))
        .unwrap_or(false);
    let mut parse_value = value.clone();
    if !has_top_level_updated_at
        && value.pointer("/status/code").and_then(Value::as_str) == Some("IMPORTED")
    {
        if let Value::Object(fields) = &mut parse_value {
            fields.insert(
                "updated_at".to_string(),
                Value::String(legacy_missing_updated_at().to_rfc3339()),
            );
        }
    }
    let disk_info = serde_json::from_value(parse_value).map_err(ReinitializeError::Json)?;
    Ok(DiskInfoDocument {
        disk_info,
        raw_value: value,
        has_top_level_updated_at,
    })
}

pub(crate) fn validate_center_signature_for_reinitialize(
    document: &DiskInfoDocument,
    security: &CenterSecurity,
) -> Result<(), ReinitializeError> {
    let disk_info = &document.disk_info;
    if disk_info.security.signature_alg != SIGNATURE_ALG_HMAC_SHA256 {
        return Err(ReinitializeError::SignatureInvalid(format!(
            "disk_info signature_alg must be {SIGNATURE_ALG_HMAC_SHA256}"
        )));
    }
    if disk_info.security.center_signature.trim().is_empty() {
        return Err(ReinitializeError::SignatureInvalid(
            "disk_info center_signature is missing".to_string(),
        ));
    }
    if security.verify_disk_info(disk_info).is_ok() {
        return Ok(());
    }
    if !document.has_top_level_updated_at && is_legacy_imported_disk_info(&document.raw_value) {
        return verify_raw_disk_info_for_reinitialize(&document.raw_value, security);
    }
    if has_legacy_updated_at_sentinel(&document.raw_value)
        && is_imported_disk_info_raw(&document.raw_value)
    {
        let mut historical_value = document.raw_value.clone();
        historical_value
            .as_object_mut()
            .ok_or_else(|| {
                ReinitializeError::SignatureInvalid("disk_info root must be an object".to_string())
            })?
            .remove("updated_at");
        return verify_raw_disk_info_for_reinitialize(&historical_value, security);
    }
    Err(ReinitializeError::SignatureInvalid(
        "disk_info center_signature verification failed".to_string(),
    ))
}

fn is_legacy_imported_disk_info(raw_disk_info: &Value) -> bool {
    raw_disk_info.get("updated_at").is_none() && is_imported_disk_info_raw(raw_disk_info)
}

fn is_imported_disk_info_raw(raw_disk_info: &Value) -> bool {
    raw_disk_info
        .pointer("/status/code")
        .and_then(Value::as_str)
        == Some("IMPORTED")
}

fn has_legacy_updated_at_sentinel(raw_disk_info: &Value) -> bool {
    raw_disk_info
        .get("updated_at")
        .and_then(Value::as_str)
        .is_some_and(|updated_at| updated_at == LEGACY_UPDATED_AT_SENTINEL)
}

fn verify_raw_disk_info_for_reinitialize(
    raw_disk_info: &Value,
    security: &CenterSecurity,
) -> Result<(), ReinitializeError> {
    security.verify_disk_info(raw_disk_info).map_err(|_| {
        ReinitializeError::SignatureInvalid(
            "disk_info center_signature verification failed".to_string(),
        )
    })
}

pub fn write_disk_info(mount_path: &Path, disk_info: &DiskInfo) -> Result<(), ReinitializeError> {
    let root = protocol_root(mount_path);
    fs::create_dir_all(&root).map_err(|source| ReinitializeError::Fs {
        action: format!("create {}", root.display()),
        source,
    })?;
    let path = root.join(DISK_INFO_FILE);
    let temp_path = root.join(format!("{}.tmp-{}", DISK_INFO_FILE, Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(disk_info).map_err(ReinitializeError::Json)?;

    {
        let mut file = File::create(&temp_path).map_err(|source| ReinitializeError::Fs {
            action: format!("create {}", temp_path.display()),
            source,
        })?;
        file.write_all(&bytes)
            .map_err(|source| ReinitializeError::Fs {
                action: format!("write {}", temp_path.display()),
                source,
            })?;
        file.sync_all().map_err(|source| ReinitializeError::Fs {
            action: format!("fsync {}", temp_path.display()),
            source,
        })?;
    }

    fs::rename(&temp_path, &path).map_err(|source| ReinitializeError::Fs {
        action: format!("rename {} to {}", temp_path.display(), path.display()),
        source,
    })?;
    sync_dir(&root)?;
    Ok(())
}

fn protocol_root(mount_path: &Path) -> PathBuf {
    mount_path.join(PROTOCOL_ROOT)
}

#[cfg(unix)]
fn sync_dir(path: &Path) -> Result<(), ReinitializeError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|source| ReinitializeError::Fs {
            action: format!("fsync {}", path.display()),
            source,
        })
}

#[cfg(not(unix))]
fn sync_dir(_path: &Path) -> Result<(), ReinitializeError> {
    Ok(())
}

#[derive(Debug, Error)]
pub enum ReinitializeError {
    #[error("disk_id={disk_id} is not registered or enabled in center")]
    DiskNotRegistered { disk_id: Uuid },
    #[error("disk_id mismatch: expected {expected}, got {actual}")]
    DiskIdentityMismatch { expected: Uuid, actual: Uuid },
    #[error("registered disk {field} mismatch: expected {expected}, got {actual}")]
    DiskRegistrationMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
    #[error("disk is not IMPORTED, got {actual}")]
    DiskNotImported { actual: String },
    #[error("seal_id mismatch: expected {expected}, got {actual:?}")]
    SealMismatch {
        expected: Uuid,
        actual: Option<Uuid>,
    },
    #[error("partial residue blocks reinitialize: {count} .partial files found")]
    PartialResidueFound { count: usize },
    #[error("{action}: {source}")]
    Fs { action: String, source: io::Error },
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("repository error: {0}")]
    Repository(String),
    #[error("disk_info signature invalid: {0}")]
    SignatureInvalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiskInfo {
    pub protocol: ProtocolInfo,
    pub disk: DiskIdentity,
    pub center: CenterInfo,
    pub edge: Option<EdgeSealInfo>,
    pub manifest: Option<ManifestInfo>,
    pub security: SecurityInfo,
    pub status: DiskStatus,
    pub updated_at: DateTime<Utc>,
}

fn legacy_missing_updated_at() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).expect("unix epoch is a valid UTC timestamp")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolInfo {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiskIdentity {
    pub disk_id: Uuid,
    pub sn: Option<String>,
    pub capacity_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CenterInfo {
    pub center_id: Uuid,
    pub center_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EdgeSealInfo {
    pub edge_code: String,
    pub export_job_id: Uuid,
    pub seal_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestInfo {
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityInfo {
    pub center_key_id: Uuid,
    pub data_key_id: Uuid,
    pub encryption_alg: String,
    pub signature_alg: String,
    pub center_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiskStatus {
    pub code: DiskStatusCode,
    pub sealed: bool,
    pub imported: bool,
    pub reusable: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiskStatusCode {
    Unregistered,
    Registered,
    Initialized,
    EdgeCopying,
    Sealed,
    CenterImporting,
    Imported,
    Error,
}

impl DiskStatusCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unregistered => "UNREGISTERED",
            Self::Registered => "REGISTERED",
            Self::Initialized => "INITIALIZED",
            Self::EdgeCopying => "EDGE_COPYING",
            Self::Sealed => "SEALED",
            Self::CenterImporting => "CENTER_IMPORTING",
            Self::Imported => "IMPORTED",
            Self::Error => "ERROR",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;

    #[derive(Default)]
    struct MemoryRepo {
        registered_disk: Option<RegisteredDiskIdentity>,
        runtime: Vec<(RuntimeStatus, Option<String>)>,
        staged_key: Option<Uuid>,
        active_key: Option<Uuid>,
        retired_key: Option<Uuid>,
        fail_stage: bool,
        fail_activate: bool,
        fail_retire_old_key: bool,
    }

    impl PostImportRepository for RefCell<MemoryRepo> {
        fn registered_disk(
            &mut self,
            disk_id: Uuid,
        ) -> Result<Option<RegisteredDiskIdentity>, ReinitializeError> {
            Ok(self
                .borrow()
                .registered_disk
                .clone()
                .filter(|registered| registered.disk_id == disk_id))
        }

        fn set_runtime(
            &mut self,
            _disk_id: Uuid,
            runtime_status: RuntimeStatus,
            last_error_code: Option<&str>,
            _error_message: Option<&str>,
        ) -> Result<(), ReinitializeError> {
            self.borrow_mut()
                .runtime
                .push((runtime_status, last_error_code.map(ToOwned::to_owned)));
            Ok(())
        }

        fn stage_new_data_key(
            &mut self,
            _disk_id: Uuid,
            data_key: &NewDataKey,
        ) -> Result<(), ReinitializeError> {
            if self.borrow().fail_stage {
                return Err(ReinitializeError::Repository("stage failed".to_string()));
            }
            self.borrow_mut().staged_key = Some(data_key.data_key_id);
            Ok(())
        }

        fn abort_staged_data_key(&mut self, data_key_id: Uuid) -> Result<(), ReinitializeError> {
            let mut repo = self.borrow_mut();
            if repo.staged_key == Some(data_key_id) {
                repo.staged_key = None;
            }
            Ok(())
        }

        fn activate_new_key(
            &mut self,
            _disk_id: Uuid,
            new_data_key_id: Uuid,
        ) -> Result<(), ReinitializeError> {
            if self.borrow().fail_activate {
                return Err(ReinitializeError::Repository("activate failed".to_string()));
            }
            let mut repo = self.borrow_mut();
            repo.active_key = Some(new_data_key_id);
            Ok(())
        }

        fn rollback_new_key_activation(
            &mut self,
            _disk_id: Uuid,
            new_data_key_id: Uuid,
        ) -> Result<(), ReinitializeError> {
            let mut repo = self.borrow_mut();
            if repo.active_key == Some(new_data_key_id) {
                repo.active_key = None;
            }
            Ok(())
        }

        fn retire_old_key(
            &mut self,
            _disk_id: Uuid,
            old_data_key_id: Uuid,
        ) -> Result<(), ReinitializeError> {
            if self.borrow().fail_retire_old_key {
                return Err(ReinitializeError::Repository("retire failed".to_string()));
            }
            let mut repo = self.borrow_mut();
            repo.retired_key = Some(old_data_key_id);
            Ok(())
        }
    }

    #[test]
    fn success_cleans_payload_and_marks_initialized_after_runtime_steps() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_disk_info(
            &temp.path,
            &imported_disk_info(disk_id, seal_id, old_key, &security()),
        )
        .unwrap();
        fs::create_dir_all(temp.root().join("data")).unwrap();
        fs::write(temp.root().join("data/object.bin"), b"sealed").unwrap();
        fs::create_dir_all(temp.root().join("manifests")).unwrap();
        fs::write(temp.root().join("manifests/export_manifest.json"), b"{}").unwrap();

        let repo = RefCell::new(MemoryRepo {
            registered_disk: Some(registered_disk(disk_id)),
            ..Default::default()
        });
        let mut service = PostImportReinitializer::new(repo, template(), security());

        let output = service
            .reinitialize_imported_disk(&temp.path, disk_id, seal_id)
            .unwrap();

        let disk_info = read_disk_info(&temp.path).unwrap();
        assert_eq!(disk_info.status.code, DiskStatusCode::Initialized);
        assert!(disk_info.status.reusable);
        assert_eq!(disk_info.edge, None);
        assert_eq!(disk_info.manifest, None);
        assert_ne!(
            disk_info.updated_at.to_rfc3339(),
            LEGACY_UPDATED_AT_SENTINEL
        );
        security().verify_disk_info(&disk_info).unwrap();
        assert_ne!(disk_info.security.data_key_id, old_key);
        assert_eq!(output.old_data_key_id, old_key);
        assert!(!temp.root().join("data/object.bin").exists());
        assert!(temp.root().join("quarantine/partial").is_dir());

        let repo = service.repo.borrow();
        assert_eq!(
            repo.runtime,
            vec![
                (RuntimeStatus::Cleaning, None),
                (RuntimeStatus::Reinitializing, None),
                (RuntimeStatus::Done, None),
            ]
        );
        assert_eq!(repo.active_key, Some(output.new_data_key_id));
        assert_eq!(repo.retired_key, Some(old_key));
    }

    #[test]
    fn cleanup_failure_preserves_import_done_boundary_and_imported_disk_info() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        let original = imported_disk_info(disk_id, seal_id, old_key, &security());
        write_minified_disk_info(&temp.path, &original).unwrap();
        let before = fs::read(temp.disk_info_path()).unwrap();
        fs::write(temp.root().join("data"), b"not a directory").unwrap();

        let repo = RefCell::new(MemoryRepo {
            registered_disk: Some(registered_disk(disk_id)),
            ..Default::default()
        });
        let mut service = PostImportReinitializer::new(repo, template(), security());

        let error = service
            .reinitialize_imported_disk(&temp.path, disk_id, seal_id)
            .unwrap_err();

        assert!(error.to_string().contains("remove"));
        assert_eq!(fs::read(temp.disk_info_path()).unwrap(), before);
        let disk_info = read_disk_info(&temp.path).unwrap();
        assert_eq!(disk_info.status.code, DiskStatusCode::Imported);
        assert_eq!(disk_info.security.data_key_id, old_key);
        security().verify_disk_info(&disk_info).unwrap();

        let repo = service.repo.borrow();
        assert_eq!(repo.staged_key, None);
        assert_eq!(repo.active_key, None);
        assert_eq!(repo.retired_key, None);
        assert_eq!(
            repo.runtime,
            vec![
                (RuntimeStatus::Cleaning, None),
                (RuntimeStatus::Error, Some(REINIT_FAILED.to_string())),
            ]
        );
    }

    #[test]
    fn activation_failure_aborts_new_key_and_leaves_disk_info_bytes_unchanged() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        let original = imported_disk_info(disk_id, seal_id, old_key, &security());
        write_minified_disk_info(&temp.path, &original).unwrap();
        let before = fs::read(temp.disk_info_path()).unwrap();

        let repo = RefCell::new(MemoryRepo {
            registered_disk: Some(registered_disk(disk_id)),
            fail_activate: true,
            ..Default::default()
        });
        let mut service = PostImportReinitializer::new(repo, template(), security());

        let error = service
            .reinitialize_imported_disk(&temp.path, disk_id, seal_id)
            .unwrap_err();

        assert!(error.to_string().contains("activate failed"));
        assert_eq!(fs::read(temp.disk_info_path()).unwrap(), before);
        let disk_info = read_disk_info(&temp.path).unwrap();
        assert_eq!(disk_info.status.code, DiskStatusCode::Imported);
        assert_eq!(disk_info.security.data_key_id, old_key);

        let repo = service.repo.borrow();
        assert_eq!(repo.staged_key, None);
        assert_eq!(repo.active_key, None);
        assert_eq!(repo.retired_key, None);
        assert!(repo
            .runtime
            .contains(&(RuntimeStatus::Error, Some(REINIT_FAILED.to_string()))));
    }

    #[test]
    fn legacy_updated_at_sentinel_imported_disk_is_verified_without_rewriting_disk() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        let mut sentinel_value = imported_disk_info_without_updated_at(disk_id, seal_id, old_key);
        sentinel_value["updated_at"] = json!(LEGACY_UPDATED_AT_SENTINEL);
        write_raw_disk_info(&temp.path, &sentinel_value).unwrap();
        let before = fs::read(temp.disk_info_path()).unwrap();
        fs::write(temp.root().join("data"), b"not a directory").unwrap();

        let repo = RefCell::new(MemoryRepo {
            registered_disk: Some(registered_disk(disk_id)),
            ..Default::default()
        });
        let mut service = PostImportReinitializer::new(repo, template(), security());

        let error = service
            .reinitialize_imported_disk(&temp.path, disk_id, seal_id)
            .unwrap_err();

        assert!(error.to_string().contains("remove"));
        assert_eq!(fs::read(temp.disk_info_path()).unwrap(), before);
        let repo = service.repo.borrow();
        assert_eq!(repo.staged_key, None);
        assert_eq!(repo.active_key, None);
        assert_eq!(repo.retired_key, None);
        assert_eq!(
            repo.runtime,
            vec![
                (RuntimeStatus::Cleaning, None),
                (RuntimeStatus::Error, Some(REINIT_FAILED.to_string())),
            ]
        );
    }

    #[test]
    fn legacy_updated_at_sentinel_still_rejects_tampered_signed_fields() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        let mut sentinel_value = imported_disk_info_without_updated_at(disk_id, seal_id, old_key);
        sentinel_value["updated_at"] = json!(LEGACY_UPDATED_AT_SENTINEL);
        sentinel_value["security"]["data_key_id"] = json!(Uuid::new_v4());
        write_raw_disk_info(&temp.path, &sentinel_value).unwrap();
        fs::create_dir_all(temp.root().join("data")).unwrap();

        let repo = RefCell::new(MemoryRepo {
            registered_disk: Some(registered_disk(disk_id)),
            ..Default::default()
        });
        let mut service = PostImportReinitializer::new(repo, template(), security());

        let error = service
            .reinitialize_imported_disk(&temp.path, disk_id, seal_id)
            .unwrap_err();

        assert!(matches!(error, ReinitializeError::SignatureInvalid(_)));
        let repo = service.repo.borrow();
        assert!(repo.runtime.is_empty());
        assert_eq!(repo.staged_key, None);
        assert_eq!(repo.active_key, None);
        assert_eq!(repo.retired_key, None);
    }

    #[test]
    fn legacy_imported_disk_info_without_updated_at_is_accepted_with_audit_sentinel() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        let mut value =
            serde_json::to_value(imported_disk_info(disk_id, seal_id, old_key, &security()))
                .unwrap();
        value.as_object_mut().unwrap().remove("updated_at");
        fs::write(
            temp.root().join(DISK_INFO_FILE),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();

        let disk_info = read_disk_info(&temp.path).unwrap();

        assert_eq!(disk_info.disk.disk_id, disk_id);
        assert_eq!(disk_info.status.code, DiskStatusCode::Imported);
        assert_eq!(disk_info.updated_at, legacy_missing_updated_at());
    }

    #[test]
    fn missing_updated_at_is_only_accepted_for_legacy_imported_disk_info() {
        for status_code in [DiskStatusCode::Initialized, DiskStatusCode::Sealed] {
            let temp = TempDisk::new();
            let disk_id = Uuid::new_v4();
            let seal_id = Uuid::new_v4();
            let old_key = Uuid::new_v4();
            let mut value =
                serde_json::to_value(imported_disk_info(disk_id, seal_id, old_key, &security()))
                    .unwrap();
            value["status"]["code"] = serde_json::Value::String(status_code.as_str().to_string());
            value.as_object_mut().unwrap().remove("updated_at");
            fs::write(
                temp.root().join(DISK_INFO_FILE),
                serde_json::to_vec_pretty(&value).unwrap(),
            )
            .unwrap();

            let error = read_disk_info(&temp.path).unwrap_err();

            assert!(matches!(error, ReinitializeError::Json(_)));
        }
    }

    #[test]
    fn empty_or_invalid_updated_at_is_rejected() {
        for updated_at in ["", "not-a-timestamp"] {
            let temp = TempDisk::new();
            let disk_id = Uuid::new_v4();
            let seal_id = Uuid::new_v4();
            let old_key = Uuid::new_v4();
            let mut value =
                serde_json::to_value(imported_disk_info(disk_id, seal_id, old_key, &security()))
                    .unwrap();
            value["updated_at"] = serde_json::Value::String(updated_at.to_string());
            fs::write(
                temp.root().join(DISK_INFO_FILE),
                serde_json::to_vec_pretty(&value).unwrap(),
            )
            .unwrap();

            let error = read_disk_info(&temp.path).unwrap_err();

            assert!(matches!(error, ReinitializeError::Json(_)));
        }
    }

    #[test]
    fn missing_critical_disk_info_fields_still_reject() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        let mut value =
            serde_json::to_value(imported_disk_info(disk_id, seal_id, old_key, &security()))
                .unwrap();
        value.as_object_mut().unwrap().remove("security");
        fs::write(
            temp.root().join(DISK_INFO_FILE),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();

        let error = read_disk_info(&temp.path).unwrap_err();

        assert!(matches!(error, ReinitializeError::Json(_)));
    }

    #[test]
    fn tampered_imported_disk_info_blocks_reinitialization() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        let mut disk_info = imported_disk_info(disk_id, seal_id, old_key, &security());
        disk_info.security.data_key_id = Uuid::new_v4();
        write_disk_info(&temp.path, &disk_info).unwrap();

        let repo = RefCell::new(MemoryRepo {
            registered_disk: Some(registered_disk(disk_id)),
            ..Default::default()
        });
        let mut service = PostImportReinitializer::new(repo, template(), security());

        let error = service
            .reinitialize_imported_disk(&temp.path, disk_id, seal_id)
            .unwrap_err();

        assert!(matches!(error, ReinitializeError::SignatureInvalid(_)));
        assert!(temp.root().join("data").exists() || temp.root().exists());
    }

    #[test]
    fn partial_residue_rejects_without_cleaning_or_retiring_key() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_disk_info(
            &temp.path,
            &imported_disk_info(disk_id, seal_id, old_key, &security()),
        )
        .unwrap();
        fs::create_dir_all(temp.root().join("data")).unwrap();
        fs::write(temp.root().join("data/object.enc.partial"), b"incomplete").unwrap();

        let repo = RefCell::new(MemoryRepo {
            registered_disk: Some(registered_disk(disk_id)),
            ..Default::default()
        });
        let mut service = PostImportReinitializer::new(repo, template(), security());

        let error = service
            .reinitialize_imported_disk(&temp.path, disk_id, seal_id)
            .unwrap_err();

        assert!(matches!(
            error,
            ReinitializeError::PartialResidueFound { .. }
        ));
        let repo = service.repo.borrow();
        assert_eq!(
            repo.runtime,
            vec![(RuntimeStatus::Error, Some(REINIT_FAILED.to_string()))]
        );
        assert_eq!(repo.active_key, None);
        assert_eq!(repo.retired_key, None);
    }

    #[test]
    fn old_data_key_binding_mismatch_is_not_an_admission_gate() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let disk_old_key = Uuid::new_v4();
        write_disk_info(
            &temp.path,
            &imported_disk_info(disk_id, seal_id, disk_old_key, &security()),
        )
        .unwrap();

        let repo = RefCell::new(MemoryRepo {
            registered_disk: Some(registered_disk(disk_id)),
            ..Default::default()
        });
        let mut service = PostImportReinitializer::new(repo, template(), security());

        let output = service
            .reinitialize_imported_disk(&temp.path, disk_id, seal_id)
            .unwrap();

        assert_eq!(output.old_data_key_id, disk_old_key);
        assert_eq!(
            read_disk_info(&temp.path).unwrap().status.code,
            DiskStatusCode::Initialized
        );
        assert_eq!(service.repo.borrow().retired_key, Some(disk_old_key));
    }

    #[test]
    fn unregistered_disk_rejects_before_cleaning_or_key_rotation() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_disk_info(
            &temp.path,
            &imported_disk_info(disk_id, seal_id, old_key, &security()),
        )
        .unwrap();
        let repo = RefCell::new(MemoryRepo::default());
        let mut service = PostImportReinitializer::new(repo, template(), security());

        let error = service
            .reinitialize_imported_disk(&temp.path, disk_id, seal_id)
            .unwrap_err();

        assert!(matches!(error, ReinitializeError::DiskNotRegistered { .. }));
        assert_eq!(
            read_disk_info(&temp.path).unwrap().security.data_key_id,
            old_key
        );
        assert!(service.repo.borrow().runtime.is_empty());
    }

    #[test]
    fn old_key_retirement_failure_warns_without_blocking_new_key_activation() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_disk_info(
            &temp.path,
            &imported_disk_info(disk_id, seal_id, old_key, &security()),
        )
        .unwrap();

        let repo = RefCell::new(MemoryRepo {
            registered_disk: Some(registered_disk(disk_id)),
            fail_retire_old_key: true,
            ..Default::default()
        });
        let mut service = PostImportReinitializer::new(repo, template(), security());

        let output = service
            .reinitialize_imported_disk(&temp.path, disk_id, seal_id)
            .unwrap();

        let disk_info = read_disk_info(&temp.path).unwrap();
        assert_eq!(disk_info.status.code, DiskStatusCode::Initialized);
        let repo = service.repo.borrow();
        assert_eq!(repo.active_key, Some(output.new_data_key_id));
        assert_eq!(repo.retired_key, None);
        assert_eq!(
            repo.runtime,
            vec![
                (RuntimeStatus::Cleaning, None),
                (RuntimeStatus::Reinitializing, None),
                (RuntimeStatus::Done, None),
            ]
        );
    }

    #[test]
    fn registered_sn_mismatch_rejects_before_cleaning() {
        let temp = TempDisk::new();
        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let old_key = Uuid::new_v4();
        write_disk_info(
            &temp.path,
            &imported_disk_info(disk_id, seal_id, old_key, &security()),
        )
        .unwrap();
        fs::create_dir_all(temp.root().join("data")).unwrap();
        fs::write(temp.root().join("data/object.bin"), b"sealed").unwrap();

        let repo = RefCell::new(MemoryRepo {
            registered_disk: Some(RegisteredDiskIdentity {
                disk_id,
                sn: Some("OTHER-SN".to_string()),
            }),
            ..Default::default()
        });
        let mut service = PostImportReinitializer::new(repo, template(), security());

        let error = service
            .reinitialize_imported_disk(&temp.path, disk_id, seal_id)
            .unwrap_err();

        assert!(matches!(
            error,
            ReinitializeError::DiskRegistrationMismatch { field: "sn", .. }
        ));
        assert!(temp.root().join("data/object.bin").exists());
        assert!(service.repo.borrow().runtime.is_empty());
    }

    fn registered_disk(disk_id: Uuid) -> RegisteredDiskIdentity {
        RegisteredDiskIdentity {
            disk_id,
            sn: Some("SN001".to_string()),
        }
    }

    fn template() -> DiskInfoTemplate {
        DiskInfoTemplate {
            protocol_version: "1.0".to_string(),
            center_id: Uuid::new_v4(),
            center_name: Some("center".to_string()),
            center_key_id: security().center_key_id(),
            signature_alg: crate::center_security::SIGNATURE_ALG_HMAC_SHA256.to_string(),
        }
    }

    fn imported_disk_info(
        disk_id: Uuid,
        seal_id: Uuid,
        data_key_id: Uuid,
        security: &CenterSecurity,
    ) -> DiskInfo {
        let mut disk_info = DiskInfo {
            protocol: ProtocolInfo {
                version: "1.0".to_string(),
            },
            disk: DiskIdentity {
                disk_id,
                sn: Some("SN001".to_string()),
                capacity_bytes: 1024,
            },
            center: CenterInfo {
                center_id: Uuid::new_v4(),
                center_name: Some("center".to_string()),
            },
            edge: Some(EdgeSealInfo {
                edge_code: "edge-a".to_string(),
                export_job_id: Uuid::new_v4(),
                seal_id: Some(seal_id),
            }),
            manifest: Some(ManifestInfo {
                manifest_sha256: "manifest-sha".to_string(),
            }),
            security: SecurityInfo {
                center_key_id: security.center_key_id(),
                data_key_id,
                encryption_alg: ENCRYPTION_ALG_AES_256_GCM.to_string(),
                signature_alg: crate::center_security::SIGNATURE_ALG_HMAC_SHA256.to_string(),
                center_signature: String::new(),
            },
            status: DiskStatus {
                code: DiskStatusCode::Imported,
                sealed: true,
                imported: true,
                reusable: false,
                last_error: None,
            },
            updated_at: Utc::now(),
        };
        disk_info.security.center_signature = security.sign_disk_info(&disk_info).unwrap();
        disk_info
    }

    fn imported_disk_info_without_updated_at(
        disk_id: Uuid,
        seal_id: Uuid,
        data_key_id: Uuid,
    ) -> serde_json::Value {
        let disk_info = imported_disk_info(disk_id, seal_id, data_key_id, &security());
        let mut value = serde_json::to_value(&disk_info).unwrap();
        value
            .as_object_mut()
            .expect("disk_info is object")
            .remove("updated_at");
        value["security"]["center_signature"] = json!("");
        let signature = security().sign_disk_info(&value).unwrap();
        value["security"]["center_signature"] = json!(signature);
        value
    }

    fn write_minified_disk_info(
        mount_path: &Path,
        disk_info: &DiskInfo,
    ) -> Result<(), ReinitializeError> {
        write_raw_disk_info(mount_path, &serde_json::to_value(disk_info)?)
    }

    fn write_raw_disk_info(
        mount_path: &Path,
        value: &serde_json::Value,
    ) -> Result<(), ReinitializeError> {
        let root = protocol_root(mount_path);
        fs::create_dir_all(&root).map_err(|source| ReinitializeError::Fs {
            action: format!("create {}", root.display()),
            source,
        })?;
        let path = root.join(DISK_INFO_FILE);
        let bytes = serde_json::to_vec(value).map_err(ReinitializeError::Json)?;
        fs::write(&path, bytes).map_err(|source| ReinitializeError::Fs {
            action: format!("write {}", path.display()),
            source,
        })
    }

    fn security() -> CenterSecurity {
        CenterSecurity::test()
    }

    struct TempDisk {
        path: PathBuf,
    }

    impl TempDisk {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("rustfs-center-test-{}", Uuid::new_v4()));
            fs::create_dir_all(path.join(PROTOCOL_ROOT)).unwrap();
            Self { path }
        }

        fn root(&self) -> PathBuf {
            self.path.join(PROTOCOL_ROOT)
        }

        fn disk_info_path(&self) -> PathBuf {
            self.root().join(DISK_INFO_FILE)
        }
    }

    impl Drop for TempDisk {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
