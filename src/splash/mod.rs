//! Splash screens, drawn: what `ShowSplash` shows over the game, a stack of
//! sprites, texts by key and bound texts (`SplashSetBindVariable`: the score
//! of Door's Dream, the friends left in `Swarm`, the coins in `WinnersRoom`),
//! fading in and out and swaying a little, with a countdown to the moment it
//! hides. The layouts are the C# game's (`assets/Splashes/Splashes.xml`, laid
//! out for 800 × 480) and the sprites its own.
//!
//! C# origin: `UIControls/SplashScreen.cs` (`Show`, `Hide`, `DrawNormal`,
//! `LoadSplashesData`, `GetSplashData`) and `RenderableWithTransition`.
//! Screen space, so it all runs in `Update` (real frame time, not the fixed
//! clock): `ShowSplash` hands over a [`SplashCommand`] from `FixedUpdate`.
//! The clock and the layout are plain data in [`screen`], unit-tested.
//!
//! What the campaign's maps use (every `ShowSplash` of `MapList.xml`'s
//! maps): `Blackout` (a black layer over the whole screen: the scene changes
//! under it), `Whiteout` (the same, white), `LetItSnowTimer` (only the
//! countdown, slot 0), `TimerAndCounter` (countdown, a gear and the coins),
//! `PongScore` (countdown and two scores), `ShowCount` (a zombie and the
//! friends left), `ReadyGo` (an arrow, "You", then "Ready" at 2 s and "Go" at
//! 4 s), `ControlsMovementInGame` and `ControlsWeaponInGame` (with the
//! keyboard's and the gamepad's variants) and `TapTextToClose`. So the
//! layers are sprites (some stretched to the screen, tinted, rotated),
//! texts by key in the game's font and language, bound texts, delayed
//! layers, layers swaying by their height, foreground layers.
//!
//! Branches of `SplashScreen`, one by one:
//! - `GetSplashData`: the variant of the splash for the last control used,
//!   `_Win` for keyboard (and mouse), `_GamePad` for a gamepad, none for
//!   touch, else the plain one: ported, by [`ActiveScheme`] when the splash
//!   is shown;
//! - `LoadSplashesData`: offsets from the screen's centre, sizes and
//!   offsets grown by the screen (`ScaleUpWhenNeeded`,
//!   `ScaleUpOnlyOffsetWhenNeeded`): ported ([`screen::placement`]), with
//!   `ButtonSizeMultiplier` 1;
//! - `Show` / `Init`: a new splash replaces the shown one at once, a fade in
//!   goes on, a shown or gone one fades in from the start, one hiding turns
//!   round; layers' `Activated` reset: ported ([`screen::Screen::show`]);
//! - `Hide`: fades out from the same brightness: ported;
//! - `DrawNormal`: the countdown in slot 0 (`"##0.0"` while held, empty
//!   otherwise), the sway (`HardasticVector2`, 16 px × parallax × height, a
//!   35 s cycle, none for `Static`), the layers' `DelaySec`, the depth order
//!   (`Height`, list order, `OnForeground`), sprite at its position with a
//!   zero origin and a counter-clockwise `DirectionRad`, stretched to the
//!   screen, `LocalizedText` and `BindTextId` in the Title font, colour
//!   times the fade: ported;
//! - `DrawOnAppearing` / `DrawOnHiding`, fade (no fade to black): ported.
//!
//! Differences and what is not ported:
//! - the splash's clock starts when the action runs; in C# the hold (the
//!   countdown, `AutoHideTimeSec`, and so `ActionOnHide`) started when the
//!   fade in had finished. Here the countdown reaches 0 and the splash
//!   starts to fade out exactly when `ActionOnHide` runs (see
//!   `actions::splash`), so a timer on the screen is the timer of the map;
//! - `IgnorePause` (the splash's clock stopping while the game is paused):
//!   `ActionOnHide`'s timer runs through pauses, so the splash does too;
//! - taps (`Clickable`, `ControlHandler_Pressed`, `OnClick`, the "Tap to
//!   continue" indicator and `ShowClickableIndicatorAfterSec`) and
//!   `SplashWaitInput`: no splash of the campaign is clickable;
//! - `AppearFadeToBlack` (the menus' fade through black with the squares of
//!   `DrawTrans`), `DrawBackground` with the menu's foreground picture
//!   (`SplashForeground`), `SetBackhroundSplash`, `BackgroundSplashVisible`,
//!   `Pause`, `TransitionStartPoint`: the menus' and the level start's, none
//!   of the campaign's splashes sets them (`ShowSplash` shows splashes that
//!   all fade and draw no background); a splash that does is shown as a
//!   plain fade;
//! - the `CaptureScreen` layer (the screen as a picture), a layer's `Sound`
//!   (`ActivateLayer`'s `PlayOnceSoundByName`), `MainTextWidth` (a layer with
//!   both a text and a bound text puts the bound text after the text; a
//!   layer here draws both from the same corner): no splash of the campaign
//!   uses them; `CreateButton` and the clickable `EndLevel` buttons are menu
//!   code;
//! - the accelerometer's tilt (`DeviceAccelerometer`) adds nothing to the
//!   sway (a desktop has none);
//! - the Title font is `FontX2` (30 pt), drawn here in the game's own font,
//!   `Pixar Two`, at 40 px; the high-definition `FontX4` at half size is the
//!   same size;
//! - `ButtonSizeMultiplier` (a phone's dpi correction) is 1, and on a portrait
//!   screen the splash is laid out on the screen as it is, not turned with
//!   the game's view;
//! - the text's `Drawer.Fonts.Scale` and `LocalizedText` are measured when
//!   C# reinitialises strings (a language change); the text is looked up
//!   again every frame here.

