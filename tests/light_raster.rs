//! The float images the light is baked on: plain sums, products, a blur.

use tilerider::light::raster::{Mask, Placement, Rect, Rgb};

#[test]
fn adding_does_not_saturate() {
    let mut c = Rgb::new(2, 2, [0.6; 3]);
    let src = Mask::new(1, 1, [1.0]);
    c.add_placed(
        &src,
        src.rect(),
        Placement::onto(src.rect(), c.rect()),
        [0.6, 0.3, 0.0],
    );
    let t = c.get(0, 0);
    assert!((t[0] - 1.2).abs() < 1e-6 && (t[1] - 0.9).abs() < 1e-6 && (t[2] - 0.6).abs() < 1e-6);
}

#[test]
fn shadows_multiply() {
    let mut m = Mask::new(4, 4, [0.8]);
    let p = [[0.0, 0.0], [4.0, 0.0], [0.0, 4.0]];
    m.shade(p, [0.5; 3]);
    m.shade(p, [0.5; 3]);
    assert!((m.get(0, 0)[0] - 0.8 * 0.25).abs() < 1e-6, "twice half");
    assert_eq!(m.get(3, 3)[0], 0.8, "outside the triangle");
}

#[test]
fn a_fading_shadow_follows_its_corners() {
    let mut m = Mask::new(8, 1, [1.0]);
    // Clear at the left corner, full at the right.
    m.shade([[0.0, -1.0], [8.0, -1.0], [8.0, 2.0]], [0.0, 1.0, 1.0]);
    let row: Vec<f32> = (0..8).map(|x| m.get(x, 0)[0]).collect();
    assert!(row.windows(2).all(|w| w[1] <= w[0]), "{row:?}");
    assert!(row[0] > 0.8 && row[7] < 0.2, "{row:?}");
}

#[test]
fn triangles_cover_texel_centres_either_way_round() {
    let mut a = Mask::new(4, 4, [1.0]);
    let mut b = Mask::new(4, 4, [1.0]);
    let p = [[0.0, 0.0], [4.0, 0.0], [0.0, 4.0]];
    a.shade(p, [1.0; 3]);
    b.shade([p[0], p[2], p[1]], [1.0; 3]);
    assert_eq!(a, b);
    assert_eq!(a.get(0, 0)[0], 0.0);
    assert_eq!(a.get(3, 3)[0], 1.0);
}

#[test]
fn blur_keeps_a_flat_image_flat() {
    let mut c = Rgb::new(64, 32, [0.5, 0.25, 1.0]);
    c.blur(4.0);
    for t in [c.get(10, 10), c.get(0, 0), c.get(63, 31)] {
        assert!(
            (t[0] - 0.5).abs() < 1e-5 && (t[1] - 0.25).abs() < 1e-5 && (t[2] - 1.0).abs() < 1e-5,
            "{t:?}"
        );
    }
}

#[test]
fn blur_spreads_a_point_evenly_and_keeps_its_total() {
    let mut m = Mask::new(65, 65, [0.0]);
    m.texels[32 * 65 + 32] = [1.0];
    m.blur(3.0);
    let total: f32 = m.texels.iter().map(|t| t[0]).sum();
    assert!((total - 1.0).abs() < 1e-4, "{total}");
    let at = |x, y| m.get(x, y)[0];
    assert!(at(32, 32) > at(35, 32) && at(35, 32) > at(40, 32), "falls off");
    assert!((at(30, 32) - at(34, 32)).abs() < 1e-7, "left and right");
    assert!((at(32, 30) - at(30, 32)).abs() < 1e-7, "along either axis");
}

#[test]
fn stretching_a_mask_samples_it_linearly() {
    let src = Mask::from_alpha(2, 1, &[0, 0, 0, 0, 255, 255, 255, 255]);
    let mut c = Mask::new(4, 1, [0.0]);
    c.add_placed(
        &src,
        src.rect(),
        Placement::onto(src.rect(), Rect::new(0.0, 0.0, 4.0, 1.0)),
        [1.0],
    );
    let row: Vec<f32> = (0..4).map(|x| c.get(x, 0)[0]).collect();
    assert_eq!(row, vec![0.0, 0.25, 0.75, 1.0]);
}

#[test]
fn a_mask_added_turned_around_its_origin_lands_where_it_is_turned_to() {
    let src = Mask::new(2, 2, [1.0]);
    let mut c = Mask::new(9, 9, [0.0]);
    // A square at (4, 4), turned a quarter turn about its own corner.
    c.add_placed(
        &src,
        src.rect(),
        Placement {
            position: [4.0, 4.0],
            origin: [0.0, 0.0],
            rotation: std::f32::consts::FRAC_PI_2,
            scale: [1.0, 1.0],
        },
        [1.0],
    );
    assert_eq!(c.get(3, 4)[0], 1.0, "left of the corner");
    assert_eq!(c.get(4, 4)[0], 0.0, "no longer right of it");
}

#[test]
fn downscaling_takes_the_mean_of_each_block() {
    let mut c = Rgb::new(4, 2, [0.0; 3]);
    c.texels[0] = [1.0; 3];
    c.texels[1] = [0.0; 3];
    c.texels[4] = [1.0, 0.0, 0.0];
    c.texels[5] = [0.0; 3];
    let small = c.downscale(2);
    assert_eq!((small.width, small.height), (2, 1));
    assert_eq!(small.get(0, 0), [0.5, 0.25, 0.25]);
    assert_eq!(small.get(1, 0), [0.0; 3]);
}

#[test]
fn resampling_takes_the_mean_of_the_area() {
    let mut m = Mask::new(8, 2, [0.0]);
    for y in 0..2 {
        for x in 0..4 {
            m.texels[y * 8 + x] = [1.0];
        }
    }
    let small = m.resample(m.rect(), 2, 1);
    assert!((small.get(0, 0)[0] - 1.0).abs() < 1e-5);
    assert!(small.get(1, 0)[0].abs() < 1e-5);
    // A part of the image only.
    let part = m.resample(Rect::new(2.0, 0.0, 4.0, 2.0), 1, 1);
    assert!((part.get(0, 0)[0] - 0.5).abs() < 0.05, "{:?}", part.get(0, 0));
}

#[test]
fn a_padded_blur_pulls_in_what_lies_beyond_the_edge() {
    let mut extended = Mask::new(33, 33, [0.0]);
    let mut padded = extended.clone();
    extended.blur(2.0);
    padded.blur_padded(2.0, Some([1.0]));
    assert!(extended.get(16, 16)[0].abs() < 1e-6, "the middle stays dark");
    assert!(padded.get(0, 16)[0] > 0.2, "{}", padded.get(0, 16)[0]);
    assert!(extended.get(0, 16)[0].abs() < 1e-6, "extended edge stays dark");
    assert!(padded.get(16, 16)[0] < 0.01, "far from the edge nothing changes");
}
