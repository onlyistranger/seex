set dotenv-load := false

default:
    @just --list

# Rust checks for the complete workspace.
check:
    cargo check --workspace --all-targets

test:
    cargo test --workspace --all-targets

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Frontend type checking and production build.
frontend-build:
    npm --prefix apps/desktop run build

frontend-dev:
    npm --prefix apps/desktop run dev

cli-help:
    cargo run -p happyjlc-cli -- --help

cli-run *args:
    cargo run -p happyjlc-cli -- {{args}}

# Full Tauri development session.
tauri-dev:
    npm --prefix apps/desktop run tauri dev

build: check frontend-build

verify: fmt-check check test clippy frontend-build

clean:
    cargo clean
