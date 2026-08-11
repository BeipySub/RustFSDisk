use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    center_security::{CenterSecurity, ENCRYPTION_ALG_AES_256_GCM, SIGNATURE_ALG_HMAC_SHA256},
    CenterConfigRecord, DiskRecord, DiskStatusCode,
};

pub const PROTOCOL_ROOT: &str = "rustfs-transfer";
pub const DISK_INFO_FILE: &str = "disk_info.json";
pub const PROTOCOL_NAME: &str = "rustfs-offline-transfer";

#[derive(Debug, Clone, Serialize)]
pub struct InitializedDiskInfoDocument {
    pub protocol: DiskInfoProtocol,
    pub disk: DiskInfoDisk,
    pub edge: DiskInfoEdge,
    pub center: DiskInfoCenter,
    pub manifest: DiskInfoManifest,
    pub security: DiskInfoSecurity,
    pub status: DiskInfoStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfoProtocol {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfoDisk {
    pub disk_id: Uuid,
    pub sn: String,
    pub capacity_bytes: i64,
    pub last_init_time: String,
    pub initialized_by: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfoEdge {
    pub edge_name: String,
    pub edge_code: String,
    pub seal_id: String,
    pub export_job_id: String,
    pub export_started_at: String,
    pub export_finished_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfoCenter {
    pub center_id: Uuid,
    pub import_job_id: String,
    pub import_started_at: String,
    pub import_finished_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfoManifest {
    pub manifest_path: String,
    pub manifest_sha256_path: String,
    pub object_count: u64,
    pub total_bytes: u64,
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfoSecurity {
    pub center_key_id: Uuid,
    pub data_key_id: Uuid,
    pub encryption_alg: String,
    pub signature_alg: String,
    pub center_signature: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfoStatus {
    pub code: DiskStatusCode,
    pub sealed: bool,
    pub imported: bool,
    pub reusable: bool,
    pub last_error: Option<String>,
}

impl DiskInfoStatus {
    pub fn initialized() -> Self {
        Self {
            code: DiskStatusCode::Initialized,
            sealed: false,
            imported: false,
            reusable: true,
            last_error: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            code: DiskStatusCode::Error,
            sealed: false,
            imported: false,
            reusable: false,
            last_error: Some(message.into()),
        }
    }
}

impl InitializedDiskInfoDocument {
    pub fn initialized(
        center_config: &CenterConfigRecord,
        disk: &DiskRecord,
        capacity_bytes: i64,
        data_key_id: Uuid,
        security: &CenterSecurity,
    ) -> Result<Self> {
        let now = Utc::now();
        let mut document = Self {
            protocol: DiskInfoProtocol {
                name: PROTOCOL_NAME.to_string(),
                version: center_config.protocol_version.clone(),
            },
            disk: DiskInfoDisk {
                disk_id: disk.disk_id,
                sn: disk.sn.clone(),
                capacity_bytes,
                last_init_time: now.to_rfc3339(),
                initialized_by: "center".to_string(),
            },
            edge: DiskInfoEdge {
                edge_name: String::new(),
                edge_code: String::new(),
                seal_id: String::new(),
                export_job_id: String::new(),
                export_started_at: String::new(),
                export_finished_at: String::new(),
            },
            center: DiskInfoCenter {
                center_id: center_config.center_id,
                import_job_id: String::new(),
                import_started_at: String::new(),
                import_finished_at: String::new(),
            },
            manifest: DiskInfoManifest {
                manifest_path: "manifests/export_manifest.json".to_string(),
                manifest_sha256_path: "manifests/export_manifest.sha256".to_string(),
                object_count: 0,
                total_bytes: 0,
                manifest_sha256: String::new(),
            },
            security: DiskInfoSecurity {
                center_key_id: security.center_key_id(),
                data_key_id,
                encryption_alg: ENCRYPTION_ALG_AES_256_GCM.to_string(),
                signature_alg: SIGNATURE_ALG_HMAC_SHA256.to_string(),
                center_signature: String::new(),
            },
            status: DiskInfoStatus::initialized(),
            created_at: now,
            updated_at: now,
        };
        document.security.center_signature = security.sign_disk_info(&document)?;
        Ok(document)
    }
}

pub fn write_initialized_disk_info(
    mount_path: &Path,
    document: &InitializedDiskInfoDocument,
) -> Result<()> {
    let root = mount_path.join(PROTOCOL_ROOT);
    fs::create_dir_all(root.join("data"))?;
    fs::create_dir_all(root.join("meta"))?;
    fs::create_dir_all(root.join("manifests"))?;
    fs::create_dir_all(root.join("logs"))?;
    fs::create_dir_all(root.join("quarantine").join("partial"))?;

    let disk_info_path = root.join(DISK_INFO_FILE);
    let tmp_path = root.join(format!("{}.tmp-{}", DISK_INFO_FILE, Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(document)?;

    {
        let mut file =
            File::create(&tmp_path).with_context(|| format!("create {}", tmp_path.display()))?;
        file.write_all(&bytes)
            .with_context(|| format!("write {}", tmp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("fsync {}", tmp_path.display()))?;
    }
    fs::rename(&tmp_path, &disk_info_path).with_context(|| {
        format!(
            "rename {} to {}",
            tmp_path.display(),
            disk_info_path.display()
        )
    })?;
    sync_directory_best_effort(&root)?;
    Ok(())
}

fn sync_directory_best_effort(path: &Path) -> Result<()> {
    match File::open(path).and_then(|file| file.sync_all()) {
        Ok(()) => Ok(()),
        Err(err) if cfg!(windows) && err.kind() == std::io::ErrorKind::PermissionDenied => Ok(()),
        Err(err) => Err(err).with_context(|| format!("fsync {}", path.display())),
    }
}
