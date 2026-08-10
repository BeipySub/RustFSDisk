use crate::disk_detection::{
    CenterDiskVerifier, DiskDetectionError, DiskVerifyRequest, DiskVerifyResponse, VerifyAction,
};
use axum::http::{header::CONTENT_TYPE, HeaderMap, HeaderValue, Method, Request, Uri};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{SecondsFormat, Utc};
use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

const QUERY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

#[derive(Debug, Clone)]
pub struct CenterHmacClient {
    base_url: String,
    edge_code: String,
    auth_key_id: String,
    edge_auth_secret: String,
}

#[derive(Debug, Clone)]
pub struct SignedRequestParts {
    pub body_sha256: String,
    pub canonical_path_with_query: String,
    pub string_to_sign: String,
    pub signature: String,
    pub timestamp: String,
    pub nonce: String,
}

impl CenterHmacClient {
    pub fn new(
        base_url: impl Into<String>,
        edge_code: impl Into<String>,
        auth_key_id: impl Into<String>,
        edge_auth_secret: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            edge_code: edge_code.into(),
            auth_key_id: auth_key_id.into(),
            edge_auth_secret: edge_auth_secret.into(),
        }
    }

    pub fn signed_json_request<T: Serialize>(
        &self,
        method: Method,
        path_with_query: &str,
        body: &T,
    ) -> anyhow::Result<Request<Vec<u8>>> {
        let body = serde_json::to_vec(body)?;
        let parts = self.sign(method.as_str(), path_with_query, &body);
        let uri: Uri = format!("{}{}", self.base_url, path_with_query).parse()?;
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header(CONTENT_TYPE, "application/json")
            .body(body)?;
        self.apply_headers(request.headers_mut(), &parts)?;
        Ok(request)
    }

    pub fn sign(&self, method: &str, path_with_query: &str, body: &[u8]) -> SignedRequestParts {
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let nonce = Uuid::new_v4().to_string();
        self.sign_with_nonce(method, path_with_query, body, timestamp, nonce)
    }

    pub fn sign_with_nonce(
        &self,
        method: &str,
        path_with_query: &str,
        body: &[u8],
        timestamp: String,
        nonce: String,
    ) -> SignedRequestParts {
        let body_sha256 = hex::encode(Sha256::digest(body));
        let canonical_path_with_query = canonical_path_with_query(path_with_query);
        let string_to_sign = format!(
            "{}\n{}\n{}\n{}\n{}",
            method.to_ascii_uppercase(),
            canonical_path_with_query,
            timestamp,
            nonce,
            body_sha256
        );
        let mut mac = HmacSha256::new_from_slice(self.edge_auth_secret.as_bytes())
            .expect("HMAC accepts any key length");
        mac.update(string_to_sign.as_bytes());
        let signature = STANDARD.encode(mac.finalize().into_bytes());

        SignedRequestParts {
            body_sha256,
            canonical_path_with_query,
            string_to_sign,
            signature,
            timestamp,
            nonce,
        }
    }

    fn apply_headers(
        &self,
        headers: &mut HeaderMap,
        parts: &SignedRequestParts,
    ) -> anyhow::Result<()> {
        headers.insert("X-Edge-Code", HeaderValue::from_str(&self.edge_code)?);
        headers.insert("X-Auth-Key-Id", HeaderValue::from_str(&self.auth_key_id)?);
        headers.insert("X-Timestamp", HeaderValue::from_str(&parts.timestamp)?);
        headers.insert("X-Nonce", HeaderValue::from_str(&parts.nonce)?);
        headers.insert("X-Body-SHA256", HeaderValue::from_str(&parts.body_sha256)?);
        headers.insert("X-Signature", HeaderValue::from_str(&parts.signature)?);
        Ok(())
    }

    pub async fn export_key(&self, request: ExportKeyRequest) -> anyhow::Result<ExportKeyResponse> {
        let http_request =
            self.signed_json_request(Method::POST, "/api/disk/export-key", &request)?;
        let response_body = send_http_request(http_request).await?;
        Ok(serde_json::from_slice(&response_body)?)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportKeyRequest {
    pub edge_code: String,
    pub disk_id: Uuid,
    pub data_key_id: Uuid,
    pub export_job_id: Uuid,
    pub status_code: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportKeyResponse {
    pub allowed: bool,
    pub data_key_id: Uuid,
    pub encryption_alg: String,
    pub disk_data_key: Option<String>,
    pub message: Option<String>,
}

impl CenterDiskVerifier for CenterHmacClient {
    fn verify_disk<'a>(
        &'a self,
        request: DiskVerifyRequest,
    ) -> crate::disk_detection::BoxFuture<'a, Result<DiskVerifyResponse, DiskDetectionError>> {
        Box::pin(async move {
            let http_request = self
                .signed_json_request(Method::POST, "/api/disk/verify", &request)
                .map_err(|err| DiskDetectionError::CenterVerify(err.to_string()))?;
            let response_body = send_http_request(http_request)
                .await
                .map_err(|err| DiskDetectionError::CenterVerify(err.to_string()))?;
            let response: DiskVerifyWire = serde_json::from_slice(&response_body)
                .map_err(|err| DiskDetectionError::CenterVerify(err.to_string()))?;
            response.try_into()
        })
    }
}

#[derive(Debug, Deserialize)]
struct DiskVerifyWire {
    allowed: bool,
    disk_id: String,
    disk_enabled: bool,
    expected_status: String,
    action: String,
    message: Option<String>,
}

impl TryFrom<DiskVerifyWire> for DiskVerifyResponse {
    type Error = DiskDetectionError;

