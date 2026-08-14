use std::env;

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{anyhow, bail, Context, Result};
use rand::{rngs::OsRng, RngCore};
use rustfs_transfer_common::crypto::{
    center_signature_canonical_json, decode_base64, decode_hex, derive_disk_data_key_from_edge_key,
    encode_base64, generate_nonce, sha256_lower_hex, sign_center_signature,
    verify_center_signature, AES_GCM_TAG_LEN,
};
use serde::Serialize;
use uuid::Uuid;

use crate::config::SecurityConfig;

pub const KEY_WRAP_ALG_LOCAL_MASTER_KEY: &str = "LOCAL-MASTER-KEY";
pub const SIGNATURE_ALG_HMAC_SHA256: &str = "HMAC-SHA256";
pub const ENCRYPTION_ALG_AES_256_GCM: &str = "AES-256-GCM";

const WRAPPED_KEY_PREFIX: &str = "local-master-key:v1";
const DATA_KEY_CONTEXT: &str = "rustfs-transfer:data-key";
const EDGE_KEY_CONTEXT: &str = "rustfs-transfer:edge-key";
const DISK_INFO_CONTEXT: &str = "rustfs-transfer:disk-info";

#[derive(Clone)]
pub struct CenterSecurity {
    local_master_key: [u8; 32],
    center_signature_key: Vec<u8>,
    center_key_id: Uuid,
}

impl CenterSecurity {
    pub fn from_config(config: &SecurityConfig) -> Result<Self> {
        let local_master_key_env = config
            .local_master_key_env
            .as_deref()
            .unwrap_or("RUSTFS_TRANSFER__SECURITY__LOCAL_MASTER_KEY");
        let center_signature_key_env = config
            .center_signature_key_env
            .as_deref()
            .unwrap_or("RUSTFS_TRANSFER__SECURITY__CENTER_SIGNATURE_KEY");

        let local_master_key = read_key_from_env(local_master_key_env)
            .with_context(|| format!("load {local_master_key_env}"))?;
        let center_signature_key = read_key_from_env(center_signature_key_env)
            .with_context(|| format!("load {center_signature_key_env}"))?;

        Ok(Self::new(local_master_key, center_signature_key))
    }

    pub fn new(local_master_key: [u8; 32], center_signature_key: [u8; 32]) -> Self {
        let center_key_id = deterministic_key_id(&center_signature_key);
        Self {
            local_master_key,
            center_signature_key: center_signature_key.to_vec(),
            center_key_id,
        }
    }

    pub fn test() -> Self {
        Self::new([0x42; 32], [0x24; 32])
    }

    pub fn center_key_id(&self) -> Uuid {
        self.center_key_id
    }

    pub fn generate_disk_data_key(&self) -> [u8; 32] {
        let mut key = [0_u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }

    pub fn wrap_disk_data_key(
        &self,
        disk_id: Uuid,
        data_key_id: Uuid,
        disk_data_key: &[u8; 32],
    ) -> Result<String> {
        let cipher = Aes256Gcm::new_from_slice(&self.local_master_key)
            .map_err(|_| anyhow!("invalid local master key length"))?;
        let nonce = generate_nonce();
        let aad = data_key_aad(disk_id, data_key_id);
        let mut encrypted = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: disk_data_key.as_slice(),
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| anyhow!("failed to wrap disk data key"))?;
        let tag = encrypted.split_off(encrypted.len() - AES_GCM_TAG_LEN);
        Ok(format!(
            "{WRAPPED_KEY_PREFIX}:{}:{}:{}",
            encode_base64(&nonce),
            encode_base64(&encrypted),
            encode_base64(&tag)
        ))
    }