pub mod screen;

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::actions::{SplashBindText, SplashCommand};
use crate::assets::{XmlText, sprite_path};
use crate::content::splashes::SplashesData;
use crate::controls::ActiveScheme;
use crate::game::{GameState, MapEntity};
use crate::info::Texts;
use screen::{Phase, Screen, corner_pivot_shift, draw_order, placement, straight_tint};

/// The Title font (`FontX2`, 30 pt at 96 dpi) in pixels.
pub const TITLE_PX: f32 = 40.0;

/// Over the HUD's text and the info panel.
const Z: i32 = 100;

/// Splash screen state: what is shown, how far along.
#[derive(Resource, Debug, Default)]
pub struct SplashState(pub Screen);

/// The splashes, once `Splashes.xml` has loaded.
#[derive(Resource, Debug)]
pub struct Splashes(pub SplashesData);

/// `Splashes.xml` while it loads.
#[derive(Resource)]
struct SplashesFile(Handle<XmlText>);

/// The sprites the splashes name, by lower-case content name.
#[derive(Resource, Default)]
struct SplashSprites(HashMap<String, Handle<Image>>);

#[derive(Resource)]
struct SplashFont(Handle<Font>);

#[derive(Component)]
struct SplashRoot;

/// One thing a layer draws: C# drew a layer's sprite, text and bound text
/// one after another at the same depth.
#[derive(Component, Clone, Debug)]
struct Piece {
    layer: usize,
    kind: Kind,
}

#[derive(Clone, Debug)]
enum Kind {
    Sprite(Handle<Image>),
    Text(String),
    Bound(usize),
}

pub struct SplashPlugin;

impl Plugin for SplashPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SplashState>()
            .init_resource::<SplashSprites>()
            .add_systems(Startup, load)
            .add_systems(Update, read_file.run_if(resource_exists::<SplashesFile>))
            .add_systems(OnEnter(GameState::Playing), reset)
            .add_systems(OnExit(GameState::Playing), reset)
            .add_systems(
                Update,
                (commands, advance, draw)
                    .chain()
                    .run_if(in_state(GameState::Playing))
                    .run_if(resource_exists::<Splashes>),
            );
    }
}