    fn try_from(value: DiskVerifyWire) -> Result<Self, Self::Error> {
        let action = match value.action.as_str() {
            "ALLOW_EXPORT" => VerifyAction::AllowExport,
            "REJECT" => VerifyAction::Reject,
            "NEED_INIT" => VerifyAction::NeedInit,
            "NEED_IMPORT_FIRST" => VerifyAction::NeedImportFirst,
            other => {
                return Err(DiskDetectionError::CenterVerify(format!(
                    "unknown verify action {other}"
                )));
            }
        };

        Ok(Self {
            allowed: value.allowed,
            disk_id: value.disk_id,
            disk_enabled: value.disk_enabled,
            expected_status: value.expected_status,
            action,
            message: value.message,
        })
    }
}

async fn send_http_request(request: Request<Vec<u8>>) -> anyhow::Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || send_http_request_blocking(request)).await?
}

fn send_http_request_blocking(request: Request<Vec<u8>>) -> anyhow::Result<Vec<u8>> {
    let uri = request.uri();
    if uri.scheme_str() != Some("http") {
        anyhow::bail!("only http center base_url is supported by the edge verifier");
    }
    let host = uri.host().ok_or_else(|| anyhow::anyhow!("missing host"))?;
    let port = uri.port_u16().unwrap_or(80);
    let authority = uri.authority().map(|value| value.as_str()).unwrap_or(host);
    let path = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");

    let mut stream = TcpStream::connect((host, port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    write!(
        stream,
        "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n",
        request.method(),
        path,
        authority,
        request.body().len()
    )?;
    for (name, value) in request.headers() {
        write!(stream, "{}: {}\r\n", name.as_str(), value.to_str()?)?;
    }
    stream.write_all(b"\r\n")?;
    stream.write_all(request.body())?;
    stream.flush()?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| anyhow::anyhow!("malformed HTTP response"))?;
    let headers = String::from_utf8_lossy(&response[..header_end]);
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| anyhow::anyhow!("malformed HTTP status line"))?;
    let body = response[(header_end + 4)..].to_vec();
    if !(200..300).contains(&status) {
        anyhow::bail!(
            "center request returned HTTP {status}: {}",
            String::from_utf8_lossy(&body)
        );
    }
    Ok(body)
}

fn canonical_path_with_query(path_with_query: &str) -> String {
    let (path, query) = path_with_query
        .split_once('?')
        .unwrap_or((path_with_query, ""));
    let path = if path.is_empty() { "/" } else { path };
    if query.is_empty() {
        return path.to_owned();
    }

    let mut pairs = query
        .split('&')
        .map(|part| part.split_once('=').unwrap_or((part, "")))
        .collect::<Vec<_>>();
    pairs.sort_by(|(left_key, left_value), (right_key, right_value)| {
        left_key.cmp(right_key).then(left_value.cmp(right_value))
    });
    let query = pairs
        .into_iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                utf8_percent_encode(key, QUERY_ENCODE_SET),
                utf8_percent_encode(value, QUERY_ENCODE_SET)
            )
        })
        .collect::<Vec<_>>()
        .join("&");

    format!("{path}?{query}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Method;
    use rustfs_transfer_common::crypto::{sha256_lower_hex, sign_hmac_base64, CanonicalRequest};

    #[test]
    fn signs_canonical_request_with_sorted_query() {
        let client = CenterHmacClient::new(
            "http://center.local",
            "edge-a",
            "auth-key",
            "example-dev-secret",
        );

        let signed = client.sign_with_nonce(
            "post",
            "/api/disk/verify?z=last&a=space value&a=first",
            br#"{"edge_code":"edge-a"}"#,
            "2026-08-09T00:00:00Z".to_owned(),
            "nonce-1".to_owned(),
        );

        assert_eq!(
            signed.canonical_path_with_query,
            "/api/disk/verify?a=first&a=space%20value&z=last"
        );
        assert!(signed.string_to_sign.starts_with("POST\n/api/disk/verify?"));
        assert!(!signed.signature.is_empty());
    }

    #[test]
    fn signed_json_request_carries_required_hmac_headers() {
        let client = CenterHmacClient::new(
            "http://center.local",
            "edge-a",
            "auth-key",
            "example-dev-secret",
        );
        let request = client
            .signed_json_request(Method::POST, "/api/edge/auth", &serde_json::json!({}))
            .expect("request builds");

        assert_eq!(request.headers()["X-Edge-Code"], "edge-a");
        assert_eq!(request.headers()["X-Auth-Key-Id"], "auth-key");
        assert!(request.headers().contains_key("X-Signature"));
        assert!(request.headers().contains_key("X-Body-SHA256"));
    }

    #[test]
    fn signatures_for_verify_and_export_key_match_shared_canonical_hmac() {
        let client = CenterHmacClient::new(
            "http://center.local",
            "edge-a",
            "auth-key",
            "example-dev-secret",
        );
        let body = br#"{"edge_code":"edge-a"}"#;

        for path in ["/api/disk/verify", "/api/disk/export-key"] {
            let signed = client.sign_with_nonce(
                "POST",
                path,
                body,
                "2026-08-10T00:00:00Z".to_owned(),
                format!("nonce-{path}"),
            );
            let expected_body_sha256 = sha256_lower_hex(body);
            let canonical = CanonicalRequest {
                method: "POST".to_string(),
                canonical_path_with_query: path.to_string(),
                timestamp: "2026-08-10T00:00:00Z".to_string(),
                nonce: format!("nonce-{path}"),
                body_sha256: expected_body_sha256.clone(),
            };

            assert_eq!(signed.body_sha256, expected_body_sha256);
            assert_eq!(signed.canonical_path_with_query, path);
            assert_eq!(
                signed.signature,
                sign_hmac_base64(b"example-dev-secret", &canonical)
            );
        }
    }
}
