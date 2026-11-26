//! Hints: the map's `<Hints>` as data, and what they do over time (switched
//! on by the player's place or by an action, flicker, blink, countdown,
//! fade out, frames).

use std::fs;
use std::time::Duration;

use avian2d::prelude::Position;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use tilerider::actions::{ActionsPlugin, Activation, CurrentActions, Inactive, OwnActivation, run};
use tilerider::content::map::{HintData, MapData};
use tilerider::content::map_list::MapList;
use tilerider::content::xna::XRect;
use tilerider::controls::LocalPlayer;
use tilerider::game::{LoadedMap, MapId};
use tilerider::hint::state::{CHECK_EVERY, FADE_SECS, Frames, HintState, triangle};
use tilerider::hint::{Hint, Look, apply_activations, check_area, step_hints};
use tilerider::map::grid::MapGrid;
use tilerider::units::CellSize;

mod common;
use common::assets_root;

fn map(name: &str) -> MapData {
    MapData::parse(&fs::read_to_string(assets_root().join(format!("Maps/Singleplayer/{name}.xml"))).unwrap()).unwrap()
}

fn hint_of(map: &MapData, id: i64) -> &HintData {
    map.hints.iter().find(|h| h.id == id).unwrap_or_else(|| panic!("no hint {id}"))
}

#[test]
fn a_maps_hints_are_read_with_every_field() {
    let garage = map("Garage");
    assert_eq!(garage.hints.items.len(), 8);
    let arrow = hint_of(&garage, 2003);
    assert_eq!(arrow.sprite, "Hints/ArrowAnimation");
    assert!(arrow.animated && !arrow.draw_on_ui);
    assert_eq!((arrow.position.x, arrow.position.y), (3.833_333_3, 7.9375));
    assert_eq!(arrow.area, XRect { x: 2, y: 3, w: 3, h: 9 });
    assert_eq!((arrow.scale, arrow.pulse_amount, arrow.timer), (1.0, 1.0, 0.0));
    assert_eq!(arrow.text_key(), None);
    // A countdown and a text.
    assert_eq!(hint_of(&garage, 2009).timer, 4.0);
    assert_eq!(hint_of(&garage, 2008).text_key(), Some("Button"));
    // An on-screen one with a direction, and an area that starts left of the map.
    let two_weeks = map("2WeeksAgo");
    assert!(hint_of(&two_weeks, 2003).draw_on_ui);
    assert_eq!(hint_of(&two_weeks, 2003).direction_deg, 270.0);
    assert_eq!(hint_of(&map("HappyNewYear"), 2001).area, XRect { x: -41, y: -30, w: 100, h: 100 });
}

#[test]
fn what_a_hint_shows_follows_its_timer_then_its_text() {
    let garage = map("Garage");
    let look = |id| Hint::new(hint_of(&garage, id), 1).look;
    assert_eq!(look(2009), Look::Countdown);
    assert_eq!(look(2008), Look::Text);
    assert_eq!(look(2003), Look::Sprite);
    // Only a sprite has frames, and only an animated one.
    assert!(Hint::new(hint_of(&garage, 2003), 3).frames.is_some());
    assert!(Hint::new(hint_of(&garage, 2008), 1).frames.is_none());
}

#[test]
fn every_campaign_hint_is_read() {
    let list = MapList::parse(&fs::read_to_string(assets_root().join("Maps/MapList.xml")).unwrap()).unwrap();
    let (mut with_hints, mut hints) = (0, 0);
    for name in list.singleplayer.iter() {
        let n = map(name).hints.items.len();
        with_hints += usize::from(n > 0);
        hints += n;
    }
    assert!(with_hints >= 28, "only {with_hints} campaign maps have hints");
    assert!(hints > 100, "only {hints} hints");
}

/// A random source that never says yes, or always does.
fn never(_: f32) -> bool {
    false
}
fn always(_: f32) -> bool {
    true
}

const DT: f32 = 1.0 / 60.0;

/// Runs a state for `secs` of game time; the phase is the clock's.
fn run_for(state: &mut HintState, secs: f32, now: &mut f32, chance: fn(f32) -> bool) {
    let steps = (secs / DT).round() as usize;
    for _ in 0..steps {
        *now += DT;
        state.update(DT, triangle(*now), chance);
    }
}

