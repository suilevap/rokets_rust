//! The snow maps' falling snow: which maps have it, how many particles a
//! frame starts, where and which way, and the mist and wind snow.

mod common;

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::math::Vec2;
use tilerider::assets;
use tilerider::content::map::LevelEffect;
use tilerider::particles::styles::{SCREEN_SNOW, WIND_SNOW, WIND_SNOW_TURNING};
use tilerider::render::snow::{
    FLAKE_CHANCE, LEAD, MIST_ALPHA, MIST_OUT, canvas_point, lead_direction, mist, screen_spawns, screen_start,
    textures, triangle, view_half, wind_direction, wind_spawns,
};

/// The mean of `count` of a frame's draws, the roll running evenly 0 to 1.
fn mean(count: impl Fn(f32) -> usize) -> f32 {
    let n = 1000;
    (0..n).map(|i| count((i as f32 + 0.5) / n as f32)).sum::<usize>() as f32 / n as f32
}

#[test]
fn the_campaign_has_two_snow_maps() {
    for name in ["LetItSnow", "HappyNewYear"] {
        let (map, _) = common::load_map(name);
        assert_eq!(map.level_effect, LevelEffect::Snow, "{name}");
    }
    let (plain, _) = common::load_map("LetItSnowDay");
    assert_eq!(plain.level_effect, LevelEffect::None);
}

#[test]
fn every_sprite_the_snow_draws_is_there() {
    for effect in [LevelEffect::Snow, LevelEffect::SnowWind] {
        let names = textures(effect);
        assert!(!names.is_empty());
        for name in names {
            let path = assets::sprite_path(name);
            assert!(common::assets_root().join(&path).is_file(), "{path}");
        }
    }
    assert!(textures(LevelEffect::None).is_empty(), "a plain map has no snow");
}

#[test]
fn a_frame_starts_as_many_flakes_as_the_ticks_it_spans_would() {
    // At amount 1 each 1/60 s tick has a chance of half: 30 a second.
    assert!((mean(|r| screen_spawns(1.0, 1.0 / 60.0, r)) - FLAKE_CHANCE).abs() < 0.01);
    // A slow frame makes up for the ticks it spans …
    assert!((mean(|r| screen_spawns(1.0, 1.0 / 15.0, r)) - 2.0).abs() < 0.01);
    // … and the snow follows the amount.
    assert!((mean(|r| screen_spawns(0.4, 1.0 / 60.0, r)) - 0.2).abs() < 0.01);
}

#[test]
fn no_snow_falls_at_no_amount() {
    for amount in [0.0, -1.0, -5.0] {
        assert_eq!(screen_spawns(amount, 0.5, 0.99), 0, "amount {amount}");
        assert_eq!(wind_spawns(4, amount, 0.5, 0.99), 0, "amount {amount}");
    }
}

#[test]
fn the_wind_tries_four_points_a_tick_cut_to_a_whole_number() {
    assert!((mean(|r| wind_spawns(4, 1.0, 1.0 / 60.0, r)) - 4.0).abs() < 0.01);
    // `(int)(4 * 0.6)` is 2.
    assert!((mean(|r| wind_spawns(4, 0.6, 1.0 / 60.0, r)) - 2.0).abs() < 0.01);
    // Below a quarter it is none: `(int)(4 * 0.2)`.
    assert_eq!(wind_spawns(4, 0.2, 1.0 / 60.0, 0.99), 0);
}

#[test]
fn a_flake_flies_back_against_the_way_the_camera_goes() {
    let lead = lead_direction(Vec2::new(3.0, 0.0));
    assert_eq!(lead, Vec2::X);
    let (at, direction) = screen_start(lead, Vec2::ZERO, Vec2::ZERO, Vec2::new(800.0, 480.0));
    assert_eq!(at, Vec2::new(LEAD / 2.0, 0.0), "half the lead's way, shifted by 256 pixels");
    assert!((Vec2::from_angle(direction) + Vec2::X).length() < 1e-5, "it goes the other way");
    // Going up and left, it flies down and right.
    let lead = lead_direction(Vec2::new(-1.0, 1.0));
    let (_, direction) = screen_start(lead, Vec2::ZERO, Vec2::ZERO, Vec2::new(800.0, 480.0));
    let going = Vec2::from_angle(direction);
    assert!(going.x > 0.0 && going.y < 0.0);
}

#[test]
fn a_flake_starts_anywhere_a_canvas_either_way_of_the_shifted_corner() {
    let canvas = Vec2::new(800.0, 480.0);
    let (a, _) = screen_start(Vec2::ZERO, Vec2::ZERO, Vec2::new(1.0, -1.0), canvas);
    assert_eq!(a, Vec2::new(800.0, -480.0));
    let (b, _) = screen_start(Vec2::ZERO, Vec2::ZERO, Vec2::new(-1.0, 1.0), canvas);
    assert_eq!(b, Vec2::new(-800.0, 480.0));
    // The pool's y is up; the canvas's is down.
    assert_eq!(canvas_point(Vec2::new(10.0, 20.0)), Vec2::new(10.0, -20.0));
}

