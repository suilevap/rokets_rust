//! The map menu: pick a map from the campaign or from all of them.
//!
//! No C# menu is ported here. The C# game chose a level on its `GlobalMap`
//! (a world map of level pictures, `GameModeSinglePlayer`, `LevelSelect`
//! ideas: `MapList.xml`'s order, a screen of pictures per level, progress
//! stars). This is the plain list the port needs to try its maps, in the
//! game's own font (`Pixar Two`, the Title font's size) and texts: a map's
//! name is the text under its key when the game's texts have one
//! (`Middleware` is "Entry Point"), and its file name otherwise.
//!
//! Two tabs, each a scrolling list:
//! - **Campaign**: `Maps/MapList.xml`'s `Singleplayer` items, in order;
//! - **All maps**: `Maps/AllMaps.txt`, every map of `Maps/Singleplayer`
//!   by name. The browser cannot list a folder, so `tools/import_content.py`
//!   writes the list when it imports (`tests/menu.rs` checks it against the
//!   folder).
//!
//! How to reach it: the game opens in it when started without a map (no
//! `-m` / `?map=`, and no `--screenshot`, `--record`, `--drive` or `--start`,
//! which name a map's run) or with `--menu`; while playing, `Esc`, the
//! gamepad's Start, or a touch screen's "Menu" button (top left) leave the
//! map for it. `Esc` / the gamepad's B / the "Back" button there start the
//! map that was left again (from its beginning: the world is not kept), when
//! there is one.
//!
//! Controls: mouse (click a tab or a map, wheel to scroll), touch (tap, drag
//! to scroll; every row is 64 px tall), keyboard (up and down, page up and
//! down, home and end, left and right or `Tab` for the tabs, `Enter` picks),
//! gamepad (d-pad, shoulder buttons for the tabs, A picks).
//!
//! Choosing a map sets [`CurrentMapName`] and goes to `Loading`, as
//! `ChangeMap` does, so a finished level still goes on to the campaign's next
//! map, from wherever it was started.
//!
//! Not ported: the level pictures and progress stars, the C# menus' fades and
//! sounds, and the texts' translation of the tabs and buttons (English
//! only; the maps' names follow `--lang`).

pub mod model;

use bevy::ecs::schedule::SystemCondition;
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::touch::Touches;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, GlobalZIndex, ScrollPosition, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use crate::assets::XmlText;
use crate::content::map_list::MapList;
use crate::controls::ActiveScheme;
use crate::game::{CurrentMapName, GameState, LoadedMap};
use crate::info::{GameFont, Texts};
use model::{Gesture, MenuModel, Tab, parse_index};

/// Whether the game opens in the menu: with `--menu`, or when nothing names
/// a map to run (`map_given`: `-m`; `scripted`: `--screenshot`, `--record`,
/// `--drive` or `--start`, which run the default map for a purpose).
pub fn starts_in_menu(menu_flag: bool, map_given: bool, scripted: bool) -> bool {
    menu_flag || !(map_given || scripted)
}

/// A row's height, logical pixels: a thumb's width.
pub const ROW_HEIGHT: f32 = 64.0;
/// The gap between rows.
const ROW_GAP: f32 = 4.0;
/// The tabs' height.
const TAB_HEIGHT: f32 = 56.0;
/// A mouse wheel notch, logical pixels.
const WHEEL_LINE: f32 = 48.0;
/// Rows a page key moves by.
const PAGE: i32 = 8;

const BACKGROUND: Color = Color::srgb(0.07, 0.08, 0.11);
const TAB_IDLE: Color = Color::srgb(0.16, 0.18, 0.24);
const ROW_IDLE: Color = Color::srgb(0.12, 0.14, 0.19);
const ACCENT: Color = Color::srgb(0.82, 0.5, 0.08);
const DIM: Color = Color::srgb(0.62, 0.66, 0.74);

/// The touch screen's way into the menu while playing: a button at the top
/// left, in logical pixels.
const BUTTON_AT: Vec2 = Vec2::new(8.0, 8.0);
const BUTTON_SIZE: Vec2 = Vec2::new(76.0, 44.0);

/// What the menu is asked to do, by any device.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// The chosen entry moves by this many.
    Step(i32),
    /// The other tab.
    SwitchTab,
    /// Show this tab.
    ShowTab(Tab),
    /// Play the chosen entry.
    Confirm,
    /// Play this entry of this list (tapped).
    Choose(Tab, usize),
    /// Go back to the map that was left.
    Back,
}

