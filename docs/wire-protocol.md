# 线上协议（v0.2）

所有结构化帧都使用 [`postcard`](https://crates.io/crates/postcard) 编码
（兼容 serde、无需 schema、确定性、字节数显著小于 JSON）。

## 传输

每对 Hub ↔ Spoke 节点共用 **一条** TCP + TLS 流。该流之上承载所有消息：
输入事件、剪贴板快照、控制帧、大对象懒拉取。**没有**多路复用和 datagram 通道。

## 帧格式

```
+----------+-------------------+
| u32 LE   | postcard payload  |
| length   | (length bytes)    |
+----------+-------------------+
```

最大单帧大小 64 MiB（足够覆盖 LazyStore 的单块 256 KiB 上限并留出余量）。

## 握手序列

```
Spoke (initiator)                  Hub (acceptor)
       │                                  │
       │── TCP connect ──────────────────▶│
       │── TLS ClientHello ──────────────▶│
       │◀── TLS ServerHello, Certificate ─│
       │── TLS Finished ─────────────────▶│
       │◀── TLS Finished ─────────────────│
       │                                  │
       │── SignedHello (initiator) ──────▶│
       │◀── SignedHello (acceptor) ───────│
       │                                  │
       │  双方都验证 SignedHello.signature │
       │  对 borderless/hello/v0 ‖ tls_   │
       │  exporter；查 known_peers.toml；  │
       │  通过即建立 Connection           │
```

`SignedHello`：

```rust
struct SignedHello {
    pubkey: [u8; 32],         // 长期 Ed25519 公钥
    name: String,             // 显示名
    max_protocol: u16,        // 当前固定 0
    signature: Vec<u8>,       // Ed25519(HELLO_BIND_LABEL || tls_exporter)
}
```

`tls_exporter` 由 `rustls::ServerConnection::export_keying_material(label="borderless/hello/v0")`
派生，长度 32 字节。这一步把长期身份钉到本次 TLS 会话上：MITM 替换 TLS 也通不过签名校验。

## 顶层帧 `WireFrame`

```rust
pub enum WireFrame {
    Control(ControlFrame),
    Input(InputEvent),         // Hub → Spoke 单向；Spoke 发的会被 Hub 丢弃
    Clipboard(ClipboardSnapshot),
    FetchRequest  { hash: [u8; 32] },
    FetchResponse { hash, chunk_idx, total, bytes },
    FetchMiss     { hash: [u8; 32] },
}
```

## 控制帧 `ControlFrame`

```rust
pub enum ControlFrame {
    Hello   { node_id, name, max_protocol },   // 应用层；目前 SignedHello 已先于此发送
    Welcome { node_id, name, protocol },
    Ping    { nonce: u64 },
    Pong    { nonce: u64 },
    Bye     { reason: String },
}
```

## 输入事件 `InputEvent`

```rust
pub enum InputEvent {
    MouseMove   { dx, dy, ts },
    MouseAbs    { x, y, screen_id },
    MouseButton { btn, pressed },
    Scroll      { dx, dy },
    Key         { code: HidUsage, pressed, modifiers: ModifierMask },
    Enter       { from: NodeId, modifiers },   // v0.3 拓扑边界事件
    Leave       { to:   NodeId },
}
```

`HidUsage` 为 USB HID Usage Code（Keyboard/Keypad page 0x07，部分 Consumer
page 0x0C 也支持）。每个 PAL 后端在本地 keycode ↔ HidUsage 之间做转换；
连线上完全不依赖具体 OS 的扫描码。

## 剪贴板快照 `ClipboardSnapshot`

```rust
pub struct ClipboardSnapshot {
    version: u64,             // Lamport 时钟，每个发起方单调递增
    origin: NodeId,           // 发起方
    created_unix_ms: u64,     // 仅用于人类日志
    items: Vec<ClipItem>,
}

pub enum ClipItem {
    Text(String),
    Html  { html, plain_fallback },
    Image { format, hash, data: LazyPayload },
    Files(Vec<FileRef>),
}

pub enum LazyPayload {
    Inline(Vec<u8>),
    OnDemand { hash: [u8; 32], size: u64 },
}
```

接收端在 `ClipboardEngine::observe_remote` 里检查 `origin != self_id` 且
`version > local_version`，否则丢弃。这条规则消灭了 A↔B 回声死循环。

## 大对象懒拉取流程

```
Sender                                   Receiver
   │── Clipboard{ items: [Image{ data: OnDemand{ hash, size }}] } ─▶│
   │                                                                 │
   │   Receiver 真的需要粘贴内容时：                                  │
   │                                                                 │
   │◀── FetchRequest{ hash } ────────────────────────────────────────│
   │── FetchResponse{ hash, chunk_idx=0, total=N, bytes }────────────▶│
   │── FetchResponse{ hash, chunk_idx=1, total=N, bytes }────────────▶│
   │   ...                                                           │
   │── FetchResponse{ hash, chunk_idx=N-1, total=N, bytes }──────────▶│
   │                                                                 │
   │   Receiver 拼接所有 chunk，重算 BLAKE3 应等于 hash               │
```

如发起方查不到该 hash 的 LazyStore 条目，回 `FetchMiss { hash }`；
接收端再决定是放弃还是向其它节点请求。

## 版本协商

所有顶层帧都包在枚举 `WireFrame` 里。未来不兼容的修改会引入 `WireFrameV1`，
由 `ControlFrame::Hello.max_protocol` 协商出双方都能讲的最高版本。

权威类型定义见 [`crates/core/src/wire.rs`](../crates/core/src/wire.rs)
和 [`crates/core/src/clipboard.rs`](../crates/core/src/clipboard.rs)。
