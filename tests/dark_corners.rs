//! The vignette: the `DarkCorners` sprite as the picture's darkening.

mod common;

use tilerider::render::dark_corners::vignette_pixels;

fn alpha(pixels: &[u8], width: usize, x: usize, y: usize) -> u8 {
    pixels[(y * width + x) * 4 + 3]
}

#[test]
fn the_whole_sprite_is_kept_and_black() {
    let (w, h) = (6, 5);
    let rgba: Vec<u8> = (0..w * h).flat_map(|i| [200, 100, 50, (i * 7 % 256) as u8]).collect();
    let (ow, oh, out) = vignette_pixels(&rgba, w, h);
    assert_eq!((ow, oh), (6, 5), "C#'s one-pixel crop is not carried over");
    assert_eq!(out.len(), ow * oh * 4);
    assert!(out.chunks(4).all(|p| p[..3] == [0, 0, 0]), "tinted black");
}

#[test]
fn clear_stays_clear_and_solid_stays_solid() {
    let rgba = [0, 0, 0, 0].repeat(9);
    let (_, _, out) = vignette_pixels(&rgba, 3, 3);
    assert_eq!(out, [0, 0, 0, 0].repeat(9));
    let rgba = [9, 9, 9, 255].repeat(9);
    let (_, _, out) = vignette_pixels(&rgba, 3, 3);
    assert_eq!(out, [0, 0, 0, 255].repeat(9));
}

#[test]
fn it_darkens_the_srgb_numbers_by_one_minus_the_alpha() {
    // The shader blends in linear light; what shows is the sRGB number of
    // light × (1 − alpha'), and it must come out the sRGB number × (1 − alpha).
    for a in [10u8, 40, 100, 163, 200] {
        let mut rgba = [0, 0, 0, 0].repeat(9);
        rgba[(4 * 4) + 3] = a; // the centre pixel
        let (_, _, out) = vignette_pixels(&rgba, 3, 3);
        let linear_keep = 1.0 - out[4 * 4 + 3] as f32 / 255.0;
        let shown = linear_keep.powf(1.0 / 2.2);
        let wanted = 1.0 - a as f32 / 255.0;
        assert!((shown - wanted).abs() < 0.01, "alpha {a}: shows {shown}, wanted {wanted}");
    }
}

#[test]
fn a_stronger_sprite_alpha_is_a_stronger_vignette() {
    let alphas: Vec<u8> = (0..=255u8).step_by(5).collect();
    let rgba: Vec<u8> = alphas.iter().flat_map(|&a| [0, 0, 0, a]).collect();
    let width = rgba.len() / 4;
    // A three-high image: only the middle row has any alpha.
    let tall = [vec![0; width * 4], rgba, vec![0; width * 4]].concat();
    let (w, _, out) = vignette_pixels(&tall, width, 3);
    let row: Vec<u8> = (0..w).map(|x| alpha(&out, w, x, 1)).collect();
    assert!(row.windows(2).all(|p| p[0] <= p[1]));
}

#[test]
fn the_real_sprite_is_clear_in_the_middle_and_dark_at_the_corners() {
    let (w, h, rgba) = common::read_png(&common::assets_root().join("Sprites/DarkCorners.png"));
    let (ow, oh, out) = vignette_pixels(&rgba, w, h);
    assert_eq!((ow, oh), (w, h));
    let at = |x: usize, y: usize| alpha(&out, ow, x, y);
    assert_eq!(at(ow / 2, oh / 2), 0, "the middle shows the picture as it is");
    assert!(at(0, oh / 2) > 0 && at(ow / 2, 0) > 0, "the outer rows darken too");
    let corner = at(0, 0).max(at(ow - 1, 0)).max(at(0, oh - 1)).max(at(ow - 1, oh - 1));
    assert!(corner > 100, "a corner is dark, not {corner}");
    assert!(corner < 255, "but never black");
}
