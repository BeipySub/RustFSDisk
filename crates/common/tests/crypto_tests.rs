use rustfs_transfer_common::crypto::{
    canonical_path_with_query, center_signature_canonical_json, decode_base64, decode_hex,
    decrypt_aes256_gcm, encode_base64, encode_hex_lower, encrypt_aes256_gcm, generate_nonce,
    pack_object_aad, sha256_lower_hex, sign_center_signature, sign_hmac_base64,
    verify_center_signature, verify_hmac_base64, CanonicalRequest, CryptoError, PackObjectAad,
    QueryParam, AES_GCM_NONCE_LEN,
};
use serde_json::json;

#[test]
fn sha256_returns_lowercase_hex() {
    assert_eq!(
        sha256_lower_hex(b"RustFS"),
        "f09a72e34ce1f6de32ade53a00d67a673175e3b834cc97d1b1276d5143a790d1"
    );
    assert!(sha256_lower_hex(b"ABCDEF")
        .chars()
        .all(|ch| !ch.is_ascii_uppercase()));
}

#[test]
fn hmac_signs_and_verifies_canonical_request() {
    let request = CanonicalRequest::new(
        "post",
        "/api/disk/verify",
        &[
            QueryParam::new("z", "last"),
            QueryParam::new("a", "space value"),
            QueryParam::new("a", "first"),
        ],
        "2026-08-08T12:00:00Z",
        "nonce-001",
        br#"{"edge_code":"edge-a"}"#,
    );

    assert_eq!(
        request.canonical_path_with_query,
        "/api/disk/verify?a=first&a=space%20value&z=last"
    );
    assert_eq!(
        request.string_to_sign(),
        "POST\n/api/disk/verify?a=first&a=space%20value&z=last\n2026-08-08T12:00:00Z\nnonce-001\n145fee78051a15bd574d960e8159aad8bb477264c245925e406fca8be4e9b3d7"
    );

    let signature = sign_hmac_base64(b"edge_auth_secret_for_http_only", &request);
    verify_hmac_base64(b"edge_auth_secret_for_http_only", &request, &signature).unwrap();
    assert_eq!(
        verify_hmac_base64(b"wrong-secret", &request, &signature),
        Err(CryptoError::HmacVerificationFailed)
    );
}

