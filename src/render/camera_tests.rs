use std::time::Duration;

use bevy::ecs::system::RunSystemOnce;

use super::*;

#[test]
fn short_side_keeps_the_landscape_view() {
    // Landscape: the vertical (short) side spans π/4.
    assert_eq!(fov_for_aspect(16.0 / 9.0), FRAC_PI_4);
    // Portrait: the horizontal (short) side spans π/4, so vertically more.
    let aspect = 9.0 / 16.0;
    let fov = fov_for_aspect(aspect);
    let horizontal = 2.0 * ((fov / 2.0).tan() * aspect).atan();
    assert!((horizontal - FRAC_PI_4).abs() < 1e-5);
    assert!(fov > FRAC_PI_4);
}

#[test]
fn portrait_turns_screen_up_into_map_right() {
    let mut view = ViewRotation::default();
    view.update_target(9.0 / 16.0);
    view.angle = view.target;
    let up = view.screen_to_world(Vec2::Y);
    assert!((up - Vec2::X).length() < 1e-5, "{up}");
    let right = view.screen_to_world(Vec2::X);
    assert!((right - Vec2::NEG_Y).length() < 1e-5, "{right}");
    // Nearly square: no switch back.
    view.update_target(1.0);
    assert_eq!(view.target, -FRAC_PI_2);
    view.update_target(16.0 / 9.0);
    assert_eq!(view.target, 0.0);
}

fn target(position: Vec2, velocity: Vec2) -> FocusTarget {
    FocusTarget {
        position,
        velocity,
        look: Vec2::ZERO,
        radius: 0.5,
        max_speed: 10.0,
    }
}

#[test]
fn aiming_leads_the_camera_five_units_and_zooms_out_less() {
    let view = ViewRotation::default();
    let projection = PerspectiveProjection::default();
    let t = FocusTarget {
        look: Vec2::new(0.0, 1.0),
        ..target(Vec2::new(4.0, 5.0), Vec2::X * 5.0)
    };
    let frame = framing([t], &view, &projection).unwrap();
    assert!((frame.focus - Vec2::new(4.0 + 3.5, 10.0)).length() < 1e-5, "{:?}", frame.focus);
    // 0.4 + 0.6 × (1 − 0.5) × 0.8.
    assert!((frame.scale - 0.64).abs() < 1e-5, "{}", frame.scale);
}

#[test]
fn solo_framing_leads_motion_and_clamps_speed_zoom() {
    let view = ViewRotation::default();
    let projection = PerspectiveProjection::default();
    for (speed, scale) in [(0.0, 1.0), (5.0, 0.7), (10.0, 0.4), (30.0, 0.4)] {
        let t = target(Vec2::new(4.0, 5.0), Vec2::X * speed);
        let frame = framing([t], &view, &projection).unwrap();
        assert!((frame.scale - scale).abs() < 1e-5);
        assert!(frame.focus.distance(t.position + t.velocity * 0.7) < 1e-5);
    }
    let mut fixed = target(Vec2::ONE, Vec2::ZERO);
    fixed.max_speed = 0.0;
    assert_eq!(framing([fixed], &view, &projection).unwrap().scale, 1.0);
}

#[test]
fn group_framing_fits_driving_margins_in_landscape_and_portrait() {
    let targets = [
        target(Vec2::new(-12.0, -4.0), Vec2::ZERO),
        target(Vec2::new(16.0, 5.0), Vec2::ZERO),
        target(Vec2::ZERO, Vec2::ZERO),
    ];
    let mut scales = Vec::new();
    for (aspect, angle) in [(5.0 / 3.0, 0.0), (3.0 / 5.0, -FRAC_PI_2)] {
        let view = ViewRotation {
            angle,
            target: angle,
        };
        let projection = PerspectiveProjection {
            fov: fov_for_aspect(aspect),
            aspect_ratio: aspect,
            ..default()
        };
        let frame = framing(targets, &view, &projection).unwrap();
        assert!(
            frame
                .focus
                .distance((targets[0].position + targets[1].position) / 3.0)
                < 1e-5
        );
        assert!(frame.scale < 1.0 && frame.scale > MIN_SCALE);
        scales.push(frame.scale);
        let transform = MainCamera {
            ground: frame.focus,
            focus: frame.focus,
            scale: frame.scale,
            destination: frame.focus,
        }
        .transform(view.up());
        let clip = bevy::math::proj::perspective_infinite_reverse(projection.fov, aspect, 1.0)
            * transform.to_matrix().inverse();
        for target in targets {
            for corner in [
                Vec2::new(-1.0, -1.0),
                Vec2::new(-1.0, 1.0),
                Vec2::new(1.0, -1.0),
                Vec2::ONE,
            ] {
                let p = clip
                    * (target.position + corner * target.radius * 6.0)
                        .extend(0.0)
                        .extend(1.0);
                assert!(
                    p.x.abs() <= p.w + 1e-4 && p.y.abs() <= p.w + 1e-4,
                    "outside frame: {p}"
                );
            }
        }
    }
    assert!((scales[0] - scales[1]).abs() < 1e-5);
}

#[test]
fn group_zoom_respects_the_furthest_camera_limit() {
    let frame = framing(
        [
            target(Vec2::ZERO, Vec2::ZERO),
            target(Vec2::splat(1000.0), Vec2::ZERO),
        ],
        &ViewRotation::default(),
        &PerspectiveProjection::default(),
    )
    .unwrap();
    assert_eq!(frame.scale, MIN_SCALE);
    assert!(
        framing(
            [],
            &ViewRotation::default(),
            &PerspectiveProjection::default()
        )
        .is_none()
    );
}

