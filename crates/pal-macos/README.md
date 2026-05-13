# borderless-pal-macos

macOS 后端实现。**只在 `target_os = "macos"` 下展开实质代码**，
其它平台编译为空 stub。

## v0.2 现状

| 能力 | 状态 |
|---|---|
| 文本剪贴板读 / 写 / 监听变化 | **可用**（`arboard`，对 `NSPasteboard.changeCount` 间接轮询） |
| 图片剪贴板读 / 写 | **可用**（`arboard::ImageData`） |
| 鼠标 / 键盘注入（Spoke 端） | **可用**：`CGEventPost(kCGHIDEventTap, ...)` 走 `CGEventSource` |
| 鼠标 / 键盘捕获（Hub 端） | **部分**：`AXIsProcessTrusted` 权限检查可用；`CGEventTap` + `CFRunLoop` 完整事件抓取推迟到 v0.3 |

## 模块

| 文件 | 作用 |
|---|---|
| `capture.rs` | 权限自检（无权限时返回 `PalError::PermissionRequired`），event tap 主循环占位 |
| `emit.rs` | `CGEvent::new_*` + `CGEventPost`；鼠标用累加坐标，键盘用 `CGKeyCode` |
| `keymap.rs` | 静态 HID ↔ Carbon `kVK_*` 表（ANSI 主键 + F-keys + 修饰键 + 数字键盘 + 方向） |
| `imp.rs` | 文本 + 图片剪贴板 |

## 权限引导（必读）

要让真实键鼠工作，用户必须勾选两个权限：

1. **System Settings → Privacy & Security → Accessibility**：勾选 `borderless`
2. **System Settings → Privacy & Security → Input Monitoring**：勾选 `borderless`

`borderless doctor` 在 macOS 上会输出这两条提醒及 Gatekeeper 临时放行命令。

## Gatekeeper

由于 release 二进制未签名，第一次运行时系统会提示"无法验证开发者"。
解决：

```bash
xattr -d com.apple.quarantine /path/to/borderless
```

或在 System Settings → Privacy & Security 末尾点 "Allow Anyway"。

## 测试

```bash
cargo test -p borderless-pal-macos
```

主要覆盖 keymap 双向 round-trip。运行时键鼠注入需在真实图形 session 中
手测，CI 端到端推迟到 v0.3。
