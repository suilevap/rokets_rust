#!/usr/bin/env python3
"""Copy the assets the Rust port uses from a TileRider (rokets) checkout.

Usage: tools/import_content.py [path/to/rokets]   (default: ../rokets)

Paths keep the original layout under assets/. The C# content names assets
the Windows way (backslashes, any case); this script normalises the names
once, here: every name in the copied XML that points at a sprite or a sound
is rewritten to forward slashes and the exact case of the file it names
(`Entities\\cargoquest` becomes `Entities/CargoQuest`), so the game loads
names as they are. Re-run it after changing maps in the editor.
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
    # The screen effects over every map: the vignette (`World.ViewDarkCorners`)
    # and the dirty lens's glass (`DirtyLensEffectDualTexture`).
    ("Sprites", "DarkCorners.png", "Sprites"),
    ("Sprites", "DirtyGlass.png", "Sprites"),
    # The snow maps' frozen glass (`FrozenGlassEffect`); the snow and the
    # mist are particle sprites, copied with `Sprites/Particles`.
    ("Sprites", "FrozenGlass*.png", "Sprites"),
    ("Sounds", "*.wav", "Sounds"),
    # The game's own font (`FontX1`, …; it has Cyrillic).
    ("Fonts", "Pixar Two.ttf", "Fonts"),
]


def index_of(root: Path) -> dict:
    """Lower-case relative path (forward slashes) -> the path with its real case."""
    return {
        p.relative_to(root).as_posix().lower(): p.relative_to(root).as_posix()
        for p in root.rglob("*")
        if p.is_file()
    }


# Elements whose text names a sprite (under `Sprites/`, without `.png`).
SPRITE_ELEMENTS = ("Sprite", "Image", "PreviewImage", "BaseSpritePath")
# Elements whose text names a folder of sprites (a car's parts).
SPRITE_FOLDER_ELEMENTS = ("SpritesPath",)
SOUND_ELEMENTS = ("Sound", "EngineSound")
MUSIC_ELEMENTS = ("MusicAmbient", "MusicDynamic1", "MusicDynamic2")

PLAY_SOUND_TEXT = re.compile(r"(PlaySound\()([^)]*)(\))")


def normalise_xml(path: Path, files: dict, folders: dict, missing: set) -> bool:
    """Rewrites the asset names in one copied XML file; True if it changed."""
    text = path.read_bytes().decode("utf-8")

    def fix(kind: str, prefix: str, ext: str):
        def sub(m):
            raw = m.group(3)
            name = raw.strip().replace("\\", "/")
            if not name:
                return m.group(0)
            if kind == "folder":
                exact = folders.get((prefix + name).lower())
                exact = exact[len(prefix):] if exact else None
            else:
                exact = files.get((prefix + name + ext).lower())
                exact = exact[len(prefix): -len(ext)] if exact else None
            if exact is None:
                missing.add((path.name, m.group(2), name))
                return m.group(1) + name + m.group(4)
            return m.group(1) + exact + m.group(4)

        return sub

    def element(names, kind, prefix, ext):
        nonlocal text
        pattern = re.compile(r"(<(%s)>)([^<]*)(</\2>)" % "|".join(names))
        text = pattern.sub(fix(kind, prefix, ext), text)

    element(SPRITE_ELEMENTS, "file", "Sprites/", ".png")
    element(SPRITE_FOLDER_ELEMENTS, "folder", "Sprites/", "")
    element(SOUND_ELEMENTS, "file", "Sounds/", ".wav")
    element(MUSIC_ELEMENTS, "file", "Sounds/Music/", ".ogg")

    def play_sound(m):
        name = m.group(2).strip()
        exact = files.get(("sounds/" + name + ".wav").lower())
        if exact is None:
            missing.add((path.name, "PlaySound()", name))
            return m.group(0)
        return m.group(1) + exact[len("Sounds/"): -len(".wav")] + m.group(3)

    text = PLAY_SOUND_TEXT.sub(play_sound, text)
    data = text.encode("utf-8")
    if data == path.read_bytes():
        return False
    path.write_bytes(data)
    return True


def normalise_names() -> None:
    files = index_of(DEST)
    folders = {
        str(Path(f).parent.as_posix()).lower(): str(Path(f).parent.as_posix())
        for f in files.values()
    }
    missing: set = set()
    changed = 0
    for path in sorted(DEST.rglob("*.xml")):
        changed += normalise_xml(path, files, folders, missing)
    print(f"normalised asset names in {changed} XML files")
    for where, element, name in sorted(missing):
        print(f"  not found: {where} <{element}> {name!r}", file=sys.stderr)


def write_map_index() -> int:
    """`assets/Maps/AllMaps.txt`: the single-player maps, one name per line.

    The map menu's "All maps" tab reads it: the browser cannot list a folder.
    Sorted by name ignoring case, as `menu::model::parse_index` sorts.
    """
    names = sorted((p.stem for p in (DEST / "Maps" / "Singleplayer").glob("*.xml")), key=lambda n: (n.lower(), n))
    (DEST / "Maps" / "AllMaps.txt").write_text("".join(n + "\n" for n in names), encoding="utf-8")
    return len(names)


def main() -> int:
    if not CONTENT.is_dir():
        print(f"content folder not found: {CONTENT}", file=sys.stderr)
        return 1
    copied = 0

    def copy_sprite(real: str) -> None:
        target = DEST / "Sprites" / real
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(CONTENT / "Sprites" / real, target)

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
    source_sprites = index_of(CONTENT / "Sprites")
    named = set()
    for m in sorted((CONTENT / "Maps").glob("**/*.xml")):
        named.update(re.findall(r"<Sprite>([^<]+)</Sprite>", m.read_text(encoding="utf-8-sig")))
    for name in sorted(named):
        real = source_sprites.get((name.strip().replace("\\", "/") + ".png").lower())
        if real and not (DEST / "Sprites" / real).exists():
            copy_sprite(real)
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
    source_music = index_of(CONTENT / "Sounds" / "Music")
    for name in sorted(songs - {""}):
        real = source_music.get((name + ".ogg").lower())
        if real:
            target = DEST / "Sounds" / "Music" / real
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(CONTENT / "Sounds" / "Music" / real, target)
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
            real = source_sprites.get(((sprite.text or "").strip().replace("\\", "/") + ".png").lower())
            if real and not (DEST / "Sprites" / real).exists():
                copy_sprite(real)
                copied += 1
    # The texts maps name by key (`ShowText`): the game's resources.
    strings = SRC / "PavXnaGame1" / "PavXnaGame1" / "Globalization"
    for path in sorted(strings.glob("Strings*.resx")):
        target = DEST / "Strings" / path.name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        copied += 1
    normalise_names()
    write_map_index()
    # The names are exact now; a manifest of the files is not needed.
    (DEST / "manifest.txt").unlink(missing_ok=True)
    print(f"copied {copied} files from {CONTENT} to {DEST}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
