# borderless

一个仅在局域网内工作的、跨平台的 **键盘 / 鼠标 / 剪贴板 共享工具**。

> 类似 Synergy / Barrier / Input Leap，但拥有更强的剪贴板体验
> （富类型、历史记录、大对象懒加载）以及全 Rust 的核心实现。

## 当前状态

**v0.1 MVP 骨架。** 现已可用：

- 完整的 Cargo workspace，按职责拆分到 9 个 crate
- mDNS 发现 + 基于 QUIC（TLS 1.3）的传输，节点身份用自签 Ed25519 长期密钥
- 首次配对（TOFU）：通过 Short Authentication String 完成握手互信
- 文本剪贴板同步，自带 origin + Lamport 版本号防回环
- 可插拔的平台抽象层（PAL），X11 / Windows / macOS 三家骨架全部就位
- `borderless` CLI：`start`、`pair`、`status`、`clip history`、`doctor`

尚未实现（已写入路线图）：

- 真正的键鼠捕获 / 注入（v0.1 仅有 PAL stub）
- 键盘共享、富剪贴板、文件懒加载
- Wayland 后端
- 图形界面

## 性能目标（作为 v1.0 的验收基准）


| 指标                | 目标       |
| ----------------- | -------- |
| 鼠标端到端延迟（同 LAN，有线） | < 8 ms   |
| 键盘端到端延迟           | < 10 ms  |
| 文本剪贴板同步           | < 50 ms  |
| 1 MB 图片剪贴板同步      | < 200 ms |
| 空闲 CPU 占用         | < 0.5%   |
| 常驻内存占用            | < 30 MB  |


## 构建

需要 Rust 1.75+。

```bash
cargo build --workspace
cargo test  --workspace
cargo run -p borderless-cli -- doctor
```

跨平台说明：

- **Linux/X11**：链接时需要 `libxcb`、`libxtst` 头文件（用于 v0.2 的真实 PAL）。
- **Linux/Wayland**：计划在 v0.3 引入，需要 GNOME 46+ / KDE 6+ 才有 `org.freedesktop.portal.InputCapture`。
- **macOS**：需要"辅助功能"和"输入监控"两项权限。运行 `borderless doctor` 查看清单。
- **Windows**：首次启动会触发 UDP 防火墙弹窗，请在私有网络/回环放行。

## 目录结构

```
crates/
  core/            协议线类型、版本时钟，无 IO
  transport/       QUIC + mDNS + 配对
  pal/             平台抽象 trait
  pal-x11/         X11 后端（XInput2 + XTest，规划中）
  pal-windows/     Windows 后端（低级 hook + SendInput，规划中）
  pal-macos/       macOS 后端（CGEventTap + CGEventPost，规划中）
  clipboard/       版本化快照 + 历史环
  input-router/    虚拟屏幕拓扑、越界检测、修饰键状态机
  cli/             borderless 二进制
docs/
  architecture.md  架构总览
  wire-protocol.md 线上协议参考
tests/
  two_nodes.rs     端到端配对 + 剪贴板同步集成测试
```

## 协议许可

MIT 或 Apache-2.0，二选一。