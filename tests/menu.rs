//! The map menu: its lists (the campaign in order, every map file), the
//! choice in them, a tap against a drag, how a pick asks for the map, and
//! when the game opens in the menu instead of a map.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use bevy::input::mouse::MouseWheel;
use bevy::input::touch::Touches;
use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use tilerider::content::map_list::MapList;
use tilerider::game::{CurrentMapName, GameState};
use tilerider::info::GameFont;
use tilerider::menu::model::{Gesture, MenuModel, OptionRow, TAP_SLOP, Tab, parse_index};
use tilerider::settings::{ControlScheme, Settings};
use tilerider::menu::{MenuPlugin, starts_in_menu};

fn assets() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"))
}

fn campaign() -> Vec<String> {
    let xml = fs::read_to_string(assets().join("Maps/MapList.xml")).unwrap();
    MapList::parse(&xml).unwrap().singleplayer.items
}

fn all_maps() -> Vec<String> {
    parse_index(&fs::read_to_string(assets().join("Maps/AllMaps.txt")).unwrap())
}

fn model() -> MenuModel {
    MenuModel::new(campaign(), all_maps())
}

#[test]
fn the_campaign_tab_lists_the_campaign_maps_in_order() {
    let m = model();
    assert_eq!(m.entries(Tab::Campaign), campaign().as_slice());
    assert_eq!(m.entries(Tab::Campaign).len(), 36);
    assert_eq!(m.entries(Tab::Campaign)[0], "Middleware");
}

#[test]
fn the_all_maps_tab_lists_every_map_file_by_name() {
    let mut files: Vec<String> = fs::read_dir(assets().join("Maps/Singleplayer"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "xml"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    files.sort_by_key(|n| n.to_lowercase());
    assert_eq!(model().entries(Tab::All), files.as_slice(), "assets/Maps/AllMaps.txt is stale: run tools/import_content.py");
}

#[test]
fn every_campaign_map_is_among_all_the_maps() {
    let all: BTreeSet<_> = all_maps().into_iter().collect();
    for map in campaign() {
        assert!(all.contains(&map), "{map}");
    }
}

#[test]
fn the_index_is_sorted_ignoring_case_without_blanks_or_repeats() {
    assert_eq!(parse_index("beta\n\n Alpha \r\nbeta\ngamma\n"), ["Alpha", "beta", "gamma"]);
}

#[test]
fn the_choice_moves_within_the_shown_list_and_stops_at_its_ends() {
    let mut m = model();
    assert_eq!(m.chosen(), Some("Middleware"));
    m.step(-1);
    assert_eq!(m.chosen(), Some("Middleware"));
    m.step(2);
    assert_eq!(m.chosen(), Some(campaign()[2].as_str()));
    m.step(1000);
    assert_eq!(m.chosen(), campaign().last().map(String::as_str));
    // Each list keeps its own choice.
    m.switch_tab(1);
    assert_eq!(m.tab(), Tab::All);
    assert_eq!(m.chosen(), Some(all_maps()[0].as_str()));
    m.switch_tab(-1);
    assert_eq!(m.chosen(), campaign().last().map(String::as_str));
}

#[test]
fn selecting_an_entry_shows_its_list_and_chooses_it() {
    let mut m = model();
    let index = all_maps().iter().position(|n| n == "Garage").unwrap();
    m.select(Tab::All, index);
    assert_eq!((m.tab(), m.chosen()), (Tab::All, Some("Garage")));
    m.select(Tab::All, 10_000);
    assert_eq!(m.chosen(), Some("Garage"), "no such entry");
}

#[test]
fn opening_over_a_map_puts_the_choice_on_it() {
    let mut m = model();
    m.focus_on("Garage");
    assert_eq!((m.tab(), m.chosen()), (Tab::Campaign, Some("Garage")));
    // A map outside the campaign is found in the full list.
    m.focus_on("EditorTest");
    assert_eq!((m.tab(), m.chosen()), (Tab::All, Some("EditorTest")));
    m.focus_on("NoSuchMap");
    assert_eq!(m.chosen(), Some("EditorTest"));
}

#[test]
fn a_press_that_stays_put_is_a_tap() {
    let mut g = Gesture::press(Vec2::new(100.0, 100.0));
    assert_eq!(g.moved(Vec2::new(103.0, 104.0)), 0.0);
    assert_eq!(g.released(), Some(Vec2::new(103.0, 104.0)));
}

#[test]
fn a_press_that_moves_far_scrolls_and_is_no_tap() {
    let mut g = Gesture::press(Vec2::new(100.0, 300.0));
    assert_eq!(g.moved(Vec2::new(100.0, 300.0 - TAP_SLOP - 5.0)), TAP_SLOP + 5.0, "the list catches up with the finger");
    assert_eq!(g.moved(Vec2::new(100.0, 250.0)), 300.0 - TAP_SLOP - 5.0 - 250.0);
    assert_eq!(g.moved(Vec2::new(100.0, 260.0)), -10.0, "back down scrolls back");
    assert_eq!(g.released(), None);
}

#[test]
fn the_game_opens_in_the_menu_unless_a_map_or_a_run_is_named() {
    assert!(starts_in_menu(false, false, false), "no options");
    assert!(!starts_in_menu(false, true, false), "-m MAP goes straight in");
    assert!(!starts_in_menu(false, false, true), "--screenshot, --record, --drive, --start run the default map");
    assert!(starts_in_menu(true, true, true), "--menu asks for it");
}

fn app_in_menu() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_state::<GameState>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<Touches>()
        .add_message::<MouseWheel>()
        .insert_resource(CurrentMapName("EditorTest".into()))
        .insert_resource(GameFont(Handle::default()))
        .insert_resource(model())
        .add_plugins(MenuPlugin);
    app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Menu);
    app.update();
    app.update();
    app
}

