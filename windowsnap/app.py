"""Main application — ties together config, window engine, hotkeys, and tray."""

import logging
import sys
import threading

from windowsnap.config import Config
from windowsnap.windows import apply_layout, get_visible_windows
from windowsnap.hotkeys import HotkeyListener
from windowsnap.tray import TrayApp

logger = logging.getLogger(__name__)


class WindowSnap:
    """Main application controller."""

    def __init__(self):
        self.config = Config()
        self.hotkey_listener = HotkeyListener()
        self.tray: TrayApp | None = None
        self._config_lock = threading.Lock()

    def trigger_layout(self, layout_name: str) -> None:
        """Apply a layout by name. Thread-safe — called from hotkey thread."""
        with self._config_lock:
            layout = self.config.get_layout(layout_name)
            gap = self.config.gap
        if not layout:
            logger.warning("Layout '%s' not found in config", layout_name)
            return

        logger.info("Triggering layout: %s", layout_name)
        try:
            apply_layout(layout_name, layout, gap)
        except Exception as e:
            logger.error("Error applying layout '%s': %s", layout_name, e)

    def refresh_windows(self) -> None:
        """Refresh the internal window list (logs what's available)."""
        windows = get_visible_windows()
        logger.info("Visible windows (%d):", len(windows))
        for w in windows:
            logger.info("  [%s] %s (minimized=%s)",
                       w.process_name or "?", w.title, w.is_minimized)

    def edit_config(self) -> None:
        """Open config in default editor."""
        self.config.open_in_editor()

    def reload_config(self) -> None:
        """Hot-reload config and re-register hotkeys. Thread-safe."""
        with self._config_lock:
            self.config.reload()
            self._register_hotkeys()
            if self.tray:
                self.tray.update_layouts(list(self.config.layouts.keys()))
        logger.info("Config reloaded and hotkeys re-registered")

    def _register_hotkeys(self) -> None:
        """Register all hotkeys from current config."""
        hotkey_map = self.config.get_hotkey_map()
        self.hotkey_listener.register(hotkey_map, self.trigger_layout)

    def quit(self) -> None:
        """Clean shutdown."""
        logger.info("Shutting down WindowSnap")
        self.hotkey_listener.stop()
        if self.tray:
            self.tray.stop()

    def run(self) -> None:
        """Start the application."""
        logger.info("Starting WindowSnap")

        # Register hotkeys
        self._register_hotkeys()

        # Create and run tray (blocks on main thread)
        self.tray = TrayApp(
            layout_names=list(self.config.layouts.keys()),
            on_layout_selected=self.trigger_layout,
            on_refresh=self.refresh_windows,
            on_edit_config=self.edit_config,
            on_reload_config=self.reload_config,
            on_quit=self.quit,
        )

        self.tray.run()


def main():
    """Entry point with logging setup."""
    # Configure logging
    log_format = "%(asctime)s [%(levelname)s] %(name)s: %(message)s"
    logging.basicConfig(
        level=logging.INFO,
        format=log_format,
        handlers=[
            logging.StreamHandler(sys.stdout),
        ],
    )
    # Keep third-party loggers quiet
    logging.getLogger("PIL").setLevel(logging.WARNING)
    logging.getLogger("keyboard").setLevel(logging.WARNING)

    # Also log to file for debugging
    try:
        from windowsnap.config import DEFAULT_CONFIG_DIR
        log_file = DEFAULT_CONFIG_DIR / "windowsnap.log"
        DEFAULT_CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        file_handler = logging.FileHandler(log_file, encoding="utf-8")
        file_handler.setFormatter(logging.Formatter(log_format))
        logging.getLogger().addHandler(file_handler)
    except Exception:
        pass

    app = WindowSnap()
    try:
        app.run()
    except KeyboardInterrupt:
        app.quit()
    except Exception as e:
        logger.critical("Fatal error: %s", e, exc_info=True)
        sys.exit(1)
