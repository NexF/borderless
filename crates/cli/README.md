# borderless-cli

`borderless` 二进制：把所有 crate 拼起来，提供命令行界面。

## 子命令

| 命令 | 作用 |
|---|---|
| `borderless start` | 启动守护：mDNS 发布 + 浏览、接受连接、主动连陌生 peer（仅在配对模式下）、剪贴板双向同步 |
| `borderless pair` | 进入配对模式（`allow_new_peers = true`）；首次连接时把对端公钥指纹写入 `known_peers.toml` |
| `borderless status` | 打印自己 NodeId、监听地址、`known_peers.toml` 中的已配对节点列表 |
| `borderless clip history` | 显示守护进程内剪贴板历史（v0.1 仅占位，v0.2 走 IPC bridge） |
| `borderless doctor` | 检测平台权限 / 环境（X11 DISPLAY、Wayland 警告、macOS 辅助功能、Win 防火墙提示） |

## 全局选项

- `--config-dir <DIR>` / `BORDERLESS_CONFIG_DIR`：覆盖默认配置目录
  （默认遵循 XDG，`~/.config/borderless`）
- `-v / -vv / -vvv`：打开更详细的日志（基于 `tracing` + `EnvFilter`）

## 配置文件

第一次运行时会自动在 `<config_dir>/config.toml` 写入默认值，结构如下：

```toml
[node]
name = "alice"

[network]
port = 38437
bind_ip = "0.0.0.0"

[clipboard]
history_size = 50
sync_text = true
```

## 状态文件

| 文件 | 存什么 |
|---|---|
| `identity.key` | 64 字节 Ed25519 长期密钥（前 32 = 私钥，后 32 = 公钥），Unix 下 0o600 |
| `known_peers.toml` | TOFU 之后存下来的对端公钥指纹列表 |
| `config.toml` | 用户配置 |

## 测试

7 个单元测试：

- `config`（4 个）：默认值合理、缺失文件自动落盘、用户覆盖 round-trip、损坏 TOML 报错
- `doctor`（3 个）：`run` 至少有 binary 这一项、格式包含状态字符与名称、字符宽度一致
