//! Gadgets of the player's car (`Weapons/ActiveGadget.cs`). The one ported
//! is the magnet, `Link` (`ChainGadget.cs`, `Entities/Chain.cs`): most of
//! the player's cars have it.
//!
//! Holding the gadget button for 0.2 s (`ChargeTimeMs`) shoots the magnet
//! towards the aim (the cursor, or ahead of the car), up to 8 units out.
//! A ray from the car finds what it would grab: something [`Linkable`]
//! (boxes, cars, moving gears; not bombs, gears or sensors) turns the rope
//! green and the magnet onto it. Letting go there links it: a rope of
//! eight points pulls it along behind the car (by how far the rope's end
//! lags behind it, `× 256`) and breaks past 12 units. Holding the button
//! again with something on the rope stops the car and drags the cargo
//! towards the aim; letting go then drops it. A heavy box is light while
//! towed (`HeavyBox`).
//!
//! On touch the top right quarter of the screen is the gadget's: hold a
//! finger there and drag from where it landed to aim (no drag: ahead).
//!
//! Not ported: C#'s touch circles and the flick that cuts the rope,
//! the highlight of what can be grabbed, sparks, the transporting beam,
//! the other gadgets (`Accelerator`, `Bomb`, `EditorPointer`).

pub mod rope;

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::car::drive_cars;
use crate::controls::PlayerIntent;
use crate::explosion::Explosive;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::layers::Layer;
use crate::render::lines::{LineMaterial, LineMeshBuilder};
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::sound::PlaySound;
use crate::units::px;
use rope::Rope;

/// `ChainGadget.ActiveDistance`: how far the magnet reaches, world units.
pub const REACH: f32 = 8.0;
/// `ActiveGadget.ChargeTimeMs`: how long the button is held before it acts.
pub const CHARGE: f32 = 0.2;
/// `HardasticVector2(…, 10)`: how fast the magnet head follows its aim.
const HEAD_RATE: f32 = 10.0;
/// `MoveCargo`: the pull towards the aim per unit, `8 × 2.56` (a box's mass).
const DRAG: f32 = 8.0 * 2.56;
/// The head's length (`ChainActive` is 24 px): the rope ends at its back.
const HEAD_LENGTH: f32 = 24.0 / 32.0;
const ACTIVE_SPRITE: &str = "Gadget\\ChainActive";
const INACTIVE_SPRITE: &str = "Gadget\\ChainInactive";

/// Can be grabbed by the magnet (`LinkablePhysicEntity.CanBeLinked`).
#[derive(Component, Debug, Default)]
pub struct Linkable;

/// A heavy box: this much denser while nothing tows it (`HeavyBox`).
#[derive(Component, Debug)]
pub struct Heavy(pub f32);

/// The rope's hold on its cargo.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Link {
    cargo: Entity,
    /// The point on the cargo, in its own frame.
    local: Vec2,
}

/// What the magnet would grab now.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Candidate {
    entity: Entity,
    at: Vec2,
    normal: Vec2,
}

/// The magnet gadget of a car.
#[derive(Component, Debug)]
pub struct Magnet {
    /// Where the rope starts on the car (`GadgetPosition`), its frame.
    attach: Vec2,
    held: f32,
    /// Shot out (the button held past the charge).
    active: bool,
    /// Far enough from the car to act on letting go (`IsOnActiveDistance`).
    far_enough: bool,
    /// Being dragged while the button is held (`_currentCargo`).
    dragging: Option<Entity>,
    candidate: Option<Candidate>,
    link: Option<Link>,
    /// The head: where it is, where it goes, which way it faces.
    head: Vec2,
    head_to: Vec2,
    head_dir: f32,
    rope: Rope,
    /// The rope's last pull on the cargo (`LinkedToForce`).
    last_force: Vec2,
    /// Drawn: the rope's mesh, and the head (active, inactive).
    views: Option<(Entity, Entity, Entity)>,
}

impl Magnet {
    pub fn new(attach: Vec2) -> Self {
        Magnet {
            attach,
            held: 0.0,
            active: false,
            far_enough: false,
            dragging: None,
            candidate: None,
            link: None,
            head: Vec2::ZERO,
            head_to: Vec2::ZERO,
            head_dir: 0.0,
            rope: Rope::new(Vec2::ZERO),
            last_force: Vec2::ZERO,
            views: None,
        }
    }

    /// Something is on the rope (`IsInSpecialState`).
    pub fn towing(&self) -> Option<Entity> {
        self.link.map(|l| l.cargo)
    }
}

