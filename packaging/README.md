# Packaging Notes

This project includes a WiX v4 template for building a per-user MSI installer,
plus PowerShell scripts for local testing without MSI.

Expected build artifacts:

- `target\release\proxy_guard_helper.exe`
- `proxy_guard_setup.pyw`
- `proxy_guard_setup.cmd`

Installer responsibilities:

- Install into `%LOCALAPPDATA%\Programs\ProxyGuard`
- Register `proxy_guard_helper.exe` under the current user's `Run` key
- Add Start Menu shortcuts for opening settings and uninstalling
- Launch the Python setup UI once after installation

The WiX template is intended as scaffolding and should be built with WiX Toolset
v4 or newer.

Helpful scripts:

- `build-msi.ps1`
- `build-portable.ps1`
- `dev-install.ps1`
- `dev-uninstall.ps1`

Current note:

- `dev-install.ps1` now installs the Python setup UI launcher for local testing.
- `build-msi.ps1` still builds the legacy Rust setup executable because the WiX
  template has not yet been migrated to bundle a Python runtime.
- `build-portable.ps1` builds the current Python setup UI into a standalone exe
  with PyInstaller, then bundles it with `proxy_guard_helper.exe`.
