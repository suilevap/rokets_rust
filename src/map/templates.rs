//! What each map entity is made of: the one place that reads its type.
//!
//! The map stores an entity type (`EntityType` in the XML, as the C#
//! `EntitiesFactory` did). Gameplay never looks at it: loading turns each
//! map entity into an [`EntityTemplate`], a set of properties that become
//! components, and systems act on those components. A new kind of entity is
//! a new arm here composed from the same pieces (or a new piece and the
//! system that gives it behaviour).
//!
//! Plain data, no Bevy app: unit-testable.

use crate::animation::{ColorTrack, GRAY, Pulse, WHITE, Wave, YELLOW};
use crate::content::map::{EntityData, EntityType, MapData};
use crate::render::layers::Layer;

/// `HeavyBox._coefficientUnlinked`.
const HEAVY_BOX_DENSITY: f32 = 64.0;
/// `Accelerator.OnCollide`: impulse `30 × mass`, i.e. 30 units/s.
const ACCELERATOR_BOOST: f32 = 30.0;
/// Radians per second gears turn (`Coin`: a turn per 6.28 s).
const GEAR_SPIN: f32 = 1.0;

/// How a map entity is built.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityTemplate {
    /// What it is drawn with; the first sprite also sizes its body.
    pub sprites: Vec<SpriteSpec>,
    /// A physics body, if it has one.
    pub body: Option<BodySpec>,
    /// Throws bodies that touch it along its facing.
    pub boost: Option<BoostSpec>,
    /// The player picks it up by driving into it (gears).
    pub pickup: bool,
    /// Turns on the spot, radians per second.
    pub spin: Option<f32>,
    /// Has a shadow (`DynamicShadow`): every physical entity except
    /// bullet-proof walls. Fixed ones marked `TryCastStaticShadow` cast it
    /// into the baked light instead ([`static_shadow_outline`]).
    pub casts_shadow: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpriteSpec {
    /// Content name under `Sprites/`.
    pub name: String,
    pub scale: f32,
    pub layer: Layer,
    /// How far ahead along the entity's facing, in sprite widths.
    pub ahead: f32,
    /// A repeating colour animation, if any.
    pub tint_loop: Option<(Wave, ColorTrack)>,
}