#[test]
fn a_hint_is_off_and_hidden_until_it_is_switched_on() {
    let mut state = HintState::new(0.0, false, 1.0);
    let mut now = 0.0;
    run_for(&mut state, 1.0, &mut now, always);
    assert!(!state.active && !state.visible);
    state.activate();
    assert!(state.active && state.visible);
}

#[test]
fn it_flickers_in_over_a_second_then_blinks_between_a_third_and_full() {
    // Quiet rolls leave it as it is; the opacity only changes at a toggle.
    let mut quiet = HintState::new(0.0, false, 1.0);
    quiet.activate();
    let mut t = 0.0;
    run_for(&mut quiet, 0.5, &mut t, never);
    assert!(quiet.visible);
    assert_eq!(quiet.opacity, 0.0, "no step brighter than the start was taken");
    // Past the second it is shown and blinks.
    run_for(&mut quiet, 0.6, &mut t, never);
    assert!(quiet.visible);
    let mut seen = (f32::MAX, f32::MIN);
    for _ in 0..120 {
        t += DT;
        quiet.update(DT, triangle(t), never);
        seen = (seen.0.min(quiet.opacity), seen.1.max(quiet.opacity));
    }
    assert!(seen.0 >= 0.3 - 1e-4 && seen.0 < 0.35, "dims to {}", seen.0);
    assert!(seen.1 > 0.95 && seen.1 <= 1.0 + 1e-4, "brightens to {}", seen.1);
}

#[test]
fn a_toggle_while_flickering_takes_a_brighter_step_the_later_it_is() {
    let mut state = HintState::new(0.0, false, 1.0);
    state.activate();
    let mut now = 0.0;
    // Visible after `activate`: the first yes-roll turns it off, the next
    // on again, each at the opacity its moment sets.
    let mut last = Vec::new();
    for _ in 0..40 {
        now += DT;
        let before = state.visible;
        state.update(DT, triangle(now), always);
        if state.visible != before {
            last.push(state.opacity);
        }
    }
    assert!(last.len() >= 2);
    assert!(last.windows(2).all(|w| w[1] >= w[0]), "{last:?}");
    assert!(last[0] >= 0.2);
}

#[test]
fn a_text_does_not_blink() {
    let mut state = HintState::new(0.0, true, 1.0);
    state.activate();
    let mut now = 0.0;
    run_for(&mut state, FADE_SECS + 0.2, &mut now, never);
    for _ in 0..90 {
        now += DT;
        state.update(DT, triangle(now), never);
        assert_eq!(state.opacity, 1.0);
        assert!(state.visible);
    }
}

#[test]
fn the_size_pulses_by_a_fifth_of_pulse_amount() {
    let mut state = HintState::new(0.0, false, 1.0);
    state.activate();
    state.update(DT, 0.0, never);
    assert!((state.size - 0.8).abs() < 1e-6);
    state.update(DT, 1.0, never);
    assert!((state.size - 1.0).abs() < 1e-6);
    let mut still = HintState::new(0.0, false, 0.0);
    still.activate();
    still.update(DT, 0.0, never);
    assert_eq!(still.size, 1.0);
    let mut big = HintState::new(0.0, false, 0.1);
    big.activate();
    big.update(DT, 0.0, never);
    assert!((big.size - 0.98).abs() < 1e-6);
}

#[test]
fn switched_off_it_fades_for_a_second_and_goes() {
    let mut state = HintState::new(0.0, true, 1.0);
    state.activate();
    let mut now = 0.0;
    run_for(&mut state, 1.5, &mut now, never);
    state.deactivate();
    assert!(state.active, "still on while it fades");
    let mut last = 1.0;
    let mut secs = 0.0;
    while state.active {
        now += DT;
        secs += DT;
        state.update(DT, triangle(now), never);
        if state.active {
            assert!(state.opacity <= last + 1e-6, "fades steadily");
            last = state.opacity;
        }
        assert!(secs < 2.0, "never went off");
    }
    assert!((secs - FADE_SECS).abs() < 0.05, "faded for {secs}");
    assert!(!state.visible);
    // It can come on again.
    state.activate();
    assert!(state.active && state.visible && !state.deactivating);
}

#[test]
fn a_hint_switched_off_in_the_frame_it_came_on_still_goes() {
    let mut state = HintState::new(0.0, false, 1.0);
    state.activate();
    state.deactivate();
    state.update(DT, 0.0, never);
    assert!(!state.active);
}

