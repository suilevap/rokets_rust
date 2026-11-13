//! Game flow: load a map, play it, go to the next one.
//!
//! `Loading` reads the map XML, the car catalogue and the campaign list,
//! then the map image, builds the grid and bakes the light, and inserts
//! [`LoadedMap`]. `Playing` spawns the world from it; every spawned entity
//! carries [`MapEntity`] and is despawned when the map is left (restart,
//! next map).

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageLoaderSettings;
use bevy::prelude::*;

use crate::assets::{self, XmlText};
use crate::content::cars::CarCatalog;
use crate::content::map::MapData;
use crate::content::map_list::MapList;
use crate::light::bake::LightMap;
use crate::light::raster::Canvas;
use crate::map::grid::MapGrid;
use crate::render::sprite::{SpriteInfo, SpriteSet};
use crate::units::CellSize;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Loading,
    Playing,
    /// Loading failed; the message is logged and shown.
    Failed,
}

/// Which map to play; changed to go to another map.
#[derive(Resource, Debug, Clone)]
pub struct CurrentMapName(pub String);

/// Everything the world is spawned from.
#[derive(Resource)]
pub struct LoadedMap {
    pub data: MapData,
    pub grid: MapGrid,
    pub light: LightMap,
    pub cell: CellSize,
    pub cars: CarCatalog,
    pub list: MapList,
    pub sprites: SpriteSet,
}

impl LoadedMap {
    /// A point in map cells to the world.
    pub fn world(&self, x: f32, y: f32) -> Vec2 {
        self.cell.to_world(x, y)
    }

    /// The light layer at a world point (what sprites are multiplied by).
    pub fn light_at(&self, p: Vec2) -> [f32; 3] {
        let (x, y) = self.cell.to_cells(p);
        self.light.sample_layer(x, y)
    }
}

/// Despawned when the map is left.
#[derive(Component, Default)]
pub struct MapEntity;

/// Sent to leave the map: restart it, or go to another.
#[derive(Message, Debug, Clone)]
pub enum ChangeMap {
    Restart,
    Next,
}

#[derive(Resource, Default)]
struct Pending {
    map: Handle<XmlText>,
    cars: Handle<XmlText>,
    list: Handle<XmlText>,
    parsed: Option<(MapData, CarCatalog, MapList)>,
    image: Option<Handle<Image>>,
    /// The lamp sprite the light is baked with (`Particles\flash2`).
    flash: Handle<Image>,
    sprites: Vec<(String, Handle<Image>)>,
}


/// The sprite names a map needs: its player car's parts and its drawn entities.
fn sprites_for(map: &MapData, cars: &CarCatalog) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for player in map.players.iter() {
        if let Some(car) = cars.get(&player.car_name) {
            names.extend(car.base_sprite());
            for part in ["Body", "Wheel"] {
                names.extend(car.part_sprite(part));
            }
        }
    }
    let mut skipped: std::collections::BTreeMap<_, usize> = Default::default();
    for e in map.entities.iter() {
        match crate::map::templates::template_for(e) {
            Some(template) => names.extend(template.sprites.into_iter().map(|s| s.name)),
            None => *skipped.entry(e.kind).or_default() += 1,
        }
    }
    if !skipped.is_empty() {
        info!("entities not supported yet (skipped): {skipped:?}");
    }
    names.sort_by_key(|n| n.to_ascii_lowercase());
    names.dedup_by_key(|n| n.to_ascii_lowercase());
    names
}

#[derive(Resource, Debug, Clone)]
pub struct LoadError(pub String);

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_message::<ChangeMap>()
            .add_systems(OnEnter(GameState::Loading), start_loading)
            .add_systems(Update, poll_loading.run_if(in_state(GameState::Loading)))
            .add_systems(OnExit(GameState::Playing), despawn_map)
            .add_systems(Update, change_map.run_if(in_state(GameState::Playing)))
            .add_systems(OnEnter(GameState::Failed), report_failure);
    }
}

fn fail(commands: &mut Commands, next: &mut NextState<GameState>, msg: String) {
    error!("{msg}");
    commands.insert_resource(LoadError(msg));
    next.set(GameState::Failed);
}

fn start_loading(
    mut commands: Commands,
    server: Res<AssetServer>,
    name: Res<CurrentMapName>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(map_path) = assets::map_path(&name.0) else {
        fail(&mut commands, &mut next, format!("no map named {:?} in assets/Maps/Singleplayer", name.0));
        return;
    };
    info!("loading map {}", map_path);
    commands.insert_resource(Pending {
        map: server.load(map_path),
        cars: server.load("Cars/Cars.xml"),
        list: server.load("Maps/MapList.xml"),
        flash: load_cpu_image(&server, "Sprites/Particles/flash2.png"),
        ..default()
    });
}

