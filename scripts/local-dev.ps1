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
Copy-Item -Force (Join-Path $repo "deploy/config/edge.example.toml") (Join-Path $configDir "edge.toml")

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
RUSTFS_TRANSFER__CONFIG_PATH=.local/dev/config/center.toml
RUSTFS_TRANSFER__DATABASE__URL=postgres://rustfs_transfer_center:CHANGE_ME_PASSWORD@127.0.0.1:5432/rustfs_transfer_center_dev
RUSTFS_TRANSFER__PATHS__DATA_DIR=.local/dev/data/center
RUSTFS_TRANSFER__PATHS__LOG_DIR=.local/dev/logs/center
RUSTFS_TRANSFER__PATHS__TRANSPORT_MOUNT_ROOT=.local/dev/mock-disks
'@ | Set-Content -Encoding UTF8 (Join-Path $localDir "center.env")

@'
RUSTFS_TRANSFER__CONFIG_PATH=.local/dev/config/edge.toml
RUSTFS_TRANSFER__DATABASE__URL=postgres://rustfs_transfer_edge:CHANGE_ME_PASSWORD@127.0.0.1:5432/rustfs_transfer_edge_dev
# Compatibility value for local legacy paths only; offline export must not
# depend on Center reachability.
RUSTFS_TRANSFER__CENTER__BASE_URL=http://127.0.0.1:8080
RUSTFS_TRANSFER__CENTER__EDGE_CODE=edge-demo
RUSTFS_TRANSFER__PATHS__DATA_DIR=.local/dev/data/edge
RUSTFS_TRANSFER__PATHS__LOG_DIR=.local/dev/logs/edge
RUSTFS_TRANSFER__PATHS__TRANSPORT_MOUNT_ROOT=.local/dev/mock-disks
'@ | Set-Content -Encoding UTF8 (Join-Path $localDir "edge.env")

Write-Host "Prepared local RustFS Transfer layout:"
Write-Host "  $localDir"
Write-Host ""
Write-Host "Mock transport disks:"
Write-Host "  $diskRoot/initialized"
Write-Host "  $diskRoot/sealed"