/// The files the lists come from, while they load.
#[derive(Resource)]
struct MenuFiles {
    campaign: Handle<XmlText>,
    all: Handle<XmlText>,
}

/// The menu's screen, while it is shown.
#[derive(Resource)]
struct MenuUi(Entity);

/// The chosen row is to be brought into view.
#[derive(Resource, Default)]
struct Reveal(bool);

#[derive(Component)]
struct ListNode(Tab);

#[derive(Component)]
struct TabButton(Tab);

#[derive(Component)]
struct Row {
    tab: Tab,
    index: usize,
}

/// The text of a row: the map's name, or (the small line) its file's.
#[derive(Component)]
struct RowLabel {
    map: String,
    file: bool,
}

#[derive(Component)]
struct BackButton;

#[derive(Component)]
struct PlayButton;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MenuAction>()
            .init_resource::<Reveal>()
            .add_systems(Startup, load_lists)
            .add_systems(Update, (read_lists, spawn_play_button, open_menu, show_play_button))
            .add_systems(
                Update,
                (
                    spawn_menu,
                    (keyboard, gamepad, pointer),
                    apply,
                    (paint, label_rows, reveal).chain(),
                )
                    .chain()
                    .run_if(in_state(GameState::Menu).and_then(resource_exists::<MenuModel>)),
            )
            .add_systems(OnExit(GameState::Menu), close_menu);
    }
}

fn load_lists(mut commands: Commands, server: Option<Res<AssetServer>>) {
    let Some(server) = server else {
        return;
    };
    commands.insert_resource(MenuFiles {
        campaign: server.load("Maps/MapList.xml"),
        all: server.load("Maps/AllMaps.txt"),
    });
}

/// Builds the lists once both files are read.
fn read_lists(
    mut commands: Commands,
    files: Option<Res<MenuFiles>>,
    texts: Option<Res<Assets<XmlText>>>,
    server: Option<Res<AssetServer>>,
) {
    let (Some(files), Some(texts), Some(server)) = (files, texts, server) else {
        return;
    };
    let (Some(campaign), Some(all)) = (texts.get(&files.campaign), texts.get(&files.all)) else {
        for handle in [&files.campaign, &files.all] {
            if let Some(bevy::asset::LoadState::Failed(e)) = server.get_load_state(handle) {
                error!("map menu: could not load a list of maps: {e}");
                commands.remove_resource::<MenuFiles>();
            }
        }
        return;
    };
    let campaign = MapList::parse(&campaign.0)
        .map_err(|e| error!("map menu: MapList.xml: {e}"))
        .map(|list| list.singleplayer.items)
        .unwrap_or_default();
    commands.insert_resource(MenuModel::new(campaign, parse_index(&all.0)));
    commands.remove_resource::<MenuFiles>();
}

fn text_font(font: &GameFont, size: f32) -> TextFont {
    TextFont { font: font.0.clone().into(), font_size: FontSize::Px(size), ..default() }
}

