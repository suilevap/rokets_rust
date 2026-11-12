//! Helpers shared by the integration tests: maps and PNGs from `assets/`.

#![allow(dead_code)]

use std::fs::File;
use std::path::{Path, PathBuf};

use tilerider::assets;
use tilerider::content::map::MapData;
use tilerider::light::raster::Canvas;
use tilerider::map::grid::MapGrid;

pub fn assets_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

/// A PNG as `(width, height, RGBA bytes)`.
pub fn read_png(path: &Path) -> (usize, usize, Vec<u8>) {
    let file = File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("png header");
    let mut buf = vec![0; reader.output_buffer_size().expect("png size")];
    let info = reader.next_frame(&mut buf).expect("png data");
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..w * h * 4].to_vec(),
        png::ColorType::Rgb => buf[..w * h * 3]
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf[..w * h * 2]
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buf[..w * h].iter().flat_map(|&g| [g, g, g, 255]).collect(),
        other => panic!("unsupported png colour type {other:?}"),
    };
    (w, h, rgba)
}

/// A map from `assets/` with its grid built from its image.
pub fn load_map(name: &str) -> (MapData, MapGrid) {
    let root = assets_root();
    let xml =
        std::fs::read_to_string(root.join(assets::map_path(name).expect("map"))).expect("read map");
    let map = MapData::parse(&xml).expect("parse map");
    let image = map
        .image
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .and_then(assets::sprite_path)
        .map(|p| read_png(&root.join(p)));
    let grid = MapGrid::build(&map, image.as_ref().map(|(w, h, p)| (*w, *h, p.as_slice())));
    (map, grid)
}

/// The lamp sprite the light is baked with.
pub fn flash() -> Canvas {
    let (w, h, data) = read_png(&assets_root().join("Sprites/Particles/flash2.png"));
    Canvas::from_rgba8(w, h, &data)
}

