"""WindowSnap diagnostic — run this to see what the app sees."""

import sys
import ctypes

# DPI awareness first
try:
    ctypes.windll.shcore.SetProcessDpiAwareness(2)
except Exception:
    try:
        ctypes.windll.user32.SetProcessDPIAware()
    except Exception:
        pass

import win32gui
import win32con
import win32process
import win32api
from ctypes import wintypes

try:
    import psutil
    HAS_PSUTIL = True
except ImportError:
    HAS_PSUTIL = False
    print("WARNING: psutil not installed — process names won't be available")

try:
    import yaml
except ImportError:
    print("WARNING: PyYAML not installed")
    yaml = None


def main():
    print("=" * 70)
    print("WindowSnap Diagnostic")
    print("=" * 70)

    # 1. Monitor detection
    print("\n--- MONITORS ---")
    monitors = []

    try:
        raw_monitors = win32api.EnumDisplayMonitors()
        for hmonitor, _hdc, _rect in raw_monitors:
            info = win32api.GetMonitorInfo(hmonitor)
            work = info["Work"]
            monitor = info["Monitor"]
            is_primary = bool(info["Flags"] & 1)
            monitors.append({
                "handle": hmonitor,
                "work_area": work,
                "full_rect": monitor,
                "is_primary": is_primary,
            })
    except Exception as e:
        print(f"  FAILED to enumerate monitors: {e}")
        return

    for i, m in enumerate(monitors):
        tag = " (PRIMARY)" if m["is_primary"] else ""
        print(f"  Monitor {i}{tag}:")
        print(f"    Full rect:  {m['full_rect']}")
        print(f"    Work area:  {m['work_area']}  (excludes taskbar)")
        wa = m["work_area"]
        print(f"    Usable:     {wa[2] - wa[0]}x{wa[3] - wa[1]} px")

    if not monitors:
        print("  NO MONITORS DETECTED — this is the problem")
        return

    # 2. Window enumeration
    print("\n--- ALL VISIBLE WINDOWS ---")
    windows = []

    skip_classes = {
        "Shell_TrayWnd", "Shell_SecondaryTrayWnd",
        "Progman", "WorkerW",
        "Windows.UI.Core.CoreWindow",
        "tooltips_class32",
        "IME", "MSCTFIME UI",
    }

    def win_callback(hwnd, _):
        if not win32gui.IsWindowVisible(hwnd):
            return True
        title = win32gui.GetWindowText(hwnd)
        if not title:
            return True

        try:
            cls = win32gui.GetClassName(hwnd)
        except Exception:
            cls = "?"

        if cls in skip_classes:
            return True

        ex_style = win32gui.GetWindowLong(hwnd, win32con.GWL_EXSTYLE)
        if ex_style & win32con.WS_EX_TOOLWINDOW:
            return True

        if not (ex_style & win32con.WS_EX_APPWINDOW):
            owner = win32gui.GetWindow(hwnd, win32con.GW_OWNER)
            if owner:
                return True

        process_name = ""
        if HAS_PSUTIL:
            try:
                _, pid = win32process.GetWindowThreadProcessId(hwnd)
                proc = psutil.Process(pid)
                process_name = proc.name()
            except Exception:
                pass

        is_min = bool(win32gui.IsIconic(hwnd))
        rect = win32gui.GetWindowRect(hwnd)

        windows.append({
            "hwnd": hwnd,
            "title": title,
            "class": cls,
            "process": process_name,
            "minimized": is_min,
            "rect": rect,
        })
        return True

    win32gui.EnumWindows(win_callback, None)

    if not windows:
        print("  NO WINDOWS FOUND — this is the problem")
        return

    for w in windows:
        tag = " [MINIMIZED]" if w["minimized"] else ""
        print(f"  hwnd={w['hwnd']:8d}  [{w['process'] or w['class']:20s}]  \"{w['title']}\"{tag}")

    # 3. Match test against default config
    print(f"\n  Total: {len(windows)} windows found")

    print("\n--- MATCH TEST (4-Column Dev) ---")
    match_tests = [
        ("Column 0", ["Claude"]),
        ("Column 1", ["Visual Studio Code"]),
        ("Column 2", ["Terminal", "PowerShell", "Command Prompt"]),
        ("Column 3", ["Chrome", "Firefox", "Edge"]),
    ]

    for col_name, searches in match_tests:
        found = None
        for search in searches:
            for w in windows:
                if search.lower() in w["title"].lower():
                    found = (search, w)
                    break
            if found:
                break

        if found:
            print(f"  {col_name}: MATCHED '{found[0]}' -> \"{found[1]['title']}\"")
        else:
            print(f"  {col_name}: NO MATCH for {searches}")
            # Show what titles we DO have, to help find the right match strings
            print(f"           Available titles containing partial matches:")
            for s in searches:
                sl = s.lower()
                for w in windows:
                    # Show titles that even partially overlap
                    if any(word in w["title"].lower() for word in sl.split()):
                        print(f"             ~ \"{w['title']}\"")

    # 4. Try actually moving a window
    print("\n--- MOVE TEST ---")
    if windows:
        test_win = windows[0]
        hwnd = test_win["hwnd"]
        original_rect = win32gui.GetWindowRect(hwnd)
        print(f"  Testing with: \"{test_win['title']}\"")
        print(f"  Current position: {original_rect}")

        # Try to move it 10 pixels and back
        try:
            if win32gui.IsIconic(hwnd):
                win32gui.ShowWindow(hwnd, win32con.SW_RESTORE)

            placement = win32gui.GetWindowPlacement(hwnd)
            if placement[1] == win32con.SW_SHOWMAXIMIZED:
                win32gui.ShowWindow(hwnd, win32con.SW_RESTORE)
                print("  (restored from maximized)")

            result = win32gui.SetWindowPos(
                hwnd,
                win32con.HWND_TOP,
                original_rect[0] + 10,
                original_rect[1],
                original_rect[2] - original_rect[0],
                original_rect[3] - original_rect[1],
                win32con.SWP_NOZORDER | win32con.SWP_NOACTIVATE,
            )
            new_rect = win32gui.GetWindowRect(hwnd)
            print(f"  After move:       {new_rect}")

            if new_rect[0] != original_rect[0]:
                print("  MOVE WORKS! Window shifted 10px right.")
                # Move it back
                win32gui.SetWindowPos(
                    hwnd,
                    win32con.HWND_TOP,
                    original_rect[0],
                    original_rect[1],
                    original_rect[2] - original_rect[0],
                    original_rect[3] - original_rect[1],
                    win32con.SWP_NOZORDER | win32con.SWP_NOACTIVATE,
                )
                print("  (moved back to original position)")
            else:
                print("  MOVE FAILED — SetWindowPos didn't change the position.")
                print("  This window might resist repositioning, or there's a permissions issue.")
        except Exception as e:
            print(f"  MOVE ERROR: {e}")

    # 5. Config check
    print("\n--- CONFIG CHECK ---")
    from pathlib import Path
    config_path = Path.home() / ".windowsnap" / "config.yaml"
    if config_path.exists():
        print(f"  Config found at: {config_path}")
        if yaml:
            try:
                with open(config_path, "r") as f:
                    cfg = yaml.safe_load(f)
                layouts = cfg.get("layouts", {})
                print(f"  Layouts defined: {list(layouts.keys())}")
                for name, layout in layouts.items():
                    cols = layout.get("columns", [])
                    hotkey = layout.get("hotkey", "none")
                    has_matches = any(col.get("match") for col in cols)
                    print(f"    {name}: {len(cols)} columns, hotkey={hotkey}, has_match_rules={has_matches}")
            except Exception as e:
                print(f"  Failed to parse config: {e}")
    else:
        print(f"  No config at {config_path} — will be created on first app run")

    print("\n" + "=" * 70)
    print("Done. Copy/paste the output above so we can figure out what's wrong.")
    print("=" * 70)


if __name__ == "__main__":
    main()
