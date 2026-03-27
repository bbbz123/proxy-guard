Proxy Guard 便携版

这个文件夹内包含：

- proxy_guard_setup.exe
  打开设置界面。
- proxy_guard_helper.exe
  在关机或重启时执行代理清理的后台 helper。
- proxy_guard.portable
  便携模式标记文件。存在这个文件时，配置会保存在程序目录旁边。

基本用法：

1. 运行 proxy_guard_setup.exe
2. 勾选你希望由 Proxy Guard 托管的代理项
3. 如有需要，可开启 helper 开机自启
4. 点击保存设置

说明：

- 便携模式下，配置保存在 .\config\config.json
- 本工具只处理当前用户的 WinINet 系统代理
- 不处理 TUN、DNS、WinHTTP、路由或网卡
