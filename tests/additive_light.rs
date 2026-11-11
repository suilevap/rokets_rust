//! How strongly the additive light shows: the eye adapting to the dark.

use tilerider::render::additive_light::target_rate;

#[test]
fn the_glow_shows_in_the_dark_and_fades_in_additive_light() {
    assert_eq!(target_rate(0.0), 1.0, "full in the dark");
    assert!(
        (target_rate(0.1) - 0.5).abs() < 1e-6,
        "half at half the threshold"
    );
    assert_eq!(target_rate(0.2), 0.0, "gone from 0.2 up");
    assert_eq!(target_rate(0.9), 0.0);
}

#[test]
fn the_glow_errs_on_the_bright_side_by_default() {
    use tilerider::render::additive_light::{DEFAULT_GLOW_BOOST, GlowBoost};
    // Twice the linear light fits XNA's sRGB sums on average; the default
    // goes past it, so the brightest parts burn out rather than fall short.
    assert_eq!(GlowBoost::default().0, DEFAULT_GLOW_BOOST);
    assert!(DEFAULT_GLOW_BOOST > 2.0);
}
