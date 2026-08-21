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
    /// Optional absolute left edge, as a percentage of the work area.
    ///
    /// When set, the column is positioned at that offset instead of flowing
    /// after its predecessor, which is what makes overlap expressible. Absolute
    /// columns sit outside the tiling flow entirely: they neither consume gaps
    /// nor shift their neighbours. Omit it and the column tiles exactly as
    /// before.
    pub x_percent: Option<u32>,
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

        // Surface layout shape up front: the motivating bug (issue #8) was invisible
        // until a layout was actually triggered, because nothing reported how many
        // columns a layout had ended up with.
        log::info!("Loaded config: {}", config.describe_layouts());
        for warning in config.validate() {
            log::warn!("{}", warning);
        }

        Ok(config)
    }

    /// One-line summary of every layout and its column count, sorted by name.
    fn describe_layouts(&self) -> String {
        if self.layouts.is_empty() {
            return "no layouts".to_string();
        }
        let mut shapes: Vec<String> = self
            .layouts
            .iter()
            .map(|(name, layout)| format!("{} ({} cols)", name, layout.columns.len()))
            .collect();
        shapes.sort();
        format!("{} layout(s): {}", shapes.len(), shapes.join(", "))
    }

    /// Sanity-check every layout, returning human-readable warnings sorted by
    /// layout name (layouts live in a HashMap, so unsorted output would shuffle
    /// between runs and make log diffs useless).
    ///
    /// The only rule that matters is "stays on screen", never "sums to 100".
    /// Tiled widths summing UNDER 100 are fine — the last tiled column stretches
    /// to absorb the slack. Over 100 is always broken: later columns march off
    /// the right edge and the last is handed a negative width.
    ///
    /// These are warnings, never errors. Refusing to load the config over one
    /// bad layout would lock the user out of every *other* layout too.
    pub fn validate(&self) -> Vec<String> {
        let mut warnings: Vec<String> = self
            .layouts
            .iter()
            .flat_map(|(name, layout)| Self::layout_warnings(name, layout))
            .collect();
        warnings.sort();
        warnings
    }

    /// Every on-screen problem found in one layout. A layout can have more than
    /// one, so this returns a list rather than the first hit.
    fn layout_warnings(name: &str, layout: &Layout) -> Vec<String> {
        if layout.columns.is_empty() {
            return vec![format!(
                concat!(
                    "Layout '{name}' has no columns, so its hotkey will do nothing. ",
                    "Note that [[layouts.<name>.columns]] appends to <name> no matter ",
                    "where the block sits in the file - check whether the column ",
                    "blocks you meant for this layout are naming a different one.",
                ),
                name = name
            )];
        }

        let mut warnings = Vec::new();

        // Absolutely positioned columns stand alone: each one either fits within
        // the work area or hangs off the right edge, regardless of its siblings.
        for (i, col) in layout.columns.iter().enumerate() {
            let Some(x) = col.x_percent else { continue };
            // saturating: a nonsense value shouldn't panic a debug build.
            let right_edge = x.saturating_add(col.width_percent);
            if right_edge > 100 {
                warnings.push(format!(
                    concat!(
                        "Layout '{name}' column {i} sits at x_percent {x} with width_percent ",
                        "{width}, so it ends at {right_edge}% and runs off the right edge of ",
                        "the screen. Keep x_percent + width_percent at or under 100.",
                    ),
                    name = name,
                    i = i,
                    x = x,
                    width = col.width_percent,
                    right_edge = right_edge
                ));
            }
        }

        // Only the tiled columns share the row between them; absolute ones sit
        // outside the flow and must not count toward the total.
        let tiled = layout.columns.iter().filter(|c| c.x_percent.is_none());
        let count = tiled.clone().count();
        let sum = tiled.fold(0u32, |acc, c| acc.saturating_add(c.width_percent));
        if sum > 100 {
            warnings.push(format!(
                concat!(
                    "Layout '{name}' has {count} tiled column(s) whose width_percent sums ",
                    "to {sum}%. Tiled columns are laid out left to right, never stacked, so ",
                    "anything over 100% runs off the right edge of the screen instead of ",
                    "overlapping - the last one ends up with a negative width. Give a column ",
                    "an x_percent if you actually want it to overlap, and if you pasted a ",
                    "column block in, check that it names the right layout.",
                ),
                name = name,
                count = count,
                sum = sum
            ));
        }

        warnings
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
#
# TWO RULES THAT BITE:
#  1. [[layouts.<name>.columns]] appends to the layout it NAMES, no matter where
#     the block sits in this file. Copying a column block into another layout's
#     section does nothing unless you change <name> too — otherwise it silently
#     piles onto the original layout.
#  2. width_percent values are literal percentages of the monitor and are never
#     normalized. Under 100 is fine — the last column stretches to fill the gap.
#     OVER 100 is broken: columns tile left to right, so the excess marches off
#     the right edge of the screen. If you want windows to OVERLAP rather than
#     tile, give them an x_percent — see "OVERLAPPING WINDOWS" at the bottom.
# WindowSnap warns about both in ~/.windowsnap/windowsnap.log when it loads.

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
    { process_name = "WindowsTerminal.exe" },
    { process_name = "powershell.exe" },
    { process_name = "pwsh.exe" },
    { process_name = "cmd.exe" },
    { title_contains = "Terminal" },
]

