//! The baked light map and the light layer over it.

use tilerider::content::map::MapData;
use tilerider::light::bake::LightMap;
use tilerider::map::grid::MapGrid;

fn map(rows: &str) -> MapData {
    MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <AmbientLight>FF000000</AmbientLight>
        <Lights><Item><Position>2.5 2.5</Position><Color>FFFFFFFF</Color><R>6</R></Item></Lights>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0;_:#00000000</CharMapDictionary>
        <CharMap>{rows}</CharMap></Asset></XnaContent>"#
    ))
    .unwrap()
}

#[test]
fn lamp_lights_and_walls_shadow() {
    let map = map("\n**********\n*........*\n*........*\n*......*.*\n*......*.*\n**********\n");
    let grid = MapGrid::build(&map, None);
    let light = LightMap::bake(&map, &grid);
    let near = light.sample(2.5, 2.5)[0];
    let far = light.sample(6.5, 2.5)[0];
    let behind_wall = light.sample(8.5, 4.5)[0];
    assert!(near > 0.8, "near {near}");
    assert!(far < near && far > 0.0, "far {far}");
    assert!(behind_wall < 0.1, "shadow {behind_wall}");
}

#[test]
fn light_layer_is_full_over_the_void() {
    // A dark room (the lamp is far away behind walls) with void to the right.
    let map = map("\n*****____\n*...*____\n*...*____\n*****____\n");
    let grid = MapGrid::build(&map, None);
    let light = LightMap::bake(&map, &grid);
    let room = light.sample_layer(2.5, 1.5);
    let void = light.sample_layer(7.5, 1.5);
    let past_edge = light.sample_layer(20.0, 1.5);
    assert_eq!(room, light.sample(2.5, 1.5), "the room keeps its light");
    assert_eq!(void, [1.0; 3]);
    assert_eq!(past_edge, [1.0; 3]);
    // Walls keep the plain light map.
    assert!(light.sample(7.5, 1.5)[0] < 1.0);
}
