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
use crate::light::raster::Mask;
use crate::map::grid::MapGrid;
use crate::render::sprite::{SpriteInfo, SpriteSet};
use crate::units::CellSize;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Loading,
    Playing,
    /// The map menu (`menu`): the maps to choose from; no map is loaded.
    Menu,
    /// Loading failed; the message is logged and shown.
    Failed,
}

/// Which map to play; changed to go to another map.
#[derive(Resource, Debug, Clone)]
pub struct CurrentMapName(pub String);

/// Present at the start when the game opens in the map menu instead of a
/// map (`menu::starts_in_menu`); taken away on the way in.
#[derive(Resource, Debug, Clone, Copy)]
pub struct StartInMenu;

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

    /// `MapManager.LineCollide` between two world points: a wall in the
    /// way (what the AI's sight is).
    pub fn line_collides(&self, a: Vec2, b: Vec2) -> bool {
        self.grid.line_collides(self.cell.0, [a.x, -a.y], [b.x, -b.y])
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

/// The id the map gives an entity (`CustomId`), by which actions name it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MapId(pub i64);

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
    /// The lamp sprite the light is baked with (`Particles/flash2`).
    flash: Handle<Image>,
    sprites: Vec<(String, Handle<Image>)>,
}


/// The sprite names a map needs: its player car's parts and its drawn entities.
fn sprites_for(map: &MapData, cars: &CarCatalog, registry: &crate::actions::ActionRegistry) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for player in map.players.iter() {
        if let Some(car) = cars.get(&player.car_name) {
            names.extend(car.base_sprite());
            for part in ["Body", "Wheel"] {
                names.extend(car.view_part_sprite(part));
            }
            for part in ["Destroyed1", "Destroyed2"] {
                names.extend(car.part_sprite(part));
            }
            if let Some((kind, _)) = car.weapon.as_deref().and_then(|w| crate::weapon::model::for_type(w, true)) {
                names.extend(crate::weapon::textures_of(kind).into_iter().map(String::from));
                if crate::weapon::has_aim_line(kind) {
                    names.extend(crate::weapon::AIM_LINE_TEXTURES.map(String::from));
                }
            }
            match car.gadget.as_deref() {
                Some("Link") => names.extend(crate::gadget::TEXTURES.map(String::from)),
                Some("Accelerator") => names.extend(crate::gadget::accelerator::TEXTURES.map(String::from)),
                _ => {}
            }
        }
    }
    // The simple cars (prototypes too, for the clones): the chassis, the
    // wreck, the guns (the non-player preset) and the health bar's pixel.
    for sc in map.simple_cars.iter() {
        let Some(car) = cars.get_simple(&sc.car_name) else {
            continue;
        };
        names.extend(car.base_sprite());
        for part in ["Destroyed1", "Destroyed2"] {
            names.extend(car.part_sprite(part));
        }
        let weapon = crate::car::weapon_type(sc.weapon_override(), car.weapon.as_deref());
        if let Some((kind, _)) = weapon.and_then(|w| crate::weapon::model::for_type(w, false)) {
            names.extend(crate::weapon::textures_of(kind).into_iter().map(String::from));
            // An enemy's aim line (`LaserVisiblityUpdate`).
            if crate::weapon::has_aim_line(kind) {
                names.extend(crate::weapon::AIM_LINE_TEXTURES.map(String::from));
            }
        }
        names.push("WhitePixel".to_string());
    }
    // What the map's actions draw (door hints, …).
    for a in &map.actions.0 {
        names.extend(registry.sprites_of(a));
    }
    // … and what the items run when they appear or are destroyed.
    let on_create_destroy = map
        .players
        .iter()
        .map(|p| (&p.action_on_create, &p.action_on_destroy))
        .chain(map.simple_cars.iter().map(|c| (&c.action_on_create, &c.action_on_destroy)))
        .chain(map.entities.iter().map(|e| (&e.action_on_create, &e.action_on_destroy)));
    for (create, destroy) in on_create_destroy {
        for text in [create, destroy].into_iter().flatten() {
            names.extend(registry.sprites_of_text(text));
        }
    }
    // The hints' pictures and the speck of the burst they leave.
    names.extend(crate::hint::sprites_of(&map.hints.items));
    // Shadows of cars and loose objects (`render/shadow`).
    names.extend(["Gradient4Pixel", "Gradient4PixelUniform"].map(String::from));
    names.extend(crate::particles::styles::TEXTURES.map(String::from));
    // The snow maps' falling snow, mist and frozen glass, every map's speed lines.
    names.extend(crate::render::snow::textures(map.level_effect).into_iter().map(String::from));
    names.extend(crate::assist::view::textures().map(String::from));
    names.extend(crate::car::speed_trail::textures().into_iter().map(String::from));
    names.extend(crate::render::frozen_glass::textures(map.level_effect).into_iter().map(String::from));
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
    names.sort();
    names.dedup();
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
    start_in_menu: Option<Res<StartInMenu>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if start_in_menu.is_some() {
        commands.remove_resource::<StartInMenu>();
        next.set(GameState::Menu);
        return;
    }
    let map_path = assets::map_path(&name.0);
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
    (pending, registry, car): (Option<ResMut<Pending>>, Res<crate::actions::ActionRegistry>, Option<Res<crate::car::CarOverride>>),
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
            Ok(mut p) => {
                if let Some(car) = car.as_deref() {
                    if p.1.get(&car.0).is_some() || p.1.get_simple(&car.0).is_some() {
                        crate::car::override_local_car(&mut p.0, &car.0);
                    } else {
                        warn!("--car {:?} is not in Cars.xml", car.0);
                    }
                }
                if let Some(image) = p.0.image.as_deref().filter(|s| !s.trim().is_empty()) {
                    pending.image = Some(load_cpu_image(&server, assets::sprite_path(image)));
                }
                for name in sprites_for(&p.0, &p.1, &registry) {
                    let handle = server.load(assets::sprite_path(&name));
                    pending.sprites.push((name, handle));
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
    let Some(flash) = rgba8(flash).map(|(w, h, data)| Mask::from_alpha(w, h, &data)) else {
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
    // `_navigation.BuildFromDistanceField`, once the map is ready.
    commands.insert_resource(crate::ai::Navigation::build(&grid, cell));
    commands.insert_resource(crate::ai::SharedTerrain::build(&grid, cell));
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
