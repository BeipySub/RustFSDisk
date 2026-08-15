use std::{fs, path::PathBuf};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use rustfs_transfer_center::center_security::{
    derive_offline_disk_data_key, sign_disk_info_with_key, verify_disk_info_with_key,
    ENCRYPTION_ALG_AES_256_GCM, SIGNATURE_ALG_HMAC_SHA256,
};
use rustfs_transfer_center::import_worker::{
    ImportErrorCode, ImportOutcome, ImportWorker, MemoryArchiveStorage, MemoryRepository,
    ProgressAggregator, DATA_KEY_STATUS_ISSUED, DATA_KEY_STATUS_SEALED_READONLY,
};
use rustfs_transfer_common::crypto::{pack_object_aad, PackObjectAad};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[test]
fn imports_fixture_sealed_disk() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    let before_disk_info: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("disk_info.json")).unwrap()).unwrap();
    let sealed_signature = before_disk_info["security"]["center_signature"]
        .as_str()
        .expect("sealed disk_info signature")
        .to_string();
    let mut repo = fixture.repository();
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    let outcome = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect("fixture disk imports");

    assert!(matches!(outcome, ImportOutcome::Imported { .. }));
    assert_eq!(repo.ledger().len(), 1);
    assert_eq!(
        storage
            .objects()
            .get(&("archive-edge-a".to_string(), "source/alpha.txt".to_string()))
            .expect("uploaded object"),
        b"hello archive"
    );
    assert_eq!(progress.snapshot().import_job_status, "DONE");
    let data_key = repo
        .data_key_state(fixture.disk_id, fixture.data_key_id)
        .expect("fixture key state");
    assert_eq!(data_key.status, DATA_KEY_STATUS_SEALED_READONLY);
    assert_eq!(data_key.export_job_id, Some(fixture.export_job_id));
    assert_eq!(data_key.seal_id, Some(fixture.seal_id));

    let disk_info: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("disk_info.json")).unwrap()).unwrap();
    assert_eq!(disk_info["status"]["code"], "IMPORTED");
    assert_eq!(disk_info["security"]["center_signature"], sealed_signature);
    assert!(verify_disk_info_with_key(&disk_info, &fixture.signature_key).is_ok());
    assert!(chrono::DateTime::parse_from_rfc3339(
        disk_info["updated_at"]
            .as_str()
            .expect("imported disk_info writes updated_at")
    )
    .is_ok());
}

#[test]
fn skips_same_seal_when_done_with_same_manifest_sha256() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    let mut repo = fixture.repository();
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect("first import succeeds");
    let outcome = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect("same seal skips");

    assert!(matches!(outcome, ImportOutcome::SkippedAlreadyDone { .. }));
    assert_eq!(repo.ledger().len(), 1);
}

#[test]
fn repeated_done_import_backfills_legacy_issued_key_binding() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    let mut repo = fixture.repository();
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect("first import succeeds");
    repo.set_data_key_lifecycle_for_test(
        fixture.disk_id,
        fixture.data_key_id,
        DATA_KEY_STATUS_ISSUED,
        None,
    );

    let outcome = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect("repeat import backfills key binding");

    assert!(matches!(outcome, ImportOutcome::SkippedAlreadyDone { .. }));
    let data_key = repo
        .data_key_state(fixture.disk_id, fixture.data_key_id)
        .expect("fixture key state");
    assert_eq!(data_key.status, DATA_KEY_STATUS_SEALED_READONLY);
    assert_eq!(data_key.export_job_id, Some(fixture.export_job_id));
    assert_eq!(data_key.seal_id, Some(fixture.seal_id));
    assert_eq!(repo.ledger().len(), 1);
}

#[test]
fn rejects_same_seal_with_different_manifest_sha256() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    let mut repo = fixture.repository();
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect("first import succeeds");

    fixture.rewrite_manifest(|manifest| {
        manifest["objects"][0]["etag"] = json!("different-etag");
    });

    let err = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect_err("mismatch is rejected");
    assert_eq!(err.code, ImportErrorCode::SealIdManifestMismatch);
}

