//! A car draws only the parts it has: a turret has `Base` and the wrecks,
//! no `Wheel` or `Body` (`LoadWheelBodyColorViews` false), so none is asked
//! for, and every part a car does ask for is a file.

use std::fs;

use tilerider::assets::sprite_path;
use tilerider::content::cars::CarCatalog;

mod common;
use common::assets_root;

fn catalogue() -> CarCatalog {
    CarCatalog::parse(&fs::read_to_string(assets_root().join("Cars/Cars.xml")).unwrap()).unwrap()
}

#[test]
fn a_turret_has_no_wheel_or_body_part() {
    let cars = catalogue();
    for name in ["TurretLoader", "TurretLoaderStat"] {
        let car = cars.get(name).unwrap();
        assert_eq!(car.view_part_sprite("Wheel"), None, "{name}");
        assert_eq!(car.view_part_sprite("Body"), None, "{name}");
        assert!(car.base_sprite().is_some(), "{name}");
    }
}

#[test]
fn a_buggy_has_its_wheel_and_body_parts() {
    let cars = catalogue();
    let car = cars.get("SandBuggyMagnet").unwrap();
    assert_eq!(car.view_part_sprite("Wheel").as_deref(), Some("Cars/SandBuggy/Wheel"));
    assert_eq!(car.view_part_sprite("Body").as_deref(), Some("Cars/SandBuggy/Body"));
}

#[test]
fn every_part_a_player_car_asks_for_exists() {
    let xml = fs::read_to_string(assets_root().join("Cars/Cars.xml")).unwrap();
    let cars = catalogue();
    let mut missing = Vec::new();
    for name in xml.split("<Name>").skip(1).filter_map(|t| t.split("</Name>").next()) {
        let Some(car) = cars.get(name) else { continue };
        for part in ["Wheel", "Body"] {
            if let Some(sprite) = car.view_part_sprite(part)
                && !assets_root().join(sprite_path(&sprite)).exists()
            {
                missing.push(sprite);
            }
        }
    }
    assert!(missing.is_empty(), "missing: {missing:?}");
}
