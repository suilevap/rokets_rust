use super::*;

#[test]
fn heights_come_in_steps() {
    // alpha 255 with threshold 96: 160 above → 6 steps → 193 px.
    assert_eq!(wall_height(255, 96), 193.0 / crate::units::PX_PER_UNIT);
    assert_eq!(wall_height(95, 96), 0.0);
    assert_eq!(wall_height(96, 96), 33.0 / crate::units::PX_PER_UNIT);
}
