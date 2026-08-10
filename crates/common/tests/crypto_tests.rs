use rustfs_transfer_common::crypto::{
    canonical_path_with_query, decode_base64, decode_hex, decrypt_aes256_gcm, encode_base64,
    encode_hex_lower, encrypt_aes256_gcm, generate_nonce, object_aad, sha256_lower_hex,
    sign_hmac_base64, verify_hmac_base64, CanonicalRequest, CryptoError, ObjectAad, QueryParam,
    AES_GCM_NONCE_LEN,
};

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
    let aad = object_aad(ObjectAad {
        disk_id: "disk-1",
        seal_id: "seal-1",
        export_job_id: "export-1",
        bucket: "source-bucket",
        object_key: "path/to/object.bin",
        chunk_group_id: None,
        chunk_index: 0,
        chunk_total: 1,
        chunk_offset_bytes: 0,
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
