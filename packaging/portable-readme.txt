Proxy Guard Portable

Files in this folder:

- proxy_guard_setup.exe
  Open the settings window.
- proxy_guard_helper.exe
  Background helper that performs cleanup on shutdown/restart.
- proxy_guard.portable
  Enables portable mode so config is stored next to the program.

Typical usage:

1. Run proxy_guard_setup.exe
2. Select managed proxy entries
3. Optionally enable helper auto-start
4. Save settings

Notes:

- In portable mode, config is stored in .\config\config.json
- This package only manages the current user's WinINet system proxy
- It does not touch TUN, DNS, WinHTTP, routes, or network adapters
