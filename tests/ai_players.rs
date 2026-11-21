//! The map's other `Players` cars: which are the AI's, how loud they are,
//! what they run when destroyed (`car::ai_players`, `CarSounds`).

mod common;

use tilerider::car::{ai_players, local_player};
use tilerider::sound::CarSounds;

fn ids<'a>(players: impl Iterator<Item = &'a tilerider::content::map::PlayerData>) -> Vec<i64> {
    players.map(|p| p.id).collect()
}

#[test]
fn every_players_car_but_the_local_one_is_the_ais() {
    // Greenlight: the player and four AttackAiFriend cars.
    let (map, _) = common::load_map("Greenlight");
    assert_eq!(local_player(&map).map(|p| p.id), Some(4001));
    assert_eq!(ids(ai_players(&map)), [4002, 4003, 4004, 4005]);
    // Revenge: the `Local` car is the third; the `NoneLocal` first is not
    // the player's.
    let (map, _) = common::load_map("Revenge");
    assert_eq!(local_player(&map).map(|p| p.id), Some(4003));
    assert_eq!(ids(ai_players(&map)), [4001, 4002, 4004]);
}

#[test]
fn with_a_local_simple_car_every_players_car_is_the_ais() {
    // Swarm's player is its zombie 3001; the two AttackAi buggies are AI.
    let (map, _) = common::load_map("Swarm");
    assert_eq!(local_player(&map).map(|p| p.id), None);
    assert_eq!(ids(ai_players(&map)), [4001, 4002]);
}

#[test]
fn with_no_local_car_the_first_is_the_players_and_prototypes_are_not_made() {
    // Race: nobody is `Local`; the first (NoneFriend) stays the player's.
    let (map, _) = common::load_map("Race");
    assert_eq!(local_player(&map).map(|p| p.id), Some(4001));
    assert_eq!(ids(ai_players(&map)), [4002, 4003, 4004]);
    // TestLab's 4003 is a prototype, there to be cloned.
    let (map, _) = common::load_map("TestLab");
    assert_eq!(ids(ai_players(&map)), [4002]);
    assert!(map.players.items.iter().any(|p| p.id == 4003 && p.is_prototype));
}

#[test]
fn a_players_car_has_its_action_on_destroy() {
    let (map, _) = common::load_map("Swarm");
    let first = map.players.items.iter().find(|p| p.id == 4001).unwrap();
    assert_eq!(first.action_on_destroy.as_deref(), Some("CaptureScreen(0,0)"));
    let (map, _) = common::load_map("Revenge");
    let boss = map.players.items.iter().find(|p| p.id == 4004).unwrap();
    assert_eq!(boss.action_on_destroy.as_deref(), Some("BossDestroy"));
}

#[test]
fn an_ais_engine_is_at_half_and_its_tyres_at_seven_tenths() {
    let local = CarSounds::new(Some("Engine"), 0.0).loudness(true);
    assert_eq!(local.loudness_factors(), (1.0, 1.0));
    let mut ai = CarSounds::new(Some("Engine"), 0.0).loudness(false);
    assert_eq!(ai.loudness_factors(), (0.5, 0.7));
    // `SetControl(id,Local)` makes it as loud as the player's.
    ai.set_loudness(true);
    assert_eq!(ai.loudness_factors(), (1.0, 1.0));
}
