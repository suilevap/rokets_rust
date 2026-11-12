//! Light-responsive dirt and sprite shadows, independent of the renderer.
use tilerider::content::map::MapData;
use tilerider::light::bake::Lamp;
use tilerider::light::effects::{
    ShadowBlend, ShadowLights, ShadowPose, directional_shadow, lens_light,
};
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
fn dirt_light_is_local_and_includes_non_additive_lamps() {
    let flash = Canvas::new(4, 4, WHITE);
    let mut l = lamp(32.0, 32.0, 8.0);
    l.use_for_dynamic_shadow = false;
    let image = lens_light(&[l], [64.0, 64.0], 1.0, &flash);
    assert_eq!((image.width, image.height), (16, 16));
    assert!(image.get(8, 8)[0] > 0.9);
    assert_eq!(&image.get(0, 0)[..3], &[0.0; 3]);
    l.additive_rate = 1.0;
    l.use_for_dynamic_shadow = true;
    assert_eq!(lens_light(&[l], [64.0, 64.0], 1.0, &flash), image);
    let dark = lens_light(&[], [64.0, 64.0], 1.0, &flash);
    assert!(dark.texels.iter().all(|c| c[..3] == [0.0; 3]));
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
fn pose(at: [f32; 2]) -> ShadowPose {
    ShadowPose {
        position: at,
        size: [1.0, 1.0],
        angle: 0.0,
    }
}
#[test]
fn gradients_point_away_from_lights_and_split_only_when_close() {
    let l = lamp(0.0, 0.0, 100.0);
    let far = directional_shadow(pose([30.0, 0.0]), &l, 1.0, 100.0, 1.0);
    let near = directional_shadow(pose([5.0, 0.0]), &l, 1.0, 100.0, 1.0);
    assert_eq!(far.len(), 1);
    assert_eq!(near.len(), 2);
    for q in far.iter().chain(&near) {
        assert!(q.positions.iter().flatten().all(|v| v.is_finite()));
        let rear = (q.positions[0][0] + q.positions[3][0]) / 2.0;
        let front = (q.positions[1][0] + q.positions[2][0]) / 2.0;
        assert!(front > rear);
        assert!(q.alpha > 0.0 && q.alpha <= 1.0);
    }
    assert!(directional_shadow(pose([0.0, 0.0]), &l, 1.0, 100.0, 1.0).is_empty());
    assert!(directional_shadow(pose([0.1, 0.0]), &l, 1.0, 100.0, 1.0).is_empty());
    assert!(directional_shadow(pose([101.0, 0.0]), &l, 1.0, 100.0, 1.0).is_empty());
}
#[test]
fn body_projection_changes_width_and_the_wall_field_limits_the_far_end() {
    let l = lamp(0.0, 0.0, 100.0);
    let mut p = pose([10.0, 0.0]);
    p.size = [4.0, 1.0];
    let a = directional_shadow(p, &l, 1.0, 100.0, 1.0);
    p.angle = std::f32::consts::FRAC_PI_2;
    let b = directional_shadow(p, &l, 1.0, 100.0, 1.0);
    let width = |q: &tilerider::light::effects::ShadowQuad| {
        (q.positions[0][0] - q.positions[3][0]).hypot(q.positions[0][1] - q.positions[3][1])
    };
    assert!((width(&b[0]) / width(&a[0]) - 4.0).abs() < 1e-4);
    let quads = directional_shadow(p, &l, 1.0, 0.25, 1.0);
    for q in quads {
        let front = [
            (q.positions[1][0] + q.positions[2][0]) / 2.0,
            (q.positions[1][1] + q.positions[2][1]) / 2.0,
        ];
        assert!(((front[0] - p.position[0]).hypot(front[1] - p.position[1]) - 1.25).abs() < 1e-4);
    }
}

#[test]
fn map_weather_keeps_its_own_lens_policy() {
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
