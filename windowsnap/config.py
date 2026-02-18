"""Configuration management for WindowSnap.

Handles loading, saving, and hot-reloading of YAML config.
Config lives at ~/.windowsnap/config.yaml
"""

import os
import shutil
import subprocess
import logging
from pathlib import Path
from typing import Any

import yaml

logger = logging.getLogger(__name__)

DEFAULT_CONFIG_DIR = Path.home() / ".windowsnap"
DEFAULT_CONFIG_PATH = DEFAULT_CONFIG_DIR / "config.yaml"
BUNDLED_CONFIG = Path(__file__).parent.parent / "config.yaml"


def get_default_config() -> dict[str, Any]:
    """Return the default configuration as a dict."""
    return {
        "settings": {
            "taskbar_offset": "auto",
            "gap": 5,
        },
        "layouts": {
            "4-column-dev": {
                "hotkey": "ctrl+alt+1",
                "monitor": 0,
                "columns": [
                    {
                        "width_percent": 25,
                        "match": [{"title_contains": "Claude"}],
                    },
                    {
                        "width_percent": 25,
                        "match": [{"title_contains": "Visual Studio Code"}],
                    },
                    {
                        "width_percent": 25,
                        "match": [
                            {"title_contains": "Terminal"},
                            {"title_contains": "PowerShell"},
                            {"title_contains": "Command Prompt"},
                        ],
                    },
                    {
                        "width_percent": 25,
                        "match": [
                            {"title_contains": "Brave"},
                            {"title_contains": "Chrome"},
                            {"title_contains": "Firefox"},
                            {"title_contains": "Edge"},
                        ],
                    },
                ],
            },
            "2-column-split": {
                "hotkey": "ctrl+alt+2",
                "monitor": 0,
                "columns": [
                    {"width_percent": 50, "match": []},
                    {"width_percent": 50, "match": []},
                ],
            },
            "main-sidebar": {
                "hotkey": "ctrl+alt+3",
                "monitor": 0,
                "columns": [
                    {"width_percent": 65, "match": []},
                    {"width_percent": 35, "match": []},
                ],
            },
            "fullscreen": {
                "hotkey": "ctrl+alt+4",
                "monitor": 0,
                "columns": [
                    {"width_percent": 100, "match": []},
                ],
            },
            "3-column": {
                "hotkey": "ctrl+alt+5",
                "monitor": 0,
                "columns": [
                    {"width_percent": 33, "match": []},
                    {"width_percent": 34, "match": []},
                    {"width_percent": 33, "match": []},
                ],
            },
        },
    }


class Config:
    """Manages WindowSnap configuration."""

    def __init__(self, config_path: Path | None = None):
        self.config_path = config_path or DEFAULT_CONFIG_PATH
        self.data: dict[str, Any] = {}
        self._ensure_config_exists()
        self.load()

    def _ensure_config_exists(self) -> None:
        """Create config dir and file if they don't exist."""
        self.config_path.parent.mkdir(parents=True, exist_ok=True)

        if not self.config_path.exists():
            if BUNDLED_CONFIG.exists():
                shutil.copy2(BUNDLED_CONFIG, self.config_path)
                logger.info("Copied bundled config to %s", self.config_path)
            else:
                # Write default config
                self.data = get_default_config()
                self.save()
                logger.info("Created default config at %s", self.config_path)

    def load(self) -> None:
        """Load config from YAML file."""
        try:
            with open(self.config_path, "r", encoding="utf-8") as f:
                self.data = yaml.safe_load(f) or {}
            logger.info("Config loaded from %s", self.config_path)
        except Exception as e:
            logger.error("Failed to load config: %s", e)
            self.data = get_default_config()

    def save(self) -> None:
        """Save current config to YAML file."""
        self.config_path.parent.mkdir(parents=True, exist_ok=True)
        with open(self.config_path, "w", encoding="utf-8") as f:
            yaml.dump(self.data, f, default_flow_style=False, sort_keys=False)

    def reload(self) -> None:
        """Hot-reload config from disk."""
        self.load()
        logger.info("Config reloaded")

    def open_in_editor(self) -> None:
        """Open config file in the default text editor."""
        try:
            os.startfile(str(self.config_path))
        except AttributeError:
            # Fallback for non-Windows (shouldn't happen, but just in case)
            subprocess.Popen(["xdg-open", str(self.config_path)])

    @property
    def settings(self) -> dict[str, Any]:
        return self.data.get("settings", {})

    @property
    def gap(self) -> int:
        return self.settings.get("gap", 5)

    @property
    def layouts(self) -> dict[str, Any]:
        return self.data.get("layouts", {})

    def get_layout(self, name: str) -> dict[str, Any] | None:
        return self.layouts.get(name)

    def get_hotkey_map(self) -> dict[str, str]:
        """Return {hotkey_string: layout_name} mapping."""
        mapping = {}
        for name, layout in self.layouts.items():
            hotkey = layout.get("hotkey")
            if hotkey:
                mapping[hotkey] = name
        return mapping
