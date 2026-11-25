//! The speed trail: when a car leaves a streak, how it lies, and when the
//! player's car at its top speed makes lines across the screen.

mod common;

use bevy::math::Vec2;
use tilerider::assets;
use tilerider::car::speed_trail::{
    CAMERA_LAG_MIN, CAMERA_LAG_PER_LINE, LINE_CHANCE, LINE_LEAD, camera_line_spawns, TOP_SPEED_AMOUNT, at_top_speed, line_spawns, line_start, shows, textures, trail_shape,
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
    
}

#[test]
fn the_sprites_it_draws_are_there() {
    for name in textures().into_iter().chain(["Gradient4Pixel"]) {
        let path = assets::sprite_path(name);
        assert!(common::assets_root().join(&path).is_file(), "{path}");
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

#[test]
fn a_camera_close_behind_its_focus_makes_no_lines() {
    for lag in [0.0, 5.0, CAMERA_LAG_MIN] {
        assert_eq!(camera_line_spawns(lag, 1.0 / 60.0, 0.99), 0, "lag {lag}");
    }
    // Over 16 but under 32 units: `(int)(l / 32)` is still none.
    assert_eq!(camera_line_spawns(30.0, 1.0 / 60.0, 0.99), 0);
    assert_eq!(camera_line_spawns(f32::NAN, 1.0 / 60.0, 0.99), 0);
}

#[test]
fn a_camera_far_behind_makes_a_line_for_each_32_units_at_each_update() {
    let tick = 1.0 / 60.0;
    assert_eq!(camera_line_spawns(CAMERA_LAG_PER_LINE, tick, 0.0), 1);
    assert_eq!(camera_line_spawns(63.0, tick, 0.0), 1);
    assert_eq!(camera_line_spawns(64.0, tick, 0.0), 2);
    assert_eq!(camera_line_spawns(200.0, tick, 0.0), 6);
    // A frame of three updates makes three times as many.
    assert_eq!(camera_line_spawns(64.0, 3.0 * tick, 0.0), 6);
}

#[test]
fn driving_does_not_make_the_camera_lines_a_jump_does() {
    // The camera eases at 1.9 a second, so it trails a car by speed / 1.9:
    // at 20 units a second about 10.5, far under what is needed.
    assert_eq!(camera_line_spawns(MAX / 1.9, 1.0 / 60.0, 0.99), 0);
    assert!(camera_line_spawns(80.0, 1.0 / 60.0, 0.0) > 0);
}

#[test]
fn a_slow_frame_rolls_the_rest_of_an_update_to_chance() {
    // Half an update of one line: made when the roll is over a half.
    let half = 0.5 / 60.0;
    assert_eq!(camera_line_spawns(40.0, half, 0.4), 0);
    assert_eq!(camera_line_spawns(40.0, half, 0.6), 1);
}

#[test]
fn the_camera_lines_fly_against_the_gap_from_the_same_start_as_a_cars() {
    // The camera's focus behind a point to the right: the lines start to
    // the right of the middle and fly left, as a car going right.
    let (at, dir) = line_start(Vec2::X, 1.0, Vec2::ZERO, Vec2::new(800.0, 480.0));
    assert_eq!(at, Vec2::new(LINE_LEAD, 0.0));
    assert!((Vec2::from_angle(dir) - Vec2::NEG_X).length() < 1e-5);
}
