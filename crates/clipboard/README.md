# borderless-clipboard

剪贴板同步引擎：版本号 + 防回环 + 历史 + 大对象懒载荷。

## 它解决的核心问题

朴素剪贴板同步（"我变了就广播"）必然会陷入死循环：
A 复制 → B 收到 → B 写到本地 OS 剪贴板 → B 的监听器又触发 → 回写给 A → ……

本 crate 的 `Engine` 用三条规则掐死循环：

1. **Lamport 版本号**：本地每次产生新内容把 `local_version += 1`；
   收到远端版本号时取 `max(local, remote)`
2. **`origin` 字段**：自己产生的快照在收到时直接 `Decision::Ignore`
3. **严格单调**：`incoming.version <= local_version` 一律视为 stale 丢弃

## 大对象懒载荷

`LazyStore` 是发送方对 >256 KiB 内容的暂存：

```rust
let store = LazyStore::new();
let snap = engine.produce_image_snapshot(&store, png_bytes, ImageFormat::Png);
// 大于阈值时 snap.items[0] 是 ClipItem::Image { data: LazyPayload::OnDemand { hash, size } }
// 真正字节躺在 store 里，等接收端发 FetchRequest 时再 serve
```

接收方收到 `LazyPayload::OnDemand` 后，于 paste 时通过
`WireFrame::FetchRequest { hash }` 向源端拉取；源端用一个或多个
`WireFrame::FetchResponse { hash, chunk_idx, total, bytes }` 分块返回；
若源端已淘汰该 hash，则回 `WireFrame::FetchMiss`。

## API 速览

```rust
let engine = Engine::new(self_node_id, /*history_limit*/ 50);

// 本地剪贴板变了：产生新快照（版本号自动 +1，落入历史）
let snap = engine.produce_text("hello".into());

// 远端来了快照：决定是否应用
match engine.observe_remote(snap_from_peer) {
    Decision::Apply(s) => os_clipboard.write(&s),
    Decision::Ignore   => {} // 自产 / stale，吃掉就好
}

// 给 CLI 用的历史
let recent = engine.history();
```

## 测试

涵盖 produce 推进版本号、self-origin / stale 丢弃、三节点链路不回环、
LazyStore 阈值切换 + FIFO 淘汰 + 重复 hash 幂等、`serve_fetch` 解析。