#[test]
fn a_standing_camera_has_no_direction_and_snow_falls_everywhere() {
    assert_eq!(lead_direction(Vec2::ZERO), Vec2::ZERO);
    assert_eq!(lead_direction(Vec2::splat(1e-4)), Vec2::ZERO);
    // Only the random vector steers.
    let (at, _) = screen_start(Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::ZERO, Vec2::new(800.0, 480.0));
    assert_eq!(at, Vec2::new(LEAD / 2.0, 0.0));
}

#[test]
fn the_screen_snow_is_mostly_flakes() {
    let of = |name: &str| SCREEN_SNOW.iter().filter(|s| s.texture.ends_with(name)).count();
    assert_eq!((of("3Pixel"), of("SnowEffect"), of("GlassDirt")), (2, 8, 1));
}

#[test]
fn the_triangle_goes_up_and_down_once_a_second() {
    assert_eq!(triangle(0.0), 0.0);
    assert!((triangle(0.25) - 0.5).abs() < 1e-6);
    assert!((triangle(0.5) - 1.0).abs() < 1e-6);
    assert!((triangle(0.75) - 0.5).abs() < 1e-6);
    assert!(triangle(3.0).abs() < 1e-5, "and again");
}

#[test]
fn the_mist_sits_outside_two_corners_and_dims_in_turn() {
    let canvas = Vec2::new(800.0, 480.0);
    // A whole turn in, the first is at its start and the triangle at 0.
    let first = mist(0, 0.0, canvas, 1.0, 1.0);
    let second = mist(1, 0.0, canvas, 1.0, 1.0);
    // Their turning point is on the corner, the sprite's middle 8 pixels
    // of its own (times 40) from it along the turn.
    assert!((first.center - (Vec2::splat(-MIST_OUT) - Vec2::X * 320.0)).length() < 1e-3);
    assert!((second.center - (canvas + Vec2::splat(MIST_OUT) - Vec2::X * 320.0)).length() < 1e-3);
    assert!((first.alpha - MIST_ALPHA * 0.5).abs() < 1e-6);
    assert!((second.alpha - MIST_ALPHA).abs() < 1e-6);
    // Half a second on, the triangle is at 1 and they have swapped.
    let (a, b) = (mist(0, 0.5, canvas, 1.0, 1.0), mist(1, 0.5, canvas, 1.0, 1.0));
    assert!((a.alpha - MIST_ALPHA).abs() < 1e-6);
    assert!((b.alpha - MIST_ALPHA * 0.5).abs() < 1e-6);
    // They turn opposite ways.
    assert!(a.rotation < 0.0 && b.rotation > 0.0);
}

#[test]
fn the_mist_follows_the_amount_and_the_windows_size() {
    let canvas = Vec2::new(1600.0, 960.0);
    assert_eq!(mist(0, 0.3, canvas, 2.0, 0.0).alpha, 0.0);
    let full = mist(0, 0.3, canvas, 2.0, 1.0);
    assert!((mist(0, 0.3, canvas, 2.0, 0.5).alpha - full.alpha / 2.0).abs() < 1e-6);
    // The turning point's distance from the middle grows with the scale.
    let corner = Vec2::splat(-MIST_OUT);
    let distance = |scale: f32| (mist(0, 0.0, canvas, scale, 1.0).center - corner).length();
    assert!((distance(2.0) - 2.0 * distance(1.0)).abs() < 1e-3);
    // The second is past the far corner of the canvas it is given.
    assert!(mist(1, 0.0, canvas, 2.0, 1.0).center.x > canvas.x - 700.0);
}

#[test]
fn wind_snow_goes_along_the_wall_not_into_it() {
    // The room grows to the right: the wall is on the left, the snow runs
    // up or down it.
    let gradient = Vec2::new(1.0, 0.0);
    let along = Vec2::from_angle(wind_direction(gradient, 0.0));
    assert!(along.dot(gradient).abs() < 1e-5);
    assert!((along - Vec2::Y).length() < 1e-5);
    // It is shaken by up to an eighth of a half turn either way.
    let shaken = wind_direction(gradient, 1.0) - wind_direction(gradient, -1.0);
    assert!((shaken - PI / 4.0).abs() < 1e-5);
    // No wall near: the gradient is zero and the angle is `GetAngle`'s 0.
    assert_eq!(wind_direction(Vec2::ZERO, 0.0), FRAC_PI_2);
}

#[test]
fn the_wind_snow_starts_in_a_box_about_the_view() {
    let half = view_half(16.0, 800.0 / 480.0);
    assert!((half.y - 16.0 * (std::f32::consts::FRAC_PI_8).tan()).abs() < 1e-5);
    assert!((half.x / half.y - 800.0 / 480.0).abs() < 1e-5);
    // Zooming out (a higher camera) widens it.
    assert!(view_half(32.0, 1.0).x > view_half(16.0, 1.0).x);
}

#[test]
fn the_wind_styles_are_the_streak_and_the_swirl() {
    assert!(!WIND_SNOW.random_frame && WIND_SNOW.texture.ends_with("3Pixel"));
    assert!(WIND_SNOW_TURNING.random_frame && WIND_SNOW_TURNING.texture.ends_with("SnowEffect"));
    // The swirl curves, the streak does not.
    assert_ne!(WIND_SNOW_TURNING.begin.rotation, 0.0);
    assert_eq!(WIND_SNOW.begin.rotation, 0.0);
}
