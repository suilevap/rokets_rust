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

use crate::content::map::{EntityData, EntityType};
use crate::render::layers::Layer;

/// `HeavyBox._coefficientUnlinked`.
const HEAVY_BOX_DENSITY: f32 = 64.0;
/// `Accelerator.OnCollide`: impulse `30 × mass`, i.e. 30 units/s.
const ACCELERATOR_BOOST: f32 = 30.0;
/// Radians per second gears turn.
const GEAR_SPIN: f32 = 1.5;

/// How a map entity is built.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityTemplate {
    pub sprite: SpriteSpec,
    /// A physics body, if it has one.
    pub body: Option<BodySpec>,
    /// Throws bodies that touch it along its facing, at this speed.
    pub boost: Option<f32>,
    /// Drawn as two arrows that light in turn (the second three quarters
    /// of a sprite ahead), flashing after a boost.
    pub pad_arrows: bool,
    /// The player picks it up by driving into it (gears).
    pub pickup: bool,
    /// Turns on the spot, radians per second.
    pub spin: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpriteSpec {
    /// Content name under `Sprites/`.
    pub name: String,
    pub scale: f32,
    pub layer: Layer,
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
    let map_sprite = |layer| SpriteSpec {
        name: e.sprite.clone(),
        scale: e.scale,
        layer,
    };
    let plain = |sprite| EntityTemplate {
        sprite,
        body: None,
        boost: None,
        pad_arrows: false,
        pickup: false,
        spin: None,
    };
    let template = match e.kind {
        EntityType::Tile => plain(map_sprite(Layer::GroundDecor)),
        EntityType::Coin => EntityTemplate {
            pickup: true,
            spin: Some(GEAR_SPIN),
            ..plain(map_sprite(Layer::Coin))
        },
        // Towing (NonLinkable), bullets passing (IgnoreWeapon) and blowing
        // up (Damagable) come with gadgets and weapons.
        EntityType::Box | EntityType::NonLinkableBox | EntityType::IgnoreWeaponBox => EntityTemplate {
            body: Some(BodySpec::pushable(e, 1.0)),
            ..plain(map_sprite(Layer::Prop))
        },
        // `EntitiesManager.CreateDamagableBox`: its own sprite, scale 1.
        EntityType::DamagableBox => EntityTemplate {
            body: Some(BodySpec::pushable(e, 1.0)),
            ..plain(SpriteSpec {
                name: "Entities\\DamagableBox".into(),
                scale: 1.0,
                layer: Layer::Prop,
            })
        },
        // As heavy as while nothing tows it.
        EntityType::HeavyBox => EntityTemplate {
            body: Some(BodySpec::pushable(e, HEAVY_BOX_DENSITY)),
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
            boost: Some(ACCELERATOR_BOOST),
            pad_arrows: true,
            ..plain(SpriteSpec {
                name: "Entities\\Accelerator".into(),
                scale: e.scale,
                layer: Layer::Accelerator,
            })
        },
        EntityType::Bomb | EntityType::CoinDynamic | EntityType::Unknown => return None,
    };
    (!template.sprite.name.is_empty()).then_some(template)
}
