"""System tray icon and menu for WindowSnap.

Uses pystray for system tray integration and Pillow for icon generation.
"""

import logging
import sys
import os
import winreg
from pathlib import Path
from typing import Callable

import pystray
from PIL import Image, ImageDraw, ImageFont

logger = logging.getLogger(__name__)

STARTUP_REG_KEY = r"Software\Microsoft\Windows\CurrentVersion\Run"
APP_NAME = "WindowSnap"


def create_icon_image(size: int = 64) -> Image.Image:
    """Generate a simple tray icon — a stylized grid/snap symbol."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Background circle
    margin = 2
    draw.ellipse(
        [margin, margin, size - margin, size - margin],
        fill=(0, 150, 136, 230),  # Teal. Obviously.
    )

    # Draw a 2x2 grid to represent window snapping
    pad = size // 5
    gap = 3
    cell_w = (size - 2 * pad - gap) // 2
    cell_h = (size - 2 * pad - gap) // 2

    for row in range(2):
        for col in range(2):
            x0 = pad + col * (cell_w + gap)
            y0 = pad + row * (cell_h + gap)
            draw.rounded_rectangle(
                [x0, y0, x0 + cell_w, y0 + cell_h],
                radius=2,
                fill=(255, 255, 255, 220),
            )

    return img


def is_startup_enabled() -> bool:
    """Check if WindowSnap is in the Windows startup registry."""
    try:
        key = winreg.OpenKey(winreg.HKEY_CURRENT_USER, STARTUP_REG_KEY, 0, winreg.KEY_READ)
        try:
            winreg.QueryValueEx(key, APP_NAME)
            return True
        except FileNotFoundError:
            return False
        finally:
            winreg.CloseKey(key)
    except Exception:
        return False


def set_startup(enable: bool) -> None:
    """Add or remove WindowSnap from Windows startup."""
    try:
        key = winreg.OpenKey(winreg.HKEY_CURRENT_USER, STARTUP_REG_KEY, 0, winreg.KEY_SET_VALUE)
        if enable:
            # Use pythonw to avoid console window on startup
            script = str(Path(__file__).parent.parent / "windowsnap.py")
            pythonw = sys.executable.replace("python.exe", "pythonw.exe")
            if not Path(pythonw).exists():
                pythonw = sys.executable
            cmd = f'"{pythonw}" "{script}"'
            winreg.SetValueEx(key, APP_NAME, 0, winreg.REG_SZ, cmd)
            logger.info("Added to startup: %s", cmd)
        else:
            try:
                winreg.DeleteValue(key, APP_NAME)
                logger.info("Removed from startup")
            except FileNotFoundError:
                pass
        winreg.CloseKey(key)
    except Exception as e:
        logger.error("Failed to modify startup: %s", e)


class TrayApp:
    """System tray application."""

    def __init__(
        self,
        layout_names: list[str],
        on_layout_selected: Callable[[str], None],
        on_refresh: Callable[[], None],
        on_edit_config: Callable[[], None],
        on_reload_config: Callable[[], None],
        on_quit: Callable[[], None],
    ):
        self.on_layout_selected = on_layout_selected
        self.on_refresh = on_refresh
        self.on_edit_config = on_edit_config
        self.on_reload_config = on_reload_config
        self.on_quit = on_quit
        self.layout_names = layout_names

        self.icon: pystray.Icon | None = None

    def _build_menu(self) -> pystray.Menu:
        """Build the tray right-click menu."""
        # Layout items
        layout_items = []
        for name in self.layout_names:
            # Closure capture
            def make_action(layout_name):
                def action(icon, item):
                    self.on_layout_selected(layout_name)
                return action

            display_name = name.replace("-", " ").title()
            layout_items.append(pystray.MenuItem(display_name, make_action(name)))

        # Separator via a disabled item
        separator = pystray.Menu.SEPARATOR

        # Startup toggle
        def toggle_startup(icon, item):
            current = is_startup_enabled()
            set_startup(not current)

        startup_item = pystray.MenuItem(
            "Start with Windows",
            toggle_startup,
            checked=lambda item: is_startup_enabled(),
        )

        menu = pystray.Menu(
            *layout_items,
            separator,
            pystray.MenuItem("Refresh Window List", lambda icon, item: self.on_refresh()),
            pystray.MenuItem("Edit Config", lambda icon, item: self.on_edit_config()),
            pystray.MenuItem("Reload Config", lambda icon, item: self.on_reload_config()),
            separator,
            startup_item,
            separator,
            pystray.MenuItem("Quit", lambda icon, item: self.on_quit()),
        )

        return menu

    def update_layouts(self, layout_names: list[str]) -> None:
        """Update the layout list (e.g., after config reload)."""
        self.layout_names = layout_names
        if self.icon:
            self.icon.menu = self._build_menu()

    def run(self) -> None:
        """Start the tray icon. This blocks until quit is called."""
        image = create_icon_image()
        menu = self._build_menu()

        # Left-click also shows the menu (fast access)
        self.icon = pystray.Icon(
            APP_NAME,
            image,
            APP_NAME,
            menu=menu,
        )

        # pystray on Windows: left-click shows menu by default if we set
        # the default action to the first menu item, OR we can just let
        # right-click work. For left-click = menu, we use a trick:
        # Set the icon's default action handler.
        # Actually, pystray shows menu on left-click by default on Windows. Nice.

        logger.info("System tray icon started")
        self.icon.run()

    def stop(self) -> None:
        """Stop the tray icon."""
        if self.icon:
            self.icon.stop()
