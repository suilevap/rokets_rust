//! `GameLibrary.CarsData`: `assets/Cars/Cars.xml`, the car catalogue.
//!
//! A car may name a `BaseCar`; it then takes everything from the base and
//! only its own weapon and gadget (as `CarsData.MergeCar` does in C#).

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
}

/// All cars by name, with `BaseCar` already resolved.
#[derive(Clone, Debug, Default)]
pub struct CarCatalog {
    cars: HashMap<String, CarData>,
}

impl CarCatalog {
    pub fn parse(xml: &str) -> Result<Self, ContentError> {
        let raw: CarsXml = parse_asset(xml)?;
        let by_name: HashMap<String, CarData> = raw
            .cars
            .items
            .into_iter()
            .map(|c| (c.name.clone(), c))
            .collect();
        let mut cars = HashMap::new();
        for (name, car) in &by_name {
            let mut resolved = car.clone();
            if let Some(base_name) = &car.base_car
                && let Some(base) = by_name.get(base_name)
            {
                resolved = base.clone();
                resolved.name = car.name.clone();
                resolved.base_car = car.base_car.clone();
                resolved.weapon = car.weapon.clone();
                resolved.weapon_positon = car.weapon_positon;
                resolved.gadget = car.gadget.clone();
                resolved.gadget_position = car.gadget_position;
            }
            cars.insert(name.clone(), resolved);
        }
        Ok(CarCatalog { cars })
    }

    pub fn get(&self, name: &str) -> Option<&CarData> {
        self.cars.get(name)
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
            .or_else(|| self.sprites_path.as_ref().map(|p| format!("{p}\\Base")))
    }

    pub fn part_sprite(&self, part: &str) -> Option<String> {
        self.sprites_path.as_ref().map(|p| format!("{p}\\{part}"))
    }
}
