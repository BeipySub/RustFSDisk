param(
    [string] $OutputDir = "target/fixtures"
)

$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
$output = if ([System.IO.Path]::IsPathRooted($OutputDir)) {
    $OutputDir
} else {
    Join-Path $repo $OutputDir
}

Push-Location $repo
try {
    cargo run -p rustfs-transfer-center --example generate_fixture -- $output
}
finally {
    Pop-Location
}
