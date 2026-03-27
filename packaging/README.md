# Packaging Notes

This folder contains the active packaging and local install scripts for the current route:

- Python setup UI
- Rust helper
- Portable distribution

Active scripts:

- `build-portable.ps1`
  Builds `proxy_guard_helper.exe`, packages `proxy_guard_setup.pyw` into a standalone setup exe with PyInstaller, and creates a portable zip.
- `dev-install.ps1`
  Copies the helper plus Python setup launcher into `%LOCALAPPDATA%\Programs\ProxyGuard` for local testing.
- `dev-uninstall.ps1`
  Removes the local test install.

Legacy MSI/WiX scaffolding has been moved to:

- `legacy\msi`
