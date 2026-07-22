# Core Guidelines

`happyjlc-core` is the shared Rust library for converting EasyEDA/LCSC components into KiCad symbols, footprints, and 3D model references.

Keep API access, importers, conversion, exporters, library management, and runner orchestration in their existing modules. Use `rustfmt`, typed project errors, and focused tests for observable behavior.

```bash
cargo check --all-targets
cargo test --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

Real EasyEDA calls are network-dependent. Tests must use deterministic fixtures or a mock HTTP server.
