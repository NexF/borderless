# borderless-pal-macos

macOS 后端实现。**只在 `target_os = "macos"` 下展开实质代码**，
其它平台编译为空 stub。

## v0.1 现状

| 能力 | 状态 |
|---|---|
| 文本剪贴板读 / 写 / 监听变化 | **可用**（`arboard`，对 `NSPasteboard.changeCount` 间接轮询） |
| 鼠标 / 键盘捕获 | stub |
| 鼠标 / 键盘注入 | stub |

## v0.2 计划

- `CGEventTap` 在会话级别捕获事件；必须在拥有 `CFRunLoop` 的独立线程上运行
- `CGEventPost(kCGHIDEventTap, ...)` 注入鼠标 / 键盘事件
- 用 `NSPasteboard` 的 `changeCount` 做监听（不需要轮询全部数据）

## 权限引导（必读）

要让真实键鼠工作，用户必须勾选两个权限：

1. **System Settings → Privacy & Security → Accessibility**：勾选 `borderless`
2. **System Settings → Privacy & Security → Input Monitoring**：勾选 `borderless`

`borderless doctor` 在 macOS 上会输出这两条提醒；v0.2 会增加自动权限请求。

## 键码翻译

macOS 内部用的是 carbon `kVK_*` 码，与 USB HID 不一致。
v0.2 会带一份手维护的 `kVK_* → HidUsage` 映射表，包含日常 ANSI/JIS/ISO 三套布局。