#[test]
fn rejects_frames_object_until_frame_import_is_implemented() {
    let fixture = SealedDiskFixture::new_plain("large.bin", b"first half".to_vec());
    fixture.rewrite_manifest(|manifest| {
        let object = &mut manifest["objects"][0];
        let pack_ref = object["pack_ref"].take();
        object["storage_mode"] = json!("FRAMES");
        object["pack_ref"] = Value::Null;
        object["frame_total"] = json!(1);
        object["frames"] = json!([{
            "frame_index": 0,
            "frame_path": pack_ref["pack_path"].clone(),
            "frame_offset_bytes": 0,
            "ciphertext_size_bytes": pack_ref["ciphertext_size_bytes"].clone(),
            "nonce": pack_ref["nonce"].clone(),
            "tag": pack_ref["tag"].clone(),
            "aad": pack_ref["aad"].clone(),
            "ciphertext_sha256": pack_ref["ciphertext_sha256"].clone()
        }]);
    });
    let mut repo = fixture.repository();
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    let err = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect_err("frames objects are rejected until frame import is implemented");

    assert_eq!(err.code, ImportErrorCode::ManifestInvalid);
    assert!(repo.ledger().is_empty());
    assert!(storage.objects().is_empty());
}

#[test]
fn invalid_manifest_path_returns_standard_error_code() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    fixture.rewrite_manifest(|manifest| {
        manifest["objects"][0]["pack_ref"]["pack_path"] = json!("../escape.enc");
    });
    let mut repo = fixture.repository();
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    let err = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect_err("path traversal is rejected");

    assert_eq!(err.code, ImportErrorCode::ManifestInvalid);
    assert!(repo.ledger().is_empty());
}

#[test]
fn aad_mismatch_returns_standard_error_code_before_decrypt() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    fixture.rewrite_manifest(|manifest| {
        manifest["objects"][0]["pack_ref"]["aad"] = json!("disk_id=other");
    });
    let mut repo = fixture.repository();
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    let err = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect_err("aad mismatch is rejected");

    assert_eq!(err.code, ImportErrorCode::ManifestInvalid);
    assert!(repo.ledger().is_empty());
}

#[test]
fn ciphertext_checksum_mismatch_returns_standard_error_code() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    fs::write(
        fixture
            .root
            .join("packs/export-fixture/pack-alpha.txt.pack"),
        b"tampered ciphertext",
    )
    .unwrap();
    let mut repo = fixture.repository();
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    let err = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect_err("tampered ciphertext is rejected");

    assert_eq!(err.code, ImportErrorCode::ChecksumMismatch);
    assert!(repo.ledger().is_empty());
}

#[test]
fn decrypt_failure_marks_job_failed_and_keeps_disk_unimported() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    fixture.rewrite_manifest(|manifest| {
        manifest["objects"][0]["pack_ref"]["tag"] =
            json!(general_purpose::STANDARD.encode([0_u8; 16]));
    });
    let mut repo = fixture.repository();
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    let err = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect_err("bad tag fails decrypt");

    assert_eq!(err.code, ImportErrorCode::DecryptFailed);
    assert!(repo.ledger().is_empty());
    let data_key = repo
        .data_key_state(fixture.disk_id, fixture.data_key_id)
        .expect("fixture key state");
    assert_eq!(data_key.status, DATA_KEY_STATUS_ISSUED);
    assert_eq!(data_key.seal_id, None);
    let disk_info: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("disk_info.json")).unwrap()).unwrap();
    assert_eq!(disk_info["status"]["code"], "SEALED");
}

