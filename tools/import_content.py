#!/usr/bin/env python3
"""Copy the assets the Rust port uses from a TileRider (rokets) checkout.

Usage: tools/import_content.py [path/to/rokets]   (default: ../rokets)

Paths keep the original layout under assets/, so names in the map XML
(e.g. "Entities\\CargoQuest", "Maps\\Singleplayer\\Middleware") resolve the
same way they do in the C# game. Re-run it after changing maps in the editor.
"""
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = Path(sys.argv[1] if len(sys.argv) > 1 else ROOT.parent / "rokets")
CONTENT = SRC / "PavXnaGame1" / "PavXnaGame.Content"
DEST = ROOT / "assets"

# (source relative to the content folder, glob, destination relative to assets/)
ITEMS = [
    ("Maps", "**/*.xml", "Maps"),
    ("Cars", "Cars.xml", "Cars"),
    ("Sprites/Maps", "**/*.png", "Sprites/Maps"),
    ("Sprites/Cars", "**/*.png", "Sprites/Cars"),
    ("Sprites/Entities", "**/*.png", "Sprites/Entities"),
    ("Sprites/Particles", "**/*.png", "Sprites/Particles"),
    ("Sprites", "Gradient*.png", "Sprites"),
    ("Sprites", "WhitePixel.png", "Sprites"),
    ("Sprites", "DirtyGlass.png", "Sprites"),
    ("Sprites", "DarkCorners.png", "Sprites"),
]


def main() -> int:
    if not CONTENT.is_dir():
        print(f"content folder not found: {CONTENT}", file=sys.stderr)
        return 1
    copied = 0
    for src_dir, pattern, dest_dir in ITEMS:
        base = CONTENT / src_dir
        for path in sorted(base.glob(pattern)):
            if not path.is_file():
                continue
            target = DEST / dest_dir / path.relative_to(base)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)
            copied += 1
    # The C# content paths ignore case; Linux and the web do not. The game
    # resolves names through this list (see src/assets.rs).
    files = sorted(
        str(p.relative_to(DEST)).replace("\\", "/")
        for p in DEST.rglob("*")
        if p.is_file() and p.name != "manifest.txt"
    )
    (DEST / "manifest.txt").write_text("\n".join(files) + "\n")
    print(f"copied {copied} files from {CONTENT} to {DEST}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
