use std::{fs, path::PathBuf};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use rustfs_transfer_center::center_security::{
    sign_disk_info_with_key, verify_disk_info_with_key, ENCRYPTION_ALG_AES_256_GCM,
    SIGNATURE_ALG_HMAC_SHA256,
};
use rustfs_transfer_center::import_worker::{
    ImportErrorCode, ImportOutcome, ImportWorker, MemoryArchiveStorage, MemoryRepository,
    ProgressAggregator, DATA_KEY_STATUS_ISSUED, DATA_KEY_STATUS_SEALED_READONLY,
};
use rustfs_transfer_common::crypto::{object_aad, ObjectAad};
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
fn does_not_write_ledger_for_missing_cross_disk_chunk() {
    let fixture = SealedDiskFixture::new_chunk("large.bin", b"first half".to_vec(), 0, 2);
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
    .expect("single valid chunk registers");

    assert!(repo.ledger().is_empty());
    assert!(storage.objects().is_empty());
    assert_eq!(progress.snapshot().import_job_status, "DONE");
}

#[test]
fn invalid_manifest_path_returns_standard_error_code() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    fixture.rewrite_manifest(|manifest| {
        manifest["objects"][0]["relative_data_path"] = json!("../escape.enc");
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
        manifest["objects"][0]["aad"] = json!("disk_id=other");
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
        fixture.root.join("data/alpha.txt.enc"),
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
        manifest["objects"][0]["tag"] = json!(general_purpose::STANDARD.encode([0_u8; 16]));
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
fn missing_target_key_does_not_update_other_disk_key() {
    let fixture = SealedDiskFixture::new_plain("alpha.txt", b"hello archive".to_vec());
    let wrong_disk_id = Uuid::new_v4();
    let mut repo = MemoryRepository::default();
    repo.register_disk(fixture.disk_id);
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
    signature_key: Vec<u8>,
}

impl SealedDiskFixture {
    fn new_plain(key: &str, plaintext: Vec<u8>) -> Self {
        Self::new_object(key, plaintext, false, 0, 1)
    }

    fn new_chunk(key: &str, plaintext: Vec<u8>, chunk_index: u32, chunk_total: u32) -> Self {
        Self::new_object(key, plaintext, true, chunk_index, chunk_total)
    }

    fn new_object(
        key: &str,
        plaintext: Vec<u8>,
        chunked: bool,
        chunk_index: u32,
        chunk_total: u32,
    ) -> Self {
        let root = std::env::temp_dir().join(format!("rustfs-transfer-test-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("manifests")).unwrap();
        fs::create_dir_all(root.join("data")).unwrap();
        fs::create_dir_all(root.join("meta")).unwrap();

        let disk_id = Uuid::new_v4();
        let seal_id = Uuid::new_v4();
        let export_job_id = Uuid::new_v4();
        let data_key_id = Uuid::new_v4();
        let center_key_id = Uuid::new_v4();
        let signature_key = vec![0x31; 32];
        let chunk_group_id = Uuid::new_v4();
        let key_bytes = vec![7_u8; 32];
        let nonce_bytes = nonce_for(chunk_index);
        let disk_id_text = disk_id.to_string();
        let seal_id_text = seal_id.to_string();
        let export_job_id_text = export_job_id.to_string();
        let chunk_group_id_text = chunk_group_id.to_string();
        let aad = object_aad(ObjectAad {
            disk_id: &disk_id_text,
            seal_id: &seal_id_text,
            export_job_id: &export_job_id_text,
            bucket: "source",
            object_key: key,
            chunk_group_id: if chunked {
                Some(chunk_group_id_text.as_str())
            } else {
                None
            },
            chunk_index,
            chunk_total,
            chunk_offset_bytes: chunk_index as u64 * plaintext.len() as u64,
        });
        let (ciphertext, tag) = encrypt(&key_bytes, &nonce_bytes, &aad, &plaintext);
        let object_path = format!("data/{key}.enc");
        fs::write(root.join(&object_path), &ciphertext).unwrap();
        fs::write(root.join(format!("meta/{key}.json")), b"{}").unwrap();

        let plain_hash = if chunked {
            sha256_hex(b"first halfsecond half")
        } else {
            sha256_hex(&plaintext)
        };
        let chunk_size_bytes = plaintext.len() as u64;
        let size_bytes = if chunked {
            chunk_size_bytes * chunk_total as u64
        } else {
            chunk_size_bytes
        };
        let object = json!({
            "bucket": "source",
            "key": key,
            "relative_data_path": object_path,
            "encrypted": true,
            "encryption_alg": ENCRYPTION_ALG_AES_256_GCM,
            "data_key_id": data_key_id,
            "nonce": general_purpose::STANDARD.encode(nonce_bytes),
            "tag": general_purpose::STANDARD.encode(tag),
            "aad": String::from_utf8(aad).unwrap(),
            "ciphertext_size_bytes": ciphertext.len() as u64,
            "ciphertext_sha256": sha256_hex(&ciphertext),
            "chunked": chunked,
            "chunk_group_id": if chunked { chunk_group_id.to_string() } else { String::new() },
            "chunk_index": chunk_index,
            "chunk_total": chunk_total,
            "chunk_offset_bytes": chunk_index as u64 * chunk_size_bytes,
            "chunk_size_bytes": chunk_size_bytes,
            "chunk_sha256": sha256_hex(&ciphertext),
            "relative_meta_path": format!("meta/{key}.json"),
            "size_bytes": size_bytes,
            "etag": "etag-1",
            "last_modified": "2026-08-09T00:00:00Z",
            "content_type": "application/octet-stream",
            "metadata": {},
            "plaintext_sha256": plain_hash,
            "exported_at": "2026-08-09T00:01:00Z",
            "object_status": "EXPORTED"
        });
        let manifest = json!({
            "manifest_version": "1.0.0",
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
                "version": "1.0.0"
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
                "total_bytes": chunk_size_bytes,
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
            signature_key,
        }
    }

    fn repository(&self) -> MemoryRepository {
        let mut repo = MemoryRepository::default();
        repo.register_disk(self.disk_id);
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

fn nonce_for(index: u32) -> [u8; 12] {
    let mut nonce = [1_u8; 12];
    nonce[11] = index as u8 + 1;
    nonce
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
