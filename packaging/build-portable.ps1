$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$distRoot = Join-Path $root "dist"
$bundleDir = Join-Path $distRoot "ProxyGuard-portable"
$zipPath = Join-Path $distRoot "ProxyGuard-portable.zip"
$helperExe = Join-Path $root "target\release\proxy_guard_helper.exe"
$setupScript = Join-Path $root "proxy_guard_setup.pyw"
$portableMarker = Join-Path $bundleDir "proxy_guard.portable"
$portableReadme = Join-Path $PSScriptRoot "portable-readme.txt"
$portableReadmeZh = Join-Path $PSScriptRoot "portable-readme-zh.txt"
$pyInstallerDist = Join-Path $distRoot "_pyinstaller"
$pyInstallerWork = Join-Path $root "build\pyinstaller"

if (-not (Test-Path -LiteralPath $setupScript)) {
    throw "Setup script not found: $setupScript"
}

if (-not (Test-Path -LiteralPath $portableReadme)) {
    throw "Portable readme not found: $portableReadme"
}

if (-not (Test-Path -LiteralPath $portableReadmeZh)) {
    throw "Portable Chinese readme not found: $portableReadmeZh"
}

$python = Get-Command py -ErrorAction SilentlyContinue
if ($python) {
    $pythonExe = $python.Source
    $pythonArgs = @("-3")
}
else {
    $python = Get-Command python -ErrorAction SilentlyContinue
    if (-not $python) {
        throw "Python 3 was not found in PATH."
    }
    $pythonExe = $python.Source
    $pythonArgs = @()
}

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    throw "cargo was not found in PATH."
}

& $pythonExe @pythonArgs -m PyInstaller --version | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "PyInstaller is not installed. Install it first with: py -3 -m pip install pyinstaller"
}

Push-Location $root
try {
    & $cargo.Source build --release -p proxy_guard_helper
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to build proxy_guard_helper.exe"
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $helperExe)) {
    throw "Built helper executable not found: $helperExe"
}

if (Test-Path -LiteralPath $bundleDir) {
    Remove-Item -LiteralPath $bundleDir -Recurse -Force
}

if (Test-Path -LiteralPath $pyInstallerDist) {
    Remove-Item -LiteralPath $pyInstallerDist -Recurse -Force
}

if (Test-Path -LiteralPath $pyInstallerWork) {
    Remove-Item -LiteralPath $pyInstallerWork -Recurse -Force
}

New-Item -ItemType Directory -Force -Path $distRoot | Out-Null
New-Item -ItemType Directory -Force -Path $bundleDir | Out-Null
New-Item -ItemType Directory -Force -Path $pyInstallerDist | Out-Null
New-Item -ItemType Directory -Force -Path $pyInstallerWork | Out-Null

& $pythonExe @pythonArgs -m PyInstaller `
    --noconfirm `
    --clean `
    --windowed `
    --onefile `
    --name proxy_guard_setup `
    --distpath $pyInstallerDist `
    --workpath $pyInstallerWork `
    --specpath $pyInstallerWork `
    $setupScript
if ($LASTEXITCODE -ne 0) {
    throw "PyInstaller build failed."
}

$builtSetupExe = Join-Path $pyInstallerDist "proxy_guard_setup.exe"
if (-not (Test-Path -LiteralPath $builtSetupExe)) {
    throw "Built setup executable not found: $builtSetupExe"
}

Copy-Item -LiteralPath $builtSetupExe -Destination (Join-Path $bundleDir "proxy_guard_setup.exe") -Force
Copy-Item -LiteralPath $helperExe -Destination (Join-Path $bundleDir "proxy_guard_helper.exe") -Force
Copy-Item -LiteralPath $portableReadme -Destination (Join-Path $bundleDir "README.txt") -Force
Copy-Item -LiteralPath $portableReadmeZh -Destination (Join-Path $bundleDir "README-中文.txt") -Force
New-Item -ItemType File -Path $portableMarker -Force | Out-Null

if (Test-Path -LiteralPath $zipPath) {
    Remove-Item -LiteralPath $zipPath -Force
}

Compress-Archive -Path (Join-Path $bundleDir "*") -DestinationPath $zipPath -Force

Write-Host "Portable folder: $bundleDir"
Write-Host "Portable zip   : $zipPath"
