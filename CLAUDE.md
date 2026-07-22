# HappyJLC 开发说明

HappyJLC 是一个 KiCad 元件转换与管理 monorepo。

## 项目结构

```text
apps/desktop/       # Tauri + TypeScript 桌面应用
crates/core/        # EasyEDA/LCSC -> KiCad 共享核心
crates/cli/         # happyjlc CLI
Cargo.toml          # workspace
justfile            # 常用开发任务
```

桌面端直接调用 `happyjlc-core`，CLI 使用同一个核心库。桌面配置文件名为 `export_config.json`，新 macOS 路径为 `~/Library/Application Support/com.happyjlc.desktop/`。

## 验证

```bash
just fmt-check
just check
just test
just frontend-build
```

真实导出会访问 EasyEDA 服务；测试必须使用 mock server 或固定 fixture。
