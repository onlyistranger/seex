# HappyJLC 使用指南

这份文档面向已经决定使用 HappyJLC 的用户，包含安装、桌面端、CLI、导出结果和本地数据说明。

## 环境要求

- Rust stable
- Node.js 20+
- npm
- [just](https://github.com/casey/just)
- macOS + KiCad（主要开发和验证环境）

## 桌面应用

从仓库根目录安装前端依赖并启动：

```bash
npm --prefix apps/desktop ci
just tauri-dev
```

只启动前端开发服务器：

```bash
just frontend-dev
```

桌面端提供以下页面和工作流：

- 监听：监听剪贴板，提取 LCSC 编号并维护历史/匹配队列。
- 元件库：扫描已导入的 `.kicad_sym`、封装和 3D 模型。
- 库存：维护数量、库位、生产记录和 BOM 扣减。
- 设置：配置监听规则、导出选项、库路径和窗口状态。

## CLI

查看帮助：

```bash
cargo run -p happyjlc-cli -- --help
```

转换单个元件：

```bash
cargo run -p happyjlc-cli -- \
  --lcsc-id C2040 \
  --full \
  --output ./library
```

批量转换：

```bash
cargo run -p happyjlc-cli -- \
  --batch ./parts.txt \
  --full \
  --parallel 4 \
  --continue-on-error \
  --output ./library
```

### 主要参数

| 参数 | 作用 |
| --- | --- |
| `--lcsc-id Cxxxxx` | 转换单个 LCSC 元件。 |
| `--batch FILE` | 从文本文件批量读取元件编号。 |
| `--full` / `--symbol` / `--footprint` / `--3d` | 选择导出资产。 |
| `--output DIR` | 指定输出目录。 |
| `--overwrite` | 覆盖已有的所有资产。 |
| `--overwrite-symbol` / `--overwrite-footprint` / `--overwrite-3d` | 分别控制资产覆盖。 |
| `--project-relative` | 使用 `${KIPRJMOD}` 生成项目相对 3D 路径。 |
| `--symbol-fill-color HEX` | 设置符号填充颜色。 |
| `--parallel N` | 设置批处理并行下载数。 |
| `--continue-on-error` | 批处理遇到失败时继续执行。 |

## 导出结果

```text
<output>/
├── <library>.kicad_sym
├── <library>.pretty/
│   └── <component>.kicad_mod
├── <library>.3dshapes/
│   └── <model>.<step|stp|wrl>
└── .checkpoint
```

符号、封装和 3D 模型可以分别选择；重复执行批处理时，checkpoint 会帮助跳过已经完成的资产。启用覆盖选项后，会按对应资产的覆盖策略重新生成。

## 3D 模型路径

桌面端和 CLI 支持以下路径模式：

- KiCad 环境变量或默认库路径。
- 项目相对路径 `${KIPRJMOD}`。
- 输出库相对路径。

导出后可以在桌面端元件库页面直接预览模型，建议在实际用于 PCB 前确认模型方向、比例和引脚位置。

## 配置与数据

macOS 默认配置文件：

```text
~/Library/Application Support/com.happyjlc.desktop/export_config.json
```

应用数据目录还可能包含：

- 监听历史和匹配结果。
- checkpoint 文件。
- 默认导出目录。
- `inventory.db` 库存数据库。

这些数据属于本地运行数据，不应提交到仓库。迁移旧版本时只迁移配置文件，不要覆盖新的库存数据库或用户导出目录。

## 常见问题

### 没有提取到 LCSC 编号

检查监听页面的关键词或正则规则，并确认剪贴板内容中包含 `C` 开头的 LCSC 编号。

### 已有元件没有被覆盖

确认是否启用了对应资产的覆盖选项。符号、封装和 3D 模型的覆盖策略可以分别设置。

### 3D 模型没有显示

确认模型文件已经导出，检查 KiCad 路径模式和模型文件扩展名，并在元件库页面重新扫描。

### 批处理失败后如何继续

保留输出目录和 checkpoint，重新执行同一批处理任务即可；如需强制重新生成，再显式启用对应覆盖选项。
