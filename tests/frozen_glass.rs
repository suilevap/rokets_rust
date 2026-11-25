//! The frozen glass: how fast the frost grows or melts, and how the two
//! sprites look as it does.

mod common;

use bevy::math::Vec2;
use tilerider::assets;
use tilerider::content::map::LevelEffect;
use tilerider::render::frozen_glass::{
    CRYSTALS_ALPHA, FRAME_ALPHA, FrozenGlass, SECONDS, cover, crystals, frame, grow, growth_speed, textures,
};

#[test]
fn the_glass_has_both_sprites_only_on_snow_maps() {
    assert_eq!(textures(LevelEffect::Snow).len(), 2);
    for name in textures(LevelEffect::Snow) {
        let path = assets::sprite_path(name);
        assert!(common::assets_root().join(&path).is_file(), "{path}");
    }
    assert!(textures(LevelEffect::None).is_empty());
    // Wind snow is the wind's alone, as `MapEffectFactory` made it.
    assert!(textures(LevelEffect::SnowWind).is_empty());
}

#[test]
fn it_starts_clear_and_freezes_at_the_usual_speed() {
    let glass = FrozenGlass::default();
    assert_eq!(glass.frost, 0.0);
    assert!((glass.speed - 1.0 / SECONDS).abs() < 1e-9);
}

#[test]
fn the_amount_sets_how_fast_the_frost_grows_not_how_much() {
    assert!((growth_speed(1.0) - 1.0 / SECONDS).abs() < 1e-9, "1 is the usual growth");
    assert_eq!(growth_speed(0.5), 0.0, "half holds it still");
    assert!(growth_speed(0.0) < 0.0, "none melts it");
    // `MapEffect(-5)`: eleven times as fast as it grows, back.
    assert!((growth_speed(-5.0) + 11.0 / SECONDS).abs() < 1e-9);
}

#[test]
fn the_frost_is_held_between_none_and_all() {
    assert_eq!(grow(0.99, 1.0, 1.0), 1.0);
    assert_eq!(grow(0.01, -1.0, 1.0), 0.0);
    assert!((grow(0.5, 0.1, 2.0) - 0.7).abs() < 1e-6);
}

#[test]
fn two_minutes_freeze_it_over_and_the_warm_room_melts_it_in_eleven_seconds() {
    let mut frost = 0.0;
    for _ in 0..(SECONDS as usize * 10) {
        frost = grow(frost, growth_speed(1.0), 0.1);
    }
    assert!((frost - 1.0).abs() < 1e-3, "frozen over after {SECONDS} s: {frost}");
    let mut seconds = 0.0;
    while frost > 0.0 {
        frost = grow(frost, growth_speed(-5.0), 0.1);
        seconds += 0.1;
    }
    assert!((seconds - SECONDS / 11.0).abs() < 0.2, "melted in {seconds} s");
}

#[test]
fn the_sprites_cover_the_canvas() {
    // 400 x 240 in 800 x 480: twice over, exactly.
    assert_eq!(cover(Vec2::new(800.0, 480.0), Vec2::new(400.0, 240.0)), 2.0);
    // A wider canvas is covered by the width, a taller one by the height.
    assert_eq!(cover(Vec2::new(1600.0, 480.0), Vec2::new(400.0, 240.0)), 4.0);
    assert_eq!(cover(Vec2::new(800.0, 960.0), Vec2::new(400.0, 240.0)), 4.0);
    for canvas in [Vec2::new(800.0, 480.0), Vec2::new(1280.0, 720.0), Vec2::new(640.0, 960.0)] {
        for sprite in [Vec2::new(400.0, 240.0), Vec2::new(400.0, 320.0)] {
            let size = sprite * cover(canvas, sprite);
            assert!(size.x >= canvas.x - 1e-3 && size.y >= canvas.y - 1e-3);
        }
    }
}

#[test]
fn the_crystals_fade_in_as_they_settle() {
    assert_eq!(crystals(0.0), (1.2, 0.0));
    let (scale, alpha) = crystals(1.0);
    assert_eq!(scale, 1.0);
    assert!((alpha - CRYSTALS_ALPHA).abs() < 1e-6);
    let (scale, alpha) = crystals(0.5);
    assert!((scale - 1.1).abs() < 1e-6 && (alpha - 0.1).abs() < 1e-6);
}

#[test]
fn the_frame_closes_in_at_a_steady_alpha() {
    assert_eq!(frame(0.0), (2.0, FRAME_ALPHA.0));
    assert_eq!(frame(1.0), (1.0, FRAME_ALPHA.1));
    assert_eq!(FRAME_ALPHA, (0.5, 0.5));
    assert!((frame(0.5).0 - 1.5).abs() < 1e-6);
}
