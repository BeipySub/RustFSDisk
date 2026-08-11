use serde::Deserialize;
use std::{env, fs, net::SocketAddr, path::Path};

#[derive(Debug, Clone, Deserialize)]
pub struct EdgeConfig {
    #[serde(default)]
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub center: CenterConfig,
    pub rustfs: RustfsConfig,
    #[serde(default)]
    pub paths: PathConfig,
    #[serde(default)]
    pub rescan: RescanConfig,
    #[serde(default)]
    pub auto_export: AutoExportConfig,
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
pub struct CenterConfig {
    pub base_url: String,
    pub edge_code: String,
    pub auth_key_id: String,
    #[serde(default)]
    pub edge_auth_secret: String,
    #[serde(default)]
    pub edge_auth_secret_env: Option<String>,
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
    pub disk_mount_roots: Vec<String>,
    pub transport_mount_root: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RescanConfig {
    pub endpoint_url: Option<String>,
    pub token: Option<String>,
    pub token_env: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AutoExportConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub start_on_ready: bool,
    #[serde(default = "default_auto_export_min_ready_disk_count")]
    pub min_ready_disk_count: usize,
    #[serde(default = "default_auto_export_cooldown_seconds")]
    pub cooldown_seconds: u64,
}

impl EdgeConfig {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let raw = fs::read_to_string(path)?;
        let mut config: Self = toml::from_str(&raw)?;
        config.apply_env_overrides();
        config.validate()?;
        Ok(config)
    }

    pub fn from_toml(raw: &str) -> anyhow::Result<Self> {
        let mut config: Self = toml::from_str(raw)?;
        config.apply_env_overrides();
        config.validate()?;
        Ok(config)
    }

    fn apply_env_overrides(&mut self) {
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
        if let Ok(token) = env::var("RUSTFS_TRANSFER__SERVER__CONTROL_API_TOKEN") {
            if !token.trim().is_empty() {
                self.server.control_api_token = Some(token);
            }
        }
        override_string("RUSTFS_TRANSFER__DATABASE__URL", &mut self.database.url);
        override_string(
            "RUSTFS_TRANSFER__CENTER__BASE_URL",
            &mut self.center.base_url,
        );
        override_string(
            "RUSTFS_TRANSFER__CENTER__EDGE_CODE",
            &mut self.center.edge_code,
        );
        override_string(
            "RUSTFS_TRANSFER__CENTER__AUTH_KEY_ID",
            &mut self.center.auth_key_id,
        );
        override_string(
            "RUSTFS_TRANSFER__CENTER__EDGE_AUTH_SECRET",
            &mut self.center.edge_auth_secret,
        );
        if self.center.edge_auth_secret.trim().is_empty() {
            if let Some(secret) = read_optional_env(self.center.edge_auth_secret_env.as_deref()) {
                self.center.edge_auth_secret = secret;
            }
        }
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
        if let Ok(root) = env::var("RUSTFS_TRANSFER__PATHS__TRANSPORT_MOUNT_ROOT") {
            if !root.trim().is_empty() {
                self.paths.disk_mount_roots = vec![root];
            }
        }
        if let Ok(roots) = env::var("RUSTFS_TRANSFER__PATHS__DISK_MOUNT_ROOTS") {
            let roots = split_list(&roots);
            if !roots.is_empty() {
                self.paths.disk_mount_roots = roots;
            }
        }
        if self.paths.disk_mount_roots.is_empty() {
            if let Some(root) = self.paths.transport_mount_root.as_ref() {
                if !root.trim().is_empty() {
                    self.paths.disk_mount_roots = vec![root.clone()];
                }
            }
        }
        if self.paths.disk_mount_roots.is_empty() {
            self.paths.disk_mount_roots = default_disk_mount_roots();
        }
        if let Ok(endpoint_url) = env::var("RUSTFS_TRANSFER__RESCAN__ENDPOINT_URL") {
            if !endpoint_url.trim().is_empty() {
                self.rescan.endpoint_url = Some(endpoint_url);
            }
        }
        if let Ok(token) = env::var("RUSTFS_TRANSFER__RESCAN__TOKEN") {
            if !token.trim().is_empty() {
                self.rescan.token = Some(token);
            }
        }
        if self.rescan.token.as_deref().unwrap_or("").trim().is_empty() {
            if let Some(token) = read_optional_env(self.rescan.token_env.as_deref()) {
                self.rescan.token = Some(token);
            }
        }
        override_bool(
            "RUSTFS_TRANSFER__AUTO_EXPORT__ENABLED",
            &mut self.auto_export.enabled,
        );
        override_bool(
            "RUSTFS_TRANSFER__AUTO_EXPORT__START_ON_READY",
            &mut self.auto_export.start_on_ready,
        );
        override_usize(
            "RUSTFS_TRANSFER__AUTO_EXPORT__MIN_READY_DISK_COUNT",
            &mut self.auto_export.min_ready_disk_count,
        );
        override_u64(
            "RUSTFS_TRANSFER__AUTO_EXPORT__COOLDOWN_SECONDS",
            &mut self.auto_export.cooldown_seconds,
        );
    }

