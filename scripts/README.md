# scripts/

仓库内可执行脚本目录。**全部用纯 shell / 内置工具实现**，不引入外部依赖。

## hooks/

git 钩子。按本仓库的开发约定，每位开发者第一次 clone 后执行**一次**：

```bash
git config --local core.hooksPath scripts/hooks
```

之后每次 `git commit` 自动运行下表中的钩子：

| 钩子 | 执行内容 |
|---|---|
| `pre-commit` | `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`（仅在 staged 包含 `*.rs` / `Cargo.*` 时触发） |

完整说明、紧急绕过姿势、提交规范等见根目录 [CONTRIBUTING.md](../CONTRIBUTING.md)。
