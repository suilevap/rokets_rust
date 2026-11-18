//! Touch: the gadget's zone is the top right quarter of the screen.

use bevy::math::Vec2;
use tilerider::controls::in_gadget_zone;

#[test]
fn the_gadget_zone_is_the_top_right_quarter() {
    let screen = Vec2::new(800.0, 480.0);
    assert!(in_gadget_zone(Vec2::new(700.0, 100.0), screen));
    assert!(!in_gadget_zone(Vec2::new(700.0, 400.0), screen), "bottom right: the stick");
    assert!(!in_gadget_zone(Vec2::new(100.0, 100.0), screen), "left: the stick");
}
