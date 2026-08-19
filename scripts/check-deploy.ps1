$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
$failures = New-Object System.Collections.Generic.List[string]

function Add-Failure($message) {
    $failures.Add($message) | Out-Null
}

$serviceFiles = @(
    "deploy/systemd/rustfs-transfer-center.service",
    "deploy/systemd/rustfs-transfer-edge.service"
)

foreach ($file in $serviceFiles) {
    $path = Join-Path $repo $file
    $content = Get-Content -Raw $path
    if ($content -notmatch "ExecStart=/opt/rustfs-transfer/") {
        Add-Failure "$file ExecStart must use /opt/rustfs-transfer/"
    }
    if ($content -notmatch "EnvironmentFile=-/etc/rustfs-transfer/") {
        Add-Failure "$file must set or load rustfs-transfer environment"
    }
}

$centerService = Get-Content -Raw (Join-Path $repo "deploy/systemd/rustfs-transfer-center.service")
if ($centerService -notmatch "EnvironmentFile=-/etc/rustfs-transfer/center.env") {
    Add-Failure "center service must load center.env for security key injection"
}
if ($centerService -notmatch "UMask=0000") {
    Add-Failure "center service must use UMask=0000 for shared transport-disk access"
}

$edgeService = Get-Content -Raw (Join-Path $repo "deploy/systemd/rustfs-transfer-edge.service")
if ($edgeService -notmatch "ExecStart=/opt/rustfs-transfer/rustfs-transfer-edge") {
    Add-Failure "edge service ExecStart must use /opt/rustfs-transfer/rustfs-transfer-edge"
}
if ($edgeService -notmatch "EnvironmentFile=-/etc/rustfs-transfer/edge.env") {
    Add-Failure "edge service must load edge.env for complete runtime configuration"
}
if ($edgeService -match "edge\.toml|RUSTFS_TRANSFER__CONFIG_PATH") {
    Add-Failure "edge service must not depend on edge.toml or CONFIG_PATH"
}
if ($edgeService -notmatch "UMask=0000") {
    Add-Failure "edge service must use UMask=0000 for shared transport-disk access"
}

