//! Cars: the map's `Players` (the player's and the AI's) and its
//! `SimpleCars`, by one function ([`spawn_car`]).
//!
//! Every car (`ControlableEntity`, 2010–2016) has a body on an Avian
//! rigid body (a box the size of the car's `Base` sprite, mass from
//! `Cars.xml`, friction 0.8, bounce 0.5, linear damping `0.95 × map
//! friction`), that sprite, [`Health`] (and a [`shield::Shield`]), a
//! [`control::Team`], a [`PlayerIntent`] (the one surface the player's
//! devices and, later, the AI write), a shadow, the weapon of its
//! `Cars.xml` entry or the map's override ([`weapon_type`]) and a wreck
//! sprite for when it is destroyed.
//!
//! - **The player's car** ([`Role::Player`], `Car`): angular damping 9,
//!   driven by [`drive::drive_step`]; two front wheels that turn towards
//!   the stick and the cabin (`Body`) on top, swaying on springs
//!   (`cabin`); smoke (`smoke`), sparks when it hits something (`impact`),
//!   engine and tyre sounds; the camera, the devices, the energy store and
//!   the magnet. It is the one with `LocalPlayer`.
//! - **The other `Players` cars** ([`Role::AiCar`]): every one of the
//!   map's `Players` that is not a prototype and not the player's
//!   (`MapManager` makes each with `CreatePlayer`: `SendRequestForNewPlayer`
//!   with the control `AiControlFactory` makes of its `Control`,
//!   `EntitiesManager.CreateCar`), and their clones. A `Car` as the
//!   player's is (body, driving, wheels, cabin, smoke, sparks, engine and
//!   tyre sounds, the magnet), driven by its control ([`crate::ai`] writes
//!   its intent, which [`drive_cars`] reads as it reads the player's).
//!   Unlike the player's: no `LocalPlayer` (the devices do not drive it),
//!   the gun's preset for others and no energy store (the control is set
//!   after the gun, and an AI's or `AINone` is no reason for
//!   `SetLocalControlPreferences`), quieter engine and tyres, a health bar
//!   when it is hurt, a static body when it cannot move (`IsStatic`: the
//!   `TurretLoaderStat` loaders) and its collider scaled as a simple car's
//!   (`CreateCar` passes `GeometryScale`, 1 on every `Players` car), the
//!   magnet may grab it when its car `CanLink`. With a control held for the
//!   player's devices (`NoneLocal`: `Control.LocalDevice`) the camera
//!   frames it too (`Camera2DOperatoAllInCadr.PlayerListUdpated` takes
//!   every player's car with a local device) and it shows no health bar.
//!   It dies as `GameModeSinglePlayer` has an AI `Car` die
//!   (`SheduleDestroy`, `CreateDestroyedEntity`; [`death`]): it never comes
//!   back. With a control that is not live yet it stands, as a simple car
//!   does.
//! - **The player's simple car** ([`Role::LocalSimple`]): a `SimpleCars`
//!   car with the `Local` control (`Swarm`, `DoorsDream1`, `TheLastStand`):
//!   a simple car in all but that the devices drive it, the camera follows
//!   it, its gun has the player's preset and energy store
//!   (`SetLocalControlPreferences`, `WeaponFactory`'s `LocalDevice`
//!   options), and it is the one with `LocalPlayer`. It is the player when
//!   no `Players` car is `Local` (C# made both, and the camera followed
//!   whichever had a local device; no map has both).
//! - **A simple car** ([`Role::Simple`], `SimpleCars`): its `Base` sprite
//!   only, angular damping 3, driven by [`simple`] (when it can move), no
//!   smoke, impact sparks or engine (`Car` alone had them); a collider
//!   scaled by `GeometryScale` or a circle; a static body when it cannot
//!   move (turrets, static targets; it turns dynamic when destroyed); a
//!   health bar when it is hurt ([`health_bar`]); gone after its wreck lay
//!   for two seconds ([`death`]). Its intent is written by its control
//!   ([`crate::ai`]: turrets, `AlwaysShoot`, `ShootPattern`); with a control
//!   that is not live yet (the behaviour-tree ones) or `AINone` it stands
//!   still, or shoved.
//!
//! Not ported: the colour of each player's car (`PlayersManager`'s
//! `_colorList`: blue, red, green, … by the order the players came, a
//! quarter lighter, drawn as the car's `ColorMask` sprite over its cabin;
//! the port draws no `ColorMask`, the player's car neither); the limit of
//! twelve players (`_colorList.Length`: a thirteenth is not made; no map
//! comes near, with clones); a player at the map's (0, 0) put on a random
//! free point (`OnNewPlayerMessageReceived`; no map has one); the second
//! `Local` (or hot-seat) car that a map with two, or a clone of the
//! player's, would make (an AI car here, held for the devices but not
//! driven by them; C# gave it the devices and the player's gun preset; no
//! map has one); the death of a `NoneLocal` AI car the way of the
//! player's (C#'s `GameModeSinglePlayer` asked `LocalDevice`, so it
//! restarted the level or spent a life and came back; here it is wrecked
//! as an AI car is: `CoverMap`, `EditorTest` and `Revenge` have one, none
//! of them gets killed in play); the player's car, `Local` or not, stays
//! dynamic when its car cannot move (2WeeksAgo's `TurretLoaderStat`, as
//! before); the repair of a `Car` (`Car.RepairUpdate`: a point a second
//! and the damaged smoke under half health; not the player's either);
//! `IsInView` round a `Car`'s exhaust puffs (C# made them only where the
//! camera looked; they are made everywhere, and unseen there); the
//! `Player` object C# made with the car (its `Name`, score and info panel:
//! multiplayer only);
//! `GeometryScale`'s lift of the sprite's depth
//! (`EntityWithGeometryScaleDelta`), the highlight border of enemies
//! (`HighlightEnemy`, `HighlightDeadEnemy`), the magnet on a
//! static car (`OnLinkTo`), the `EnergyComponent` of enemies (it spends
//! nothing for a car the player does not drive: no store is made), and a
//! second `Local` simple car on a map (the first is the player's; no map
//! has two).

