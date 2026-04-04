# Project Status

## Current State
- **Python version**: stable on `main` branch, fully functional
- **Rust port**: on `rust-port` branch, compiles and builds (2.2MB release binary)
  - All features implemented: tray menu, hotkeys, window snapping, config, startup toggle
  - Config format: TOML (at `~/.windowsnap/config.toml`)
  - Needs live testing by user

## Architecture (Rust)
- `src/main.rs` — App struct (winit event loop), tray menu, icon gen, hotkey dispatch, startup registry
- `src/config.rs` — Serde TOML config, path helpers, default config generation
- `src/windows.rs` — Win32 window enum, monitor detection, matching, slot calc, move (DWM border comp)
- `src/errors.rs` — MessageBoxW wrapper

## Open Issues
- #1: Port WindowSnap to Rust (in progress)
