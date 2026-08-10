//! 加密、签名和哈希工具入口。

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
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
    pub export_job_id: &'a str,
    pub bucket: &'a str,
    pub object_key: &'a str,
    pub chunk_index: u32,
    pub chunk_total: u32,
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

pub fn object_aad(aad: ObjectAad<'_>) -> Vec<u8> {
    format!(
        "{}/{}/{}/{}/{}/{}",
        aad.disk_id,
        aad.export_job_id,
        aad.bucket,
        aad.object_key,
        aad.chunk_index,
        aad.chunk_total
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
