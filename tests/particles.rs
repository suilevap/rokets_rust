//! The particle pool: effects move and fade over their life, and a full
//! pool gives way to new effects without dropping the ones that matter.

use bevy::math::Vec2;
use tilerider::particles::{Pool, State, Style};

static FADE: Style = Style {
    begin: State {
        speed: 2.0,
        scale: Vec2::new(1.0, 1.0),
        view_rotation: 1.0,
        rotation: 0.0,
        color: [0.5, 0.5, 0.5, 1.0],
        height: 0.0,
    },
    end: State {
        speed: 0.0,
        scale: Vec2::new(3.0, 3.0),
        view_rotation: 1.0,
        rotation: 0.0,
        color: [0.0; 4],
        height: -1.0,
    },
    lifetime: 1.0,
    texture: "T",
    additive: false,
    random_frame: false,
    origin: Vec2::splat(0.5),
};

#[test]
fn a_style_moves_from_begin_to_end_and_fades_out() {
    let half = FADE.state_at(0.5);
    assert_eq!(half.scale, Vec2::splat(2.0));
    assert_eq!(half.speed, 1.0);
    assert_eq!(half.height, -0.5);
    // Fading to transparent keeps the colour and fades the alpha.
    assert_eq!(half.color, [0.5, 0.5, 0.5, 0.5]);
}

#[test]
fn particles_travel_along_their_direction_and_die_at_the_end_of_life() {
    let mut pool = Pool::new(4);
    pool.try_add(&FADE, Vec2::ZERO, 0.0, 0.0);
    for _ in 0..10 {
        pool.step(0.05);
    }
    let p = pool.iter().next().expect("alive at 0.5 s");
    assert!(p.position.x > 0.5 && p.position.y.abs() < 1e-6, "{}", p.position);
    assert!((p.view_direction - 0.5).abs() < 1e-5);
    for _ in 0..11 {
        pool.step(0.05);
    }
    assert_eq!(pool.iter().count(), 0);
}

#[test]
fn a_full_pool_replaces_the_oldest_optional_particle() {
    let mut pool = Pool::new(2);
    pool.add(&FADE, Vec2::new(100.0, 0.0), 0.0, 0.0);
    for x in 0..5 {
        pool.try_add(&FADE, Vec2::new(x as f32, 0.0), 0.0, 0.0);
    }
    let xs: Vec<f32> = pool.iter().map(|p| p.position.x).collect();
    assert_eq!(xs.len(), 2);
    assert!(xs.contains(&100.0), "kept: {xs:?}");
    assert!(xs.contains(&4.0), "newest: {xs:?}");

    // Once its life ends, the kept one's slot is free again.
    for _ in 0..21 {
        pool.step(0.05);
    }
    pool.add(&FADE, Vec2::ZERO, 0.0, 0.0);
    pool.add(&FADE, Vec2::ZERO, 0.0, 0.0);
    assert_eq!(pool.iter().count(), 2);
}

#[test]
fn additive_textures_are_premultiplied_as_xna_built_them() {
    use tilerider::render::premultiply::premultiply;
    // Opaque keeps its colour; half alpha halves it; clear goes black.
    assert_eq!(premultiply(&[200, 100, 50, 255, 200, 100, 50, 128, 200, 100, 50, 0]), vec![200, 100, 50, 255, 100, 50, 25, 128, 0, 0, 0, 0]);
}