fn camera_world() -> (World, Entity, Entity) {
    let mut world = World::new();
    world.insert_resource(Time::<()>::default());
    world.insert_resource(ViewRotation::default());
    world.insert_resource(CameraShake::default());
    let camera = world
        .spawn((
            MainCamera::default(),
            Transform::default(),
            Projection::default(),
        ))
        .id();
    let target = world
        .spawn((
            CameraTarget {
                radius: 0.5,
                max_speed: 10.0,
            },
            Transform::from_xyz(4.0, 5.0, 0.0),
            LinearVelocity(Vec2::X * 10.0),
        ))
        .id();
    (world, camera, target)
}

fn tick(world: &mut World, dt: f32) {
    world
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(dt));
    world.run_system_once(follow_target).unwrap();
}

#[test]
fn speed_zoom_is_smooth_across_frame_rates_and_returns_when_stopped() {
    let mut scales = Vec::new();
    for (frames, dt) in [(10, 0.1), (100, 0.01)] {
        let (mut world, camera, target) = camera_world();
        for _ in 0..frames {
            tick(&mut world, dt);
        }
        let cam = world.get::<MainCamera>(camera).unwrap();
        assert!(cam.scale > MIN_SCALE && cam.scale < 1.0);
        assert!(cam.focus.x > 4.0, "the view should lead the moving target");
        assert!(world.get::<Transform>(camera).unwrap().translation.z > CAMERA_HEIGHT);
        scales.push(cam.scale);
        world.get_mut::<LinearVelocity>(target).unwrap().0 = Vec2::ZERO;
        for _ in 0..600 {
            tick(&mut world, 1.0 / 60.0);
        }
        let cam = world.get::<MainCamera>(camera).unwrap();
        assert!((cam.scale - 1.0).abs() < 1e-4);
        assert!(cam.focus.distance(Vec2::new(4.0, 5.0)) < 1e-4);
    }
    assert!((scales[0] - scales[1]).abs() < 1e-5);
}

#[test]
fn missing_or_despawned_targets_leave_a_finite_camera() {
    let (mut world, camera, target) = camera_world();
    tick(&mut world, 0.1);
    let before = *world.get::<Transform>(camera).unwrap();
    world.despawn(target);
    tick(&mut world, 1.0);
    let after = world.get::<Transform>(camera).unwrap();
    assert_eq!(after.translation, before.translation);
    assert_eq!(after.rotation, before.rotation);
}

fn insert_map(world: &mut World) {
    use crate::content::map::MapData;
    use crate::light::{bake::LightMap, raster::Mask};
    use crate::map::grid::MapGrid;
    use crate::units::CellSize;
    let data = MapData::parse(r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <CharMapDictionary>.:#100000A0</CharMapDictionary><CharMap>.</CharMap></Asset></XnaContent>"#).unwrap();
    let grid = MapGrid::build(&data, None);
    let light = LightMap::bake(&data, &grid, &Mask::from_alpha(1, 1, &[255; 4]), &[]);
    world.insert_resource(LoadedMap {
        data,
        grid,
        light,
        cell: CellSize::from_map_scale(64.0),
        cars: default(),
        list: default(),
        sprites: default(),
    });
}

#[test]
fn map_entry_resets_previous_focus_and_zoom_and_handles_fixed_targets() {
    let (mut world, camera, target) = camera_world();
    insert_map(&mut world);
    world.entity_mut(target).remove::<LinearVelocity>();
    *world.get_mut::<MainCamera>(camera).unwrap() = MainCamera {
        ground: Vec2::splat(100.0),
        focus: Vec2::splat(200.0),
        scale: MIN_SCALE,
        destination: Vec2::splat(300.0),
    };
    world.run_system_once(snap_to_target).unwrap();
    let cam = world.get::<MainCamera>(camera).unwrap();
    assert_eq!(cam.ground, Vec2::new(4.0, 5.0));
    assert_eq!(cam.focus, cam.ground);
    assert_eq!(cam.destination, cam.focus, "no lag at the start of a map");
    assert_eq!(cam.scale, 1.0);
    assert_eq!(
        world.get::<Transform>(camera).unwrap().translation.z,
        CAMERA_HEIGHT
    );
    world.despawn(target);
    world.run_system_once(snap_to_target).unwrap();
    let cam = world.get::<MainCamera>(camera).unwrap();
    assert_eq!(cam.ground, Vec2::new(1.0, -1.0));
    assert_eq!(cam.focus, cam.ground);
    assert_eq!(cam.scale, 1.0);
}

#[test]
fn a_jolt_shakes_the_view_then_dies_away() {
    let mut shake = CameraShake::default();
    assert_eq!(shake.step(1.0 / 60.0), Vec2::ZERO, "still until jolted");
    shake.add(0.5);
    let first: f32 = (0..5).map(|_| shake.step(1.0 / 60.0).length()).sum();
    assert!(first > 0.0 && first < 5.0 * 1.0, "{first}");
    for _ in 0..60 {
        shake.step(1.0 / 60.0);
    }
    assert_eq!(shake.step(1.0 / 60.0), Vec2::ZERO, "settled within a second");
}
