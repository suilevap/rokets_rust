//! Bakes a map's light and writes it as PNGs, to compare with the C#
//! game's light targets pixel by pixel.
//!
//! `cargo run --example light_dump -- <map> <out dir>` writes
//! `final.png` (the light layer, C# `FinalLightTarget`) and `walls.png`
//! (the walls' light, C# `_lightMapTargetTmp`).

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use tilerider::assets;
use tilerider::content::map::MapData;
use tilerider::light::bake::LightMap;
use tilerider::light::raster::Canvas;
use tilerider::map::grid::MapGrid;
use tilerider::map::templates::static_shadows;

fn read_png(path: &Path) -> (usize, usize, Vec<u8>) {
    let mut decoder =
        png::Decoder::new(std::io::BufReader::new(File::open(path).expect("open png")));
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

fn sprite_size(root: &Path, name: &str) -> Option<[f32; 2]> {
    let path = root.join(assets::sprite_path(name)?);
    let decoder = png::Decoder::new(std::io::BufReader::new(File::open(path).ok()?));
    let info = decoder.read_info().ok()?.info().clone();
    Some([info.width as f32, info.height as f32])
}

fn write_png(path: &Path, (w, h, texels): (usize, usize, Vec<[f32; 3]>)) {
    let file = BufWriter::new(File::create(path).expect("create png"));
    let mut encoder = png::Encoder::new(file, w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let data: Vec<u8> = texels
        .iter()
        .flat_map(|c| c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8))
        .collect();
    encoder
        .write_header()
        .expect("png header")
        .write_image_data(&data)
        .expect("png data");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (name, out) = match &args[..] {
        [_, name, out] => (name.as_str(), Path::new(out)),
        _ => {
            eprintln!("usage: light_dump <map> <out dir>");
            std::process::exit(2);
        }
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let map_path = assets::map_path(name).unwrap_or_else(|| panic!("no map {name:?}"));
    let xml = std::fs::read_to_string(root.join(map_path)).expect("read map");
    let map = MapData::parse(&xml).expect("parse map");
    let image = map
        .image
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .and_then(assets::sprite_path)
        .map(|p| read_png(&root.join(p)));
    let grid = MapGrid::build(&map, image.as_ref().map(|(w, h, p)| (*w, *h, p.as_slice())));
    let (fw, fh, fdata) = read_png(&root.join("Sprites/Particles/flash2.png"));
    let flash = Canvas::from_rgba8(fw, fh, &fdata);
    let start = std::time::Instant::now();
    let light = LightMap::bake(
        &map,
        &grid,
        &flash,
        &static_shadows(&map, |name| sprite_size(&root, name)),
    );
    eprintln!(
        "{name}: baked in {:?}, frame {:?}",
        start.elapsed(),
        light.frame
    );
    std::fs::create_dir_all(out).expect("create out dir");
    write_png(&out.join("final.png"), light.layer_image());
    write_png(&out.join("walls.png"), light.walls_image());
}
