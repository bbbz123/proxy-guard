# Proxy Guard

`Proxy Guard` 是一个面向 Windows 的代理残留清理工具。

当前主路线只有这一条：

- Python 设置界面：`proxy_guard_setup.pyw`
- Rust 后台 helper：`proxy_guard_helper`
- 便携打包：`packaging\build-portable.ps1`

工具目标是只清理用户自己选中的 WinINet 系统代理项，尽量减少因为本地代理残留导致的开机后网络异常问题。

## 当前组成

- `proxy_guard_core`
  负责代理扫描、规则匹配、配置读写和清理逻辑。
- `proxy_guard_helper`
  在关机或重启时执行清理。
- `proxy_guard_setup.pyw`
  当前主设置界面，用来扫描代理、勾选托管规则并保存配置。
- `packaging`
  当前使用中的便携打包和本地安装脚本。
- `legacy`
  历史兼容文件，目前只保留旧 MSI/WiX 方案参考。

## 运行特点

- 只处理当前用户的 WinINet 系统代理
- 只清理用户明确勾选的代理项
- 不处理 Clash 进程本身
- 不处理 TUN、DNS、WinHTTP、路由或网卡

## 配置文件

默认配置位置：

`%LOCALAPPDATA%\ProxyGuard\config.json`

便携模式下：

`.\config\config.json`

## 日常使用

直接打开设置界面：

```bat
test-ui.bat
```

运行测试并构建 helper：

```bat
test.bat
```

## 便携打包

当前推荐的发布方式：

```powershell
powershell -ExecutionPolicy Bypass -File .\packaging\build-portable.ps1
```

这个脚本会：

- 构建 `proxy_guard_helper.exe`
- 用 PyInstaller 打包 `proxy_guard_setup.pyw`
- 生成便携目录和便携压缩包

## 本地安装测试

安装到当前用户目录：

```powershell
powershell -ExecutionPolicy Bypass -File .\packaging\dev-install.ps1
```

卸载：

```powershell
powershell -ExecutionPolicy Bypass -File .\packaging\dev-uninstall.ps1
```
