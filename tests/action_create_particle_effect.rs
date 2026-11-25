//! `CreateParticleEffect` (Swarm's `CreateNewFriendEffect`): the zombie
//! border flares at an entity.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, CurrentActions, run};
use tilerider::game::MapId;
use tilerider::particles::styles::{NEW_ZOMBIE_FRIEND, TEXTURES};
use tilerider::render::particles::Particles;

mod common;

fn app() -> App {
    let (map, _) = common::load_map("Swarm");
    let mut app = App::new();
    app.add_plugins(ActionsPlugin)
        .init_resource::<Particles>()
        .insert_resource(CurrentActions(map.actions));
    app
}

fn count(app: &App) -> usize {
    app.world().resource::<Particles>().iter().count()
}

#[test]
fn swarm_has_the_action_and_its_texture_is_loaded() {
    let (map, _) = common::load_map("Swarm");
    assert!(map.actions.0.iter().any(|a| a.kind == "CreateParticleEffect"));
    assert!(TEXTURES.contains(&NEW_ZOMBIE_FRIEND.texture));
}

#[test]
fn it_flares_the_border_on_the_entity_that_set_it_off_turned_as_it_faces() {
    let mut app = app();
    let at = Vec3::new(12.0, 7.0, 0.0);
    app.world_mut().spawn((
        MapId(3001),
        Transform::from_translation(at).with_rotation(Quat::from_rotation_z(1.25)),
        GlobalTransform::from(Transform::from_translation(at).with_rotation(Quat::from_rotation_z(1.25))),
    ));
    // `ItemId` 0 in the map: the actor's.
    assert!(run(app.world_mut(), "CreateNewFriendEffect", Some(3001), 0));
    let particles = app.world().resource::<Particles>();
    let p = particles.iter().next().expect("one particle");
    assert_eq!(count(&app), 1);
    assert_eq!(p.position, Vec2::new(12.0, 7.0));
    assert!((p.view_direction - 1.25).abs() < 1e-4, "{}", p.view_direction);
    assert!(std::ptr::eq(p.style, &NEW_ZOMBIE_FRIEND));
    assert_eq!(p.style.lifetime, 0.3);
    assert!(p.style.additive);
}

#[test]
fn it_succeeds_without_an_entity_so_the_next_action_runs_but_draws_nothing() {
    let mut app = app();
    assert!(run(app.world_mut(), "CreateNewFriendEffect", Some(9999), 0));
    assert!(run(app.world_mut(), "CreateNewFriendEffect", None, 0));
    assert_eq!(count(&app), 0);
}

#[test]
fn the_effect_swells_and_is_gone_in_three_tenths_of_a_second() {
    let s = &NEW_ZOMBIE_FRIEND;
    assert_eq!(s.state_at(0.0).scale, Vec2::splat(0.65));
    assert_eq!(s.state_at(0.3).scale, Vec2::splat(12.0));
    assert_eq!(s.state_at(0.15).color[3], 0.5, "fades out");
}
