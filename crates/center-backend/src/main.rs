use std::{env, path::PathBuf};

use anyhow::Context;
use aws_sdk_s3::config::Credentials;
use rustfs_transfer_center::{
    center_security::{CenterSecurity, SIGNATURE_ALG_HMAC_SHA256},
    config::RustfsConfig,
    import_runtime::ProductionCenterImportControlService,
    reinitialize_runtime::ProductionCenterReinitializeControlService,
    reinitializer::DiskInfoTemplate,
    router, AppState, CenterConfig, CenterService, CenterStore, PgCenterStore,
};
use sqlx::{postgres::PgPoolOptions, Row};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const DEFAULT_CONFIG_PATH: &str = "/etc/rustfs-transfer/center.toml";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config_path = config_path();
    let config = CenterConfig::load(&config_path)
        .with_context(|| format!("load center config from {}", config_path.display()))?;
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
    let center_config = service_center_config(&pool).await?;
    let reinitialize_control = ProductionCenterReinitializeControlService::new(
        pool.clone(),
        DiskInfoTemplate {
            protocol_version: center_config.protocol_version,
            center_id: center_config.center_id,
            center_name: None,
            center_key_id: security.center_key_id(),
            signature_alg: SIGNATURE_ALG_HMAC_SHA256.to_string(),
        },
        security.clone(),
    );

    let service = CenterService::new(CenterStore::Pg(PgCenterStore::new(pool)), security);
    let app = router(
        AppState::new(service)
            .with_control_api_token(config.server.control_api_token.clone())
            .with_import_control(std::sync::Arc::new(import_control))
            .with_reinitialize_control(std::sync::Arc::new(reinitialize_control)),
    );
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

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().json())
        .init();
}

fn config_path() -> PathBuf {
    env::var("RUSTFS_TRANSFER__CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_CONFIG_PATH))
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

async fn service_center_config(pool: &sqlx::PgPool) -> anyhow::Result<CenterConfigForReinit> {
    let row = sqlx::query(
        "SELECT center_id, protocol_version FROM center_config ORDER BY id ASC LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .context("load center_config for reinitialize control")?;
    Ok(CenterConfigForReinit {
        center_id: row.get("center_id"),
        protocol_version: row.get("protocol_version"),
    })
}

struct CenterConfigForReinit {
    center_id: uuid::Uuid,
    protocol_version: String,
}
