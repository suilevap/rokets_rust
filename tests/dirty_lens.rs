//! The dirty lens over the picture: which maps have it, how big and how
//! bright each lamp's blob is, which part of the glass the screen shows, and
//! how the explosion's and the laser's flashes of it start, shrink and fade.

mod common;

use bevy::math::Vec2;
use tilerider::content::map::LevelEffect;
use tilerider::content::xna::XColor;
use tilerider::render::dirty_lens::{
    BLOB_PX, CROSSING_CHANCE, Flash, GLASS_ASPECT, LIGHT_SCALE, MAX_FLASHES, MAX_RADIUS, MIN_RADIUS, blob_color,
    blob_radius, explosion_radius, flash_color, flash_state, gain, glass_window, shows,
};

#[test]
fn a_plain_map_has_the_lens_and_a_snow_map_does_not() {
    assert!(shows(LevelEffect::None));
    assert!(!shows(LevelEffect::Snow));
    assert!(!shows(LevelEffect::SnowWind));
}

#[test]
fn maps_say_which_effect_they_have() {
    let (campaign, _) = common::load_map("LetItSnowDay");
    assert_eq!(campaign.level_effect, LevelEffect::None);
    let (snowy, _) = common::load_map("LetItSnow");
    assert_eq!(snowy.level_effect, LevelEffect::Snow);
}