pub mod cabin;
pub mod control;
pub mod death;
pub mod drive;
pub mod health_bar;
pub mod impact;
pub mod shield;
pub mod simple;
pub mod smoke;

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::content::cars::CarData;
use crate::content::map::{MapData, PlayerData, SimpleCarData};
use crate::controls::{LocalPlayer, PlayerIntent};
use crate::game::{GameState, LoadedMap, MapEntity, MapId};
use crate::render::camera::CameraTarget;
use crate::render::layers::Layer;
use crate::explosion::Health;
use crate::game_mode::Wreckable;
use crate::sound::CarSounds;
use crate::render::shadow::CastsShadow;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::units::{map_degrees_to_world, px};
use drive::{Body, DriveParams, DriveState, drive_step};

/// `Entity.MaxAngularVelocityDefault`.
const MAX_ANGULAR_VELOCITY: f32 = 50.0;
/// How fast the drawn wheels follow the stick (`Hardastic(0, 12)`).
const WHEEL_TURN_RATE: f32 = 12.0;

/// Debug: start the player's car at this point (map cells) instead of the
/// map's own start (`--start x,y`); used once, on the first map.
#[derive(Resource, Debug, Clone, Copy)]
pub struct StartOverride(pub Vec2);

/// `Entity.Radius`: half the longer side of the car's sprite, world units.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct CarRadius(pub f32);

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
        app.insert_resource(smoke::SmokeRng(rand::SeedableRng::seed_from_u64(7)))
            .add_systems(OnEnter(GameState::Playing), (spawn_player_car, spawn_ai_players, spawn_simple_cars).chain())
            .add_plugins((shield::ShieldPlugin, health_bar::HealthBarPlugin, death::DeathPlugin))
            .add_message::<impact::CarImpact>()
            .add_systems(
                FixedUpdate,
                (
                    drive_cars,
                    simple::drive_simple_cars,
                    simple::limit_spin,
                    smoke::car_smoke,
                    // The contacts of the last physics step, with the
                    // speeds going into them, before they are forgotten.
                    impact::detect_impacts,
                    impact::impact_sparks,
                    impact::remember_velocity,
                    cabin::sway_cabins,
                )
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
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
    // Only the first map: later maps start where they say.
    let at = start.map(|s| s.0);
    commands.remove_resource::<StartOverride>();
    let at = at.map(|c| (map.world(c.x, c.y), None));
    spawn_local_car(&mut commands, &map, &quad, &mut materials, at);
}