/// Puts the menu on screen once its lists are known.
fn spawn_menu(
    mut commands: Commands,
    ui: Option<Res<MenuUi>>,
    mut model: ResMut<MenuModel>,
    font: Res<GameFont>,
    played: Option<Res<LoadedMap>>,
    name: Res<CurrentMapName>,
    mut reveal: ResMut<Reveal>,
) {
    if ui.is_some() {
        return;
    }
    if played.is_some() {
        model.focus_on(&name.0);
    }
    reveal.0 = true;
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(BACKGROUND),
            GlobalZIndex(1000),
        ))
        .id();
    let column = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            max_width: Val::Px(760.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(root).add_child(column);
    let title = commands
        .spawn((Text::new("TileRider"), text_font(&font, 40.0), TextColor(Color::WHITE)))
        .id();
    commands.entity(column).add_child(title);

    let tabs = commands
        .spawn(Node { column_gap: Val::Px(8.0), width: Val::Percent(100.0), ..default() })
        .id();
    commands.entity(column).add_child(tabs);
    for tab in Tab::BOTH {
        let button = commands
            .spawn((
                TabButton(tab),
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    height: Val::Px(TAB_HEIGHT),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(TAB_IDLE),
                children![(Text::new(tab.title()), text_font(&font, 26.0), TextColor(Color::WHITE))],
            ))
            .id();
        commands.entity(tabs).add_child(button);
    }

    for tab in Tab::BOTH {
        let list = commands
            .spawn((
                ListNode(tab),
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    // A scrolling list shrinks to the room left; its rows do not.
                    min_height: Val::Px(0.0),
                    row_gap: Val::Px(ROW_GAP),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
            ))
            .id();
        commands.entity(column).add_child(list);
        for (index, map) in model.entries(tab).iter().enumerate() {
            let row = commands
                .spawn((
                    Row { tab, index },
                    Node {
                        height: Val::Px(ROW_HEIGHT),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::horizontal(Val::Px(16.0)),
                        ..default()
                    },
                    BackgroundColor(ROW_IDLE),
                    children![
                        (
                            RowLabel { map: map.clone(), file: false },
                            Text::new(map.clone()),
                            text_font(&font, 26.0),
                            TextColor(Color::WHITE),
                        ),
                        (
                            RowLabel { map: map.clone(), file: true },
                            Text::new(""),
                            text_font(&font, 16.0),
                            TextColor(DIM),
                        ),
                    ],
                ))
                .id();
            commands.entity(list).add_child(row);
        }
    }

    let footer = commands
        .spawn(Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(column).add_child(footer);
    let hint = commands
        .spawn((
            Text::new("Arrows: choose   Enter: play   Esc: back"),
            text_font(&font, 16.0),
            TextColor(DIM),
        ))
        .id();
    let back = commands
        .spawn((
            BackButton,
            Node {
                height: Val::Px(TAB_HEIGHT),
                padding: UiRect::horizontal(Val::Px(24.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(TAB_IDLE),
            children![(Text::new("Back"), text_font(&font, 26.0), TextColor(Color::WHITE))],
        ))
        .id();
    commands.entity(footer).add_children(&[hint, back]);
    commands.insert_resource(MenuUi(root));
}

fn close_menu(mut commands: Commands, ui: Option<Res<MenuUi>>) {
    if let Some(ui) = ui {
        commands.entity(ui.0).despawn();
        commands.remove_resource::<MenuUi>();
    }
}

fn keyboard(keys: Res<ButtonInput<KeyCode>>, mut actions: MessageWriter<MenuAction>) {
    let steps = [
        (KeyCode::ArrowUp, -1),
        (KeyCode::KeyW, -1),
        (KeyCode::ArrowDown, 1),
        (KeyCode::KeyS, 1),
        (KeyCode::PageUp, -PAGE),
        (KeyCode::PageDown, PAGE),
        (KeyCode::Home, -100_000),
        (KeyCode::End, 100_000),
    ];
    for (key, step) in steps {
        if keys.just_pressed(key) {
            actions.write(MenuAction::Step(step));
        }
    }
    if keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::ArrowRight, KeyCode::KeyA, KeyCode::KeyD, KeyCode::Tab]) {
        actions.write(MenuAction::SwitchTab);
    }
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]) {
        actions.write(MenuAction::Confirm);
    }
    if keys.just_pressed(KeyCode::Escape) {
        actions.write(MenuAction::Back);
    }
}

fn gamepad(pads: Query<&Gamepad>, mut actions: MessageWriter<MenuAction>) {
    let pressed = |button| pads.iter().any(|p| p.just_pressed(button));
    if pressed(GamepadButton::DPadUp) {
        actions.write(MenuAction::Step(-1));
    }
    if pressed(GamepadButton::DPadDown) {
        actions.write(MenuAction::Step(1));
    }
    if pressed(GamepadButton::DPadLeft)
        || pressed(GamepadButton::DPadRight)
        || pressed(GamepadButton::LeftTrigger)
        || pressed(GamepadButton::RightTrigger)
    {
        actions.write(MenuAction::SwitchTab);
    }
    if pressed(GamepadButton::South) {
        actions.write(MenuAction::Confirm);
    }
    if pressed(GamepadButton::East) {
        actions.write(MenuAction::Back);
    }
}

/// Scrolls a list by `dy` logical pixels, to its ends and no further.
fn scroll_by(position: &mut ScrollPosition, node: &ComputedNode, dy: f32) {
    let room = ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.0);
    position.y = (position.y + dy).clamp(0.0, room);
}

type Hit<'a> = (&'a ComputedNode, &'a UiGlobalTransform);

