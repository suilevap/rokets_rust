//! The map's navigation graph as a Bevy resource, and the `SetPath` action.
//!
//! C# origin: `MapManager._navigation` (a `NavGraph` built when the map is
//! ready: `BuildFromDistanceField`), `MapManager.GetPath`, `SetAreaWeight`
//! (called by the `SetNavigationWeight` action, `ActionRunner.cs`) and
//! `ActionRunner.SetPathRun`. The algorithms are in [`crate::map::nav`]; this
//! is the part that knows the app.
//!
//! - [`Navigation`]: the graph, built from the grid by `game.rs` as the map
//!   is loaded (the distance field the graph is cut from is
//!   [`MapGrid::chamfer_distance`](crate::map::grid::MapGrid::chamfer_distance),
//!   the one the light and the walls use: C#'s `CreateDistanceField`). It is
//!   shared (an `Arc` around a lock): the trees' blackboards ask it for paths
//!   from inside a tick, doors close and open links in it and an area's
//!   weight can change.
//! - `ChangeAreaWeight(area, weight)` (`ActionRunner.ChangeAreaWeightRun`,
//!   `MapManager.SetAreaWeight`): every node in the rectangle (map cells, y
//!   down; its edges count) takes the weight, which multiplies the length of
//!   the links it leaves and of the way to and from it, so searches avoid (or
//!   prefer) the area. A path already made is not changed.
//! - `SetPath(item, path, closed, type)`: gives the car's tree a path to
//!   follow (`Path`, what `MoveBase` follows) or to patrol (`PatrolPath`).
//!   The points are map cells, y down, turned into world points as
//!   everything else on the map.
//!
//! `--nav` draws the graph (`NavGraph.Draw`, under `DEBUG && AI`): the links
//! between nodes (green; red from the nodes whose weight is above 1, the
//! more the redder, as `NavNode.Draw`'s `Color.Lerp(Green, Red, (Weight - 1) /
//! 4)`) and a tick across each node a car crosses along an axis
//! (`Horizontal`/`Vertical`: where the path takes its two points). It is drawn
//! once when the map starts (a door's links and a changed weight are not
//! redrawn).
//!
//! Not ported: the areas' rectangles of `BuildFromDistanceField`'s `DEBUG &&
//! AI` drawing, the path of each car (`AiBlackboard.Draw`: its `Path` and the
//! line to its target), and `DebugGetPath` (the `O` and `P` keys put a path's
//! ends at the cursor).

use std::sync::{Arc, PoisonError, RwLock};

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;

use crate::actions::{AddMapAction, MapIds, Run};
use crate::ai::AiBlackboard;
use crate::content::actions::defs::{ChangeAreaWeight, PathType, SetPath};
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::map::grid::MapGrid;
use crate::map::nav::Orientation;
use crate::render::layers::Layer;
use crate::render::lines::{LineMaterial, LineMeshBuilder};
use crate::map::nav::{NavGraph, NavPath};
use crate::units::CellSize;

/// The map's [`NavGraph`], shared.
#[derive(Resource, Clone, Debug)]
pub struct Navigation(Arc<RwLock<NavGraph>>);

impl Navigation {
    /// Wraps a graph.
    pub fn new(graph: NavGraph) -> Navigation {
        Navigation(Arc::new(RwLock::new(graph)))
    }

    /// The graph of a map (`_navigation.BuildFromDistanceField`).
    pub fn build(grid: &MapGrid, cell: CellSize) -> Navigation {
        Navigation::new(NavGraph::from_grid(grid, cell.0))
    }

    /// Reads the graph.
    pub fn read<R>(&self, f: impl FnOnce(&NavGraph) -> R) -> R {
        f(&self.0.read().unwrap_or_else(PoisonError::into_inner))
    }

