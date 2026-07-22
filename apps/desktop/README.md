# HappyJLC Desktop

HappyJLC 桌面应用用于监听剪贴板中的 LCSC 元件编号、管理已导入的 KiCad 库，并调用 workspace 中的 `happyjlc-core` 生成符号、封装和 3D 模型。

## 功能

- 监听剪贴板并提取 LCSC 编号。
- 管理历史记录、匹配结果和待导出队列。
- 分别启用符号、封装和 3D 模型导出。
- 分别控制各类资产的覆盖策略。
- 支持自动、项目相对和库相对 3D 模型路径。
- 扫描、筛选、编辑和删除已导入符号。
- 管理库存、位置和 BOM 导入。
- 保存窗口、监听和导出配置。

## 开发

从仓库根目录执行：

```bash
npm --prefix apps/desktop ci
just frontend-build
just tauri-dev
```

后端检查和测试：

```bash
cargo check -p happyjlc-desktop --all-targets
cargo test -p happyjlc-desktop --lib
```

## 数据位置

macOS 配置文件位于：

```text
~/Library/Application Support/com.happyjlc.desktop/export_config.json
```

桌面端直接调用 `happyjlc-core`，不需要安装或配置外部转换 CLI。

## 平台

主要验证环境是 macOS + KiCad。Windows 桌面端由 CI 生成，但当前不作为主要开发环境。

## 许可证

本项目使用 CC BY-NC 4.0，详见仓库根目录 [`LICENSE`](../../LICENSE)。
