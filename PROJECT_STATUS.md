# Project Status

## Current State
- **Rust version is `main`** — the Python implementation was removed; Rust port merged (issue #1 done)
  - All features: tray menu, hotkeys, window snapping, TOML config (`~/.windowsnap/config.toml`), startup toggle
  - 2.2MB release binary, no dependencies
- **Privacy policy**: hosted on GitHub Pages at https://paperhurts.github.io/window-snap/privacy.html
  (`gh-pages` branch, plain static HTML — landing page at site root; issue #2)
- **v0.2.0 released 2026-07-16** (PR #4, issue #3 closed): process-based window matching
  - Combined `title_contains` + `process_name` rules now AND (process was silently ignored before)
  - First unit tests (11, `cargo test`); default + user's live config match terminals/browsers by exe name
  - User-verified live; binary attached to the GitHub release
  - Known behavior (issue #5): multiple windows matching one column → topmost in Z-order wins;
    pin a specific window with an AND rule like { process_name = "WindowsTerminal.exe", title_contains = "my-project" }

## Architecture (Rust)
- `src/main.rs` — App struct (winit event loop), tray menu, icon gen, hotkey dispatch, startup registry
- `src/config.rs` — Serde TOML config, path helpers, default config generation
- `src/windows.rs` — Win32 window enum, monitor detection, matching, slot calc, move (DWM border comp); unit tests at bottom
- `src/errors.rs` — MessageBoxW wrapper

## Open Issues
- #5: Multi-match ambiguity — same layout can move a different window each run (document Z-order behavior + AND-rule pinning pattern)
- #6: Release builds have no logging sink — wire up the unused `Config::log_path()`
