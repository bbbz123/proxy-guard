# Proxy Guard

`Proxy Guard` 是一个面向 Windows 的代理残留清理工具，用来在关机或重启时清理用户自己选中的系统代理项，尽量减少因为本地代理残留导致的“下次开机没网”问题。

它的定位很克制，只处理当前用户的 WinINet 系统代理，不接管 Clash，不处理 TUN、DNS、路由、WinHTTP 或网卡。

## 项目结构

- `proxy_guard_core`
  共享核心库，负责代理扫描、规则匹配、配置读写、注册表访问和清理逻辑。
- `proxy_guard_helper`
  Rust 编写的后台 helper，按当前用户运行，在关机/重启时执行清理。
- `proxy_guard_setup`
  旧的 Rust/Slint 设置界面 crate，目前保留在工作区中，但默认不再作为主设置界面使用。
- `proxy_guard_setup.pyw`
  当前使用的 Python `tkinter` 设置界面，用来扫描当前代理、选择托管规则并保存配置。
- `packaging`
  本地安装、便携打包和 WiX MSI 脚本。

## 运行方式

- helper 作为当前用户后台进程运行。
- helper 会在触发关机/重启事件时重新读取配置，所以改完设置后不需要重启 helper。
- 默认清理范围是“关机/重启（非注销）”。
- 也支持可选地把“注销”纳入清理范围。

## 配置文件

默认配置文件位置：

`%LOCALAPPDATA%\ProxyGuard\config.json`

配置内容主要包括：

- `managed_rules`
- `cleanup_scope`
- `cleanup_on_login`
- `auto_start_helper`
- `meta`

如果使用便携模式，配置会保存在程序目录旁边的 `config\config.json`。

## 本地测试

运行 Rust 测试并构建 helper：

```bat
test.bat
```

运行测试、构建后直接打开设置界面：

```bat
test.bat open
```

直接双击打开图形设置界面：

```bat
test-ui.bat
```

## 本地安装体验

构建 helper 后，可以用本地脚本模拟安装：

```powershell
powershell -ExecutionPolicy Bypass -File .\packaging\dev-install.ps1
```

卸载本地安装：

```powershell
powershell -ExecutionPolicy Bypass -File .\packaging\dev-uninstall.ps1
```

## 便携打包

当前推荐的分发方式是便携包：

```powershell
powershell -ExecutionPolicy Bypass -File .\packaging\build-portable.ps1
```

这个脚本会：

- 构建 `proxy_guard_helper.exe`
- 用 PyInstaller 把 `proxy_guard_setup.pyw` 打成独立 `exe`
- 生成便携目录和便携压缩包

## MSI 打包

WiX 模板位于：

`packaging\wix\Product.wxs`

如果本机已安装 WiX Toolset v4，可以运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\packaging\build-msi.ps1
```

注意：当前 WiX 模板仍沿用旧的 Rust 设置程序路径，尚未迁移到 Python GUI 的最终安装版方案。

## 当前限制

- `ShutdownOnly` 只保留兼容意义，Windows 在这个会话结束路径上并不能可靠地区分“关机”和“重启”。
- 目前没有托盘菜单，需要重新运行设置界面来改配置。
- 便携包已经可用，但安装版体验和卸载 UX 仍然可以继续打磨。
