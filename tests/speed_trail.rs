//! The speed trail: when a car leaves a streak, how it lies, and when the
//! player's car at its top speed makes lines across the screen.

mod common;

use bevy::math::Vec2;
use tilerider::assets;
use tilerider::car::speed_trail::{
    LINE_CHANCE, LINE_LEAD, TOP_SPEED_AMOUNT, at_top_speed, line_spawns, line_start, shows, textures, trail_shape,
};
use tilerider::content::map::LevelEffect;

/// A car 2 long and 1 wide, radius 1, top speed 20.
const SIZE: Vec2 = Vec2::new(2.0, 1.0);
const RADIUS: f32 = 1.0;
const MAX: f32 = 20.0;

fn shape(velocity: Vec2, angle: f32, roll: f32) -> Option<tilerider::car::speed_trail::TrailShape> {
    trail_shape(velocity, MAX, RADIUS, angle, SIZE, roll)
}

#[test]
fn only_a_plain_map_has_the_trail() {
    assert!(shows(LevelEffect::None));
    assert!(!shows(LevelEffect::Snow));
    assert!(!shows(LevelEffect::SnowWind));
    assert!(textures(LevelEffect::Snow).is_empty());
}

#[test]
fn the_sprites_it_draws_are_there() {
    for name in textures(LevelEffect::None).into_iter().chain(["Gradient4Pixel"]) {
        let path = assets::sprite_path(name).unwrap_or_else(|| panic!("{name} is not in the assets"));
        assert!(common::assets_root().join(path).is_file(), "{path}");
    }
}

#[test]
fn the_campaign_has_plain_maps_where_cars_have_it() {
    let (map, _) = common::load_map("LetItSnowDay");
    assert!(shows(map.level_effect));
    let (snow, _) = common::load_map("LetItSnow");
    assert!(!shows(snow.level_effect));
}

#[test]
fn a_slow_car_leaves_nothing() {
    // The length is a fifth of the speed and must pass half the radius:
    // 2.5 radii a second.
    assert_eq!(shape(Vec2::new(2.5, 0.0), 0.0, 0.5), None);
    assert_eq!(shape(Vec2::ZERO, 0.0, 0.5), None);
    assert!(shape(Vec2::new(2.6, 0.0), 0.0, 0.5).is_some());
}

#[test]
fn the_streak_trails_the_way_the_car_goes_not_the_way_it_faces() {
    // Going up while facing right (a drift): the streak points down.
    let s = shape(Vec2::new(0.0, 10.0), 0.0, 0.5).unwrap();
    assert!((Vec2::from_angle(s.dir) - Vec2::NEG_Y).length() < 1e-5, "{}", s.dir);
    let s = shape(Vec2::new(10.0, 0.0), 0.0, 0.5).unwrap();
    assert!((Vec2::from_angle(s.dir) - Vec2::NEG_X).length() < 1e-5, "{}", s.dir);
}

#[test]
fn it_is_longer_and_stronger_the_faster_the_car() {
    let slow = shape(Vec2::new(8.0, 0.0), 0.0, 1.0).unwrap();
    let fast = shape(Vec2::new(16.0, 0.0), 0.0, 1.0).unwrap();
    assert!(fast.texel.x > slow.texel.x);
    assert!(fast.alpha > slow.alpha);
    // At top speed half opaque; a third of a texel per unit of the width.
    let top = shape(Vec2::new(MAX, 0.0), 0.0, 1.0).unwrap();
    assert!((top.alpha - 0.5).abs() < 1e-6);
    assert!((top.texel.x - 1.0 / 3.0 * (MAX * 0.2 + RADIUS)).abs() < 1e-5);
    // Faster than the top speed does not make it more than opaque.
    assert!(shape(Vec2::new(MAX * 5.0, 0.0), 0.0, 1.0).unwrap().alpha <= 1.0);
}

