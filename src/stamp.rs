//! The stamp that `ShowSuccess` puts on the screen: "door opened",
//! "objective done", "wrong button". A picture (`Particles/Success`,
//! `SuccessDoor` or `SuccessAnti`) that appears a third of the way into
//! the window from its top left, tilted a little, and fades over three
//! seconds, with a second copy that swells to one and a half times its
//! size and fades in half a second, as it lands.
//!
//! C# origin: `ParticleSystemManager.CreateSuccessUI`,
//! `CreateSuccessDoorUI` and `CreateAntiSuccessUI` (the UI particle pool,
//! `_success*UIStyle` and `_success*UIStyle2`), called by
//! `UIManager.ShowSuccess`. The styles are `particles::styles::SUCCESS_*`;
//! the action that makes the stamps is `actions::success`.
//!
//! Bevy UI in `Update`, in screen space (as the splashes and the player
//! HUD). A [`Stamp`] is one such particle, an entity of its own that the
//! map owns, so a stamp does not outlive a restart.
//!
//! Not ported: the C# pool is shared with the other UI particles and
//! fixed in size (`MaxParticleCountUI`), so many stamps at once could push
//! older ones out; each stamp here is an entity that lives its time.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::assets::sprite_path;
use crate::content::actions::defs::SuccessIndicator;
use crate::game::{GameState, MapEntity};
use crate::particles::Style;
use crate::particles::styles::{ANTI_SUCCESS_STAMP, SUCCESS_DOOR_STAMP, SUCCESS_STAMP};

/// Over the player HUD (80) and the splashes (100).
const Z: i32 = 110;

/// The texture's side in the C# game's pixels; all three are square.
const SIDE: f32 = 128.0;

/// How far right of the third of the window a stamp may land, pixels
/// (`_successPositionInitial + UnitX * 64 * NextFloatFake()`).
pub const SCATTER: f32 = 64.0;

/// The two halves of a stamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Half {
    /// Holds its size, fades in three seconds.
    Hold,
    /// Swells to one and a half times, fades in half a second.
    Swell,
}

/// One particle of a success stamp.
#[derive(Component, Clone, Debug)]
pub struct Stamp {
    pub kind: SuccessIndicator,
    pub half: Half,
    /// Pixels right of a third of the window's width.
    pub offset_x: f32,
    /// The picture's turn, radians (both halves the same).
    pub tilt: f32,
    pub age: f32,
}

impl Stamp {
    pub fn style(&self) -> &'static Style {
        let styles = match self.kind {
            SuccessIndicator::Standard => &SUCCESS_STAMP,
            SuccessIndicator::DoorOpen => &SUCCESS_DOOR_STAMP,
            SuccessIndicator::AntiSuccess => &ANTI_SUCCESS_STAMP,
        };
        &styles[self.half as usize]
    }
}

/// The three pictures a stamp uses.
pub const SPRITES: [&str; 3] = [SUCCESS_STAMP[0].texture, SUCCESS_DOOR_STAMP[0].texture, ANTI_SUCCESS_STAMP[0].texture];

pub struct StampPlugin;

impl Plugin for StampPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress, animate).chain().run_if(in_state(GameState::Playing)));
    }
}

/// Gives a new stamp its node and picture.
fn dress(mut commands: Commands, server: Res<AssetServer>, new: Query<(Entity, &Stamp), Added<Stamp>>) {
    for (entity, stamp) in &new {
        commands.entity(entity).insert((
            Name::new("Success stamp"),
            MapEntity,
            Node { position_type: PositionType::Absolute, ..default() },
            ImageNode {
                image: server.load(sprite_path(stamp.style().texture)),
                image_mode: NodeImageMode::Stretch,
                ..default()
            },
            UiTransform::default(),
            Visibility::Hidden,
            GlobalZIndex(Z),
            Pickable::IGNORE,
        ));
    }
}

fn animate(
    mut commands: Commands,
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    images: Res<Assets<Image>>,
    mut stamps: Query<(Entity, &mut Stamp, &mut Node, &mut ImageNode, &mut UiTransform, &mut Visibility)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let screen = Vec2::new(window.width(), window.height());
    // `UISizeMultiplier`.
    let k = (screen.x / 800.0).min(screen.y / 480.0);
    let dt = time.delta_secs();
    for (entity, mut stamp, mut node, mut image, mut transform, mut visibility) in &mut stamps {
        // A picture that has not loaded has not started its life.
        if !images.contains(&image.image) {
            continue;
        }
        stamp.age += dt;
        let style = stamp.style();
        if stamp.age >= style.lifetime {
            commands.entity(entity).despawn();
            continue;
        }
        let state = style.state_at(stamp.age);
        let size = Vec2::splat(SIDE) * state.scale * k;
        let centre = screen / 3.0 + Vec2::X * stamp.offset_x;
        node.left = Val::Px(centre.x - size.x / 2.0);
        node.top = Val::Px(centre.y - size.y / 2.0);
        node.width = Val::Px(size.x);
        node.height = Val::Px(size.y);
        transform.rotation = Rot2::radians(stamp.tilt);
        let c = state.color;
        image.color = Color::srgba(c[0], c[1], c[2], c[3]);
        *visibility = Visibility::Inherited;
    }
}