/// The magnet's sounds (`ChainStart`, `ChainConnect`, `ChainBrake`).
fn sound(play: &mut MessageWriter<PlaySound>, name: &'static str, at: Vec2) {
    play.write(PlaySound {
        name,
        at: Some(at),
        pitch: 0.0,
        volume: 1.0,
    });
}

/// The gadgets' steps, for input to come before.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GadgetSystems;

pub struct GadgetPlugin;

impl Plugin for GadgetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (use_magnets, pull_ropes)
                .chain()
                .in_set(GadgetSystems)
                .before(drive_cars)
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, (spawn_views, draw_magnets).chain().run_if(in_state(GameState::Playing)))
        .add_systems(Startup, spawn_touch_zone)
        .add_systems(Update, show_touch_zone);
    }
}

type Car<'a> = (Entity, &'a mut Magnet, &'a mut PlayerIntent, &'a Position, &'a Rotation, &'a Collider);

/// The button: charge, shoot out and aim, then link or let go
/// (`PlayerCarControlBase.GadgetControl`, `ActiveGadget`, `ChainGadget`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn use_magnets(
    time: Res<Time>,
    spatial: SpatialQuery,
    mut cars: Query<Car>,
    linkable: Query<(), With<Linkable>>,
    skipped: Query<(), Or<(With<Sensor>, With<Explosive>)>>,
    mut cargo: Query<(&Position, &Rotation, Forces), Without<Magnet>>,
    mut heavy: Query<(&Heavy, &mut ColliderDensity)>,
    mut play: MessageWriter<PlaySound>,
) {
    let dt = time.delta_secs();
    for (car, mut m, mut intent, pos, rot, collider) in &mut cars {
        let at = pos.0;
        let facing = Vec2::from_angle(rot.as_radians());
        let radius = collider.shape_scaled().as_cuboid().map_or(1.0, |c| c.half_extents.max_element());
        if intent.gadget {
            m.held += dt;
            if m.held < CHARGE {
                continue;
            }
            if !m.active {
                // `OnStartActivity`.
                m.active = true;
                m.head = at;
                m.head_to = at;
                m.dragging = m.towing();
                sound(&mut play, "ChainStart", at);
            }
            // `GetTargetPosition`: the aim, no further than the reach.
            let aim = intent
                .gadget_at
                .or(intent.gadget_dir.map(|d| at + d * REACH))
                .unwrap_or(at + facing * REACH);
            let target = at + (aim - at).clamp_length_max(REACH);
            m.far_enough = target.distance(at) >= radius;
            match m.dragging {
                Some(c) => {
                    // `MoveCargo`: the car stands while it drags.
                    intent.move_dir = Vec2::ZERO;
                    let Some(link) = m.link.filter(|l| l.cargo == c) else {
                        m.dragging = None;
                        continue;
                    };
                    if let Ok((cpos, crot, mut forces)) = cargo.get_mut(c) {
                        let point = cpos.0 + crot * link.local;
                        let force = (target - point) * DRAG - m.last_force * 0.5;
                        forces.apply_force(force);
                    }
                }
                None => {
                    // `FindNewCargo`.
                    let delta = target - at;
                    m.candidate = m.candidate.filter(|c| c.at.distance(at) <= REACH + 1.0);
                    let mut to = at;
                    if m.far_enough && let Ok(dir) = Dir2::new(delta) {
                        let filter = SpatialQueryFilter::from_excluded_entities([car]);
                        let hit = spatial.cast_ray_predicate(at, dir, delta.length(), true, &filter, &|e| !skipped.contains(e));
                        match hit {
                            Some(hit) if linkable.contains(hit.entity) => {
                                let p = at + dir * hit.distance;
                                m.candidate = Some(Candidate { entity: hit.entity, at: p, normal: hit.normal });
                                to = p;
                            }
                            Some(hit) => {
                                m.candidate = None;
                                to = at + dir * hit.distance;
                            }
                            None => {
                                // Keep a candidate the aim has not swung far from.
                                if let Some(c) = m.candidate
                                    && (c.at - at).angle_to(delta).abs() > std::f32::consts::PI / 6.0
                                {
                                    m.candidate = None;
                                }
                                to = m.candidate.map_or(target, |c| c.at);
                            }
                        }
                    } else {
                        m.candidate = None;
                    }
                    m.head_to = to;
                    // `HardasticVector2`.
                    let k = (HEAD_RATE * dt).min(1.0);
                    m.head = m.head.lerp(m.head_to, k);
                    m.head_dir = match m.candidate {
                        Some(c) if m.head.distance(m.head_to) < 0.1 => (-c.normal).to_angle(),
                        _ => (m.head - at).to_angle(),
                    };
                }
            }
        } else {
            if m.held >= CHARGE && m.active {
                // `EndActivity` / `OnApplyActivity`.
                let grab = m.candidate.filter(|_| m.far_enough && m.dragging.is_none());
                match grab {
                    Some(c) => {
                        if let Ok((cpos, crot, _)) = cargo.get_mut(c.entity) {
                            let local = crot.inverse() * (c.at - c.normal * 0.125 - cpos.0);
                            unlink(&mut m, &mut heavy);
                            m.link = Some(Link { cargo: c.entity, local });
                            if let Ok((h, mut density)) = heavy.get_mut(c.entity) {
                                density.0 /= h.0;
                            }
                            sound(&mut play, "ChainConnect", at);
                        }
                    }
                    None => {
                        if m.link.is_some() {
                            sound(&mut play, "ChainBrake", at);
                        } else {
                            sound(&mut play, "ChainStart", at);
                        }
                        unlink(&mut m, &mut heavy);
                    }
                }
            }
            m.held = 0.0;
            m.active = false;
            m.far_enough = false;
            m.dragging = None;
            m.candidate = None;
        }
    }
}

