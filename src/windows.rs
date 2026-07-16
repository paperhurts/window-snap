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

    let num_columns = columns.len() as i32;
    let total_gap_width = gap * (num_columns + 1);
    let usable_width = total_width - total_gap_width;

    let mut slots = Vec::new();
    let mut x_offset = work_left + gap;

    for (i, col) in columns.iter().enumerate() {
        let pct = col.width_percent as i32;
        let mut col_width = usable_width * pct / 100;

        // Last column absorbs rounding remainder
        if i == columns.len() - 1 {
            col_width = (work_right - gap) - x_offset;
        }

        slots.push(LayoutSlot {
            x: x_offset,
            y: work_top + gap,
            width: col_width,
            height: total_height - (2 * gap),
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

    for (i, (col, slot)) in layout.columns.iter().zip(slots.iter()).enumerate() {
        if col.match_rules.is_empty() {
            log::debug!("Column {}: no match rules, skipping", i);
            continue;
        }

        let matched = match_window(&mut available, &col.match_rules);

        if let Some(win) = matched {
            #[cfg(windows)]
            {
                move_window(win.hwnd, slot);
            }
            log::info!(
                "Column {}: placed '{}' ({}, hwnd=0x{:x})",
                i, win.title, win.process_name, win.hwnd
            );
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
}