/// The map's `Players` cars other than the player's ([`ai_players`]), each
/// at its place, driven by its control (`MapManager.CreatePlayer`).
pub fn spawn_ai_players(
    mut commands: Commands,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for data in ai_players(&map.data) {
        // `MathHelper.DegToRad(-DirectionDeg)` in C#'s y-down space.
        let (at, angle) = (map.world(data.position.x, data.position.y), map_degrees_to_world(data.direction_deg));
        spawn_player(&mut commands, &map, &quad, &mut materials, data, data.id, at, angle, Role::AiCar);
    }
}

/// The `Players` car the player drives: the first with the `Local`
/// control, else the first that is not a prototype; none when a
/// `SimpleCars` car is the player's ([`local_simple_car`]).
pub fn local_player(data: &MapData) -> Option<&PlayerData> {
    if local_simple_car(data).is_some() {
        return None;
    }
    let players = &data.players.items;
    players
        .iter()
        .find(|p| p.control == "Local" && !p.is_prototype)
        .or_else(|| players.iter().find(|p| !p.is_prototype))
}

/// The map's `Players` cars that the AI drives (or nobody): every one
/// that is not a prototype, but the player's ([`local_player`]), in the
/// map's order (`MapManager`: `CreatePlayer` for each).
pub fn ai_players(data: &MapData) -> impl Iterator<Item = &PlayerData> {
    let local = local_player(data).map(|p| p.id);
    data.players.items.iter().filter(move |p| !p.is_prototype && Some(p.id) != local)
}

/// One of the map's `Players` cars as `data` describes it, with map id
/// `id`, at a world point and angle (its own place, or a clone's), as
/// `role` says (the player's or the AI's).
#[allow(clippy::too_many_arguments)]
pub fn spawn_player(
    commands: &mut Commands,
    map: &LoadedMap,
    quad: &UnitQuad,
    materials: &mut Assets<StandardMaterial>,
    data: &PlayerData,
    id: i64,
    at: Vec2,
    angle: f32,
    role: Role,
) -> Option<Entity> {
    let Some(car) = map.cars.get(&data.car_name) else {
        warn!("car {:?} (id {id}) is not in Cars.xml", data.car_name);
        return None;
    };
    let spec = CarSpec {
        id,
        car,
        position: at,
        angle,
        control: &data.control,
        weapon: None,
        role,
    };
    spawn_car(commands, map, quad, materials, spec)
}

/// The map's `SimpleCars` (`MapManager` "Simple Cars": every one that is
/// not a prototype, made after the players).
pub fn spawn_simple_cars(
    mut commands: Commands,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let local = local_simple_car(&map.data).map(|c| c.id);
    // The player's simple car comes with the player ([`spawn_local_car`]).
    for data in map.data.simple_cars.iter().filter(|c| !c.is_prototype && Some(c.id) != local) {
        let at = map.world(data.position.x, data.position.y);
        spawn_simple_car(&mut commands, &map, &quad, &mut materials, data, data.id, at, map_degrees_to_world(data.direction_deg));
    }
}

/// One of the map's `SimpleCars` as `data` describes it, with map id `id`,
/// at a world point and angle (its own place, or a clone's):
/// `EntitiesManager.CreateSimpleCar`.
#[allow(clippy::too_many_arguments)]
pub fn spawn_simple_car(
    commands: &mut Commands,
    map: &LoadedMap,
    quad: &UnitQuad,
    materials: &mut Assets<StandardMaterial>,
    data: &SimpleCarData,
    id: i64,
    at: Vec2,
    angle: f32,
) -> Option<Entity> {
    let Some(car) = map.cars.get_simple(&data.car_name) else {
        // C# would fail on the missing car's data.
        warn!("simple car {:?} (id {id}) is not in Cars.xml's SimpleCars", data.car_name);
        return None;
    };
    let spec = CarSpec {
        id,
        car,
        position: at,
        angle,
        control: &data.control,
        weapon: data.weapon_override(),
        role: if Some(data.id) == local_simple_car(&map.data).map(|c| c.id) && id == data.id { Role::LocalSimple } else { Role::Simple },
    };
    spawn_car(commands, map, quad, materials, spec)
}

/// The `SimpleCars` car the player drives: the first with the `Local`
/// control, when no `Players` car has it.
pub fn local_simple_car(data: &MapData) -> Option<&SimpleCarData> {
    let players = &data.players.items;
    if players.iter().any(|p| p.control == "Local" && !p.is_prototype) {
        return None;
    }
    data.simple_cars.iter().find(|c| c.control.trim() == "Local" && !c.is_prototype)
}

