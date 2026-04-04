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
