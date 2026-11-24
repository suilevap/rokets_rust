//! The canvas the snow maps' effects draw on, and the alpha they blend at.

use bevy::math::Vec2;
use tilerider::content::map::LevelEffect;
use tilerider::render::effect_canvas::{MIDDLING_PICTURE, blend_alpha, canvas_size, needed, shows, ui_scale};

#[test]
fn only_a_snow_map_has_the_screen_effects() {
    assert!(shows(LevelEffect::Snow));
    assert!(!shows(LevelEffect::None));
    assert!(!shows(LevelEffect::SnowWind));
}

#[test]
fn a_plain_map_has_a_canvas_for_the_speed_lines_only() {
    assert!(needed(LevelEffect::None));
    assert!(needed(LevelEffect::Snow));
    assert!(!needed(LevelEffect::SnowWind));
}

#[test]
fn the_canvas_is_landscape_whatever_way_the_window_is() {
    assert_eq!(canvas_size(Vec2::new(1280.0, 720.0)), Vec2::new(1280.0, 720.0));
    assert_eq!(canvas_size(Vec2::new(720.0, 1280.0)), Vec2::new(1280.0, 720.0));
}

#[test]
fn the_interface_scale_is_the_smaller_ratio_to_the_reference() {
    assert_eq!(ui_scale(Vec2::new(800.0, 480.0)), 1.0);
    assert_eq!(ui_scale(Vec2::new(1600.0, 960.0)), 2.0);
    // 1280 x 720 is 1.6 times as wide and 1.5 times as tall.
    assert_eq!(ui_scale(Vec2::new(1280.0, 720.0)), 1.5);
}

#[test]
fn a_veil_blends_to_the_same_number_in_linear_light() {
    let srgb = |c: f32| c.powf(1.0 / 2.2);
    for (a, picture) in [(0.2_f32, 0.5_f32), (0.5, 0.5), (0.15, 0.3), (0.5, 0.8)] {
        // XNA: the numbers mix.
        let wanted = picture + a * (1.0 - picture);
        // Bevy: the light mixes.
        let linear = picture.powf(2.2);
        let got = srgb(linear + blend_alpha(a, picture) * (1.0 - linear));
        assert!((got - wanted).abs() < 1e-3, "alpha {a} over {picture}: {got} against {wanted}");
    }
}

#[test]
fn a_veil_is_never_stronger_than_its_alpha_and_none_stays_none() {
    assert_eq!(blend_alpha(0.0, MIDDLING_PICTURE), 0.0);
    assert_eq!(blend_alpha(1.0, MIDDLING_PICTURE), 1.0);
    let mut last = 0.0;
    for i in 1..10 {
        let a = i as f32 / 10.0;
        let b = blend_alpha(a, MIDDLING_PICTURE);
        assert!(b < a && b > last, "{a} -> {b}");
        last = b;
    }
    // Over white there is nothing to add.
    assert_eq!(blend_alpha(0.4, 1.0), 0.4);
}
