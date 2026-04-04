use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub layouts: HashMap<String, Layout>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    #[serde(default = "default_gap")]
    pub gap: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { gap: default_gap() }
    }
}

fn default_gap() -> i32 {
    5
}

#[derive(Debug, Deserialize, Clone)]
pub struct Layout {
    pub hotkey: Option<String>,
    #[serde(default)]
    pub monitor: usize,
    #[serde(default)]
    pub columns: Vec<Column>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Column {
    #[serde(default)]
    pub width_percent: u32,
    #[serde(default, rename = "match")]
    pub match_rules: Vec<MatchRule>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MatchRule {
    pub title_contains: Option<String>,
    pub process_name: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        let path = Self::config_path();
        if !path.exists() {
            // Write default config on first run
            Self::config_dir(); // ensure dir exists
            fs::write(&path, DEFAULT_CONFIG)
                .map_err(|e| format!("Failed to write default config: {}", e))?;
            log::info!("Created default config at {}", path.display());
        }
        let contents = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read config at {}: {}", path.display(), e))?;
        let config: Config =
            toml::from_str(&contents).map_err(|e| format!("Failed to parse config: {}", e))?;
        Ok(config)
    }

    /// Config directory: ~/.windowsnap/
    pub fn config_dir() -> PathBuf {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".windowsnap");
        fs::create_dir_all(&path).ok();
        path
    }

    pub fn config_path() -> PathBuf {
        let mut path = Self::config_dir();
        path.push("config.toml");
        path
    }

    pub fn log_path() -> PathBuf {
        let mut path = Self::config_dir();
        path.push("windowsnap.log");
        path
    }

    /// Return {hotkey_string: layout_name} mapping.
    pub fn get_hotkey_map(&self) -> HashMap<String, String> {
        let mut mapping = HashMap::new();
        for (name, layout) in &self.layouts {
            if let Some(ref hotkey) = layout.hotkey {
                mapping.insert(hotkey.clone(), name.clone());
            }
        }
        mapping
    }
}

const DEFAULT_CONFIG: &str = r#"# WindowSnap Configuration
# Edit this file to customize layouts, hotkeys, and window matching rules.
# After editing, use "Reload Config" from the tray menu (no restart needed).

[settings]
gap = 5    # pixel gap between windows

# ─── Primary dev layout: Claude | VS Code | Terminal | Browser ───
[layouts.4-column-dev]
hotkey = "ctrl+alt+1"
monitor = 0

[[layouts.4-column-dev.columns]]
width_percent = 25
match = [{ title_contains = "Claude" }]

[[layouts.4-column-dev.columns]]
width_percent = 25
match = [{ title_contains = "Visual Studio Code" }]

[[layouts.4-column-dev.columns]]
width_percent = 25
match = [
    { title_contains = "Terminal" },
    { title_contains = "PowerShell" },
    { title_contains = "Command Prompt" },
]

[[layouts.4-column-dev.columns]]
width_percent = 25
match = [
    { title_contains = "Brave" },
    { title_contains = "Chrome" },
    { title_contains = "Firefox" },
    { title_contains = "Edge" },
]

# ─── Chat + browse: Signal | Claude | Browser (half screen) ───
[layouts.chat-browse]
hotkey = "ctrl+alt+2"
monitor = 0

[[layouts.chat-browse.columns]]
width_percent = 25
match = [{ title_contains = "Signal" }]

[[layouts.chat-browse.columns]]
width_percent = 25
match = [{ title_contains = "Claude" }]

[[layouts.chat-browse.columns]]
width_percent = 50
match = [
    { title_contains = "Brave" },
    { title_contains = "Chrome" },
    { title_contains = "Firefox" },
    { title_contains = "Edge" },
]

# ─── BlueStacks: up to 4 instances, 25% each ───
[layouts.bluestacks]
hotkey = "ctrl+alt+3"
monitor = 0

[[layouts.bluestacks.columns]]
width_percent = 25
match = [{ process_name = "HD-Player.exe" }]

[[layouts.bluestacks.columns]]
width_percent = 25
match = [{ process_name = "HD-Player.exe" }]

[[layouts.bluestacks.columns]]
width_percent = 25
match = [{ process_name = "HD-Player.exe" }]

[[layouts.bluestacks.columns]]
width_percent = 25
match = [{ process_name = "HD-Player.exe" }]

# ─── HOW TO ADD A CUSTOM LAYOUT ───
#
# [layouts.my-layout]
# hotkey = "ctrl+alt+6"          # pick any unused combo
# monitor = 0                    # 0 = primary, 1 = secondary, etc.
#
# [[layouts.my-layout.columns]]
# width_percent = 40
# match = [{ title_contains = "Slack" }]
#
# [[layouts.my-layout.columns]]
# width_percent = 60
# match = [{ title_contains = "Chrome" }]
#
# Match rules are OR'd — the first window matching any rule gets placed.
# Two match types: title_contains (window title) and process_name (exe name).
# Empty match = [] means "skip this slot".
"#;