    pub fn unwrap_disk_data_key(
        &self,
        disk_id: Uuid,
        data_key_id: Uuid,
        encrypted_key: &str,
    ) -> Result<[u8; 32]> {
        let mut parts = encrypted_key.split(':');
        let prefix = [parts.next(), parts.next()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(":");
        if prefix != WRAPPED_KEY_PREFIX {
            bail!("unsupported key wrap format; old mock/plaintext-equivalent data must be reinitialized");
        }
        let nonce = decode_required_b64(parts.next(), "wrapped key is missing nonce")?;
        let ciphertext = decode_required_b64(parts.next(), "wrapped key is missing ciphertext")?;
        let tag = decode_required_b64(parts.next(), "wrapped key is missing tag")?;
        if parts.next().is_some() {
            bail!("wrapped key has unexpected fields");
        }
        if nonce.len() != 12 || tag.len() != AES_GCM_TAG_LEN {
            bail!("wrapped key nonce or tag length is invalid");
        }

        let cipher = Aes256Gcm::new_from_slice(&self.local_master_key)
            .map_err(|_| anyhow!("invalid local master key length"))?;
        let mut payload = ciphertext;
        payload.extend_from_slice(&tag);
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: payload.as_ref(),
                    aad: data_key_aad(disk_id, data_key_id).as_bytes(),
                },
            )
            .map_err(|_| anyhow!("failed to unwrap disk data key with configured master key"))?;
        plaintext
            .try_into()
            .map_err(|_| anyhow!("unwrapped disk data key is not 32 bytes"))
    }

    pub fn wrap_edge_key(&self, edge_code: &str, edge_key: &str) -> Result<String> {
        let key = edge_key.trim();
        if key.is_empty() {
            bail!("edge_key must not be empty");
        }
        let cipher = Aes256Gcm::new_from_slice(&self.local_master_key)
            .map_err(|_| anyhow!("invalid local master key length"))?;
        let nonce = generate_nonce();
        let mut encrypted = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: key.as_bytes(),
                    aad: edge_key_aad(edge_code).as_bytes(),
                },
            )
            .map_err(|_| anyhow!("failed to wrap edge key"))?;
        let tag = encrypted.split_off(encrypted.len() - AES_GCM_TAG_LEN);
        Ok(format!(
            "{WRAPPED_KEY_PREFIX}:{}:{}:{}",
            encode_base64(&nonce),
            encode_base64(&encrypted),
            encode_base64(&tag)
        ))
    }

    pub fn unwrap_edge_key(&self, edge_code: &str, edge_key_ciphertext: &str) -> Result<String> {
        let value = edge_key_ciphertext.trim();
        if !value.starts_with(WRAPPED_KEY_PREFIX) {
            return Ok(value.to_string());
        }
        let mut parts = value.split(':');
        let prefix = [parts.next(), parts.next()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(":");
        if prefix != WRAPPED_KEY_PREFIX {
            bail!("unsupported edge key wrap format");
        }
        let nonce = decode_required_b64(parts.next(), "edge key is missing nonce")?;
        let ciphertext = decode_required_b64(parts.next(), "edge key is missing ciphertext")?;
        let tag = decode_required_b64(parts.next(), "edge key is missing tag")?;
        if parts.next().is_some() {
            bail!("edge key has unexpected fields");
        }
        if nonce.len() != 12 || tag.len() != AES_GCM_TAG_LEN {
            bail!("edge key nonce or tag length is invalid");
        }

        let cipher = Aes256Gcm::new_from_slice(&self.local_master_key)
            .map_err(|_| anyhow!("invalid local master key length"))?;
        let mut payload = ciphertext;
        payload.extend_from_slice(&tag);
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: payload.as_ref(),
                    aad: edge_key_aad(edge_code).as_bytes(),
                },
            )
            .map_err(|_| anyhow!("failed to unwrap edge key with configured master key"))?;
        String::from_utf8(plaintext).map_err(|_| anyhow!("edge key is not valid UTF-8"))
    }

    pub fn sign_disk_info<T: Serialize>(&self, disk_info: &T) -> Result<String> {
        sign_disk_info_with_key(disk_info, &self.center_signature_key)
    }

    pub fn verify_disk_info<T: Serialize>(&self, disk_info: &T) -> Result<()> {
        verify_disk_info_with_key(disk_info, &self.center_signature_key)
    }

    pub(crate) fn center_signature_key_bytes(&self) -> Vec<u8> {
        self.center_signature_key.clone()
    }
}

pub fn sign_disk_info_with_key<T: Serialize>(
    disk_info: &T,
    signature_key: &[u8],
) -> Result<String> {
    sign_center_signature(signature_key, disk_info).map_err(|err| anyhow!(err.to_string()))
}

pub fn verify_disk_info_with_key<T: Serialize>(disk_info: &T, signature_key: &[u8]) -> Result<()> {
    verify_center_signature(signature_key, disk_info).map_err(|err| anyhow!(err.to_string()))
}

