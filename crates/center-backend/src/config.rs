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
    pub disk_polling: DiskPollingConfig,
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

#[derive(Debug, Clone, Deserialize)]
pub struct DiskPollingConfig {
    #[serde(default = "default_disk_polling_enabled")]
    pub enabled: bool,
    #[serde(default = "default_disk_polling_interval_seconds")]
    pub interval_seconds: u64,
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

    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::default();
        config.apply_env_overrides();
        config.validate()?;
        Ok(config)
    }

    fn apply_env_overrides(&mut self) {
        override_uuid("CENTER_ID", &mut self.center.center_id);
        override_string("CENTER_NAME", &mut self.center.center_name);
        override_string("PROTOCOL_VERSION", &mut self.center.protocol_version);
        override_socket_addr("CENTER_BIND", &mut self.server.bind);
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
        override_optional_string("CONTROL_API_TOKEN", &mut self.server.control_api_token);
        override_string("DATABASE_URL", &mut self.database.url);
        override_string("RUSTFS_ENDPOINT", &mut self.rustfs.endpoint);
        override_string("RUSTFS_REGION", &mut self.rustfs.region);
        override_optional_string("RUSTFS_ACCESS_KEY", &mut self.rustfs.access_key_id);
        override_optional_string("RUSTFS_SECRET_KEY", &mut self.rustfs.secret_access_key);
        override_string("DATA_DIR", &mut self.paths.data_dir);
        override_string("LOG_DIR", &mut self.paths.log_dir);
        override_optional_string("TRANSPORT_MOUNT_ROOT", &mut self.paths.transport_mount_root);
        override_bool("DISK_POLLING_ENABLED", &mut self.disk_polling.enabled);
        override_u64(
            "DISK_POLLING_INTERVAL_SECONDS",
            &mut self.disk_polling.interval_seconds,
        );
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.center.center_id.is_nil() {
            anyhow::bail!("center.center_id must not be empty");
        }
        ensure_non_empty("center.center_name", &self.center.center_name)?;
        ensure_non_empty("center.protocol_version", &self.center.protocol_version)?;
        ensure_non_empty("database.url", &self.database.url)?;
        ensure_non_empty("rustfs.endpoint", &self.rustfs.endpoint)?;
        Ok(())
    }
}

impl Default for CenterConfig {
    fn default() -> Self {
        Self {
            center: CenterIdentityConfig::default(),
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            rustfs: RustfsConfig::default(),
            paths: PathConfig::default(),
            disk_polling: DiskPollingConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

impl Default for CenterIdentityConfig {
    fn default() -> Self {
        Self {
            center_id: Uuid::nil(),
            center_name: default_center_name(),
            protocol_version: default_protocol_version(),
        }
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

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_connections: default_db_max_connections(),
        }
    }
}

impl Default for RustfsConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            region: default_region(),
            access_key_id: None,
            secret_access_key: None,
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

impl Default for DiskPollingConfig {
    fn default() -> Self {
        Self {
            enabled: default_disk_polling_enabled(),
            interval_seconds: default_disk_polling_interval_seconds(),
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

fn override_bool(name: &str, value: &mut bool) {
    if let Ok(env_value) = env::var(name) {
        match env_value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => *value = true,
            "0" | "false" | "no" | "off" => *value = false,
            _ => {}
        }
    }
}

fn override_u64(name: &str, value: &mut u64) {
    if let Ok(env_value) = env::var(name) {
        if let Ok(parsed) = env_value.trim().parse::<u64>() {
            *value = parsed;
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

fn default_disk_polling_enabled() -> bool {
    true
}

fn default_disk_polling_interval_seconds() -> u64 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn loads_minimal_config_with_defaults() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_center_env();
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

    #[test]
    fn loads_complete_config_from_short_env_without_toml() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_center_env();
        std::env::set_var("CENTER_ID", "00000000-0000-0000-0000-000000000002");
        std::env::set_var("CENTER_NAME", "Center Dev");
        std::env::set_var("PROTOCOL_VERSION", "1.0");
        std::env::set_var("CENTER_BIND", "0.0.0.0:18080");
        std::env::set_var("CONTROL_API_TOKEN", "token");
        std::env::set_var("DATABASE_URL", "postgres://center:center@localhost/center");
        std::env::set_var("RUSTFS_ENDPOINT", "http://127.0.0.1:9000");
        std::env::set_var("RUSTFS_ACCESS_KEY", "access");
        std::env::set_var("RUSTFS_SECRET_KEY", "secret");
        std::env::set_var("DATA_DIR", ".runtime/center-data");
        std::env::set_var("LOG_DIR", ".runtime/center-log");
        std::env::set_var("TRANSPORT_MOUNT_ROOT", ".runtime/mnt");
        std::env::set_var("DISK_POLLING_ENABLED", "false");
        std::env::set_var("DISK_POLLING_INTERVAL_SECONDS", "3");

        let config = CenterConfig::from_env().expect("env-only config loads");

        assert_eq!(config.center.center_name, "Center Dev");
        assert_eq!(config.server.bind.port(), 18080);
        assert_eq!(config.server.control_api_token.as_deref(), Some("token"));
        assert_eq!(
            config.database.url,
            "postgres://center:center@localhost/center"
        );
        assert_eq!(config.rustfs.access_key_id.as_deref(), Some("access"));
        assert_eq!(
            config.paths.transport_mount_root.as_deref(),
            Some(".runtime/mnt")
        );
        assert!(!config.disk_polling.enabled);
        assert_eq!(config.disk_polling.interval_seconds, 3);
        clear_center_env();
    }

    fn clear_center_env() {
        std::env::remove_var("CENTER_ID");
        std::env::remove_var("CENTER_NAME");
        std::env::remove_var("PROTOCOL_VERSION");
        std::env::remove_var("CENTER_BIND");
        std::env::remove_var("CONTROL_API_TOKEN");
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("RUSTFS_ENDPOINT");
        std::env::remove_var("RUSTFS_REGION");
        std::env::remove_var("RUSTFS_ACCESS_KEY");
        std::env::remove_var("RUSTFS_SECRET_KEY");
        std::env::remove_var("DATA_DIR");
        std::env::remove_var("LOG_DIR");
        std::env::remove_var("TRANSPORT_MOUNT_ROOT");
        std::env::remove_var("DISK_POLLING_ENABLED");
        std::env::remove_var("DISK_POLLING_INTERVAL_SECONDS");
    }
}
