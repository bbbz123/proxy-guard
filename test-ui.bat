@echo off
setlocal

pushd "%~dp0"

set "SETUP_CMD=%~dp0proxy_guard_setup.cmd"
set "HELPER=%~dp0target\release\proxy_guard_helper.exe"
echo [INFO] Building latest helper binary...

where cargo >nul 2>nul
if errorlevel 1 (
    echo [ERROR] cargo was not found in PATH.
    echo Install Rust first or build the project once before using this launcher.
    pause
    popd
    exit /b 1
)

cargo build --release -p proxy_guard_helper
if errorlevel 1 (
    echo.
    echo [ERROR] Failed to build release helper.
    pause
    popd
    exit /b 1
)

if not exist "%HELPER%" (
    echo [ERROR] Helper executable was not found:
    echo %HELPER%
    pause
    popd
    exit /b 1
)

if not exist "%SETUP_CMD%" (
    echo [ERROR] Setup launcher was not found:
    echo %SETUP_CMD%
    pause
    popd
    exit /b 1
)

call "%SETUP_CMD%"
if errorlevel 1 (
    echo.
    echo [ERROR] Failed to launch Python setup UI.
    pause
    popd
    exit /b 1
)

popd
exit /b 0