/// `Chain.Unlink`: the cargo goes, a heavy box heavy again.
fn unlink(m: &mut Magnet, heavy: &mut Query<(&Heavy, &mut ColliderDensity)>) {
    if let Some(link) = m.link.take()
        && let Ok((h, mut density)) = heavy.get_mut(link.cargo)
    {
        density.0 *= h.0;
    }
}

/// `Chain.Update`: the rope from the car to the cargo (pulling it) or to
/// the head; too long, it breaks.
#[allow(clippy::type_complexity)]
fn pull_ropes(
    mut cars: Query<(&mut Magnet, &Position, &Rotation)>,
    mut cargo: Query<(&Position, &Rotation, Forces), Without<Magnet>>,
    mut heavy: Query<(&Heavy, &mut ColliderDensity)>,
    mut play: MessageWriter<PlaySound>,
) {
    for (mut m, pos, rot) in &mut cars {
        let from = pos.0 + rot * m.attach;
        if let Some(link) = m.link {
            let Ok((cpos, crot, mut forces)) = cargo.get_mut(link.cargo) else {
                m.link = None;
                continue;
            };
            let to = cpos.0 + crot * link.local;
            if pos.0.distance(to) > rope::MAX_LENGTH {
                sound(&mut play, "ChainBrake", pos.0);
                unlink(&mut m, &mut heavy);
                continue;
            }
            let force = m.rope.step(from, to);
            m.last_force = force;
            forces.apply_force_at_point(force, to);
        } else if m.active {
            let to = Rope::reach(from, m.head - Vec2::from_angle(m.head_dir) * HEAD_LENGTH);
            m.rope.step(from, to);
        } else {
            m.rope.lay(from, from);
        }
    }
}

/// The rope's mesh and the magnet head, made once per magnet.
fn spawn_views(
    mut commands: Commands,
    mut magnets: Query<&mut Magnet>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lines: ResMut<Assets<LineMaterial>>,
) {
    for mut m in &mut magnets {
        if m.views.is_some() {
            continue;
        }
        let rope = commands
            .spawn((
                Name::new("Magnet rope"),
                MapEntity,
                Mesh3d(meshes.add(LineMeshBuilder::default().build())),
                MeshMaterial3d(lines.add(LineMaterial {
                    color: LinearRgba::WHITE,
                    camera_offset: 0.0,
                })),
                Transform::default(),
                Visibility::Hidden,
                bevy::camera::visibility::NoFrustumCulling,
            ))
            .id();
        let tint = SpriteTint {
            color: [1.0; 4],
            lit: true,
        };
        let mut head = |name: &str| {
            let sprite = map.sprites.get(name).map(|i| sprite_bundle(i, 1.0, Layer::CarBody, tint, &quad, &mut materials));
            match sprite {
                Some(s) => commands.spawn((Name::new("Magnet head"), MapEntity, s, Visibility::Hidden)).id(),
                None => commands.spawn((MapEntity, Transform::default(), Visibility::Hidden)).id(),
            }
        };
        let (active, inactive) = (head(ACTIVE_SPRITE), head(INACTIVE_SPRITE));
        m.views = Some((rope, active, inactive));
    }
}

