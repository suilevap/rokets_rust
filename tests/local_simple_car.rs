//! Which car the player drives: a `Players` car with the `Local` control,
//! else a `SimpleCars` one (`car::local_simple_car`).

mod common;

use tilerider::car::local_simple_car;

#[test]
fn a_local_simple_car_is_the_players_when_no_players_car_is_local() {
    // Swarm's `Players` are two AttackAi buggies; its zombie 3001 is `Local`.
    let (map, _) = common::load_map("Swarm");
    assert_eq!(local_simple_car(&map).map(|c| c.id), Some(3001));
    // DoorsDream1 has no `Players` at all.
    let (map, _) = common::load_map("DoorsDream1");
    assert!(local_simple_car(&map).is_some());
}

#[test]
fn a_local_players_car_wins_over_any_simple_car() {
    let (mut map, _) = common::load_map("Swarm");
    map.players.items[0].control = "Local".into();
    assert_eq!(local_simple_car(&map).map(|c| c.id), None);
}

#[test]
fn maps_without_a_local_simple_car_have_none() {
    let (map, _) = common::load_map("Arena");
    assert_eq!(local_simple_car(&map).map(|c| c.id), None);
}
