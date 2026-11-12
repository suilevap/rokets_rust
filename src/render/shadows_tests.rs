use super::*;
use crate::content::map::MapData;
use crate::light::{bake::LightMap, raster::Canvas};
use crate::map::grid::MapGrid;
use crate::render::sprite::{SpriteInfo, SpriteSet};
use crate::units::CellSize;
use bevy::ecs::system::RunSystemOnce;
use bevy::mesh::VertexAttributeValues;
use std::time::Duration;

fn setup() -> (World, Entity) {
    let mut world = World::new();
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_secs_f32(0.5));
    world.insert_resource(time);
    world.insert_resource(LightingEffects::default());
    world.insert_resource(Assets::<Mesh>::default());
    world.insert_resource(Assets::<StandardMaterial>::default());
    let data = MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Scale>32</Scale>
        <CharMapDictionary>.:#100000A0</CharMapDictionary><CharMap>........
........
........
........
........
........
........
........</CharMap><Lights><Item><Position>2.5 2.5</Position><Color>FFFFFFFF</Color><R>8</R>
        <UseForDynamicShadow>true</UseForDynamicShadow></Item></Lights></Asset></XnaContent>"#,
    )
    .unwrap();
    let grid = MapGrid::build(&data, None);
    let light = LightMap::bake(&data, &grid, &Canvas::from_rgba8(1, 1, &[255; 4]), &[]);
    let mut sprites = SpriteSet::default();
    for name in ["Gradient4Pixel", "Gradient4PixelUniform"] {
        sprites.insert(
            name,
            SpriteInfo {
                image: default(),
                size: Vec2::splat(4.0),
            },
        );
    }
    world.insert_resource(LoadedMap {
        data,
        grid,
        light,
        cell: CellSize::from_map_scale(32.0),
        cars: default(),
        list: default(),
        sprites,
    });
    let caster = world
        .spawn((
            ShadowCaster { size: Vec2::ONE },
            GlobalTransform::from_translation(Vec3::new(5.5, -2.5, 0.0)),
        ))
        .id();
    world.run_system_once(prepare_shadows).unwrap();
    (world, caster)
}
fn alpha(world: &World, handle: &Handle<Mesh>) -> f32 {
    let mesh = world.resource::<Assets<Mesh>>().get(handle).unwrap();
    let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("colors");
    };
    colors.iter().map(|c| c[3]).fold(0.0, f32::max)
}
fn width(world: &World, handle: &Handle<Mesh>) -> f32 {
    let mesh = world.resource::<Assets<Mesh>>().get(handle).unwrap();
    let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        panic!("positions");
    };
    p.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max)
        - p.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min)
}
#[test]
fn removing_a_caster_clears_both_batched_shadows() {
    let (mut world, caster) = setup();
    let system = world.register_system(update_shadows);
    world.run_system(system).unwrap();
    let directional = world.resource::<Shadows>().directional.clone();
    let ambient = world.resource::<Shadows>().ambient.clone();
    assert!(alpha(&world, &directional) > 0.0);
    assert!(alpha(&world, &ambient) > 0.0);
    world.despawn(caster);
    world.run_system(system).unwrap();
    assert_eq!(alpha(&world, &directional), 0.0);
    assert_eq!(alpha(&world, &ambient), 0.0);
}
#[test]
fn changing_caster_size_refreshes_a_stationary_batch() {
    let (mut world, caster) = setup();
    let system = world.register_system(update_shadows);
    world.run_system(system).unwrap();
    let ambient = world.resource::<Shadows>().ambient.clone();
    let before = width(&world, &ambient);
    world.get_mut::<ShadowCaster>(caster).unwrap().size.x = 2.0;
    world.run_system(system).unwrap();
    assert!((width(&world, &ambient) - before * 2.0).abs() < 1e-5);
}