/// `Chain.Draw` (a line along the rope, thinner as it stretches, green
/// on something to grab) and `ChainGadget.Draw` (the head).
#[allow(clippy::type_complexity)]
fn draw_magnets(
    magnets: Query<(&Magnet, &Position)>,
    mut ropes: Query<(&Mesh3d, &MeshMaterial3d<LineMaterial>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut lines: ResMut<Assets<LineMaterial>>,
    mut views: Query<(&mut Transform, &mut Visibility)>,
    map: Res<LoadedMap>,
) {
    for (m, pos) in &magnets {
        let Some((rope, active, inactive)) = m.views else {
            continue;
        };
        let shown = m.active || m.link.is_some();
        if let Ok((_, mut v)) = views.get_mut(rope) {
            v.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
        }
        if shown && let Ok((mesh, material)) = ropes.get_mut(rope) {
            let points = m.rope.points();
            let end = *points.last().unwrap_or(&pos.0);
            let width = px(6.0 - 4.0 * (pos.0.distance(end) / rope::MAX_LENGTH)).max(px(1.0));
            let z = Layer::CarBody.z();
            let mut b = LineMeshBuilder::default();
            for w in points.windows(2) {
                b.add_line(w[0].extend(z), w[1].extend(z), width);
            }
            if let Some(mut mesh) = meshes.get_mut(&mesh.0) {
                *mesh = b.build();
            }
            let color = if m.candidate.is_some() { LinearRgba::GREEN } else { LinearRgba::WHITE };
            if let Some(mut mat) = lines.get_mut(&material.0)
                && mat.color != color
            {
                mat.color = color;
            }
        }
        // The head while it is out and nothing is on the rope; its sprite's
        // back edge on the rope's end (`Origin = (Width, Height / 2)`).
        let head_shown = m.active && m.dragging.is_none();
        for (view, on) in [(active, m.candidate.is_some()), (inactive, m.candidate.is_none())] {
            let Ok((mut t, mut v)) = views.get_mut(view) else {
                continue;
            };
            let visible = head_shown && on;
            v.set_if_neq(if visible { Visibility::Inherited } else { Visibility::Hidden });
            if visible {
                let name = if view == active { ACTIVE_SPRITE } else { INACTIVE_SPRITE };
                let half = map.sprites.get(name).map_or(HEAD_LENGTH, |i| px(i.size.x)) / 2.0;
                let dir = Vec2::from_angle(m.head_dir);
                t.translation = (m.head - dir * half).extend(Layer::CarBody.z());
                t.rotation = Quat::from_rotation_z(m.head_dir);
            }
        }
    }
}

/// Where the gadget's touch zone is, shown on touch screens.
#[derive(Component)]
struct TouchZone;

/// The circle's size, logical pixels.
const ZONE_CIRCLE: f32 = 96.0;

fn spawn_touch_zone(mut commands: Commands) {
    commands.spawn((
        TouchZone,
        Node {
            position_type: PositionType::Absolute,
            // The middle of the top right quarter (`in_gadget_zone`).
            left: Val::Percent(75.0),
            top: Val::Percent(25.0),
            margin: UiRect {
                left: Val::Px(-ZONE_CIRCLE / 2.0),
                top: Val::Px(-ZONE_CIRCLE / 2.0),
                ..default()
            },
            width: Val::Px(ZONE_CIRCLE),
            height: Val::Px(ZONE_CIRCLE),
            border: UiRect::all(Val::Px(3.0)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
        Pickable::IGNORE,
    ));
}

/// Faint while it waits, bright while a finger holds it; only on touch,
/// and only with a magnet.
fn show_touch_zone(
    scheme: Res<crate::controls::ActiveScheme>,
    touch: Res<crate::controls::GadgetTouch>,
    magnets: Query<(), (With<Magnet>, With<crate::controls::LocalPlayer>)>,
    mut zones: Query<(&mut BorderColor, &mut BackgroundColor), With<TouchZone>>,
) {
    let shown = *scheme == crate::controls::ActiveScheme::Touch && !magnets.is_empty();
    let alpha = if !shown { 0.0 } else if touch.finger.is_some() { 0.6 } else { 0.25 };
    for (mut border, mut back) in &mut zones {
        let edge = Color::srgba(1.0, 1.0, 1.0, alpha);
        if border.top != edge {
            *border = BorderColor::all(edge);
            back.0 = Color::srgba(1.0, 1.0, 1.0, alpha * 0.25);
        }
    }
}