/// The player's car: at its place on the map, or at `at` (world) with its
/// own heading or the one given (world radians). The `Players` car with the
/// `Local` control, else the `SimpleCars` one ([`local_simple_car`]), else
/// the first of `Players`.
pub fn spawn_local_car(
    commands: &mut Commands,
    map: &LoadedMap,
    quad: &UnitQuad,
    materials: &mut Assets<StandardMaterial>,
    at: Option<(Vec2, Option<f32>)>,
) -> Option<Entity> {
    if let Some(data) = local_simple_car(&map.data) {
        let (position, angle) = match at {
            Some((p, heading)) => (p, heading.unwrap_or_else(|| map_degrees_to_world(data.direction_deg))),
            None => (map.world(data.position.x, data.position.y), map_degrees_to_world(data.direction_deg)),
        };
        return spawn_simple_car(commands, map, quad, materials, data, data.id, position, angle);
    }
    let Some(player) = local_player(&map.data) else {
        warn!("map {} has no player", map.data.name);
        return None;
    };
    let (position, heading) = at.unwrap_or((map.world(player.position.x, player.position.y), None));
    let angle = heading.unwrap_or_else(|| map_degrees_to_world(player.direction_deg));
    spawn_player(commands, map, quad, materials, player, player.id, position, angle, Role::Player)
}

/// Whose car it is: what it gets beyond the body, the sprite, the health,
/// the weapon, the team and the intent every car has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The local player's car (`Car`): the camera follows it, the devices
    /// drive it, with wheels, a swaying cabin, smoke, engine and tyre
    /// sounds, the energy store and the magnet.
    Player,
    /// A car of the map's `SimpleCars` (`ControlableEntity`): one sprite,
    /// its own simple driving ([`simple`]), a health bar when hurt.
    Simple,
    /// A `SimpleCars` car with the `Local` control: a simple car that the
    /// devices drive and the camera follows, with the player's gun.
    LocalSimple,
    /// Another of the map's `Players` (`Car`): the player's body, wheels,
    /// cabin, smoke and sounds, driven by its control ([`crate::ai`]),
    /// with the others' gun and a health bar when hurt.
    AiCar,
}

/// What a car is made from.
pub struct CarSpec<'a> {
    /// The map's id for it (`CustomId`).
    pub id: i64,
    pub car: &'a CarData,
    pub position: Vec2,
    pub angle: f32,
    /// Its `ControlType`'s name.
    pub control: &'a str,
    /// The map's weapon type when it overrides the car's (`SimpleCarData.Weapon`).
    pub weapon: Option<&'a str>,
    pub role: Role,
}

/// The weapon type to make: the map's override, else the car's own
/// (`SimpleCarData.Weapon`, `Default`). An override of `None` makes no
/// weapon, and then the car's own is made all the same (the factory
/// answers null, and `ControlableEntity` fills an empty hand with the
/// car's weapon on its first update).
pub fn weapon_type<'a>(over: Option<&'a str>, own: Option<&'a str>) -> Option<&'a str> {
    match over {
        Some("None") | None => own,
        Some(over) => Some(over),
    }
}