#[test]
fn a_countdown_starts_when_switched_on_and_stops_at_zero() {
    let mut state = HintState::new(4.0, false, 1.0);
    assert_eq!(state.countdown.unwrap().remains, 0.0);
    state.activate();
    assert_eq!(state.countdown.unwrap().text(), "4.0");
    let mut now = 0.0;
    run_for(&mut state, 1.5, &mut now, never);
    assert_eq!(state.countdown.unwrap().text(), "2.5");
    run_for(&mut state, 5.0, &mut now, never);
    assert_eq!(state.countdown.unwrap().text(), "0.0");
    // Fading out holds it; switching on again starts over.
    state.deactivate();
    state.activate();
    assert_eq!(state.countdown.unwrap().text(), "4.0");
    // The count does not run while the hint is off.
    let mut off = HintState::new(4.0, false, 1.0);
    run_for(&mut off, 2.0, &mut now, never);
    assert_eq!(off.countdown.unwrap().remains, 0.0);
}

#[test]
fn the_area_is_looked_at_at_once_then_once_a_period() {
    let mut state = HintState::new(0.0, false, 1.0);
    let due: Vec<bool> = [0.016, 0.5, 1.0, 1.02, 1.5, 2.1, 2.2].iter().map(|&t| state.check_due(t)).collect();
    assert_eq!(due, [true, false, false, true, false, true, false]);
    assert_eq!(CHECK_EVERY, 1.0);
}

#[test]
fn leaving_the_area_fades_it_and_returning_during_the_fade_waits_for_it_to_end() {
    let mut state = HintState::new(0.0, true, 1.0);
    state.player_is_in_area(true);
    assert!(state.active);
    state.player_is_in_area(true);
    assert!(!state.deactivating);
    state.player_is_in_area(false);
    assert!(state.deactivating);
    // Back in while it still fades: nothing, it is on.
    state.player_is_in_area(true);
    assert!(state.deactivating && state.active);
    let mut now = 0.0;
    run_for(&mut state, 1.2, &mut now, never);
    assert!(!state.active);
    state.player_is_in_area(true);
    assert!(state.active && !state.deactivating);
    // Out of the area with the hint off does nothing.
    let mut off = HintState::new(0.0, true, 1.0);
    off.player_is_in_area(false);
    assert!(!off.deactivating);
}

#[test]
fn frames_advance_by_the_frame_time_and_skip_what_a_long_step_covers() {
    let mut frames = Frames::new(3, None);
    assert_eq!(frames.current(), 0);
    frames.advance(0.4);
    assert_eq!(frames.current(), 0);
    frames.advance(0.2);
    assert_eq!(frames.current(), 1);
    frames.advance(0.5);
    assert_eq!(frames.current(), 2);
    frames.advance(0.5);
    assert_eq!(frames.current(), 0, "loops");
    // A long step moves on by as many frames as it spans.
    frames.advance(1.7);
    assert_eq!(frames.current(), 0);
    // A given sequence picks the frames and their order.
    let mut seq = Frames::new(4, Some(&[3, 1]));
    assert_eq!(seq.current(), 3);
    seq.advance(0.6);
    assert_eq!(seq.current(), 1);
    seq.advance(0.6);
    assert_eq!(seq.current(), 3);
}

// The systems, in a small world.

const CELL: f32 = 2.0;

fn at(x: f32, y: f32) -> Vec2 {
    CellSize(CELL).to_world(x, y)
}

fn loaded_map() -> LoadedMap {
    let data = MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>
.
</CharMap></Asset></XnaContent>"#,
    )
    .unwrap();
    let grid = MapGrid::build(&data, None);
    let light = tilerider::light::bake::LightMap::bake(&data, &grid, &tilerider::light::raster::Mask::from_alpha(1, 1, &[255; 4]), &[]);
    LoadedMap { data, grid, light, cell: CellSize(CELL), cars: default(), list: default(), sprites: default() }
}

fn hint_data(area: &str, text: bool) -> HintData {
    let text = if text { "<Text>Go</Text>" } else { "" };
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Hints><Item><Id>2001</Id>
        <Position>5.5 5.5</Position><Sprite>WhitePixel</Sprite><DirectionDeg>0</DirectionDeg>
        <Animated>false</Animated><Timer>0</Timer><Scale>1</Scale><Area>{area}</Area>
        <DrawOnUI>false</DrawOnUI>{text}</Item></Hints></Asset></XnaContent>"#
    );
    MapData::parse(&xml).unwrap().hints.items.remove(0)
}

