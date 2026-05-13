# borderless-core

协议核心类型。**纯逻辑、零 IO、无平台代码**。

## 包含什么

| 模块 | 内容 |
|---|---|
| `node` | `NodeId`（由 BLAKE3 截断 Ed25519 公钥而来）、`ProtocolVersion` |
| `hid` | `HidUsage`（USB HID Usage Code）、`ModifierMask` 修饰键位掩码 |
| `input` | `InputEvent` 输入事件枚举（鼠标、按键、`Enter`/`Leave`）|
| `clipboard` | `ClipboardSnapshot`、`ClipItem`、`LazyPayload`（256 KiB 阈值） |
| `wire` | `WireFrame` 顶层信封：`Control` / `Input` / `Clipboard` / `FetchRequest` / `FetchResponse` / `FetchMiss`；`ControlFrame`（`Hello`/`Welcome`/`Ping`/`Pong`/`Bye`） |

顺手提供 `encode<T>` / `decode<T>` 两个 postcard 包装函数，省去调用方写一遍。

## 为什么要单独放一个 crate

把"线上格式"与"网络/平台代码"严格隔离的好处：

1. 协议改动可以在零依赖、毫秒级编译反馈下做单测
2. 未来要写 GUI、移动端、CI 工具直接 `borderless-core = ...` 即可
3. 协议层任何变更都很容易被代码评审捕获

## v0.2 协议要点

- 一切跑在一条 TCP+TLS 流上，无独立 datagram 通道
- 帧布局：`u32 LE length || postcard(WireFrame)`
- HID 表覆盖 26 字母 + 10 数字 + F1–F24 + 修饰键 + 常用编辑键 + 数字键盘 + 部分 Consumer 控制键
- 大于 `INLINE_THRESHOLD` (256 KiB) 的图片以 `LazyPayload::OnDemand { hash, size }` 占位，配合 `FetchRequest` / `FetchResponse` / `FetchMiss` 拉取

## 测试

序列化 round-trip 覆盖每个 `InputEvent` / `ClipboardSnapshot` / `WireFrame`
变体（含 `Fetch*`），确保 postcard 编解码字节相等。

```bash
cargo test -p borderless-core
```
