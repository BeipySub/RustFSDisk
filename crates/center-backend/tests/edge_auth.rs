use axum::{
    body::{to_bytes, Body},
    http::{HeaderMap, HeaderName, Method, Request, StatusCode},
};
use chrono::{Duration, Utc};
use rustfs_transfer_center::{
    center_auth::signed_headers, center_security::CenterSecurity, router, AppState, CenterService,
    DataKeyRecord, DataKeyStatus, DiskRecord, DiskStatusCode, EdgeRecord, MemoryCenterStore,
    PROTOCOL_VERSION,
};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

const EDGE_CODE: &str = "edge-a";
const AUTH_KEY_ID: &str = "auth-key-a";
const SECRET: &[u8] = b"edge-auth-secret";

fn state(edge_status: &str) -> AppState {
    AppState::new(CenterService::memory(store_with_edge(edge_status)))
}

fn store_with_edge(edge_status: &str) -> MemoryCenterStore {
    let mut store = MemoryCenterStore::default();
    store.edges.insert(
        EDGE_CODE.to_string(),
        EdgeRecord {
            edge_code: EDGE_CODE.to_string(),
            edge_name: "Edge A".to_string(),
            auth_key_id: AUTH_KEY_ID.to_string(),
            auth_secret: String::from_utf8(SECRET.to_vec()).unwrap(),
            edge_status: edge_status.to_string(),
        },
    );
    store
}

fn state_with_disk_and_key(edge_status: &str) -> (AppState, Uuid, Uuid, Uuid) {
    let mut store = store_with_edge(edge_status);
    let security = CenterSecurity::test();
    let disk_id = Uuid::new_v4();
    let data_key_id = Uuid::new_v4();
    let export_job_id = Uuid::new_v4();
    store.disks.insert(
        disk_id,
        DiskRecord {
            disk_id,
            sn: "SN-C3".to_string(),
            capacity_bytes: 1024,
            disk_enabled: true,
        },
    );
    store.disks_by_sn.insert("SN-C3".to_string(), disk_id);
    store.data_keys.insert(
        data_key_id,
        DataKeyRecord {
            data_key_id,
            disk_id,
            edge_code: None,
            export_job_id: None,
            encrypted_key: security
                .wrap_disk_data_key(disk_id, data_key_id, &[3_u8; 32])
                .unwrap(),
            status: DataKeyStatus::Active,
        },
    );

    (
        AppState::new(CenterService::memory(store)),
        disk_id,
        data_key_id,
        export_job_id,
    )
}

async fn post_edge_auth(
    state: AppState,
    body: &'static [u8],
    timestamp: String,
    nonce: &str,
    secret: &[u8],
) -> (StatusCode, Value) {
    let headers = signed_headers(
        Method::POST.as_str(),
        "/api/edge/auth",
        EDGE_CODE,
        AUTH_KEY_ID,
        &timestamp,
        nonce,
        body,
        secret,
    )
    .unwrap();
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/api/edge/auth")
        .body(Body::from(body))
        .unwrap();
    *request.headers_mut() = headers;

    let response = router(state).oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn post_json(
    state: AppState,
    path: &str,
    body: Vec<u8>,
    headers: Option<HeaderMap>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(headers) = headers {
        let request_headers = builder.headers_mut().unwrap();
        for (name, value) in headers {
            if let Some(name) = name {
                request_headers.insert(name, value);
            }
        }
    }

    let response = router(state)
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn verify_body(disk_id: Uuid) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "edge_code": EDGE_CODE,
        "disk_id": disk_id,
        "sn": "SN-C3",
        "capacity_bytes": 1024,
        "free_bytes": 512,
        "status_code": DiskStatusCode::Initialized,
        "protocol_version": PROTOCOL_VERSION
    }))
    .unwrap()
}

fn export_key_body(disk_id: Uuid, data_key_id: Uuid, export_job_id: Uuid) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "edge_code": EDGE_CODE,
        "disk_id": disk_id,
        "data_key_id": data_key_id,
        "export_job_id": export_job_id,
        "status_code": DiskStatusCode::Initialized
    }))
    .unwrap()
}

fn signed(
    method: Method,
    path: &str,
    body: &[u8],
    timestamp: String,
    nonce: &str,
    secret: &[u8],
) -> HeaderMap {
    signed_headers(
        method.as_str(),
        path,
        EDGE_CODE,
        AUTH_KEY_ID,
        &timestamp,
        nonce,
        body,
        secret,
    )
    .unwrap()
}

fn lowercase_header_names(headers: HeaderMap) -> HeaderMap {
    let mut lowered = HeaderMap::new();
    for (name, value) in headers {
        if let Some(name) = name {
            lowered.insert(
                HeaderName::from_bytes(name.as_str().to_ascii_lowercase().as_bytes()).unwrap(),
                value,
            );
        }
    }
    lowered
}