[[layouts.4-column-dev.columns]]
width_percent = 25
match = [
    { process_name = "brave.exe" },
    { process_name = "chrome.exe" },
    { process_name = "firefox.exe" },
    { process_name = "msedge.exe" },
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
    { process_name = "brave.exe" },
    { process_name = "chrome.exe" },
    { process_name = "firefox.exe" },
    { process_name = "msedge.exe" },
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
# Two match types (both case-insensitive substring checks):
#   title_contains — window title. Fragile: breaks if the window is renamed.
#   process_name   — executable name (e.g. "powershell.exe"). Survives renames;
#                    prefer this for terminals and browsers.
# A single rule with BOTH fields requires both to match (AND), e.g.
#   { process_name = "powershell.exe", title_contains = "admin" }
# If several windows match, the topmost one wins — use an AND rule to pin a
# specific window (e.g. a terminal you renamed after your project).
# Empty match = [] means "skip this slot".
#
# ─── OVERLAPPING WINDOWS ───
#
# By default columns TILE: each one starts where the previous ended, so more
# columns always means narrower windows. On a single monitor that runs out fast
# — five columns leaves every window too narrow to use.
#
# Add x_percent to a column to place it at an absolute position instead. It then
# sits outside the tiling flow: it does not shift its neighbours, and it may
# overlap them. Columns declared LATER are stacked on top.
#
# [[layouts.focus.columns]]
# width_percent = 45              # tiled as usual
# match = [{ process_name = "doc-md.exe" }]
#
# [[layouts.focus.columns]]
# width_percent = 55              # tiled, fills the rest of the row
# match = [{ title_contains = "Brave" }]
#
# [[layouts.focus.columns]]
# x_percent = 0                   # absolute: pinned to the left edge...
# width_percent = 22              # ...and laid ON TOP of the two above
# match = [{ title_contains = "Claude" }]
#
# Rule of thumb: x_percent + width_percent should stay at or under 100, or the
# column hangs off the right edge. WindowSnap warns in the log if it does.
#
# Remember: the layout name in [[layouts.<name>.columns]] is what decides which
# layout a column joins — not where you paste it.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let config: Config = toml::from_str(DEFAULT_CONFIG).expect("default config must parse");
        assert!(!config.layouts.is_empty());
        // The terminal column of the dev layout must carry process-based rules.
        let dev = &config.layouts["4-column-dev"];
        let terminal_col = &dev.columns[2];
        assert!(terminal_col
            .match_rules
            .iter()
            .any(|r| r.process_name.as_deref() == Some("powershell.exe")));
    }

    fn parse(toml_str: &str) -> Config {
        toml::from_str(toml_str).expect("test config must parse")
    }

    #[test]
    fn shipped_default_config_is_warning_free() {
        // Guards our own template: if we ship a layout that doesn't sum to 100,
        // every user sees a warning on first run.
        assert_eq!(parse(DEFAULT_CONFIG).validate(), Vec::<String>::new());
    }

    #[test]
    fn widths_summing_to_100_produce_no_warnings() {
        let config = parse(
            r#"
[[layouts.good.columns]]
width_percent = 30
[[layouts.good.columns]]
width_percent = 70
"#,
        );
        assert_eq!(config.validate(), Vec::<String>::new());
    }

    #[test]
    fn overfull_layout_warns_with_name_and_sum() {
        // The real-world case from issue #8: column blocks pasted under the wrong
        // table name piled onto one layout, pushing windows off the screen.
        let config = parse(
            r#"
[[layouts.4-column-dev.columns]]
width_percent = 25
[[layouts.4-column-dev.columns]]
width_percent = 50
[[layouts.4-column-dev.columns]]
width_percent = 50
"#,
        );
        let warnings = config.validate();
        assert_eq!(
            warnings.len(),
            1,
            "expected one warning, got {:?}",
            warnings
        );
        assert!(warnings[0].contains("4-column-dev"), "{}", warnings[0]);
        assert!(warnings[0].contains("125"), "{}", warnings[0]);
        assert!(
            warnings[0].contains('3'),
            "column count missing: {}",
            warnings[0]
        );
    }

    #[test]
    fn underfull_layout_is_silent() {
        // Summing under 100 is NOT a problem: calculate_slots hands the leftover
        // width to the last column, so everything still lands on screen. Warning
        // here would be noise — the rule is "stays on screen", not "sums to 100".
        // See windows::tests::underfull_widths_still_reach_the_screen_edge.
        let config = parse(
            r#"
[[layouts.sparse.columns]]
width_percent = 40
"#,
        );
        assert_eq!(config.validate(), Vec::<String>::new());
    }

    #[test]
    fn absolute_column_running_past_the_right_edge_warns() {
        let config = parse(
            r#"
[[layouts.spill.columns]]
x_percent = 70
width_percent = 50
"#,
        );
        let warnings = config.validate();
        assert_eq!(warnings.len(), 1, "expected one warning, got {:?}", warnings);
        assert!(warnings[0].contains("120"), "{}", warnings[0]);
        assert!(warnings[0].contains("x_percent"), "{}", warnings[0]);
    }

    #[test]
    fn overlapping_absolute_columns_are_not_a_problem() {
        // Two windows deliberately sharing the middle of the screen. Both fit,
        // so there is nothing to warn about even though they sum to 115%.
        let config = parse(
            r#"
[[layouts.overlap.columns]]
x_percent = 0
width_percent = 60
[[layouts.overlap.columns]]
x_percent = 45
width_percent = 55
"#,
        );
        assert_eq!(config.validate(), Vec::<String>::new());
    }

    #[test]
    fn absolute_columns_are_excluded_from_the_tiled_sum() {
        // The tiled columns fill the row exactly; the absolute one sits on top of
        // them. Counting it toward the total would produce a bogus warning.
        let config = parse(
            r#"
[[layouts.mixed.columns]]
width_percent = 40
[[layouts.mixed.columns]]
width_percent = 60
[[layouts.mixed.columns]]
x_percent = 0
width_percent = 25
"#,
        );
        assert_eq!(config.validate(), Vec::<String>::new());
    }

    #[test]
    fn layout_with_no_columns_warns_clearly() {
        // Same typo class: the section exists but every column block named a
        // different layout. "sums to 0%" would be a confusing way to say this.
        let config = parse(
            r#"
[layouts.orphan]
hotkey = "ctrl+alt+9"
"#,
        );
        let warnings = config.validate();
        assert_eq!(
            warnings.len(),
            1,
            "expected one warning, got {:?}",
            warnings
        );
        assert!(warnings[0].contains("orphan"), "{}", warnings[0]);
        assert!(warnings[0].contains("no columns"), "{}", warnings[0]);
    }

    #[test]
    fn warnings_are_sorted_for_stable_log_output() {
        // Layouts live in a HashMap, so unsorted output would shuffle per run.
        let config = parse(
            r#"
[[layouts.zeta.columns]]
width_percent = 150
[[layouts.alpha.columns]]
width_percent = 150
[[layouts.mid.columns]]
width_percent = 150
"#,
        );
        let warnings = config.validate();
        assert_eq!(warnings.len(), 3);
        let mut sorted = warnings.clone();
        sorted.sort();
        assert_eq!(warnings, sorted, "validate() must return sorted warnings");
    }
}
