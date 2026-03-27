$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$releaseDir = Join-Path $root "target\release"
$helperExe = Join-Path $releaseDir "proxy_guard_helper.exe"
$setupScript = Join-Path $root "proxy_guard_setup.pyw"
$setupCmd = Join-Path $root "proxy_guard_setup.cmd"
$installDir = Join-Path $env:LOCALAPPDATA "Programs\ProxyGuard"
$runKeyPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"

if (-not (Test-Path -LiteralPath $helperExe)) {
    throw "Release helper binary not found. Run 'cargo build --release' first."
}

if (-not (Test-Path -LiteralPath $setupScript)) {
    throw "Python setup script not found: $setupScript"
}

if (-not (Test-Path -LiteralPath $setupCmd)) {
    throw "Python setup launcher not found: $setupCmd"
}

New-Item -ItemType Directory -Force -Path $installDir | Out-Null
Copy-Item -LiteralPath $helperExe -Destination (Join-Path $installDir "proxy_guard_helper.exe") -Force
Copy-Item -LiteralPath $setupScript -Destination (Join-Path $installDir "proxy_guard_setup.pyw") -Force
Copy-Item -LiteralPath $setupCmd -Destination (Join-Path $installDir "proxy_guard_setup.cmd") -Force

$installedHelper = Join-Path $installDir "proxy_guard_helper.exe"
$installedSetup = Join-Path $installDir "proxy_guard_setup.cmd"

Set-ItemProperty -LiteralPath $runKeyPath -Name "ProxyGuardHelper" -Value ('"{0}"' -f $installedHelper)

Start-Process -FilePath $installedSetup

Write-Host "Installed Proxy Guard to $installDir"
Write-Host "Registered startup entry: ProxyGuardHelper"