pub fn disk_info_canonical_json<T: Serialize>(disk_info: &T) -> Result<String> {
    center_signature_canonical_json(disk_info).map_err(|err| anyhow!(err.to_string()))
}

pub fn disk_data_key_base64(key: &[u8; 32]) -> String {
    encode_base64(key)
}

pub fn derive_offline_disk_data_key(
    edge_key: &str,
    edge_code: &str,
    disk_id: Uuid,
    data_key_id: Uuid,
    export_job_id: Uuid,
    seal_id: Uuid,
) -> Result<[u8; 32]> {
    derive_disk_data_key_from_edge_key(
        edge_key,
        edge_code,
        disk_id,
        data_key_id,
        export_job_id,
        seal_id,
    )
    .map_err(|error| anyhow!(error.to_string()))
}

fn read_key_from_env(env_name: &str) -> Result<[u8; 32]> {
    let raw = env::var(env_name).map_err(|_| anyhow!("{env_name} is not set"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with("CHANGE_ME") {
        bail!("{env_name} must contain a real 32-byte base64 or hex key");
    }
    let bytes = decode_base64(trimmed)
        .or_else(|_| decode_hex(trimmed))
        .map_err(|error| anyhow!(error.to_string()))?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("{env_name} must decode to exactly 32 bytes"))
}

fn deterministic_key_id(key: &[u8; 32]) -> Uuid {
    let digest = sha256_lower_hex(key);
    let bytes = hex::decode(&digest[..32]).expect("sha256 hex prefix is valid");
    Uuid::from_slice(&bytes).expect("16-byte key id")
}

fn data_key_aad(disk_id: Uuid, data_key_id: Uuid) -> String {
    format!("{DATA_KEY_CONTEXT}:{disk_id}:{data_key_id}")
}

fn edge_key_aad(edge_code: &str) -> String {
    format!("{EDGE_KEY_CONTEXT}:{}", edge_code.trim())
}

fn decode_required_b64(value: Option<&str>, missing_message: &str) -> Result<Vec<u8>> {
    let value = value.ok_or_else(|| anyhow!(missing_message.to_string()))?;
    decode_base64(value).map_err(|error| anyhow!(error.to_string()))
}

pub fn signature_audit_context() -> &'static str {
    DISK_INFO_CONTEXT
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wraps_disk_data_key_without_plaintext_equivalence() {
        let security = CenterSecurity::test();
        let disk_id = Uuid::new_v4();
        let data_key_id = Uuid::new_v4();
        let disk_data_key = [9_u8; 32];

        let wrapped = security
            .wrap_disk_data_key(disk_id, data_key_id, &disk_data_key)
            .unwrap();

        assert!(wrapped.starts_with(WRAPPED_KEY_PREFIX));
        assert!(!wrapped.contains(&encode_base64(&disk_data_key)));
        assert_eq!(
            security
                .unwrap_disk_data_key(disk_id, data_key_id, &wrapped)
                .unwrap(),
            disk_data_key
        );
    }

    #[test]
    fn wrong_master_key_rejects_wrapped_data_key() {
        let security = CenterSecurity::test();
        let other = CenterSecurity::new([1_u8; 32], [0x24; 32]);
        let disk_id = Uuid::new_v4();
        let data_key_id = Uuid::new_v4();
        let wrapped = security
            .wrap_disk_data_key(disk_id, data_key_id, &[7_u8; 32])
            .unwrap();

        assert!(other
            .unwrap_disk_data_key(disk_id, data_key_id, &wrapped)
            .is_err());
    }

    #[test]
    fn wraps_edge_key_without_plaintext_equivalence() {
        let security = CenterSecurity::test();
        let wrapped = security.wrap_edge_key("edge-a", "edge-key-a").unwrap();

        assert!(wrapped.starts_with(WRAPPED_KEY_PREFIX));
        assert!(!wrapped.contains("edge-key-a"));
        assert_eq!(
            security.unwrap_edge_key("edge-a", &wrapped).unwrap(),
            "edge-key-a"
        );
        assert!(security.unwrap_edge_key("edge-b", &wrapped).is_err());
    }

    #[test]
    fn signs_and_rejects_tampered_disk_info() {
        let security = CenterSecurity::test();
        let mut disk_info = json!({
            "protocol": {"name": "rustfs-offline-transfer", "version": "1.0.0"},
            "disk": {"disk_id": Uuid::new_v4(), "sn": "SN001", "capacity_bytes": 1024},
            "center": {"center_id": Uuid::new_v4()},
            "security": {
                "center_key_id": security.center_key_id(),
                "signature_alg": SIGNATURE_ALG_HMAC_SHA256,
                "encryption_alg": ENCRYPTION_ALG_AES_256_GCM,
                "data_key_id": Uuid::new_v4(),
                "center_signature": ""
            },
            "status": {"code": "INITIALIZED"}
        });
        let signature = security.sign_disk_info(&disk_info).unwrap();
        disk_info["security"]["center_signature"] = json!(signature);
        security.verify_disk_info(&disk_info).unwrap();

        disk_info["security"]["data_key_id"] = json!(Uuid::new_v4());
        assert!(security.verify_disk_info(&disk_info).is_err());
    }

    #[test]
    fn imported_disk_info_keeps_signature_valid_after_center_import_mark_without_compat_path() {
        let security = CenterSecurity::test();
        let mut disk_info = json!({
            "protocol": {"name": "rustfs-offline-transfer", "version": "1.0.0"},
            "disk": {"disk_id": Uuid::new_v4(), "sn": "SN001", "capacity_bytes": 1024},
            "center": {
                "center_id": Uuid::new_v4(),
                "import_job_id": "",
                "import_started_at": "",
                "import_finished_at": ""
            },
            "security": {
                "center_key_id": security.center_key_id(),
                "signature_alg": SIGNATURE_ALG_HMAC_SHA256,
                "encryption_alg": ENCRYPTION_ALG_AES_256_GCM,
                "data_key_id": Uuid::new_v4(),
                "center_signature": ""
            },
            "status": {"code": "SEALED"}
        });
        let signature = security.sign_disk_info(&disk_info).unwrap();
        disk_info["security"]["center_signature"] = json!(signature);

        disk_info["status"]["code"] = json!("IMPORTED");
        disk_info["center"]["import_job_id"] = json!(Uuid::new_v4().to_string());
        disk_info["center"]["import_started_at"] = json!("2026-08-10T00:00:00Z");
        disk_info["center"]["import_finished_at"] = json!("2026-08-10T00:00:00Z");
        security.verify_disk_info(&disk_info).unwrap();

        disk_info["center"]["center_id"] = json!(Uuid::new_v4());
        security.verify_disk_info(&disk_info).unwrap();

        disk_info["security"]["data_key_id"] = json!(Uuid::new_v4());
        assert!(security.verify_disk_info(&disk_info).is_err());
    }

    #[test]
    fn missing_env_key_fails_startup_security_load() {
        let local_env = format!("RUSTFS_TRANSFER__TEST_LOCAL_{}", Uuid::new_v4());
        let signature_env = format!("RUSTFS_TRANSFER__TEST_SIGNATURE_{}", Uuid::new_v4());
        let config = SecurityConfig {
            local_master_key_env: Some(local_env),
            center_signature_key_env: Some(signature_env),
        };

        let error = match CenterSecurity::from_config(&config) {
            Ok(_) => panic!("missing security env should fail"),
            Err(error) => error,
        };

        assert!(format!("{error:#}").contains("is not set"));
    }

    #[test]
    fn env_placeholder_key_fails_startup_security_load() {
        let local_env = format!("RUSTFS_TRANSFER__TEST_LOCAL_{}", Uuid::new_v4());
        let signature_env = format!("RUSTFS_TRANSFER__TEST_SIGNATURE_{}", Uuid::new_v4());
        env::set_var(&local_env, "CHANGE_ME_32_BYTE_BASE64_OR_HEX_KEY");
        env::set_var(&signature_env, encode_base64(&[8_u8; 32]));
        let config = SecurityConfig {
            local_master_key_env: Some(local_env.clone()),
            center_signature_key_env: Some(signature_env.clone()),
        };

        let error = match CenterSecurity::from_config(&config) {
            Ok(_) => panic!("placeholder security env should fail"),
            Err(error) => error,
        };

        env::remove_var(local_env);
        env::remove_var(signature_env);
        assert!(format!("{error:#}").contains("real 32-byte"));
    }
}