/// A car of any kind: `Entity`, `PhysicEntity`, `ControlableEntity` and
/// (for the `Players`) `Car` in one place. Returns it, or `None` when its
/// data or sprites are missing.
pub fn spawn_car(
    commands: &mut Commands,
    map: &LoadedMap,
    quad: &UnitQuad,
    materials: &mut Assets<StandardMaterial>,
    spec: CarSpec,
) -> Option<Entity> {
    let CarSpec { id, car, position, angle, control, weapon, role } = spec;
    let Some(base) = car.base_sprite().and_then(|s| map.sprites.get(&s).cloned()) else {
        warn!("car {} has no Base sprite", car.name);
        return None;
    };
    let player = role == Role::Player;
    // A `Car`: one of the map's `Players`, the player's or the AI's.
    let full = matches!(role, Role::Player | Role::AiCar);
    // The devices drive it and the camera follows it.
    let local = matches!(role, Role::Player | Role::LocalSimple);
    let kind = control::control_kind(control);
    let size = base.size * px(1.0) * car.scale;
    let radius = size.max_element() / 2.0;
    let friction = map.data.friction.unwrap_or(1.0);

    let mut entity = commands.spawn((
        Name::new(format!("{} {} {id}", if local { "Player car" } else { "Car" }, car.name)),
        MapEntity,
        MapId(id),
        control::Team(kind.team),
        control::AutoAimVisible(!kind.ai_none),
        CarRadius(radius),
        PlayerIntent::default(),
        CastsShadow { size },
        Health::full(car.max_health),
        Wreckable {
            sprites: ["Destroyed1", "Destroyed2"].map(|d| car.part_sprite(d).unwrap_or_default()),
            scale: car.scale,
            layer: if full { Layer::CarBase } else { Layer::CarBody },
        },
        Transform::from_translation(position.extend(0.0)).with_rotation(Quat::from_rotation_z(angle)),
        Visibility::default(),
    ));
    if car.shield > 0 {
        entity.insert(shield::Shield::new(car.shield));
    }
    if kind.local_device {
        entity.insert(control::LocalDevice);
    }
    // `PhysicEntity`: a box of the sprite's size (a simple car's scaled by
    // its `GeometryScale`, or a circle; an AI's `Car` too), `Entity`'s
    // damping. A `Car` turns less (`AngularDamping` 9; a simple car keeps 3).
    let (collider, static_body) = if player {
        (Collider::rectangle(size.x, size.y), false)
    } else {
        let scaled = size * car.geometry_scale;
        let collider = if car.use_circle_for_geometry {
            Collider::circle(scaled.max_element() / 2.0)
        } else {
            Collider::rectangle(scaled.x, scaled.y)
        };
        (collider, !car.can_move)
    };
    entity.insert((
        if static_body { RigidBody::Static } else { RigidBody::Dynamic },
        collider,
        CollisionEventsEnabled,
        impact::LastVelocity::default(),
        LinearDamping(0.95 * friction),
        AngularDamping(if full { 9.0 } else { 3.0 }),
        Friction::new(0.8).with_combine_rule(CoefficientCombine::GeometricMean),
        Restitution::new(0.5).with_combine_rule(CoefficientCombine::Max),
    ));
    if !static_body {
        entity.insert(TransformInterpolation);
    }
    if car.mass > 0.0 {
        entity.insert(Mass(car.mass));
    }
    // `WeaponFactory`: a weapon of the car's own type or the map's, at
    // `WeaponPositon` (pixels, y down; the gun itself is drawn unscaled).
    // The player's preset for the player, the others' for the rest.
    if let Some((kind, options)) = weapon_type(weapon, car.weapon.as_deref()).and_then(|w| crate::weapon::model::for_type(w, local)) {
        let at = car.weapon_positon.map_or(Vec2::ZERO, |p| Vec2::new(p.x, -p.y) * px(1.0) * car.scale);
        let mut rng = <rand::rngs::SmallRng as rand::SeedableRng>::seed_from_u64(id as u64);
        let mut gun = crate::weapon::Weapon::new(kind, options, at, size, local, &mut rng);
        // `CreateSimpleCar`: a static car with a gun the map names does not
        // aim into the walls beside it (`AdjustWeaponMaxAngle`).
        if static_body && weapon.is_some_and(|w| w != "None") {
            crate::weapon::max_angle::adjust(&mut gun, &map.grid, map.cell.0, position, angle, map.cell.0 * 2.5);
        }
        entity.insert(gun);
        if local {
            entity.insert(crate::weapon::PlayerEnergy::default());
        }
        if !full {
            // The gun's loop (a Minigun's) sounds on the car.
            entity.insert(CarSounds::new(None, 0.0));
        }
    }
    if full {
        insert_car_parts(&mut entity, car, radius, friction, kind);
    }
    if player {
        insert_player_parts(&mut entity, car, radius, friction, control);
    } else {
        insert_simple_parts(&mut entity, car, radius, kind, !full);
        crate::ai::set_control(&mut entity, control, id as u64);
        // The camera frames every player's car held for a local device
        // (`PlayerListUdpated`): a `NoneLocal` one of the AI's too.
        if local || (role == Role::AiCar && kind.local_device) {
            let max_speed = if !car.can_move {
                0.0
            } else if full {
                DriveParams::new(car, radius, friction).max_speed
            } else {
                simple::SimpleDrive::new(car.speed, car.acceleration, car.controllability, radius).max_speed
            };
            entity.insert(CameraTarget { radius, max_speed });
        }
        if local {
            entity.insert(LocalPlayer);
        }
    }
    entity.with_children(|parent| {
        if full {
            spawn_car_sprites(parent, map, car, quad, materials);
        } else {
            parent.spawn(sprite_bundle(&base, car.scale, Layer::CarBody, SpriteTint { color: [1.0; 4], lit: true }, quad, materials));
        }
    });
    Some(entity.id())
}

