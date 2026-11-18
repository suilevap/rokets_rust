//! Respawn points and lives.

use bevy::math::Vec2;
use tilerider::game_mode::Respawn;

#[test]
fn without_a_point_death_restarts_and_with_one_three_lives_bring_it_back() {
    let mut r = Respawn::default();
    assert!(!r.on_death(), "no respawn point yet: restart");
    assert_eq!(r.lives, 3, "a restart costs no life");
    r.point = Some((Vec2::ZERO, 0.0));
    assert!(r.on_death() && r.on_death() && r.on_death());
    assert_eq!(r.lives, 0);
    assert!(!r.on_death(), "the fourth death restarts the level");
}
