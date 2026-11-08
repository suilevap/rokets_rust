//! Map props the car interacts with: boxes and accelerator pads.
//!
//! - **Boxes** (`Box`, `NonLinkableBox`, `IgnoreWeaponBox`, `DamagableBox`,
//!   `HeavyBox` in `EntitiesFactory`): plain physics bodies the car pushes
//!   around, as big as their sprite, density 1, friction 0.8, bounce 0.5,
//!   damped like every C# entity. A heavy box is 64 times heavier (and
//!   harder to turn) while nothing tows it (`HeavyBox`); towing comes with
//!   the magnet gadget. The kinds differ later: which can be towed, which
//!   bullets pass, which blow up when hit. A `DamagableBox` always uses its
//!   own sprite (`EntitiesManager.CreateDamagableBox`).
//!   `RestrictX` / `RestrictY` / `RestrictRotation` lock those axes; all
//!   three make the box static (`Entity.SetRestrictions`).
//! - **Accelerators** (`Accelerator`): a pad on the floor that any body
//!   touching it gets thrown along, 30 units/s faster (an impulse of
//!   30 × mass). It is drawn twice, the second arrow three quarters of its
//!   length ahead; the arrows light up yellow in turn every second, and
//!   both flash white after something used the pad.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::content::map::EntityData;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::layers::Layer;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::units::{map_degrees_to_world, px};

/// `ControlableEntity`-style damping for all entities (`Entity` ctor).
const ANGULAR_DAMPING: f32 = 0.95;
const LINEAR_DAMPING: f32 = 0.95;
/// `HeavyBox._coefficientUnlinked`.
const HEAVY_BOX_FACTOR: f32 = 64.0;
/// Speed an accelerator adds, units per second.
pub const ACCELERATOR_BOOST: f32 = 30.0;
/// Seconds both arrows flash after the pad was used.
const ACCELERATOR_FLASH: f32 = 1.0;

/// What a map entity becomes, for the kinds this module handles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PropKind {
    /// A pushable box; `density` scales its mass and inertia.
    Box { density: f32 },
    Accelerator,
}

impl PropKind {
    /// The prop for a map entity type, if this module handles it.
    pub fn from_entity_type(kind: &str) -> Option<PropKind> {
        match kind {
            "Box" | "NonLinkableBox" | "IgnoreWeaponBox" | "DamagableBox" => Some(PropKind::Box { density: 1.0 }),
            "HeavyBox" => Some(PropKind::Box {
                density: HEAVY_BOX_FACTOR,
            }),
            "Accelerator" => Some(PropKind::Accelerator),
            _ => None,
        }
    }
}

/// The sprite an entity is drawn with: some kinds ignore the map's choice.
pub fn entity_sprite(e: &EntityData) -> Option<String> {
    match e.kind.as_str() {
        "DamagableBox" => Some("Entities\\DamagableBox".into()),
        "Accelerator" => Some("Entities\\Accelerator".into()),
        _ => (!e.sprite.is_empty()).then(|| e.sprite.clone()),
    }
}

/// The scale an entity is drawn and sized with.
fn entity_scale(e: &EntityData) -> f32 {
    if e.kind == "DamagableBox" { 1.0 } else { e.scale }
}

/// The axes the map locks for an entity, or `None` if it is fixed in place.
pub fn locked_axes(restrict_x: bool, restrict_y: bool, restrict_rotation: bool) -> Option<LockedAxes> {
    if restrict_x && restrict_y && restrict_rotation {
        return None;
    }
    let mut axes = LockedAxes::new();
    if restrict_x {
        axes = axes.lock_translation_x();
    }
    if restrict_y {
        axes = axes.lock_translation_y();
    }
    if restrict_rotation {
        axes = axes.lock_rotation();
    }
    Some(axes)
}

#[derive(Component, Debug)]
pub struct Accelerator {
    /// Direction it throws things, world space.
    pub dir: Vec2,
    /// Seconds of white flash left.
    flash: f32,
}

/// One of an accelerator's two arrows (0 = back, 1 = front).
#[derive(Component, Debug)]
struct AcceleratorArrow(usize);

pub struct PropsPlugin;

impl Plugin for PropsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_props).add_systems(
            Update,
            (boost_on_contact, blink_accelerators).run_if(in_state(GameState::Playing)),
        );
    }
}