#[test]
fn hmac_empty_body_uses_empty_byte_sha256() {
    let request = CanonicalRequest::new("GET", "", &[], "2026-08-08T12:00:00Z", "n", b"");

    assert_eq!(request.canonical_path_with_query, "/");
    assert_eq!(
        request.body_sha256,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn center_signature_covers_only_protocol_disk_and_key_references() {
    let key = b"center-signature-key";
    let mut disk_info = disk_info_for_center_signature();
    let signature = sign_center_signature(key, &disk_info).unwrap();
    disk_info["security"]["center_signature"] = json!(signature);

    verify_center_signature(key, &disk_info).unwrap();
    let canonical = center_signature_canonical_json(&disk_info).unwrap();
    assert!(canonical.contains("\"protocol\""));
    assert!(canonical.contains("\"disk\""));
    assert!(canonical.contains("\"security\""));
    assert!(!canonical.contains("\"center\""));
    assert!(!canonical.contains("\"status\""));
    assert!(!canonical.contains("\"edge\""));
    assert!(!canonical.contains("\"manifest\""));
    assert!(!canonical.contains("center_signature"));

    for path in ["center", "status", "edge", "manifest"] {
        let mut changed = disk_info.clone();
        changed[path] = json!({"changed": true});
        verify_center_signature(key, &changed).unwrap();
    }
}

#[test]
fn center_signature_rejects_any_covered_field_change() {
    let key = b"center-signature-key";
    let mut disk_info = disk_info_for_center_signature();
    let signature = sign_center_signature(key, &disk_info).unwrap();
    disk_info["security"]["center_signature"] = json!(signature);

    for (path, value) in [
        ("/protocol/name", json!("other-protocol")),
        ("/protocol/version", json!("9.9.9")),
        ("/disk/disk_id", json!("changed-disk")),
        ("/disk/capacity_bytes", json!(2048)),
        ("/security/center_key_id", json!("changed-center-key")),
        ("/security/signature_alg", json!("OTHER")),
        ("/security/data_key_id", json!("changed-data-key")),
    ] {
        let mut changed = disk_info.clone();
        *changed.pointer_mut(path).expect("fixture field exists") = value;
        assert_eq!(
            verify_center_signature(key, &changed),
            Err(CryptoError::CenterSignatureVerificationFailed),
            "{path} must be covered"
        );
    }
}

#[test]
fn query_canonicalization_sorts_and_percent_encodes_names_and_values() {
    assert_eq!(
        canonical_path_with_query(
            "/api/disk/export-key",
            &[
                QueryParam::new("b", "2/2"),
                QueryParam::new("a space", "x+y"),
                QueryParam::new("b", "1")
            ],
        ),
        "/api/disk/export-key?a%20space=x%2By&b=1&b=2%2F2"
    );
}

#[test]
fn aes_gcm_round_trips_with_protocol_aad() {
    let key = [7_u8; 32];
    let nonce = [3_u8; AES_GCM_NONCE_LEN];
    let aad = pack_object_aad(PackObjectAad {
        disk_id: "disk-1",
        seal_id: "seal-1",
        export_job_id: "export-1",
        object_id: "object-1",
        bucket: "source-bucket",
        object_key: "path/to/object.bin",
        pack_path: "packs/export-1/pack-object-1.pack",
        pack_offset_bytes: 0,
        plaintext_sha256: "plaintext-sha",
    });

    let encrypted = encrypt_aes256_gcm(&key, &nonce, b"plaintext object bytes", &aad).unwrap();
    assert_ne!(encrypted.ciphertext, b"plaintext object bytes");

    let decrypted =
        decrypt_aes256_gcm(&key, &nonce, &encrypted.ciphertext, &encrypted.tag, &aad).unwrap();
    assert_eq!(decrypted, b"plaintext object bytes");
}

#[test]
fn aes_gcm_rejects_tampered_tag_and_wrong_aad() {
    let key = [9_u8; 32];
    let nonce = [4_u8; AES_GCM_NONCE_LEN];
    let aad = b"disk/export/bucket/key/0/1";
    let encrypted = encrypt_aes256_gcm(&key, &nonce, b"data", aad).unwrap();

    let mut tampered_tag = encrypted.tag;
    tampered_tag[0] ^= 0x01;
    assert_eq!(
        decrypt_aes256_gcm(&key, &nonce, &encrypted.ciphertext, &tampered_tag, aad),
        Err(CryptoError::DecryptFailed)
    );

    assert_eq!(
        decrypt_aes256_gcm(
            &key,
            &nonce,
            &encrypted.ciphertext,
            &encrypted.tag,
            b"other/aad"
        ),
        Err(CryptoError::DecryptFailed)
    );
}

#[test]
fn aes_gcm_validates_key_nonce_and_tag_lengths() {
    assert_eq!(
        encrypt_aes256_gcm(&[1_u8; 31], &[2_u8; AES_GCM_NONCE_LEN], b"data", b"aad"),
        Err(CryptoError::InvalidAesKeyLength)
    );
    assert_eq!(
        encrypt_aes256_gcm(&[1_u8; 32], &[2_u8; 11], b"data", b"aad"),
        Err(CryptoError::InvalidNonceLength)
    );
    assert_eq!(
        decrypt_aes256_gcm(
            &[1_u8; 32],
            &[2_u8; AES_GCM_NONCE_LEN],
            b"data",
            &[0_u8; 15],
            b"aad"
        ),
        Err(CryptoError::InvalidTagLength)
    );
}

#[test]
fn base64_hex_and_nonce_helpers_are_usable() {
    assert_eq!(encode_hex_lower(&[0xab, 0xcd]), "abcd");
    assert_eq!(decode_hex("abcd").unwrap(), vec![0xab, 0xcd]);

    assert_eq!(encode_base64(b"RustFS"), "UnVzdEZT");
    assert_eq!(decode_base64("UnVzdEZT").unwrap(), b"RustFS");

    let first = generate_nonce();
    let second = generate_nonce();
    assert_eq!(first.len(), AES_GCM_NONCE_LEN);
    assert_ne!(first, second);
}

fn disk_info_for_center_signature() -> serde_json::Value {
    json!({
        "protocol": {
            "name": "rustfs-offline-transfer",
            "version": "1.0.0"
        },
        "disk": {
            "disk_id": "disk-001",
            "sn": "SN001",
            "capacity_bytes": 1024,
            "last_init_time": "2026-08-11T00:00:00Z",
            "initialized_by": "center-a"
        },
        "center": {
            "center_id": "center-a",
            "import_job_id": "import-001",
            "import_started_at": "2026-08-11T01:00:00Z",
            "import_finished_at": "2026-08-11T01:01:00Z"
        },
        "edge": {
            "edge_code": "edge-a",
            "export_job_id": "export-001",
            "seal_id": "seal-001"
        },
        "manifest": {
            "manifest_path": "manifests/export_manifest.json",
            "manifest_sha256_path": "manifests/export_manifest.sha256",
            "object_count": 57,
            "total_bytes": 951301462,
            "manifest_sha256": "manifest-sha"
        },
        "security": {
            "center_key_id": "center-key-001",
            "data_key_id": "data-key-001",
            "encryption_alg": "AES-256-GCM",
            "signature_alg": "HMAC-SHA256",
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
    })
}
