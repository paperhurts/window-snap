//! Window management engine using Win32 API.
//!
//! Handles enumerating windows, matching them to layout slots,
//! and moving/resizing them to target positions.

use crate::config::{Column, Layout, MatchRule};

#[cfg(windows)]
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, TRUE};
#[cfg(windows)]
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
#[cfg(windows)]
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
#[cfg(windows)]
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Debug)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
    pub is_minimized: bool,
}

#[derive(Debug)]
pub struct MonitorInfo {
    pub index: usize,
    pub work_area: (i32, i32, i32, i32), // left, top, right, bottom
    pub full_rect: (i32, i32, i32, i32),
    pub is_primary: bool,
}

pub struct LayoutSlot {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Set DPI awareness — critical for correct positioning on high-DPI displays.
/// Call once at startup.
#[cfg(windows)]
pub fn set_dpi_awareness() {
    unsafe {
        // Try per-monitor DPI awareness (Windows 8.1+)
        let result = windows::Win32::UI::HiDpi::SetProcessDpiAwareness(
            windows::Win32::UI::HiDpi::PROCESS_PER_MONITOR_DPI_AWARE,
        );
        if result.is_err() {
            // Fallback to basic DPI awareness
            let _ = SetProcessDPIAware();
        }
    }
}

#[cfg(not(windows))]
pub fn set_dpi_awareness() {}

/// Enumerate all monitors and their work areas.
#[cfg(windows)]
pub fn get_monitors() -> Vec<MonitorInfo> {
    let mut monitors: Vec<MonitorInfo> = Vec::new();

    unsafe extern "system" fn callback(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let monitors = &mut *(data.0 as *mut Vec<MonitorInfo>);

        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
            let work = info.rcWork;
            let full = info.rcMonitor;
            let is_primary = (info.dwFlags & MONITORINFOF_PRIMARY) != 0;

            monitors.push(MonitorInfo {
                index: monitors.len(),
                work_area: (work.left, work.top, work.right, work.bottom),
                full_rect: (full.left, full.top, full.right, full.bottom),
                is_primary,
            });
        }
        TRUE
    }

    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(callback),
            LPARAM(&mut monitors as *mut Vec<MonitorInfo> as isize),
        );
    }

    // Sort: primary first, then by x-coordinate
    monitors.sort_by(|a, b| {
        ((!a.is_primary) as u8, a.full_rect.0).cmp(&((!b.is_primary) as u8, b.full_rect.0))
    });
    for (i, m) in monitors.iter_mut().enumerate() {
        m.index = i;
    }

    log::info!("Detected {} monitor(s)", monitors.len());
    monitors
}

#[cfg(not(windows))]
pub fn get_monitors() -> Vec<MonitorInfo> {
    Vec::new()
}

/// Enumerate all visible, top-level windows worth managing.
#[cfg(windows)]
pub fn get_visible_windows() -> Vec<WindowInfo> {
    let mut result: Vec<WindowInfo> = Vec::new();

    // Classes to skip — system UI, tooltips, etc.
    const SKIP_CLASSES: &[&str] = &[
        "Shell_TrayWnd",
        "Shell_SecondaryTrayWnd",
        "Progman",
        "WorkerW",
        "Windows.UI.Core.CoreWindow",
        "tooltips_class32",
        "IME",
        "MSCTFIME UI",
    ];

    unsafe extern "system" fn callback(hwnd: HWND, data: LPARAM) -> BOOL {
        let windows = &mut *(data.0 as *mut Vec<WindowInfo>);

        if !IsWindowVisible(hwnd).as_bool() {
            return TRUE;
        }

        // Get title
        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, &mut title_buf);
        if title_len == 0 {
            return TRUE;
        }
        let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

        // Skip by class
        let mut class_buf = [0u16; 256];
        let class_len = GetClassNameW(hwnd, &mut class_buf);
        if class_len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);
            for skip in SKIP_CLASSES {
                if class_name == *skip {
                    return TRUE;
                }
            }
        }

        // Skip tool windows
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return TRUE;
        }

        // Must be a top-level window (no owner, or an app window)
        if ex_style & WS_EX_APPWINDOW.0 == 0 {
            let owner = GetWindow(hwnd, GW_OWNER);
            if owner.is_ok() && owner.unwrap() != HWND::default() {
                return TRUE;
            }
        }

        // Get process name
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let process_name = get_process_name(pid);

        let is_minimized = IsIconic(hwnd).as_bool();

        windows.push(WindowInfo {
            hwnd: hwnd.0 as isize,
            title,
            process_name,
            is_minimized,
        });

        TRUE
    }

    unsafe {
        let _ = EnumWindows(
            Some(callback),
            LPARAM(&mut result as *mut Vec<WindowInfo> as isize),
        );
    }

    log::debug!("Found {} manageable windows", result.len());
    result
}

