# borderless-pal-x11

Linux / X11 后端实现。**只在 `target_os = "linux"` 下展开实质代码**，
其它平台编译为空 stub，让 workspace 在所有平台上都能 `cargo check`。

## v0.1 现状

| 能力 | 状态 |
|---|---|
| 文本剪贴板读 / 写 / 监听变化 | **可用**（`arboard`，4 Hz 轮询） |
| 鼠标 / 键盘捕获 | stub（返回 `Ok(())`，不实际抓事件） |
| 鼠标 / 键盘注入 | stub（返回 `PalError::Unsupported`） |

剪贴板监听在 v0.1 用了一个独立线程做 4 Hz 轮询。
理论上也可以走 X11 `XFixesSelectionNotify`，但额外的复杂度对 v0.1
"先把网络和防回环跑通"的目标不是必要，留到 v0.2。

## v0.2 计划

- `XInput2` 原始事件 (`XI_RawMotion` / `XI_RawButtonPress` / `XI_RawKeyPress`)
  做不抢占的全局监听；越界时再 `XGrabPointer`
- `XTest` 注入鼠标 / 键盘事件
- 用 `XFixesSelectionNotify` 替代轮询

## 关于 Wayland

XWayland 模式下本 crate 也能用文本剪贴板；但全局键鼠在 Wayland 上
本来就被合成器拒绝了。原生 Wayland 后端会在 `pal-wayland` 单独提供，
v0.3 落地。

## 平台依赖

仅在 Linux 上引入 `x11rb`、`arboard`；其他平台 `Cargo.toml` 中条件编译，
`cargo check` 不会牵连。
