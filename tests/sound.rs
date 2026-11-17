//! Sound cues: how loud a sound is from where the camera looks.

use bevy::math::Vec2;
use tilerider::sound::{HEARING, heard};

#[test]
fn sounds_fade_with_distance_and_quiet_ones_carry_less() {
    let ear = Vec2::ZERO;
    assert_eq!(heard(1.0, None, ear), 1.0, "no place: at the listener");
    assert_eq!(heard(1.0, Some(ear), ear), 1.0);
    let half = heard(1.0, Some(Vec2::new(HEARING / 2.0, 0.0)), ear);
    assert!((half - 0.5).abs() < 1e-6);
    assert_eq!(heard(1.0, Some(Vec2::new(0.0, -HEARING)), ear), 0.0, "out of earshot");
    assert_eq!(heard(0.5, Some(Vec2::new(HEARING * 0.6, 0.0)), ear), 0.0, "a quiet one carries half as far");
}
