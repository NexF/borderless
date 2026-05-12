# 线上协议（v0）

所有结构化帧都使用 [`postcard`](https://crates.io/crates/postcard) 编码
（兼容 serde、无需 schema、确定性、字节数显著小于 JSON）。

## 通道

每对节点共用一条 QUIC 连接，复用以下逻辑通道：

| 通道 | QUIC 投递方式 | 帧类型 |
|---|---|---|
| 控制 | 双向流 `0` | `ControlFrame` |
| 输入（低延迟）| 不可靠数据报 | `InputDatagram` |
| 输入（可靠） | 单向流 | `InputFrame` |
| 剪贴板 | 单向流 | `ClipboardFrame` |
| 懒拉取 | 由接收端开启的双向流 | `hash → bytes` |

## 帧格式

每条流上的帧都使用长度前缀编码：

```
+----------+-------------------+
| u32 LE   | postcard payload  |
| length   | (length bytes)    |
+----------+-------------------+
```

数据报里只放一个 postcard 编码后的载荷，不带长度前缀；它们必须能塞进
单个 QUIC datagram MTU。

## 版本协商

所有顶层帧都包在枚举 `WireV0` 里：

```rust
pub enum WireV0 {
    Hello { node_id: NodeId, name: String, ... },
    Control(ControlFrame),
    Input(InputFrame),
    Clipboard(ClipboardFrame),
}
```

未来不兼容的版本会加入 `WireV1`，由 `Hello` 帧协商出双方都能讲的最高版本。

权威类型定义见 [`crates/core/src/wire.rs`](../crates/core/src/wire.rs)。
