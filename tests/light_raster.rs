//! The render-target stand-in the light is drawn with.

use tilerider::light::raster::{BLACK, Blend, Canvas, Placement, Rect, WHITE};

#[test]
fn additive_blending_saturates() {
    let mut c = Canvas::new(2, 2, [0.6, 0.6, 0.6, 1.0]);
    let src = Canvas::new(1, 1, [0.6, 0.6, 0.6, 1.0]);
    c.draw(
        &src,
        src.rect(),
        Placement::onto(src.rect(), c.rect()),
        WHITE,
        Blend::Additive,
    );
    assert_eq!(c.get(0, 0), WHITE);
}

#[test]
fn alpha_blending_is_premultiplied() {
    let mut c = Canvas::new(1, 1, WHITE);
    let src = Canvas::from_rgba8(1, 1, &[0, 0, 0, 128]);
    c.draw(
        &src,
        src.rect(),
        Placement::onto(src.rect(), c.rect()),
        WHITE,
        Blend::Alpha,
    );
    let v = c.get(0, 0)[0];
    assert!((v - (1.0 - 128.0 / 255.0)).abs() < 1e-6, "{v}");
}

#[test]
fn triangles_cover_texel_centres_either_way_round() {
    let mut a = Canvas::new(4, 4, WHITE);
    let mut b = Canvas::new(4, 4, WHITE);
    let p = [[0.0, 0.0], [4.0, 0.0], [0.0, 4.0]];
    a.triangle(p, [BLACK; 3], Blend::Alpha);
    b.triangle([p[0], p[2], p[1]], [BLACK; 3], Blend::Alpha);
    assert_eq!(a, b);
    assert_eq!(a.get(0, 0), BLACK);
    assert_eq!(a.get(3, 3), WHITE);
}

#[test]
fn blur_keeps_a_flat_target_flat() {
    let mut c = Canvas::new(64, 32, [0.5, 0.5, 0.5, 1.0]);
    c.blur(WHITE);
    let v = c.get(10, 10)[0];
    assert!((v - 0.5).abs() < 0.01, "{v}");
}

#[test]
fn stretching_a_texture_samples_it_linearly() {
    let src = Canvas::from_rgba8(2, 1, &[0, 0, 0, 255, 255, 255, 255, 255]);
    let mut c = Canvas::new(4, 1, BLACK);
    c.draw(
        &src,
        src.rect(),
        Placement::onto(src.rect(), Rect::new(0.0, 0.0, 4.0, 1.0)),
        WHITE,
        Blend::Alpha,
    );
    let row: Vec<f32> = (0..4).map(|x| c.get(x, 0)[0]).collect();
    assert_eq!(row, vec![0.0, 0.25, 0.75, 1.0]);
}

#[test]
fn blur_with_the_original_keeps_a_flat_target_flat() {
    let mut c = Canvas::new(64, 32, [0.5, 0.5, 0.5, 1.0]);
    c.blur_with_original(BLACK);
    // Four copies at 63 / 255 each: just under the original.
    let v = c.get(10, 10)[0];
    assert!((v - 0.5 * 4.0 * 63.0 / 255.0).abs() < 0.01, "{v}");
}
