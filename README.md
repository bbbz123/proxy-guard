# Proxy Guard

`Proxy Guard` is a Windows helper that clears user-selected system proxy entries
during shutdown/restart to reduce connectivity issues caused by stale local
proxy settings.

The tool is intentionally narrow:

- Only touches the current user's WinINet proxy settings
- Only clears rules the user explicitly selected in setup
- Does not manage Clash, TUN, DNS, routes, WinHTTP, or network adapters

## Workspace Layout

- `proxy_guard_core`
  Shared parsing, candidate scanning, config persistence, registry access, and cleanup logic.
- `proxy_guard_helper`
  A hidden background helper that runs per-user and listens for session-end events.
- `proxy_guard_setup`
  The older Rust/Slint setup crate, kept in the workspace but no longer used by default.
- `proxy_guard_setup.pyw`
  The current Python `tkinter` setup window for scanning current proxies and saving managed rules.
- `packaging`
  WiX scaffolding plus helper PowerShell scripts for local install/uninstall.

## Runtime Behavior

- The helper is designed as a per-user background process.
- It reloads the config at shutdown/restart time, so setup changes apply without restarting the helper.
- Default scope is `shutdown + restart`.
- Logoff cleanup is optional and currently controlled by the saved cleanup scope.

## Config File

Config is stored at:

`%LOCALAPPDATA%\ProxyGuard\config.json`

The file contains:

- `managed_rules`
- `cleanup_scope`
- `meta`

## Building

```powershell
cargo build --release
```

Or on Windows, use the helper batch file:

```bat
test.bat
```

To run the build/test flow and then open the setup UI automatically:

```bat
test.bat open
```

To simply double-click a batch file and open the graphical setup window:

```bat
test-ui.bat
```

## Local Tryout Without MSI

After building the helper binary, you can use the included helper scripts:

```powershell
powershell -ExecutionPolicy Bypass -File .\packaging\dev-install.ps1
```

This currently targets the Rust helper binary for local installation. The Python
setup UI is launched locally through `test-ui.bat`.

To uninstall the local script-based install:

```powershell
powershell -ExecutionPolicy Bypass -File .\packaging\dev-uninstall.ps1
```

## MSI Packaging

WiX scaffolding lives in `packaging\wix\Product.wxs`.

If WiX Toolset v4 is available, a package build can be driven with:

```powershell
powershell -ExecutionPolicy Bypass -File .\packaging\build-msi.ps1
```

## Current Limitations

- `ShutdownOnly` is best-effort because Windows does not reliably expose a clean
  "shutdown but not restart" distinction through the session-end path used here.
- No tray menu is provided; rerun setup through `test-ui.bat` or `proxy_guard_setup.pyw`.
- The WiX template is scaffolded and ready for local packaging, but full MSI UX
  polish such as "delete config on uninstall" is not implemented yet.
