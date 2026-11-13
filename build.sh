#!/bin/sh
# Cross-builds DockX.exe for Windows from Linux (Rust 1.97 + MinGW).
set -e
cd "$(dirname "$0")"
export PATH=/usr/lib/rust-1.97/bin:$PATH
RUSTC_BOOTSTRAP=1 cargo-1.97 build --release --bin DockX \
  -Zbuild-std=std,panic_abort \
  --target x86_64-pc-windows-gnu "$@"
