"""Build WindowSnap into a single .exe using PyInstaller.

Usage:
    pip install pyinstaller
    python build.py

Output: dist/WindowSnap.exe
"""

import subprocess
import sys

def main():
    cmd = [
        sys.executable, "-m", "PyInstaller",
        "--onefile",
        "--windowed",              # No console window on launch
        "--name", "WindowSnap",
        "--icon", "NONE",          # Uses default; swap with .ico path if you make one
        "--add-data", "config.yaml;.",  # Bundle the default config alongside the exe
        "--hidden-import", "win32gui",
        "--hidden-import", "win32con",
        "--hidden-import", "win32api",
        "--hidden-import", "win32process",
        "--hidden-import", "pystray._win32",  # pystray backend for Windows
        "windowsnap.py",
    ]

    print("Building WindowSnap.exe...")
    print(f"Command: {' '.join(cmd)}\n")

    result = subprocess.run(cmd)

    if result.returncode == 0:
        print("\n" + "=" * 50)
        print("Build complete! Your exe is at: dist\\WindowSnap.exe")
        print("Share that single file — it includes everything.")
        print("=" * 50)
    else:
        print(f"\nBuild failed with exit code {result.returncode}")
        sys.exit(1)


if __name__ == "__main__":
    main()
