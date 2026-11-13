//! One map XML with every section the game reads.

use tilerider::content::map::{EntityType, MapData};
use tilerider::content::xna::XColor;

const SAMPLE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<XnaContent>
  <Asset Type="GameLibrary.MapData">
    <Name>Sample</Name>
    <Image>Maps\Singleplayer\Sample</Image>
    <Scale>64</Scale>
    <ImageWallTreshold>96</ImageWallTreshold>
    <AmbientLight>0C0C0C0C</AmbientLight>
    <OutlandColorLT>FF00FFFF</OutlandColorLT>
    <OutlandColorLB>FF7FFFD4</OutlandColorLB>
    <Lights>
      <Item><Position>8.5 3.5</Position><Color>FFFFFF00</Color><R>12</R></Item>
    </Lights>
    <Players>
      <Item><Id>4001</Id><Name>Player1</Name><Position>9.5 3.5</Position>
        <DirectionDeg>90</DirectionDeg><Control>Local</Control><CarName>SandBuggy</CarName></Item>
    </Players>
    <Entities>
      <Item><Id>1001</Id><Position>5.5 3.5</Position><Sprite>Entities\Coin</Sprite>
        <Animated>false</Animated><Scale>1</Scale><Type>Coin</Type>
        <ActionOnCreate Null="true" /></Item>
    </Entities>
    <Triggers>
      <Item><Id>3</Id><Area>32 17 3 2</Area><Amount>999</Amount><Active>true</Active>
        <Actions><Item>NextMap()</Item></Actions><Subscribers>4001 1004</Subscribers></Item>
    </Triggers>
    <Actions>
      <Item Type="GameLibrary.ShowSuccess"><Name>ShowSuccess</Name><RunOnMapStart>false</RunOnMapStart></Item>
    </Actions>
  </Asset>
</XnaContent>"#;

#[test]
fn parses_sample() {
    let map = MapData::parse(SAMPLE).unwrap();
    assert_eq!(map.name, "Sample");
    assert_eq!(map.scale, 64.0);
    assert_eq!(map.ambient_light, XColor::rgba(12, 12, 12, 12));
    // Given colours are read; missing ones take the C# defaults.
    assert_eq!(map.outland_color_l_t, XColor::rgba(0, 255, 255, 255));
    assert_eq!(map.outland_color_r_t, XColor::rgba(0, 191, 255, 255));
    assert_eq!(map.outland_color_r_b, XColor::rgba(255, 165, 0, 255));
    assert_eq!(map.outland_color_l_b, XColor::rgba(127, 255, 212, 255));
    assert_eq!(map.lights.items[0].color, XColor::rgba(255, 255, 0, 255));
    assert_eq!(map.players.items[0].direction_deg, 90.0);
    assert_eq!(map.entities.items[0].kind, EntityType::Coin);
    let trigger = &map.triggers.items[0];
    assert_eq!(trigger.actions.items, vec!["NextMap()".to_string()]);
    assert_eq!(trigger.subscribers, vec![4001, 1004]);
    assert_eq!(map.actions.items[0].short_kind(), "ShowSuccess");
}

#[test]
fn entity_types_parse_and_unknown_ones_are_kept_apart() {
    let xml = |kind: &str| {
        format!(
            r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Entities>
            <Item><Id>1</Id><Position>1 1</Position>{kind}</Item></Entities></Asset></XnaContent>"#
        )
    };
    let kind = |inner: &str| MapData::parse(&xml(inner)).unwrap().entities.items[0].kind;
    assert_eq!(kind("<Type>HeavyBox</Type>"), EntityType::HeavyBox);
    assert_eq!(kind("<Type>Accelerator</Type>"), EntityType::Accelerator);
    assert_eq!(kind(""), EntityType::Box, "Box when absent, as in C#");
    assert_eq!(kind("<Type>Spaceship</Type>"), EntityType::Unknown);
}

#[test]
fn weather_styles_parse_with_default_and_unknown_values() {
    use tilerider::content::map::LevelEffect;
    for (tag, expected) in [
        ("", LevelEffect::None),
        ("<LevelEffect>None</LevelEffect>", LevelEffect::None),
        ("<LevelEffect>Snow</LevelEffect>", LevelEffect::Snow),
        ("<LevelEffect>SnowWind</LevelEffect>", LevelEffect::SnowWind),
        (
            "<LevelEffect>FutureWeather</LevelEffect>",
            LevelEffect::Other,
        ),
    ] {
        let data = MapData::parse(&format!(
            r#"<XnaContent><Asset Type="GameLibrary.MapData">{tag}</Asset></XnaContent>"#
        ))
        .unwrap();
        assert_eq!(data.level_effect, expected);
    }
}
