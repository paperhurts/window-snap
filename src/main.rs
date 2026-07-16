// Hide console window on Windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod errors;
mod windows;

use config::Config;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::WindowId;

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};

use std::collections::HashMap;

const MENU_ID_EDIT_CONFIG: &str = "edit_config";
const MENU_ID_RELOAD_CONFIG: &str = "reload_config";
const MENU_ID_TOGGLE_STARTUP: &str = "toggle_startup";
const MENU_ID_QUIT: &str = "quit";

/// Initialize logging. Debug builds log to stderr (visible under `cargo run`).
/// Release builds have no console (windows_subsystem = "windows"), so they log
/// to ~/.windowsnap/windowsnap.log instead, rotating once at 512 KB so the pair
/// of files stays bounded at ~1 MB.
fn init_logging() {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    builder.format_timestamp_secs();

    if !cfg!(debug_assertions) {
        let path = Config::log_path();

        const MAX_LOG_BYTES: u64 = 512 * 1024;
        if std::fs::metadata(&path).map(|m| m.len() > MAX_LOG_BYTES).unwrap_or(false) {
            let rotated = path.with_extension("log.1");
            let _ = std::fs::rename(&path, rotated);
        }

        match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            Ok(file) => {
                builder.target(env_logger::Target::Pipe(Box::new(file)));
            }
            Err(_) => {
                // Fall back to the (invisible) stderr logger rather than dying.
            }
        }
    }

    builder.init();
}

fn main() {
    init_logging();

    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            errors::show_error(
                "Config Error",
                &format!("{}\n\nPath: {}", e, Config::config_path().display()),
            );
            // Fall back to empty config so tray still shows
            Config {
                settings: Default::default(),
                layouts: Default::default(),
            }
        }
    };

    if config.layouts.is_empty() {
        errors::show_error(
            "No Layouts Configured",
            &format!(
                "No layouts found in config. Add [layouts.xxx] sections to:\n\n{}",
                Config::config_path().display()
            ),
        );
    }

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::new(config);

    event_loop.run_app(&mut app).expect("Event loop failed");
}

struct App {
    config: Config,
    _tray: Option<TrayIcon>,
    hotkey_manager: Option<GlobalHotKeyManager>,
    hotkey_layout_map: HashMap<u32, String>,
}

impl App {
    fn new(config: Config) -> Self {
        App {
            config,
            _tray: None,
            hotkey_manager: None,
            hotkey_layout_map: HashMap::new(),
        }
    }

    fn build_menu(&self) -> Menu {
        let menu = Menu::new();

        // Layout items
        let mut names: Vec<&String> = self.config.layouts.keys().collect();
        names.sort();
        for name in names {
            let display_name = name
                .replace('-', " ")
                .split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().to_string() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            let item = MenuItem::with_id(
                format!("layout_{}", name),
                &display_name,
                true,
                None,
            );
            menu.append(&item).ok();
        }

        menu.append(&PredefinedMenuItem::separator()).ok();

        let edit = MenuItem::with_id(MENU_ID_EDIT_CONFIG, "Edit Config", true, None);
        let reload = MenuItem::with_id(MENU_ID_RELOAD_CONFIG, "Reload Config", true, None);
        menu.append(&edit).ok();
        menu.append(&reload).ok();

        menu.append(&PredefinedMenuItem::separator()).ok();

        let startup_label = if is_startup_enabled() {
            "Start with Windows \u{2713}"
        } else {
            "Start with Windows"
        };
        let startup = MenuItem::with_id(MENU_ID_TOGGLE_STARTUP, startup_label, true, None);
        menu.append(&startup).ok();

        menu.append(&PredefinedMenuItem::separator()).ok();

        let quit = MenuItem::with_id(MENU_ID_QUIT, "Quit", true, None);
        menu.append(&quit).ok();

        menu
    }

    fn rebuild_tray(&mut self) {
        let menu = self.build_menu();
        let icon = create_icon();

        match TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("WindowSnap")
            .with_icon(icon)
            .build()
        {
            Ok(tray) => {
                self._tray = Some(tray);
            }
            Err(e) => {
                errors::show_error("Tray Error", &format!("Failed to build tray icon: {}", e));
            }
        }
    }

