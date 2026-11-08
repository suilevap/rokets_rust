//! Which map entities become props, and how the map locks their axes.

use avian2d::prelude::LockedAxes;
use tilerider::content::map::EntityType;
use tilerider::props::{PropKind, locked_axes};

#[test]
fn box_kinds_and_accelerators() {
    for kind in [
        EntityType::Box,
        EntityType::NonLinkableBox,
        EntityType::IgnoreWeaponBox,
        EntityType::DamagableBox,
    ] {
        assert_eq!(PropKind::from_entity_type(kind), Some(PropKind::Box { density: 1.0 }), "{kind:?}");
    }
    // A heavy box is 64 times heavier while nothing tows it.
    assert_eq!(PropKind::from_entity_type(EntityType::HeavyBox), Some(PropKind::Box { density: 64.0 }));
    assert_eq!(PropKind::from_entity_type(EntityType::Accelerator), Some(PropKind::Accelerator));
    assert_eq!(PropKind::from_entity_type(EntityType::Coin), None);
    assert_eq!(PropKind::from_entity_type(EntityType::Bomb), None);
}

#[test]
fn restrictions_lock_axes_and_all_three_fix_in_place() {
    let bits = |x, y, r| locked_axes(x, y, r).map(|a| a.to_bits());
    assert_eq!(bits(false, false, false), Some(LockedAxes::new().to_bits()));
    assert_eq!(bits(false, false, true), Some(LockedAxes::new().lock_rotation().to_bits()));
    assert_eq!(
        bits(true, false, true),
        Some(LockedAxes::new().lock_translation_x().lock_rotation().to_bits())
    );
    assert_eq!(bits(false, true, false), Some(LockedAxes::new().lock_translation_y().to_bits()));
    assert!(locked_axes(true, true, true).is_none());
}
