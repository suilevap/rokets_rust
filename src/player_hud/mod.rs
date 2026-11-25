//! The HUD the player sees all through a level: the health cells and the
//! shield's thin cells at the top, the gadget's and the weapon's icons
//! left of them, and the battery of lives in the middle of the screen when
//! one is spent. Bevy UI in `Update` (screen space, as the splashes), the
//! layout is [`model`] (plain data, tested). The energy bar under the
//! cells is `weapon::view`'s.
//!
//! C# origin: `PlayerCarControlBase.DrawUI` (`DrawHealth`, `DrawGadgetIcon`,
//! `DrawIcon`, the shield half of `DrawHealth`) from `UIManager
//! .DrawControlUI`, and `ParticleSystemManager.ShowEnergy`
//! (`GameModeSinglePlayer.UpdateLives`, `_battery*UiStyle`,
//! `_healthBarDamageUiStyle`).
//!
//! What C# draws, and the branches of it:
//! - `DrawControlUI`: only for a car with `Visible`, `LocalDevice`,
//!   `Settings.DrawCarControl` and no `World.Pause`; `DrawUI` only while
//!   the car is alive: ported (the local player's car, physics not
//!   paused; a wreck has lost `LocalPlayer`). A car without control
//!   (`NoInput`, C#'s `WhoHasLocalControl` lacks it) shows nothing.
//! - `DrawHealth`: one cell of 24 × 16 for 4 health points, 0.75 white;
//!   the cells just lost fade to red and then dark over 2.5 s while red
//!   cells drop away (`CreateHealthBarDamage`), cells gained fade from
//!   green, the last cell shows its part as a smaller cell, and a
//!   brighter shimmer runs along the bar (`highlightN`): all ported
//!   ([`model::health_cells`], [`model::HealthTrail`]).
//! - the shield (`ControlledEntity.Shield`): thin cells after the health
//!   cells, 4 points each, the whole ones lit, the rest faint, the part
//!   that is growing back as a part of a cell's height; flashing yellow
//!   and black while it is down: ported ([`model::shield_cells`]).
//! - `DrawGadgetIcon`: only when the last device was not touch
//!   (`LastUserControlType != Touch`); the gadget's icon (the magnet's or
//!   the accelerator's), then the weapon's (every gun has the same), each
//!   at the cell's height, 0.75 white, the gadget's turned 45 degrees
//!   counter-clockwise: ported ([`model::icons`]).
//! - `ShowEnergy(lives)`: a battery fading over 3 s with a white bar per
//!   life left and a red one (1 s) for the one spent, a red battery when
//!   none were left: ported ([`model::lives_fx`]); it is drawn whatever
//!   the car is doing, over everything.
//!
//! Not ported:
//! - `PlayerUIOffset`, which moves the rows aside in hot-seat games with
//!   several local players: one local player here;
//! - the heal particle (`CreateHealthBarHeal` is never called in C#), and
//!   the damage particle's spin (`ViewRotation = 3`, an almost symmetric
//!   rectangle);
//! - the larger `MaxHealth` of easy mode (`EasyModeMaxHealthDelta`, the
//!   port has no easy mode);
//! - the score and combo (`ScoreManager.Draw`) and the touch screen's own
//!   pictures (the port draws no touch controls yet);
//! - the gadget icon of `PreciseGrabGadget` and the `Bomb` (not ported).
//!
//! The icons and the health row are laid out for 800 × 480 and grow with
//! the smaller of the window's width and height (`UISizeMultiplier`).

pub mod model;

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use avian2d::prelude::{Physics, PhysicsTime};

use crate::actions::Respawned;
use crate::assets::sprite_path;
use crate::car::shield::Shield;
use crate::control_highlight::triangle;
use crate::controls::{ActiveScheme, LocalPlayer, NoInput};
use crate::explosion::Health;
use crate::game::{GameState, MapEntity};
use crate::game_mode::LifeSpent;
use crate::gadget::{Magnet, accelerator::Accelerator};
use crate::weapon::Weapon;
use model::{Fx, Gear, HealthTrail, Item};

/// Under the control hints (90), over the world.
const Z: i32 = 80;

#[derive(Resource, Default)]
struct HudSprites(HashMap<&'static str, Handle<Image>>);

/// What the HUD remembers between frames.
#[derive(Resource, Default)]
struct HudState {
    /// The car the trail belongs to.
    trail: Option<(Entity, HealthTrail)>,
    /// Pictures that fade, and their age.
    fx: Vec<(Fx, f32)>,
}

#[derive(Component)]
struct HudNode(usize);

pub struct PlayerHudPlugin;

impl Plugin for PlayerHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudSprites>()
            .init_resource::<HudState>()
            .add_systems(Startup, load)
            .add_systems(Update, draw.run_if(in_state(GameState::Playing)));
    }
}

