//! Asset names are used as they are: every sprite, sound and song the
//! content and the code name is a file under `assets/` with exactly that
//! name (forward slashes, exact case). `tools/import_content.py` normalises
//! the C# content's names once; nothing in the game folds case or slashes.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use tilerider::assets::{music_path, sound_path, sprite_path};
use tilerider::content::map::MapData;
use tilerider::content::map_list::MapList;

mod common;
use common::assets_root;

/// Every file under `assets/`, as `Dir/Sub/File.ext`, with its real case.
fn files() -> BTreeSet<String> {
    fn walk(dir: &Path, prefix: &str, out: &mut BTreeSet<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            let path = format!("{prefix}{name}");
            if entry.file_type().unwrap().is_dir() {
                walk(&entry.path(), &format!("{path}/"), out);
            } else {
                out.insert(path);
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(&assets_root(), "", &mut out);
    out
}

/// The XML files the campaign plays: its maps, the parts they are built
/// from, and the cars. (Splashes are checked with the campaign's splashes in
/// `splash_screen.rs`; the menus', the other maps' and the previews' sprites
/// are not imported.)
fn xml_files(files: &BTreeSet<String>) -> Vec<String> {
    let list = MapList::parse(&fs::read_to_string(assets_root().join("Maps/MapList.xml")).unwrap()).unwrap();
    let mut out: Vec<String> = list.singleplayer.iter().map(|m| tilerider::assets::map_path(m)).collect();
    out.extend(files.iter().filter(|f| f.starts_with("Maps/Parts/") || *f == "Cars/Cars.xml").cloned());
    out
}

/// The text of every `<tag>value</tag>` in `xml`.
fn values<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        rest = &rest[i + open.len()..];
        let Some(j) = rest.find(&close) else { break };
        out.push(&rest[..j]);
        rest = &rest[j..];
    }
    out
}

/// Each name must be a file, written as the file is.
fn check(files: &BTreeSet<String>, found: &mut Vec<String>, from: &str, name: &str, path: String) {
    if name.contains('\\') || name != name.trim() {
        found.push(format!("{from}: {name:?} has a backslash or spaces"));
    } else if !files.contains(&path) {
        found.push(format!("{from}: {name:?} is not a file ({path})"));
    }
}

#[test]
fn every_name_in_the_content_is_a_file() {
    let files = files();
    let mut bad = Vec::new();
    let mut checked = 0;
    for file in &xml_files(&files) {
        let xml = fs::read_to_string(assets_root().join(file)).unwrap();
        for tag in ["Sprite", "Image", "BaseSpritePath"] {
            for name in values(&xml, tag).into_iter().filter(|n| !n.trim().is_empty()) {
                check(&files, &mut bad, file, name, sprite_path(name));
                checked += 1;
            }
        }
        for name in values(&xml, "SpritesPath").into_iter().filter(|n| !n.trim().is_empty()) {
            let prefix = format!("Sprites/{name}/");
            if name.contains('\\') || !files.iter().any(|f| f.starts_with(&prefix)) {
                bad.push(format!("{file}: sprites folder {name:?} is not a folder"));
            }
            checked += 1;
        }
        for tag in ["Sound", "EngineSound"] {
            for name in values(&xml, tag).into_iter().filter(|n| !n.trim().is_empty()) {
                check(&files, &mut bad, file, name, sound_path(name));
                checked += 1;
            }
        }
        for tag in ["MusicAmbient", "MusicDynamic1", "MusicDynamic2"] {
            for name in values(&xml, tag).into_iter().filter(|n| !n.trim().is_empty()) {
                check(&files, &mut bad, file, name, music_path(name));
                checked += 1;
            }
        }
        // `PlaySound(Name)` written as the text of an action.
        for part in xml.split("PlaySound(").skip(1) {
            let name = part.split(')').next().unwrap();
            check(&files, &mut bad, file, name, sound_path(name));
            checked += 1;
        }
    }
    assert!(checked > 1000, "only {checked} names found");
    assert!(bad.is_empty(), "{} names do not resolve:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn the_campaign_maps_and_their_images_are_files() {
    let files = files();
    let list = MapList::parse(&fs::read_to_string(assets_root().join("Maps/MapList.xml")).unwrap()).unwrap();
    assert!(list.singleplayer.items.len() > 20);
    let mut bad = Vec::new();
    for name in list.singleplayer.iter() {
        let path = tilerider::assets::map_path(name);
        if !files.contains(&path) {
            bad.push(format!("{name}: no {path}"));
            continue;
        }
        let map = MapData::parse(&fs::read_to_string(assets_root().join(&path)).unwrap()).unwrap();
        if let Some(image) = map.image.as_deref().filter(|s| !s.is_empty()) {
            check(&files, &mut bad, name, image, sprite_path(image));
        }
        for song in [&map.music_ambient, &map.music_dynamic1, &map.music_dynamic2].into_iter().flatten() {
            if !song.is_empty() {
                check(&files, &mut bad, name, song, music_path(song));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn every_sound_the_game_loads_is_a_file() {
    let files = files();
    for name in tilerider::sound::NAMES {
        assert!(files.contains(&sound_path(name)), "no sound {name}");
    }
}

/// Names the code carries are the same: every string literal under `src/`
/// that starts with a sprite folder is a sprite, with its exact case.
#[test]
fn every_sprite_the_code_names_is_a_file() {
    fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    const FOLDERS: [&str; 10] =
        ["Particles", "Entities", "UI", "Weapons", "Special", "Minimap", "Gadget", "Cars", "Hints", "Maps"];
    let files = files();
    let mut found = Vec::new();
    let mut paths = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut paths);
    let mut checked = 0;
    for path in paths {
        let text = fs::read_to_string(&path).unwrap();
        for line in text.lines().filter(|l| !l.trim_start().starts_with("//")) {
            for literal in line.split('"').skip(1).step_by(2) {
                // A Windows-style name (`Particles\\flash`) is found too: the
                // check refuses its backslash.
                let Some((folder, _)) = literal.split_once(['/', '\\']) else { continue };
                if !FOLDERS.contains(&folder) || literal.contains(['{', '.', ' ']) {
                    continue;
                }
                check(&files, &mut found, &path.display().to_string(), literal, sprite_path(literal));
                checked += 1;
            }
        }
    }
    assert!(checked > 80, "only {checked} names found");
    assert!(found.is_empty(), "{}", found.join("\n"));
}
