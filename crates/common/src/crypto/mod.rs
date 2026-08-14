//! 加密、签名和哈希工具入口。

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Serialize;
use serde_json::{json, Number, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

pub const AES_256_GCM_KEY_LEN: usize = 32;
pub const AES_GCM_NONCE_LEN: usize = 12;
pub const AES_GCM_TAG_LEN: usize = 16;

const QUERY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    #[error("AES-256-GCM key must be 32 bytes")]
    InvalidAesKeyLength,
    #[error("AES-GCM nonce must be 12 bytes")]
    InvalidNonceLength,
    #[error("AES-GCM tag must be 16 bytes")]
    InvalidTagLength,
    #[error("AES-GCM encryption failed")]
    EncryptFailed,
    #[error("AES-GCM decryption failed")]
    DecryptFailed,
    #[error("hex decode failed: {0}")]
    HexDecodeFailed(String),
    #[error("base64 decode failed: {0}")]
    Base64DecodeFailed(String),
    #[error("HMAC verification failed")]
    HmacVerificationFailed,
    #[error("edge_key must not be empty")]
    EmptyEdgeKey,
    #[error("disk_info.security is missing")]
    DiskInfoSecurityMissing,
    #[error("disk_info center_signature is missing")]
    CenterSignatureMissing,
    #[error("disk_info center_signature verification failed")]
    CenterSignatureVerificationFailed,
    #[error("disk_info JSON serialization failed: {0}")]
    DiskInfoJsonFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryParam {
    pub name: String,
    pub value: String,
}

impl QueryParam {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalRequest {
    pub method: String,
    pub canonical_path_with_query: String,
    pub timestamp: String,
    pub nonce: String,
    pub body_sha256: String,
}

impl CanonicalRequest {
    pub fn new(
        method: impl AsRef<str>,
        path: impl AsRef<str>,
        query: &[QueryParam],
        timestamp: impl Into<String>,
        nonce: impl Into<String>,
        body: &[u8],
    ) -> Self {
        Self {
            method: method.as_ref().to_ascii_uppercase(),
            canonical_path_with_query: canonical_path_with_query(path.as_ref(), query),
            timestamp: timestamp.into().trim().to_owned(),
            nonce: nonce.into().trim().to_owned(),
            body_sha256: sha256_lower_hex(body),
        }
    }

    pub fn string_to_sign(&self) -> String {
        [
            self.method.as_str(),
            self.canonical_path_with_query.as_str(),
            self.timestamp.as_str(),
            self.nonce.as_str(),
            self.body_sha256.as_str(),
        ]
        .join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AesGcmCiphertext {
    pub ciphertext: Vec<u8>,
    pub tag: [u8; AES_GCM_TAG_LEN],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectAad<'a> {
    pub disk_id: &'a str,
    pub seal_id: &'a str,
    pub export_job_id: &'a str,
    pub bucket: &'a str,
    pub object_key: &'a str,
    pub chunk_group_id: Option<&'a str>,
    pub chunk_index: u32,
    pub chunk_total: u32,
    pub chunk_offset_bytes: u64,
}

pub fn sha256_lower_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn encode_hex_lower(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

pub fn decode_hex(value: &str) -> Result<Vec<u8>, CryptoError> {
    hex::decode(value).map_err(|err| CryptoError::HexDecodeFailed(err.to_string()))
}

pub fn encode_base64(bytes: &[u8]) -> String {
    BASE64_STANDARD.encode(bytes)
}

pub fn decode_base64(value: &str) -> Result<Vec<u8>, CryptoError> {
    BASE64_STANDARD
        .decode(value)
        .map_err(|err| CryptoError::Base64DecodeFailed(err.to_string()))
}

pub fn generate_nonce() -> [u8; AES_GCM_NONCE_LEN] {
    let mut nonce = [0_u8; AES_GCM_NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

pub fn generate_aes256_key() -> [u8; AES_256_GCM_KEY_LEN] {
    let mut key = [0_u8; AES_256_GCM_KEY_LEN];
    OsRng.fill_bytes(&mut key);
    key
}

pub fn canonical_path_with_query(path: &str, query: &[QueryParam]) -> String {
    let canonical_path = if path.is_empty() { "/" } else { path };
    let canonical_query = canonical_query(query);
    if canonical_query.is_empty() {
        canonical_path.to_owned()
    } else {
        format!("{canonical_path}?{canonical_query}")
    }
}

pub fn canonical_query(query: &[QueryParam]) -> String {
    let mut encoded = query
        .iter()
        .map(|param| {
            (
                percent_encode_query_component(&param.name),
                percent_encode_query_component(&param.value),
            )
        })
        .collect::<Vec<_>>();
    encoded.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    encoded
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn percent_encode_query_component(value: &str) -> String {
    utf8_percent_encode(value, QUERY_ENCODE_SET).to_string()
}

pub fn sign_hmac_base64(edge_auth_secret: &[u8], canonical: &CanonicalRequest) -> String {
    sign_string_hmac_base64(edge_auth_secret, &canonical.string_to_sign())
}

pub fn sign_string_hmac_base64(edge_auth_secret: &[u8], string_to_sign: &str) -> String {
    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(edge_auth_secret).expect("HMAC accepts any key length");
    mac.update(string_to_sign.as_bytes());
    encode_base64(&mac.finalize().into_bytes())
}

pub fn verify_hmac_base64(
    edge_auth_secret: &[u8],
    canonical: &CanonicalRequest,
    signature_base64: &str,
) -> Result<(), CryptoError> {
    let expected = decode_base64(signature_base64)?;
    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(edge_auth_secret).expect("HMAC accepts any key length");
    mac.update(canonical.string_to_sign().as_bytes());
    mac.verify_slice(&expected)
        .map_err(|_| CryptoError::HmacVerificationFailed)
}

pub fn derive_disk_data_key_from_edge_key(
    edge_key: &str,
    edge_code: &str,
    disk_id: impl std::fmt::Display,
    data_key_id: impl std::fmt::Display,
    export_job_id: impl std::fmt::Display,
    seal_id: impl std::fmt::Display,
) -> Result<[u8; AES_256_GCM_KEY_LEN], CryptoError> {
    let edge_key = edge_key.trim();
    if edge_key.is_empty() {
        return Err(CryptoError::EmptyEdgeKey);
    }
    let message = format!(
        "rustfs-transfer:offline-disk-data-key:v1\nedge_code={}\ndisk_id={}\ndata_key_id={}\nexport_job_id={}\nseal_id={}",
        edge_code.trim(),
        disk_id,
        data_key_id,
        export_job_id,
        seal_id
    );
    let mut mac = <HmacSha256 as Mac>::new_from_slice(edge_key.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(message.as_bytes());
    let bytes = mac.finalize().into_bytes();
    let mut key = [0_u8; AES_256_GCM_KEY_LEN];
    key.copy_from_slice(&bytes);
    Ok(key)
}

pub fn center_signature_payload<T: Serialize>(disk_info: &T) -> Result<Value, CryptoError> {
    let value = serde_json::to_value(disk_info)
        .map_err(|err| CryptoError::DiskInfoJsonFailed(err.to_string()))?;
    center_signature_payload_value(&value)
}

pub fn center_signature_payload_value(disk_info: &Value) -> Result<Value, CryptoError> {
    let security = disk_info
        .get("security")
        .ok_or(CryptoError::DiskInfoSecurityMissing)?;
    Ok(json!({
        "disk": disk_info.get("disk").cloned().unwrap_or(Value::Null),
        "protocol": disk_info.get("protocol").cloned().unwrap_or(Value::Null),
        "security": {
            "center_key_id": security.get("center_key_id").cloned().unwrap_or(Value::Null),
            "data_key_id": security.get("data_key_id").cloned().unwrap_or(Value::Null),
            "signature_alg": security.get("signature_alg").cloned().unwrap_or(Value::Null),
        }
    }))
}

pub fn center_signature_canonical_json<T: Serialize>(disk_info: &T) -> Result<String, CryptoError> {
    let payload = center_signature_payload(disk_info)?;
    Ok(canonical_json(&payload))
}

pub fn sign_center_signature<T: Serialize>(
    signature_key: &[u8],
    disk_info: &T,
) -> Result<String, CryptoError> {
    let canonical = center_signature_canonical_json(disk_info)?;
    Ok(sign_string_hmac_base64(signature_key, &canonical))
}

pub fn verify_center_signature<T: Serialize>(
    signature_key: &[u8],
    disk_info: &T,
) -> Result<(), CryptoError> {
    let value = serde_json::to_value(disk_info)
        .map_err(|err| CryptoError::DiskInfoJsonFailed(err.to_string()))?;
    let signature = value
        .get("security")
        .and_then(|security| security.get("center_signature"))
        .and_then(Value::as_str)
        .filter(|signature| !signature.trim().is_empty())
        .ok_or(CryptoError::CenterSignatureMissing)?;
    let expected = sign_center_signature(signature_key, &value)?;
    let expected_bytes = decode_base64(&expected)?;
    let signature_bytes = decode_base64(signature)?;
    if expected_bytes.len() != signature_bytes.len() {
        return Err(CryptoError::CenterSignatureVerificationFailed);
    }
    let mut diff = 0_u8;
    for (left, right) in expected_bytes.iter().zip(signature_bytes.iter()) {
        diff |= left ^ right;
    }
    if diff == 0 {
        Ok(())
    } else {
        Err(CryptoError::CenterSignatureVerificationFailed)
    }
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(number) => canonical_number(number),
        Value::String(value) => serde_json::to_string(value).expect("string serialization"),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(map) => {
            let sorted = map.iter().collect::<BTreeMap<_, _>>();
            format!(
                "{{{}}}",
                sorted
                    .into_iter()
                    .map(|(key, value)| format!(
                        "{}:{}",
                        serde_json::to_string(key).expect("key serialization"),
                        canonical_json(value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

fn canonical_number(number: &Number) -> String {
    number.to_string()
}

pub fn object_aad(aad: ObjectAad<'_>) -> Vec<u8> {
    format!(
        "disk_id={};seal_id={};export_job_id={};bucket={};key={};chunk_group_id={};chunk_index={};chunk_total={};chunk_offset_bytes={}",
        aad.disk_id,
        aad.seal_id,
        aad.export_job_id,
        aad.bucket,
        aad.object_key,
        aad.chunk_group_id.unwrap_or_default(),
        aad.chunk_index,
        aad.chunk_total,
        aad.chunk_offset_bytes
    )
    .into_bytes()
}

pub fn encrypt_aes256_gcm(
    disk_data_key: &[u8],
    nonce: &[u8],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<AesGcmCiphertext, CryptoError> {
    let cipher = aes256_gcm_cipher(disk_data_key)?;
    validate_aes_gcm_nonce(nonce)?;
    let nonce = Nonce::from_slice(nonce);
    let mut encrypted = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CryptoError::EncryptFailed)?;
    let tag = encrypted
        .split_off(encrypted.len().saturating_sub(AES_GCM_TAG_LEN))
        .try_into()
        .map_err(|_| CryptoError::EncryptFailed)?;
    Ok(AesGcmCiphertext {
        ciphertext: encrypted,
        tag,
    })
}

pub fn decrypt_aes256_gcm(
    disk_data_key: &[u8],
    nonce: &[u8],
    ciphertext: &[u8],
    tag: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if tag.len() != AES_GCM_TAG_LEN {
        return Err(CryptoError::InvalidTagLength);
    }
    let cipher = aes256_gcm_cipher(disk_data_key)?;
    validate_aes_gcm_nonce(nonce)?;
    let nonce = Nonce::from_slice(nonce);
    let mut combined = Vec::with_capacity(ciphertext.len() + tag.len());
    combined.extend_from_slice(ciphertext);
    combined.extend_from_slice(tag);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: &combined,
                aad,
            },
        )
        .map_err(|_| CryptoError::DecryptFailed)
}

fn aes256_gcm_cipher(disk_data_key: &[u8]) -> Result<Aes256Gcm, CryptoError> {
    if disk_data_key.len() != AES_256_GCM_KEY_LEN {
        return Err(CryptoError::InvalidAesKeyLength);
    }
    Aes256Gcm::new_from_slice(disk_data_key).map_err(|_| CryptoError::InvalidAesKeyLength)
}

fn validate_aes_gcm_nonce(nonce: &[u8]) -> Result<(), CryptoError> {
    if nonce.len() != AES_GCM_NONCE_LEN {
        return Err(CryptoError::InvalidNonceLength);
    }
    Ok(())
}
