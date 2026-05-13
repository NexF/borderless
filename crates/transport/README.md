# borderless-transport

网络层：TCP + rustls TLS + Ed25519 应用层身份 + TOFU 配对。

## 模块速览

| 文件              | 作用                                                                   |
| --------------- | -------------------------------------------------------------------- |
| `identity.rs`   | 长期 Ed25519 密钥对的生成 / 持久化（`identity.key`，Unix 下 0o600）、消息签名与验签         |
| `cert.rs`       | 每会话生成的 ECDSA 自签证书；TLS 层一律放行，认证延后到应用层                                 |
| `peer_store.rs` | `known_peers.toml` 的内存视图 + 持久化，按 32 字节公钥索引                           |
| `sas.rs`        | 6 位 Short Authentication String 派生（含 TLS 导出绑定）                       |
| `connection.rs` | 已认证的 `Connection`：在 `tokio_rustls::TlsStream` 上跑 4-byte LE 长度前缀 + postcard 帧 |
| `listener.rs`   | Hub 端：绑定 TCP socket，TLS accept，运行 `handshake`                        |
| `connector.rs`  | Spoke 端：dial TCP，TLS connect，运行 `handshake`                          |

## 信任模型

- TLS 仅提供**机密性 + 通道绑定**，证书一律放行
- 真正的身份认证在应用层：`SignedHello { pubkey, name, signature }`，
  `signature` 是对 `"borderless/hello/v0" || tls_exporter` 的 Ed25519 签名
- 配对成功后把对端公钥写入 `known_peers.toml`；下次相同公钥免确认通过
- Spoke 可用 `Connector::dial(..., expected_node_pubkey)` 钉死 Hub 公钥

这一手把「加密 ≠ 信任」分得很清楚：即便 TLS 层被中间人替换，对方仍需要
持有 Ed25519 私钥才能伪造合法 Hello。

## 测试

- 内嵌单元测试覆盖 identity、peer store、SAS
- `tests/hub_spoke.rs`：TOFU 配对 + 双向帧往返、严格模式陌生方拒绝、Spoke 钉死错误公钥时拒绝

```bash
cargo test -p borderless-transport
```
