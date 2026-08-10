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
    if ($content -notmatch "RUSTFS_TRANSFER__") {
        Add-Failure "$file must set or load RUSTFS_TRANSFER__ environment"
    }
}

$centerService = Get-Content -Raw (Join-Path $repo "deploy/systemd/rustfs-transfer-center.service")
if ($centerService -notmatch "EnvironmentFile=-/etc/rustfs-transfer/center.env") {
    Add-Failure "center service must load center.env for security key injection"
}

$centerToml = Get-Content -Raw (Join-Path $repo "deploy/config/center.example.toml")
if ($centerToml -notmatch "control_api_token_env\s*=\s*`"RUSTFS_TRANSFER__SERVER__CONTROL_API_TOKEN`"") {
    Add-Failure "center config example must reference CENTER CONTROL API token env"
}
if ($centerToml -notmatch "local_master_key_env\s*=\s*`"RUSTFS_TRANSFER__SECURITY__LOCAL_MASTER_KEY`"") {
    Add-Failure "center config example must reference LOCAL_MASTER_KEY env"
}
if ($centerToml -notmatch "center_signature_key_env\s*=\s*`"RUSTFS_TRANSFER__SECURITY__CENTER_SIGNATURE_KEY`"") {
    Add-Failure "center config example must reference CENTER_SIGNATURE_KEY env"
}

$centerEnv = Get-Content -Raw (Join-Path $repo "deploy/config/center.env.example")
if ($centerEnv -notmatch "RUSTFS_TRANSFER__SERVER__CONTROL_API_TOKEN=CHANGE_ME_CENTER_CONTROL_API_TOKEN") {
    Add-Failure "center.env.example must include CENTER CONTROL API token placeholder"
}
if ($centerEnv -notmatch "RUSTFS_TRANSFER__SECURITY__LOCAL_MASTER_KEY=CHANGE_ME_32_BYTE_BASE64_OR_HEX_KEY") {
    Add-Failure "center.env.example must include LOCAL_MASTER_KEY placeholder"
}
if ($centerEnv -notmatch "RUSTFS_TRANSFER__SECURITY__CENTER_SIGNATURE_KEY=CHANGE_ME_32_BYTE_BASE64_OR_HEX_KEY") {
    Add-Failure "center.env.example must include CENTER_SIGNATURE_KEY placeholder"
}

$udevPath = Join-Path $repo "deploy/udev/99-rustfs-transfer-disk.rules"
$udev = Get-Content -Raw $udevPath
if ($udev -notmatch "rustfs-transfer-disk-rescan@%k.service") {
    Add-Failure "udev rule must only trigger the disk rescan systemd template"
}

$udevExecutableLines = ($udev -split "`n") | Where-Object { $_ -notmatch "^\s*#" }
if (($udevExecutableLines -join "`n") -match "(?i)(export|import|cleanup|reinitializ|mkfs|format)") {
    Add-Failure "udev executable rule must not directly run business, formatting, cleanup, or reinit work"
}

$rescanServicePath = Join-Path $repo "deploy/systemd/rustfs-transfer-disk-rescan@.service"
$rescanService = Get-Content -Raw $rescanServicePath
if ($rescanService -notmatch "ExecStart=/opt/rustfs-transfer/bin/rustfs-transfer-rescan --device /dev/%I --trigger udev") {
    Add-Failure "disk rescan service must notify the edge daemon rescan endpoint"
}
if ($rescanService -notmatch "EnvironmentFile=-/etc/rustfs-transfer/edge.env") {
    Add-Failure "disk rescan service must load edge.env for the local rescan token"
}
if ($rescanService -match "(?i)(export-key|import-worker|cleanup|reinitializ|mkfs|format)") {
    Add-Failure "disk rescan service must not directly run business, formatting, cleanup, or reinit work"
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
