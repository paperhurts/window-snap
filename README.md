# WindowSnap

Custom window arrangement system tray app for Windows 11. Because Microsoft's snap layouts are bad and you deserve better.

## Setup

```bash
# Install dependencies
pip install -r requirements.txt

# Run
python windowsnap.py
```

Or run as a module:

```bash
python -m windowsnap
```

## Usage

Once running, WindowSnap lives in your system tray (bottom-right, near the clock).

**Left-click** the tray icon to see the layout menu.
**Right-click** for the full menu including settings.

### Default Hotkeys

| Hotkey | Layout | Description |
|--------|--------|-------------|
| `Ctrl+Alt+1` | 4-Column Dev | Claude \| VS Code \| Terminal \| Browser |
| `Ctrl+Alt+2` | 2-Column Split | Two equal columns |
| `Ctrl+Alt+3` | Main + Sidebar | 65% main \| 35% sidebar |
| `Ctrl+Alt+4` | Fullscreen | Single window maximized |
| `Ctrl+Alt+5` | 3-Column | Three equal columns |

### Tray Menu Options

- **Layout names** — click to apply that layout
- **Refresh Window List** — re-scan open windows (logged to console/log file)
- **Edit Config** — opens `~/.windowsnap/config.yaml` in your default editor
- **Reload Config** — hot-reload config without restarting the app
- **Start with Windows** — toggle auto-start on login
- **Quit** — exit the app

## Customizing Layouts

All configuration lives in `~/.windowsnap/config.yaml`. Edit it directly — no GUI needed.

### Adding a Layout

```yaml
layouts:
  my-layout:
    hotkey: ctrl+alt+6          # optional
    monitor: 0                  # 0 = primary, 1 = secondary
    columns:
      - width_percent: 40
        match:
          - title_contains: "Slack"
      - width_percent: 60
        match:
          - title_contains: "Chrome"
```

### Window Matching

Each column has `match` rules that find windows by title:

```yaml
match:
  - title_contains: "Chrome"    # matches any window with "Chrome" in the title
  - title_contains: "Firefox"   # rules are OR'd — first match wins
```

- Rules are **case-insensitive**
- Multiple rules per column are OR'd (first match wins)
- `match: []` (empty) means "skip this slot"
- If a matched window is minimized, it gets restored automatically

### Multi-Monitor

Set `monitor: 0` for primary, `monitor: 1` for secondary, etc. Monitor detection is automatic.

### Gaps

The `gap` setting controls pixel spacing between windows:

```yaml
settings:
  gap: 5    # 0 for no gaps, 10 for more breathing room
```

## Windows Startup

Use the "Start with Windows" toggle in the tray menu, or manually add a shortcut to:

```
pythonw.exe "C:\path\to\windowsnap.py"
```

in your Startup folder (`shell:startup`).

## Logging

Logs are written to `~/.windowsnap/windowsnap.log` and stdout (if running from a terminal).

## Troubleshooting

**Hotkeys not working?** The `keyboard` library may need admin privileges. Try running from an elevated terminal.

**Windows not moving?** Some apps (UWP, certain Electron apps) resist resizing. WindowSnap does best-effort.

**Wrong monitor?** Check `monitor: N` in your layout config. Run "Refresh Window List" to see what WindowSnap detects.
