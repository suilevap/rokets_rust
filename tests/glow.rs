use tilerider::render::glow::alpha_for;

#[test]
fn a_tints_strength_is_what_the_squared_alpha_adds() {
    for strength in [0.0, 0.1, 0.5, 0.75, 1.0] {
        let alpha = alpha_for(strength);
        assert!((alpha * alpha - strength).abs() < 1e-6, "{strength}");
    }
}

#[test]
fn a_negative_strength_adds_nothing() {
    assert_eq!(alpha_for(-0.5), 0.0);
}
