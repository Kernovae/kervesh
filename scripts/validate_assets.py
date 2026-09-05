#!/usr/bin/env python3
"""Validate all clean runtime assets used by the application."""
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"


def main():
    errors = []
    print(f"Validating assets in {ASSETS}...")

    # 1. Validate SVG syntax
    svg_files = list(ASSETS.rglob("*.svg"))
    print(f"Checking {len(svg_files)} SVG files for valid XML...")
    for svg_path in svg_files:
        try:
            tree = ET.parse(svg_path)
            root_elem = tree.getroot()
            if not root_elem.tag.endswith("svg"):
                errors.append(f"Invalid root tag in {svg_path}: {root_elem.tag}")
        except Exception as e:
            errors.append(f"Failed to parse SVG {svg_path}: {e}")

    # 2. Check essential app icons
    essential_icons = [
        ASSETS / "app-icons" / "kervesh-dark-256.png",
        ASSETS / "app-icons" / "kervesh-light-256.png",
        ASSETS / "app-icons" / "kervesh.ico",
        ASSETS / "org.kernovae.Kervesh.svg",
        ASSETS / "brand" / "png" / "kervesh-mark-white-256.png",
        ASSETS / "brand" / "png" / "kervesh-mark-black-256.png",
    ]
    for icon in essential_icons:
        if not icon.is_file():
            errors.append(f"Missing essential asset {icon}")

    # 3. Check file types (dark and light 16px)
    file_types = [
        "folder", "file", "pdf", "text", "markdown", "rust", "shell", "config",
        "json", "image", "archive", "database", "key", "certificate", "executable", "log", "symlink"
    ]
    for ft in file_types:
        for theme in ["dark", "light"]:
            p = ASSETS / "file-types" / "png" / theme / "16" / f"{ft}.png"
            if not p.is_file():
                errors.append(f"Missing file type asset {p}")

    # 4. Check UI icons (dark and light)
    ui_icons_16 = [
        "back", "forward", "up", "refresh", "upload", "new-file", "new-folder",
        "download", "copy", "rename", "permissions", "delete", "pause", "cancel",
        "retry", "terminal", "host", "hosts", "inspector", "search", "bookmark",
        "connect", "disconnect", "files", "transfer"
    ]
    ui_icons_20 = ["new-connection", "split", "sftp", "monitor", "settings"]

    for icon in ui_icons_16:
        for theme in ["dark", "light"]:
            p = ASSETS / "ui-icons" / "png" / theme / "16" / f"{icon}.png"
            if not p.is_file():
                errors.append(f"Missing UI icon {p}")

    for icon in ui_icons_20:
        for theme in ["dark", "light"]:
            p = ASSETS / "ui-icons" / "png" / theme / "20" / f"{icon}.png"
            if not p.is_file():
                errors.append(f"Missing UI icon {p}")

    if errors:
        print(f"\nVALIDATION FAILED with {len(errors)} errors:")
        for err in errors:
            print(f"  - {err}")
        raise SystemExit(1)
    else:
        print(f"All runtime assets validated successfully! 0 errors.")


if __name__ == "__main__":
    main()
