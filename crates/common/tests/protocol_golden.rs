use std::collections::BTreeMap;

use rustfs_transfer_common::error::TransferErrorCode;
use rustfs_transfer_common::protocol::*;
use serde_json::{json, Value};

fn assert_round_trip<T>(value: &T, expected: Value)
where
    T: std::fmt::Debug + PartialEq + serde::de::DeserializeOwned + serde::Serialize,
{
    let serialized = serde_json::to_value(value).expect("serialize golden value");
    assert_eq!(serialized, expected);
    let deserialized: T = serde_json::from_value(serialized).expect("deserialize golden value");
    assert_eq!(&deserialized, value);
}

fn assert_no_bare_status(value: &Value) {
    match value {
        Value::Object(map) => {
            assert!(
                !map.contains_key("status"),
                "payload must not contain bare status key: {value}"
            );
            for child in map.values() {
                assert_no_bare_status(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                assert_no_bare_status(item);
            }
        }
        _ => {}
    }
}

#[test]
fn disk_info_golden_json_allows_nested_status_object_only() {
    let disk_info = DiskInfo {
        protocol: DiskInfoProtocol {
            name: PROTOCOL_NAME.to_owned(),
            version: PROTOCOL_VERSION.to_owned(),
        },
        disk: DiskInfoDisk {
            sn: "SN-001".to_owned(),
            disk_id: "disk-001".to_owned(),
            capacity_bytes: 1_000_000,
            last_init_time: "2026-08-09T08:00:00Z".to_owned(),
            initialized_by: "center".to_owned(),
        },
        status: DiskInfoStatus::from_code(DiskStatusCode::Initialized, ""),
        edge: DiskInfoEdge {
            edge_name: String::new(),
            edge_code: String::new(),
            seal_id: String::new(),
            export_job_id: String::new(),
            export_started_at: String::new(),
            export_finished_at: String::new(),
        },
        center: DiskInfoCenter {
            center_id: "center-001".to_owned(),
            import_job_id: String::new(),
            import_started_at: String::new(),
            import_finished_at: String::new(),
        },
        manifest: DiskInfoManifest::default(),
        security: DiskInfoSecurity {
            center_signature: "signature".to_owned(),
            signature_alg: SIGNATURE_ALG_HMAC_SHA256.to_owned(),
            center_key_id: "center-key-001".to_owned(),
            encryption_alg: ENCRYPTION_ALG_AES_256_GCM.to_owned(),
            data_key_id: "data-key-001".to_owned(),
        },
    };

    assert_round_trip(
        &disk_info,
        json!({
            "protocol": {"name": "rustfs-offline-transfer", "version": "2.0.0"},
            "disk": {
                "sn": "SN-001",
                "disk_id": "disk-001",
                "capacity_bytes": 1000000,
                "last_init_time": "2026-08-09T08:00:00Z",
                "initialized_by": "center"
            },
            "status": {
                "code": "INITIALIZED",
                "sealed": false,
                "imported": false,
                "reusable": false,
                "last_error": ""
            },
            "edge": {
                "edge_name": "",
                "edge_code": "",
                "seal_id": "",
                "export_job_id": "",
                "export_started_at": "",
                "export_finished_at": ""
            },
            "center": {
                "center_id": "center-001",
                "import_job_id": "",
                "import_started_at": "",
                "import_finished_at": ""
            },
            "manifest": {
                "manifest_path": "manifests/export_manifest.json",
                "manifest_sha256_path": "manifests/export_manifest.sha256",
                "object_count": 0,
                "total_bytes": 0,
                "manifest_sha256": ""
            },
            "security": {
                "center_signature": "signature",
                "signature_alg": "HMAC-SHA256",
                "center_key_id": "center-key-001",
                "encryption_alg": "AES-256-GCM",
                "data_key_id": "data-key-001"
            }
        }),
    );
}

#[test]
fn export_manifest_golden_json_has_prefixed_object_status() {
    let manifest = ExportManifest {
        manifest_version: MANIFEST_VERSION.to_owned(),
        seal_id: "seal-001".to_owned(),
        export_job_id: "export-job-001".to_owned(),
        disk_id: "disk-001".to_owned(),
        edge_code: "edge-a".to_owned(),
        create_time: "2026-08-09T08:10:00Z".to_owned(),
        objects: vec![ManifestObject {
            object_id: "object-001".to_owned(),
            bucket: "source-bucket".to_owned(),
            key: "dir/file.txt".to_owned(),
            storage_mode: StorageMode::Pack,
            data_key_id: "data-key-001".to_owned(),
            pack_ref: Some(ManifestPackRef {
                pack_path: "packs/export-job-001/pack-000001.pack".to_owned(),
                pack_index_path: "packs/export-job-001/pack-000001.idx".to_owned(),
                pack_offset_bytes: 0,
                ciphertext_size_bytes: 42,
                nonce: "nonce-001".to_owned(),
                tag: "tag-001".to_owned(),
                aad: "{\"aad_type\":\"PACK_OBJECT\"}".to_owned(),
                ciphertext_sha256: "cipher-sha".to_owned(),
            }),
            frames: Vec::new(),
            frame_total: 0,
            relative_meta_path: "meta/source-bucket/dir/file.txt.json".to_owned(),
            size_bytes: 40,
            estimated_landing_bytes: 4096,
            etag: "\"etag\"".to_owned(),
            last_modified: "2026-08-09T08:05:00Z".to_owned(),
            content_type: "text/plain".to_owned(),
            metadata: BTreeMap::from([("owner".to_owned(), "qa".to_owned())]),
            plaintext_sha256: "plain-sha".to_owned(),
            exported_at: "2026-08-09T08:11:00Z".to_owned(),
            object_status: ObjectStatus::Exported,
        }],
    };

    let expected = json!({
        "manifest_version": "2.0.0",
        "seal_id": "seal-001",
        "export_job_id": "export-job-001",
        "disk_id": "disk-001",
        "edge_code": "edge-a",
        "create_time": "2026-08-09T08:10:00Z",
        "objects": [{
            "object_id": "object-001",
            "bucket": "source-bucket",
            "key": "dir/file.txt",
            "storage_mode": "PACK",
            "data_key_id": "data-key-001",
            "pack_ref": {
                "pack_path": "packs/export-job-001/pack-000001.pack",
                "pack_index_path": "packs/export-job-001/pack-000001.idx",
                "pack_offset_bytes": 0,
                "ciphertext_size_bytes": 42,
                "nonce": "nonce-001",
                "tag": "tag-001",
                "aad": "{\"aad_type\":\"PACK_OBJECT\"}",
                "ciphertext_sha256": "cipher-sha"
            },
            "frame_total": 0,
            "relative_meta_path": "meta/source-bucket/dir/file.txt.json",
            "size_bytes": 40,
            "estimated_landing_bytes": 4096,
            "etag": "\"etag\"",
            "last_modified": "2026-08-09T08:05:00Z",
            "content_type": "text/plain",
            "metadata": {"owner": "qa"},
            "plaintext_sha256": "plain-sha",
            "exported_at": "2026-08-09T08:11:00Z",
            "object_status": "EXPORTED"
        }]
    });

    assert_round_trip(&manifest, expected.clone());
    assert_no_bare_status(&expected);
}

#[test]
fn http_payload_golden_json_uses_semantic_status_fields() {
    let auth_response = EdgeAuthResponse {
        allowed: true,
        edge_code: "edge-a".to_owned(),
        edge_name: "Edge A".to_owned(),
        edge_status: EdgeStatus::Active,
        server_time: "2026-08-09T08:12:00Z".to_owned(),
        message: None,
    };
    let verify_request = DiskVerifyRequest {
        edge_code: "edge-a".to_owned(),
        disk_id: "disk-001".to_owned(),
        sn: Some("SN-001".to_owned()),
        capacity_bytes: 1_000_000,
        free_bytes: 900_000,
        status_code: DiskStatusCode::Initialized,
        protocol_version: PROTOCOL_VERSION.to_owned(),
    };
    let verify_response = DiskVerifyResponse {
        allowed: true,
        disk_id: "disk-001".to_owned(),
        disk_enabled: true,
        expected_status: DiskStatusCode::Initialized,
        action: DiskVerifyAction::AllowExport,
        message: None,
    };
    let export_key_response = DiskExportKeyResponse {
        allowed: true,
        data_key_id: "data-key-001".to_owned(),
        encryption_alg: ENCRYPTION_ALG_AES_256_GCM.to_owned(),
        disk_data_key: Some("base64-key".to_owned()),
        message: None,
    };
    let disk_detail = DiskDetailResponse {
        disk_id: "disk-001".to_owned(),
        sn: Some("SN-001".to_owned()),
        enabled: true,
        last_init_time: Some("2026-08-09T08:00:00Z".to_owned()),
        remark: Some("lab disk".to_owned()),
    };
    let error = ApiErrorResponse {
        error_code: TransferErrorCode::InvalidStatus,
        message: "status is not allowed for this action".to_owned(),
        request_id: "req-001".to_owned(),
    };

    let payloads = [
        serde_json::to_value(auth_response).unwrap(),
        serde_json::to_value(verify_request).unwrap(),
        serde_json::to_value(verify_response).unwrap(),
        serde_json::to_value(export_key_response).unwrap(),
        serde_json::to_value(disk_detail).unwrap(),
        serde_json::to_value(error).unwrap(),
    ];

    assert_eq!(payloads[0]["edge_status"], "ACTIVE");
    assert_eq!(payloads[1]["status_code"], "INITIALIZED");
    assert_eq!(payloads[2]["disk_enabled"], true);
    assert_eq!(payloads[3]["encryption_alg"], "AES-256-GCM");
    assert_eq!(payloads[4]["enabled"], true);
    assert_eq!(payloads[5]["error_code"], "INVALID_STATUS");

    for payload in payloads {
        assert_no_bare_status(&payload);
    }
}

#[test]
fn websocket_progress_golden_json_uses_prefixed_status_fields() {
    let event = CopyProgressEvent {
        event_type: WebSocketEventType::CopyProgress,
        event_time: "2026-08-09T08:13:00Z".to_owned(),
        source: EventSource::Edge,
        edge_code: "edge-a".to_owned(),
        export_job_id: "export-job-001".to_owned(),
        disk_status_code: DiskStatusCode::EdgeCopying,
        export_job_status: ExportJobStatus::Copying,
        global_progress: ProgressSummary {
            total_bytes: 100,
            done_bytes: 40,
            remaining_bytes: 60,
            speed_bytes_per_sec: 10,
            object_total: 2,
            object_done: 1,
            object_remaining: 1,
        },
        disks: vec![DiskCopyProgress {
            disk_id: "disk-001".to_owned(),
            disk_sn: "SN-001".to_owned(),
            mount_path: "/mnt/rustfs-transfer/disk-001".to_owned(),
            runtime_status: RuntimeStatus::Copying,
            total_bytes: 100,
            done_bytes: 40,
            remaining_bytes: 60,
            capacity_bytes: 1_000_000,
            free_bytes: 900_000,
            reserve_bytes: 1,
            object_budget_bytes: 899_999,
            speed_bytes_per_sec: 10,
            object_total: 2,
            object_done: 1,
            object_remaining: 1,
            current_object: Some(CurrentObjectProgress {
                bucket: "source-bucket".to_owned(),
                object_id: "object-001".to_owned(),
                storage_mode: StorageMode::Pack,
                key: "dir/file.txt".to_owned(),
                display_name: "file.txt".to_owned(),
                size_bytes: 100,
                done_bytes: 40,
                remaining_bytes: 60,
                speed_bytes_per_sec: 10,
                frame_index: 0,
                frame_total: 0,
                object_status: ObjectStatus::Copying,
            }),
            message: "copying".to_owned(),
        }],
        message: "copying objects".to_owned(),
    };

    let serialized = serde_json::to_value(&event).unwrap();
    assert_eq!(serialized["event_type"], "COPY_PROGRESS");
    assert_eq!(serialized["source"], "edge");
    assert_eq!(serialized["disk_status_code"], "EDGE_COPYING");
    assert_eq!(serialized["export_job_status"], "COPYING");
    assert_eq!(serialized["disks"][0]["runtime_status"], "COPYING");
    assert_eq!(
        serialized["disks"][0]["current_object"]["object_status"],
        "COPYING"
    );
    assert_no_bare_status(&serialized);

    let deserialized: CopyProgressEvent = serde_json::from_value(serialized).unwrap();
    assert_eq!(deserialized, event);
}
