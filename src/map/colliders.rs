//! Static wall colliders: wall cells merged into rectangles
//! (`PhysicsManager.GetGeomsFromDistanceField`), slippery and dead
//! (`ConfigureWall`: friction 0.01, no bounce).

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::game::{GameState, LoadedMap, MapEntity};

pub struct WallCollidersPlugin;

impl Plugin for WallCollidersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_wall_colliders);
    }
}

fn spawn_wall_colliders(mut commands: Commands, map: Res<LoadedMap>) {
    let cell = map.cell.0;
    let rects = map.grid.wall_rects();
    let shapes: Vec<(Vec2, Rotation, Collider)> = rects
        .iter()
        .map(|&(x, y, w, h)| {
            let center = map.world(x as f32 + w as f32 / 2.0, y as f32 + h as f32 / 2.0);
            (
                center,
                Rotation::IDENTITY,
                Collider::rectangle(w as f32 * cell, h as f32 * cell),
            )
        })
        .collect();
    // The map's edge is solid too: a frame one cell thick around it.
    let (w, h) = (map.grid.width as f32 * cell, map.grid.height as f32 * cell);
    let frame = [
        (Vec2::new(w / 2.0, cell / 2.0), Vec2::new(w + 2.0 * cell, cell)),
        (Vec2::new(w / 2.0, -h - cell / 2.0), Vec2::new(w + 2.0 * cell, cell)),
        (Vec2::new(-cell / 2.0, -h / 2.0), Vec2::new(cell, h)),
        (Vec2::new(w + cell / 2.0, -h / 2.0), Vec2::new(cell, h)),
    ];
    let mut shapes = shapes;
    shapes.extend(
        frame
            .into_iter()
            .map(|(c, s)| (c, Rotation::IDENTITY, Collider::rectangle(s.x, s.y))),
    );
    commands.spawn((
        Name::new("Walls"),
        MapEntity,
        RigidBody::Static,
        Collider::compound(shapes),
        Friction::new(0.01),
        Restitution::new(0.0),
        Transform::default(),
    ));
    info!("{} wall colliders", rects.len());
}
