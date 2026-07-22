# HappyJLC 开发指南

这份文档面向维护 HappyJLC 的开发者，描述 workspace 边界、数据流、验证方式和发布流程。产品介绍和用户操作请先看根目录 [README](../README.md) 与 [使用指南](usage.md)。

## Workspace

```text
apps/desktop/             # Tauri 2 + TypeScript 桌面应用
  src/                    # Vite 前端和交互界面
  src-tauri/              # Rust 后端、状态管理、库存和 Tauri 命令
crates/core/              # EasyEDA/LCSC -> KiCad 共享转换核心
crates/cli/               # happyjlc 命令行入口
docs/                     # 用户和开发者文档
.github/workflows/        # 唯一的仓库级 CI / release
Cargo.toml                # Rust workspace 配置
justfile                  # 本地开发任务
```

桌面端和 CLI 都依赖 `happyjlc-core`。转换逻辑应集中放在 `crates/core/src/`，不要在桌面端复制另一套转换实现。

## 核心数据流

```text
剪贴板变化
  → clipboard listener
  → LCSC ID 提取与去重
  → MonitorState / 导出队列
  → Tauri command 或 CLI RunRequest
  → EasyEDA API
  → 内部 symbol / footprint / model 模型
  → KiCad exporters
  → 库文件、模型文件和 checkpoint
```

桌面端库存流程在导出库文件后扫描元件来源和元数据，再通过库存数据库维护数量、库位与 BOM 生产记录。

## 模块职责

### `crates/core`

- `easyeda/`：EasyEDA API、数据模型和 SVG/封装解析。
- `symbol_converter.rs`：符号转换流水线。
- `footprint_converter.rs`：封装转换流水线。
- `model_converter.rs`：3D 模型下载、转换和引用生成。
- `kicad/`：KiCad 输出模型和 exporters。
- `runner.rs`：串行/并行执行和 checkpoint 续跑。
- `library.rs`：输出目录、库文件和覆盖策略。

### `crates/cli`

负责 clap 参数解析、请求构建、日志初始化、并行进度展示和退出码处理。转换行为必须通过 `happyjlc-core` 完成。

### `apps/desktop/src-tauri`

负责 Tauri 命令、剪贴板监听、桌面配置、库扫描、库存数据库和导出任务调度。Tauri 命令统一在 `lib.rs` 注册。

### `apps/desktop/src`

负责页面布局、中文文案、状态渲染、事件监听、队列操作和 3D 模型预览。前端通过 `invoke` 调用后端命令，通过 Tauri event 接收状态和进度变化。

## 状态与并发约定

- 桌面运行状态统一通过 `MonitorState` 和 `Mutex` 管理。
- 不要在跨 `await` 的范围内持有状态锁。
- 导出过程通过进度和完成事件更新前端，不要让前端轮询具体实现细节。
- 库存数据库操作集中在 `InventoryRepository`，导入 BOM 前先完成字段和数量校验。
- 新增可观察行为时，为状态转换、配置读写、导出选项和错误路径补测试。

## 本地验证

完整验证：

```bash
just verify
```

等价的独立任务：

```bash
just fmt-check
just check
just test
just clippy
just frontend-build
```

核心 crate：

```bash
cargo check -p happyjlc-core --all-targets
cargo test -p happyjlc-core --all-targets
cargo clippy -p happyjlc-core --all-targets -- -D warnings
```

网络测试必须使用 mock server 或固定 fixture，不能依赖实时 EasyEDA 服务。前端构建会检查 TypeScript 类型并生成 Vite production bundle。

## CI 与 Release

根目录 `.github/workflows/ci.yml` 负责：

- Rust 格式、check、test 和 Clippy。
- 前端依赖安装和 production build。

根目录 `.github/workflows/release.yml` 在 `v*` tag 上运行，负责构建：

- macOS Apple Silicon / Intel DMG。
- Windows NSIS 安装包。
- Linux、macOS、Windows CLI 二进制。

构建产物统一以 HappyJLC 命名，并在最后创建 GitHub Release。添加新发布平台时，需要同时更新构建矩阵、产物路径和 README 的下载说明。

## 许可证和来源

HappyJLC 使用 CC BY-NC 4.0，原始项目来源和相关项目见根目录 [LICENSE](../LICENSE) 与 [README 的致谢部分](../README.md#致谢与相关项目)。
