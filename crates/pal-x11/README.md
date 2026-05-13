# borderless-pal-x11

Linux / X11 后端实现。**只在 `target_os = "linux"` 下展开实质代码**，
其它平台编译为空 stub，让 workspace 在所有平台上都能 `cargo check`。

## v0.2 现状

| 能力 | 状态 |
|---|---|
| 文本剪贴板读 / 写 / 监听变化 | **可用**（`arboard`，4 Hz 轮询） |
| 图片剪贴板（PNG/BMP）读 / 写 | **可用**（`arboard::ImageData`） |
| 鼠标 / 键盘捕获（Hub 端） | **可用**：`XInput2` `RawKeyPress`/`RawKeyRelease`/`RawButton*`/`RawMotion`，独立线程跑 `wait_for_event` |
| 鼠标 / 键盘注入（Spoke 端） | **可用**：`XTest` `XTestFakeRelativeMotionEvent` / `XTestFakeButtonEvent` / `XTestFakeKeyEvent` |
| 越界 grab / ungrab | 推迟到 v0.3（v0.2 仅 Listen 模式） |

## 模块

| 文件 | 作用 |
|---|---|
| `capture.rs` | XInput2 监听 + HID 翻译 + 修饰键追踪 → `EventSink` |
| `emit.rs` | XTest 注入；预先扫描当前 keymap，HID → keysym → keycode |
| `keymap.rs` | 静态 HID Usage ↔ X11 keysym 表（含 alphanumeric / F1–F24 / 修饰键 / 标点 / 方向 / 数字键盘） |
| `imp.rs` | 文本 + 图片剪贴板（基于 `arboard`），4 Hz 轮询 |

## 关于 Wayland

XWayland 模式下本 crate 的剪贴板能用；全局键鼠在 Wayland 上本来就被
合成器拒绝。原生 Wayland 后端会在独立 crate 提供，v0.3 之后落地。

## 平台依赖

仅在 Linux 上引入 `x11rb`、`arboard`；其它平台 `Cargo.toml` 中条件编译，
`cargo check` 不会牵连。

## 测试

```bash
cargo test -p borderless-pal-x11
```

涵盖 keymap 静态表的双向 round-trip。运行时行为依赖 `DISPLAY`，CI 中
通过 `Xvfb :99` 跑一遍 self-loop 注入 → capture 收到。
