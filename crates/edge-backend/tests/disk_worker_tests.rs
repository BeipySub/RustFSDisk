use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use aes_gcm::aead::{AeadInPlace, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce, Tag};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::Utc;
use rustfs_transfer_edge::disk_worker::{
    DiskWorker, DiskWorkerConfig, ExportObjectRepository, ExportObjectTask, ExportedObjectUpdate,
    ObjectSource, Result, SourceObjectHead,
};
use rustfs_transfer_edge::progress::ProgressAggregator;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("rustfs-edge-worker-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone)]
struct SourceObject {
    bytes: Vec<u8>,
    head: SourceObjectHead,
}

#[derive(Default)]
struct MemorySource {
    objects: BTreeMap<(String, String), SourceObject>,
}

impl MemorySource {
    fn insert(&mut self, bucket: &str, key: &str, bytes: &[u8]) -> SourceObjectHead {
        let head = SourceObjectHead {
            etag: format!("etag-{}", hex::encode(Sha256::digest(bytes))),
            size_bytes: bytes.len() as u64,
            last_modified: Utc::now(),
            content_type: Some("application/octet-stream".to_string()),
            metadata: BTreeMap::from([("owner".to_string(), "edge".to_string())]),
        };
        self.objects.insert(
            (bucket.to_string(), key.to_string()),
            SourceObject {
                bytes: bytes.to_vec(),
                head: head.clone(),
            },
        );
        head
    }
}

impl ObjectSource for MemorySource {
    fn head_object(&self, bucket: &str, key: &str) -> Result<SourceObjectHead> {
        Ok(self
            .objects
            .get(&(bucket.to_string(), key.to_string()))
            .expect("source object exists")
            .head
            .clone())
    }

    fn open_object(
        &self,
        bucket: &str,
        key: &str,
        offset: u64,
        length: u64,
    ) -> Result<Box<dyn Read>> {
        let object = self
            .objects
            .get(&(bucket.to_string(), key.to_string()))
            .expect("source object exists");
        let start = offset as usize;
        let end = start + length as usize;
        Ok(Box::new(Cursor::new(object.bytes[start..end].to_vec())))
    }
}

#[derive(Default)]
struct MemoryRepo {
    assigned: RefCell<Vec<ExportObjectTask>>,
    exported: RefCell<Vec<ExportedObjectUpdate>>,
    failed: RefCell<Vec<(i64, String, String)>>,
    runtime: RefCell<Vec<(String, Option<String>)>>,
}

impl ExportObjectRepository for MemoryRepo {
    fn assigned_objects(
        &self,
        _export_job_id: Uuid,
        disk_id: Uuid,
    ) -> Result<Vec<ExportObjectTask>> {
        Ok(self
            .assigned
            .borrow()
            .iter()
            .filter(|object| object.id > 0)
            .filter(|_| disk_id != Uuid::nil())
            .cloned()
            .collect())
    }

    fn mark_copying(&self, _object_id: i64, _partial_path: &str) -> Result<()> {
        Ok(())
    }

    fn mark_exported(&self, _object_id: i64, exported: &ExportedObjectUpdate) -> Result<()> {
        self.exported.borrow_mut().push(exported.clone());
        Ok(())
    }

    fn mark_failed(&self, object_id: i64, error_code: &str, error_message: &str) -> Result<()> {
        self.failed.borrow_mut().push((
            object_id,
            error_code.to_string(),
            error_message.to_string(),
        ));
        Ok(())
    }

    fn load_exported_objects(
        &self,
        _export_job_id: Uuid,
        _disk_id: Uuid,
    ) -> Result<Vec<ExportedObjectUpdate>> {
        Ok(self.exported.borrow().clone())
    }

    fn mark_disk_runtime(
        &self,
        _disk_id: Uuid,
        runtime_status: &str,
        error_code: Option<&str>,
    ) -> Result<()> {
        self.runtime
            .borrow_mut()
            .push((runtime_status.to_string(), error_code.map(str::to_string)));
        Ok(())
    }

    fn mark_disk_runtime_done_after_seal(&self, _disk_id: Uuid) -> Result<()> {
        self.runtime.borrow_mut().push(("DONE".to_string(), None));
        Ok(())
    }

    fn mark_job_sealed_checkpoint(
        &self,
        _export_job_id: Uuid,
        _copied_count: u64,
        _copied_bytes: u64,
    ) -> Result<()> {
        Ok(())
    }
}

#[test]
fn worker_encrypts_writes_manifest_and_seals_disk() {
    let temp = TempDir::new();
    let protocol_root = temp.path().join("rustfs-transfer");
    fs::create_dir_all(&protocol_root).expect("create protocol root");

    let disk_id = Uuid::new_v4();
    let export_job_id = Uuid::new_v4();
    let seal_id = Uuid::new_v4();
    let data_key_id = Uuid::new_v4();
    fs::write(
        protocol_root.join("disk_info.json"),
        serde_json::to_vec_pretty(&json!({
            "disk": { "disk_id": disk_id, "sn": "sn-1", "capacity_bytes": 1000000u64 },
            "status": { "code": "INITIALIZED", "sealed": false, "imported": false, "reusable": true, "last_error": "" },
            "security": { "data_key_id": data_key_id, "encryption_alg": "AES-256-GCM" }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut source = MemorySource::default();
    let bytes = b"hello rustfs transfer disk";
    let head = source.insert("bucket-a", "folder/object.txt", bytes);
    let repo = MemoryRepo::default();
    repo.assigned.borrow_mut().push(ExportObjectTask {
        id: 7,
        object_id: Uuid::new_v4(),
        bucket: "bucket-a".to_string(),
        object_key: "folder/object.txt".to_string(),
        etag: head.etag.clone(),
        size_bytes: head.size_bytes,
        last_modified: head.last_modified + chrono::Duration::milliseconds(494),
    });

    let key = [9_u8; 32];
    let config = DiskWorkerConfig {
        disk_id,
        disk_sn: "sn-1".to_string(),
        mount_path: temp.path().to_path_buf(),
        edge_code: "edge-a".to_string(),
        edge_name: "Edge A".to_string(),
        export_job_id,
        seal_id,
        data_key_id,
        disk_data_key: key,
        free_bytes: 1000000,
    };
    let progress = ProgressAggregator::new("edge-a", export_job_id.to_string());
    let worker = DiskWorker::new(config, &source, &repo, progress.clone());

    let manifest = worker.run().expect("worker succeeds");

    assert_eq!(manifest.seal_id, seal_id);
    assert_eq!(
        manifest.objects.len(),
        1,
        "failed: {:?}",
        repo.failed.borrow()
    );
    assert_eq!(manifest.objects[0].object_status, "EXPORTED");
    assert_eq!(
        manifest.objects[0].storage_mode,
        rustfs_transfer_common::protocol::StorageMode::Pack
    );
    assert_eq!(manifest.objects[0].frame_total, 0);
    assert_eq!(
        manifest.objects[0].pack_ref.pack_path,
        format!("packs/{export_job_id}/pack-7.pack")
    );

    let manifest_bytes = fs::read(protocol_root.join("manifests/export_manifest.json")).unwrap();
    let manifest_sha =
        fs::read_to_string(protocol_root.join("manifests/export_manifest.sha256")).unwrap();
    assert_eq!(manifest_sha, hex::encode(Sha256::digest(&manifest_bytes)));

    let disk_info: serde_json::Value =
        serde_json::from_slice(&fs::read(protocol_root.join("disk_info.json")).unwrap()).unwrap();
    assert_eq!(disk_info["status"]["code"], "SEALED");
    assert_eq!(disk_info["edge"]["seal_id"], seal_id.to_string());
    assert_eq!(disk_info["manifest"]["manifest_sha256"], manifest_sha);

    let object = &manifest.objects[0];
    assert!(object.pack_ref.aad.contains("PACK_OBJECT"));
    let mut ciphertext = fs::read(protocol_root.join(&object.pack_ref.pack_path)).unwrap();
    let nonce = BASE64.decode(&object.pack_ref.nonce).unwrap();
    let tag = BASE64.decode(&object.pack_ref.tag).unwrap();
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    cipher
        .decrypt_in_place_detached(
            Nonce::from_slice(&nonce),
            object.pack_ref.aad.as_bytes(),
            &mut ciphertext,
            Tag::from_slice(&tag),
        )
        .expect("decrypt exported object");
    assert_eq!(ciphertext, bytes);

    let disk_text = collect_text_files(&protocol_root);
    assert!(!disk_text.contains(&BASE64.encode(key)));
    assert!(repo.failed.borrow().is_empty());
    assert_eq!(
        repo.runtime.borrow().as_slice(),
        &[("COPYING".to_string(), None), ("DONE".to_string(), None)]
    );
    assert_eq!(repo.exported.borrow().len(), 1);
    assert_eq!(
        progress
            .snapshot("COPY_PROGRESS", "snapshot")
            .global_progress
            .done_bytes,
        bytes.len() as u64
    );
}

fn collect_text_files(root: &Path) -> String {
    let mut out = String::new();
    collect_text_files_inner(root, &mut out);
    out
}

fn collect_text_files_inner(path: &Path, out: &mut String) {
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_text_files_inner(&path, out);
        } else if let Ok(text) = fs::read_to_string(&path) {
            out.push_str(&text);
        }
    }
}
