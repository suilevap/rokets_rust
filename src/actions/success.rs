//! `ShowSuccess`: the stamp on the screen ("door opened", "objective
//! done", "wrong button"), and its sound. It does not end the level; that
//! is `MoveToNextMap` (`actions::level`).
//!
//! C# origin: `ActionRunner.Run(ShowSuccess)`, which only called
//! `UIManager.ShowSuccess(action.SuccessType)`: for `Standard`,
//! `DoorOpen` and `AntiSuccess` a stamp particle (`CreateSuccessUI`,
//! `CreateSuccessDoorUI`, `CreateAntiSuccessUI`) at a third of the
//! window's width and height plus 0 to 64 pixels to the right, tilted by
//! up to 22.5 degrees either way; always `SoundManager.Stamp`, at the
//! listener. The stamp is two [`Stamp`] particles, drawn by `stamp`.
//!
//! Not ported: C# drew nothing for a `SuccessType` it did not know, and
//! played the sound all the same; the type here is an enum of the three,
//! so there is no such case. The random numbers are not C#'s table of
//! fake randoms (`NextFloatFake`).

use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use super::{AddMapAction, Run};
use crate::content::actions::defs::ShowSuccess;
use crate::sound::PlaySound;
use crate::stamp::{Half, SCATTER, Stamp};

pub struct SuccessActionsPlugin;

impl Plugin for SuccessActionsPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<Messages<PlaySound>>() {
            app.add_message::<PlaySound>();
        }
        app.add_map_action::<ShowSuccess, _>(show_success);
    }
}

fn show_success(
    In(run): In<Run<ShowSuccess>>,
    mut commands: Commands,
    mut rng: Local<Option<SmallRng>>,
    mut play: MessageWriter<PlaySound>,
) -> bool {
    let rng = rng.get_or_insert_with(|| SmallRng::seed_from_u64(7));
    let offset_x = SCATTER * rng.random::<f32>();
    let tilt = rng.random_range(-1.0..1.0) * std::f32::consts::PI / 8.0;
    for half in [Half::Hold, Half::Swell] {
        commands.spawn(Stamp { kind: run.data.success_type, half, offset_x, tilt, age: 0.0 });
    }
    play.write(PlaySound { name: "Stamp", at: None, pitch: 0.0, volume: 1.0 });
    true
}