/// What every `Car` has, the player's or the AI's: its driving, the
/// swaying cabin, engine and tyre sounds (quieter for an AI's), smoke and
/// the magnet.
fn insert_car_parts(entity: &mut EntityCommands, car: &CarData, radius: f32, friction: f32, kind: control::ControlKind) {
    let params = DriveParams::new(car, radius, friction);
    entity.insert((
        cabin::CabinSway::new(car.body_elastic, radius),
        CarSounds::new(Some(car.engine_sound.as_str()).filter(|e| !e.is_empty()), car.wheels_sound_pitch)
            .loudness(kind.local_device),
        smoke::SmokePoints {
            wheel: car
                .left_wheel_position
                .map_or(Vec2::ZERO, |w| Vec2::new(w.x, -w.y) * px(1.0) * car.scale),
            tail: Vec2::new(-radius, 0.0),
            idle: car.load_wheel_body_color_views,
        },
        Car {
            params,
            drive: DriveState::default(),
        },
    ));
    // `GadgetFactory`: the magnet (`Link`), at `GadgetPosition` (pixels,
    // y down). An AI's waits for a tree that uses it (`CaptureTheBox`).
    if car.gadget.as_deref() == Some("Link") {
        let at = car.gadget_position.map_or(Vec2::ZERO, |p| Vec2::new(p.x, -p.y) * px(1.0) * car.scale);
        entity.insert(crate::gadget::Magnet::new(at));
    }
}

/// What only the player's `Car` has: the devices, the camera.
fn insert_player_parts(entity: &mut EntityCommands, car: &CarData, radius: f32, friction: f32, control: &str) {
    let params = DriveParams::new(car, radius, friction);
    entity.insert((LocalPlayer, CameraTarget { radius, max_speed: params.max_speed }));
    // Waiting for `SetControl(id,Local)`, as on LetItSnow's intro.
    if !crate::controls::is_players(control) {
        entity.insert(crate::controls::NoInput);
    }
}

/// What a car the player does not drive has (a simple car, an AI's
/// `Car`): the simple driving (a simple car's), its health bar,
/// whatever the magnet may do with it, and no hand on it when the control
/// is `AINone`.
fn insert_simple_parts(entity: &mut EntityCommands, car: &CarData, radius: f32, kind: control::ControlKind, simple_drive: bool) {
    if car.can_move {
        if simple_drive {
            entity.insert(simple::SimpleDrive::new(car.speed, car.acceleration, car.controllability, radius));
        }
        // What the magnet may grab (`CanLink`); a static car it could not
        // pull (`OnLinkTo` made it dynamic while linked; not ported).
        if car.can_link {
            entity.insert(crate::gadget::Linkable);
        }
    }
    if !kind.local_device {
        entity.insert(health_bar::HealthBar { radius, left: 0.0 });
    }
    if kind.ai_none {
        entity.insert(crate::controls::NoInput);
    }
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
            parent.spawn((sprite_bundle(&info, car.scale, Layer::CarBody, lit, quad, materials), cabin::Cabin));
        }
    }
}

/// Every `Car` drives by its intent (`Car.MovementControl`), whoever
/// writes it: the devices or the AI. A static one (a car that cannot move)
/// works its wheels and engine but is not moved.
pub fn drive_cars(mut cars: Query<(Forces, &mut Car, &PlayerIntent, &RigidBody)>) {
    for (mut forces, mut car, intent, rigid_body) in &mut cars {
        let angle = forces.rotation().as_radians();
        let body = Body {
            angle,
            velocity: forces.linear_velocity(),
            angular_velocity: forces.angular_velocity(),
        };
        let params = car.params;
        let out = drive_step(&params, &mut car.drive, body, intent.move_dir);
        if *rigid_body == RigidBody::Static {
            continue;
        }
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
