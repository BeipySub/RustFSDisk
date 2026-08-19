use serde::Deserialize;
use std::{env, fs, net::SocketAddr, path::Path};

#[derive(Debug, Clone, Deserialize)]
pub struct EdgeConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub edge: EdgeIdentityConfig,
    #[serde(default)]
    pub rustfs: RustfsConfig,
    #[serde(default)]
    pub paths: PathConfig,
    #[serde(default)]
    pub disk_polling: DiskPollingConfig,
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub auto_export: AutoExportConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_db_max_connections")]
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EdgeIdentityConfig {
    pub edge_code: String,
    pub edge_key: String,
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

#[derive(Debug, Clone, Deserialize)]
pub struct ScanConfig {
    #[serde(default = "default_scan_reuse_window_minutes")]
    pub reuse_window_minutes: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiskPollingConfig {
    #[serde(default = "default_disk_polling_enabled")]
    pub enabled: bool,
    #[serde(default = "default_disk_polling_interval_seconds")]
    pub interval_seconds: u64,
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

    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::default();
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
        override_socket_addr("EDGE_BIND", &mut self.server.bind);
        override_string("DATABASE_URL", &mut self.database.url);
        override_string("EDGE_CODE", &mut self.edge.edge_code);
        override_string("EDGE_KEY", &mut self.edge.edge_key);
        override_string("RUSTFS_ENDPOINT", &mut self.rustfs.endpoint);
        override_string("RUSTFS_REGION", &mut self.rustfs.region);
        override_optional_string("RUSTFS_ACCESS_KEY", &mut self.rustfs.access_key_id);
        override_optional_string("RUSTFS_SECRET_KEY", &mut self.rustfs.secret_access_key);
        override_string("DATA_DIR", &mut self.paths.data_dir);
        override_string("LOG_DIR", &mut self.paths.log_dir);
        if let Ok(root) = env::var("TRANSPORT_MOUNT_ROOT") {
            if !root.trim().is_empty() {
                self.paths.disk_mount_roots = vec![root];
            }
        }
        if let Ok(roots) = env::var("DISK_MOUNT_ROOTS") {
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
        override_bool("DISK_POLLING_ENABLED", &mut self.disk_polling.enabled);
        override_u64(
            "DISK_POLLING_INTERVAL_SECONDS",
            &mut self.disk_polling.interval_seconds,
        );
        override_u64("SCAN_REUSE_MINUTES", &mut self.scan.reuse_window_minutes);
        override_bool("AUTO_EXPORT_ENABLED", &mut self.auto_export.enabled);
        override_bool(
            "AUTO_EXPORT_START_ON_READY",
            &mut self.auto_export.start_on_ready,
        );
        override_usize(
            "AUTO_EXPORT_MIN_READY_DISK_COUNT",
            &mut self.auto_export.min_ready_disk_count,
        );
        override_u64(
            "AUTO_EXPORT_COOLDOWN_SECONDS",
            &mut self.auto_export.cooldown_seconds,
        );
    }

    fn validate(&self) -> anyhow::Result<()> {
        ensure_non_empty("database.url", &self.database.url)?;
        ensure_non_empty("edge.edge_code", &self.edge.edge_code)?;
        ensure_non_empty("edge.edge_key", &self.edge.edge_key)?;
        ensure_non_empty("rustfs.endpoint", &self.rustfs.endpoint)?;
        ensure_optional_non_empty("rustfs.access_key_id", &self.rustfs.access_key_id)?;
        ensure_optional_non_empty("rustfs.secret_access_key", &self.rustfs.secret_access_key)?;
        Ok(())
    }
}

impl Default for EdgeConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            edge: EdgeIdentityConfig::default(),
            rustfs: RustfsConfig::default(),
            paths: PathConfig::default(),
            disk_polling: DiskPollingConfig::default(),
            scan: ScanConfig::default(),
            auto_export: AutoExportConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
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

impl Default for EdgeIdentityConfig {
    fn default() -> Self {
        Self {
            edge_code: String::new(),
            edge_key: String::new(),
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
            disk_mount_roots: default_disk_mount_roots(),
            transport_mount_root: None,
        }
    }
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            reuse_window_minutes: default_scan_reuse_window_minutes(),
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
        if env_value.trim().is_empty() {
            return;
        }
        *value = env_value;
    }
}

fn override_socket_addr(name: &str, value: &mut SocketAddr) {
    if let Ok(env_value) = env::var(name) {
        if let Ok(parsed) = env_value.trim().parse::<SocketAddr>() {
            *value = parsed;
        }
    }
}

fn override_optional_string(name: &str, value: &mut Option<String>) {
    if let Ok(env_value) = env::var(name) {
        if env_value.trim().is_empty() {
            return;
        }
        *value = Some(env_value);
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
    vec![
        "/mnt/rustfs-transfer".to_owned(),
        "/media".to_owned(),
        "/run/media".to_owned(),
    ]
}

fn default_auto_export_min_ready_disk_count() -> usize {
    1
}

fn default_scan_reuse_window_minutes() -> u64 {
    24 * 60
}

fn default_auto_export_cooldown_seconds() -> u64 {
    60
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
        clear_rustfs_credential_env();
        let raw = r#"
            [database]
            url = "postgres://edge:edge@localhost/edge"

            [edge]
            edge_code = "edge-a"
            edge_key = "example-dev-key"

            [rustfs]
            endpoint = "http://127.0.0.1:9000"
            access_key_id = "edge-access-key"
            secret_access_key = "edge-secret-key"
        "#;

        let config = EdgeConfig::from_toml(raw).expect("config loads");

        assert_eq!(config.server.bind.port(), 8081);
        assert_eq!(config.edge.edge_code, "edge-a");
        assert_eq!(config.edge.edge_key, "example-dev-key");
        assert_eq!(config.rustfs.region, "us-east-1");
        assert_eq!(
            config.paths.disk_mount_roots,
            vec!["/mnt/rustfs-transfer", "/media", "/run/media"]
        );
        assert!(!config.auto_export.enabled);
        assert!(!config.auto_export.start_on_ready);
        assert_eq!(config.scan.reuse_window_minutes, 24 * 60);
        assert_eq!(config.auto_export.min_ready_disk_count, 1);
        assert_eq!(config.auto_export.cooldown_seconds, 60);
        assert!(config.disk_polling.enabled);
        assert_eq!(config.disk_polling.interval_seconds, 1);
    }

    #[test]
    fn loads_example_style_edge_key_env_and_transport_root() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_rustfs_credential_env();
        std::env::set_var("EDGE_KEY", "key-from-env");
        let raw = r#"
            [database]
            url = "postgres://edge:edge@localhost/edge"

            [edge]
            edge_code = "edge-a"
            edge_key = "example-dev-key"

            [rustfs]
            endpoint = "http://127.0.0.1:9000"
            access_key_id = "edge-access-key"
            secret_access_key = "edge-secret-key"

            [paths]
            transport_mount_root = "/mnt/rustfs-transfer"
        "#;

        let config = EdgeConfig::from_toml(raw).expect("config loads");

        assert_eq!(config.edge.edge_key, "key-from-env");
        assert_eq!(config.paths.disk_mount_roots, vec!["/mnt/rustfs-transfer"]);
        std::env::remove_var("EDGE_KEY");
    }

    #[test]
    fn edge_identity_is_required_for_offline_export() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_rustfs_credential_env();
        let raw = r#"
            [database]
            url = "postgres://edge:edge@localhost/edge"

            [edge]
            edge_code = "edge-a"
            edge_key = "example-dev-key"

            [rustfs]
            endpoint = "http://127.0.0.1:9000"
            access_key_id = "edge-access-key"
            secret_access_key = "edge-secret-key"
        "#;

        let config = EdgeConfig::from_toml(raw).expect("offline edge config loads");

        assert_eq!(config.edge.edge_code, "edge-a");
        assert_eq!(config.edge.edge_key, "example-dev-key");
    }

    #[test]
    fn loads_complete_config_from_env_without_toml() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_rustfs_credential_env();
        std::env::set_var("DATABASE_URL", "postgres://edge:edge@localhost/edge");
        std::env::set_var("EDGE_CODE", "edge-a");
        std::env::set_var("EDGE_KEY", "edge-key-from-env");
        std::env::set_var("RUSTFS_ENDPOINT", "http://127.0.0.1:9000");
        std::env::set_var("RUSTFS_ACCESS_KEY", "env-access-key");
        std::env::set_var("RUSTFS_SECRET_KEY", "env-secret-key");

        let config = EdgeConfig::from_env().expect("env-only config loads");

        assert_eq!(config.server.bind.port(), 8081);
        assert_eq!(config.edge.edge_code, "edge-a");
        assert_eq!(config.edge.edge_key, "edge-key-from-env");
        assert_eq!(config.rustfs.endpoint, "http://127.0.0.1:9000");
        clear_rustfs_credential_env();
    }

    #[test]
    fn auto_export_env_overrides_support_gray_enable_and_rollback() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_rustfs_credential_env();
        std::env::set_var("AUTO_EXPORT_ENABLED", "true");
        std::env::set_var("AUTO_EXPORT_START_ON_READY", "1");
        std::env::set_var("AUTO_EXPORT_MIN_READY_DISK_COUNT", "2");
        std::env::set_var("AUTO_EXPORT_COOLDOWN_SECONDS", "300");
        std::env::set_var("SCAN_REUSE_MINUTES", "15");
        std::env::set_var("DISK_POLLING_ENABLED", "false");
        std::env::set_var("DISK_POLLING_INTERVAL_SECONDS", "3");
        let raw = r#"
            [database]
            url = "postgres://edge:edge@localhost/edge"

            [edge]
            edge_code = "edge-a"
            edge_key = "example-dev-key"

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
        assert_eq!(config.scan.reuse_window_minutes, 15);
        assert!(!config.disk_polling.enabled);
        assert_eq!(config.disk_polling.interval_seconds, 3);

        std::env::set_var("AUTO_EXPORT_ENABLED", "false");
        std::env::set_var("AUTO_EXPORT_START_ON_READY", "off");
        let config = EdgeConfig::from_toml(raw).expect("config reloads");

        assert!(!config.auto_export.enabled);
        assert!(!config.auto_export.start_on_ready);

        clear_short_env();
    }

    #[test]
    fn rustfs_credentials_can_be_supplied_by_transfer_env() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_rustfs_credential_env();
        std::env::set_var("RUSTFS_ACCESS_KEY", "env-access-key");
        std::env::set_var("RUSTFS_SECRET_KEY", "env-secret-key");
        let raw = r#"
            [database]
            url = "postgres://edge:edge@localhost/edge"

            [edge]
            edge_code = "edge-a"
            edge_key = "example-dev-key"

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
        std::env::remove_var("RUSTFS_ACCESS_KEY");
        std::env::remove_var("RUSTFS_SECRET_KEY");
    }

    #[test]
    fn missing_rustfs_credentials_are_diagnostic_config_errors() {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        clear_rustfs_credential_env();
        let raw = r#"
            [database]
            url = "postgres://edge:edge@localhost/edge"

            [edge]
            edge_code = "edge-a"
            edge_key = "example-dev-key"

            [rustfs]
            endpoint = "http://127.0.0.1:9000"
            access_key_id = "edge-access-key"
        "#;

        let error = EdgeConfig::from_toml(raw).expect_err("missing secret should fail");

        assert!(error.to_string().contains("rustfs.secret_access_key"));
    }

    fn clear_rustfs_credential_env() {
        clear_short_env();
    }

    fn clear_short_env() {
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("EDGE_BIND");
        std::env::remove_var("EDGE_CODE");
        std::env::remove_var("EDGE_KEY");
        std::env::remove_var("RUSTFS_ENDPOINT");
        std::env::remove_var("RUSTFS_REGION");
        std::env::remove_var("RUSTFS_ACCESS_KEY");
        std::env::remove_var("RUSTFS_SECRET_KEY");
        std::env::remove_var("DATA_DIR");
        std::env::remove_var("LOG_DIR");
        std::env::remove_var("TRANSPORT_MOUNT_ROOT");
        std::env::remove_var("DISK_MOUNT_ROOTS");
        std::env::remove_var("DISK_POLLING_ENABLED");
        std::env::remove_var("DISK_POLLING_INTERVAL_SECONDS");
        std::env::remove_var("SCAN_REUSE_MINUTES");
        std::env::remove_var("AUTO_EXPORT_ENABLED");
        std::env::remove_var("AUTO_EXPORT_START_ON_READY");
        std::env::remove_var("AUTO_EXPORT_MIN_READY_DISK_COUNT");
        std::env::remove_var("AUTO_EXPORT_COOLDOWN_SECONDS");
    }
}
