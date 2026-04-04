# WindowSnap

Custom window arrangement system tray app for Windows 11. Because Microsoft's snap layouts are bad and you deserve better.

## Setup

```bash
# Build
cargo build --release

# Run (release — no console window)
./target/release/window-snap.exe

# Run (debug — logs visible in terminal)
cargo run
```

The release binary is a single 2.2MB `.exe` with no dependencies.

## Usage

Once running, WindowSnap lives in your system tray (bottom-right, near the clock).

**Right-click** the tray icon for the full menu.

### Default Hotkeys

| Hotkey | Layout | Description |
|--------|--------|-------------|
| `Ctrl+Alt+1` | 4-Column Dev | Claude \| VS Code \| Terminal \| Browser |
| `Ctrl+Alt+2` | Dev Lite | Claude \| Browser (wide) \| Terminal |
| `Ctrl+Alt+3` | Chat Browse | Signal \| Claude \| Browser (wide) |
| `Ctrl+Alt+4` | BlueStacks | Up to 4 instances, 25% each |
| `Ctrl+Alt+5` | Claude CLI | Claude \| VS Code/Terminal (wide) \| Browser |

### Tray Menu Options

- **Layout names** — click to apply that layout
- **Edit Config** — opens `~/.windowsnap/config.toml` in your default editor
- **Reload Config** — hot-reload config without restarting the app
- **Start with Windows** — toggle auto-start on login
- **Quit** — exit the app

## Customizing Layouts

All configuration lives in `~/.windowsnap/config.toml`. Edit it directly — no GUI needed.

### Adding a Layout

```toml
[layouts.my-layout]
hotkey = "ctrl+alt+6"          # optional
monitor = 0                    # 0 = primary, 1 = secondary

[[layouts.my-layout.columns]]
width_percent = 40
match = [{ title_contains = "Slack" }]

[[layouts.my-layout.columns]]
width_percent = 60
match = [{ title_contains = "Chrome" }]
```

### Window Matching

Each column has `match` rules that find windows by title or process name:

```toml
match = [
    { title_contains = "Chrome" },    # matches any window with "Chrome" in the title
    { title_contains = "Firefox" },   # rules are OR'd — first match wins
    { process_name = "HD-Player.exe" },  # match by executable name
]
```

- Rules are **case-insensitive**
- Multiple rules per column are OR'd (first match wins)
- `match = []` (empty) means "skip this slot"
- If a matched window is minimized, it gets restored automatically

### Multi-Monitor

Set `monitor = 0` for primary, `monitor = 1` for secondary, etc. Monitor detection is automatic.

### Gaps

The `gap` setting controls pixel spacing between windows:

```toml
[settings]
gap = 5    # 0 for no gaps, 10 for more breathing room
```

## Windows Startup

Use the "Start with Windows" toggle in the tray menu. This adds/removes a registry entry that launches the exe on login.

## Logging

In debug mode (`cargo run`), logs go to stdout. The release build has no console window.

## Troubleshooting

**Hotkeys not working?** Some hotkey combinations may conflict with other apps. Try a different combo in your config.

**Windows not moving?** Some apps (UWP, certain Electron apps) resist resizing. WindowSnap does best-effort. Running as admin may help.

**Wrong monitor?** Check `monitor = N` in your layout config.