#[test]
fn it_is_as_wide_as_the_car_seen_from_behind_and_as_long_side_on() {
    // Going the way it faces: the car's width. Sideways: its length.
    let ahead = shape(Vec2::new(10.0, 0.0), 0.0, 1.0).unwrap();
    let aside = shape(Vec2::new(0.0, 10.0), 0.0, 1.0).unwrap();
    assert!((aside.texel.y / ahead.texel.y - SIZE.x / SIZE.y).abs() < 1e-4);
    // Backwards is the same as ahead.
    let back = shape(Vec2::new(-10.0, 0.0), 0.0, 1.0).unwrap();
    assert!((back.texel.y - ahead.texel.y).abs() < 1e-5);
}

#[test]
fn it_flickers_between_half_and_the_whole_width() {
    let whole = shape(Vec2::new(10.0, 0.0), 0.0, 1.0).unwrap().texel.y;
    let half = shape(Vec2::new(10.0, 0.0), 0.0, 0.0).unwrap().texel.y;
    assert!((half / whole - 0.5).abs() < 1e-5);
    // Nothing else flickers.
    let a = shape(Vec2::new(10.0, 0.0), 0.0, 0.0).unwrap();
    let b = shape(Vec2::new(10.0, 0.0), 0.0, 1.0).unwrap();
    assert_eq!((a.dir, a.alpha, a.texel.x), (b.dir, b.alpha, b.texel.x));
}

#[test]
fn lines_need_the_player_the_stick_and_the_top_speed() {
    let fast = Vec2::new(MAX, 0.0);
    assert!(at_top_speed(true, Vec2::X, fast, MAX));
    assert!(!at_top_speed(false, Vec2::X, fast, MAX), "an AI's car makes none");
    assert!(!at_top_speed(true, Vec2::ZERO, fast, MAX), "coasting makes none");
    // The square of the speed over the maximum's must pass 0.93.
    let edge = MAX * TOP_SPEED_AMOUNT.sqrt();
    assert!(!at_top_speed(true, Vec2::X, Vec2::new(edge * 0.999, 0.0), MAX));
    assert!(at_top_speed(true, Vec2::X, Vec2::new(edge * 1.001, 0.0), MAX));
    // Over the top speed counts too.
    assert!(at_top_speed(true, Vec2::X, fast * 2.0, MAX));
}

#[test]
fn a_tick_in_four_starts_a_line_at_any_frame_rate() {
    let mean = |dt: f32| {
        let n = 1000;
        (0..n).map(|i| line_spawns(dt, (i as f32 + 0.5) / n as f32)).sum::<usize>() as f32 / n as f32
    };
    assert!((mean(1.0 / 60.0) - LINE_CHANCE).abs() < 0.01);
    assert!((mean(1.0 / 15.0) - 4.0 * LINE_CHANCE).abs() < 0.01);
    assert_eq!(line_spawns(0.0, 0.99), 0);
}

#[test]
fn a_line_starts_ahead_of_the_car_and_flies_back() {
    let canvas = Vec2::new(800.0, 480.0);
    let (at, direction) = line_start(Vec2::new(MAX, 0.0), MAX, Vec2::ZERO, canvas);
    assert!((at - Vec2::new(LINE_LEAD, 0.0)).length() < 1e-3, "{at}");
    assert!((Vec2::from_angle(direction) - Vec2::NEG_X).length() < 1e-5, "{direction}");
    // The spread is a screen either way.
    let (far, _) = line_start(Vec2::new(MAX, 0.0), MAX, Vec2::ONE, canvas);
    assert!((far - Vec2::new(LINE_LEAD + 800.0, 480.0)).length() < 1e-3, "{far}");
    // A car going up sends it flying down.
    let (_, down) = line_start(Vec2::new(0.0, MAX), MAX, Vec2::ZERO, canvas);
    assert!((Vec2::from_angle(down) - Vec2::NEG_Y).length() < 1e-5, "{down}");
}