fn spawn_props(
    mut commands: Commands,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let friction = map.data.friction.unwrap_or(1.0);
    let lit = SpriteTint {
        color: [1.0; 4],
        lit: true,
    };
    for e in map.data.entities.iter() {
        let Some(kind) = PropKind::from_entity_type(&e.kind) else {
            continue;
        };
        let Some(info) = entity_sprite(e).and_then(|s| map.sprites.get(&s).cloned()) else {
            continue;
        };
        let scale = entity_scale(e);
        let size = info.size * px(1.0) * scale;
        let angle = map_degrees_to_world(e.direction_deg);
        let transform =
            Transform::from_translation(map.world(e.position.x, e.position.y).extend(0.0))
                .with_rotation(Quat::from_rotation_z(angle));
        let mut entity = commands.spawn((
            Name::new(format!("{} {}", e.kind, e.id)),
            MapEntity,
            transform,
            Visibility::default(),
            Collider::rectangle(size.x, size.y),
        ));
        match kind {
            PropKind::Box { density } => {
                match locked_axes(e.restrict_x, e.restrict_y, e.restrict_rotation) {
                    Some(axes) => {
                        entity.insert((RigidBody::Dynamic, axes, TransformInterpolation));
                    }
                    None => {
                        entity.insert(RigidBody::Static);
                    }
                }
                entity.insert((
                    ColliderDensity(density),
                    Friction::new(0.8).with_combine_rule(CoefficientCombine::GeometricMean),
                    Restitution::new(0.5).with_combine_rule(CoefficientCombine::Max),
                    LinearDamping(LINEAR_DAMPING * friction),
                    AngularDamping(ANGULAR_DAMPING),
                ));
                entity.with_children(|p| {
                    p.spawn(sprite_bundle(&info, scale, Layer::Prop, lit, &quad, &mut materials));
                });
            }
            PropKind::Accelerator => {
                entity.insert((
                    RigidBody::Static,
                    Sensor,
                    CollisionEventsEnabled,
                    Accelerator {
                        dir: Vec2::from_angle(angle),
                        flash: 0.0,
                    },
                ));
                let ahead = size.x * 0.75;
                entity.with_children(|p| {
                    for (i, offset) in [0.0, ahead].into_iter().enumerate() {
                        let mut bundle = sprite_bundle(&info, scale, Layer::Accelerator, lit, &quad, &mut materials);
                        bundle.2.translation.x = offset;
                        p.spawn((bundle, AcceleratorArrow(i)));
                    }
                });
            }
        }
    }
}

fn boost_on_contact(
    mut started: MessageReader<CollisionStart>,
    mut pads: Query<&mut Accelerator>,
    mut bodies: Query<(&RigidBody, &mut LinearVelocity)>,
) {
    for event in started.read() {
        let pairs = [(event.collider1, event.body2), (event.collider2, event.body1)];
        for (pad, other) in pairs {
            let (Ok(mut pad), Some(other)) = (pads.get_mut(pad), other) else {
                continue;
            };
            if let Ok((RigidBody::Dynamic, mut v)) = bodies.get_mut(other) {
                v.0 += pad.dir * ACCELERATOR_BOOST;
                pad.flash = ACCELERATOR_FLASH;
            }
        }
    }
}

fn blink_accelerators(
    time: Res<Time>,
    mut pads: Query<(&mut Accelerator, &Children)>,
    mut arrows: Query<(&AcceleratorArrow, &mut SpriteTint)>,
) {
    const YELLOW: [f32; 4] = [1.0, 1.0, 0.0, 1.0];
    const GRAY: [f32; 4] = [0.5, 0.5, 0.5, 1.0];
    let phase = time.elapsed_secs() % 1.0;
    for (mut pad, children) in &mut pads {
        pad.flash = (pad.flash - time.delta_secs()).max(0.0);
        for child in children.iter() {
            let Ok((arrow, mut tint)) = arrows.get_mut(child) else {
                continue;
            };
            let color = if pad.flash > 0.0 {
                let t = pad.flash / ACCELERATOR_FLASH;
                std::array::from_fn(|i| GRAY[i] + (1.0 - GRAY[i]) * t)
            } else {
                let lit_arrow = if phase < 0.4 {
                    Some(0)
                } else if phase < 0.8 {
                    Some(1)
                } else {
                    None
                };
                if lit_arrow == Some(arrow.0) { YELLOW } else { GRAY }
            };
            if tint.color != color {
                tint.color = color;
            }
        }
    }
}
