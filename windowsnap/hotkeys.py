"""Global hotkey listener for WindowSnap.

Uses the `keyboard` library for global hotkey registration.
Runs in a background thread.
"""

import logging
import threading
from typing import Callable

import keyboard

logger = logging.getLogger(__name__)


class HotkeyListener:
    """Manages global hotkey registration and listening."""

    def __init__(self):
        self._registered: list[str] = []
        self._lock = threading.Lock()

    def register(self, hotkey_map: dict[str, str], callback: Callable[[str], None]) -> None:
        """Register hotkeys from a {hotkey_string: layout_name} mapping.

        Args:
            hotkey_map: e.g. {"ctrl+alt+1": "4-column-dev"}
            callback: Called with layout_name when hotkey is pressed
        """
        self.unregister_all()

        with self._lock:
            for hotkey_str, layout_name in hotkey_map.items():
                try:
                    # Create a closure that captures both the hotkey and layout name
                    def make_handler(hotkey, name):
                        def handler():
                            logger.info("Hotkey triggered: %s -> %s", hotkey, name)
                            callback(name)
                        return handler

                    keyboard.add_hotkey(hotkey_str, make_handler(hotkey_str, layout_name))
                    self._registered.append(hotkey_str)
                    logger.info("Registered hotkey: %s -> %s", hotkey_str, layout_name)
                except Exception as e:
                    logger.warning("Failed to register hotkey '%s': %s", hotkey_str, e)

    def unregister_all(self) -> None:
        """Remove all registered hotkeys."""
        with self._lock:
            for hotkey_str in self._registered:
                try:
                    keyboard.remove_hotkey(hotkey_str)
                except Exception:
                    pass
            self._registered.clear()

    def stop(self) -> None:
        """Clean shutdown."""
        self.unregister_all()
