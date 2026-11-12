//! The player's car: spawned from the map's `Players`, driven by
//! [`drive::drive_step`] on an Avian rigid body.
//!
//! Body (from `Entity`, `PhysicEntity`, `ControlableEntity`, `Car`): a box
//! the size of the car's `Base` sprite, mass from `Cars.xml`, friction 0.8,
//! bounce 0.5, linear damping `0.95 × map friction`, angular damping 9.
//! Drawn from its own sprites: the chassis (`Base`), two front wheels that
//! turn towards the stick, and the cabin (`Body`) on top.

pub mod drive;

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::content::cars::CarData;
use crate::controls::{LocalPlayer, PlayerIntent};
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::camera::CameraTarget;
use crate::render::layers::Layer;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::units::{map_degrees_to_world, px};
use drive::{Body, DriveParams, DriveState, drive_step};

/// `Entity.MaxAngularVelocityDefault`.
const MAX_ANGULAR_VELOCITY: f32 = 50.0;
/// Heights of the car's layers above the ground plane.
/// How fast the drawn wheels follow the stick (`Hardastic(0, 12)`).
const WHEEL_TURN_RATE: f32 = 12.0;

/// Debug: start the player's car at this point (map cells) instead of the
/// map's own start (`--start x,y`); used once, on the first map.
#[derive(Resource, Debug, Clone, Copy)]
pub struct StartOverride(pub Vec2);

#[derive(Component, Debug)]
pub struct Car {
    pub params: DriveParams,
    pub drive: DriveState,
}

/// A front wheel sprite; it turns with the stick.
#[derive(Component, Debug, Default)]
pub struct FrontWheel {
    /// Position on the car, world units.
    offset: Vec2,
    angle: f32,
}

pub struct CarPlugin;

impl Plugin for CarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_player_car)
            .add_systems(FixedUpdate, drive_cars.run_if(in_state(GameState::Playing)))
            .add_systems(Update, turn_front_wheels.run_if(in_state(GameState::Playing)));
    }
}

pub fn spawn_player_car(
    mut commands: Commands,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    start: Option<Res<StartOverride>>,
) {
    let players = &map.data.players.items;
    let Some(player) = players
        .iter()
        .find(|p| p.control == "Local" && !p.is_prototype)
        .or_else(|| players.iter().find(|p| !p.is_prototype))
    else {
        warn!("map {} has no player", map.data.name);
        return;
    };
    let Some(car) = map.cars.get(&player.car_name) else {
        warn!("car {:?} is not in Cars.xml", player.car_name);
        return;
    };
    let Some(base) = car.base_sprite().and_then(|s| map.sprites.get(&s).cloned()) else {
        warn!("car {} has no Base sprite", car.name);
        return;
    };
    let size = base.size * px(1.0) * car.scale;
    let radius = size.max_element() / 2.0;
    let friction = map.data.friction.unwrap_or(1.0);
    let at = start.map_or(Vec2::new(player.position.x, player.position.y), |s| s.0);
    // Only the first map: later maps start where they say.
    commands.remove_resource::<StartOverride>();
    let position = map.world(at.x, at.y);
    let angle = map_degrees_to_world(player.direction_deg);

    let params = DriveParams::new(car, radius, friction);
    let mut entity = commands.spawn((
        Name::new(format!("Player car {}", car.name)),
        MapEntity,
        LocalPlayer,
        PlayerIntent::default(),
        CameraTarget { radius, max_speed: params.max_speed },
        Car {
            params,
            drive: DriveState::default(),
        },
        Transform::from_translation(position.extend(0.0)).with_rotation(Quat::from_rotation_z(angle)),
        Visibility::default(),
    ));
    entity.insert((
        RigidBody::Dynamic,
        Collider::rectangle(size.x, size.y),
        LinearDamping(0.95 * friction),
        AngularDamping(9.0),
        Friction::new(0.8).with_combine_rule(CoefficientCombine::GeometricMean),
        Restitution::new(0.5).with_combine_rule(CoefficientCombine::Max),
        TransformInterpolation,
    ));
    if car.mass > 0.0 {
        entity.insert(Mass(car.mass));
    }
    entity.with_children(|parent| {
        spawn_car_sprites(parent, &map, car, &quad, &mut materials);
    });
}

fn spawn_car_sprites(
    parent: &mut ChildSpawnerCommands,
    map: &LoadedMap,
    car: &CarData,
    quad: &UnitQuad,
    materials: &mut Assets<StandardMaterial>,
) {
    let lit = SpriteTint {
        color: [1.0; 4],
        lit: true,
    };
    let sprite = |name: Option<String>| name.and_then(|n| map.sprites.get(&n).cloned());
    if let Some(info) = sprite(car.base_sprite()) {
        parent.spawn(sprite_bundle(&info, car.scale, Layer::CarBase, lit, quad, materials));
    }
    if car.load_wheel_body_color_views {
        if let (Some(info), Some(left)) = (sprite(car.part_sprite("Wheel")), car.left_wheel_position) {
            // Sprite pixels, y down → world, y up.
            let at = Vec2::new(left.x, -left.y) * px(1.0) * car.scale;
            for side in [at, Vec2::new(at.x, -at.y)] {
                parent.spawn((
                    sprite_bundle(&info, car.scale, Layer::CarWheel, lit, quad, materials),
                    FrontWheel { offset: side, angle: 0.0 },
                ));
            }
        }
        if let Some(info) = sprite(car.part_sprite("Body")) {
            parent.spawn(sprite_bundle(&info, car.scale, Layer::CarBody, lit, quad, materials));
        }
    }
}

pub fn drive_cars(mut cars: Query<(Forces, &mut Car, &PlayerIntent)>) {
    for (mut forces, mut car, intent) in &mut cars {
        let angle = forces.rotation().as_radians();
        let body = Body {
            angle,
            velocity: forces.linear_velocity(),
            angular_velocity: forces.angular_velocity(),
        };
        let params = car.params;
        let out = drive_step(&params, &mut car.drive, body, intent.move_dir);
        *forces.linear_velocity_mut() = out.velocity;
        *forces.angular_velocity_mut() = out.angular_velocity.clamp(-MAX_ANGULAR_VELOCITY, MAX_ANGULAR_VELOCITY);
        let position = forces.position();
        for f in out.forces {
            let world_point = position + Vec2::from_angle(angle).rotate(f.local_point);
            forces.apply_force_at_point(f.force, world_point);
        }
    }
}

fn turn_front_wheels(
    time: Res<Time>,
    cars: Query<(&Car, &Children)>,
    mut wheels: Query<(&mut FrontWheel, &mut Transform)>,
) {
    let k = (WHEEL_TURN_RATE * time.delta_secs()).min(1.0);
    for (car, children) in &cars {
        let target = car.drive.wheel_angle.clamp(-0.6, 0.6);
        for child in children.iter() {
            if let Ok((mut wheel, mut transform)) = wheels.get_mut(child) {
                wheel.angle += (target - wheel.angle) * k;
                transform.rotation = Quat::from_rotation_z(wheel.angle);
                transform.translation = wheel.offset.extend(Layer::CarWheel.z());
            }
        }
    }
}