    /// Changes the graph (a door's links, an area's weight).
    pub fn write<R>(&self, f: impl FnOnce(&mut NavGraph) -> R) -> R {
        f(&mut self.0.write().unwrap_or_else(PoisonError::into_inner))
    }

    /// `MapManager.GetPath`: the way from one world point to another.
    pub fn get_path(&self, start: Vec2, end: Vec2) -> Option<NavPath> {
        self.read(|graph| graph.get_path(start, end))
    }
}

/// Present when the graph is to be drawn (`--nav`).
#[derive(Resource, Default)]
pub struct NavDebug;

pub struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<SetPath, _>(set_path)
            .add_map_action::<ChangeAreaWeight, _>(change_area_weight)
            .add_systems(OnEnter(GameState::Playing), draw_graph.run_if(resource_exists::<NavDebug>));
    }
}

/// `NavGraph.Draw`: the links, and the nodes a path crosses straight.
fn draw_graph(
    navigation: Res<Navigation>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LineMaterial>>,
) {
    const WIDTH: f32 = 0.06;
    let z = Layer::Glow.z();
    let (mut links, mut dear, mut ticks) = (LineMeshBuilder::default(), LineMeshBuilder::default(), LineMeshBuilder::default());
    navigation.read(|graph| {
        let half = 0.25;
        for node in graph.nodes() {
            let target = if node.weight > 1.0 { &mut dear } else { &mut links };
            for link in &node.links {
                let to = graph.nodes()[link.to].pos;
                target.add_line(node.pos.extend(z), to.extend(z), WIDTH);
            }
            let along = match node.orientation {
                Orientation::Horizontal => Vec2::X,
                Orientation::Vertical => Vec2::Y,
                Orientation::None => continue,
            };
            ticks.add_line((node.pos - along * half).extend(z), (node.pos + along * half).extend(z), WIDTH * 2.0);
        }
    });
    for (lines, color) in [
        (links, LinearRgba::new(0.1, 0.9, 0.2, 0.6)),
        (dear, LinearRgba::new(0.9, 0.1, 0.1, 0.6)),
        (ticks, LinearRgba::new(1.0, 0.6, 0.1, 0.9)),
    ] {
        if lines.is_empty() {
            continue;
        }
        commands.spawn((
            Name::new("Navigation graph"),
            MapEntity,
            Mesh3d(meshes.add(lines.build())),
            MeshMaterial3d(materials.add(LineMaterial { color, ..default() })),
            NoFrustumCulling,
        ));
    }
}

/// `ActionRunner.SetPathRun`: the path goes to the car's behaviour tree, if it
/// has one; any other target is left as it was. `Type` says which: `Path`
/// (followed) or `Patrol`.
fn set_path(In(run): In<Run<SetPath>>, ids: MapIds, map: Res<LoadedMap>, mut brains: Query<&mut AiBlackboard>) -> bool {
    if let Some(e) = ids.entity(run.target(run.data.item_id))
        && let Ok(mut blackboard) = brains.get_mut(e)
    {
        let points = run.data.path.iter().map(|p| map.world(p.x, p.y)).collect();
        let path = NavPath::new(points, run.data.closed);
        match run.data.kind {
            PathType::Path => blackboard.path = Some(path),
            PathType::Patrol => blackboard.patrol_path = Some(path),
        }
    }
    true
}

/// `ActionRunner.ChangeAreaWeightRun`: `MapManager.SetAreaWeight(weight,
/// area)` with the rectangle's corners in the world (the map's rows go down).
fn change_area_weight(In(run): In<Run<ChangeAreaWeight>>, navigation: Res<Navigation>, map: Res<LoadedMap>) -> bool {
    let area = run.data.area;
    let top_left = map.world(area.x as f32, area.y as f32);
    let bottom_right = map.world((area.x + area.w) as f32, (area.y + area.h) as f32);
    navigation.write(|graph| graph.set_weight(run.data.weight, top_left.min(bottom_right), top_left.max(bottom_right)));
    true
}
