//! `GameLibrary.CarsData`: `assets/Cars/Cars.xml`, the car catalogue.
//!
//! A car may name a `BaseCar`; it then takes everything from the base and
//! only its own weapon and gadget (as `CarsData.MergeCar` does in C#; the
//! base's own base first, as `GetCarInfo` recurses).
//!
//! The file lists two catalogues: `Cars`, the players' cars, and
//! `SimpleCars`, the ones the map's `SimpleCars` name (targets, turrets,
//! enemies; `GetSimpleCarInfo`, which merges only the weapon and its
//! position, as `MergeSimpleCar`). They are the same data here
//! ([`CarData`]; a simple car has no wheels or gadget). A name's first
//! entry wins in each.
//!
//! Not ported: easy mode's `EasyModeMaxHealthDelta` and
//! `EasyModeShieldDelta` (C# adds them on phones only; `Settings.IsEasyMode`
//! is false for the desktop and the browser).

use std::collections::HashMap;

use serde::Deserialize;

use super::xna::{Items, XVec2, boolean};
use super::{ContentError, parse_asset};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CarData {
    pub name: String,
    #[serde(default)]
    pub base_car: Option<String>,
    /// Folder under `Sprites/` holding `Base`, `Body`, `Wheel`, `Border`.
    #[serde(default)]
    pub sprites_path: Option<String>,
    #[serde(default)]
    pub base_sprite_path: Option<String>,
    /// Top speed factor (C# multiplies the default max speed by `0.5 * Speed`).
    #[serde(default)]
    pub speed: f32,
    #[serde(default)]
    pub acceleration: f32,
    /// Mass; -1 means "from the collider's density".
    #[serde(default = "minus_one")]
    pub mass: f32,
    /// Turning acceleration factor.
    #[serde(default)]
    pub controllability: f32,
    #[serde(default)]
    pub max_health: i32,
    #[serde(default)]
    pub weapon: Option<String>,
    #[serde(default)]
    pub weapon_positon: Option<XVec2>,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default, deserialize_with = "boolean")]
    pub flying: bool,
    #[serde(default = "yes", deserialize_with = "boolean")]
    pub can_move: bool,
    /// The magnet may grab it (`CanLink`, `LinkablePhysicEntity.CanBeLinked`).
    #[serde(default, deserialize_with = "boolean")]
    pub can_link: bool,
    /// Its collider is this share of the sprite's size (`GeometryScale`).
    #[serde(default = "one")]
    pub geometry_scale: f32,
    /// A circle collider instead of a box (`UseCircleForGeometry`).
    #[serde(default, deserialize_with = "boolean")]
    pub use_circle_for_geometry: bool,
    /// Points of shield (`Shield`, a byte; `ShieldComponent`).
    #[serde(default)]
    pub shield: u8,
    /// Front-left wheel, in sprite pixels from the car's centre.
    #[serde(default)]
    pub left_wheel_position: Option<XVec2>,
    #[serde(default = "one")]
    pub body_elastic: f32,
    #[serde(default)]
    pub gadget: Option<String>,
    #[serde(default)]
    pub gadget_position: Option<XVec2>,
    #[serde(default = "yes", deserialize_with = "boolean")]
    pub load_wheel_body_color_views: bool,
    /// The engine's loop (`Sounds/<name>.wav`); empty for none.
    #[serde(default = "engine")]
    pub engine_sound: String,
    /// Octaves added to the tyres' squeal.
    #[serde(default)]
    pub wheels_sound_pitch: f32,
}

fn engine() -> String {
    "Engine".to_string()
}

fn minus_one() -> f32 {
    -1.0
}
fn one() -> f32 {
    1.0
}
fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CarsXml {
    #[serde(default)]
    cars: Items<CarData>,
    #[serde(default)]
    simple_cars: Items<CarData>,
}

/// All cars by name, with `BaseCar` already resolved.
#[derive(Clone, Debug, Default)]
pub struct CarCatalog {
    cars: HashMap<String, CarData>,
    simple_cars: HashMap<String, CarData>,
}

/// How deep a `BaseCar` chain may go (C# recursed without a limit; a
/// cycle would never end there).
const MAX_BASES: usize = 8;

/// `GetCarInfo` / `GetSimpleCarInfo`: the first entry of each name (a
/// duplicate is ignored, `FirstOrDefault`), its base merged in.
fn resolve_all(items: Vec<CarData>) -> HashMap<String, CarData> {
    let mut by_name: HashMap<String, CarData> = HashMap::new();
    for c in items {
        by_name.entry(c.name.clone()).or_insert(c);
    }
    let resolve = |car: &CarData| {
        let mut chain = vec![car];
        while let Some(base) = chain.last().and_then(|c| c.base_car.as_deref()).filter(|b| !b.is_empty()).and_then(|b| by_name.get(b)) {
            if chain.len() >= MAX_BASES {
                break;
            }
            chain.push(base);
        }
        // From the root out: each takes everything but its own weapon and
        // gadget from the one before.
        let mut resolved = chain.pop().expect("a car").clone();
        while let Some(next) = chain.pop() {
            resolved.name = next.name.clone();
            resolved.base_car = next.base_car.clone();
            resolved.weapon = next.weapon.clone();
            resolved.weapon_positon = next.weapon_positon;
            resolved.gadget = next.gadget.clone();
            resolved.gadget_position = next.gadget_position;
        }
        resolved
    };
    by_name.iter().map(|(name, car)| (name.clone(), resolve(car))).collect()
}

impl CarCatalog {
    pub fn parse(xml: &str) -> Result<Self, ContentError> {
        let raw: CarsXml = parse_asset(xml)?;
        // The first car of a name wins (`GetCarInfo`: `FirstOrDefault`):
        // `SandBuggyMagnetMinigun` is listed twice, with a Minigun first.
        Ok(CarCatalog {
            cars: resolve_all(raw.cars.items),
            simple_cars: resolve_all(raw.simple_cars.items),
        })
    }

    /// A player's car (`GetCarInfo`).
    pub fn get(&self, name: &str) -> Option<&CarData> {
        self.cars.get(name)
    }

    /// A car a map's `SimpleCars` names (`GetSimpleCarInfo`).
    pub fn get_simple(&self, name: &str) -> Option<&CarData> {
        self.simple_cars.get(name)
    }

    pub fn len(&self) -> usize {
        self.cars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cars.is_empty()
    }
}

impl CarData {
    /// The sprite the body collider and the chassis are drawn from.
    pub fn base_sprite(&self) -> Option<String> {
        self.base_sprite_path
            .clone()
            .or_else(|| self.sprites_path.as_ref().map(|p| format!("{p}/Base")))
    }

    /// The sprite of the `Wheel` or `Body` part drawn over the chassis, or
    /// `None` when the car has no such parts: `LoadWheelBodyColorViews` is
    /// false (`Car.LoadWheelBodyColorViews` skips loading them, as the
    /// turrets, which have only `Base`, `Border` and the wrecks).
    pub fn view_part_sprite(&self, part: &str) -> Option<String> {
        self.load_wheel_body_color_views.then(|| self.part_sprite(part)).flatten()
    }

    pub fn part_sprite(&self, part: &str) -> Option<String> {
        self.sprites_path.as_ref().map(|p| format!("{p}/{part}"))
    }
}
