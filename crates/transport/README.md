# borderless-transport

网络层：QUIC + 节点身份 + mDNS 发现 + TOFU 配对。

## 模块速览


| 文件              | 作用                                                                   |
| --------------- | -------------------------------------------------------------------- |
| `identity.rs`   | 长期 Ed25519 密钥对的生成 / 持久化（`identity.key`，Unix 下 0o600）、消息签名与验签         |
| `cert.rs`       | 每会话生成的 ECDSA 自签证书；TLS 层一律放行，认证延后到应用层                                 |
| `peer_store.rs` | `known_peers.toml` 的内存视图 + 持久化，按 32 字节公钥索引                           |
| `sas.rs`        | 6 位 Short Authentication String 派生（含 TLS 导出绑定）                       |
| `discovery.rs`  | mDNS 服务（`_borderless._udp.local.`）发布 + 浏览                            |
| `endpoint.rs`   | 高层 `Endpoint`，统一 `connect()` / `accept()`，并实现 Hello 双向签名校验 + 指纹 TOFU |


## 信任模型

- TLS 仅提供**机密性 + 通道绑定**，证书一律放行
- 真正的身份认证在应用层：`SignedHello { pubkey, name, signature }`，
`signature` 是对 `"borderless/hello/v0" || tls_exporter` 的 Ed25519 签名
- 配对成功后把对端公钥写入 `known_peers.toml`；下次相同公钥免确认通过

这一手把"加密 ≠ 信任"分得很清楚，避免了"我证书被劫持就完了"的脆弱设计。

## 测试

- 6 个内嵌单元测试（identity、peer store、SAS）
- 4 个 `tests/two_nodes.rs` 集成测试：配对 + 双向剪贴板 + 防回环、严格模式陌生方拒绝、闭连后重连、连续 16 帧顺序保持
- 1 个 `tests/discovery.rs` 集成测试：两节点 mDNS 互相发现

```bash
cargo test -p borderless-transport
```