#[cfg(not(windows))]
pub fn get_visible_windows() -> Vec<WindowInfo> {
    Vec::new()
}

/// Get the executable name for a process ID.
#[cfg(windows)]
fn get_process_name(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        let handle = match handle {
            Ok(h) => h,
            Err(_) => return String::new(),
        };

        let mut buf = [0u16; 512];
        let len = GetModuleFileNameExW(handle, None, &mut buf);
        let _ = windows::Win32::Foundation::CloseHandle(handle);

        if len == 0 {
            return String::new();
        }

        let full_path = String::from_utf16_lossy(&buf[..len as usize]);
        // Extract just the filename
        full_path
            .rsplit(|c| c == '\\' || c == '/')
            .next()
            .unwrap_or("")
            .to_string()
    }
}

/// Check whether a single rule matches a window.
/// title_contains and process_name are each case-insensitive substring checks;
/// when a rule specifies both, the window must satisfy both (AND).
fn rule_matches(rule: &MatchRule, window: &WindowInfo) -> bool {
    let title_search = rule.title_contains.as_deref().unwrap_or("");
    let proc_search = rule.process_name.as_deref().unwrap_or("");

    // A rule with no criteria matches nothing.
    if title_search.is_empty() && proc_search.is_empty() {
        return false;
    }

    if !title_search.is_empty()
        && !window.title.to_lowercase().contains(&title_search.to_lowercase())
    {
        return false;
    }

    if !proc_search.is_empty()
        && !window
            .process_name
            .to_lowercase()
            .contains(&proc_search.to_lowercase())
    {
        return false;
    }

    true
}

/// Find the best matching window for a set of match rules.
/// Rules are OR'd — first rule that matches any window wins.
/// Removes the matched window from the pool.
///
/// When several windows match a rule, the earliest in the pool wins
/// (EnumWindows order = Z-order, topmost first — i.e. roughly the most
/// recently used matching window), preferring non-minimized candidates.
/// To deterministically target one specific window among several, use a
/// combined AND rule, e.g.
/// `{ process_name = "WindowsTerminal.exe", title_contains = "my-project" }`.
pub fn match_window(
    windows: &mut Vec<WindowInfo>,
    rules: &[MatchRule],
) -> Option<WindowInfo> {
    for rule in rules {
        // Find candidates, prefer non-minimized
        let mut best_idx: Option<usize> = None;
        let mut best_minimized = true;

        for (i, w) in windows.iter().enumerate() {
            if rule_matches(rule, w) {
                if best_idx.is_none() || (best_minimized && !w.is_minimized) {
                    best_idx = Some(i);
                    best_minimized = w.is_minimized;
                }
            }
        }

        if let Some(idx) = best_idx {
            return Some(windows.remove(idx));
        }
    }

    None
}

/// Claim the windows a column is entitled to, removing them from the pool so no
/// later column can re-place them.
///
/// A normal column takes at most one window. A `match_all` column keeps claiming
/// until nothing matches, which is what lets a single slot resize every browser
/// window instead of only whichever one happened to be on top.
///
/// Returned in match order, so the first entry is the window that was topmost.
pub fn claim_windows(pool: &mut Vec<WindowInfo>, col: &Column) -> Vec<WindowInfo> {
    let mut claimed = Vec::new();
    while let Some(win) = match_window(pool, &col.match_rules) {
        claimed.push(win);
        if !col.match_all {
            break;
        }
    }
    claimed
}

