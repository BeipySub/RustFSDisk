use crate::config::{DatabaseConfig, EdgeConfig, PathConfig, RustfsConfig};
use aws_sdk_s3::config::Credentials;
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};
use uuid::Uuid;

pub type HealthFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

pub trait DatabaseAdapter: Send + Sync {
    fn check<'a>(&'a self) -> HealthFuture<'a>;
}

pub trait ObjectStoreAdapter: Send + Sync {
    fn check<'a>(&'a self) -> HealthFuture<'a>;
}

pub trait DiskAdapter: Send + Sync {
    fn mount_roots(&self) -> Vec<PathBuf>;
}

pub trait Clock: Send + Sync {
    fn now_utc(&self) -> DateTime<Utc>;
}

pub trait IdGenerator: Send + Sync {
    fn new_uuid(&self) -> Uuid;
}

#[derive(Clone)]
pub struct AdapterBundle {
    pub database: Arc<dyn DatabaseAdapter>,
    pub object_store: Arc<dyn ObjectStoreAdapter>,
    pub disk: Arc<dyn DiskAdapter>,
    pub clock: Arc<dyn Clock>,
    pub ids: Arc<dyn IdGenerator>,
    pub pg_pool: Option<PgPool>,
    pub s3_client: Option<aws_sdk_s3::Client>,
}

impl AdapterBundle {
    pub async fn from_config(config: &EdgeConfig) -> anyhow::Result<Self> {
        let database = Arc::new(PostgresDatabase::connect(&config.database).await?);
        let object_store = Arc::new(S3ObjectStore::from_config(&config.rustfs).await?);
        Ok(Self {
            database: database.clone(),
            object_store: object_store.clone(),
            disk: Arc::new(LocalDiskAdapter::from_config(&config.paths)),
            clock: Arc::new(SystemClock),
            ids: Arc::new(UuidGenerator),
            pg_pool: Some(database.pool()),
            s3_client: Some(object_store.client()),
        })
    }
}

pub struct PostgresDatabase {
    pool: PgPool,
}

impl PostgresDatabase {
    pub async fn connect(config: &DatabaseConfig) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .connect_lazy(&config.url)?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> PgPool {
        self.pool.clone()
    }
}

impl DatabaseAdapter for PostgresDatabase {
    fn check<'a>(&'a self) -> HealthFuture<'a> {
        Box::pin(async move {
            sqlx::query("SELECT 1").execute(&self.pool).await?;
            Ok(())
        })
    }
}

pub struct S3ObjectStore {
    #[allow(dead_code)]
    client: aws_sdk_s3::Client,
    #[allow(dead_code)]
    endpoint: String,
}

impl S3ObjectStore {
    pub async fn from_config(config: &RustfsConfig) -> anyhow::Result<Self> {
        let credentials = Self::rustfs_credentials(config)?;
        let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .credentials_provider(credentials)
            .region(aws_config::Region::new(config.region.clone()))
            .endpoint_url(config.endpoint.clone())
            .load()
            .await;

        Ok(Self {
            client: aws_sdk_s3::Client::new(&sdk_config),
            endpoint: config.endpoint.clone(),
        })
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
            "rustfs-transfer-edge-config",
        ))
    }

    pub fn client(&self) -> aws_sdk_s3::Client {
        self.client.clone()
    }
}

impl ObjectStoreAdapter for S3ObjectStore {
    fn check<'a>(&'a self) -> HealthFuture<'a> {
        Box::pin(async move {
            if self.endpoint.trim().is_empty() {
                anyhow::bail!("rustfs endpoint is empty");
            }
            Ok(())
        })
    }
}

fn required_rustfs_secret<'a>(field: &str, value: &'a Option<String>) -> anyhow::Result<&'a str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{field} must not be empty"))
}

pub struct LocalDiskAdapter {
    mount_roots: Vec<PathBuf>,
}

impl LocalDiskAdapter {
    pub fn from_config(config: &PathConfig) -> Self {
        Self {
            mount_roots: config.disk_mount_roots.iter().map(PathBuf::from).collect(),
        }
    }
}

impl DiskAdapter for LocalDiskAdapter {
    fn mount_roots(&self) -> Vec<PathBuf> {
        self.mount_roots.clone()
    }
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub struct UuidGenerator;

impl IdGenerator for UuidGenerator {
    fn new_uuid(&self) -> Uuid {
        Uuid::new_v4()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeDisk;

    impl DiskAdapter for FakeDisk {
        fn mount_roots(&self) -> Vec<PathBuf> {
            vec![PathBuf::from("/tmp/fake-disk-root")]
        }
    }

    #[test]
    fn disk_adapter_can_be_replaced_in_tests() {
        let adapter: Arc<dyn DiskAdapter> = Arc::new(FakeDisk);

        assert_eq!(
            adapter.mount_roots(),
            vec![PathBuf::from("/tmp/fake-disk-root")]
        );
    }

    #[tokio::test]
    async fn s3_client_uses_configured_credentials_without_aws_environment() {
        clear_aws_credential_environment();
        let config = RustfsConfig {
            endpoint: "http://127.0.0.1:9000".to_owned(),
            region: "us-east-1".to_owned(),
            access_key_id: Some("edge-access-key".to_owned()),
            secret_access_key: Some("edge-secret-key".to_owned()),
        };

        let credentials = S3ObjectStore::rustfs_credentials(&config)
            .expect("configured RustFS credentials should resolve without AWS environment");

        assert_eq!(credentials.access_key_id(), "edge-access-key");
    }

    #[test]
    fn s3_adapter_reports_missing_credentials_before_scan_dispatch() {
        let config = RustfsConfig {
            endpoint: "http://127.0.0.1:9000".to_owned(),
            region: "us-east-1".to_owned(),
            access_key_id: Some("edge-access-key".to_owned()),
            secret_access_key: None,
        };

        let error =
            S3ObjectStore::rustfs_credentials(&config).expect_err("missing secret should fail");

        assert!(error.to_string().contains("rustfs.secret_access_key"));
    }

    fn clear_aws_credential_environment() {
        for key in [
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
            "AWS_PROFILE",
            "AWS_SHARED_CREDENTIALS_FILE",
            "AWS_CONFIG_FILE",
            "AWS_CONTAINER_CREDENTIALS_FULL_URI",
            "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
            "AWS_WEB_IDENTITY_TOKEN_FILE",
            "AWS_ROLE_ARN",
        ] {
            std::env::remove_var(key);
        }
    }
}
