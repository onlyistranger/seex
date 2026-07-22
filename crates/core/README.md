# HappyJLC Core

`happyjlc-core` 是 HappyJLC workspace 的共享转换核心，负责将 EasyEDA/LCSC 元件数据转换为 KiCad 符号、封装和 3D 模型引用。

## 模块

- `easyeda/`：EasyEDA API、数据模型和 SVG/封装解析。
- `symbol_converter.rs`：符号转换。
- `footprint_converter.rs`：封装转换。
- `model_converter.rs`：3D 模型下载和引用生成。
- `kicad/`：KiCad 输出模型和 exporter。
- `runner.rs`：并行、串行和 checkpoint 续跑。
- `library.rs`：输出目录和覆盖策略。

## 输出

```text
<output>/
├── <library>.kicad_sym
├── <library>.pretty/<component>.kicad_mod
├── <library>.3dshapes/<model>.<step|stp|wrl>
└── .checkpoint
```

3D 模型路径支持 KiCad 环境变量、`${KIPRJMOD}` 项目相对路径和库相对路径。符号、封装和模型可以分别控制覆盖行为。

## 测试

核心测试必须使用固定数据或 mock HTTP server。常用命令：

```bash
cargo check -p happyjlc-core --all-targets
cargo test -p happyjlc-core --all-targets
cargo clippy -p happyjlc-core --all-targets -- -D warnings
```

## 许可证

CC BY-NC 4.0，详见仓库根目录 [`LICENSE`](../../LICENSE)。