    fn register_hotkeys(&mut self) {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                log::error!("Failed to create hotkey manager: {}", e);
                return;
            }
        };

        self.hotkey_layout_map.clear();

        for (name, layout) in &self.config.layouts {
            if let Some(ref hotkey_str) = layout.hotkey {
                match parse_hotkey(hotkey_str) {
                    Some(hotkey) => {
                        if let Err(e) = manager.register(hotkey) {
                            log::warn!("Failed to register hotkey '{}': {}", hotkey_str, e);
                        } else {
                            self.hotkey_layout_map.insert(hotkey.id(), name.clone());
                            log::info!("Registered hotkey: {} -> {}", hotkey_str, name);
                        }
                    }
                    None => {
                        log::warn!("Failed to parse hotkey string: '{}'", hotkey_str);
                    }
                }
            }
        }

        self.hotkey_manager = Some(manager);
    }

    fn handle_menu_event(&mut self, event: MenuEvent, event_loop: &ActiveEventLoop) {
        let id = event.id().0.as_str();

        match id {
            MENU_ID_QUIT => {
                log::info!("Quit requested");
                event_loop.exit();
            }
            MENU_ID_EDIT_CONFIG => {
                let path = Config::config_path();
                let path_str = path.to_string_lossy();
                let _ = std::process::Command::new("cmd")
                    .args(["/c", "start", "", &path_str])
                    .spawn();
            }
            MENU_ID_RELOAD_CONFIG => {
                match Config::load() {
                    Ok(config) => {
                        self.config = config;
                        self.register_hotkeys();
                        self.rebuild_tray();
                        log::info!("Config reloaded");
                    }
                    Err(e) => {
                        errors::show_error("Config Reload Failed", &e);
                    }
                }
            }
            MENU_ID_TOGGLE_STARTUP => {
                let current = is_startup_enabled();
                set_startup(!current);
                self.rebuild_tray();
            }
            other => {
                if let Some(name) = other.strip_prefix("layout_") {
                    let gap = self.config.settings.gap;
                    if let Some(layout) = self.config.layouts.get(name) {
                        log::info!("Menu: triggering layout '{}'", name);
                        windows::apply_layout(name, layout, gap);
                    }
                }
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if self._tray.is_none() {
            self.rebuild_tray();
            self.register_hotkeys();
            log::info!("WindowSnap started");
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Poll tray menu events
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            self.handle_menu_event(event, event_loop);
        }
        // Poll global hotkey events
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if let Some(name) = self.hotkey_layout_map.get(&event.id()).cloned() {
                let gap = self.config.settings.gap;
                if let Some(layout) = self.config.layouts.get(&name) {
                    log::info!("Hotkey: triggering layout '{}'", name);
                    windows::apply_layout(&name, layout, gap);
                }
            }
        }
    }
}

/// Parse a hotkey string like "ctrl+alt+1" into a global_hotkey::hotkey::HotKey.
fn parse_hotkey(s: &str) -> Option<HotKey> {
    use global_hotkey::hotkey::{Code, Modifiers};

    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut mods = Modifiers::empty();
    let mut key_part = "";

    for part in &parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" => mods |= Modifiers::ALT,
            "shift" => mods |= Modifiers::SHIFT,
            "super" | "win" | "meta" => mods |= Modifiers::SUPER,
            _ => key_part = part,
        }
    }

    let code = match key_part.to_lowercase().as_str() {
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        "a" => Code::KeyA,
        "b" => Code::KeyB,
        "c" => Code::KeyC,
        "d" => Code::KeyD,
        "e" => Code::KeyE,
        "f" => Code::KeyF,
        "g" => Code::KeyG,
        "h" => Code::KeyH,
        "i" => Code::KeyI,
        "j" => Code::KeyJ,
        "k" => Code::KeyK,
        "l" => Code::KeyL,
        "m" => Code::KeyM,
        "n" => Code::KeyN,
        "o" => Code::KeyO,
        "p" => Code::KeyP,
        "q" => Code::KeyQ,
        "r" => Code::KeyR,
        "s" => Code::KeyS,
        "t" => Code::KeyT,
        "u" => Code::KeyU,
        "v" => Code::KeyV,
        "w" => Code::KeyW,
        "x" => Code::KeyX,
        "y" => Code::KeyY,
        "z" => Code::KeyZ,
        "f1" => Code::F1,
        "f2" => Code::F2,
        "f3" => Code::F3,
        "f4" => Code::F4,
        "f5" => Code::F5,
        "f6" => Code::F6,
        "f7" => Code::F7,
        "f8" => Code::F8,
        "f9" => Code::F9,
        "f10" => Code::F10,
        "f11" => Code::F11,
        "f12" => Code::F12,
        "space" => Code::Space,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "escape" | "esc" => Code::Escape,
        "left" => Code::ArrowLeft,
        "right" => Code::ArrowRight,
        "up" => Code::ArrowUp,
        "down" => Code::ArrowDown,
        _ => return None,
    };

    Some(HotKey::new(Some(mods), code))
}

