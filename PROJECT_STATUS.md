# Project Status

## Current State
- **Rust version is `main`** — the Python implementation was removed; Rust port merged (issue #1 done)
  - All features: tray menu, hotkeys, window snapping, TOML config (`~/.windowsnap/config.toml`), startup toggle
  - 2.2MB release binary, no dependencies
- **Privacy policy**: hosted on GitHub Pages at https://paperhurts.github.io/window-snap/privacy.html
  (`gh-pages` branch, plain static HTML — landing page at site root; issue #2)
- **In flight**: `issue-3-process-match` branch (unpushed) — process-based window matching
  - Fixed: combined `title_contains` + `process_name` rules now AND (process was silently ignored)
  - First unit tests added (10, `cargo test`); default + live config updated to match terminals/browsers by exe name
  - Waiting on user live-test per tasks/user.md before push/PR

## Architecture (Rust)
- `src/main.rs` — App struct (winit event loop), tray menu, icon gen, hotkey dispatch, startup registry
- `src/config.rs` — Serde TOML config, path helpers, default config generation
- `src/windows.rs` — Win32 window enum, monitor detection, matching, slot calc, move (DWM border comp); unit tests at bottom
- `src/errors.rs` — MessageBoxW wrapper

## Open Issues
- #3: Process-name matching for terminals/browsers + combined-rule AND semantics (implemented, awaiting user test)