#[test]
fn a_blob_is_as_big_as_its_lamp_up_to_a_limit() {
    // A 40-cell lamp on a map of 3-unit cells is far past the limit.
    assert_eq!(blob_radius(40.0, 3.0), LIGHT_SCALE * MAX_RADIUS);
    // A tiny one is held at the least.
    assert_eq!(blob_radius(0.1, 3.0), LIGHT_SCALE * MIN_RADIUS);
    // In between it follows the lamp: 3 cells of 2 units are 6 units.
    assert!((blob_radius(3.0, 2.0) - LIGHT_SCALE * 6.0).abs() < 1e-6);
    // Bigger lamps never get smaller blobs.
    let radii: Vec<f32> = (1..30).map(|r| blob_radius(r as f32, 1.0)).collect();
    assert!(radii.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn a_blob_has_the_lamps_colour_dimmed_by_its_alpha() {
    let [r, g, b] = blob_color(XColor::rgba(255, 255, 29, 255));
    assert_eq!((r, g), (1.0, 1.0));
    assert!((b - 29.0 / 255.0).abs() < 1e-6, "the blue of 1D");
    // A faint lamp (alpha 0x38) adds a faint blob.
    let [r, _, _] = blob_color(XColor::rgba(255, 255, 255, 0x38));
    assert!((r - 0x38 as f32 / 255.0).abs() < 1e-6);
    // No alpha, no light.
    assert_eq!(blob_color(XColor::rgba(255, 128, 0, 0)), [0.0; 3]);
}

#[test]
fn the_gain_is_the_effects_alpha_twice_and_the_shaders_two() {
    assert!((gain(1.0) - 2.0 * 0.6 * 0.6).abs() < 1e-6);
    assert!((gain(2.0) - 2.0 * gain(1.0)).abs() < 1e-6);
    assert_eq!(gain(0.0), 0.0);
}

#[test]
fn the_glass_fills_a_screen_of_its_own_shape() {
    let (scale, offset) = glass_window(GLASS_ASPECT);
    assert_eq!((scale, offset), (Vec2::ONE, Vec2::ZERO));
}

#[test]
fn a_wider_screen_crops_the_glass_top_and_bottom_a_narrower_one_the_sides() {
    let (scale, offset) = glass_window(GLASS_ASPECT * 2.0);
    assert_eq!(scale.x, 1.0);
    assert!((scale.y - 0.5).abs() < 1e-6, "half the height shows");
    assert!((offset.y - 0.25).abs() < 1e-6, "the middle half");

    let (scale, offset) = glass_window(GLASS_ASPECT / 2.0);
    assert_eq!(scale.y, 1.0);
    assert!((scale.x - 0.5).abs() < 1e-6);
    assert!((offset.x - 0.25).abs() < 1e-6);
}

#[test]
fn the_glass_window_is_centred_and_never_outside_the_glass() {
    for aspect in [1.0, 1.2, GLASS_ASPECT, 2.0, 2.4, 3.5] {
        let (scale, offset) = glass_window(aspect);
        assert!(scale.x > 0.0 && scale.y > 0.0 && scale.x <= 1.0 && scale.y <= 1.0);
        let far = offset + scale;
        assert!((far.x - (1.0 - offset.x)).abs() < 1e-6, "centred across, at {aspect}");
        assert!((far.y - (1.0 - offset.y)).abs() < 1e-6, "centred down, at {aspect}");
    }
}

#[test]
fn an_explosion_flash_is_a_yellow_blob_of_48_units_shrinking_to_32_in_a_second() {
    let look = Flash::Explosion.look();
    assert_eq!(look.color, [1.0, 1.0, 0.0]);
    assert_eq!(look.lifetime, 1.0);
    // `flash2` is 32 pixels, a world unit 32: the scale is the width.
    assert_eq!(BLOB_PX, 32.0);
    let start = flash_state(Flash::Explosion, 0.0).unwrap();
    assert!((start.diameter - 48.0).abs() < 1e-4);
    assert_eq!(start.fade, 1.0);
    let middle = flash_state(Flash::Explosion, 0.5).unwrap();
    assert!((middle.diameter - 40.0).abs() < 1e-4);
    assert!((middle.fade - 0.5).abs() < 1e-6);
    let late = flash_state(Flash::Explosion, 0.999).unwrap();
    assert!((late.diameter - 32.0).abs() < 0.02);
}

#[test]
fn a_laser_flash_is_a_red_blob_of_24_units_shrinking_to_16_in_a_quarter_second() {
    let look = Flash::Laser.look();
    assert_eq!(look.color, [1.0, 0.0, 0.0]);
    assert_eq!(look.lifetime, 0.25);
    let start = flash_state(Flash::Laser, 0.0).unwrap();
    assert!((start.diameter - 24.0).abs() < 1e-4);
    let half = flash_state(Flash::Laser, 0.125).unwrap();
    assert!((half.diameter - 20.0).abs() < 1e-4);
    assert!((half.fade - 0.5).abs() < 1e-6);
}

#[test]
fn a_flash_is_gone_when_its_life_is_over() {
    assert!(flash_state(Flash::Explosion, 1.0).is_none());
    assert!(flash_state(Flash::Explosion, 7.0).is_none());
    assert!(flash_state(Flash::Laser, 0.25).is_none());
    assert!(flash_state(Flash::Laser, 0.2499).is_some());
}

#[test]
fn a_flash_fades_in_its_colour_and_its_blend_both() {
    let boost = 1.0;
    let full = flash_color(Flash::Explosion, 1.0, boost);
    assert!((full[0] - gain(boost)).abs() < 1e-6);
    assert_eq!(full[2], 0.0, "yellow has no blue");
    // Half way it has a quarter of the light.
    let half = flash_color(Flash::Explosion, 0.5, boost);
    assert!((half[0] - full[0] / 4.0).abs() < 1e-6);
    // Red has red only.
    let red = flash_color(Flash::Laser, 1.0, boost);
    assert_eq!((red[1], red[2]), (0.0, 0.0));
    assert!(red[0] > 0.0);
    // Nothing is left at the end, and a lens turned off adds nothing.
    assert_eq!(flash_color(Flash::Laser, 0.0, boost), [0.0; 3]);
    assert_eq!(flash_color(Flash::Explosion, 1.0, 0.0), [0.0; 3]);
}

#[test]
fn a_blast_flashes_when_it_is_within_two_ranges_of_the_view() {
    assert_eq!(explosion_radius(5.0), 10.0);
    assert_eq!(explosion_radius(0.0), 0.0);
}

#[test]
fn the_flashes_are_few_and_the_laser_chance_is_one_in_four() {
    assert_eq!(MAX_FLASHES, 64);
    assert_eq!(CROSSING_CHANCE, 0.25);
}