fn load(mut commands: Commands, server: Res<AssetServer>) {
    commands.insert_resource(SplashesFile(server.load("Splashes/Splashes.xml")));
    commands.insert_resource(SplashFont(server.load("Fonts/Pixar Two.ttf")));
}

/// Parses `Splashes.xml` and starts loading the sprites it names.
fn read_file(
    mut commands: Commands,
    file: Res<SplashesFile>,
    assets: Res<Assets<XmlText>>,
    server: Res<AssetServer>,
    mut sprites: ResMut<SplashSprites>,
) {
    let Some(xml) = assets.get(&file.0) else {
        return;
    };
    commands.remove_resource::<SplashesFile>();
    let data = match SplashesData::parse(&xml.0) {
        Ok(data) => data,
        Err(e) => {
            error!("Splashes.xml: {e}");
            return;
        }
    };
    // Only the splashes the campaign shows have their sprites imported.
    for layer in data.splashes.iter().flat_map(|s| s.layers.iter()) {
        let Some(name) = layer.sprite_name() else {
            continue;
        };
        if let Some(path) = sprite_path(name) {
            sprites.0.entry(name.to_ascii_lowercase()).or_insert_with(|| server.load(path));
        }
    }
    commands.insert_resource(Splashes(data));
}

/// A map starts or ends with no splash and its slot 0 empty.
fn reset(mut state: ResMut<SplashState>) {
    state.0.clear();
}

/// `SplashScreen.Show` and `HideScreen`.
fn commands(
    mut commands: Commands,
    mut requests: MessageReader<SplashCommand>,
    mut state: ResMut<SplashState>,
    (splashes, scheme): (Res<Splashes>, Res<ActiveScheme>),
    (sprites, font): (Res<SplashSprites>, Res<SplashFont>),
    roots: Query<Entity, With<SplashRoot>>,
) {
    for request in requests.read() {
        match request {
            SplashCommand::Hide => state.0.hide(),
            SplashCommand::Show { name, auto_hide } => {
                let suffix = match *scheme {
                    ActiveScheme::Touch => "",
                    ActiveScheme::Keyboard => "_Win",
                    ActiveScheme::Gamepad => "_GamePad",
                };
                let Some(data) = splashes.0.variant(name, suffix) else {
                    // C#'s `GetSplashData` returned null and threw.
                    warn!("ShowSplash: no splash called {name}");
                    continue;
                };
                state.0.show(data.clone(), *auto_hide);
                for root in &roots {
                    commands.entity(root).despawn();
                }
                spawn(&mut commands, data, &sprites, &font);
            }
        }
    }
}

/// The root and one node for each thing each layer draws, back to front.
fn spawn(commands: &mut Commands, data: &crate::content::splashes::SplashData, sprites: &SplashSprites, font: &SplashFont) {
    let layers = &data.layers.items;
    let mut pieces = Vec::new();
    for (rank, &i) in draw_order(data).iter().enumerate() {
        let layer = &layers[i];
        let mut kinds = Vec::new();
        if let Some(name) = layer.sprite_name() {
            match sprites.0.get(&name.to_ascii_lowercase()) {
                Some(handle) => kinds.push(Kind::Sprite(handle.clone())),
                None => warn!("splash {}: no sprite {name}", data.name),
            }
        }
        if let Some(key) = layer.text_key() {
            kinds.push(Kind::Text(key.to_string()));
        }
        if layer.bind_text_id >= 0 {
            kinds.push(Kind::Bound(layer.bind_text_id as usize));
        }
        for (n, kind) in kinds.into_iter().enumerate() {
            pieces.push((i, kind, (rank * 3 + n) as i32));
        }
    }
    commands
        .spawn((
            Name::new(format!("Splash {}", data.name)),
            SplashRoot,
            MapEntity,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GlobalZIndex(Z),
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            for (layer, kind, z) in pieces {
                let node = Node { position_type: PositionType::Absolute, ..default() };
                let common = (Piece { layer, kind: kind.clone() }, ZIndex(z), UiTransform::IDENTITY, Visibility::Hidden, Pickable::IGNORE);
                match kind {
                    Kind::Sprite(handle) => {
                        root.spawn((common, node, ImageNode::new(handle)));
                    }
                    Kind::Text(_) | Kind::Bound(_) => {
                        root.spawn((
                            common,
                            node,
                            Text::new(""),
                            TextFont { font: font.0.clone().into(), font_size: FontSize::Px(TITLE_PX), ..default() },
                            TextColor(Color::WHITE),
                            TextLayout::no_wrap(),
                        ));
                    }
                }
            }
        });
}