#[tokio::test]
async fn correct_signature_allows_edge_auth() {
    let nonce = format!("nonce-ok-{}", Uuid::new_v4());
    let (status, body) = post_edge_auth(
        state("ACTIVE"),
        br#"{"edge_code":"edge-a","client_version":"1.0.0"}"#,
        Utc::now().to_rfc3339(),
        &nonce,
        SECRET,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["allowed"], true);
    assert_eq!(body["edge_code"], EDGE_CODE);
    assert_eq!(body["edge_status"], "ACTIVE");
    assert!(body.get("status").is_none());
}

#[tokio::test]
async fn bad_signature_returns_unauthorized() {
    let nonce = format!("nonce-bad-signature-{}", Uuid::new_v4());
    let (status, body) = post_edge_auth(
        state("ACTIVE"),
        br#"{"edge_code":"edge-a"}"#,
        Utc::now().to_rfc3339(),
        &nonce,
        b"wrong-secret",
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error_code"], "UNAUTHORIZED");
}

#[tokio::test]
async fn timestamp_skew_returns_unauthorized() {
    let nonce = format!("nonce-skew-{}", Uuid::new_v4());
    let (status, body) = post_edge_auth(
        state("ACTIVE"),
        br#"{"edge_code":"edge-a"}"#,
        (Utc::now() - Duration::seconds(301)).to_rfc3339(),
        &nonce,
        SECRET,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error_code"], "UNAUTHORIZED");
}

#[tokio::test]
async fn replayed_nonce_returns_unauthorized() {
    let state = state("ACTIVE");
    let timestamp = Utc::now().to_rfc3339();
    let nonce = format!("nonce-replay-{}", Uuid::new_v4());
    let first = post_edge_auth(
        state.clone(),
        br#"{"edge_code":"edge-a"}"#,
        timestamp.clone(),
        &nonce,
        SECRET,
    )
    .await;
    let second = post_edge_auth(
        state,
        br#"{"edge_code":"edge-a"}"#,
        timestamp,
        &nonce,
        SECRET,
    )
    .await;

    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(second.0, StatusCode::UNAUTHORIZED);
    assert_eq!(second.1["error_code"], "UNAUTHORIZED");
}

#[tokio::test]
async fn disabled_edge_returns_unauthorized() {
    let nonce = format!("nonce-disabled-{}", Uuid::new_v4());
    let (status, body) = post_edge_auth(
        state("DISABLED"),
        br#"{"edge_code":"edge-a"}"#,
        Utc::now().to_rfc3339(),
        &nonce,
        SECRET,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error_code"], "UNAUTHORIZED");
}

#[tokio::test]
async fn body_edge_code_mismatch_returns_invalid_request() {
    let nonce = format!("nonce-mismatch-{}", Uuid::new_v4());
    let (status, body) = post_edge_auth(
        state("ACTIVE"),
        br#"{"edge_code":"edge-b"}"#,
        Utc::now().to_rfc3339(),
        &nonce,
        SECRET,
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error_code"], "INVALID_REQUEST");
}

#[tokio::test]
async fn health_and_ready_are_available() {
    for path in ["/healthz", "/readyz"] {
        let response = router(state("ACTIVE"))
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn disk_verify_requires_hmac_headers() {
    let (state, disk_id, _, _) = state_with_disk_and_key("ACTIVE");
    let (status, body) = post_json(state, "/api/disk/verify", verify_body(disk_id), None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error_code"], "UNAUTHORIZED");
    assert!(body.get("disk_data_key").is_none());
}

#[tokio::test]
async fn disk_verify_accepts_correct_hmac_signature() {
    let (state, disk_id, _, _) = state_with_disk_and_key("ACTIVE");
    let body = verify_body(disk_id);
    let headers = signed(
        Method::POST,
        "/api/disk/verify",
        &body,
        Utc::now().to_rfc3339(),
        &format!("nonce-verify-ok-{}", Uuid::new_v4()),
        SECRET,
    );

    let (status, body) = post_json(state, "/api/disk/verify", body, Some(headers)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["allowed"], true);
    assert_eq!(body["action"], "ALLOW_EXPORT");
    assert!(body.get("status").is_none());
}

#[tokio::test]
async fn disk_verify_accepts_lowercase_hmac_header_names() {
    let (state, disk_id, _, _) = state_with_disk_and_key("ACTIVE");
    let body = verify_body(disk_id);
    let headers = lowercase_header_names(signed(
        Method::POST,
        "/api/disk/verify",
        &body,
        Utc::now().to_rfc3339(),
        &format!("nonce-verify-lowercase-{}", Uuid::new_v4()),
        SECRET,
    ));

    let (status, body) = post_json(state, "/api/disk/verify", body, Some(headers)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["allowed"], true);
}

#[tokio::test]
async fn disk_verify_rejects_bad_timestamp_replay_body_path_and_method_signature() {
    let (state, disk_id, _, _) = state_with_disk_and_key("ACTIVE");
    let body = verify_body(disk_id);

    let bad_secret = signed(
        Method::POST,
        "/api/disk/verify",
        &body,
        Utc::now().to_rfc3339(),
        &format!("nonce-verify-bad-secret-{}", Uuid::new_v4()),
        b"wrong-secret",
    );
    let (status, response) = post_json(
        state.clone(),
        "/api/disk/verify",
        body.clone(),
        Some(bad_secret),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(response["error_code"], "UNAUTHORIZED");

    let stale = signed(
        Method::POST,
        "/api/disk/verify",
        &body,
        (Utc::now() - Duration::seconds(301)).to_rfc3339(),
        &format!("nonce-verify-stale-{}", Uuid::new_v4()),
        SECRET,
    );
    let (status, _) = post_json(state.clone(), "/api/disk/verify", body.clone(), Some(stale)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let replay_nonce = format!("nonce-verify-replay-{}", Uuid::new_v4());
    let replay_headers = signed(
        Method::POST,
        "/api/disk/verify",
        &body,
        Utc::now().to_rfc3339(),
        &replay_nonce,
        SECRET,
    );
    let first = post_json(
        state.clone(),
        "/api/disk/verify",
        body.clone(),
        Some(replay_headers.clone()),
    )
    .await;
    let second = post_json(
        state.clone(),
        "/api/disk/verify",
        body.clone(),
        Some(replay_headers),
    )
    .await;
    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(second.0, StatusCode::UNAUTHORIZED);

    let signed_for_other_body = signed(
        Method::POST,
        "/api/disk/verify",
        br#"{"edge_code":"edge-a"}"#,
        Utc::now().to_rfc3339(),
        &format!("nonce-verify-body-{}", Uuid::new_v4()),
        SECRET,
    );
    let (status, _) = post_json(
        state.clone(),
        "/api/disk/verify",
        body.clone(),
        Some(signed_for_other_body),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let signed_for_other_path = signed(
        Method::POST,
        "/api/disk/verify?unexpected=1",
        &body,
        Utc::now().to_rfc3339(),
        &format!("nonce-verify-path-{}", Uuid::new_v4()),
        SECRET,
    );
    let (status, _) = post_json(
        state.clone(),
        "/api/disk/verify",
        body.clone(),
        Some(signed_for_other_path),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let signed_for_get = signed(
        Method::GET,
        "/api/disk/verify",
        &body,
        Utc::now().to_rfc3339(),
        &format!("nonce-verify-method-{}", Uuid::new_v4()),
        SECRET,
    );
    let (status, _) = post_json(state, "/api/disk/verify", body, Some(signed_for_get)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn disk_verify_rejects_disabled_edge_before_business_logic() {
    let (state, disk_id, _, _) = state_with_disk_and_key("DISABLED");
    let body = verify_body(disk_id);
    let headers = signed(
        Method::POST,
        "/api/disk/verify",
        &body,
        Utc::now().to_rfc3339(),
        &format!("nonce-verify-disabled-{}", Uuid::new_v4()),
        SECRET,
    );

    let (status, body) = post_json(state, "/api/disk/verify", body, Some(headers)).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error_code"], "UNAUTHORIZED");
}

#[tokio::test]
async fn export_key_requires_hmac_and_does_not_leak_key_on_auth_failure() {
    let (state, disk_id, data_key_id, export_job_id) = state_with_disk_and_key("ACTIVE");
    let body = export_key_body(disk_id, data_key_id, export_job_id);

    let (status, response) =
        post_json(state.clone(), "/api/disk/export-key", body.clone(), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(response["error_code"], "UNAUTHORIZED");
    assert!(response.get("disk_data_key").is_none());

    let bad_headers = signed(
        Method::POST,
        "/api/disk/export-key",
        &body,
        Utc::now().to_rfc3339(),
        &format!("nonce-export-bad-{}", Uuid::new_v4()),
        b"wrong-secret",
    );
    let (status, response) =
        post_json(state, "/api/disk/export-key", body, Some(bad_headers)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(response["error_code"], "UNAUTHORIZED");
    assert!(response.get("disk_data_key").is_none());
}

#[tokio::test]
async fn export_key_accepts_correct_hmac_and_omits_expires_at() {
    let (state, disk_id, data_key_id, export_job_id) = state_with_disk_and_key("ACTIVE");
    let body = export_key_body(disk_id, data_key_id, export_job_id);
    let headers = signed(
        Method::POST,
        "/api/disk/export-key",
        &body,
        Utc::now().to_rfc3339(),
        &format!("nonce-export-ok-{}", Uuid::new_v4()),
        SECRET,
    );

    let (status, response) = post_json(state, "/api/disk/export-key", body, Some(headers)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["allowed"], true);
    assert!(response.get("disk_data_key").is_some());
    assert!(response.get("expires_at").is_none());
}