impl SpriteSpec {
    fn new(name: impl Into<String>, scale: f32, layer: Layer) -> Self {
        SpriteSpec {
            name: name.into(),
            scale,
            layer,
            ahead: 0.0,
            tint_loop: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoostSpec {
    /// Speed added, units per second.
    pub speed: f32,
    /// Played over the entity's sprites after each boost.
    pub flash: Option<Pulse>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodySpec {
    pub kind: BodyKind,
    /// Mass per area (1 for a C# entity; more for heavy boxes).
    pub density: f32,
    pub lock_x: bool,
    pub lock_y: bool,
    pub lock_rotation: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    /// Moves when pushed.
    Dynamic,
    /// Never moves.
    Static,
    /// Never moves and does not block; reports contacts.
    Sensor,
}

impl BodySpec {
    /// A pushable body with the map's axis locks; all three locks fix it in
    /// place (`Entity.SetRestrictions`).
    fn pushable(e: &EntityData, density: f32) -> BodySpec {
        let fixed = e.restrict_x && e.restrict_y && e.restrict_rotation;
        BodySpec {
            kind: if fixed { BodyKind::Static } else { BodyKind::Dynamic },
            density,
            lock_x: e.restrict_x,
            lock_y: e.restrict_y,
            lock_rotation: e.restrict_rotation,
        }
    }
}

/// The template for a map entity, or `None` for types not ported yet.
pub fn template_for(e: &EntityData) -> Option<EntityTemplate> {
    let map_sprite = |layer| SpriteSpec::new(e.sprite.clone(), e.scale, layer);
    let plain = |sprite| EntityTemplate {
        sprites: vec![sprite],
        body: None,
        boost: None,
        pickup: false,
        spin: None,
        casts_shadow: false,
    };
    let template = match e.kind {
        EntityType::Tile => plain(map_sprite(Layer::GroundDecor)),
        EntityType::Coin => EntityTemplate {
            pickup: true,
            spin: Some(GEAR_SPIN),
            casts_shadow: true,
            ..plain(map_sprite(Layer::Coin))
        },
        // Towing (NonLinkable), bullets passing (IgnoreWeapon) and blowing
        // up (Damagable) come with gadgets and weapons.
        EntityType::Box | EntityType::NonLinkableBox => EntityTemplate {
            body: Some(BodySpec::pushable(e, 1.0)),
            casts_shadow: true,
            ..plain(map_sprite(Layer::Prop))
        },
        EntityType::IgnoreWeaponBox => EntityTemplate {
            body: Some(BodySpec::pushable(e, 1.0)),
            ..plain(map_sprite(Layer::Prop))
        },
        // `EntitiesManager.CreateDamagableBox`: its own sprite, scale 1.
        EntityType::DamagableBox => EntityTemplate {
            body: Some(BodySpec::pushable(e, 1.0)),
            casts_shadow: true,
            ..plain(SpriteSpec::new("Entities\\DamagableBox", 1.0, Layer::Prop))
        },
        // As heavy as while nothing tows it.
        EntityType::HeavyBox => EntityTemplate {
            body: Some(BodySpec::pushable(e, HEAVY_BOX_DENSITY)),
            casts_shadow: true,
            ..plain(map_sprite(Layer::Prop))
        },
        EntityType::Accelerator => EntityTemplate {
            body: Some(BodySpec {
                kind: BodyKind::Sensor,
                density: 1.0,
                lock_x: false,
                lock_y: false,
                lock_rotation: false,
            }),
            boost: Some(BoostSpec {
                speed: ACCELERATOR_BOOST,
                flash: Some(Pulse {
                    start: WHITE,
                    end: GRAY,
                    duration: 1.0,
                }),
            }),
            sprites: accelerator_arrows(e.scale),
            casts_shadow: true,
            ..plain(SpriteSpec::new("", 0.0, Layer::Accelerator))
        },
        EntityType::Bomb | EntityType::CoinDynamic | EntityType::Unknown => return None,
    };
    template
        .sprites
        .iter()
        .all(|s| !s.name.is_empty())
        .then_some(template)
}

/// The outline a fixed entity's shadow is baked from, in map cells (y
/// down), or `None` if it casts none into the light: a body that never
/// moves, a shadow, and `TryCastStaticShadow` (`MapManager`, static
/// shadows for static objects). `size` is its body's size in cells.
pub fn static_shadow_outline(e: &EntityData, template: &EntityTemplate, size: [f32; 2]) -> Option<[[f32; 2]; 4]> {
    let fixed = template.body.is_some_and(|b| b.kind != BodyKind::Dynamic);
    if !(e.try_cast_static_shadow && template.casts_shadow && fixed) {
        return None;
    }
    // C# turned the body by `-DirectionDeg` in its y-down world.
    let (sin, cos) = (-e.direction_deg).to_radians().sin_cos();
    let (hx, hy) = (size[0] / 2.0, size[1] / 2.0);
    Some([(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)].map(|(x, y)| {
        [e.position.x + x * cos - y * sin, e.position.y + x * sin + y * cos]
    }))
}

/// The outlines of a map's fixed objects that cast shadows into the
/// light, sized by their first sprite as their bodies are; `sprite_size`
/// gives a sprite's size in pixels.
pub fn static_shadows(map: &MapData, sprite_size: impl Fn(&str) -> Option<[f32; 2]>) -> Vec<[[f32; 2]; 4]> {
    map.entities
        .iter()
        .filter_map(|e| {
            let template = template_for(e)?;
            let first = template.sprites.first()?;
            let [w, h] = sprite_size(&first.name)?;
            // Pixels × scale, in cells of `map.scale` pixels.
            let k = first.scale / map.scale;
            static_shadow_outline(e, &template, [w * k, h * k])
        })
        .collect()
}

/// `Accelerator.Draw`: the arrow twice, the second three quarters of its
/// length ahead, lit yellow in turn on the shared 1 s sawtooth (0–0.4 the
/// first, 0.4–0.8 the second, then neither).
fn accelerator_arrows(scale: f32) -> Vec<SpriteSpec> {
    let wave = Wave::Sawtooth { period: 1.0 };
    let arrow = |ahead, track| SpriteSpec {
        ahead,
        tint_loop: Some((wave, track)),
        ..SpriteSpec::new("Entities\\Accelerator", scale, Layer::Accelerator)
    };
    vec![
        arrow(0.0, ColorTrack::steps(&[(0.0, YELLOW), (0.4, GRAY)])),
        arrow(0.75, ColorTrack::steps(&[(0.0, GRAY), (0.4, YELLOW), (0.8, GRAY)])),
    ]
}
