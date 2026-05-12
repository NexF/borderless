# borderless-core

协议核心类型。**纯逻辑、零 IO、无平台代码**。

## 包含什么

| 模块 | 内容 |
|---|---|
| `node` | `NodeId`（由 BLAKE3 截断 Ed25519 公钥而来）、`ProtocolVersion` |
| `hid` | `HidUsage`（USB HID Usage Code）、`ModifierMask` 修饰键位掩码 |
| `input` | `InputEvent` 输入事件枚举（鼠标、按键、`Enter`/`Leave`）|
| `clipboard` | `ClipboardSnapshot`、`ClipItem`、`LazyPayload`（256 KiB 阈值） |
| `wire` | `WireFrame` 顶层信封 + `ControlFrame`（`Hello`/`Welcome`/`Ping`/`Pong`/`Bye`）|

顺手提供 `encode<T>` / `decode<T>` 两个 postcard 包装函数，省去调用方写一遍。

## 为什么要单独放一个 crate

把"线上格式"与"网络/平台代码"严格隔离的好处：

1. 协议改动可以在零依赖、毫秒级编译反馈下做单测
2. 未来要写 GUI、移动端、CI 工具直接 `borderless-core = ...` 即可
3. 协议层任何变更都很容易被代码评审捕获

## 测试

5 个序列化往返用例，确保每种 `InputEvent` / `ClipboardSnapshot` /
`WireFrame` 经过 postcard 编解码后字节相等。

```bash
cargo test -p borderless-core
```