/// Calculate pixel positions for each column in a layout.
pub fn calculate_slots(
    columns: &[Column],
    monitor: &MonitorInfo,
    gap: i32,
) -> Vec<LayoutSlot> {
    if columns.is_empty() {
        return Vec::new();
    }

    let (work_left, work_top, work_right, work_bottom) = monitor.work_area;
    let total_width = work_right - work_left;
    let total_height = work_bottom - work_top;

    // Columns carrying an explicit x_percent are placed absolutely and sit
    // outside the tiling flow: they neither consume gaps nor shift their
    // neighbours. Gap budgeting and the remainder rule below therefore run over
    // the tiled columns alone, which keeps a layout with no x_percent byte-for-
    // byte identical to the old behaviour.
    let tiled_count = columns.iter().filter(|c| c.x_percent.is_none()).count() as i32;
    let usable_width = total_width - gap * (tiled_count + 1);

    // Absolute columns measure against the full work area (minus the outer gaps)
    // so that x_percent = 0 means "flush left" and width_percent = 100 means
    // "full width", no matter how many other columns the layout has.
    let absolute_span = total_width - (2 * gap);

    // The remainder rule targets the last *tiled* column, which is not
    // necessarily the last column in the layout.
    let last_tiled = columns.iter().rposition(|c| c.x_percent.is_none());

    let y = work_top + gap;
    let height = total_height - (2 * gap);

    let mut slots = Vec::with_capacity(columns.len());
    let mut x_offset = work_left + gap;

    for (i, col) in columns.iter().enumerate() {
        if let Some(x_pct) = col.x_percent {
            slots.push(LayoutSlot {
                x: work_left + gap + absolute_span * x_pct as i32 / 100,
                width: absolute_span * col.width_percent as i32 / 100,
                y,
                height,
            });
            continue;
        }

        let mut col_width = usable_width * col.width_percent as i32 / 100;

        // Last tiled column absorbs rounding remainder
        if Some(i) == last_tiled {
            col_width = (work_right - gap) - x_offset;
        }

        slots.push(LayoutSlot {
            x: x_offset,
            y,
            width: col_width,
            height,
        });

        x_offset += col_width + gap;
    }

    slots
}

/// Move and resize a window to the target slot.
/// Handles restoring minimized windows and DWM border compensation.
#[cfg(windows)]
fn move_window(hwnd_val: isize, slot: &LayoutSlot) -> bool {
    let hwnd = HWND(hwnd_val as *mut _);

    unsafe {
        // Restore if minimized
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        // If maximized, restore first so we can resize
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        if GetWindowPlacement(hwnd, &mut placement).is_ok()
            && placement.showCmd == SW_SHOWMAXIMIZED.0 as u32
        {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        // DWM border compensation — account for invisible borders
        let (adjusted_x, adjusted_y, adjusted_w, adjusted_h) = {
            let mut frame_rect = RECT::default();
            let frame_result = DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut frame_rect as *mut RECT as *mut _,
                std::mem::size_of::<RECT>() as u32,
            );

            if frame_result.is_ok() {
                let mut win_rect = RECT::default();
                if GetWindowRect(hwnd, &mut win_rect).is_ok() {
                    let border_left = frame_rect.left - win_rect.left;
                    let border_top = frame_rect.top - win_rect.top;
                    let border_right = win_rect.right - frame_rect.right;
                    let border_bottom = win_rect.bottom - frame_rect.bottom;

                    (
                        slot.x - border_left,
                        slot.y - border_top,
                        slot.width + border_left + border_right,
                        slot.height + border_top + border_bottom,
                    )
                } else {
                    (slot.x, slot.y, slot.width, slot.height)
                }
            } else {
                (slot.x, slot.y, slot.width, slot.height)
            }
        };

        let result = SetWindowPos(
            hwnd,
            HWND_TOP,
            adjusted_x,
            adjusted_y,
            adjusted_w,
            adjusted_h,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );

        if result.is_ok() {
            log::debug!(
                "Moved window to ({}, {}, {}x{})",
                slot.x, slot.y, slot.width, slot.height
            );
            true
        } else {
            log::warn!("Failed to move window hwnd={}", hwnd_val);
            false
        }
    }
}

