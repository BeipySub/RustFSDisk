use serde::Deserialize;
use std::{env, fs, net::SocketAddr, path::Path};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct CenterConfig {
    pub center: CenterIdentityConfig,
    #[serde(default)]
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub rustfs: RustfsConfig,
    #[serde(default)]
    pub paths: PathConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CenterIdentityConfig {
    pub center_id: Uuid,
    #[serde(default = "default_center_name")]
    pub center_name: String,
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,
    #[serde(default)]
    pub control_api_token: Option<String>,
    #[serde(default)]
    pub control_api_token_env: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_db_max_connections")]
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RustfsConfig {
    pub endpoint: String,
    #[serde(default = "default_region")]
    pub region: String,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathConfig {
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    #[serde(default = "default_log_dir")]
    pub log_dir: String,
    #[serde(default)]
    pub transport_mount_root: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SecurityConfig {
    pub local_master_key_env: Option<String>,
    pub center_signature_key_env: Option<String>,
}

impl CenterConfig {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let raw = fs::read_to_string(path)?;
        Self::from_toml(&raw)
    }

    pub fn from_toml(raw: &str) -> anyhow::Result<Self> {
        let mut config: Self = toml::from_str(raw)?;
        config.apply_env_overrides();
        config.validate()?;
        Ok(config)
    }

    fn apply_env_overrides(&mut self) {
        override_uuid(
            "RUSTFS_TRANSFER__CENTER__CENTER_ID",
            &mut self.center.center_id,
        );
        override_string(
            "RUSTFS_TRANSFER__CENTER__CENTER_NAME",
            &mut self.center.center_name,
        );
        override_string(
            "RUSTFS_TRANSFER__CENTER__PROTOCOL_VERSION",
            &mut self.center.protocol_version,
        );
        override_socket_addr("RUSTFS_TRANSFER__SERVER__BIND", &mut self.server.bind);
        if self
            .server
            .control_api_token
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            if let Some(token) = read_optional_env(self.server.control_api_token_env.as_deref()) {
                self.server.control_api_token = Some(token);
            }
        }
        override_optional_string(
            "RUSTFS_TRANSFER__SERVER__CONTROL_API_TOKEN",
            &mut self.server.control_api_token,
        );
        override_string("RUSTFS_TRANSFER__DATABASE__URL", &mut self.database.url);
        override_string(
            "RUSTFS_TRANSFER__RUSTFS__ENDPOINT",
            &mut self.rustfs.endpoint,
        );
        override_string("RUSTFS_TRANSFER__RUSTFS__REGION", &mut self.rustfs.region);
        override_optional_string(
            "RUSTFS_TRANSFER__RUSTFS__ACCESS_KEY_ID",
            &mut self.rustfs.access_key_id,
        );
        override_optional_string(
            "RUSTFS_TRANSFER__RUSTFS__SECRET_ACCESS_KEY",
            &mut self.rustfs.secret_access_key,
        );
        override_string("RUSTFS_TRANSFER__PATHS__DATA_DIR", &mut self.paths.data_dir);
        override_string("RUSTFS_TRANSFER__PATHS__LOG_DIR", &mut self.paths.log_dir);
        override_optional_string(
            "RUSTFS_TRANSFER__PATHS__TRANSPORT_MOUNT_ROOT",
            &mut self.paths.transport_mount_root,
        );
    }

    fn validate(&self) -> anyhow::Result<()> {
        ensure_non_empty("center.center_name", &self.center.center_name)?;
        ensure_non_empty("center.protocol_version", &self.center.protocol_version)?;
        ensure_non_empty("database.url", &self.database.url)?;
        ensure_non_empty("rustfs.endpoint", &self.rustfs.endpoint)?;
        Ok(())
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            control_api_token: None,
            control_api_token_env: None,
        }
    }
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            log_dir: default_log_dir(),
            transport_mount_root: None,
        }
    }
}

fn override_string(name: &str, value: &mut String) {
    if let Ok(env_value) = env::var(name) {
        if !env_value.trim().is_empty() {
            *value = env_value;
        }
    }
}

fn override_socket_addr(name: &str, value: &mut SocketAddr) {
    if let Ok(env_value) = env::var(name) {
        if !env_value.trim().is_empty() {
            if let Ok(parsed) = env_value.parse() {
                *value = parsed;
            }
        }
    }
}

fn override_uuid(name: &str, value: &mut Uuid) {
    if let Ok(env_value) = env::var(name) {
        if !env_value.trim().is_empty() {
            if let Ok(parsed) = env_value.parse() {
                *value = parsed;
            }
        }
    }
}

fn override_optional_string(name: &str, value: &mut Option<String>) {
    if let Ok(env_value) = env::var(name) {
        if !env_value.trim().is_empty() {
            *value = Some(env_value);
        }
    }
}

fn read_optional_env(name: Option<&str>) -> Option<String> {
    let name = name?.trim();
    if name.is_empty() {
        return None;
    }
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn ensure_non_empty(field: &str, value: &str) -> anyhow::Result<()> {
    if value.trim().is_empty() {
        anyhow::bail!("{field} must not be empty");
    }
    Ok(())
}

fn default_bind() -> SocketAddr {
    "0.0.0.0:8080".parse().expect("valid default bind address")
}

fn default_db_max_connections() -> u32 {
    5
}

fn default_region() -> String {
    "us-east-1".to_owned()
}

fn default_center_name() -> String {
    "RustFS Transfer Center".to_owned()
}

fn default_protocol_version() -> String {
    crate::PROTOCOL_VERSION.to_owned()
}

fn default_data_dir() -> String {
    "/var/lib/rustfs-transfer/center".to_owned()
}

fn default_log_dir() -> String {
    "/var/log/rustfs-transfer/center".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_minimal_config_with_defaults() {
        let raw = r#"
            [center]
            center_id = "00000000-0000-0000-0000-000000000001"

            [database]
            url = "postgres://center:center@localhost/center"

            [rustfs]
            endpoint = "http://127.0.0.1:9000"
        "#;

        let config = CenterConfig::from_toml(raw).expect("config loads");

        assert_eq!(config.server.bind.port(), 8080);
        assert_eq!(
            config.center.center_id,
            "00000000-0000-0000-0000-000000000001"
                .parse::<Uuid>()
                .unwrap()
        );
        assert_eq!(config.center.protocol_version, crate::PROTOCOL_VERSION);
        assert_eq!(config.rustfs.region, "us-east-1");
        assert_eq!(config.paths.data_dir, "/var/lib/rustfs-transfer/center");
    }
}
