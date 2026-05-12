# borderless-pal-windows

Windows 后端实现。**只在 `target_os = "windows"` 下展开实质代码**，
其它平台编译为空 stub。

## v0.1 现状

| 能力 | 状态 |
|---|---|
| 文本剪贴板读 / 写 / 监听变化 | **可用**（`arboard`，4 Hz 轮询） |
| 鼠标 / 键盘捕获 | stub |
| 鼠标 / 键盘注入 | stub |

## v0.2 计划

- `SetWindowsHookEx(WH_MOUSE_LL / WH_KEYBOARD_LL)` 装低级钩子，
  在专用线程上运行消息循环，把回调拍下来的事件交给 `EventSink`
- `SendInput` 批量注入；注意"非托管 INPUT 数组"的对齐和键码翻译
- 把剪贴板监听换成 `AddClipboardFormatListener` + 不可见消息窗
  （比轮询省 CPU 也更及时）

## 部署须知（写给未来的自己 / 用户）

- 首次启动会被 Windows Defender Firewall 拦下来弹窗，需要选"允许"，
  我们会在安装包里走 `netsh advfirewall` 写白名单
- 低级钩子需要 UI 会话；服务模式（Session 0）下不能用，要做成
  "登录后用户态启动"
