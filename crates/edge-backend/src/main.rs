use anyhow::Context;
use rustfs_transfer_edge::{app, AppState, EdgeConfig};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    configure_open_file_creation_permissions();
    init_tracing();

    let config = EdgeConfig::from_env().context("load edge config from environment")?;
    let bind = config.server.bind;
    let edge_code = config.edge.edge_code.clone();
    let state = AppState::from_config(config).await?;
    state.request_startup_disk_scan().await;
    state.spawn_disk_polling();
    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("bind edge HTTP listener on {bind}"))?;

    info!(
        service = "rustfs-transfer-edge",
        bind = %bind,
        edge_code = %edge_code,
        "starting edge axum server"
    );
    axum::serve(listener, app(state))
        .await
        .context("edge axum server exited")?;
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
