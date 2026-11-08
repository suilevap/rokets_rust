//! The draw order table is ordered and above the floor.

use tilerider::render::layers::{FLOOR_TOP, Layer};

#[test]
fn layers_are_ordered_and_above_the_floor() {
    for pair in Layer::ALL.windows(2) {
        let (lower, upper) = (pair[0], pair[1]);
        assert!(lower < upper);
        assert!(lower.z() < upper.z(), "{lower:?} must sit below {upper:?}");
        assert!(lower.depth_bias() < upper.depth_bias());
    }
    assert!(Layer::ALL.iter().all(|l| l.z() > FLOOR_TOP));
    // Order never lifts a sprite towards the camera.
    assert!(Layer::ALL.iter().all(|l| l.z() < 0.02));
}
