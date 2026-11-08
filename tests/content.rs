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
    assert_eq!(car.sprites_path.as_deref(), Some("Cars\\SandBuggy"));
    assert_eq!(car.gadget.as_deref(), Some("EditorPointer"));
    assert!((car.mass - 2.97).abs() < 1e-6);
}
