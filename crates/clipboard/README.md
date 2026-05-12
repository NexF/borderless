# borderless-clipboard

剪贴板同步引擎：版本号 + 防回环 + 历史。

## 它解决的核心问题

朴素剪贴板同步（"我变了就广播"）必然会陷入死循环：
A 复制 → B 收到 → B 写到本地 OS 剪贴板 → B 的监听器又触发 → 回写给 A → ……

本 crate 的 `Engine` 用三条规则掐死循环：

1. **Lamport 版本号**：本地每次产生新内容把 `local_version += 1`；
  收到远端版本号时取 `max(local, remote)`
2. `**origin` 字段**：自己产生的快照在收到时直接 `Decision::Ignore`
3. **严格单调**：`incoming.version <= local_version` 一律视为 stale 丢弃

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

7 个单元测试，覆盖：

- `produce` 正确推进版本号
- self-origin 快照被丢弃
- stale 版本被丢弃
- 接受新远端后本地下次 `produce` 必须超越对端版本
- **三节点链路（A→B→C→A）不会回环**（防止间接回声）
- 历史环形 buffer 受 `history_limit` 限制

