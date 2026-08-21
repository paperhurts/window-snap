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

- **v0.3.0 (2026-07-16, issues #5 + #6)**: release builds log to `~/.windowsnap/windowsnap.log`
  (rotates at 512 KB; per-column placements logged with process + hwnd); multi-match behavior
  documented — topmost matching window wins, pin with an AND rule (`process_name` + `title_contains`)

## Open Issues
- **#8** (bug) — config validation. **Implemented**, branch `issue-8-validate-layout-widths`,
  awaiting user test confirmation before push.
- **#9** (enhancement) — overlapping windows via `x_percent`. **Implemented**, branch
  `issue-9-overlap-columns` (stacked on #8), awaiting user test confirmation before push.
  Deferred within #9: `y_percent`/`height_percent`, and explicit z-order control
  (z-order currently follows config order).
- **#10** (enhancement) — a column places only one matching window; the rest are
  ignored. Surfaced by `chat-browse` resizing only the topmost of 5 Brave windows.
  Documented behaviour (#5), but keeps surprising. Proposal: opt-in `match_all` that
  stacks every match into the slot, now practical because #9 landed. Not started —
  awaiting a decision on stack-vs-cascade.

## In Flight (2026-08-21)
- **Config validation (#8)**: `Config::validate()` warns at load/reload when a layout
  cannot fit on screen — tiled widths summing >100, absolute columns running past the
  right edge, or a layout with no columns. Plus one INFO line per load listing every
  layout and its column count. Warnings never block loading. The rule is deliberately
  "stays on screen", **not** "sums to 100": under 100 is fine because the last tiled
  column absorbs the slack.
- **Overlap (#9)**: `Column.x_percent` places a column absolutely instead of tiling.
  Absolute columns consume no gap, do not shift neighbours, and may overlap. Z-order
  is applied in column order (last declared on top) only for layouts that use the
  field. Fully backward compatible.
- Tests: 12 → 28.
- User's live config rewritten twice today; backups at `config.toml.pre-docmd-fix`
  and `config.toml.pre-overlap`.

## Measured Environment Facts (2026-08-21)
- Primary monitor work area is **2560** wide (earlier notes assuming 1920 were wrong).
- The **Claude desktop app clamps itself to ~616px minimum width** — requesting 255px
  yields 616px. Not a WindowSnap bug; size its column accordingly.
- `SetWindowPos` in `move_window` passes `SWP_NOZORDER`, so moving a window never
  changes stacking. Overlap support needed a separate raise pass.
