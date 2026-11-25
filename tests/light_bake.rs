//! The baked light: lamps, wall shadows, the void, the walls' light, the
//! additive light.

mod common;

use tilerider::content::map::MapData;
use tilerider::light::bake::{LightMap, tone};
use tilerider::map::grid::MapGrid;

fn map(rows: &str) -> MapData {
    map_with_lamp(rows, 2.5, 2.5)
}

fn map_with_lamp(rows: &str, x: f32, y: f32) -> MapData {
    lamp_map(rows, x, y, 0.0)
}

fn lamp_map(rows: &str, x: f32, y: f32, additive_rate: f32) -> MapData {
    MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <AmbientLight>FF000000</AmbientLight>
        <Lights><Item><Position>{x} {y}</Position><Color>FFFFFFFF</Color><R>6</R>
        <AdditiveRate>{additive_rate}</AdditiveRate></Item></Lights>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0;_:#00000000</CharMapDictionary>
        <CharMap>{rows}</CharMap></Asset></XnaContent>"#
    ))
    .unwrap()
}

#[test]
fn lamp_lights_and_walls_shadow() {
    let map = map("\n**********\n*........*\n*........*\n*......*.*\n*......*.*\n**********\n");
    let grid = MapGrid::build(&map, None);
    let light = LightMap::bake(&map, &grid, &common::flash(), &[]);
    let near = light.sample_layer(2.5, 2.5)[0];
    let far = light.sample_layer(6.5, 2.5)[0];
    let behind_wall = light.sample_layer(8.5, 4.5)[0];
    assert!(near > 0.8, "near {near}");
    // The layer holds linear multipliers: lit well away from the lamp is
    // still a few percent, not a tenth.
    assert!(far < near && far > 0.02, "far {far}");
    assert!(behind_wall < far / 2.0, "shadow {behind_wall}, far {far}");
}

#[test]
fn light_layer_is_full_over_the_void_and_past_the_border() {
    // A dark room (its lamp lies far off the map) with void to the right.
    let map = map_with_lamp(
        "\n*****____\n*...*____\n*...*____\n*****____\n",
        -40.0,
        -40.0,
    );
    let grid = MapGrid::build(&map, None);
    let light = LightMap::bake(&map, &grid, &common::flash(), &[]);
    assert!(light.sample_layer(2.5, 1.5)[0] < 0.5, "the room is dark");
    assert_eq!(light.sample_layer(7.5, 1.5), [1.0; 3], "the void");
    assert_eq!(light.sample_layer(30.0, 1.5), [1.0; 3], "past the border");
}

#[test]
fn layer_covers_the_map_and_four_cells_around_it() {
    let map = map("\n*****\n*...*\n*****\n");
    let grid = MapGrid::build(&map, None);
    let light = LightMap::bake(&map, &grid, &common::flash(), &[]);
    // Scale 64 px: two world units a cell; the border is 8 units.
    assert_eq!(light.layer_rect(), [-4.0, -4.0, 5.0 + 8.0, 3.0 + 8.0]);
    let (w, h, _) = light.layer_image();
    assert_eq!(w, 512, "the long side");
    assert_eq!(h, (512.0 * 11.0 / 13.0) as usize);
}

#[test]
fn walls_light_is_black_off_its_image() {
    let map = map("\n*****\n*...*\n*****\n");
    let grid = MapGrid::build(&map, None);
    let light = LightMap::bake(&map, &grid, &common::flash(), &[]);
    assert_eq!(light.sample(-10.0, 1.0), [0.0; 3]);
    assert!(light.sample(2.5, 1.5)[0] > 0.0);
}

#[test]
fn only_additive_lamps_make_additive_light() {
    let rows = "\n************\n*..........*\n*..........*\n*..........*\n************\n";
    let plain = lamp_map(rows, 2.5, 2.5, 0.0);
    let grid = MapGrid::build(&plain, None);
    assert!(LightMap::bake(&plain, &grid, &common::flash(), &[]).additive.is_none());

    let glowing = lamp_map(rows, 2.5, 2.5, 1.0);
    let light = LightMap::bake(&glowing, &grid, &common::flash(), &[]);
    let additive = light.additive.as_ref().expect("an additive lamp");
    // Its light reaches the spot under it; past its radius there is none.
    assert!(additive.brightness_at(2.5, 2.5) > 0.2, "under the lamp");
    assert_eq!(additive.brightness_at(30.0, 2.5), 0.0, "off the map");
    // The glow lies over the same rectangle as the light layer, lit
    // somewhere and dark somewhere else.
    let (w, h, texels) = additive.image();
    let (lw, lh, _) = light.layer_image();
    assert_eq!((w, h), (lw, lh));
    let brightest = texels.iter().map(|c| c[0]).fold(0.0f32, f32::max);
    let darkest = texels.iter().map(|c| c[0]).fold(1.0f32, f32::min);
    assert!(brightest > 0.3 && darkest == 0.0, "{brightest} {darkest}");
}

#[test]
fn the_tone_curve_is_clamped_monotone_and_darkens_the_middle() {
    assert_eq!(tone(0.0), 0.0);
    assert_eq!(tone(1.0), 1.0);
    assert_eq!(tone(3.0), 1.0, "overbright light is full light, no more");
    assert_eq!(tone(-1.0), 0.0);
    assert!(tone(0.5) < 0.5, "a sum of 0.5 lets less than half through");
    let steps: Vec<f32> = (0..=20).map(|i| tone(i as f32 / 20.0)).collect();
    assert!(steps.windows(2).all(|w| w[0] < w[1]), "{steps:?}");
}

#[test]
fn lamps_add_and_only_the_sum_is_clamped() {
    // The same lamp twice gives more light than once where one is not
    // yet full, and no more than full where it is.
    let rows = "\n**********\n*........*\n*........*\n*........*\n**********\n";
    let one = map(rows);
    let two = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <AmbientLight>FF000000</AmbientLight>
        <Lights><Item><Position>2.5 2.5</Position><Color>FFFFFFFF</Color><R>6</R></Item>
        <Item><Position>2.5 2.5</Position><Color>FFFFFFFF</Color><R>6</R></Item></Lights>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0;_:#00000000</CharMapDictionary>
        <CharMap>{rows}</CharMap></Asset></XnaContent>"#
    ))
    .unwrap();
    let grid = MapGrid::build(&one, None);
    let a = LightMap::bake(&one, &grid, &common::flash(), &[]);
    let b = LightMap::bake(&two, &grid, &common::flash(), &[]);
    let (mid_a, mid_b) = (a.sample_layer(5.5, 2.5)[0], b.sample_layer(5.5, 2.5)[0]);
    assert!(mid_b > mid_a * 1.5, "{mid_a} -> {mid_b}");
    assert!(b.sample_layer(2.5, 2.5)[0] <= 1.0);
}
