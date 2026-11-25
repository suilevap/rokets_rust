//! Every map, the car catalogue and the campaign list in assets/ parse.

use std::fs;
use std::path::Path;

use tilerider::content::cars::CarCatalog;
use tilerider::content::map::MapData;
use tilerider::content::map_list::MapList;

fn assets() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"))
}

#[test]
fn every_singleplayer_map_parses() {
    let dir = assets().join("Maps/Singleplayer");
    let mut count = 0;
    let mut failures = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "xml") {
            count += 1;
            let xml = fs::read_to_string(&path).unwrap();
            if let Err(e) = MapData::parse(&xml) {
                failures.push(format!("{}: {e}", path.display()));
            }
        }
    }
    assert!(count >= 60, "only {count} maps found in {}", dir.display());
    assert!(failures.is_empty(), "maps failed to parse:\n{}", failures.join("\n"));
}

#[test]
fn every_campaign_map_exists() {
    let list = MapList::parse(&fs::read_to_string(assets().join("Maps/MapList.xml")).unwrap()).unwrap();
    assert!(!list.singleplayer.items.is_empty());
    for name in list.singleplayer.iter() {
        let path = assets().join(format!("Maps/Singleplayer/{name}.xml"));
        assert!(path.exists(), "{name} is in MapList.xml but {} is missing", path.display());
    }
}

#[test]
fn car_catalogue_resolves_base_cars() {
    let cars = CarCatalog::parse(&fs::read_to_string(assets().join("Cars/Cars.xml")).unwrap()).unwrap();
    let car = cars.get("SandBuggyEditorPointer").expect("car");
    assert_eq!(car.sprites_path.as_deref(), Some("Cars/SandBuggy"));
    assert_eq!(car.gadget.as_deref(), Some("EditorPointer"));
    assert!((car.mass - 2.97).abs() < 1e-6);
}

#[test]
fn the_first_car_of_a_name_wins() {
    // `SandBuggyMagnetMinigun` is listed twice; C#'s `GetCarInfo` takes
    // the first, with the Minigun, not the later Railgun.
    let cars = CarCatalog::parse(&fs::read_to_string(assets().join("Cars/Cars.xml")).unwrap()).unwrap();
    let car = cars.get("SandBuggyMagnetMinigun").unwrap();
    assert_eq!(car.weapon.as_deref(), Some("Minigun360"));
}

#[test]
fn texts_read_by_key_and_missing_ones_show_their_key() {
    let xml = fs::read_to_string(assets().join("Strings/Strings.resx")).unwrap();
    let strings = tilerider::content::strings::Strings::parse(&xml).unwrap();
    assert_eq!(strings.get("Be_careful"), "Be Careful");
    assert_eq!(strings.get("No_such_key"), "#No_such_key");
}

#[test]
fn a_language_falls_back_to_the_default_texts() {
    let read = |name: &str| {
        tilerider::content::strings::Strings::parse(&fs::read_to_string(assets().join(name)).unwrap()).unwrap()
    };
    let ru = read("Strings/Strings.ru.resx").over(read("Strings/Strings.resx"));
    assert_eq!(ru.get("Where_Is_My_Box"), "Где моя коробка?");
    let only = tilerider::content::strings::Strings::parse(r#"<root><data name="A"><value>a</value></data></root>"#).unwrap();
    let base = tilerider::content::strings::Strings::parse(r#"<root><data name="B"><value>b</value></data></root>"#).unwrap();
    assert_eq!(only.over(base).get("B"), "b");
}
