use std::{collections::BTreeMap, fs, path::PathBuf};

use rustfs_transfer_common::{
    crypto::{
        encode_base64, encrypt_aes256_gcm, pack_object_aad, sha256_lower_hex, sign_hmac_base64,
        CanonicalRequest, PackObjectAad,
    },
    protocol::{
        DiskInfo, DiskInfoCenter, DiskInfoDisk, DiskInfoEdge, DiskInfoManifest, DiskInfoProtocol,
        DiskInfoSecurity, DiskInfoStatus, DiskStatusCode, ExportManifest, ManifestObject,
        ManifestPackRef, ObjectStatus, StorageMode, TransferDisk, ENCRYPTION_ALG_AES_256_GCM,
        MANIFEST_VERSION, PROTOCOL_NAME, PROTOCOL_ROOT_DIR, PROTOCOL_VERSION,
        SIGNATURE_ALG_HMAC_SHA256,
    },
};
use serde_json::json;
use uuid::Uuid;

fn main() -> anyhow::Result<()> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/fixtures"));
    if output.exists() {
        fs::remove_dir_all(&output)?;
    }
    fs::create_dir_all(&output)?;

    let initialized = output.join("initialized-disk");
    let sealed = output.join("sealed-disk");
    let hmac = output.join("hmac");

    write_initialized_disk(&initialized)?;
    write_sealed_disk(&sealed)?;
    write_hmac_samples(&hmac)?;

    println!("fixtures written to {}", output.display());
    Ok(())
}

fn write_initialized_disk(mount_path: &PathBuf) -> anyhow::Result<()> {
    let disk_id = Uuid::new_v4();
    let data_key_id = Uuid::new_v4();
    let disk = TransferDisk::new(mount_path);
    disk.ensure_layout()?;
    disk.write_disk_info(&DiskInfo {
        protocol: DiskInfoProtocol {
            name: PROTOCOL_NAME.to_string(),
            version: PROTOCOL_VERSION.to_string(),
        },
        disk: DiskInfoDisk {
            sn: "FIXTURE-SN-INITIALIZED".to_string(),
            disk_id: disk_id.to_string(),
            capacity_bytes: 8 * 1024 * 1024 * 1024,
            last_init_time: "2026-08-09T00:00:00Z".to_string(),
            initialized_by: "fixture-generator".to_string(),
        },
        status: DiskInfoStatus::from_code(DiskStatusCode::Initialized, ""),
        edge: empty_edge(),
        center: DiskInfoCenter {
            center_id: Uuid::new_v4().to_string(),
            import_job_id: String::new(),
            import_started_at: String::new(),
            import_finished_at: String::new(),
        },
        manifest: DiskInfoManifest::default(),
        security: DiskInfoSecurity {
            center_signature: "fixture-signature".to_string(),
            signature_alg: SIGNATURE_ALG_HMAC_SHA256.to_string(),
            center_key_id: Uuid::new_v4().to_string(),
            encryption_alg: ENCRYPTION_ALG_AES_256_GCM.to_string(),
            data_key_id: data_key_id.to_string(),
        },
    })?;
    Ok(())
}

