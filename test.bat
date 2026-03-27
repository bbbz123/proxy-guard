@echo off
setlocal

pushd "%~dp0"

set "SETUP_SCRIPT=%~dp0proxy_guard_setup.pyw"
set "SETUP_CMD=%~dp0proxy_guard_setup.cmd"

where cargo >nul 2>nul
if errorlevel 1 (
    echo [ERROR] cargo was not found in PATH.
    echo Install Rust first, then retry.
    popd
    exit /b 1
)

echo [1/3] Running Rust tests...
cargo test -p proxy_guard_core -p proxy_guard_helper
if errorlevel 1 (
    echo.
    echo [ERROR] cargo test failed.
    popd
    exit /b 1
)

where py >nul 2>nul
if errorlevel 1 (
    where python >nul 2>nul
    if errorlevel 1 (
        echo.
        echo [ERROR] Python 3 was not found in PATH.
        popd
        exit /b 1
    )
    set "PYTHON=python"
) else (
    set "PYTHON=py -3"
)

echo.
echo [2/3] Checking Python setup UI syntax...
%PYTHON% -m py_compile "%SETUP_SCRIPT%"
if errorlevel 1 (
    echo.
    echo [ERROR] Python setup UI syntax check failed.
    popd
    exit /b 1
)

echo.
echo [3/3] Building release helper...
cargo build --release -p proxy_guard_helper
if errorlevel 1 (
    echo.
    echo [ERROR] cargo build --release -p proxy_guard_helper failed.
    popd
    exit /b 1
)

set "HELPER=%~dp0target\release\proxy_guard_helper.exe"

echo.
echo [OK] Build and tests completed successfully.
echo Helper: "%HELPER%"
echo Setup : "%SETUP_CMD%"

if /i "%~1"=="open" (
    if exist "%SETUP_CMD%" (
        echo.
        echo Launching setup UI...
        call "%~dp0test-ui.bat"
    ) else (
        echo.
        echo [WARN] Setup launcher was not found.
    )
)

echo.
echo Usage:
echo   test.bat
echo   test.bat open

popd
exit /b 0