fn load(mut sprites: ResMut<HudSprites>, server: Res<AssetServer>) {
    for name in model::SPRITES {
        sprites.0.insert(name, server.load(sprite_path(name)));
    }
}

type Player = (
    Entity,
    &'static Health,
    Option<&'static Shield>,
    Has<Magnet>,
    Has<Accelerator>,
    Has<Weapon>,
    Has<Respawned>,
);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    mut commands: Commands,
    time: Res<Time>,
    (physics, scheme, sprites, images): (Res<Time<Physics>>, Res<ActiveScheme>, Res<HudSprites>, Res<Assets<Image>>),
    windows: Query<&Window, With<PrimaryWindow>>,
    player: Query<Player, (With<LocalPlayer>, Without<NoInput>)>,
    mut spent: MessageReader<LifeSpent>,
    mut state: ResMut<HudState>,
    mut nodes: Query<(Entity, &HudNode, &mut Node, &mut ImageNode, &mut UiTransform, &mut Visibility)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let screen = Vec2::new(window.width(), window.height());
    let k = (screen.x / 800.0).min(screen.y / 480.0);
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    // Every picture must have loaded to be measured.
    let mut sizes = HashMap::new();
    for (name, handle) in &sprites.0 {
        let Some(image) = images.get(handle) else {
            return;
        };
        sizes.insert(*name, image.size_f32());
    }
    let size = |name: &str| sizes.get(name).copied().unwrap_or(Vec2::ONE);

    let mut items: Vec<Item> = Vec::new();
    let health = player.iter().next().filter(|_| !physics.is_paused());
    if let Some((car, hp, shield, magnet, accelerator, weapon, respawned)) = health {
        if state.trail.as_ref().is_none_or(|(e, _)| *e != car) {
            // A car that came back fills up from nothing.
            state.trail = Some((car, HealthTrail::new(if respawned { 0 } else { hp.max })));
        }
        let trail = &mut state.trail.as_mut().unwrap().1;
        let step = trail.step(now, hp.hp);
        let previous = trail.previous;
        for cell in step.lost {
            state.fx.push((model::damage_fx(screen.x, k, hp.max, cell), 0.0));
        }
        let phase = (now * 1000.0 % 6280.0) / 6280.0;
        items.extend(model::health_cells(screen.x, k, hp.max, hp.hp, previous, step.remain, phase));
        if let Some(shield) = shield {
            items.extend(model::shield_cells(
                screen.x,
                k,
                hp.max,
                shield.max() as i32,
                shield.float_value(),
                triangle(now),
            ));
        }
        // `LastUserControlType != Touch`.
        if *scheme != ActiveScheme::Touch {
            items.extend(model::icons(screen.x, k, hp.max, Gear { magnet, accelerator, weapon }, size));
        }
    }
    for LifeSpent { lives } in spent.read() {
        state.fx.extend(model::lives_fx(screen, k, *lives).into_iter().map(|f| (f, 0.0)));
    }
    for (fx, age) in &mut state.fx {
        *age += dt;
        items.extend(fx.at(*age));
    }
    state.fx.retain(|(fx, age)| *age < fx.life);

    // Grow the pool of nodes, then put the items on them.
    let have = nodes.iter().count();
    for i in have..items.len() {
        commands.spawn((
            Name::new("Player HUD item"),
            HudNode(i),
            MapEntity,
            Node { position_type: PositionType::Absolute, ..default() },
            ImageNode { image_mode: NodeImageMode::Stretch, ..default() },
            UiTransform::default(),
            Visibility::Hidden,
            GlobalZIndex(Z),
            ZIndex(i as i32),
            Pickable::IGNORE,
        ));
    }
    for (_, index, mut node, mut image, mut transform, mut visibility) in &mut nodes {
        let Some(item) = items.get(index.0) else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        let top_left = item.center - item.size / 2.0;
        node.left = Val::Px(top_left.x);
        node.top = Val::Px(top_left.y);
        node.width = Val::Px(item.size.x);
        node.height = Val::Px(item.size.y);
        if let Some(handle) = sprites.0.get(item.sprite)
            && image.image != *handle
        {
            image.image = handle.clone();
        }
        image.color = Color::srgba(item.color[0], item.color[1], item.color[2], item.color[3]);
        transform.rotation = Rot2::radians(item.rotation);
        *visibility = Visibility::Inherited;
    }
}
