# borderless-cli

`borderless` 二进制：把所有 crate 拼起来，提供命令行界面。v0.2 起为 C/S 架构。

## 子命令

| 命令 | 作用 |
|---|---|
| `borderless serve` | 启动 Hub。绑定 TCP+TLS 监听端口，接受 Spoke 连接，做剪贴板转发 + 输入事件单向广播。可选 `--bind <ip:port>` 覆盖配置；可选 `--accept-new-peers` 接受首次配对的 Spoke。 |
| `borderless connect [<host:port>]` | 启动 Spoke。Dial 指定 Hub 并保持长连。首次配对加 `--pair`；可选 `--pin <pubkey>` 钉死 Hub 公钥（零 TOFU 信任假设）。无参数时从 `config.toml` 的 `[client].server_addr` 读地址。 |
| `borderless status` | 打印自己 NodeId、当前角色、监听 / 服务端地址、`known_peers.toml` 中的已配对节点列表 |
| `borderless clip history` | （v0.3 计划）经 IPC 与守护进程对话显示剪贴板历史 |
| `borderless clip set <text>` | 把文本写入本机 OS 剪贴板（演示和测试用） |
| `borderless clip get` | 读本机 OS 剪贴板并打印到 stdout |
| `borderless doctor` | 角色感知的平台 / 网络自检：Hub 检查端口绑定 + 防火墙提示；Spoke 检查可达性。 |

## 全局选项

- `--config-dir <DIR>` / `BORDERLESS_CONFIG_DIR`：覆盖默认配置目录
  （默认遵循 XDG，`~/.config/borderless`）
- `-v / -vv / -vvv`：打开更详细的日志（基于 `tracing` + `EnvFilter`）

## 配置文件

第一次运行时会自动在 `<config_dir>/config.toml` 写入默认值，结构如下：

```toml
[node]
name = "alice"

[role]
kind = "unconfigured"      # serve/connect 子命令首次运行时自动改为 "hub" 或 "spoke"

[hub]                      # 仅当 kind = "hub" 时被读取
bind_ip = "0.0.0.0"
port = 38437
accept_new_peers = false   # 等价于以前的 pair 模式开关

[client]                   # 仅当 kind = "spoke" 时被读取
server_addr = "192.168.10.5:38437"
expected_server_id = ""    # 可选，hex 编码 32 字节 Hub 公钥

[clipboard]
history_size = 50
sync_text = true
sync_image = true          # 大于 256 KiB 走懒拉取

[input]
enabled = true             # spoke 端可关掉键鼠注入只跑剪贴板
```

## 状态文件

| 文件 | 存什么 |
|---|---|
| `identity.key` | 64 字节 Ed25519 长期密钥（前 32 = 私钥，后 32 = 公钥），Unix 下 0o600 |
| `known_peers.toml` | TOFU 之后存下来的对端公钥指纹列表 |
| `config.toml` | 用户配置 |

## 测试

```bash
cargo test -p borderless-cli
```

涵盖 config 默认 / 持久化、doctor 在 Hub / Spoke / Unconfigured 三态下的输出。
