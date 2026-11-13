#![cfg_attr(windows, windows_subsystem = "windows")]
//! DockX 2.0 — macOS-style dock + Dynamic Island for Windows, by X44TO.

fn main() {
    #[cfg(windows)]
    dockx::win::run();
    #[cfg(not(windows))]
    eprintln!("DockX runs on Windows. Use the `demo` binary to render the preview video.");
}
