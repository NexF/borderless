# scripts/

仓库内可执行脚本目录。**全部用纯 shell / 内置工具实现**，不引入外部依赖。

## hooks/

git 钩子。第一次 clone 仓库后，执行**一次**：

```bash
git config --local core.hooksPath scripts/hooks
```

之后每次 `git commit` 都会自动跑：

| 钩子 | 执行内容 | 大概耗时（增量） |
|---|---|---|
| `pre-commit` | `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` | < 2 秒 |

设计原则：

1. **零额外依赖**：纯 bash + cargo，不要求 `pre-commit` / `husky` 之类
2. **自动跳过非 Rust 改动**：只改文档、CI、配置时不会拖慢 commit
3. **不跑测试**：测试交给 CI，本地频繁 commit 时不打扰你；如果你想本地也跑，把
   `pre-commit` 复制成 `pre-push` 并在末尾追加 `cargo test --workspace`
4. **可绕过**：紧急情况用 `git commit --no-verify` 跳过

## 临时关掉钩子

```bash
git config --local --unset core.hooksPath
```

## 故障排查

- **找不到 cargo**：钩子会自动 source `~/.cargo/env`。如果你装在别处，
  把那一行改成你自己的路径，或在登录 shell 里把 cargo 加到 PATH。
- **commit 卡很久**：第一次跑 clippy 会编一遍依赖；之后增量。
