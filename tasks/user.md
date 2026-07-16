# Testing: Process-Based Window Matching (issue #3)

## What Changed
Branch: `issue-3-process-match` (not pushed — waiting on your test confirmation).

1. **Engine fix** (`src/windows.rs`): a match rule with BOTH `title_contains` and `process_name` now requires both (AND). Previously the process name was silently ignored if a title was set. Title-only and process-only rules behave exactly as before.
2. **First unit tests** in the repo: 10 tests covering `match_window` and `calculate_slots` (`cargo test`).
3. **Default config** (baked into the binary for first-run) now matches terminals and browsers by process name.
4. **Your live config** (`~/.windowsnap/config.toml`) was updated: the terminal columns in `4-column-dev`, `dev-lite`, and `claude-cli` now match `WindowsTerminal.exe` / `powershell.exe` / `pwsh.exe` / `cmd.exe` first, with the old title rules kept as fallbacks. Nothing else changed. **Backup at `~/.windowsnap/config.toml.bak`** — restore by copying it back.

## How to Test
1. Rebuild and restart the app (the engine fix is in the binary):
   ```
   cd C:\dev\window-snap
   cargo build --release
   ```
   Quit the running WindowSnap from the tray, then start `target\release\window-snap.exe`.
   (No restart needed for the config change alone — "Reload Config" covers that — but the AND fix needs the new binary.)
2. Rename a PowerShell window: in PowerShell run
   `$Host.UI.RawUI.WindowTitle = "totally not a shell"`
   (In Windows Terminal you can also just rename the tab.)
3. Press **Ctrl+Alt+1** (4-column-dev).
4. **Expected**: the renamed PowerShell/Terminal window still snaps into column 3. Before this change it was skipped.
5. Also spot-check Ctrl+Alt+2 and Ctrl+Alt+5 — their terminal columns got the same treatment.

When it works, say so and I'll push the branch and open the PR / close issue #3.

---

# Testing: Privacy Policy on GitHub Pages

## What Changed
A privacy policy is now hosted via GitHub Pages (issue #2). It lives on a new orphan `gh-pages` branch — `main` and `rust-port` code are untouched. The policy states WindowSnap collects no data (no telemetry, no network access, local-only config).

## How to Test
1. Open https://paperhurts.github.io/window-snap/privacy.html — the policy should load, styled, in both light and dark mode (follows system theme).
2. Open https://paperhurts.github.io/window-snap/ — a small landing page linking to the repo and the policy.
3. Read the policy content and confirm it matches reality — especially if the app ever gains an update checker or any network feature, this page must be updated first.
4. Use the privacy.html URL in any store listing form that asks for a privacy policy.

---

# Testing: WindowSnap Rust Port

## What Changed
The entire Python app has been ported to Rust on the `rust-port` branch. Same functionality, new binary. Config format changed from YAML to TOML.

## How to Test

### 1. Build & Run
```bash
cd C:\dev\window-snap
cargo run
```
You'll see log output in the terminal. A tray icon (teal circle with white grid) should appear.

### 2. First-Run Config
- On first run, it creates `~/.windowsnap/config.toml` with default layouts
- Check that the file exists at `C:\Users\paper\.windowsnap\config.toml`
- **Note**: This is a TOML file, not the old YAML. Your existing YAML config is unaffected.

### 3. Tray Menu
- **Right-click** the tray icon — you should see:
  - Layout names (4 Column Dev, Bluestacks, Chat Browse)
  - Edit Config / Reload Config
  - Start with Windows (toggle)
  - Quit

### 4. Test Layouts
- Open several windows (Claude, VS Code, a terminal, a browser)
- Click "4 Column Dev" from the tray menu, or press **Ctrl+Alt+1**
- Windows should snap into 4 equal columns with 5px gaps

### 5. Test Hotkeys
- **Ctrl+Alt+1** → 4 Column Dev
- **Ctrl+Alt+2** → Chat Browse
- **Ctrl+Alt+3** → Bluestacks

### 6. Test Config Operations
- Click "Edit Config" — should open config.toml in your default editor
- Change `gap = 5` to `gap = 10`
- Click "Reload Config" — re-apply a layout, gaps should be wider
- Change it back, reload again

### 7. Test Startup Toggle
- Click "Start with Windows" — should show checkmark
- Click again — checkmark should disappear
- Verify via: `reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v WindowSnap`

### 8. Release Build
```bash
cargo build --release
# Binary at target/release/window-snap.exe (2.2MB)
# No console window in release mode
```

## Expected Behavior
- Identical to the Python version: same layouts, same hotkeys, same tray menu
- Config is TOML instead of YAML, but the structure is the same
- Much faster startup, smaller binary, no Python dependency
