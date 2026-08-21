# Testing: Overlapping Windows (issue #9)

## What Changed
Branch: `issue-9-overlap-columns` (builds on the #8 branch). Not pushed.

Columns could only ever tile — each starting where the last ended — so every extra
column made everything narrower, and widths over 100% ran off screen rather than
overlapping. A column can now carry **`x_percent`**, which pins it to an absolute
position instead. Absolute columns sit outside the tiling flow: they do not consume
a gap, do not shift their neighbours, and may overlap them. Columns declared later
are stacked on top. **A layout with no `x_percent` behaves exactly as before.**

## Your config was rewritten (backup: `~/.windowsnap/config.toml.pre-overlap`)
`4-column-dev` (Ctrl+Alt+1) is now, based on your answer that doc-md, the browser,
and the terminal/editor are what you need readable at once:

| Column | Width | Actual px |
| --- | --- | --- |
| doc-md | 40% tiled | 1030 |
| VS Code **or** terminal (whichever is open) | 30% tiled | 776 |
| Browser | 30% tiled | 776 |
| Claude | `x_percent = 0`, 24% — **laid on top of doc-md** | 616 |

Your browser went from ~380px to 776px. Claude no longer costs a column.
The other four layouts are unchanged (still pure tiling) so you can compare.

## Two things I measured that you should know
- **Your monitor is 2560 wide**, not 1920. Earlier percentages I quoted were off.
- **The Claude app enforces a ~616px minimum width.** I asked for 255px and got 616.
  That is Claude, not WindowSnap. Its column is set to 24% so the config asks for
  what it will actually get. It also means Claude covers ~60% of doc-md when
  stacked — if that bothers you, move it with `x_percent` or drop it from the layout.

## How to Test
1. Quit the WindowSnap in your tray (it is still the old build).
2. Launch `target\verify\release\window-snap.exe`.
3. Press **Ctrl+Alt+1**. Expect: doc-md wide on the left, terminal/VS Code and the
   browser each ~776px, and Claude sitting on top of doc-md's left edge.
4. Press **Ctrl+Alt+2** (dev-lite) for the old pure-tiled behaviour to compare.
5. Check `%USERPROFILE%\.windowsnap\windowsnap.log` — you should see
   `Layout '4-column-dev' has absolute columns: stacked 4 window(s) in column order`
   and no WARN lines.

## Already verified end-to-end
Ran the build on a spare hotkey so it would not fight your running instance, then
read back the real window rectangles: Claude spans -2..614 while doc-md spans
-2..1028, so the overlap is real and every tiled column kept its full width.
28 tests pass, including one that proves a layout without `x_percent` is unchanged.

## To tune it
`x_percent` is the left edge, `width_percent` the width, both percentages of the
screen. Keep their sum at or under 100 or the column hangs off the right edge
(WindowSnap warns in the log if it does). Tray -> Reload Config to apply.

---

# Testing: Config Validation Warnings (issue #8)

## What Changed
Branch: `issue-8-validate-layout-widths`. Code change is in `src/config.rs` only.

1. **`Config::validate()`** — on every load and every "Reload Config", WindowSnap now
   writes a WARN to `~/.windowsnap/windowsnap.log` when a layout is broken:
   - widths summing **over** 100% (columns tile left-to-right, so the excess runs off
     the right edge and the last column gets a *negative* width)
   - a layout with **no columns** (usually a pasted `[[layouts.<name>.columns]]` block
     that named the wrong layout)
2. **Under 100% does NOT warn.** The last column stretches to absorb the slack, so the
   layout still fills the screen. The rule is "stays on screen", not "sums to 100".
3. **One INFO line per load** showing layout shape up front:
   `Loaded config: 5 layout(s): 4-column-dev (5 cols), bluestacks (4 cols), ...`
   This is what would have made the doc-md bug obvious in one glance.
4. Warnings never block loading — one bad layout must not lock you out of the others.
5. Config template comments updated with both gotchas. No behavior change to snapping.

## Verification already done
- `cargo test` — 20 pass (6 new in `config.rs`, 2 new in `windows.rs`).
  The two `windows.rs` tests prove the engine claims: an underfull layout still reaches
  the screen edge, and an overfull one produces an off-screen column with negative width.
- **End-to-end against your real broken config**: ran the new binary with the pre-fix
  `config.toml` and confirmed the log emitted
  `WARN ... Layout '4-column-dev' has 8 column(s) whose width_percent sums to 300%`.
  Your live config was restored immediately afterward (hash-verified identical).

## How to Test
1. The **old** binary is still running in your tray — this build is at
   `targeterify
   `target\verify\release\window-snap.exe` (built to a side directory so it wouldn't
   have to kill your running instance).
2. Quit the tray app, then launch the new binary.
3. Open `%USERPROFILE%\.windowsnap\windowsnap.log` — you should see the `Loaded config:`
   line listing every layout and its column count, and **no warnings** (your config is
   currently valid).
4. To see a warning fire: bump any `width_percent` up by 50, tray → **Reload Config**,
   and check the log tail.

## Still open
Your point that a 15%-wide browser is useless is **not solved by this change**. The
engine cannot overlap windows at all today — see issue #9. Your config widths are
untouched pending that decision.

---

# Testing: Release Logging + Multi-Match Docs (issues #5, #6)

## What Changed
Branch: `issue-5-6-logging-and-match-docs` (PR closes both issues). v0.3.0.

1. **Release builds now log** to `~/.windowsnap/windowsnap.log` (rotates at 512 KB to `windowsnap.log.1`). Debug `cargo run` still logs to the terminal. Each layout application logs which window went where, with process name and hwnd.
2. **Multi-match behavior documented** (no logic change): topmost matching window wins a slot. README + config comments now show the AND-rule pinning pattern: `{ process_name = "WindowsTerminal.exe", title_contains = "my-project" }`.
3. Heads-up: your log file had 71 KB of old Python-version entries from February; the Rust app now appends below them. It'll rotate away naturally.

## How to Test (already machine-verified end-to-end)
I launched the new build and fired Ctrl+Alt+1 programmatically — the log shows all 4 columns placed with process names. If you want to see it yourself:
1. The new build is already running in your tray (v0.3.0 binary).
2. Press **Ctrl+Alt+1**, then open `%USERPROFILE%\.windowsnap\windowsnap.log` — the tail shows the layout application and exactly which window landed in each column.
3. Optional: add a pinning rule for your project terminal (see README "When Several Windows Match").

---

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
