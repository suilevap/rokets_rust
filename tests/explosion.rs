//! Blasts: how hard they throw what is around them.

use bevy::math::Vec2;
use tilerider::explosion::{Blast, blast_impulse};

const BOMB: Blast = Blast {
    range: 2.0,
    force: 0.5,
    damage: 20,
};

#[test]
fn a_blast_throws_away_from_its_centre_and_fades_with_distance() {
    let near = blast_impulse(Vec2::ZERO, &BOMB, Vec2::new(1.0, 0.0), 0.5);
    let far = blast_impulse(Vec2::ZERO, &BOMB, Vec2::new(2.0, 0.0), 0.5);
    assert!(near.x > 0.0 && near.y == 0.0, "{near}");
    assert!(far.x > 0.0 && far.x < near.x / 2.0, "{far} vs {near}");
    let left = blast_impulse(Vec2::ZERO, &BOMB, Vec2::new(0.0, -1.0), 0.5);
    assert!(left.y < 0.0 && left.x.abs() < 1e-6);
}

#[test]
fn nothing_beyond_the_range_and_more_for_bigger_bodies() {
    assert_eq!(blast_impulse(Vec2::ZERO, &BOMB, Vec2::new(3.0, 0.0), 0.5), Vec2::ZERO);
    let small = blast_impulse(Vec2::ZERO, &BOMB, Vec2::new(1.5, 0.0), 0.3);
    let big = blast_impulse(Vec2::ZERO, &BOMB, Vec2::new(1.5, 0.0), 0.8);
    assert!(big.length() > small.length());
}

#[test]
fn a_body_on_top_of_the_blast_is_still_thrown() {
    let on_top = blast_impulse(Vec2::ZERO, &BOMB, Vec2::new(0.1, 0.0), 1.0);
    assert!(on_top.length() > 0.0);
}
