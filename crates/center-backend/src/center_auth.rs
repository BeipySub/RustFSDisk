use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{HeaderMap, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Duration, Utc};
use rustfs_transfer_common::{
    crypto::{sha256_lower_hex, verify_hmac_base64, CanonicalRequest, QueryParam},
    error::TransferErrorCode,
    protocol::{EdgeAuthResponse, EdgeStatus},
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{AppState, EdgeRecord};

pub const HEADER_EDGE_CODE: &str = "X-Edge-Code";
pub const HEADER_AUTH_KEY_ID: &str = "X-Auth-Key-Id";
pub const HEADER_TIMESTAMP: &str = "X-Timestamp";
pub const HEADER_NONCE: &str = "X-Nonce";
pub const HEADER_BODY_SHA256: &str = "X-Body-SHA256";
pub const HEADER_SIGNATURE: &str = "X-Signature";

const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_TIMESTAMP_SKEW_SECONDS: i64 = 300;
const NONCE_TTL_SECONDS: i64 = 600;

static NONCE_CACHE: LazyLock<NonceCache> = LazyLock::new(NonceCache::default);

#[derive(Debug, Deserialize)]
struct EdgeAuthBody {
    #[serde(default)]
    client_version: Option<String>,
}

pub async fn edge_auth_handler(
    State(state): State<AppState>,
    request: Request<Body>,
) -> Result<Json<EdgeAuthResponse>, AuthError> {
    let authenticated = authenticate_edge_request(&state, request).await?;
    let auth_body: EdgeAuthBody = serde_json::from_slice(&authenticated.body)
        .map_err(|_| AuthError::invalid_request(authenticated.request_id, "invalid_json"))?;

    info!(
        request_id = %authenticated.request_id,
        edge_code = %authenticated.edge.edge_code,
        auth_key_id = %authenticated.edge.auth_key_id,
        client_version = auth_body.client_version.as_deref().unwrap_or(""),
        "edge auth accepted"
    );

    Ok(Json(EdgeAuthResponse {
        allowed: true,
        edge_code: authenticated.edge.edge_code,
        edge_name: authenticated.edge.edge_name,
        edge_status: EdgeStatus::Active,
        server_time: authenticated.verified_at.to_rfc3339(),
        message: None,
    }))
}

#[derive(Debug)]
pub struct AuthenticatedEdgeRequest {
    pub request_id: Uuid,
    pub edge: EdgeRecord,
    pub body: Vec<u8>,
    pub verified_at: DateTime<Utc>,
}

pub async fn authenticate_edge_request(
    state: &AppState,
    request: Request<Body>,
) -> Result<AuthenticatedEdgeRequest, AuthError> {
    let request_id = Uuid::new_v4();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    let body = to_bytes(request.into_body(), MAX_BODY_BYTES)
        .await
        .map_err(|_| AuthError::invalid_request(request_id, "body_read_failed"))?
        .to_vec();

    let edge_code = required_header(&headers, HEADER_EDGE_CODE)
        .map_err(|reason| AuthError::unauthorized(request_id, None, None, reason))?;
    let auth_key_id = required_header(&headers, HEADER_AUTH_KEY_ID)
        .map_err(|reason| AuthError::unauthorized(request_id, Some(&edge_code), None, reason))?;
    let timestamp = required_header(&headers, HEADER_TIMESTAMP).map_err(|reason| {
        AuthError::unauthorized(request_id, Some(&edge_code), Some(&auth_key_id), reason)
    })?;
    let nonce = required_header(&headers, HEADER_NONCE).map_err(|reason| {
        AuthError::unauthorized(request_id, Some(&edge_code), Some(&auth_key_id), reason)
    })?;
    let body_sha256 = required_header(&headers, HEADER_BODY_SHA256).map_err(|reason| {
        AuthError::unauthorized(request_id, Some(&edge_code), Some(&auth_key_id), reason)
    })?;
    let signature = required_header(&headers, HEADER_SIGNATURE).map_err(|reason| {
        AuthError::unauthorized(request_id, Some(&edge_code), Some(&auth_key_id), reason)
    })?;

    let edge = state
        .service
        .edge_for_auth(&edge_code)
        .await
        .map_err(|_| {
            AuthError::unauthorized(
                request_id,
                Some(&edge_code),
                Some(&auth_key_id),
                "edge_lookup_failed",
            )
        })?
        .ok_or_else(|| {
            AuthError::unauthorized(
                request_id,
                Some(&edge_code),
                Some(&auth_key_id),
                "edge_not_found",
            )
        })?;

    if edge.edge_status != "ACTIVE" || edge.auth_key_id != auth_key_id {
        return Err(AuthError::unauthorized(
            request_id,
            Some(&edge_code),
            Some(&auth_key_id),
            "edge_disabled_or_key_mismatch",
        ));
    }

    let request_time = DateTime::parse_from_rfc3339(&timestamp)
        .map_err(|_| {
            AuthError::unauthorized(
                request_id,
                Some(&edge_code),
                Some(&auth_key_id),
                "bad_timestamp",
            )
        })?
        .with_timezone(&Utc);
    let now = Utc::now();
    if (now - request_time).num_seconds().abs() > MAX_TIMESTAMP_SKEW_SECONDS {
        return Err(AuthError::unauthorized(
            request_id,
            Some(&edge_code),
            Some(&auth_key_id),
            "timestamp_skew",
        ));
    }

    if sha256_lower_hex(&body) != body_sha256 {
        return Err(AuthError::unauthorized(
            request_id,
            Some(&edge_code),
            Some(&auth_key_id),
            "body_sha256_mismatch",
        ));
    }

    let canonical = CanonicalRequest {
        method: method.as_str().to_ascii_uppercase(),
        canonical_path_with_query: canonical_path_with_query(&uri),
        timestamp,
        nonce: nonce.clone(),
        body_sha256,
    };
    verify_hmac_base64(edge.auth_secret.as_bytes(), &canonical, &signature).map_err(|_| {
        AuthError::unauthorized(
            request_id,
            Some(&edge_code),
            Some(&auth_key_id),
            "bad_signature",
        )
    })?;

    if !NONCE_CACHE.insert_once(&auth_key_id, &nonce, now) {
        return Err(AuthError::unauthorized(
            request_id,
            Some(&edge_code),
            Some(&auth_key_id),
            "nonce_replay",
        ));
    }

    reject_edge_code_mismatch(request_id, &body, &edge.edge_code)?;

    Ok(AuthenticatedEdgeRequest {
        request_id,
        edge,
        body,
        verified_at: now,
    })
}

pub fn signed_headers(
    method: &str,
    path_with_query: &str,
    edge_code: &str,
    auth_key_id: &str,
    timestamp: &str,
    nonce: &str,
    body: &[u8],
    secret: &[u8],
) -> anyhow::Result<HeaderMap> {
    let body_sha256 = sha256_lower_hex(body);
    let canonical = CanonicalRequest {
        method: method.to_ascii_uppercase(),
        canonical_path_with_query: canonical_path_with_query_from_raw(path_with_query),
        timestamp: timestamp.to_owned(),
        nonce: nonce.to_owned(),
        body_sha256: body_sha256.clone(),
    };
    let signature = rustfs_transfer_common::crypto::sign_hmac_base64(secret, &canonical);
    let mut headers = HeaderMap::new();
    headers.insert(HEADER_EDGE_CODE, edge_code.parse()?);
    headers.insert(HEADER_AUTH_KEY_ID, auth_key_id.parse()?);
    headers.insert(HEADER_TIMESTAMP, timestamp.parse()?);
    headers.insert(HEADER_NONCE, nonce.parse()?);
    headers.insert(HEADER_BODY_SHA256, body_sha256.parse()?);
    headers.insert(HEADER_SIGNATURE, signature.parse()?);
    Ok(headers)
}

#[derive(Debug)]
pub struct AuthError {
    status: StatusCode,
    error_code: TransferErrorCode,
    message: &'static str,
    request_id: Uuid,
}

impl AuthError {
    fn unauthorized(
        request_id: Uuid,
        edge_code: Option<&str>,
        auth_key_id: Option<&str>,
        reason: &'static str,
    ) -> Self {
        warn!(
            request_id = %request_id,
            edge_code = edge_code.unwrap_or(""),
            auth_key_id = auth_key_id.unwrap_or(""),
            reject_reason = reason,
            "edge auth rejected"
        );
        Self {
            status: StatusCode::UNAUTHORIZED,
            error_code: TransferErrorCode::Unauthorized,
            message: "unauthorized",
            request_id,
        }
    }

    fn invalid_request(request_id: Uuid, reason: &'static str) -> Self {
        warn!(
            request_id = %request_id,
            reject_reason = reason,
            "edge auth request invalid"
        );
        Self {
            status: StatusCode::BAD_REQUEST,
            error_code: TransferErrorCode::InvalidRequest,
            message: "invalid request",
            request_id,
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(AuthErrorBody {
                error_code: self.error_code,
                message: self.message,
                request_id: self.request_id.to_string(),
            }),
        )
            .into_response()
    }
}

#[derive(Debug, Serialize)]
struct AuthErrorBody {
    error_code: TransferErrorCode,
    message: &'static str,
    request_id: String,
}

#[derive(Default)]
struct NonceCache {
    entries: Mutex<HashMap<String, DateTime<Utc>>>,
}

impl NonceCache {
    fn insert_once(&self, auth_key_id: &str, nonce: &str, now: DateTime<Utc>) -> bool {
        let mut entries = self.entries.lock().expect("nonce cache lock poisoned");
        entries.retain(|_, expires_at| *expires_at > now);
        let key = format!("{auth_key_id}:{nonce}");
        if entries.contains_key(&key) {
            return false;
        }
        entries.insert(key, now + Duration::seconds(NONCE_TTL_SECONDS));
        true
    }
}

fn required_header(headers: &HeaderMap, name: &str) -> Result<String, &'static str> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or("missing_required_header")
}

