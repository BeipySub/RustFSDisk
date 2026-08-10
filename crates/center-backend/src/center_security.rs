use std::{collections::BTreeMap, env};

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{anyhow, bail, Context, Result};
use rand::{rngs::OsRng, RngCore};
use rustfs_transfer_common::crypto::{
    decode_base64, decode_hex, encode_base64, generate_nonce, sha256_lower_hex, AES_GCM_TAG_LEN,
};
use serde::Serialize;
use serde_json::{json, Number, Value};
use uuid::Uuid;

use crate::config::SecurityConfig;

pub const KEY_WRAP_ALG_LOCAL_MASTER_KEY: &str = "LOCAL-MASTER-KEY";
pub const SIGNATURE_ALG_HMAC_SHA256: &str = "HMAC-SHA256";
pub const ENCRYPTION_ALG_AES_256_GCM: &str = "AES-256-GCM";

const WRAPPED_KEY_PREFIX: &str = "local-master-key:v1";
const DATA_KEY_CONTEXT: &str = "rustfs-transfer:data-key";
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
    let canonical = disk_info_canonical_json(disk_info)?;
    Ok(rustfs_transfer_common::crypto::sign_string_hmac_base64(
        signature_key,
        &canonical,
    ))
}

pub fn verify_disk_info_with_key<T: Serialize>(disk_info: &T, signature_key: &[u8]) -> Result<()> {
    let value = serde_json::to_value(disk_info)?;
    let signature = value
        .get("security")
        .and_then(|security| security.get("center_signature"))
        .and_then(Value::as_str)
        .filter(|signature| !signature.trim().is_empty())
        .ok_or_else(|| anyhow!("disk_info center_signature is missing"))?;
    let expected = sign_disk_info_with_key(&value, signature_key)?;
    if expected != signature {
        bail!("disk_info center_signature verification failed");
    }
    Ok(())
}

pub fn disk_info_canonical_json<T: Serialize>(disk_info: &T) -> Result<String> {
    let value = serde_json::to_value(disk_info)?;
    let security = value
        .get("security")
        .ok_or_else(|| anyhow!("disk_info.security is missing"))?;
    let covered = json!({
        "center": value.get("center").cloned().unwrap_or(Value::Null),
        "disk": value.get("disk").cloned().unwrap_or(Value::Null),
        "protocol": value.get("protocol").cloned().unwrap_or(Value::Null),
        "security": {
            "center_key_id": security.get("center_key_id").cloned().unwrap_or(Value::Null),
            "data_key_id": security.get("data_key_id").cloned().unwrap_or(Value::Null),
            "signature_alg": security.get("signature_alg").cloned().unwrap_or(Value::Null),
        }
    });
    Ok(canonical_json(&covered))
}

pub fn disk_data_key_base64(key: &[u8; 32]) -> String {
    encode_base64(key)
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

fn decode_required_b64(value: Option<&str>, missing_message: &str) -> Result<Vec<u8>> {
    let value = value.ok_or_else(|| anyhow!(missing_message.to_string()))?;
    decode_base64(value).map_err(|error| anyhow!(error.to_string()))
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
