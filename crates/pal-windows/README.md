# borderless-pal-windows

Windows 后端实现。**只在 `target_os = "windows"` 下展开实质代码**，
其它平台编译为空 stub。

## v0.2 现状

| 能力 | 状态 |
|---|---|
| 文本剪贴板读 / 写 / 监听变化 | **可用**（`arboard`，4 Hz 轮询） |
| 图片剪贴板读 / 写 | **可用**（`arboard::ImageData`，BMP/CF_DIB） |
| 鼠标 / 键盘捕获（Hub 端） | **可用**：`SetWindowsHookExW(WH_KEYBOARD_LL / WH_MOUSE_LL)`，独立线程跑 `GetMessageW` |
| 鼠标 / 键盘注入（Spoke 端） | **可用**：`SendInput`（扫描码 + 扩展键标志，鼠标相对/绝对 + 滚轮） |

## 模块

| 文件 | 作用 |
|---|---|
| `capture.rs` | LL hook 安装与消息循环；C 回调用 `OnceLock` + `Mutex` 拿到全局 `EventSink` 与修饰键状态 |
| `emit.rs` | `SendInput` 注入；区分键盘（HID → VK + scan code + 扩展位）与鼠标 |
| `keymap.rs` | 静态 HID ↔ Windows VK + scan code 表（带 letter / digit 扫描码数组） |
| `imp.rs` | 文本 + 图片剪贴板 |

## 部署须知

- 首次启动会被 Windows Defender Firewall 弹窗，选"允许"；后续可通过
  `netsh advfirewall firewall add rule` 写白名单
- 低级钩子要求 UI 会话；服务模式（Session 0）下不可用，须以登录用户身份运行
- `SendInput` 扫描码方案确保游戏 / 远程桌面等场景下输入能落到目标进程

## 测试

```bash
cargo test -p borderless-pal-windows
```

主要覆盖 keymap 双向 round-trip。CI 中需要真实图形 session 才能跑端到端
hook，`#[cfg(feature = "ci-graphical")]` 保留扩展位（v0.3 接通）。
