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

### Overlapping Windows

By default columns **tile**: each starts where the previous one ended, so adding a
column always makes every window narrower. On a single monitor that runs out fast —
five columns leaves nothing wide enough to read.

Give a column an `x_percent` to place it at an absolute position instead. It then sits
outside the tiling flow: it does not shift its neighbours, and it may overlap them.
Columns declared **later** are stacked on top.

```toml
[[layouts.focus.columns]]
width_percent = 45              # tiled as usual
match = [{ process_name = "doc-md.exe" }]

[[layouts.focus.columns]]
width_percent = 55              # tiled, fills the rest of the row
match = [{ title_contains = "Brave" }]

[[layouts.focus.columns]]
x_percent = 0                   # absolute: pinned to the left edge...
width_percent = 22              # ...and laid on top of the two above
match = [{ title_contains = "Claude" }]
```

Widths are literal percentages of the work area and are never normalized:

| Layout | Result |
| --- | --- |
| Tiled widths sum **under** 100 | Fine — the last tiled column stretches to fill the row. |
| Tiled widths sum **over** 100 | Broken — the excess runs off the right edge and the last column gets a negative width. This does *not* produce overlap. |
| `x_percent + width_percent` over 100 | That column hangs off the right edge. |

WindowSnap warns about all three in the log when it loads the config, and never
refuses to load — one bad layout must not lock you out of the others.

### Window Matching

Each column has `match` rules that find windows by title, process name, or both:

```toml
match = [
    { process_name = "WindowsTerminal.exe" },  # match by executable name (robust)
    { title_contains = "Chrome" },             # match by window title (fragile)
    { process_name = "powershell.exe", title_contains = "admin" },  # both = AND
]
```

- Rules are **case-insensitive** substring checks
- Multiple rules per column are OR'd (first match wins)
- A single rule with **both** `process_name` and `title_contains` requires both to match (AND)
- `match = []` (empty) means "skip this slot"
- If a matched window is minimized, it gets restored automatically

**Prefer `process_name` over `title_contains`** — titles change when you rename a
terminal, switch browser tabs, or open a different document; the exe name doesn't.
Common ones: `WindowsTerminal.exe`, `powershell.exe`, `pwsh.exe`, `cmd.exe`,
`brave.exe`, `chrome.exe`, `firefox.exe`, `msedge.exe`, `Code.exe`, `claude.exe`,
`Signal.exe`.

### When Several Windows Match

A column places exactly one window. If several windows match (say five terminals
and one terminal slot), the **topmost matching window** wins — roughly the one you
used most recently — preferring non-minimized windows. Re-applying a layout can
therefore pick a different terminal than last time.

To deterministically pin one specific window, combine process and title in a
single AND rule and list it first:

```toml
match = [
    { process_name = "WindowsTerminal.exe", title_contains = "my-project" },  # this one
    { process_name = "WindowsTerminal.exe" },                                 # else any terminal
]
```

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

- Debug mode (`cargo run`): logs go to the terminal.
- Release build (no console window): logs go to `~/.windowsnap/windowsnap.log`,
  including which window was matched and placed for every column. The file rotates
  once at 512 KB (previous log kept as `windowsnap.log.1`), so it stays under ~1 MB.
- Every load and every "Reload Config" logs one line summarising each layout and its
  column count, followed by a warning for any layout whose columns cannot fit on
  screen. If a layout is not doing what you expect, read that first — a column block
  pasted under the wrong `[[layouts.<name>.columns]]` header shows up here immediately.

## Troubleshooting

**Hotkeys not working?** Some hotkey combinations may conflict with other apps. Try a different combo in your config.

**Windows not moving?** Some apps (UWP, certain Electron apps) resist resizing. WindowSnap does best-effort. Running as admin may help.

**Wrong monitor?** Check `monitor = N` in your layout config.