$centerToml = Get-Content -Raw (Join-Path $repo "deploy/config/center.example.toml")
if ($centerToml -notmatch "control_api_token_env\s*=\s*`"CONTROL_API_TOKEN`"") {
    Add-Failure "center config example must reference CENTER CONTROL API token env"
}
if ($centerToml -notmatch "local_master_key_env\s*=\s*`"LOCAL_MASTER_KEY`"") {
    Add-Failure "center config example must reference LOCAL_MASTER_KEY env"
}
if ($centerToml -notmatch "center_signature_key_env\s*=\s*`"CENTER_SIGNATURE_KEY`"") {
    Add-Failure "center config example must reference CENTER_SIGNATURE_KEY env"
}

$centerEnv = Get-Content -Raw (Join-Path $repo "deploy/config/center.env.example")
if ($centerEnv -match "RUSTFS_TRANSFER__|center\.toml|CENTER_CONFIG_PATH") {
    Add-Failure "center.env.example must use short env fields and must not require center.toml"
}
if ($centerEnv -notmatch "CONTROL_API_TOKEN=CHANGE_ME_CENTER_CONTROL_API_TOKEN") {
    Add-Failure "center.env.example must include CENTER CONTROL API token placeholder"
}
if ($centerEnv -notmatch "LOCAL_MASTER_KEY=CHANGE_ME_32_BYTE_BASE64_OR_HEX_KEY") {
    Add-Failure "center.env.example must include LOCAL_MASTER_KEY placeholder"
}
if ($centerEnv -notmatch "CENTER_SIGNATURE_KEY=CHANGE_ME_32_BYTE_BASE64_OR_HEX_KEY") {
    Add-Failure "center.env.example must include CENTER_SIGNATURE_KEY placeholder"
}
if ($centerEnv -notmatch "DISK_POLLING_ENABLED=true") {
    Add-Failure "center.env.example must enable built-in disk polling by default"
}
if ($centerEnv -notmatch "DISK_POLLING_INTERVAL_SECONDS=1") {
    Add-Failure "center.env.example must include the disk polling interval"
}

if (Test-Path (Join-Path $repo "deploy/config/edge.example.toml")) {
    Add-Failure "edge.example.toml must be removed; Edge runtime configuration is edge.env only"
}

$edgeEnv = Get-Content -Raw (Join-Path $repo "deploy/config/edge.env.example")
if ($edgeEnv -match "RUSTFS_TRANSFER__|edge\.toml") {
    Add-Failure "edge.env.example must not reference edge.toml or CONFIG_PATH"
}
if ($edgeEnv -notmatch "EDGE_BIND=0.0.0.0:8081") {
    Add-Failure "edge.env.example must include the Edge server bind address"
}
if ($edgeEnv -notmatch "DATABASE_URL=postgres://rustfs_transfer_edge:CHANGE_ME_PASSWORD@127.0.0.1:5432/rustfs_transfer_edge") {
    Add-Failure "edge.env.example must include the database URL placeholder"
}
if ($edgeEnv -notmatch "EDGE_CODE=edge-demo") {
    Add-Failure "edge.env.example must include the Edge code placeholder"
}
if ($edgeEnv -notmatch "EDGE_KEY=CHANGE_ME_EDGE_KEY_FROM_CENTER") {
    Add-Failure "edge.env.example must include the Center generated EDGE_KEY placeholder"
}
if ($edgeEnv -notmatch "RUSTFS_ENDPOINT=http://127.0.0.1:9000") {
    Add-Failure "edge.env.example must include RustFS endpoint"
}
if ($edgeEnv -notmatch "RUSTFS_ACCESS_KEY=CHANGE_ME_ACCESS_KEY") {
    Add-Failure "edge.env.example must include RustFS access key placeholder"
}
if ($edgeEnv -notmatch "RUSTFS_SECRET_KEY=CHANGE_ME_SECRET_KEY") {
    Add-Failure "edge.env.example must include RustFS secret key placeholder"
}
if ($edgeEnv -notmatch "DISK_MOUNT_ROOTS=/mnt/rustfs-transfer,/media,/run/media") {
    Add-Failure "edge.env.example must include explicit Edge disk mount roots"
}
if ($edgeEnv -notmatch "DISK_POLLING_ENABLED=true") {
    Add-Failure "edge.env.example must enable built-in disk polling by default"
}
if ($edgeEnv -notmatch "DISK_POLLING_INTERVAL_SECONDS=1") {
    Add-Failure "edge.env.example must include the disk polling interval"
}
if ($edgeEnv -match "RUSTFS_TRANSFER__CENTER__|RUSTFS_TRANSFER__SERVER__CONTROL_API_TOKEN|EDGE_AUTH_SECRET|AUTH_KEY_ID") {
    Add-Failure "edge.env.example must not include legacy Center auth fields or Edge control token"
}
if ($edgeEnv -notmatch "AUTO_EXPORT_ENABLED=false") {
    Add-Failure "edge.env.example must keep auto export disabled by default"
}
if ($edgeEnv -notmatch "AUTO_EXPORT_START_ON_READY=false") {
    Add-Failure "edge.env.example must keep start_on_ready disabled by default"
}
if ($edgeEnv -notmatch "AUTO_EXPORT_MIN_READY_DISK_COUNT=1") {
    Add-Failure "edge.env.example must include min ready disk count"
}
if ($edgeEnv -notmatch "AUTO_EXPORT_COOLDOWN_SECONDS=60") {
    Add-Failure "edge.env.example must include cooldown seconds"
}

$removedPaths = @(
    "deploy/udev/99-rustfs-transfer-disk.rules",
    "deploy/udev/99-rustfs-transfer-disk-dev.rules",
    "deploy/udev/README.md",
    "deploy/systemd/rustfs-transfer-disk-rescan@.service",
    "deploy/systemd/rustfs-transfer-disk-rescan-dev@.service",
    "scripts/dev-rescan-helper.sh",
    "crates/edge-backend/src/bin/rescan.rs"
)

foreach ($removedPath in $removedPaths) {
    if (Test-Path (Join-Path $repo $removedPath)) {
        Add-Failure "$removedPath must be removed; Edge disk changes are detected by built-in polling"
    }
}

$deployText = Get-ChildItem -Path (Join-Path $repo "deploy") -Recurse -File |
    ForEach-Object { Get-Content -Raw $_.FullName }

$joinedDeployText = $deployText -join "`n"
if ($joinedDeployText -notmatch "CHANGE_ME") {
    Add-Failure "deploy examples should use CHANGE_ME placeholders for secret-like values"
}

$secretPattern = "(?i)(AKIA[0-9A-Z]{16}|BEGIN (RSA|OPENSSH|EC|PRIVATE) KEY|secret_access_key\s*=\s*`"(?!CHANGE_ME)[^`"]{32,}`")"
if ($joinedDeployText -match $secretPattern) {
    Add-Failure "deploy examples appear to contain a real secret-like value"
}

$scriptText = Get-ChildItem -Path (Join-Path $repo "scripts") -Recurse -File |
    ForEach-Object { Get-Content -Raw $_.FullName }
if (($scriptText -join "`n") -match $secretPattern) {
    Add-Failure "scripts appear to contain a real secret-like value"
}

if ($failures.Count -gt 0) {
    $failures | ForEach-Object { Write-Error $_ }
    exit 1
}

Write-Host "deploy static checks passed"