/// Mouse and touch: the wheel and a drag scroll the list, a tap or a click
/// picks a tab, a row or the back button.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn pointer(
    touches: Res<Touches>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut lists: Query<(&ListNode, &mut ScrollPosition, &ComputedNode)>,
    shapes: (
        Query<(&ListNode, Hit)>,
        Query<(&TabButton, Hit)>,
        Query<(&Row, Hit)>,
        Query<Hit, With<BackButton>>,
    ),
    model: Res<MenuModel>,
    mut gesture: Local<Option<Gesture>>,
    mut actions: MessageWriter<MenuAction>,
) {
    let shown = model.tab();
    let mut list = lists.iter_mut().find(|(l, _, _)| l.0 == shown);
    for w in wheel.read() {
        let dy = match w.unit {
            MouseScrollUnit::Line => -w.y * WHEEL_LINE,
            MouseScrollUnit::Pixel => -w.y,
        };
        if let Some((_, position, node)) = list.as_mut() {
            scroll_by(position, node, dy);
        }
    }

    let window = windows.single().ok();
    let now = touches
        .iter()
        .next()
        .map(|t| t.position())
        .or_else(|| mouse.pressed(MouseButton::Left).then(|| window.and_then(Window::cursor_position)).flatten());
    match (now, gesture.as_mut()) {
        (Some(at), None) => *gesture = Some(Gesture::press(at)),
        (Some(at), Some(g)) => {
            let dy = g.moved(at);
            if dy != 0.0
                && let Some((_, position, node)) = list.as_mut()
            {
                scroll_by(position, node, dy);
            }
        }
        (None, Some(_)) => {
            let Some(at) = gesture.take().and_then(Gesture::released) else {
                return;
            };
            let scale = window.map_or(1.0, Window::scale_factor);
            let point = at * scale;
            let (list_shapes, tabs, rows, back) = shapes;
            if let Some((tab, _)) = tabs.iter().find(|(_, (n, t))| n.contains_point(**t, point)) {
                actions.write(MenuAction::ShowTab(tab.0));
            } else if back.iter().any(|(n, t)| n.contains_point(*t, point)) {
                actions.write(MenuAction::Back);
            } else if list_shapes.iter().any(|(l, (n, t))| l.0 == shown && n.contains_point(*t, point))
                && let Some((row, _)) = rows
                    .iter()
                    .find(|(r, (n, t))| r.tab == shown && n.contains_point(**t, point))
            {
                actions.write(MenuAction::Choose(row.tab, row.index));
            }
        }
        (None, None) => {}
    }
}

/// Carries the actions out: the choice moves, a map is asked for.
fn apply(
    mut actions: MessageReader<MenuAction>,
    mut model: ResMut<MenuModel>,
    mut reveal: ResMut<Reveal>,
    mut name: ResMut<CurrentMapName>,
    played: Option<Res<LoadedMap>>,
    mut next: ResMut<NextState<GameState>>,
) {
    for action in actions.read() {
        match *action {
            MenuAction::Step(n) => {
                model.step(n);
                reveal.0 = true;
            }
            MenuAction::SwitchTab => {
                model.switch_tab(1);
                reveal.0 = true;
            }
            MenuAction::ShowTab(tab) => {
                model.set_tab(tab);
                reveal.0 = true;
            }
            MenuAction::Choose(tab, index) => {
                model.select(tab, index);
                play(&model, &mut name, &mut next);
            }
            MenuAction::Confirm => play(&model, &mut name, &mut next),
            MenuAction::Back => {
                if played.is_some() {
                    next.set(GameState::Loading);
                }
            }
        }
    }
}

/// Asks for the chosen map: the loading state reads [`CurrentMapName`].
fn play(model: &MenuModel, name: &mut CurrentMapName, next: &mut NextState<GameState>) {
    if let Some(map) = model.chosen() {
        name.0 = map.to_string();
        next.set(GameState::Loading);
    }
}

