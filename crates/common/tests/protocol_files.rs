use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use rustfs_transfer_common::error::TransferErrorCode;
use rustfs_transfer_common::protocol::{
    validate_relative_path, DiskInfo, DiskInfoCenter, DiskInfoDisk, DiskInfoEdge, DiskInfoManifest,
    DiskInfoProtocol, DiskInfoSecurity, DiskInfoStatus, DiskStatusCode, ExportManifest,
    ManifestObject, ManifestPackRef, ObjectStatus, StorageMode, TransferDisk, DISK_INFO_PATH,
    ENCRYPTION_ALG_AES_256_GCM, MANIFEST_PATH, MANIFEST_SHA256_PATH, MANIFEST_VERSION,
    PROTOCOL_NAME, PROTOCOL_ROOT_DIR, PROTOCOL_VERSION, SIGNATURE_ALG_HMAC_SHA256,
};
use uuid::Uuid;

#[test]
fn writes_and_reads_protocol_files_atomically() {
    let mount = temp_mount("protocol-files");
    let disk = TransferDisk::new(&mount);
    disk.ensure_layout().unwrap();

    let disk_info = sample_disk_info();
    disk.write_disk_info(&disk_info).unwrap();
    assert_eq!(disk.read_disk_info().unwrap(), disk_info);

    let metadata_path = "meta/object-1.json";
    let metadata = BTreeMap::from([("content-type".to_string(), "text/plain".to_string())]);
    disk.write_metadata(metadata_path, &metadata).unwrap();
    let read_metadata: BTreeMap<String, String> = disk.read_metadata(metadata_path).unwrap();
    assert_eq!(read_metadata, metadata);

    disk.write_object_atomic("packs/export-job-1/pack-000001.pack", b"ciphertext")
        .unwrap();
    assert_eq!(
        fs::read(
            mount
                .join(PROTOCOL_ROOT_DIR)
                .join("packs/export-job-1/pack-000001.pack")
        )
        .unwrap(),
        b"ciphertext"
    );

    let manifest = sample_manifest();
    let sha256 = disk.write_manifest(&manifest).unwrap();
    assert_eq!(disk.read_manifest().unwrap(), manifest);
    assert_eq!(
        fs::read_to_string(mount.join(PROTOCOL_ROOT_DIR).join(MANIFEST_SHA256_PATH))
            .unwrap()
            .trim(),
        sha256
    );
    assert!(mount.join(PROTOCOL_ROOT_DIR).join(DISK_INFO_PATH).is_file());
    assert!(mount.join(PROTOCOL_ROOT_DIR).join(MANIFEST_PATH).is_file());

    remove_temp_mount(mount);
}

#[test]
fn rejects_path_traversal_and_absolute_paths() {
    for bad_path in [
        "/data/object.enc",
        "../data/object.enc",
        "data/../object.enc",
    ] {
        let err = validate_relative_path(bad_path).unwrap_err();
        assert_eq!(err.code, TransferErrorCode::ManifestInvalid);
    }
}

#[test]
fn manifest_checksum_mismatch_returns_checksum_error() {
    let mount = temp_mount("checksum-mismatch");
    let disk = TransferDisk::new(&mount);
    disk.ensure_layout().unwrap();
    disk.write_manifest(&sample_manifest()).unwrap();

    fs::write(
        mount.join(PROTOCOL_ROOT_DIR).join(MANIFEST_SHA256_PATH),
        "0000\n",
    )
    .unwrap();

    let err = disk.read_manifest().unwrap_err();
    assert_eq!(err.code, TransferErrorCode::ChecksumMismatch);

    remove_temp_mount(mount);
}

#[test]
fn partial_scan_returns_count_bytes_and_relative_paths() {
    let mount = temp_mount("partial-scan");
    let disk = TransferDisk::new(&mount);
    disk.ensure_layout().unwrap();
    let root = mount.join(PROTOCOL_ROOT_DIR);

    fs::write(root.join("data/a.enc.partial"), b"1234").unwrap();
    fs::create_dir_all(root.join("meta/nested")).unwrap();
    fs::write(root.join("meta/nested/b.json.partial"), b"12").unwrap();
    fs::write(root.join("data/complete.enc"), b"ok").unwrap();

    let scan = disk.scan_partials().unwrap();
    assert_eq!(scan.count, 2);
    assert_eq!(scan.bytes, 6);
    assert_eq!(
        scan.paths,
        vec![
            "data/a.enc.partial".to_string(),
            "meta/nested/b.json.partial".to_string()
        ]
    );

    remove_temp_mount(mount);
}

