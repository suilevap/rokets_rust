//! What each map entity type is built from.

use tilerider::animation::{GRAY, WHITE, YELLOW};
use tilerider::content::map::{EntityData, EntityType, MapData};
use tilerider::map::templates::{BodyKind, template_for};
use tilerider::render::layers::Layer;

fn entity(kind: &str, extra: &str) -> EntityData {
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Entities>
        <Item><Id>1</Id><Position>1 1</Position><Sprite>Entities\Thing</Sprite><Scale>2</Scale>
        <Type>{kind}</Type>{extra}</Item></Entities></Asset></XnaContent>"#
    );
    MapData::parse(&xml).unwrap().entities.items.remove(0)
}

#[test]
fn decor_is_only_a_sprite() {
    let t = template_for(&entity("Tile", "")).unwrap();
    assert_eq!(t.sprites[0].layer, Layer::GroundDecor);
    assert_eq!((t.body, t.boost, t.pickup, t.spin), (None, None, false, None));
}

#[test]
fn gears_are_spinning_pickups() {
    let t = template_for(&entity("Coin", "")).unwrap();
    assert!(t.pickup && t.spin.is_some() && t.body.is_none());
    assert_eq!(t.sprites[0].layer, Layer::Coin);
}

#[test]
fn boxes_are_pushable_and_heavy_ones_denser() {
    for kind in ["Box", "NonLinkableBox", "IgnoreWeaponBox"] {
        let body = template_for(&entity(kind, "")).unwrap().body.unwrap();
        assert_eq!((body.kind, body.density), (BodyKind::Dynamic, 1.0), "{kind}");
    }
    let heavy = template_for(&entity("HeavyBox", "")).unwrap().body.unwrap();
    assert_eq!(heavy.density, 64.0);
    // A damagable box ignores the map's sprite and scale.
    let damagable = template_for(&entity("DamagableBox", "")).unwrap();
    let sprite = &damagable.sprites[0];
    assert_eq!((sprite.name.as_str(), sprite.scale), ("Entities\\DamagableBox", 1.0));
}

#[test]
fn map_locks_axes_and_all_three_fix_a_box() {
    let one = template_for(&entity("Box", "<RestrictRotation>true</RestrictRotation>")).unwrap().body.unwrap();
    assert!(one.lock_rotation && !one.lock_x && one.kind == BodyKind::Dynamic);
    let all = "<RestrictX>true</RestrictX><RestrictY>true</RestrictY><RestrictRotation>true</RestrictRotation>";
    assert_eq!(template_for(&entity("Box", all)).unwrap().body.unwrap().kind, BodyKind::Static);
}

#[test]
fn accelerators_are_boosting_sensors_with_arrows() {
    let t = template_for(&entity("Accelerator", "")).unwrap();
    assert_eq!(t.body.unwrap().kind, BodyKind::Sensor);
    let boost = t.boost.as_ref().unwrap();
    assert_eq!(boost.speed, 30.0);
    // After a boost both arrows flash white, fading to gray over a second.
    let flash = boost.flash.unwrap();
    assert_eq!((flash.start, flash.end, flash.duration), (WHITE, GRAY, 1.0));
    // Two arrows, the second three quarters ahead, lit yellow in turn.
    assert_eq!(t.sprites.len(), 2);
    assert!(t.sprites.iter().all(|s| s.name == "Entities\\Accelerator"));
    assert_eq!((t.sprites[0].ahead, t.sprites[1].ahead), (0.0, 0.75));
    let lit_at = |i: usize, phase: f32| {
        let (wave, track) = t.sprites[i].tint_loop.clone().unwrap();
        track.sample(wave.phase(phase)) == YELLOW
    };
    assert!(lit_at(0, 0.1) && !lit_at(1, 0.1));
    assert!(!lit_at(0, 0.5) && lit_at(1, 0.5));
    assert!(!lit_at(0, 0.9) && !lit_at(1, 0.9));
}

#[test]
fn types_not_ported_have_no_template() {
    for kind in ["Bomb", "CoinDynamic", "Spaceship"] {
        assert!(template_for(&entity(kind, "")).is_none(), "{kind}");
    }
    assert_eq!(entity("Spaceship", "").kind, EntityType::Unknown);
}