/// Neon cyan/magenta icon with 2x2 grid and "!" glyph — synthwave WindowSnap
fn create_icon() -> Icon {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    let center = size as f32 / 2.0;
    let radius = center - 1.0;

    // Grid cell parameters
    let pad = (size as f32 * 0.28) as i32;
    let gap = 3i32;
    let cell_w = (size as i32 - 2 * pad - gap) / 2;
    let cell_h = cell_w;

    // "!" glyph — 3px wide, centered in the icon
    // Stem: rows 5..=11, dot: row 13..=14
    let bang_x_center = size as i32 / 2;
    let bang_half_w = 1; // 3px wide

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;

            if dist <= radius {
                // Angle-based cyan→magenta gradient
                let angle = dy.atan2(dx);
                let t = (angle + std::f32::consts::PI) / (2.0 * std::f32::consts::PI);

                let edge_factor = (dist / radius).powi(3);

                // Cyan (#00fff0) → Magenta (#ff00ff)
                let glow_r = 255.0 * t;
                let glow_g = 255.0 * (1.0 - t);
                let glow_b = 240.0 + 15.0 * t;

                // Dark base: #0a0a1a
                let base_r = 10.0;
                let base_g = 10.0;
                let base_b = 26.0;

                let mut r = base_r + (glow_r - base_r) * edge_factor * 0.8;
                let mut g = base_g + (glow_g - base_g) * edge_factor * 0.8;
                let mut b = base_b + (glow_b - base_b) * edge_factor * 0.8;

                // Grid cells — neon cyan fill
                let px = x as i32;
                let py = y as i32;
                let mut in_cell = false;

                for row in 0..2 {
                    for col in 0..2 {
                        let cx0 = pad + col * (cell_w + gap);
                        let cy0 = pad + row * (cell_h + gap);
                        let cx1 = cx0 + cell_w;
                        let cy1 = cy0 + cell_h;
                        if px >= cx0 && px < cx1 && py >= cy0 && py < cy1 {
                            in_cell = true;
                        }
                    }
                }

                if in_cell {
                    // Neon cyan grid cells
                    r = 0.0;
                    g = 255.0;
                    b = 240.0;
                }

                // "!" overlay — bright magenta, drawn on top of everything
                let is_bang_stem = py >= 5 && py <= 11
                    && px >= bang_x_center - bang_half_w
                    && px <= bang_x_center + bang_half_w;
                let is_bang_dot = py >= 13 && py <= 14
                    && px >= bang_x_center - bang_half_w
                    && px <= bang_x_center + bang_half_w;

                if is_bang_stem || is_bang_dot {
                    r = 255.0;
                    g = 0.0;
                    b = 255.0;
                }

                rgba[idx] = r.clamp(0.0, 255.0) as u8;
                rgba[idx + 1] = g.clamp(0.0, 255.0) as u8;
                rgba[idx + 2] = b.clamp(0.0, 255.0) as u8;
                rgba[idx + 3] = 255;
            } else if dist <= radius + 1.0 {
                // Anti-aliased edge with gradient
                let alpha = ((1.0 - (dist - radius)) * 255.0).clamp(0.0, 255.0);
                let angle = dy.atan2(dx);
                let t = (angle + std::f32::consts::PI) / (2.0 * std::f32::consts::PI);
                rgba[idx] = (255.0 * t).clamp(0.0, 255.0) as u8;
                rgba[idx + 1] = (255.0 * (1.0 - t) * 0.5).clamp(0.0, 255.0) as u8;
                rgba[idx + 2] = (240.0 + 15.0 * t).clamp(0.0, 255.0) as u8;
                rgba[idx + 3] = alpha as u8;
            }
        }
    }

    Icon::from_rgba(rgba, size, size).expect("Failed to create icon")
}

// ─── Startup registry helpers ───
// Uses reg.exe CLI — avoids low-level Win32 registry API ergonomic issues.

const STARTUP_REG_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const STARTUP_VALUE_NAME: &str = "WindowSnap";

fn reg_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("reg");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

fn is_startup_enabled() -> bool {
    let mut cmd = reg_command();
    cmd.args(["query", STARTUP_REG_KEY, "/v", STARTUP_VALUE_NAME]);
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

fn set_startup(enable: bool) {
    if enable {
        let exe_path = std::env::current_exe()
            .map(|p| format!("\"{}\"", p.display()))
            .unwrap_or_default();
        let mut cmd = reg_command();
        cmd.args([
            "add", STARTUP_REG_KEY,
            "/v", STARTUP_VALUE_NAME,
            "/t", "REG_SZ",
            "/d", &exe_path,
            "/f",
        ]);
        match cmd.output() {
            Ok(o) if o.status.success() => log::info!("Added to startup: {}", exe_path),
            _ => log::error!("Failed to add to startup"),
        }
    } else {
        let mut cmd = reg_command();
        cmd.args([
            "delete", STARTUP_REG_KEY,
            "/v", STARTUP_VALUE_NAME,
            "/f",
        ]);
        match cmd.output() {
            Ok(o) if o.status.success() => log::info!("Removed from startup"),
            _ => log::error!("Failed to remove from startup"),
        }
    }
}