    fn validate(&self) -> anyhow::Result<()> {
        ensure_non_empty("database.url", &self.database.url)?;
        ensure_non_empty("center.base_url", &self.center.base_url)?;
        ensure_non_empty("center.edge_code", &self.center.edge_code)?;
        ensure_non_empty("center.auth_key_id", &self.center.auth_key_id)?;
        ensure_non_empty("center.edge_auth_secret", &self.center.edge_auth_secret)?;
        ensure_non_empty("rustfs.endpoint", &self.rustfs.endpoint)?;
        ensure_optional_non_empty("rustfs.access_key_id", &self.rustfs.access_key_id)?;
        ensure_optional_non_empty("rustfs.secret_access_key", &self.rustfs.secret_access_key)?;
        Ok(())
    }

    pub fn rescan_endpoint_url(&self) -> String {
        self.rescan
            .endpoint_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "http://127.0.0.1:{}/internal/disk/rescan",
                    self.server.bind.port()
                )
            })
    }

    pub fn rescan_token(&self) -> Option<&str> {
        self.rescan
            .token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
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
            disk_mount_roots: default_disk_mount_roots(),
            transport_mount_root: None,
        }
    }
}

impl Default for AutoExportConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            start_on_ready: false,
            min_ready_disk_count: default_auto_export_min_ready_disk_count(),
            cooldown_seconds: default_auto_export_cooldown_seconds(),
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

