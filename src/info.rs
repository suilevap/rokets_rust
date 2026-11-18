//! Texts shown over the game (`UIManager.ShowInfo`, made by `ShowText`):
//! lines found by key in the game's texts (`assets/Strings/Strings.resx`,
//! with a [`Language`]'s file over it, `Strings.ru.resx`), drawn in the
//! game's font (`Pixar Two`, `FontX*`), for `ShowTimeSec` (7 s when not given), the physics
//! paused meanwhile with `PauseGame`. A newer text replaces the shown one.
//!
//! Not ported: the C# panel's look and its slide in, tapping it away.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::actions::{AddMapAction, Run};
use crate::assets::XmlText;
use crate::content::actions::defs::ShowText;
use crate::content::strings::Strings;
use crate::game::GameState;

/// `ShowInfo`'s time when the action gives none.
const DEFAULT_SECS: f32 = 7.0;

/// The game's texts by key, once loaded.
#[derive(Resource, Debug, Default)]
pub struct Texts(pub Strings);

/// The language of the texts (`ru`, `de`, `es`, `it`, `be`); none:
/// English.
#[derive(Resource, Debug, Default, Clone)]
pub struct Language(pub Option<String>);

/// The texts files being loaded: the default, and the language's.
#[derive(Resource)]
struct TextsFile(Handle<XmlText>, Option<Handle<XmlText>>);

/// The game's font, for texts over the game.
#[derive(Resource)]
struct GameFont(Handle<Font>);

/// The text on screen now: seconds left, and whether it paused the game.
#[derive(Resource, Debug, Default)]
pub struct Shown {
    pub text: Option<String>,
    left: f32,
    paused: bool,
}

#[derive(Component)]
struct InfoText;

/// Behind the text, while there is one.
const BACKDROP: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);

pub struct InfoPlugin;

impl Plugin for InfoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Texts>()
            .init_resource::<Language>()
            .init_resource::<Shown>()
            .add_map_action::<ShowText, _>(show_text)
            .add_systems(Startup, (load_texts, spawn_panel).chain())
            .add_systems(Update, read_texts.run_if(resource_exists::<TextsFile>))
            .add_systems(OnEnter(GameState::Playing), |mut shown: ResMut<Shown>| *shown = Shown::default())
            .add_systems(Update, (count_down, show).chain());
    }
}

fn load_texts(mut commands: Commands, server: Res<AssetServer>, language: Res<Language>) {
    let lang = language.0.as_ref().map(|l| server.load(format!("Strings/Strings.{l}.resx")));
    commands.insert_resource(TextsFile(server.load("Strings/Strings.resx"), lang));
    commands.insert_resource(GameFont(server.load("Fonts/Pixar Two.ttf")));
}

fn read_texts(
    mut commands: Commands,
    file: Res<TextsFile>,
    server: Res<AssetServer>,
    assets: Res<Assets<XmlText>>,
    mut texts: ResMut<Texts>,
) {
    let Some(base) = assets.get(&file.0) else {
        return;
    };
    let read = |xml: &XmlText, name: &str| {
        Strings::parse(&xml.0).map_err(|e| warn!("{name}: {e}")).unwrap_or_default()
    };
    let mut strings = read(base, "Strings.resx");
    if let Some(lang) = &file.1 {
        match assets.get(lang) {
            Some(xml) => strings = read(xml, "language texts").over(strings),
            None if matches!(server.get_load_state(lang), Some(bevy::asset::LoadState::Failed(_))) => {
                warn!("no texts for that language; English");
            }
            None => return,
        }
    }
    texts.0 = strings;
    commands.remove_resource::<TextsFile>();
}

fn show_text(
    In(run): In<Run<ShowText>>,
    texts: Res<Texts>,
    mut shown: ResMut<Shown>,
    mut physics: Option<ResMut<Time<Physics>>>,
) -> bool {
    let d = run.data;
    if d.text.is_empty() {
        return false;
    }
    let lines: Vec<String> = d.text.iter().map(|k| texts.0.get(k)).collect();
    if shown.paused && !d.pause_game && let Some(p) = physics.as_mut() {
        p.unpause();
    }
    *shown = Shown {
        text: Some(lines.join("\n")),
        left: if d.show_time_sec > 0.0 { d.show_time_sec } else { DEFAULT_SECS },
        paused: d.pause_game,
    };
    if d.pause_game && let Some(p) = physics.as_mut() {
        p.pause();
    }
    true
}

fn count_down(time: Res<Time>, mut shown: ResMut<Shown>, mut physics: Option<ResMut<Time<Physics>>>) {
    if shown.text.is_none() {
        return;
    }
    // Game time: a pause stops only the physics, so it counts on.
    shown.left -= time.delta_secs();
    if shown.left <= 0.0 {
        if shown.paused && let Some(p) = physics.as_mut() {
            p.unpause();
        }
        *shown = Shown::default();
    }
}

fn spawn_panel(mut commands: Commands, font: Res<GameFont>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(18.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_child((
            InfoText,
            Text::new(" "),
            TextFont {
                font: font.0.clone().into(),
                font_size: FontSize::Px(28.0),
                ..default()
            },
            TextLayout::justify(Justify::Center),
            TextColor(Color::WHITE),
            BackgroundColor(Color::NONE),
            Node {
                padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                ..default()
            },
        ));
}

/// The text, or a blank on a clear box.
fn show(shown: Res<Shown>, mut panel: Query<(&mut Text, &mut BackgroundColor), With<InfoText>>) {
    if !shown.is_changed() {
        return;
    }
    for (mut text, mut back) in &mut panel {
        text.0 = shown.text.clone().unwrap_or_else(|| " ".into());
        back.0 = if shown.text.is_some() { BACKDROP } else { Color::NONE };
    }
}
