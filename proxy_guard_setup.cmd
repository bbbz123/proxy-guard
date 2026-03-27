@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
set "SETUP_SCRIPT=%SCRIPT_DIR%proxy_guard_setup.pyw"

if not exist "%SETUP_SCRIPT%" (
    echo [ERROR] Setup script was not found:
    echo %SETUP_SCRIPT%
    exit /b 1
)

set "PYTHON_GUI="
where pyw >nul 2>nul
if not errorlevel 1 set "PYTHON_GUI=pyw -3"
if not defined PYTHON_GUI (
    where pythonw >nul 2>nul
    if not errorlevel 1 set "PYTHON_GUI=pythonw"
)
if not defined PYTHON_GUI (
    where py >nul 2>nul
    if not errorlevel 1 set "PYTHON_GUI=py -3"
)
if not defined PYTHON_GUI (
    where python >nul 2>nul
    if not errorlevel 1 set "PYTHON_GUI=python"
)
if not defined PYTHON_GUI (
    echo [ERROR] Python was not found in PATH.
    exit /b 1
)

start "" %PYTHON_GUI% "%SETUP_SCRIPT%"
exit /b 0
