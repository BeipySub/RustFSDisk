use anyhow::Context;
use axum::http::Uri;
use rustfs_transfer_edge::EdgeConfig;
use serde_json::json;
use std::{
    env,
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

fn main() -> anyhow::Result<()> {
    let args = Args::parse()?;
    let config = EdgeConfig::from_env().context("load edge config from environment")?;
    let token = config
        .rescan_token()
        .context("RUSTFS_TRANSFER__RESCAN__TOKEN or [rescan].token_env must be configured")?;
    let endpoint_url = config.rescan_endpoint_url();
    let body = serde_json::to_vec(&json!({
        "trigger": args.trigger,
        "device": args.device,
    }))?;

    let response = post_rescan(&endpoint_url, token, &body)
        .with_context(|| format!("notify edge daemon rescan endpoint {endpoint_url}"))?;
    println!("{response}");
    Ok(())
}

struct Args {
    device: Option<String>,
    trigger: String,
}

impl Args {
    fn parse() -> anyhow::Result<Self> {
        let mut device = None;
        let mut trigger = "udev".to_owned();
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--device" => {
                    device = args.next();
                }
                "--trigger" => {
                    trigger = args.next().unwrap_or_else(|| "manual".to_owned());
                }
                "--help" | "-h" => {
                    println!("Usage: rustfs-transfer-rescan [--device /dev/sdX] [--trigger udev]");
                    std::process::exit(0);
                }
                other => anyhow::bail!("unknown argument {other}"),
            }
        }
        Ok(Self { device, trigger })
    }
}

fn post_rescan(endpoint_url: &str, token: &str, body: &[u8]) -> anyhow::Result<String> {
    let uri: Uri = endpoint_url.parse()?;
    if uri.scheme_str() != Some("http") {
        anyhow::bail!("rescan endpoint must use http loopback");
    }
    let host = uri.host().context("rescan endpoint missing host")?;
    if host != "127.0.0.1" && host != "localhost" {
        anyhow::bail!("rescan endpoint must target loopback");
    }
    let port = uri.port_u16().unwrap_or(80);
    let authority = uri.authority().map(|value| value.as_str()).unwrap_or(host);
    let path = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");

    let mut stream = TcpStream::connect((host, port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nContent-Type: application/json\r\nX-Rescan-Token: {token}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .context("malformed HTTP response")?;
    let headers = String::from_utf8_lossy(&response[..header_end]);
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .context("malformed HTTP status line")?;
    let body = String::from_utf8_lossy(&response[(header_end + 4)..]).to_string();
    if !(200..300).contains(&status) {
        anyhow::bail!("edge daemon returned HTTP {status}: {body}");
    }
    Ok(body)
}
