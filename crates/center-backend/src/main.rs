use std::{env, path::PathBuf};

use anyhow::Context;
use aws_sdk_s3::config::Credentials;
use rustfs_transfer_center::{
    center_security::{CenterSecurity, SIGNATURE_ALG_HMAC_SHA256},
    config::RustfsConfig,
    import_runtime::ProductionCenterImportControlService,
    reinitialize_runtime::ProductionCenterReinitializeControlService,
    reinitializer::DiskInfoTemplate,
    router, AppState, CenterConfig, CenterIdentity, CenterService, CenterStore, PgCenterStore,
};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    configure_open_file_creation_permissions();
    init_tracing();

    let config = load_config().context("load center config")?;
    let security = CenterSecurity::from_config(&config.security)
        .context("load center security keys from configured environment variables")?;

    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .connect_lazy(&config.database.url)
        .context("initialize center PostgreSQL pool")?;

    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .credentials_provider(rustfs_credentials(&config.rustfs)?)
        .region(aws_config::Region::new(config.rustfs.region.clone()))
        .endpoint_url(config.rustfs.endpoint.clone())
        .load()
        .await;
    let s3_client = aws_sdk_s3::Client::new(&sdk_config);
    let import_control =
        ProductionCenterImportControlService::new(pool.clone(), s3_client, security.clone());
    let reinitialize_control = ProductionCenterReinitializeControlService::new(
        pool.clone(),
        DiskInfoTemplate {
            protocol_version: config.center.protocol_version.clone(),
            center_id: config.center.center_id,
            center_name: Some(config.center.center_name.clone()),
            center_key_id: security.center_key_id(),
            signature_alg: SIGNATURE_ALG_HMAC_SHA256.to_string(),
        },
        security.clone(),
    );

    let center_identity = CenterIdentity::from(&config.center);
    let service = CenterService::new(
        CenterStore::Pg(PgCenterStore::new(pool)),
        security,
        center_identity,
    );
    let app_state = AppState::new(service)
        .with_control_api_token(config.server.control_api_token.clone())
        .with_import_control(std::sync::Arc::new(import_control))
        .with_reinitialize_control(std::sync::Arc::new(reinitialize_control));
    if config.disk_polling.enabled {
        app_state.spawn_disk_polling(config.disk_polling.interval_seconds);
    }
    let app = router(app_state);
    let listener = TcpListener::bind(config.server.bind)
        .await
        .with_context(|| format!("bind center HTTP listener on {}", config.server.bind))?;

    info!(
        service = "rustfs-transfer-center",
        bind = %config.server.bind,
        "starting center axum server"
    );
    axum::serve(listener, app)
        .await
        .context("center axum server exited")?;
    Ok(())
}

#[cfg(unix)]
fn configure_open_file_creation_permissions() {
    // Transport disk files and directories must be usable by every Linux user.
    unsafe { libc::umask(0) };
}

#[cfg(not(unix))]
fn configure_open_file_creation_permissions() {}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().json())
        .init();
}

fn load_config() -> anyhow::Result<CenterConfig> {
    match env::var("CENTER_CONFIG_PATH") {
        Ok(path) if !path.trim().is_empty() => CenterConfig::load(PathBuf::from(path)),
        _ => CenterConfig::from_env(),
    }
}

fn rustfs_credentials(config: &RustfsConfig) -> anyhow::Result<Credentials> {
    let access_key_id = required_rustfs_secret("rustfs.access_key_id", &config.access_key_id)?;
    let secret_access_key =
        required_rustfs_secret("rustfs.secret_access_key", &config.secret_access_key)?;
    Ok(Credentials::new(
        access_key_id,
        secret_access_key,
        None,
        None,
        "rustfs-transfer-center-config",
    ))
}

fn required_rustfs_secret<'a>(field: &str, value: &'a Option<String>) -> anyhow::Result<&'a str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{field} must not be empty"))
}
