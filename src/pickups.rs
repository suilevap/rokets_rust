//! Map entities drawn so far: decor tiles and coins (gears).
//!
//! - `Tile`: a flat sprite on the ground (finish markings, signs, grass).
//! - `Coin`: a gear the player picks up by driving into it; it spins. The
//!   count is shown in the debug text until the HUD exists.
//!
//! Boxes and accelerators are in `props.rs`; types not handled anywhere are
//! reported once per map when it loads.

use bevy::prelude::*;

use crate::content::map::EntityType;
use crate::controls::LocalPlayer;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::layers::Layer;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::units::map_degrees_to_world;

/// Radians per second.
const COIN_SPIN: f32 = 1.5;

#[derive(Component, Debug)]
pub struct Coin {
    /// Pick-up distance from the car's centre, in world units.
    pub radius: f32,
}

/// Coins picked up on this map.
#[derive(Resource, Debug, Default)]
pub struct CoinCount {
    pub collected: u32,
    pub total: u32,
}

#[derive(Message, Debug)]
pub struct CoinCollected;

/// Whether this module spawns entities of this type.
pub fn handles(kind: EntityType) -> bool {
    matches!(kind, EntityType::Coin | EntityType::Tile)
}

pub struct PickupsPlugin;

impl Plugin for PickupsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CoinCount>()
            .add_message::<CoinCollected>()
            .add_systems(OnEnter(GameState::Playing), spawn_entities)
            .add_systems(Update, (spin_coins, collect_coins).run_if(in_state(GameState::Playing)));
    }
}

fn spawn_entities(
    mut commands: Commands,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut coins: ResMut<CoinCount>,
) {
    *coins = CoinCount::default();
    let lit = SpriteTint {
        color: [1.0; 4],
        lit: true,
    };
    for e in map.data.entities.iter() {
        let layer = match e.kind {
            EntityType::Coin => Layer::Coin,
            EntityType::Tile => Layer::GroundDecor,
            _ => continue,
        };
        let Some(info) = map.sprites.get(&e.sprite) else {
            continue;
        };
        let position = map.world(e.position.x, e.position.y);
        let angle = map_degrees_to_world(e.direction_deg);
        let mut entity = commands.spawn((
            Name::new(format!("{:?} {}", e.kind, e.id)),
            MapEntity,
            Transform::from_translation(position.extend(0.0)).with_rotation(Quat::from_rotation_z(angle)),
            Visibility::default(),
        ));
        let size = info.size * crate::units::px(1.0) * e.scale;
        entity.with_children(|p| {
            p.spawn(sprite_bundle(info, e.scale, layer, lit, &quad, &mut materials));
        });
        if e.kind == EntityType::Coin {
            coins.total += 1;
            entity.insert(Coin {
                radius: size.max_element() / 2.0,
            });
        }
    }
}

fn spin_coins(time: Res<Time>, mut coins: Query<&mut Transform, With<Coin>>) {
    for mut t in &mut coins {
        t.rotate_z(COIN_SPIN * time.delta_secs());
    }
}

fn collect_coins(
    mut commands: Commands,
    players: Query<&Transform, (With<LocalPlayer>, Without<Coin>)>,
    coins: Query<(Entity, &Transform, &Coin)>,
    mut count: ResMut<CoinCount>,
    mut collected: MessageWriter<CoinCollected>,
) {
    for player in &players {
        let p = player.translation.truncate();
        for (entity, t, coin) in &coins {
            // The car's size is about one cell; half of it plus the coin's.
            if t.translation.truncate().distance(p) < coin.radius + 1.0 {
                commands.entity(entity).despawn();
                count.collected += 1;
                collected.write(CoinCollected);
            }
        }
    }
}
