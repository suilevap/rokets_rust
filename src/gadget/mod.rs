//! Gadgets of the player's car (`Weapons/ActiveGadget.cs`). Two are ported:
//! the magnet, `Link` (`ChainGadget.cs`, `Entities/Chain.cs`), which most
//! of the player's cars have, and the accelerator ([`accelerator`]).
//!
//! The magnet:
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
//! The enemies' cars use it too, by the same intent (`PlayerIntent::gadget`,
//! `gadget_dir`, `gadget_cancel`): the AI's `Grab` and `Drop` actions
//! (`ai::nodes::cargo`) hold the button, aim it and let go, as C#'s
//! `FireType.GadgetActivityStart`, `End` and `Cancel` did. The 0.2 s charge
//! holds for them too (C#'s AI skipped it: its flags went straight to
//! `ProcessActivity`).
//!
//! Not ported: C#'s touch circles and the flick that cuts the rope,
//! the highlight of what can be grabbed, sparks, the transporting beam,
//! the other gadgets (`Bomb`, `EditorPointer`).

pub mod accelerator;
pub mod rope;

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::car::drive_cars;
use crate::controls::PlayerIntent;
use crate::explosion::Explosive;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::layers::Layer;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::sound::PlaySound;
use crate::units::px;
use accelerator::Accelerator;
use rope::Rope;

/// `ChainGadget.ActiveDistance`: how far the magnet reaches, world units.
pub const REACH: f32 = 8.0;
/// `ActiveGadget.ChargeTimeMs`: how long the button is held before it acts.
pub const CHARGE: f32 = 0.2;
/// `HardasticVector2(…, 10)`: how fast the magnet head follows its aim.
const HEAD_RATE: f32 = 10.0;
/// `MoveCargo`: the pull towards the aim per unit, `8 × 2.56` (a box's mass).
const DRAG: f32 = 8.0 * 2.56;
/// `MoveCargo`: nearer the car than this, the cargo is not pressed into it.
const NEAR: f32 = 0.25;
/// The head's length (`ChainActive` is 24 px): the rope ends at its back.
const HEAD_LENGTH: f32 = 24.0 / 32.0;
const ACTIVE_SPRITE: &str = "Gadget\\ChainActive";
/// The rope's pieces (`SpritePath`): plain, and fading for its ends.
const PIXEL: &str = "WhitePixel";
const GRADIENT: &str = "Gradient2Pixel";
/// The rope's shadow on the floor (`Color.Black * 0.5`).
const SHADOW_ALPHA: f32 = 0.5;
const INACTIVE_SPRITE: &str = "Gadget\\ChainInactive";

/// Every texture the magnet draws with, to load with a map that gives a
/// car one.
pub const TEXTURES: [&str; 4] = [ACTIVE_SPRITE, INACTIVE_SPRITE, PIXEL, GRADIENT];

/// A car with a gadget the player's button works (`_hasGadget`).
pub type WithGadget = Or<(With<Magnet>, With<Accelerator>)>;

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

/// What a magnet is drawn with: the rope's pieces (each with its shadow)
/// and the head (active, inactive).
#[derive(Debug)]
struct Views {
    pieces: Vec<(Entity, Entity)>,
    active: Entity,
    inactive: Entity,
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
    views: Option<Views>,
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

    /// The entities its rope and head are drawn with (made on the first
    /// frame it exists); to despawn when the car loses the gadget.
    pub fn view_entities(&self) -> Vec<Entity> {
        self.views.iter().flat_map(|v| v.pieces.iter().flat_map(|&(a, b)| [a, b]).chain([v.active, v.inactive])).collect()
    }

    /// Something is on the rope (`IsInSpecialState`, `LinkedSlave`).
    pub fn towing(&self) -> Option<Entity> {
        self.link.map(|l| l.cargo)
    }

