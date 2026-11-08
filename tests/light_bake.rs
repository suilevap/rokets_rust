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