fn canonical_path_with_query(uri: &Uri) -> String {
    let query = uri.query().map(parse_query_params).unwrap_or_default();
    CanonicalRequest::new("GET", uri.path(), &query, "", "", b"").canonical_path_with_query
}

fn canonical_path_with_query_from_raw(path_with_query: &str) -> String {
    let (path, query) = path_with_query
        .split_once('?')
        .unwrap_or((path_with_query, ""));
    CanonicalRequest::new("GET", path, &parse_query_params(query), "", "", b"")
        .canonical_path_with_query
}

fn parse_query_params(query: &str) -> Vec<QueryParam> {
    if query.is_empty() {
        return Vec::new();
    }
    query
        .split('&')
        .map(|part| {
            let (name, value) = part.split_once('=').unwrap_or((part, ""));
            QueryParam::new(name, value)
        })
        .collect()
}

fn reject_edge_code_mismatch(
    request_id: Uuid,
    body: &[u8],
    expected_edge_code: &str,
) -> Result<(), AuthError> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| AuthError::invalid_request(request_id, "invalid_json"))?;
    if let Some(body_edge_code) = value.get("edge_code").and_then(|value| value.as_str()) {
        if body_edge_code != expected_edge_code {
            return Err(AuthError::invalid_request(request_id, "edge_code_mismatch"));
        }
    }
    Ok(())
}