#[test]
fn offline_disk_data_key_matches_edge_derivation_vector() {
    let key = derive_offline_disk_data_key(
        "edge-secret-fixture",
        "edge-a",
        "11111111-1111-1111-1111-111111111111".parse().unwrap(),
        "22222222-2222-2222-2222-222222222222".parse().unwrap(),
        "33333333-3333-3333-3333-333333333333".parse().unwrap(),
        "44444444-4444-4444-4444-444444444444".parse().unwrap(),
    )
    .unwrap();

    assert_eq!(
        hex::encode(key),
        "16fcbbcde45c48904fbd38a690115ef35287479bb3efd5a3cf5c74c5bb23d633"
    );
}

#[test]
fn wrong_edge_authorization_secret_fails_decrypt() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    let mut repo = fixture.repository();
    repo.put_edge("edge-a", "wrong-edge-secret", "ACTIVE");
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    let err = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect_err("wrong edge auth secret cannot decrypt offline disk");

    assert_eq!(err.code, ImportErrorCode::DecryptFailed);
    assert!(repo.ledger().is_empty());
    assert!(storage.objects().is_empty());
}

#[test]
fn disabled_edge_is_rejected_before_import_claim() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    let mut repo = fixture.repository();
    repo.put_edge("edge-a", fixture.edge_auth_secret.clone(), "DISABLED");
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    let err = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect_err("disabled edge cannot import");

    assert_eq!(err.code, ImportErrorCode::ManifestInvalid);
    assert!(repo.jobs().is_empty());
    assert!(repo.ledger().is_empty());
}

#[test]
fn disabled_disk_is_rejected_before_import_claim() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    let mut repo = fixture.repository();
    repo.disable_disk(fixture.disk_id);
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    let err = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect_err("disabled disk cannot import");

    assert_eq!(err.code, ImportErrorCode::ManifestInvalid);
    assert!(repo.jobs().is_empty());
    assert!(repo.ledger().is_empty());
}

#[test]
fn missing_target_key_does_not_update_other_disk_key() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    let wrong_disk_id = Uuid::new_v4();
    let mut repo = MemoryRepository::default();
    repo.register_disk(fixture.disk_id);
    repo.register_edge("edge-a", fixture.edge_auth_secret.clone());
    repo.put_issued_data_key(
        wrong_disk_id,
        fixture.data_key_id,
        fixture.export_job_id,
        fixture.key.clone(),
    );
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    let err = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect_err("target disk key is missing");

    assert_eq!(err.code, ImportErrorCode::DecryptFailed);
    let data_key = repo
        .data_key_state(wrong_disk_id, fixture.data_key_id)
        .expect("wrong disk key state");
    assert_eq!(data_key.status, DATA_KEY_STATUS_ISSUED);
    assert_eq!(data_key.seal_id, None);
}

#[test]
fn tampered_disk_info_signature_is_rejected_before_import_claim() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    let disk_info_path = fixture.root.join("disk_info.json");
    let mut disk_info: Value = serde_json::from_slice(&fs::read(&disk_info_path).unwrap()).unwrap();
    disk_info["security"]["data_key_id"] = json!(Uuid::new_v4());
    fs::write(
        disk_info_path,
        serde_json::to_vec_pretty(&disk_info).unwrap(),
    )
    .unwrap();
    let mut repo = fixture.repository();
    let mut storage = MemoryArchiveStorage::default();
    let mut progress = ProgressAggregator::default();

    let err = ImportWorker::new(
        &mut repo,
        &mut storage,
        &mut progress,
        fixture.signature_key.clone(),
    )
    .import_sealed_disk(&fixture.root)
    .expect_err("tampered disk_info is rejected");

    assert_eq!(err.code, ImportErrorCode::SignatureInvalid);
    assert!(repo.jobs().is_empty());
}

struct SealedDiskFixture {
    root: PathBuf,
    disk_id: Uuid,
    seal_id: Uuid,
    export_job_id: Uuid,
    data_key_id: Uuid,
    key: Vec<u8>,
    edge_auth_secret: String,
    signature_key: Vec<u8>,
}