fn poll_loading(
    mut commands: Commands,
    server: Res<AssetServer>,
    texts: Res<Assets<XmlText>>,
    images: Res<Assets<Image>>,
    pending: Option<ResMut<Pending>>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    if pending.parsed.is_none() {
        let (Some(map), Some(cars), Some(list)) = (
            texts.get(&pending.map),
            texts.get(&pending.cars),
            texts.get(&pending.list),
        ) else {
            for handle in [&pending.map, &pending.cars, &pending.list] {
                if let Some(bevy::asset::LoadState::Failed(e)) = server.get_load_state(handle) {
                    fail(&mut commands, &mut next, format!("could not load content: {e}"));
                    commands.remove_resource::<Pending>();
                }
            }
            return;
        };
        let parsed = (|| -> Result<_, String> {
            let map = MapData::parse(&map.0).map_err(|e| format!("map: {e}"))?;
            let cars = CarCatalog::parse(&cars.0).map_err(|e| format!("Cars.xml: {e}"))?;
            let list = MapList::parse(&list.0).map_err(|e| format!("MapList.xml: {e}"))?;
            Ok((map, cars, list))
        })();
        match parsed {
            Ok(p) => {
                if let Some(image) = p.0.image.as_deref().filter(|s| !s.trim().is_empty()) {
                    match assets::sprite_path(image) {
                        Some(path) => {
                            pending.image = Some(load_cpu_image(&server, path));
                        }
                        None => warn!("map image {image:?} not found; using the CharMap only"),
                    }
                }
                for name in sprites_for(&p.0, &p.1) {
                    match assets::sprite_path(&name) {
                        Some(path) => {
                            let handle = server.load(path);
                            pending.sprites.push((name, handle));
                        }
                        None => warn!("sprite {name:?} not found"),
                    }
                }
                pending.parsed = Some(p);
            }
            Err(e) => {
                fail(&mut commands, &mut next, e);
                commands.remove_resource::<Pending>();
                return;
            }
        }
    }

    let image = match &pending.image {
        Some(handle) => match images.get(handle) {
            Some(image) => Some(image),
            None => {
                if let Some(bevy::asset::LoadState::Failed(e)) = server.get_load_state(handle) {
                    fail(&mut commands, &mut next, format!("could not load the map image: {e}"));
                    commands.remove_resource::<Pending>();
                }
                return;
            }
        },
        None => None,
    };

    let Some(flash) = images.get(&pending.flash) else {
        if let Some(bevy::asset::LoadState::Failed(e)) = server.get_load_state(&pending.flash) {
            fail(&mut commands, &mut next, format!("could not load the lamp sprite: {e}"));
            commands.remove_resource::<Pending>();
        }
        return;
    };
    let Some(flash) = rgba8(flash).map(|(w, h, data)| Canvas::from_rgba8(w, h, &data)) else {
        fail(&mut commands, &mut next, "the lamp sprite is not 8-bit RGBA".to_string());
        commands.remove_resource::<Pending>();
        return;
    };

    let mut sprites = SpriteSet::default();
    for (name, handle) in &pending.sprites {
        match images.get(handle) {
            Some(image) => sprites.insert(
                name,
                SpriteInfo {
                    image: handle.clone(),
                    size: image.size().as_vec2(),
                },
            ),
            None => match server.get_load_state(handle) {
                Some(bevy::asset::LoadState::Failed(e)) => warn!("sprite {name:?}: {e}"),
                _ => return,
            },
        }
    }

    let (data, cars, list) = pending.parsed.take().expect("parsed above");
    let pixels = image.and_then(rgba8);
    let grid = MapGrid::build(&data, pixels.as_ref().map(|(w, h, p)| (*w, *h, p.as_slice())));
    let objects = crate::map::templates::static_shadows(&data, |name| sprites.get(name).map(|i| i.size.into()));
    let light = LightMap::bake(&data, &grid, &flash, &objects);
    let cell = CellSize::from_map_scale(data.scale);
    info!(
        "map {} ready: {}x{} cells, {} lamps",
        data.name, grid.width, grid.height, light.lamps.len()
    );
    commands.insert_resource(LoadedMap {
        data,
        grid,
        light,
        cell,
        cars,
        list,
        sprites,
    });
    commands.remove_resource::<Pending>();
    next.set(GameState::Playing);
}

/// Loads an image keeping its pixels on the CPU too.
fn load_cpu_image(server: &AssetServer, path: impl Into<bevy::asset::AssetPath<'static>>) -> Handle<Image> {
    server
        .load_builder()
        .with_settings(|s: &mut ImageLoaderSettings| {
            s.asset_usage = RenderAssetUsages::all();
        })
        .load(path)
}

/// An image's pixels as `(width, height, RGBA bytes)`; PNGs load as 8-bit
/// RGBA, anything else is `None`.
fn rgba8(img: &Image) -> Option<(usize, usize, Vec<u8>)> {
    let size = img.size();
    let data = img.data.as_ref()?;
    (data.len() == (size.x * size.y * 4) as usize).then(|| (size.x as usize, size.y as usize, data.clone()))
}

fn despawn_map(mut commands: Commands, entities: Query<Entity, With<MapEntity>>) {
    for e in &entities {
        commands.entity(e).despawn();
    }
}

fn change_map(
    mut messages: MessageReader<ChangeMap>,
    map: Res<LoadedMap>,
    mut name: ResMut<CurrentMapName>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(msg) = messages.read().last() else {
        return;
    };
    if let ChangeMap::Next = msg {
        match map.list.next_after(&name.0) {
            Some(next_map) => name.0 = next_map.to_string(),
            None => info!("{} is the last map; restarting it", name.0),
        }
    }
    next.set(GameState::Loading);
}

fn report_failure(error: Option<Res<LoadError>>, mut commands: Commands) {
    let text = error.map_or_else(|| "loading failed".to_string(), |e| e.0.clone());
    commands.spawn((
        Text::new(text),
        Node {
            position_type: PositionType::Absolute,
            top: px(16.0),
            left: px(16.0),
            ..default()
        },
    ));
}

/// `bevy::ui::px` without importing it everywhere.
fn px(v: f32) -> Val {
    Val::Px(v)
}
