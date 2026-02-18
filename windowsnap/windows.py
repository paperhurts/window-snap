"""Window management engine using Win32 API.

Handles enumerating windows, matching them to layout slots,
and moving/resizing them to target positions.
"""

import logging
from dataclasses import dataclass, field
from typing import Any

import win32gui
import win32con
import win32process
import win32api
import ctypes
from ctypes import wintypes
import psutil

logger = logging.getLogger(__name__)

# DPI awareness — critical for correct positioning on high-DPI displays
try:
    ctypes.windll.shcore.SetProcessDpiAwareness(2)  # PROCESS_PER_MONITOR_DPI_AWARE
except Exception:
    try:
        ctypes.windll.user32.SetProcessDPIAware()
    except Exception:
        pass


@dataclass
class WindowInfo:
    """Info about a visible window."""
    hwnd: int
    title: str
    process_name: str = ""
    is_minimized: bool = False
    is_visible: bool = True
    rect: tuple[int, int, int, int] = (0, 0, 0, 0)  # left, top, right, bottom


@dataclass
class MonitorInfo:
    """Info about a display monitor."""
    index: int
    handle: int
    work_area: tuple[int, int, int, int]  # left, top, right, bottom (excludes taskbar)
    full_rect: tuple[int, int, int, int]  # full monitor rect
    is_primary: bool = False


@dataclass
class LayoutSlot:
    """A target position for a window in a layout."""
    x: int
    y: int
    width: int
    height: int


def get_monitors() -> list[MonitorInfo]:
    """Enumerate all monitors and their work areas."""
    monitors = []

    try:
        raw_monitors = win32api.EnumDisplayMonitors()
    except Exception as e:
        logger.error("Failed to enumerate monitors: %s", e)
        return []

    for hmonitor, _hdc, _rect in raw_monitors:
        try:
            info = win32api.GetMonitorInfo(hmonitor)
            work = info["Work"]
            monitor = info["Monitor"]
            is_primary = bool(info["Flags"] & 1)

            monitors.append(MonitorInfo(
                index=len(monitors),
                handle=hmonitor,
                work_area=(work[0], work[1], work[2], work[3]),
                full_rect=(monitor[0], monitor[1], monitor[2], monitor[3]),
                is_primary=is_primary,
            ))
        except Exception as e:
            logger.warning("Failed to get info for monitor %s: %s", hmonitor, e)

    if not monitors:
        logger.error("No monitors detected")
        return []

    # Sort so primary is index 0
    monitors.sort(key=lambda m: (not m.is_primary, m.full_rect[0]))
    for i, m in enumerate(monitors):
        m.index = i

    logger.info("Detected %d monitor(s)", len(monitors))
    for m in monitors:
        logger.debug("  Monitor %d: work_area=%s primary=%s", m.index, m.work_area, m.is_primary)

    return monitors


def get_visible_windows() -> list[WindowInfo]:
    """Enumerate all visible, top-level windows worth managing."""
    windows = []

    # Classes to skip — system UI, tooltips, etc.
    # Note: #32770 (dialog) is intentionally NOT skipped — some apps use dialogs as main windows
    skip_classes = {
        "Shell_TrayWnd", "Shell_SecondaryTrayWnd",  # taskbar
        "Progman", "WorkerW",  # desktop
        "Windows.UI.Core.CoreWindow",  # some UWP overlays
        "tooltips_class32",
        "IME", "MSCTFIME UI",
    }

    def callback(hwnd, _):
        if not win32gui.IsWindowVisible(hwnd):
            return True

        # Skip windows with no title
        title = win32gui.GetWindowText(hwnd)
        if not title:
            return True

        # Skip by class
        try:
            cls = win32gui.GetClassName(hwnd)
            if cls in skip_classes:
                return True
        except Exception:
            pass

        # Skip windows with WS_EX_TOOLWINDOW style (floating toolbars, etc.)
        ex_style = win32gui.GetWindowLong(hwnd, win32con.GWL_EXSTYLE)
        if ex_style & win32con.WS_EX_TOOLWINDOW:
            return True

        # Must be a top-level window (no owner, or an app window)
        if not (ex_style & win32con.WS_EX_APPWINDOW):
            owner = win32gui.GetWindow(hwnd, win32con.GW_OWNER)
            if owner:
                return True

        # Get process name
        process_name = ""
        try:
            _, pid = win32process.GetWindowThreadProcessId(hwnd)
            proc = psutil.Process(pid)
            process_name = proc.name()
        except Exception:
            pass

        is_minimized = bool(win32gui.IsIconic(hwnd))
        rect = win32gui.GetWindowRect(hwnd)

        windows.append(WindowInfo(
            hwnd=hwnd,
            title=title,
            process_name=process_name,
            is_minimized=is_minimized,
            rect=rect,
        ))
        return True

    win32gui.EnumWindows(callback, None)

    logger.debug("Found %d manageable windows", len(windows))
    return windows


def match_window(windows: list[WindowInfo], match_rules: list[dict[str, str]]) -> WindowInfo | None:
    """Find the best matching window for a set of match rules.

    Rules are OR'd — first rule that matches any window wins.
    Supports: title_contains, process_name (case-insensitive).
    Within a rule, tries to find the most recently active match.
    """
    if not match_rules:
        return None

    for rule in match_rules:
        title_contains = rule.get("title_contains", "")
        process_name = rule.get("process_name", "")

        if not title_contains and not process_name:
            continue

        candidates = []
        for w in windows:
            if title_contains and title_contains.lower() in w.title.lower():
                candidates.append(w)
            elif process_name and process_name.lower() in w.process_name.lower():
                candidates.append(w)

        if candidates:
            # Prefer non-minimized windows
            non_min = [w for w in candidates if not w.is_minimized]
            if non_min:
                return non_min[0]
            return candidates[0]

    return None