impl SealedDiskFixture {
    fn new_plain(key: &str, plaintext: Vec<u8>) -> Self {
        Self::new_object(key, plaintext)
    }

    fn new_object(key: &str, plaintext: Vec<u8>) -> Self {
        let root = std::env::temp_dir().join(format!("rustfs-transfer-test-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("manifests")).unwrap();
        fs::create_dir_all(root.join("packs/export-fixture")).unwrap();
        fs::create_dir_all(root.join("meta")).unwrap();

        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let export_job_id = Uuid::new_v4();
        let data_key_id = Uuid::new_v4();
        let center_key_id = Uuid::new_v4();
        let signature_key = vec![0x31; 32];
        let object_id = Uuid::new_v4();
        let edge_auth_secret = "edge-a-offline-secret".to_string();
        let key_bytes = derive_offline_disk_data_key(
            &edge_auth_secret,
            "edge-a",
            disk_id,
            data_key_id,
            export_job_id,
            seal_id,
        )
        .unwrap()
        .to_vec();
        let nonce_bytes = nonce_for();
        let disk_id_text = disk_id.to_string();
        let seal_id_text = seal_id.to_string();
        let export_job_id_text = export_job_id.to_string();
        let object_id_text = object_id.to_string();
        let plaintext_sha256 = sha256_hex(&plaintext);
        let object_path = format!("packs/export-fixture/pack-{key}.pack");
        let index_path = format!("packs/export-fixture/pack-{key}.idx");
        let aad = pack_object_aad(PackObjectAad {
            disk_id: &disk_id_text,
            seal_id: &seal_id_text,
            export_job_id: &export_job_id_text,
            object_id: &object_id_text,
            bucket: "source",
            object_key: key,
            pack_path: &object_path,
            pack_offset_bytes: 0,
            plaintext_sha256: &plaintext_sha256,
        });
        let (ciphertext, tag) = encrypt(&key_bytes, &nonce_bytes, &aad, &plaintext);
        fs::write(root.join(&object_path), &ciphertext).unwrap();
        fs::write(root.join(&index_path), b"{}").unwrap();
        fs::write(root.join(format!("meta/{key}.json")), b"{}").unwrap();

        let size_bytes = plaintext.len() as u64;
        let object = json!({
            "object_id": object_id,
            "bucket": "source",
            "key": key,
            "storage_mode": "PACK",
            "data_key_id": data_key_id,
            "pack_ref": {
                "pack_path": object_path,
                "pack_index_path": index_path,
                "pack_offset_bytes": 0,
                "ciphertext_size_bytes": ciphertext.len() as u64,
                "nonce": general_purpose::STANDARD.encode(nonce_bytes),
                "tag": general_purpose::STANDARD.encode(tag),
                "aad": String::from_utf8(aad).unwrap(),
                "ciphertext_sha256": sha256_hex(&ciphertext)
            },
            "frames": [],
            "frame_total": 0,
            "relative_meta_path": format!("meta/{key}.json"),
            "size_bytes": size_bytes,
            "etag": "etag-1",
            "last_modified": "2026-08-09T00:00:00Z",
            "content_type": "application/octet-stream",
            "metadata": {},
            "plaintext_sha256": plaintext_sha256,
            "exported_at": "2026-08-09T00:01:00Z",
            "object_status": "EXPORTED",
            "estimated_landing_bytes": size_bytes + 4096
        });
        let manifest = json!({
            "manifest_version": "2.0.0",
            "seal_id": seal_id,
            "export_job_id": export_job_id,
            "disk_id": disk_id,
            "edge_code": "edge-a",
            "create_time": "2026-08-09T00:01:00Z",
            "objects": [object]
        });
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        let manifest_sha = sha256_hex(&manifest_bytes);
        fs::write(root.join("manifests/export_manifest.json"), &manifest_bytes).unwrap();
        fs::write(root.join("manifests/export_manifest.sha256"), &manifest_sha).unwrap();

        let disk_info = json!({
            "protocol": {
                "name": "rustfs-offline-transfer",
                "version": "2.0.0"
            },
            "disk": {
                "disk_id": disk_id,
                "sn": "SN-FIXTURE",
                "capacity_bytes": 1024,
                "last_init_time": "2026-08-09T00:00:00Z",
                "initialized_by": "center"
            },
            "status": {
                "code": "SEALED",
                "sealed": true,
                "imported": false,
                "reusable": false
            },
            "edge": {
                "edge_code": "edge-a",
                "seal_id": seal_id,
                "export_job_id": export_job_id
            },
            "center": {
                "center_id": Uuid::new_v4(),
                "import_job_id": "",
                "import_started_at": "",
                "import_finished_at": ""
            },
            "manifest": {
                "manifest_path": "manifests/export_manifest.json",
                "manifest_sha256_path": "manifests/export_manifest.sha256",
                "object_count": 1,
                "total_bytes": size_bytes,
                "manifest_sha256": manifest_sha
            },
            "security": {
                "center_signature": "",
                "signature_alg": SIGNATURE_ALG_HMAC_SHA256,
                "center_key_id": center_key_id,
                "encryption_alg": ENCRYPTION_ALG_AES_256_GCM,
                "data_key_id": data_key_id
            }
        });
        let mut disk_info = disk_info;
        disk_info["security"]["center_signature"] =
            json!(sign_disk_info_with_key(&disk_info, &signature_key).unwrap());
        fs::write(
            root.join("disk_info.json"),
            serde_json::to_vec_pretty(&disk_info).unwrap(),
        )
        .unwrap();

        Self {
            root,
            disk_id,
            seal_id,
            export_job_id,
            data_key_id,
            key: key_bytes,
            edge_auth_secret,
            signature_key,
        }
    }

    fn repository(&self) -> MemoryRepository {
        let mut repo = MemoryRepository::default();
        repo.register_disk(self.disk_id);
        repo.register_edge("edge-a", self.edge_auth_secret.clone());
        repo.put_data_key(self.disk_id, self.data_key_id, self.key.clone());
        repo
    }

    fn rewrite_manifest(&self, edit: impl FnOnce(&mut Value)) {
        self.rewrite_manifest_inner(edit, true);
    }

    fn rewrite_manifest_inner(&self, edit: impl FnOnce(&mut Value), update_disk_info_sha: bool) {
        let manifest_path = self.root.join("manifests/export_manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        edit(&mut manifest);
        let bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        let manifest_sha = sha256_hex(&bytes);
        fs::write(&manifest_path, bytes).unwrap();
        fs::write(
            self.root.join("manifests/export_manifest.sha256"),
            &manifest_sha,
        )
        .unwrap();
        if update_disk_info_sha {
            let disk_info_path = self.root.join("disk_info.json");
            let mut disk_info: Value =
                serde_json::from_slice(&fs::read(&disk_info_path).unwrap()).unwrap();
            disk_info["manifest"]["manifest_sha256"] = json!(manifest_sha);
            disk_info["security"]["center_signature"] =
                json!(sign_disk_info_with_key(&disk_info, &self.signature_key).unwrap());
            fs::write(
                disk_info_path,
                serde_json::to_vec_pretty(&disk_info).unwrap(),
            )
            .unwrap();
        }
    }
}

fn encrypt(key: &[u8], nonce: &[u8], aad: &[u8], plaintext: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let cipher = Aes256Gcm::new_from_slice(key).unwrap();
    let mut payload = cipher
        .encrypt(
            Nonce::from_slice(nonce),
            aes_gcm::aead::Payload {
                msg: plaintext,
                aad,
            },
        )
        .unwrap();
    let tag = payload.split_off(payload.len() - 16);
    (payload, tag)
}

fn nonce_for() -> [u8; 12] {
    [1_u8; 12]
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