fn press(app: &mut App, key: KeyCode) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
}

fn state(app: &App) -> GameState {
    *app.world().resource::<State<GameState>>().get()
}

#[test]
fn enter_asks_for_the_chosen_map() {
    let mut app = app_in_menu();
    assert_eq!(state(&app), GameState::Menu);
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter);
    app.update();
    assert_eq!(state(&app), GameState::Loading);
    assert_eq!(app.world().resource::<CurrentMapName>().0, campaign()[2]);
}

#[test]
fn the_other_tab_picks_from_all_the_maps() {
    let mut app = app_in_menu();
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::End);
    press(&mut app, KeyCode::Enter);
    app.update();
    assert_eq!(state(&app), GameState::Loading);
    assert_eq!(app.world().resource::<CurrentMapName>().0, all_maps().last().unwrap().as_str());
}

#[test]
fn escape_does_nothing_when_there_is_no_map_to_go_back_to() {
    let mut app = app_in_menu();
    press(&mut app, KeyCode::Escape);
    app.update();
    assert_eq!(state(&app), GameState::Menu);
    assert_eq!(app.world().resource::<CurrentMapName>().0, "EditorTest");
}

fn to_options(app: &mut App) {
    press(app, KeyCode::Tab);
    press(app, KeyCode::Tab);
    assert_eq!(app.world().resource::<MenuModel>().tab(), Tab::Options);
}

fn settings(app: &App) -> Settings {
    app.world().resource::<Settings>().clone()
}

#[test]
fn tab_goes_round_the_three_tabs() {
    let mut app = app_in_menu();
    let tab = |app: &App| app.world().resource::<MenuModel>().tab();
    let seen: Vec<Tab> = (0..3)
        .map(|_| {
            press(&mut app, KeyCode::Tab);
            tab(&app)
        })
        .collect();
    assert_eq!(seen, [Tab::All, Tab::Options, Tab::Campaign]);
}

#[test]
fn the_options_tab_has_a_row_for_each_option() {
    let m = model();
    assert_eq!(m.len(Tab::Options), OptionRow::ALL.len());
    assert!(m.entries(Tab::Options).is_empty());
    assert_eq!(m.chosen(), Some("Middleware"), "the options are no map");
}

#[test]
fn enter_changes_the_chosen_option_and_asks_for_no_map() {
    let mut app = app_in_menu();
    to_options(&mut app);
    // Controls: Classic -> Smart -> Classic.
    press(&mut app, KeyCode::Enter);
    assert_eq!(settings(&app).control_scheme, ControlScheme::Smart);
    press(&mut app, KeyCode::Enter);
    assert_eq!(settings(&app).control_scheme, ControlScheme::Classic);
    assert_eq!(state(&app), GameState::Menu);
}

#[test]
fn left_and_right_change_the_value_and_the_arrows_choose_the_row() {
    let mut app = app_in_menu();
    to_options(&mut app);
    press(&mut app, KeyCode::ArrowRight);
    assert_eq!(settings(&app).control_scheme, ControlScheme::Smart);
    press(&mut app, KeyCode::ArrowLeft);
    assert_eq!(settings(&app).control_scheme, ControlScheme::Classic);
    assert_eq!(app.world().resource::<MenuModel>().tab(), Tab::Options, "left and right stay on the tab");

    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::ArrowRight);
    assert!(!settings(&app).auto_fire);
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter);
    assert!(!settings(&app).auto_aim);
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter);
    assert!(!settings(&app).tap_to_go);
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter);
    assert!(!settings(&app).music);
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::ArrowRight);
    assert_eq!(settings(&app).language.as_deref(), Some("ru"));
    press(&mut app, KeyCode::ArrowLeft);
    assert_eq!(settings(&app).language, None);
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::ArrowRight);
    assert_eq!(app.world().resource::<MenuModel>().chosen_option(), OptionRow::Language, "the last row stays");
}

#[test]
fn the_smart_rows_are_dimmed_until_the_scheme_is_smart() {
    let mut s = Settings::default();
    for row in [OptionRow::AutoFire, OptionRow::AutoAim, OptionRow::TapToGo] {
        assert!(row.dimmed(&s));
    }
    for row in [OptionRow::Controls, OptionRow::Music, OptionRow::Language] {
        assert!(!row.dimmed(&s));
    }
    s.control_scheme = ControlScheme::Smart;
    assert!(!OptionRow::AutoFire.dimmed(&s));
}

#[test]
fn the_options_rows_say_their_values() {
    let mut s = Settings::default();
    assert_eq!(OptionRow::Music.value(&s), "On");
    OptionRow::Music.adjust(&mut s, 1);
    assert_eq!(OptionRow::Music.value(&s), "Off");
    assert_eq!(OptionRow::Language.value(&s), "English");
}

#[test]
fn the_other_tabs_still_use_left_and_right_for_the_tabs() {
    let mut app = app_in_menu();
    press(&mut app, KeyCode::ArrowRight);
    assert_eq!(app.world().resource::<MenuModel>().tab(), Tab::All);
    press(&mut app, KeyCode::ArrowLeft);
    assert_eq!(app.world().resource::<MenuModel>().tab(), Tab::Campaign);
}
