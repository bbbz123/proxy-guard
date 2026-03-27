$ErrorActionPreference = "Stop"

$installDir = Join-Path $env:LOCALAPPDATA "Programs\ProxyGuard"
$runKeyPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$configDir = Join-Path $env:LOCALAPPDATA "ProxyGuard"

if (Test-Path -LiteralPath $runKeyPath) {
    Remove-ItemProperty -LiteralPath $runKeyPath -Name "ProxyGuardHelper" -ErrorAction SilentlyContinue
}

$helperProcess = Get-Process | Where-Object { $_.Path -eq (Join-Path $installDir "proxy_guard_helper.exe") }
foreach ($process in $helperProcess) {
    Stop-Process -Id $process.Id -Force
}

if (Test-Path -LiteralPath $installDir) {
    Remove-Item -LiteralPath $installDir -Recurse -Force
}

Write-Host "Removed installed files from $installDir"
Write-Host "Config directory was kept at $configDir"
