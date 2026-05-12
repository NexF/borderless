# 参与贡献

欢迎 issue 与 pull request。本文档说清楚 **本地开发工作流**、
**质量门禁**、**发布流程** 三件事。

---

## 1. 环境准备

- Rust 1.75+，**用官方 [rustup](https://rustup.rs/) 安装**，不要用
  `apt install cargo`（Ubuntu 仓库版本太旧）。
- Linux 还需要 `libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev`
  来编 X11 剪贴板后端（`apt install` 即可）。

第一次 clone 之后**只需做一件事**就能让本地体验和 CI 完全对齐：

```bash
git config --local core.hooksPath scripts/hooks
```

它告诉 git 把项目内的 [scripts/hooks/](scripts/hooks/) 当成钩子目录使用。
具体钩子做了什么见 [scripts/README.md](scripts/README.md)。

---

## 2. 常用命令

| 想干啥 | 命令 | 增量耗时 |
|---|---|---|
| 看代码能不能编 | `cargo check --workspace` | < 1s |
| Debug 构建 | `cargo build --workspace` | 1-2s |
| Release 构建 | `cargo build --workspace --release` | 30-60s |
| 跑全部 42 测试 | `cargo test --workspace` | 1-2s |
| 跑 CLI | `cargo run -p borderless-cli -- <subcommand>` | < 1s |
| 检查格式 | `cargo fmt --all -- --check` | < 1s |
| 修复格式 | `cargo fmt --all` | < 1s |
| 检查 lint | `cargo clippy --workspace --all-targets -- -D warnings` | 1-3s |
| 清空中间产物 | `cargo clean` | 即时 |

注意 `--` 后面的参数是传给程序本身（不是 cargo），例如：

```bash
cargo run -p borderless-cli -- start -v
# 等价于：./target/debug/borderless start -v
```

---

## 3. 质量门禁与自动化流程

```
你写代码 → git commit
              │
              ▼
   ┌─────────────────────────────────────────┐
   │ pre-commit 钩子（本地，可选启用）          │
   │                                         │
   │ 1. 检查 staged 文件，没 .rs / Cargo.*     │
   │    就直接放行（改文档/CI 不打扰）          │
   │                                         │
   │ 2. cargo fmt --all -- --check           │
   │    格式不对 → 红字阻塞                   │
   │                                         │
   │ 3. cargo clippy -D warnings             │
   │    任何 lint → 红字阻塞                  │
   └─────────────────────────────────────────┘
              │ 全过
              ▼
        commit 成功 → git push
              │
              ▼
   ┌─────────────────────────────────────────┐
   │ GitHub Actions CI（远端，每次必跑）        │
   │  .github/workflows/ci.yml               │
   │                                         │
   │ 1. fmt 校验                             │
   │ 2. clippy（与本地等价的 -D warnings）     │
   │ 3. test 矩阵：Linux / macOS / Windows   │
   │    各自完整 build + 全部 42 测试          │
   └─────────────────────────────────────────┘
```

每条 push / pull request 都自动跑 [ci.yml](.github/workflows/ci.yml)。
CI 红了**不要**用 `--no-verify` 强推到 main——先在本地修干净再推。

### 紧急绕过本地钩子

罕见场景（commit 写 WIP、改 CI 配置、纯改文档），可以跳过本地钩子：

```bash
git commit --no-verify -m "wip"
```

但你**绕不过 CI**——CI 没有 `--no-verify` 概念，PR 仍会被红灯拦住。

### 临时关闭钩子

```bash
git config --local --unset core.hooksPath
# 重新启用：再跑一次 git config --local core.hooksPath scripts/hooks
```

---

## 4. 测试约定

- **单元测试**：写在源文件里 `#[cfg(test)] mod tests`，覆盖该模块内部不变量。
- **集成测试**：放在 `crates/<X>/tests/*.rs`，可以使用真实依赖（QUIC、mDNS 等）。
- **永远不要写需要手动操作的"测试"**：
  - 不需要真实键鼠输入的（路由器、修饰键状态机）→ 必须自动化
  - 不需要真实物理屏幕的（QUIC、mDNS、剪贴板防回环）→ 必须自动化
  - 需要鼠标真的越过屏幕物理边界的 → 跳过，靠真机手动验证

跑测试的两种姿势：

```bash
cargo test --workspace                              # 全部 42 个
cargo test -p borderless-transport --test two_nodes # 只跑端到端
cargo test -p borderless-clipboard -- --nocapture   # 看 println! 输出
```

---

## 5. 发布流程

打 git tag 即触发 [release.yml](.github/workflows/release.yml)，自动产
**5 个平台**的二进制并挂到 GitHub Release：

```bash
# 在 main 上确认本地 fmt + clippy + test 全绿
git tag v0.2.0 -m "v0.2.0: keyboard sharing + image clipboard"
git push origin v0.2.0
# 等 ~10 分钟，看 https://github.com/NexF/borderless/releases
```

产出文件：

| 文件 | 适用 |
|---|---|
| `borderless-x86_64-unknown-linux-gnu.tar.gz` | x64 Linux |
| `borderless-aarch64-unknown-linux-gnu.tar.gz` | ARM64 Linux（树莓派、Ampere 等） |
| `borderless-x86_64-apple-darwin.tar.gz` | Intel Mac |
| `borderless-aarch64-apple-darwin.tar.gz` | Apple Silicon Mac（M1/M2/M3） |
| `borderless-x86_64-pc-windows-msvc.zip` | x64 Windows |

注意 tag 命名遵循 `vX.Y.Z` 格式；不符合的 tag 不会触发 release。

### 如果 release workflow 红了

打开 [Actions 页面](https://github.com/NexF/borderless/actions) 查看具体哪一个目标失败。
单个目标失败不会阻止其他平台的产物上传（`fail-fast: false`），所以
"5 个里有 4 个能用"也是可恢复局面。

---

## 6. 代码风格

- 跟随 `rustfmt` 默认（`edition = "2021"`），不要手动调字段对齐
- clippy 的所有默认 lint 都按 `-D warnings` 当错误处理；
  确实要绕开某条 lint 时，在该处加 `#[allow(clippy::xxx)]` **并写注释解释为什么**
- 公开 API 必须有文档注释（`///`），并支持 `cargo doc --workspace --open` 浏览
- 错误用 `thiserror::Error` 派生，**不要**直接 `panic!()`，除非是 `unreachable!()`
  类不可能分支
- 二进制大小 / 启动时间是项目的核心指标之一，引新依赖时考虑下

---

## 7. 提交信息约定

宽松版 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/)：

| 前缀 | 用于 |
|---|---|
| `feat:` | 新功能 |
| `fix:` | bug 修复 |
| `style:` | 仅格式 / 注释 / 空白（无逻辑变化） |
| `chore:` | 构建脚本、依赖升级、CI、配置 |
| `docs:` | 仅文档 |
| `test:` | 仅测试 |
| `refactor:` | 行为不变的重构 |
| `perf:` | 性能优化 |

主标题 ≤ 72 字符，正文换行后写 "为什么这么改"，避免只描述 "改了什么"。

---

## 8. 路线图

详见 [docs/architecture.md §路线图](docs/architecture.md)。
当前 v0.1 是 MVP 骨架；v0.2 主要是真正接通三平台的键鼠捕获 / 注入。