/// Bring a window to the top of the z-order without moving, resizing, or
/// focusing it. Used to stack overlapping columns in the order the config
/// declares them.
#[cfg(windows)]
fn raise_window(hwnd_val: isize) {
    unsafe {
        let _ = SetWindowPos(
            HWND(hwnd_val as *mut _),
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(hwnd: isize, title: &str, process: &str, minimized: bool) -> WindowInfo {
        WindowInfo {
            hwnd,
            title: title.to_string(),
            process_name: process.to_string(),
            is_minimized: minimized,
        }
    }

    fn rule(title: Option<&str>, process: Option<&str>) -> MatchRule {
        MatchRule {
            title_contains: title.map(String::from),
            process_name: process.map(String::from),
        }
    }

    #[test]
    fn matches_by_title_case_insensitive() {
        let mut pool = vec![win(1, "Windows PowerShell", "powershell.exe", false)];
        let matched = match_window(&mut pool, &[rule(Some("powershell"), None)]);
        assert_eq!(matched.unwrap().hwnd, 1);
        assert!(pool.is_empty());
    }

    #[test]
    fn matches_renamed_window_by_process() {
        // The bug report: a renamed PowerShell window must still match on process.
        let mut pool = vec![
            win(1, "Notes - Notepad", "notepad.exe", false),
            win(2, "my custom window name", "powershell.exe", false),
        ];
        let matched = match_window(&mut pool, &[rule(None, Some("powershell.exe"))]);
        assert_eq!(matched.unwrap().hwnd, 2);
    }

    #[test]
    fn combined_rule_requires_both_title_and_process() {
        // Rule with both fields is an AND: title alone must not match.
        let mut pool = vec![
            win(1, "admin session", "cmd.exe", false),
            win(2, "admin session", "powershell.exe", false),
        ];
        let matched = match_window(
            &mut pool,
            &[rule(Some("admin"), Some("powershell.exe"))],
        );
        assert_eq!(matched.unwrap().hwnd, 2);
        assert_eq!(pool.len(), 1);
        assert_eq!(pool[0].hwnd, 1);
    }

    #[test]
    fn combined_rule_matches_nothing_when_only_one_side_holds() {
        let mut pool = vec![
            win(1, "admin session", "cmd.exe", false),      // title yes, process no
            win(2, "build output", "powershell.exe", false), // process yes, title no
        ];
        let matched = match_window(
            &mut pool,
            &[rule(Some("admin"), Some("powershell.exe"))],
        );
        assert!(matched.is_none());
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn first_rule_wins_over_later_rules() {
        let mut pool = vec![
            win(1, "some page - Firefox", "firefox.exe", false),
            win(2, "some page - Brave", "brave.exe", false),
        ];
        let rules = [
            rule(None, Some("brave.exe")),
            rule(None, Some("firefox.exe")),
        ];
        let matched = match_window(&mut pool, &rules);
        assert_eq!(matched.unwrap().hwnd, 2);
    }

    #[test]
    fn earliest_window_in_pool_wins_among_equal_matches() {
        // Contract: pool order is EnumWindows Z-order (topmost first), and the
        // earliest non-minimized match wins. See match_window docs.
        let mut pool = vec![
            win(1, "reader", "WindowsTerminal.exe", false),
            win(2, "window-snap", "WindowsTerminal.exe", false),
        ];
        let matched = match_window(&mut pool, &[rule(None, Some("WindowsTerminal.exe"))]);
        assert_eq!(matched.unwrap().hwnd, 1);
    }

    #[test]
    fn prefers_non_minimized_candidate() {
        let mut pool = vec![
            win(1, "PowerShell", "powershell.exe", true),
            win(2, "PowerShell", "powershell.exe", false),
        ];
        let matched = match_window(&mut pool, &[rule(None, Some("powershell.exe"))]);
        assert_eq!(matched.unwrap().hwnd, 2);
    }

    #[test]
    fn matched_window_leaves_pool_for_repeat_rules() {
        // BlueStacks-style: same process rule on consecutive columns grabs distinct windows.
        let mut pool = vec![
            win(1, "BlueStacks 1", "HD-Player.exe", false),
            win(2, "BlueStacks 2", "HD-Player.exe", false),
        ];
        let rules = [rule(None, Some("HD-Player.exe"))];
        let first = match_window(&mut pool, &rules).unwrap();
        let second = match_window(&mut pool, &rules).unwrap();
        assert_ne!(first.hwnd, second.hwnd);
        assert!(match_window(&mut pool, &rules).is_none());
    }

    #[test]
    fn empty_rule_list_matches_nothing() {
        let mut pool = vec![win(1, "PowerShell", "powershell.exe", false)];
        assert!(match_window(&mut pool, &[]).is_none());
        assert_eq!(pool.len(), 1);
    }

    fn test_monitor() -> MonitorInfo {
        MonitorInfo {
            index: 0,
            work_area: (0, 0, 1920, 1040), // taskbar-clipped 1080p
            full_rect: (0, 0, 1920, 1080),
            is_primary: true,
        }
    }

    fn col(width_percent: u32) -> Column {
        Column {
            width_percent,
            x_percent: None,
            match_all: false,
            match_rules: Vec::new(),
        }
    }

    /// A column pinned to an absolute left edge, outside the tiling flow.
    fn abs_col(x_percent: u32, width_percent: u32) -> Column {
        Column {
            width_percent,
            x_percent: Some(x_percent),
            match_all: false,
            match_rules: Vec::new(),
        }
    }

    #[test]
    fn slots_fill_work_area_with_gaps() {
        let gap = 5;
        let columns = [col(25), col(25), col(25), col(25)];
        let slots = calculate_slots(&columns, &test_monitor(), gap);
        assert_eq!(slots.len(), 4);
        assert_eq!(slots[0].x, gap);
        assert_eq!(slots[0].y, gap);
        assert_eq!(slots[0].height, 1040 - 2 * gap);
        // Last column ends flush against the right work edge minus gap.
        let last = &slots[3];
        assert_eq!(last.x + last.width, 1920 - gap);
        // Columns don't overlap and are separated by exactly one gap.
        for pair in slots.windows(2) {
            assert_eq!(pair[0].x + pair[0].width + gap, pair[1].x);
        }
    }

    #[test]
    fn last_slot_absorbs_rounding_remainder() {
        let gap = 0;
        let columns = [col(33), col(33), col(33)]; // 99% — remainder goes to last
        let slots = calculate_slots(&columns, &test_monitor(), gap);
        let total: i32 = slots.iter().map(|s| s.width).sum();
        assert_eq!(total, 1920);
    }

    #[test]
    fn underfull_widths_still_reach_the_screen_edge() {
        // 40 + 20 = 60%, yet the last column absorbs the slack and the layout
        // still fills the work area. Widths under 100 are self-correcting, which
        // is why Config::validate stays quiet about them.
        let gap = 5;
        let columns = [col(40), col(20)];
        let slots = calculate_slots(&columns, &test_monitor(), gap);
        let last = slots.last().unwrap();
        assert_eq!(last.x + last.width, 1920 - gap);
        assert!(slots.iter().all(|s| s.width > 0));
    }

    /// A column carrying match rules, for the claim tests.
    fn matching_col(match_all: bool, rules: Vec<MatchRule>) -> Column {
        Column {
            width_percent: 30,
            x_percent: None,
            match_all,
            match_rules: rules,
        }
    }

    fn title_rule(title: &str) -> MatchRule {
        MatchRule {
            title_contains: Some(title.to_string()),
            process_name: None,
        }
    }

    #[test]
    fn normal_column_claims_only_the_topmost_match() {
        let mut pool = vec![
            win(1, "Coat Check - Brave", "brave.exe", false),
            win(2, "Recipes - Brave", "brave.exe", false),
            win(3, "Signal", "Signal.exe", false),
        ];
        let claimed = claim_windows(&mut pool, &matching_col(false, vec![title_rule("Brave")]));
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].hwnd, 1);
        // The unclaimed browser window is still available to later columns.
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn match_all_column_claims_every_match_in_order() {
        let mut pool = vec![
            win(1, "Coat Check - Brave", "brave.exe", false),
            win(2, "Signal", "Signal.exe", false),
            win(3, "Recipes - Brave", "brave.exe", false),
            win(4, "Downloads - Brave", "brave.exe", false),
        ];
        let claimed = claim_windows(&mut pool, &matching_col(true, vec![title_rule("Brave")]));
        // Match order is z-order, so the window that was on top comes first —
        // apply_layout relies on that to keep it on top of its stackmates.
        assert_eq!(
            claimed.iter().map(|w| w.hwnd).collect::<Vec<_>>(),
            vec![1, 3, 4]
        );
        // Non-matching windows are untouched.
        assert_eq!(pool.len(), 1);
        assert_eq!(pool[0].hwnd, 2);
    }

    #[test]
    fn match_all_column_leaves_nothing_for_a_later_duplicate_column() {
        // Two columns with identical rules: the first is match_all, so the
        // second must come up empty rather than re-placing a claimed window.
        let mut pool = vec![
            win(1, "A - Brave", "brave.exe", false),
            win(2, "B - Brave", "brave.exe", false),
        ];
        let first = claim_windows(&mut pool, &matching_col(true, vec![title_rule("Brave")]));
        let second = claim_windows(&mut pool, &matching_col(true, vec![title_rule("Brave")]));
        assert_eq!(first.len(), 2);
        assert!(second.is_empty());
    }

    #[test]
    fn match_all_column_with_no_matches_claims_nothing() {
        let mut pool = vec![win(1, "Signal", "Signal.exe", false)];
        let claimed = claim_windows(&mut pool, &matching_col(true, vec![title_rule("Brave")]));
        assert!(claimed.is_empty());
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn absolute_column_is_positioned_at_its_x_percent() {
        let gap = 5;
        // 1920 wide, minus the two outer gaps, leaves a 1910px span to measure against.
        let slots = calculate_slots(&[abs_col(50, 25)], &test_monitor(), gap);
        assert_eq!(slots[0].x, gap + 1910 / 2);
        assert_eq!(slots[0].width, 1910 / 4);
    }

    #[test]
    fn absolute_columns_may_overlap() {
        // The point of the feature: two windows both wide enough to use, sharing
        // the middle of the screen. Tiling cannot express this.
        let slots = calculate_slots(&[abs_col(0, 60), abs_col(45, 55)], &test_monitor(), 5);
        let (a, b) = (&slots[0], &slots[1]);
        assert!(
            b.x < a.x + a.width,
            "expected overlap: first ends at {}, second starts at {}",
            a.x + a.width,
            b.x
        );
        // Both keep their full requested width — neither is squeezed by the other.
        assert_eq!(a.width, 1910 * 60 / 100);
        assert_eq!(b.width, 1910 * 55 / 100);
    }

    #[test]
    fn absolute_columns_do_not_disturb_tiled_neighbours() {
        // Mixed layout: the tiled columns must land exactly where they would if
        // the absolute one were not in the list at all.
        let monitor = test_monitor();
        let tiled_only = calculate_slots(&[col(30), col(70)], &monitor, 5);
        let mixed = calculate_slots(&[col(30), abs_col(10, 40), col(70)], &monitor, 5);

        assert_eq!((mixed[0].x, mixed[0].width), (tiled_only[0].x, tiled_only[0].width));
        assert_eq!((mixed[2].x, mixed[2].width), (tiled_only[1].x, tiled_only[1].width));
    }

    #[test]
    fn last_tiled_column_absorbs_remainder_even_when_an_absolute_column_follows() {
        // The remainder rule targets the last *tiled* column, not the last column.
        let gap = 5;
        let slots = calculate_slots(&[col(50), col(50), abs_col(0, 30)], &test_monitor(), gap);
        assert_eq!(slots[1].x + slots[1].width, 1920 - gap);
    }

    #[test]
    fn layout_without_x_percent_is_unchanged() {
        // Backward compatibility: the new field must not shift existing layouts.
        let monitor = test_monitor();
        let columns = [col(25), col(25), col(25), col(25)];
        let slots = calculate_slots(&columns, &monitor, 5);
        assert_eq!(slots[0].x, 5);
        assert_eq!(slots.last().unwrap().x + slots.last().unwrap().width, 1920 - 5);
        assert!(slots.windows(2).all(|w| w[1].x >= w[0].x + w[0].width));
    }

    #[test]
    fn overfull_widths_run_off_the_screen_instead_of_overlapping() {
        // Columns are tiled (x_offset += width + gap), never stacked, so widths
        // over 100% cannot produce overlap — the excess marches past the right
        // edge and the last column is handed a negative width. This is the
        // breakage Config::validate warns about.
        let gap = 5;
        let columns = [col(50), col(50), col(50), col(50)]; // 200%
        let slots = calculate_slots(&columns, &test_monitor(), gap);
        assert!(
            slots.iter().any(|s| s.x + s.width > 1920),
            "expected a column past the right edge: {:?}",
            slots.iter().map(|s| (s.x, s.width)).collect::<Vec<_>>()
        );
        assert!(
            slots.last().unwrap().width < 0,
            "expected the last column to get a negative width, got {}",
            slots.last().unwrap().width
        );
    }
}

/// Apply a layout: match windows to slots and move them.
pub fn apply_layout(layout_name: &str, layout: &Layout, gap: i32) {
    set_dpi_awareness();

    let monitors = get_monitors();
    if monitors.is_empty() {
        log::error!("No monitors detected");
        return;
    }

    let monitor_idx = if layout.monitor < monitors.len() {
        layout.monitor
    } else {
        log::warn!(
            "Monitor {} not found, using primary",
            layout.monitor
        );
        0
    };
    let monitor = &monitors[monitor_idx];

    let slots = calculate_slots(&layout.columns, monitor, gap);
    if slots.is_empty() {
        log::warn!("Layout '{}' has no columns", layout_name);
        return;
    }

    let mut available = get_visible_windows();

    log::info!(
        "Applying layout '{}' ({} columns) on monitor {}",
        layout_name,
        layout.columns.len(),
        monitor_idx
    );

    // Tracked in column order so overlapping layouts can be stacked afterwards.
    let mut placed: Vec<isize> = Vec::new();

    for (i, (col, slot)) in layout.columns.iter().zip(slots.iter()).enumerate() {
        if col.match_rules.is_empty() {
            log::debug!("Column {}: no match rules, skipping", i);
            continue;
        }

        let claimed = claim_windows(&mut available, col);

        if !claimed.is_empty() {
            for win in &claimed {
                #[cfg(windows)]
                {
                    move_window(win.hwnd, slot);
                }
                log::info!(
                    "Column {}: placed '{}' ({}, hwnd=0x{:x})",
                    i, win.title, win.process_name, win.hwnd
                );
            }

            if claimed.len() > 1 {
                log::info!(
                    "Column {}: stacked {} windows at the same position",
                    i,
                    claimed.len()
                );
            }

            // Reversed so that after the raise pass below the first window
            // claimed — the one that was already on top — stays on top of its
            // stackmates, instead of the layout silently promoting a different
            // browser tab to the front.
            placed.extend(claimed.iter().rev().map(|w| w.hwnd));
        } else {
            let match_desc: Vec<&str> = col
                .match_rules
                .iter()
                .map(|r| {
                    r.title_contains
                        .as_deref()
                        .or(r.process_name.as_deref())
                        .unwrap_or("?")
                })
                .collect();
            log::info!("Column {}: no match for [{}]", i, match_desc.join(", "));
        }
    }

    // A purely tiled layout has nothing overlapping, so stacking is unobservable
    // and reordering would churn the z-order for no reason. Once a layout places
    // columns absolutely they can overlap, and z-order becomes part of the
    // layout: raising each window in column order leaves the last-declared
    // column on top.
    if layout.columns.iter().any(|c| c.x_percent.is_some()) {
        #[cfg(windows)]
        for hwnd in &placed {
            raise_window(*hwnd);
        }
        log::info!(
            "Layout '{}' has absolute columns: stacked {} window(s) in column order",
            layout_name,
            placed.len()
        );
    }
}
