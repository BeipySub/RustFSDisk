$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
$localDir = Join-Path $repo ".local/dev"
$diskRoot = Join-Path $localDir "mock-disks"
$configDir = Join-Path $localDir "config"
$logDir = Join-Path $localDir "logs"
$dataDir = Join-Path $localDir "data"

$dirs = @(
    "$diskRoot/initialized/rustfs-transfer/data",
    "$diskRoot/initialized/rustfs-transfer/meta",
    "$diskRoot/initialized/rustfs-transfer/manifests",
    "$diskRoot/initialized/rustfs-transfer/logs",
    "$diskRoot/initialized/rustfs-transfer/quarantine/partial",
    "$diskRoot/sealed/rustfs-transfer/data",
    "$diskRoot/sealed/rustfs-transfer/meta",
    "$diskRoot/sealed/rustfs-transfer/manifests",
    "$diskRoot/sealed/rustfs-transfer/logs",
    "$diskRoot/sealed/rustfs-transfer/quarantine/partial",
    "$configDir",
    "$logDir/center",
    "$logDir/edge",
    "$dataDir/center",
    "$dataDir/edge"
)

foreach ($dir in $dirs) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
}

Copy-Item -Force (Join-Path $repo "deploy/config/center.example.toml") (Join-Path $configDir "center.toml")

@'
{
  "protocol": { "version": "1.0" },
  "disk": {
    "disk_id": "mock-disk-initialized",
    "sn": "MOCK-SN-INITIALIZED",
    "capacity_bytes": 107374182400
  },
  "status": {
    "code": "INITIALIZED",
    "sealed": false,
    "imported": false,
    "reusable": true,
    "last_error": null
  },
  "security": {
    "center_key_id": "mock-center-key-id",
    "data_key_id": "mock-data-key-id",
    "encryption_alg": "AES-256-GCM",
    "signature_alg": "HMAC-SHA256"
  }
}
'@ | Set-Content -Encoding UTF8 (Join-Path $diskRoot "initialized/rustfs-transfer/disk_info.json")

@'
{
  "protocol": { "version": "1.0" },
  "disk": {
    "disk_id": "mock-disk-sealed",
    "sn": "MOCK-SN-SEALED",
    "capacity_bytes": 107374182400
  },
  "status": {
    "code": "SEALED",
    "sealed": true,
    "imported": false,
    "reusable": false,
    "last_error": null
  },
  "security": {
    "center_key_id": "mock-center-key-id",
    "data_key_id": "mock-data-key-id",
    "encryption_alg": "AES-256-GCM",
    "signature_alg": "HMAC-SHA256"
  },
  "edge": {
    "edge_code": "edge-demo",
    "seal_id": "mock-seal-id"
  }
}
'@ | Set-Content -Encoding UTF8 (Join-Path $diskRoot "sealed/rustfs-transfer/disk_info.json")

@'
{
  "protocol_version": "1.0",
  "disk_id": "mock-disk-sealed",
  "seal_id": "mock-seal-id",
  "edge_code": "edge-demo",
  "objects": []
}
'@ | Set-Content -Encoding UTF8 (Join-Path $diskRoot "sealed/rustfs-transfer/manifests/export_manifest.json")

"sha256-not-calculated-in-day1-fixture  export_manifest.json" | Set-Content -Encoding UTF8 (Join-Path $diskRoot "sealed/rustfs-transfer/manifests/export_manifest.sha256")

@'
CENTER_BIND=0.0.0.0:8080
CENTER_ID=00000000-0000-0000-0000-000000000001
CENTER_NAME=RustFS Transfer Center
PROTOCOL_VERSION=1.0
CONTROL_API_TOKEN=CHANGE_ME_CENTER_CONTROL_API_TOKEN
DATABASE_URL=postgres://rustfs_transfer_center:CHANGE_ME_PASSWORD@127.0.0.1:5432/rustfs_transfer_center_dev
RUSTFS_ENDPOINT=http://127.0.0.1:9000
RUSTFS_ACCESS_KEY=CHANGE_ME_ACCESS_KEY
RUSTFS_SECRET_KEY=CHANGE_ME_SECRET_KEY
DATA_DIR=.local/dev/data/center
LOG_DIR=.local/dev/logs/center
TRANSPORT_MOUNT_ROOT=.local/dev/mock-disks
DISK_POLLING_ENABLED=true
DISK_POLLING_INTERVAL_SECONDS=1
LOCAL_MASTER_KEY=CHANGE_ME_32_BYTE_BASE64_OR_HEX_KEY
CENTER_SIGNATURE_KEY=CHANGE_ME_32_BYTE_BASE64_OR_HEX_KEY
'@ | Set-Content -Encoding UTF8 (Join-Path $localDir "center.env")

@'
EDGE_BIND=0.0.0.0:8081
DATABASE_URL=postgres://rustfs_transfer_edge:CHANGE_ME_PASSWORD@127.0.0.1:5432/rustfs_transfer_edge_dev
EDGE_CODE=edge-demo
EDGE_KEY=CHANGE_ME_EDGE_KEY_FROM_CENTER
RUSTFS_ENDPOINT=http://127.0.0.1:9000
RUSTFS_ACCESS_KEY=CHANGE_ME_ACCESS_KEY
RUSTFS_SECRET_KEY=CHANGE_ME_SECRET_KEY
DATA_DIR=.local/dev/data/edge
LOG_DIR=.local/dev/logs/edge
TRANSPORT_MOUNT_ROOT=.local/dev/mock-disks
DISK_POLLING_ENABLED=true
DISK_POLLING_INTERVAL_SECONDS=1
'@ | Set-Content -Encoding UTF8 (Join-Path $localDir "edge.env")

Write-Host "Prepared local RustFS Transfer layout:"
Write-Host "  $localDir"
Write-Host ""
Write-Host "Mock transport disks:"
Write-Host "  $diskRoot/initialized"
Write-Host "  $diskRoot/sealed"
