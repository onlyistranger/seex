# HappyJLC Desktop

这是一个 Tauri 2 + TypeScript/Vite 桌面应用，负责剪贴板监听、LCSC 编号提取、KiCad 库管理和导出。导出逻辑由 `happyjlc-core` 提供，桌面端不再启动外部转换 CLI。

关键模块：

- `src-tauri/src/controller.rs`：应用状态与业务流程。
- `src-tauri/src/monitor.rs`：剪贴板监听与状态模型。
- `src-tauri/src/config.rs`：`export_config.json` 配置读写。
- `src-tauri/src/export.rs`：共享核心导出适配器。
- `src/main.ts`：单文件 SPA 和 Tauri 事件处理。

macOS 配置目录为 `~/Library/Application Support/com.happyjlc.desktop/`。使用根目录 `justfile` 的任务进行构建和测试。