def calculate_slots(
    layout: dict[str, Any],
    monitor: MonitorInfo,
    gap: int = 5,
) -> list[LayoutSlot]:
    """Calculate pixel positions for each column in a layout."""
    columns = layout.get("columns", [])
    if not columns:
        return []

    work_left, work_top, work_right, work_bottom = monitor.work_area
    total_width = work_right - work_left
    total_height = work_bottom - work_top

    # Account for gaps: gap on each side of each column, plus outer edges
    num_columns = len(columns)
    total_gap_width = gap * (num_columns + 1)  # gaps between and on edges
    usable_width = total_width - total_gap_width

    slots = []
    x_offset = work_left + gap

    for i, col in enumerate(columns):
        pct = col.get("width_percent", 0)
        col_width = int(usable_width * pct / 100)

        # Last column gets remaining pixels to avoid rounding gaps
        if i == num_columns - 1:
            col_width = (work_right - gap) - x_offset

        slots.append(LayoutSlot(
            x=x_offset,
            y=work_top + gap,
            width=col_width,
            height=total_height - (2 * gap),
        ))

        x_offset += col_width + gap

    return slots


def move_window(hwnd: int, slot: LayoutSlot) -> bool:
    """Move and resize a window to the target slot.

    Handles restoring minimized windows and dealing with
    extended window frames (DWM margins).
    """
    try:
        # Restore if minimized
        if win32gui.IsIconic(hwnd):
            win32gui.ShowWindow(hwnd, win32con.SW_RESTORE)

        # If maximized, restore first so we can resize
        placement = win32gui.GetWindowPlacement(hwnd)
        if placement[1] == win32con.SW_SHOWMAXIMIZED:
            win32gui.ShowWindow(hwnd, win32con.SW_RESTORE)

        # Account for invisible DWM borders (Windows 10/11 thing)
        # The actual visible border is smaller than the window rect
        try:
            rect = ctypes.wintypes.RECT()
            DWMWA_EXTENDED_FRAME_BOUNDS = 9
            ctypes.windll.dwmapi.DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                ctypes.byref(rect),
                ctypes.sizeof(rect),
            )
            win_rect = win32gui.GetWindowRect(hwnd)
            # Calculate the invisible border sizes
            border_left = rect.left - win_rect[0]
            border_top = rect.top - win_rect[1]
            border_right = win_rect[2] - rect.right
            border_bottom = win_rect[3] - rect.bottom

            # Compensate by expanding the target rect
            adjusted_x = slot.x - border_left
            adjusted_y = slot.y - border_top
            adjusted_w = slot.width + border_left + border_right
            adjusted_h = slot.height + border_top + border_bottom
        except Exception:
            # If DWM query fails, use raw values
            adjusted_x = slot.x
            adjusted_y = slot.y
            adjusted_w = slot.width
            adjusted_h = slot.height

        # Move and resize
        win32gui.SetWindowPos(
            hwnd,
            win32con.HWND_TOP,
            adjusted_x,
            adjusted_y,
            adjusted_w,
            adjusted_h,
            win32con.SWP_NOZORDER | win32con.SWP_NOACTIVATE,
        )

        logger.debug("Moved window %s to (%d, %d, %d, %d)",
                      win32gui.GetWindowText(hwnd),
                      slot.x, slot.y, slot.width, slot.height)
        return True

    except Exception as e:
        logger.warning("Failed to move window %s: %s",
                       win32gui.GetWindowText(hwnd), e)
        return False


def apply_layout(layout_name: str, layout: dict[str, Any], gap: int = 5) -> None:
    """Apply a layout: match windows to slots and move them."""
    monitors = get_monitors()

    # Determine target monitor
    monitor_idx = layout.get("monitor", 0)
    if monitor_idx >= len(monitors):
        logger.warning("Monitor %d not found, using primary", monitor_idx)
        monitor_idx = 0
    monitor = monitors[monitor_idx]

    columns = layout.get("columns", [])
    slots = calculate_slots(layout, monitor, gap)

    if not slots:
        logger.warning("Layout '%s' has no columns", layout_name)
        return

    # Get all visible windows
    all_windows = get_visible_windows()
    used_hwnds: set[int] = set()

    logger.info("Applying layout '%s' (%d columns) on monitor %d",
                layout_name, len(columns), monitor_idx)

    for i, (col, slot) in enumerate(zip(columns, slots)):
        match_rules = col.get("match", [])

        if not match_rules:
            # Empty match — skip this slot
            logger.debug("Column %d: no match rules, skipping", i)
            continue

        # Filter out already-used windows
        available = [w for w in all_windows if w.hwnd not in used_hwnds]
        matched = match_window(available, match_rules)

        if matched:
            move_window(matched.hwnd, slot)
            used_hwnds.add(matched.hwnd)
            logger.info("Column %d: placed '%s'", i, matched.title)
        else:
            match_desc = ", ".join(
                r.get("title_contains") or r.get("process_name") or "?" for r in match_rules
            )
            logger.info("Column %d: no match for [%s]", i, match_desc)
