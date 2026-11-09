//! The baked light against the C# game's own light targets
//! (`tests/data/light_cs`).

mod common;

use std::path::Path;

use tilerider::light::bake::LightMap;

/// Mean and largest per-channel difference (0..255), and the share of
/// texels off by more than 16.
fn compare(name: &str, file: &str, (w, h, ours): (usize, usize, Vec<[f32; 3]>)) -> (f32, u8, f32) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/light_cs")
        .join(name)
        .join(file);
    let (cw, ch, theirs) = common::read_png(&path);
    assert_eq!((w, h), (cw, ch), "{name} {file}: size");
    let (mut sum, mut max, mut off) = (0.0, 0u8, 0usize);
    for (i, c) in ours.iter().enumerate() {
        let mut worst = 0u8;
        for k in 0..3 {
            let a = (c[k].clamp(0.0, 1.0) * 255.0).round() as i32;
            let d = (a - theirs[i * 4 + k] as i32).unsigned_abs() as u8;
            sum += d as f32;
            worst = worst.max(d);
        }
        max = max.max(worst);
        if worst > 16 {
            off += 1;
        }
    }
    let n = ours.len() as f32;
    (sum / (n * 3.0), max, off as f32 / n)
}

#[test]
fn light_matches_the_csharp_game() {
    let flash = common::flash();
    for name in ["EditorTest", "LightTest", "LetItSnowDay", "Middleware"] {
        let (map, grid) = common::load_map(name);
        let light = LightMap::bake(&map, &grid, &flash);
        for (file, image) in [
            ("final.png", light.layer_image()),
            ("walls.png", light.walls_image()),
        ] {
            let (mean, max, off) = compare(name, file, image);
            eprintln!(
                "{name} {file}: mean {mean:.2}, max {max}, off by >16: {:.2}%",
                off * 100.0
            );
            // Rounding, and hairlines along slanted shadow edges.
            assert!(mean < 1.0, "{name} {file}: mean difference {mean}");
            assert!(
                off < 0.005,
                "{name} {file}: {:.2}% of texels differ",
                off * 100.0
            );
        }
    }
}
