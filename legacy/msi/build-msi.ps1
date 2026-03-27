$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$wixFile = Join-Path $PSScriptRoot "Product.wxs"
$outputDir = Join-Path $root "dist"
$outputMsi = Join-Path $outputDir "ProxyGuard.msi"

if (-not (Test-Path -LiteralPath $wixFile)) {
    throw "WiX source file not found: $wixFile"
}

$wix = Get-Command wix.exe -ErrorAction SilentlyContinue
if (-not $wix) {
    throw "wix.exe was not found in PATH. Install WiX Toolset v4 first."
}

New-Item -ItemType Directory -Force -Path $outputDir | Out-Null

Push-Location $root
try {
    cargo build --release -p proxy_guard_helper -p proxy_guard_setup
    & $wix.Source build $wixFile -o $outputMsi
}
finally {
    Pop-Location
}

Write-Host "Built MSI: $outputMsi"