/// Colours the chosen row and tab, shows the shown list, the back button
/// when there is a map to go back to.
#[allow(clippy::type_complexity)]
fn paint(
    model: Res<MenuModel>,
    played: Option<Res<LoadedMap>>,
    mut rows: Query<(&Row, &mut BackgroundColor), (Without<TabButton>, Without<BackButton>)>,
    mut tabs: Query<(&TabButton, &mut BackgroundColor), (Without<Row>, Without<BackButton>)>,
    mut lists: Query<(&ListNode, &mut Node), Without<BackButton>>,
    mut back: Query<&mut Node, With<BackButton>>,
) {
    for (row, mut color) in &mut rows {
        let want = if row.tab == model.tab() && row.index == model.selected(row.tab) { ACCENT } else { ROW_IDLE };
        if color.0 != want {
            color.0 = want;
        }
    }
    for (tab, mut color) in &mut tabs {
        let want = if tab.0 == model.tab() { ACCENT } else { TAB_IDLE };
        if color.0 != want {
            color.0 = want;
        }
    }
    for (list, mut node) in &mut lists {
        let want = if list.0 == model.tab() { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    for mut node in &mut back {
        let want = if played.is_some() { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
}

/// The rows' texts: the map's name under its key in the game's texts, the
/// file's name in small print under it when that differs.
fn label_rows(texts: Option<Res<Texts>>, mut labels: Query<(&RowLabel, &mut Text)>) {
    for (label, mut text) in &mut labels {
        let name = texts.as_ref().and_then(|t| t.0.find(&label.map)).filter(|n| !n.trim().is_empty());
        let want = match (label.file, name) {
            (false, Some(name)) => name.trim(),
            (false, None) => &label.map,
            // The file's name, when the name above says something else.
            (true, Some(name)) if !same_name(name, &label.map) => &label.map,
            (true, _) => "",
        };
        if text.0 != want {
            text.0 = want.to_string();
        }
    }
}

/// Whether a text is the file's name spelled out (`No Time to Chill`,
/// `NoTimeToChill`): the same letters, ignoring case and spaces.
fn same_name(text: &str, file: &str) -> bool {
    let letters = |s: &str| s.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect::<String>();
    letters(text) == letters(file)
}

/// Scrolls the shown list so the chosen row is in it.
fn reveal(
    model: Res<MenuModel>,
    mut wanted: ResMut<Reveal>,
    mut lists: Query<(&ListNode, &mut ScrollPosition, &ComputedNode)>,
) {
    if !wanted.0 {
        return;
    }
    let Some((_, mut position, node)) = lists.iter_mut().find(|(l, _, _)| l.0 == model.tab()) else {
        return;
    };
    let view = node.size().y * node.inverse_scale_factor();
    if view <= 0.0 {
        // Not laid out yet.
        return;
    }
    wanted.0 = false;
    let top = model.selected(model.tab()) as f32 * (ROW_HEIGHT + ROW_GAP);
    let bottom = top + ROW_HEIGHT;
    if top < position.y {
        position.y = top;
    } else if bottom > position.y + view {
        position.y = bottom - view;
    }
}

/// `Esc` or the gamepad's Start while playing: the menu.
fn open_menu(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    touches: Res<Touches>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if *state.get() != GameState::Playing {
        return;
    }
    let tapped = touches.iter_just_pressed().any(|t| {
        let at = t.position();
        at.x >= BUTTON_AT.x
            && at.y >= BUTTON_AT.y
            && at.x <= BUTTON_AT.x + BUTTON_SIZE.x
            && at.y <= BUTTON_AT.y + BUTTON_SIZE.y
    });
    if keys.just_pressed(KeyCode::Escape) || pads.iter().any(|p| p.just_pressed(GamepadButton::Start)) || tapped {
        next.set(GameState::Menu);
    }
}

fn spawn_play_button(mut commands: Commands, font: Option<Res<GameFont>>, mut done: Local<bool>) {
    let Some(font) = font.filter(|_| !*done) else {
        return;
    };
    *done = true;
    commands.spawn((
        PlayButton,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(BUTTON_AT.x),
            top: Val::Px(BUTTON_AT.y),
            width: Val::Px(BUTTON_SIZE.x),
            height: Val::Px(BUTTON_SIZE.y),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
        GlobalZIndex(150),
        Pickable::IGNORE,
        children![(Text::new("Menu"), text_font(&font, 22.0), TextColor(Color::srgba(1.0, 1.0, 1.0, 0.8)))],
    ));
}

/// The button is there for a touch screen, which has no `Esc`.
fn show_play_button(
    state: Res<State<GameState>>,
    scheme: Option<Res<ActiveScheme>>,
    mut button: Query<&mut Node, With<PlayButton>>,
) {
    let want = if *state.get() == GameState::Playing && scheme.is_some_and(|s| *s == ActiveScheme::Touch) {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut button {
        if node.display != want {
            node.display = want;
        }
    }
}