    /// What the magnet would grab if let go now (`ChainGadget.Candidate`).
    pub fn candidate(&self) -> Option<Entity> {
        self.candidate.map(|c| c.entity)
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
        .add_systems(Update, show_touch_zone)
        .add_plugins(accelerator::AcceleratorPlugin);
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
    sizes: Query<(&ComputedMass, &Collider, Option<&LockedAxes>), Without<Magnet>>,
    mut heavy: Query<(&Heavy, &mut ColliderDensity)>,
    mut play: MessageWriter<PlaySound>,
) {
    let dt = time.delta_secs();
    for (car, mut m, mut intent, pos, rot, collider) in &mut cars {
        let at = pos.0;
        if std::mem::take(&mut intent.gadget_cancel) {
            // `ChainGadget.Cancel`: the cargo goes, nothing is linked.
            if m.link.is_some() {
                sound(&mut play, "ChainBrake", at);
            }
            unlink(&mut m, &mut heavy);
            m.held = 0.0;
            m.active = false;
            m.far_enough = false;
            m.dragging = None;
            m.candidate = None;
            continue;
        }
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
                        let mut force = (target - point) * DRAG - m.last_force * 0.5;
                        let (mass, shape, locks) = sizes.get(c).map_or((1.0, None, None), |(m, s, l)| (m.value(), Some(s), l));
                        let slides = locks.is_some_and(|l| !l.is_rotation_locked() && (l.is_translation_x_locked() || l.is_translation_y_locked()));
                        if slides {
                            // On a rail: pulled at the rope's point.
                            forces.apply_force_at_point(force, point);
                        } else {
                            // Against the car it may not be pressed into it,
                            // nor the car pushed with it.
                            let gap = shape.map_or(f32::INFINITY, |shape| {
                                let d = cpos.0 - at;
                                d.length() - radius_along(collider, rot, d) - radius_along(shape, crot, -d)
                            });
                            if gap < NEAR {
                                let away = (cpos.0 - at).normalize_or_zero();
                                let closing = forces.linear_velocity().dot(away);
                                if closing < 0.0 {
                                    forces.apply_linear_impulse(away * -closing * mass * 1.1);
                                }
                                let push = force.dot(away);
                                if push < 0.0 {
                                    force -= away * push;
                                }
                            }
                            forces.apply_force(force);
                        }
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

/// `GetEstimatedRadius`: how far a body reaches from its middle along
/// `dir` (a box's half size seen that way).
fn radius_along(collider: &Collider, rot: &Rotation, dir: Vec2) -> f32 {
    let dir = dir.normalize_or_zero();
    match collider.shape_scaled().as_cuboid() {
        Some(c) => {
            let (x, y) = (*rot * Vec2::X, *rot * Vec2::Y);
            dir.dot(x).abs() * c.half_extents.x + dir.dot(y).abs() * c.half_extents.y
        }
        None => 0.5,
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

/// The rope's pieces and the magnet head, made once per magnet.
fn spawn_views(
    mut commands: Commands,
    mut magnets: Query<&mut Magnet>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for mut m in &mut magnets {
        if m.views.is_some() {
            continue;
        }
        let mut sprite = |name: &str, layer: Layer, tint: SpriteTint| {
            let sprite = map.sprites.get(name).map(|i| sprite_bundle(i, 1.0, layer, tint, &quad, &mut materials));
            match sprite {
                Some(s) => commands.spawn((Name::new("Magnet"), MapEntity, s, Visibility::Hidden)).id(),
                None => commands.spawn((MapEntity, Transform::default(), Visibility::Hidden)).id(),
            }
        };
        let lit = SpriteTint {
            color: [1.0; 4],
            lit: true,
        };
        let shadow = SpriteTint {
            color: [0.0, 0.0, 0.0, SHADOW_ALPHA],
            lit: false,
        };
        // `SpritePath.Draw(…, useShadow, enableAlphaBegin, enableAlphaEnd)`:
        // the two end pieces fade out (`Gradient2Pixel`).
        let pieces = (0..rope::POINTS - 1)
            .map(|i| {
                let texture = if i == 0 || i == rope::POINTS - 2 { GRADIENT } else { PIXEL };
                (sprite(texture, Layer::CarBody, lit), sprite(texture, Layer::Shadow, shadow))
            })
            .collect();
        let (active, inactive) = (sprite(ACTIVE_SPRITE, Layer::CarBody, lit), sprite(INACTIVE_SPRITE, Layer::CarBody, lit));
        m.views = Some(Views { pieces, active, inactive });
    }
}

/// `Chain.Draw`: a lit strip along the rope (`DepthConst.Chain`, over the
/// cars), thinner as it stretches, green on something to grab, its ends
/// fading out, with its shadow on the floor; and `ChainGadget.Draw`, the
/// head.
#[allow(clippy::type_complexity)]
fn draw_magnets(
    magnets: Query<(&Magnet, &Position)>,
    mut views: Query<(&mut Transform, &mut Visibility)>,
    mut tints: Query<&mut SpriteTint>,
    map: Res<LoadedMap>,
) {
    for (m, pos) in &magnets {
        let Some(v) = &m.views else {
            continue;
        };
        let (active, inactive) = (v.active, v.inactive);
        let shown = m.active || m.link.is_some();
        let points = m.rope.points();
        let end = *points.last().unwrap_or(&pos.0);
        let width = px(6.0 - 4.0 * (pos.0.distance(end) / rope::MAX_LENGTH)).max(px(1.0));
        let color = if m.candidate.is_some() { [0.0, 1.0, 0.0, 1.0] } else { [1.0; 4] };
        for (i, (&(piece, shadow), w)) in v.pieces.iter().zip(points.windows(2)).enumerate() {
            let delta = w[1] - w[0];
            // The last piece turned round: its clear end at the cargo.
            let turn = delta.to_angle() + if i == rope::POINTS - 2 { std::f32::consts::PI } else { 0.0 };
            let visible = shown && delta.length() > 1e-4;
            for (e, layer) in [(piece, Layer::CarBody), (shadow, Layer::Shadow)] {
                let Ok((mut t, mut vis)) = views.get_mut(e) else {
                    continue;
                };
                vis.set_if_neq(if visible { Visibility::Inherited } else { Visibility::Hidden });
                if visible {
                    t.translation = ((w[0] + w[1]) / 2.0).extend(layer.z());
                    t.rotation = Quat::from_rotation_z(turn);
                    t.scale = Vec3::new(delta.length(), width, 1.0);
                }
            }
            if let Ok(mut tint) = tints.get_mut(piece)
                && tint.color != color
            {
                tint.color = color;
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
/// and only with a gadget: the magnet's always, the accelerator's while it
/// is ready (`GetTouchView`).
fn show_touch_zone(
    scheme: Res<crate::controls::ActiveScheme>,
    touch: Res<crate::controls::GadgetTouch>,
    gadgets: Query<(Has<Magnet>, Option<&Accelerator>), With<crate::controls::LocalPlayer>>,
    mut zones: Query<(&mut BorderColor, &mut BackgroundColor), With<TouchZone>>,
) {
    let has = gadgets.iter().any(|(magnet, accelerator)| magnet || accelerator.is_some());
    let waiting = gadgets.iter().any(|(magnet, accelerator)| magnet || accelerator.is_some_and(Accelerator::ready));
    let shown = *scheme == crate::controls::ActiveScheme::Touch && has;
    let alpha = if !shown {
        0.0
    } else if touch.finger.is_some() {
        0.6
    } else if waiting {
        0.25
    } else {
        0.0
    };
    for (mut border, mut back) in &mut zones {
        let edge = Color::srgba(1.0, 1.0, 1.0, alpha);
        if border.top != edge {
            *border = BorderColor::all(edge);
            back.0 = Color::srgba(1.0, 1.0, 1.0, alpha * 0.25);
        }
    }
}
