# borderless-pal

平台抽象层（Platform Abstraction Layer）的 **trait 定义**。

具体实现散落在 `pal-x11` / `pal-windows` / `pal-macos` 三个 crate 中；
本 crate 只提供契约。

## 核心 trait

```rust
trait InputCapture { async start / stop / set_mode }
trait InputEmit    { async emit }
trait Clipboard    { async read / write + watch }
```

加上一个共用的 `CaptureMode { Off, Listen, Grab }`，
分别对应"什么都不做 / 听不抢占 / 完全抓取并屏蔽本地分发"三种模式。

## 设计原则

- **小而锐利**：trait 表面尽量小，便于写假实现做单测
- **async 友好但不绑定特定运行时**：用 `tokio::sync::mpsc` 作为事件出口，
  调用方爱用什么调度都行
- **错误统一为 `PalError`**：`Unsupported`、`PermissionRequired`、`Backend`、
  `Io` 四种基础形态，便于 CLI 把"我没权限"和"我崩了"区分对待

## 为什么 v0.1 主要是契约而非实现

输入捕获/注入这块每平台都有自己的"性格"（macOS 要权限弹窗、Windows 要消息泵、
Wayland 要 portal），强行拼到一起反而难调试。
v0.1 先把网络与剪贴板做扎实，trait 就位，等 v0.2 各平台分别填实即可。