#[test]
fn partial_paths_and_non_exported_objects_do_not_enter_valid_manifest() {
    let mount = temp_mount("partial-manifest");
    let disk = TransferDisk::new(&mount);

    let mut partial_manifest = sample_manifest();
    partial_manifest.objects[0]
        .pack_ref
        .as_mut()
        .unwrap()
        .pack_path = "packs/object-1.pack.partial".to_string();
    let err = disk.write_manifest(&partial_manifest).unwrap_err();
    assert_eq!(err.code, TransferErrorCode::ManifestInvalid);

    let mut failed_manifest = sample_manifest();
    failed_manifest.objects[0].object_status = ObjectStatus::Failed;
    let err = disk.write_manifest(&failed_manifest).unwrap_err();
    assert_eq!(err.code, TransferErrorCode::ManifestInvalid);

    remove_temp_mount(mount);
}

fn sample_disk_info() -> DiskInfo {
    DiskInfo {
        protocol: DiskInfoProtocol {
            name: PROTOCOL_NAME.to_string(),
            version: PROTOCOL_VERSION.to_string(),
        },
        disk: DiskInfoDisk {
            disk_id: "disk-1".to_string(),
            sn: "sn-1".to_string(),
            capacity_bytes: 1024,
            last_init_time: "2026-08-09T00:00:00Z".to_string(),
            initialized_by: "center".to_string(),
        },
        status: DiskInfoStatus::from_code(DiskStatusCode::Initialized, ""),
        edge: DiskInfoEdge {
            edge_code: String::new(),
            edge_name: String::new(),
            seal_id: String::new(),
            export_job_id: String::new(),
            export_started_at: String::new(),
            export_finished_at: String::new(),
        },
        center: DiskInfoCenter {
            center_id: "center-1".to_string(),
            import_job_id: String::new(),
            import_started_at: String::new(),
            import_finished_at: String::new(),
        },
        manifest: DiskInfoManifest::default(),
        security: DiskInfoSecurity {
            center_signature: "sig".to_string(),
            signature_alg: SIGNATURE_ALG_HMAC_SHA256.to_string(),
            center_key_id: "center-key-1".to_string(),
            encryption_alg: ENCRYPTION_ALG_AES_256_GCM.to_string(),
            data_key_id: "data-key-1".to_string(),
        },
    }
}

fn sample_manifest() -> ExportManifest {
    ExportManifest {
        manifest_version: MANIFEST_VERSION.to_string(),
        seal_id: "seal-1".to_string(),
        export_job_id: "export-job-1".to_string(),
        disk_id: "disk-1".to_string(),
        edge_code: "edge-a".to_string(),
        create_time: "2026-08-09T00:00:00Z".to_string(),
        objects: vec![ManifestObject {
            object_id: "11111111-1111-1111-1111-111111111111".to_string(),
            bucket: "bucket".to_string(),
            key: "object.txt".to_string(),
            storage_mode: StorageMode::Pack,
            data_key_id: "data-key-1".to_string(),
            pack_ref: Some(ManifestPackRef {
                pack_path: "packs/export-job-1/pack-000001.pack".to_string(),
                pack_index_path: "packs/export-job-1/pack-000001.idx".to_string(),
                pack_offset_bytes: 0,
                ciphertext_size_bytes: 10,
                nonce: "nonce".to_string(),
                tag: "tag".to_string(),
                aad: "aad".to_string(),
                ciphertext_sha256: "cipher-sha".to_string(),
            }),
            frames: Vec::new(),
            frame_total: 0,
            relative_meta_path: "meta/object-1.json".to_string(),
            size_bytes: 10,
            estimated_landing_bytes: 4096,
            etag: "etag".to_string(),
            last_modified: "2026-08-09T00:00:00Z".to_string(),
            content_type: "text/plain".to_string(),
            metadata: BTreeMap::new(),
            plaintext_sha256: "plain-sha".to_string(),
            exported_at: "2026-08-09T00:00:01Z".to_string(),
            object_status: ObjectStatus::Exported,
        }],
    }
}

fn temp_mount(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("rustfs-transfer-{name}-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn remove_temp_mount(path: PathBuf) {
    let _ = fs::remove_dir_all(path);
}