fn override_usize(name: &str, value: &mut usize) {
    if let Ok(env_value) = env::var(name) {
        if let Ok(parsed) = env_value.trim().parse::<usize>() {
            *value = parsed;
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

fn split_list(value: &str) -> Vec<String> {
    value
        .split(|ch| ch == ';' || ch == ',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn ensure_non_empty(field: &str, value: &str) -> anyhow::Result<()> {
    if value.trim().is_empty() {
        anyhow::bail!("{field} must not be empty");
    }
    Ok(())
}

fn ensure_optional_non_empty(field: &str, value: &Option<String>) -> anyhow::Result<()> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|_| ())
        .ok_or_else(|| anyhow::anyhow!("{field} must not be empty"))
}

fn default_bind() -> SocketAddr {
    "0.0.0.0:8081".parse().expect("valid default bind address")
}

fn default_db_max_connections() -> u32 {
    5
}

fn default_region() -> String {
    "us-east-1".to_owned()
}

fn default_data_dir() -> String {
    "/var/lib/rustfs-transfer".to_owned()
}

fn default_log_dir() -> String {
    "/var/log/rustfs-transfer".to_owned()
}

fn default_disk_mount_roots() -> Vec<String> {
    vec!["/mnt/rustfs-transfer".to_owned()]
}

fn default_auto_export_min_ready_disk_count() -> usize {
    1
}

fn default_auto_export_cooldown_seconds() -> u64 {
    60
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn loads_minimal_config_with_defaults() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_rustfs_credential_env();
        let raw = r#"
            [database]
            url = "postgres://edge:edge@localhost/edge"

            [center]
            base_url = "http://center.local:8080"
            edge_code = "edge-a"
            auth_key_id = "auth-key-example"
            edge_auth_secret = "example-dev-secret"

            [rustfs]
            endpoint = "http://127.0.0.1:9000"
            access_key_id = "edge-access-key"
            secret_access_key = "edge-secret-key"
        "#;

        let config = EdgeConfig::from_toml(raw).expect("config loads");

        assert_eq!(config.server.bind.port(), 8081);
        assert_eq!(config.center.edge_code, "edge-a");
        assert_eq!(config.server.control_api_token, None);
        assert_eq!(config.rustfs.region, "us-east-1");
        assert_eq!(config.paths.disk_mount_roots, vec!["/mnt/rustfs-transfer"]);
        assert!(!config.auto_export.enabled);
        assert!(!config.auto_export.start_on_ready);
        assert_eq!(config.auto_export.min_ready_disk_count, 1);
        assert_eq!(config.auto_export.cooldown_seconds, 60);
        assert_eq!(
            config.rescan_endpoint_url(),
            "http://127.0.0.1:8081/internal/disk/rescan"
        );
    }

    #[test]
    fn loads_example_style_secret_env_and_transport_root() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_rustfs_credential_env();
        std::env::set_var("RUSTFS_TRANSFER__TEST_EDGE_SECRET", "secret-from-env");
        std::env::set_var("RUSTFS_TRANSFER__TEST_RESCAN_TOKEN", "rescan-token");
        std::env::set_var("RUSTFS_TRANSFER__TEST_CONTROL_TOKEN", "control-token");
        let raw = r#"
            [server]
            control_api_token_env = "RUSTFS_TRANSFER__TEST_CONTROL_TOKEN"

            [database]
            url = "postgres://edge:edge@localhost/edge"

            [center]
            base_url = "http://center.local:8080"
            edge_code = "edge-a"
            auth_key_id = "auth-key-example"
            edge_auth_secret_env = "RUSTFS_TRANSFER__TEST_EDGE_SECRET"

            [rustfs]
            endpoint = "http://127.0.0.1:9000"
            access_key_id = "edge-access-key"
            secret_access_key = "edge-secret-key"

            [paths]
            transport_mount_root = "/mnt/rustfs-transfer"

            [rescan]
            token_env = "RUSTFS_TRANSFER__TEST_RESCAN_TOKEN"
        "#;

        let config = EdgeConfig::from_toml(raw).expect("config loads");

        assert_eq!(config.center.edge_auth_secret, "secret-from-env");
        assert_eq!(
            config.server.control_api_token.as_deref(),
            Some("control-token")
        );
        assert_eq!(config.paths.disk_mount_roots, vec!["/mnt/rustfs-transfer"]);
        assert_eq!(config.rescan_token(), Some("rescan-token"));
        std::env::remove_var("RUSTFS_TRANSFER__TEST_EDGE_SECRET");
        std::env::remove_var("RUSTFS_TRANSFER__TEST_RESCAN_TOKEN");
        std::env::remove_var("RUSTFS_TRANSFER__TEST_CONTROL_TOKEN");
    }

    #[test]
    fn auto_export_env_overrides_support_gray_enable_and_rollback() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_rustfs_credential_env();
        std::env::set_var("RUSTFS_TRANSFER__AUTO_EXPORT__ENABLED", "true");
        std::env::set_var("RUSTFS_TRANSFER__AUTO_EXPORT__START_ON_READY", "1");
        std::env::set_var("RUSTFS_TRANSFER__AUTO_EXPORT__MIN_READY_DISK_COUNT", "2");
        std::env::set_var("RUSTFS_TRANSFER__AUTO_EXPORT__COOLDOWN_SECONDS", "300");
        let raw = r#"
            [database]
            url = "postgres://edge:edge@localhost/edge"

            [center]
            base_url = "http://center.local:8080"
            edge_code = "edge-a"
            auth_key_id = "auth-key-example"
            edge_auth_secret = "example-dev-secret"

            [rustfs]
            endpoint = "http://127.0.0.1:9000"
            access_key_id = "edge-access-key"
            secret_access_key = "edge-secret-key"
        "#;

        let config = EdgeConfig::from_toml(raw).expect("config loads");

        assert!(config.auto_export.enabled);
        assert!(config.auto_export.start_on_ready);
        assert_eq!(config.auto_export.min_ready_disk_count, 2);
        assert_eq!(config.auto_export.cooldown_seconds, 300);

        std::env::set_var("RUSTFS_TRANSFER__AUTO_EXPORT__ENABLED", "false");
        std::env::set_var("RUSTFS_TRANSFER__AUTO_EXPORT__START_ON_READY", "off");
        let config = EdgeConfig::from_toml(raw).expect("config reloads");

        assert!(!config.auto_export.enabled);
        assert!(!config.auto_export.start_on_ready);

        std::env::remove_var("RUSTFS_TRANSFER__AUTO_EXPORT__ENABLED");
        std::env::remove_var("RUSTFS_TRANSFER__AUTO_EXPORT__START_ON_READY");
        std::env::remove_var("RUSTFS_TRANSFER__AUTO_EXPORT__MIN_READY_DISK_COUNT");
        std::env::remove_var("RUSTFS_TRANSFER__AUTO_EXPORT__COOLDOWN_SECONDS");
    }

    #[test]
    fn rustfs_credentials_can_be_supplied_by_transfer_env() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_rustfs_credential_env();
        std::env::set_var("RUSTFS_TRANSFER__RUSTFS__ACCESS_KEY_ID", "env-access-key");
        std::env::set_var(
            "RUSTFS_TRANSFER__RUSTFS__SECRET_ACCESS_KEY",
            "env-secret-key",
        );
        let raw = r#"
            [database]
            url = "postgres://edge:edge@localhost/edge"

            [center]
            base_url = "http://center.local:8080"
            edge_code = "edge-a"
            auth_key_id = "auth-key-example"
            edge_auth_secret = "example-dev-secret"

            [rustfs]
            endpoint = "http://127.0.0.1:9000"
        "#;

        let config = EdgeConfig::from_toml(raw).expect("config loads");

        assert_eq!(
            config.rustfs.access_key_id.as_deref(),
            Some("env-access-key")
        );
        assert_eq!(
            config.rustfs.secret_access_key.as_deref(),
            Some("env-secret-key")
        );
        std::env::remove_var("RUSTFS_TRANSFER__RUSTFS__ACCESS_KEY_ID");
        std::env::remove_var("RUSTFS_TRANSFER__RUSTFS__SECRET_ACCESS_KEY");
    }

    #[test]
    fn missing_rustfs_credentials_are_diagnostic_config_errors() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_rustfs_credential_env();
        let raw = r#"
            [database]
            url = "postgres://edge:edge@localhost/edge"

            [center]
            base_url = "http://center.local:8080"
            edge_code = "edge-a"
            auth_key_id = "auth-key-example"
            edge_auth_secret = "example-dev-secret"

            [rustfs]
            endpoint = "http://127.0.0.1:9000"
            access_key_id = "edge-access-key"
        "#;

        let error = EdgeConfig::from_toml(raw).expect_err("missing secret should fail");

        assert!(error.to_string().contains("rustfs.secret_access_key"));
    }

    fn clear_rustfs_credential_env() {
        std::env::remove_var("RUSTFS_TRANSFER__RUSTFS__ACCESS_KEY_ID");
        std::env::remove_var("RUSTFS_TRANSFER__RUSTFS__SECRET_ACCESS_KEY");
        std::env::remove_var("RUSTFS_TRANSFER__AUTO_EXPORT__ENABLED");
        std::env::remove_var("RUSTFS_TRANSFER__AUTO_EXPORT__START_ON_READY");
        std::env::remove_var("RUSTFS_TRANSFER__AUTO_EXPORT__MIN_READY_DISK_COUNT");
        std::env::remove_var("RUSTFS_TRANSFER__AUTO_EXPORT__COOLDOWN_SECONDS");
    }
}
