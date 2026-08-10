use super::{ObjectHead, ObjectSummary, ScanError};
use aws_sdk_s3::Client;
use chrono::{DateTime, Utc};
use std::{collections::BTreeMap, future::Future, pin::Pin, time::SystemTime};

pub type BoxFutureResult<'a, T> = Pin<Box<dyn Future<Output = Result<T, ScanError>> + Send + 'a>>;

pub struct ObjectBody {
    pub bytes: Vec<u8>,
}

pub trait RustFsReadClient: Send + Sync {
    fn list_buckets(&self) -> BoxFutureResult<'_, Vec<String>>;

    fn list_objects<'a>(&'a self, bucket: &'a str) -> BoxFutureResult<'a, Vec<ObjectSummary>>;

    fn head_object<'a>(
        &'a self,
        bucket: &'a str,
        object_key: &'a str,
    ) -> BoxFutureResult<'a, ObjectHead>;

    fn get_object<'a>(
        &'a self,
        bucket: &'a str,
        object_key: &'a str,
    ) -> BoxFutureResult<'a, ObjectBody>;
}

#[derive(Debug, Clone)]
pub struct AwsS3RustFsReadClient {
    client: Client,
}

impl AwsS3RustFsReadClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl RustFsReadClient for AwsS3RustFsReadClient {
    fn list_buckets(&self) -> BoxFutureResult<'_, Vec<String>> {
        Box::pin(async move {
            let output = self
                .client
                .list_buckets()
                .send()
                .await
                .map_err(|err| ScanError::RustFs(err.to_string()))?;

            Ok(output
                .buckets()
                .iter()
                .filter_map(|bucket| bucket.name().map(str::to_owned))
                .collect())
        })
    }

    fn list_objects<'a>(&'a self, bucket: &'a str) -> BoxFutureResult<'a, Vec<ObjectSummary>> {
        Box::pin(async move {
            let mut objects = Vec::new();
            let mut continuation_token = None;

            loop {
                let output = self
                    .client
                    .list_objects_v2()
                    .bucket(bucket)
                    .set_continuation_token(continuation_token)
                    .send()
                    .await
                    .map_err(|err| ScanError::RustFs(err.to_string()))?;

                objects.extend(output.contents().iter().filter_map(|object| {
                    object.key().map(|object_key| ObjectSummary {
                        bucket: bucket.to_owned(),
                        object_key: object_key.to_owned(),
                    })
                }));

                if output.is_truncated().unwrap_or(false) {
                    continuation_token = output.next_continuation_token().map(str::to_owned);
                } else {
                    break;
                }
            }

            Ok(objects)
        })
    }

    fn head_object<'a>(
        &'a self,
        bucket: &'a str,
        object_key: &'a str,
    ) -> BoxFutureResult<'a, ObjectHead> {
        Box::pin(async move {
            let output = self
                .client
                .head_object()
                .bucket(bucket)
                .key(object_key)
                .send()
                .await
                .map_err(|err| ScanError::RustFs(err.to_string()))?;

            let last_modified = output
                .last_modified()
                .ok_or_else(|| ScanError::RustFs("HEAD missing last_modified".to_owned()))
                .and_then(smithy_time_to_utc)?;
            let etag = output
                .e_tag()
                .ok_or_else(|| ScanError::RustFs("HEAD missing ETag".to_owned()))?
                .to_owned();
            let size_bytes = output
                .content_length()
                .ok_or_else(|| ScanError::RustFs("HEAD missing content_length".to_owned()))?;
            let metadata = output
                .metadata()
                .map(|metadata| {
                    metadata
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect::<BTreeMap<_, _>>()
                })
                .unwrap_or_default();

            Ok(ObjectHead {
                bucket: bucket.to_owned(),
                object_key: object_key.to_owned(),
                etag,
                size_bytes,
                last_modified,
                metadata,
            })
        })
    }

    fn get_object<'a>(
        &'a self,
        bucket: &'a str,
        object_key: &'a str,
    ) -> BoxFutureResult<'a, ObjectBody> {
        Box::pin(async move {
            let output = self
                .client
                .get_object()
                .bucket(bucket)
                .key(object_key)
                .send()
                .await
                .map_err(|err| ScanError::RustFs(err.to_string()))?;
            let bytes = output
                .body
                .collect()
                .await
                .map_err(|err| ScanError::RustFs(err.to_string()))?
                .into_bytes()
                .to_vec();

            Ok(ObjectBody { bytes })
        })
    }
}

fn smithy_time_to_utc(
    timestamp: &aws_sdk_s3::primitives::DateTime,
) -> Result<DateTime<Utc>, ScanError> {
    let system_time = SystemTime::try_from(*timestamp)
        .map_err(|err| ScanError::RustFs(format!("invalid last_modified: {err}")))?;
    Ok(DateTime::<Utc>::from(system_time))
}