/// Goes on in time, and writes the countdown to slot 0 of the bound texts.
fn advance(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<SplashState>,
    mut bound: ResMut<SplashBindText>,
    roots: Query<Entity, With<SplashRoot>>,
) {
    if !state.0.visible() {
        return;
    }
    state.0.update(time.delta_secs());
    let countdown = state.0.countdown();
    if bound.get(0) != Some(countdown.as_str()) {
        bound.set(0, countdown);
    }
    if !state.0.visible() {
        for root in &roots {
            commands.entity(root).despawn();
        }
    }
}

type PieceData = (
    &'static Piece,
    &'static mut Node,
    &'static mut UiTransform,
    &'static mut Visibility,
    Option<&'static mut ImageNode>,
    Option<(&'static mut Text, &'static mut TextColor, &'static mut TextFont)>,
    &'static ComputedNode,
);

/// Puts every node where `screen` says, with the fade and the texts.
fn draw(
    state: Res<SplashState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    images: Res<Assets<Image>>,
    texts: Res<Texts>,
    bound: Res<SplashBindText>,
    mut pieces: Query<PieceData>,
) {
    let (Some(splash), Ok(window)) = (state.0.splash(), windows.single()) else {
        return;
    };
    let view = Vec2::new(window.width(), window.height());
    let alpha = if state.0.phase() == Phase::Invisible { 0.0 } else { state.0.alpha() };
    for (piece, mut node, mut transform, mut visibility, image, text, computed) in &mut pieces {
        let layer = &splash.layers.items[piece.layer];
        let place = placement(splash, layer, piece.layer, view, state.0.sway());
        let tint = straight_tint(layer.color, alpha);
        let color = Color::srgba(tint[0], tint[1], tint[2], tint[3]);
        let mut shown = state.0.layer_active(piece.layer);

        // The size the piece has now: the sprite's own, or what the text
        // laid out to (a frame late).
        let size = match (&piece.kind, image, text) {
            (Kind::Sprite(handle), Some(mut image), _) => {
                image.color = color;
                match images.get(handle) {
                    Some(i) => {
                        let size = if place.stretch { view } else { i.size_f32() * place.scale };
                        node.width = Val::Px(size.x);
                        node.height = Val::Px(size.y);
                        size
                    }
                    None => {
                        shown = false;
                        Vec2::ZERO
                    }
                }
            }
            (kind, _, Some((mut text, mut text_color, mut font))) => {
                let line = match kind {
                    Kind::Text(key) => Some(texts.0.get(key)),
                    Kind::Bound(slot) => bound.get(*slot).map(str::to_string),
                    Kind::Sprite(_) => None,
                };
                shown &= line.is_some();
                if let Some(line) = line.filter(|l| text.0 != *l) {
                    text.0 = line;
                }
                text_color.0 = color;
                let px = FontSize::Px(TITLE_PX * place.scale);
                if font.font_size != px {
                    font.font_size = px;
                }
                computed.size() * computed.inverse_scale_factor()
            }
            _ => Vec2::ZERO,
        };

        let origin = if place.stretch { Vec2::ZERO } else { place.top_left };
        node.left = Val::Px(origin.x);
        node.top = Val::Px(origin.y);
        // C# turned a sprite about its top left corner, counter-clockwise.
        let angle = -layer.direction_rad;
        let shift = corner_pivot_shift(size, angle);
        transform.rotation = Rot2::radians(angle);
        transform.translation = Val2::px(shift.x, shift.y);
        let wanted = if shown && alpha > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}
