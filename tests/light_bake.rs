//! The baked light: lamps, wall shadows, the void, the walls' light.

mod common;

use tilerider::content::map::MapData;
use tilerider::light::bake::LightMap;
use tilerider::map::grid::MapGrid;

fn map(rows: &str) -> MapData {
    map_with_lamp(rows, 2.5, 2.5)
}

fn map_with_lamp(rows: &str, x: f32, y: f32) -> MapData {
    MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <AmbientLight>FF000000</AmbientLight>
        <Lights><Item><Position>{x} {y}</Position><Color>FFFFFFFF</Color><R>6</R></Item></Lights>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0;_:#00000000</CharMapDictionary>
        <CharMap>{rows}</CharMap></Asset></XnaContent>"#
    ))
    .unwrap()
}

#[test]
fn lamp_lights_and_walls_shadow() {
    let map = map("\n**********\n*........*\n*........*\n*......*.*\n*......*.*\n**********\n");
    let grid = MapGrid::build(&map, None);
    let light = LightMap::bake(&map, &grid, &common::flash());
    let near = light.sample_layer(2.5, 2.5)[0];
    let far = light.sample_layer(6.5, 2.5)[0];
    let behind_wall = light.sample_layer(8.5, 4.5)[0];
    assert!(near > 0.8, "near {near}");
    assert!(far < near && far > 0.05, "far {far}");
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
    let light = LightMap::bake(&map, &grid, &common::flash());
    assert!(light.sample_layer(2.5, 1.5)[0] < 0.5, "the room is dark");
    assert_eq!(light.sample_layer(7.5, 1.5), [1.0; 3], "the void");
    assert_eq!(light.sample_layer(30.0, 1.5), [1.0; 3], "past the border");
}

#[test]
fn layer_covers_the_map_and_four_cells_around_it() {
    let map = map("\n*****\n*...*\n*****\n");
    let grid = MapGrid::build(&map, None);
    let light = LightMap::bake(&map, &grid, &common::flash());
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
    let light = LightMap::bake(&map, &grid, &common::flash());
    assert_eq!(light.sample(-10.0, 1.0), [0.0; 3]);
    assert!(light.sample(2.5, 1.5)[0] > 0.0);
}