fn timed_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)))
        .insert_resource(loaded_map())
        .add_message::<Activation>()
        .add_systems(Update, (check_area, apply_activations, step_hints).chain());
    app
}

fn state_of(app: &mut App, e: Entity) -> (bool, bool, bool) {
    let s = &app.world().get::<Hint>(e).unwrap().state;
    (s.active, s.visible, s.deactivating)
}

fn advance(app: &mut App, secs: f32) {
    for _ in 0..(secs * 10.0).round() as usize {
        app.update();
    }
}

#[test]
fn the_player_in_the_area_switches_the_hint_on_within_a_second_and_leaving_switches_it_off() {
    let mut app = timed_app();
    let hint = app.world_mut().spawn(Hint::new(&hint_data("2 2 3 3", true), 1)).id();
    let player = app.world_mut().spawn((LocalPlayer, Position(at(0.5, 0.5)))).id();
    advance(&mut app, 2.0);
    assert!(!state_of(&mut app, hint).0, "the player is outside");
    app.world_mut().entity_mut(player).insert(Position(at(3.5, 3.5)));
    advance(&mut app, 1.2);
    assert!(state_of(&mut app, hint).0, "the player came in a second ago");
    advance(&mut app, 1.0);
    assert!(state_of(&mut app, hint).1);
    // A text is whole (opaque) once it has flickered in.
    assert_eq!(app.world().get::<Hint>(hint).unwrap().state.opacity, 1.0);
    app.world_mut().entity_mut(player).insert(Position(at(7.5, 7.5)));
    advance(&mut app, 1.2);
    assert!(state_of(&mut app, hint).2, "fading out");
    advance(&mut app, 1.2);
    assert!(!state_of(&mut app, hint).0, "gone");
}

#[test]
fn an_area_without_size_is_never_checked_and_no_player_changes_nothing() {
    let mut app = timed_app();
    let none = app.world_mut().spawn(Hint::new(&hint_data("3 3 0 0", false), 1)).id();
    let waiting = app.world_mut().spawn(Hint::new(&hint_data("2 2 3 3", false), 1)).id();
    advance(&mut app, 2.0);
    assert!(!state_of(&mut app, none).0 && !state_of(&mut app, waiting).0, "nobody to see");
    app.world_mut().spawn((LocalPlayer, Position(at(3.5, 3.5))));
    advance(&mut app, 1.2);
    assert!(!state_of(&mut app, none).0, "inside the cell, but there is no area to be in");
    assert!(state_of(&mut app, waiting).0);
}

#[test]
fn activate_and_deactivate_entity_switch_a_hint_without_hiding_it() {
    let xml = r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>
        <Item Type="GameLibrary.ActivateEntityActionData"><Name>On</Name><ItemId>2001</ItemId><Deactivate>false</Deactivate></Item>
        <Item Type="GameLibrary.ActivateEntityActionData"><Name>Off</Name><ItemId>2001</ItemId><Deactivate>true</Deactivate></Item>
        </Actions></Asset></XnaContent>"#;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, ActionsPlugin))
        .add_message::<tilerider::explosion::Explosion>()
        .add_message::<tilerider::explosion::Destroyed>()
        .insert_resource(CurrentActions(MapData::parse(xml).unwrap().actions));
    let hint = app
        .world_mut()
        .spawn((MapId(2001), OwnActivation, Transform::default(), Hint::new(&hint_data("0 0 0 0", false), 1)))
        .id();
    assert!(run(app.world_mut(), "On", None, 0));
    app.world_mut().run_system_once(apply_activations).unwrap();
    assert!(state_of(&mut app, hint).0);
    assert!(run(app.world_mut(), "Off", None, 0));
    app.world_mut().run_system_once(apply_activations).unwrap();
    assert_eq!(state_of(&mut app, hint), (true, true, true), "fading, not cut");
    assert!(!app.world().entity(hint).contains::<Inactive>());
    assert!(!matches!(app.world().get::<Visibility>(hint), Some(Visibility::Hidden)));
    // Destroy takes it out of the world.
    assert!(run(app.world_mut(), "DestroyEntity(2001)", None, 0));
    app.update();
    assert!(app.world().get_entity(hint).is_err());
}
