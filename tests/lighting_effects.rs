//! Allocation bounds and lamp-selection/fade behaviour, independent of rendering.
use tilerider::content::map::MapData;
use tilerider::light::bake::Lamp;
use tilerider::light::effects::{ShadowBlend, ShadowLights, lens_light};
use tilerider::light::raster::{Canvas, WHITE};
use tilerider::map::grid::MapGrid;

fn lamp(x: f32, y: f32, radius: f32) -> Lamp {
    Lamp {
        x,
        y,
        radius,
        color: [1.0, 0.5, 0.25],
        use_for_dynamic_shadow: true,
        additive_rate: 0.0,
    }
}
fn grid(rows: &str) -> MapGrid {
    let map = MapData::parse(&format!(r#"<XnaContent><Asset Type="GameLibrary.MapData"><Scale>32</Scale>
        <CharMapDictionary>#:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>{rows}</CharMap></Asset></XnaContent>"#)).unwrap();
    MapGrid::build(&map, None)
}
#[test]
fn dirt_light_resolution_is_bounded_and_handles_tiny_maps() {
    let flash = Canvas::new(1, 1, WHITE);
    for room in [[0.0, 0.0], [2.0, 1.0], [4000.0, 3000.0], [100000.0, 1.0]] {
        let image = lens_light(&[], room, 1.0, &flash);
        assert!((1..=512).contains(&image.width));
        assert!((1..=512).contains(&image.height));
    }
}
#[test]
fn shadow_lookup_respects_walls_and_lamp_flags() {
    let g = grid(".......\n...#...\n...#...");
    let mut l = lamp(1.5, 1.5, 10.0);
    let cache = ShadowLights::bake(&g, &[l]);
    assert_eq!(cache.at(2.5, 1.5), Some(0));
    assert_eq!(cache.at(5.5, 1.5), None);
    assert_eq!(cache.at(-0.1, 0.0), None);
    assert_eq!(cache.at(f32::NAN, 0.0), None);
    l.use_for_dynamic_shadow = false;
    assert_eq!(ShadowLights::bake(&g, &[l]).at(2.5, 1.5), None);
}
#[test]
fn overlapping_strong_lights_suppress_an_ambiguous_direction() {
    let g = grid(".......\n.......\n.......");
    let lamps = [lamp(1.5, 1.5, 6.0), lamp(5.5, 1.5, 6.0)];
    assert_eq!(ShadowLights::bake(&g, &lamps).at(3.5, 1.5), None);
}
#[test]
fn changing_and_reversing_lamps_is_continuous_and_fades_into_darkness() {
    let mut blend = ShadowBlend::default();
    assert!(!blend.update(Some(0), 0.5));
    assert_eq!(blend.weights(), &[(0, 1.0)]);
    assert!(blend.update(Some(1), 0.25));
    assert_eq!(blend.weights(), &[(0, 0.5), (1, 0.5)]);
    blend.update(Some(0), 0.1);
    assert!((blend.weights()[0].1 - 0.7).abs() < 1e-5);
    assert!((blend.weights()[1].1 - 0.3).abs() < 1e-5);
    assert!(!blend.update(None, 0.5));
    assert!(blend.weights().is_empty());
}