fn write_sealed_disk(mount_path: &PathBuf) -> anyhow::Result<()> {
    let disk_id = Uuid::new_v4();
    let seal_id = Uuid::new_v4();
    let export_job_id = Uuid::new_v4();
    let data_key_id = Uuid::new_v4();
    let edge_code = "edge-a";
    let bucket = "source";
    let key = "alpha.txt";
    let plaintext = b"hello archive fixture";
    let object_id = Uuid::new_v4();
    let disk_data_key = [7_u8; 32];
    let nonce = [3_u8; 12];
    let pack_path = "packs/export-fixture/pack-000001.pack";
    let pack_index_path = "packs/export-fixture/pack-000001.idx";
    let plaintext_sha256 = sha256_lower_hex(plaintext);
    let aad = pack_object_aad(PackObjectAad {
        disk_id: &disk_id.to_string(),
        seal_id: &seal_id.to_string(),
        export_job_id: &export_job_id.to_string(),
        object_id: &object_id.to_string(),
        bucket,
        object_key: key,
        pack_path,
        pack_offset_bytes: 0,
        plaintext_sha256: &plaintext_sha256,
    });
    let encrypted = encrypt_aes256_gcm(&disk_data_key, &nonce, plaintext, &aad)?;
    let relative_meta_path = "meta/source/alpha.txt.json";

    let disk = TransferDisk::new(mount_path);
    disk.ensure_layout()?;
    disk.write_object_atomic(pack_path, &encrypted.ciphertext)?;
    disk.write_metadata(
        relative_meta_path,
        &json!({
            "bucket": bucket,
            "key": key,
            "content_type": "text/plain",
            "etag": "fixture-etag-1"
        }),
    )?;

    let object = ManifestObject {
        object_id: object_id.to_string(),
        bucket: bucket.to_string(),
        key: key.to_string(),
        storage_mode: StorageMode::Pack,
        data_key_id: data_key_id.to_string(),
        pack_ref: Some(ManifestPackRef {
            pack_path: pack_path.to_string(),
            pack_index_path: pack_index_path.to_string(),
            pack_offset_bytes: 0,
            ciphertext_size_bytes: encrypted.ciphertext.len() as u64,
            nonce: encode_base64(&nonce),
            tag: encode_base64(&encrypted.tag),
            aad: String::from_utf8(aad)?,
            ciphertext_sha256: sha256_lower_hex(&encrypted.ciphertext),
        }),
        frames: Vec::new(),
        frame_total: 0,
        relative_meta_path: relative_meta_path.to_string(),
        size_bytes: plaintext.len() as u64,
        estimated_landing_bytes: plaintext.len() as u64 + 4096,
        etag: "fixture-etag-1".to_string(),
        last_modified: "2026-08-09T00:00:00Z".to_string(),
        content_type: "text/plain".to_string(),
        metadata: BTreeMap::new(),
        plaintext_sha256,
        exported_at: "2026-08-09T00:01:00Z".to_string(),
        object_status: ObjectStatus::Exported,
    };
    let manifest = ExportManifest {
        manifest_version: MANIFEST_VERSION.to_string(),
        seal_id: seal_id.to_string(),
        export_job_id: export_job_id.to_string(),
        disk_id: disk_id.to_string(),
        edge_code: edge_code.to_string(),
        create_time: "2026-08-09T00:01:00Z".to_string(),
        objects: vec![object],
    };
    let manifest_sha256 = disk.write_manifest(&manifest)?;
    disk.write_disk_info(&DiskInfo {
        protocol: DiskInfoProtocol {
            name: PROTOCOL_NAME.to_string(),
            version: PROTOCOL_VERSION.to_string(),
        },
        disk: DiskInfoDisk {
            sn: "FIXTURE-SN-SEALED".to_string(),
            disk_id: disk_id.to_string(),
            capacity_bytes: 8 * 1024 * 1024 * 1024,
            last_init_time: "2026-08-09T00:00:00Z".to_string(),
            initialized_by: "fixture-generator".to_string(),
        },
        status: DiskInfoStatus::from_code(DiskStatusCode::Sealed, ""),
        edge: DiskInfoEdge {
            edge_name: "Edge A".to_string(),
            edge_code: edge_code.to_string(),
            seal_id: seal_id.to_string(),
            export_job_id: export_job_id.to_string(),
            export_started_at: "2026-08-09T00:00:30Z".to_string(),
            export_finished_at: "2026-08-09T00:01:00Z".to_string(),
        },
        center: DiskInfoCenter {
            center_id: Uuid::new_v4().to_string(),
            import_job_id: String::new(),
            import_started_at: String::new(),
            import_finished_at: String::new(),
        },
        manifest: DiskInfoManifest {
            object_count: 1,
            total_bytes: plaintext.len() as u64,
            manifest_sha256,
            ..DiskInfoManifest::default()
        },
        security: DiskInfoSecurity {
            center_signature: "fixture-signature".to_string(),
            signature_alg: SIGNATURE_ALG_HMAC_SHA256.to_string(),
            center_key_id: Uuid::new_v4().to_string(),
            encryption_alg: ENCRYPTION_ALG_AES_256_GCM.to_string(),
            data_key_id: data_key_id.to_string(),
        },
    })?;
    fs::write(
        mount_path
            .join(PROTOCOL_ROOT_DIR)
            .join("fixture_import_meta.json"),
        serde_json::to_vec_pretty(&json!({
            "disk_id": disk_id,
            "seal_id": seal_id,
            "data_key_id": data_key_id,
            "disk_data_key_base64": encode_base64(&disk_data_key),
            "edge_code": edge_code
        }))?,
    )?;
    Ok(())
}

fn write_hmac_samples(output: &PathBuf) -> anyhow::Result<()> {
    fs::create_dir_all(output)?;
    let body = br#"{"edge_code":"edge-a","client_version":"1.0.0"}"#;
    let canonical = CanonicalRequest::new(
        "POST",
        "/api/edge/auth",
        &[],
        "2026-08-09T00:00:00Z",
        "fixture-nonce-1",
        body,
    );
    let signature = sign_hmac_base64(b"edge-auth-secret", &canonical);
    fs::write(
        output.join("edge-auth-good.json"),
        serde_json::to_vec_pretty(&json!({
            "method": "POST",
            "path": "/api/edge/auth",
            "headers": {
                "X-Edge-Code": "edge-a",
                "X-Auth-Key-Id": "auth-key-a",
                "X-Timestamp": "2026-08-09T00:00:00Z",
                "X-Nonce": "fixture-nonce-1",
                "X-Body-SHA256": canonical.body_sha256,
                "X-Signature": signature
            },
            "body": serde_json::from_slice::<serde_json::Value>(body)?
        }))?,
    )?;
    fs::write(
        output.join("edge-auth-bad-signature.json"),
        serde_json::to_vec_pretty(&json!({
            "method": "POST",
            "path": "/api/edge/auth",
            "headers": {
                "X-Edge-Code": "edge-a",
                "X-Auth-Key-Id": "auth-key-a",
                "X-Timestamp": "2026-08-09T00:00:00Z",
                "X-Nonce": "fixture-nonce-2",
                "X-Body-SHA256": canonical.body_sha256,
                "X-Signature": "invalid-signature"
            },
            "body": serde_json::from_slice::<serde_json::Value>(body)?
        }))?,
    )?;
    Ok(())
}

fn empty_edge() -> DiskInfoEdge {
    DiskInfoEdge {
        edge_name: String::new(),
        edge_code: String::new(),
        seal_id: String::new(),
        export_job_id: String::new(),
        export_started_at: String::new(),
        export_finished_at: String::new(),
    }
}
