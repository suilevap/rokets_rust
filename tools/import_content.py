#!/usr/bin/env python3
"""Copy the assets the Rust port uses from a TileRider (rokets) checkout.

Usage: tools/import_content.py [path/to/rokets]   (default: ../rokets)

Paths keep the original layout under assets/, so names in the map XML
(e.g. "Entities\\CargoQuest", "Maps\\Singleplayer\\Middleware") resolve the
same way they do in the C# game. Re-run it after changing maps in the editor.
"""
import re
import shutil
import sys
from xml.etree import ElementTree
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
    ("Sprites/Gadget", "**/*.png", "Sprites/Gadget"),
    ("Sprites/Weapons", "*.png", "Sprites/Weapons"),
    # The aim line's strips.
    ("Sprites/Special", "LaserLine*.png", "Sprites/Special"),
    # The route line `AddPathComponent` draws (`DottedSpritePath`).
    ("Sprites/Special", "DottedLine.png", "Sprites/Special"),
    ("Sprites/Minimap", "*.png", "Sprites/Minimap"),
    # The Zombie's wave ring (`CreateZombieCircleEffect`).
    ("Sprites/UI", "BigCircle.png", "Sprites/UI"),
    # The rings of `SetWeapon`'s "collected" effect (`CreateWeaponCollectedUI`,
    # `CreateMagnetCollectedUI`).
    ("Sprites/UI", "BigCircleGun.png", "Sprites/UI"),
    ("Sprites/UI", "BigCircleGadget.png", "Sprites/UI"),
    # The ring that closes onto the car while a gadget's button is held
    # (`ActiveGadget.DrawControl`).
    ("Sprites/UI", "GadgetUsage*.png", "Sprites/UI"),
    # The weapon's and gadget's icons beside the health bar
    # (`PlayerCarControlBase.DrawGadgetIcon`).
    ("Sprites/UI/GadgetIcons", "*.png", "Sprites/UI/GadgetIcons"),
    ("Sprites", "Gradient*.png", "Sprites"),
    ("Sprites", "WhitePixel.png", "Sprites"),
    ("Sounds", "*.wav", "Sounds"),
    # The game's own font (`FontX1`, …; it has Cyrillic).
    ("Fonts", "Pixar Two.ttf", "Fonts"),
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
    # Sprites the maps name (`<Sprite>`) from folders not copied whole,
    # such as `UI\IconLikeCircle` for `AttachAdditionalView`.
    named = set()
    for m in sorted((CONTENT / "Maps").glob("**/*.xml")):
        named.update(re.findall(r"<Sprite>([^<]+)</Sprite>", m.read_text(encoding="utf-8-sig")))
    for name in sorted(named):
        rel = Path("Sprites") / (name.strip().replace("\\", "/") + ".png")
        if (CONTENT / rel).is_file() and not (DEST / rel).exists():
            (DEST / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(CONTENT / rel, DEST / rel)
            copied += 1
    # The songs the single-player maps play (`MusicAmbient`, `MusicDynamic1`,
    # `MusicDynamic2`, by file name; as `PlayLevelMusic` plays them): the
    # `.ogg` the DesktopGL content project builds. The menus' and the level
    # end's songs stay behind, as do other maps' (not in the content folder).
    songs = set()
    for m in sorted((CONTENT / "Maps" / "Singleplayer").glob("*.xml")):
        songs.update(
            s.strip()
            for s in re.findall(r"<Music(?:Ambient|Dynamic1|Dynamic2)>([^<]*)<", m.read_text(encoding="utf-8-sig"))
        )
    for name in sorted(songs - {""}):
        rel = Path("Sounds") / "Music" / (name + ".ogg")
        if (CONTENT / rel).is_file():
            (DEST / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(CONTENT / rel, DEST / rel)
            copied += 1
    # The splashes (`ShowSplash`): their layout, and the sprites of the
    # ones the maps show (the menus' and the credits' stay behind).
    splashes = CONTENT / "Splashes" / "Splashes.xml"
    target = DEST / "Splashes" / "Splashes.xml"
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(splashes, target)
    copied += 1
    shown = set()
    for m in sorted((CONTENT / "Maps").glob("**/*.xml")):
        shown.update(re.findall(r"<SplashName>([^<]+)</SplashName>", m.read_text(encoding="utf-8-sig")))
    shown.update(n + s for n in list(shown) for s in ("_Win", "_GamePad"))
    tree = ElementTree.fromstring(splashes.read_text(encoding="utf-8-sig"))
    for item in tree.findall("./Asset/Splashes/Item"):
        if item.findtext("Name") not in shown:
            continue
        for sprite in item.findall("./Layers/Item/Sprite"):
            rel = Path("Sprites") / ((sprite.text or "").strip().replace("\\", "/") + ".png")
            if (sprite.text or "").strip() and (CONTENT / rel).is_file() and not (DEST / rel).exists():
                (DEST / rel).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(CONTENT / rel, DEST / rel)
                copied += 1
    # The texts maps name by key (`ShowText`): the game's resources.
    strings = SRC / "PavXnaGame1" / "PavXnaGame1" / "Globalization"
    for path in sorted(strings.glob("Strings*.resx")):
        target = DEST / "Strings" / path.name
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
