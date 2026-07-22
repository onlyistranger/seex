# Desktop App Guidelines

HappyJLC desktop is a Tauri 2 application for monitoring the clipboard, extracting LCSC component IDs, and exporting KiCad libraries through the shared `happyjlc-core` crate.

## Structure

- `src/`: TypeScript/Vite single-page frontend.
- `src-tauri/src/lib.rs`: Tauri commands and application entry point.
- `src-tauri/src/controller.rs`: state and workflow orchestration.
- `src-tauri/src/monitor.rs`: clipboard monitoring state and listener.
- `src-tauri/src/config.rs`: `AppConfig`, `ExportConfig`, and monitor settings.
- `src-tauri/src/export.rs`: adapter from desktop export state to `happyjlc-core`.

## Commands

```bash
npm run dev
npm run build
cd src-tauri && cargo check
cd src-tauri && cargo test
```

From the workspace root, prefer `just check`, `just test`, and `just frontend-build`.

Tauri commands must be registered in `src-tauri/src/lib.rs`. Do not hold the monitor mutex across an await point. Add focused Rust tests for changed state, config, parsing, and export behavior.
