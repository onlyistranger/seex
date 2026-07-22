# AGENTS.md

@/Users/alddp/.codex/RTK.md

## 项目概述

这是 HappyJLC 的 KiCad 元件转换与管理 monorepo：

- `crates/core/`：EasyEDA/LCSC 到 KiCad 的共享转换核心。
- `crates/cli/`：`happyjlc` CLI 入口。
- `apps/desktop/`：Tauri 桌面应用，直接调用 `happyjlc-core`。

目标环境是 macOS + KiCad，许可证为 CC BY-NC 4.0。

## CodeGraph

仓库包含 `.codegraph/` 索引。探索代码和回答代码问题时，优先使用
`codegraph_explore`、`codegraph_node`、`codegraph_search`、`codegraph_callers`，仅在它们不覆盖时使用文本搜索或读取文件。

## 常用命令

```bash
just check
just test
just fmt-check
just clippy
just frontend-build
just tauri-dev
```

CLI 调试：

```bash
cargo run -p happyjlc-cli -- --help
cargo run -p happyjlc-cli -- --lcsc-id C2040 --full --output ./library
```

## 结构约定

- 转换逻辑放在 `crates/core/src/`，桌面端不要复制转换实现。
- Tauri 命令在 `apps/desktop/src-tauri/src/lib.rs` 注册。
- 桌面状态统一通过 `MonitorState` 和 `Mutex` 管理，避免跨 `await` 持有锁。
- 配置模型位于 `apps/desktop/src-tauri/src/config.rs`，导出配置使用 `export` 命名。
- macOS 原生配置目录为 `~/Library/Application Support/com.happyjlc.desktop/`。
- Rust 使用 `rustfmt` 和 `snake_case`；TypeScript 使用 `camelCase` 和 `PascalCase`。

## 测试

新增或修改解析、配置、状态转换和导出行为时，应补充针对可观察行为的测试。网络行为必须使用 mock 或 fixture，不能依赖真实 EasyEDA 服务。
