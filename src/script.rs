//! Scripted driving, for comparing the car with the C# game: a sequence of
//! stick positions held for a time, run in the fixed 60 Hz steps, with the
//! car's position, angle and speed logged every 0.1 s.
//!
//! Format: `x,y:seconds;x,y:seconds` with the stick in world directions
//! (y up), e.g. `1,0:2;0,1:1;0,0:1` (right for 2 s, up for 1 s, let go).
//! Values after the stick hold buttons meanwhile: `g` the gadget, aimed
//! ahead of the car (`0,0,g:1` shoots the magnet out for a second), `f`
//! the trigger, the gun firing where it points (`1,0,f:2`); the Railgun
//! fires on the first step of it, as on the fire key's press. `--run NAME`
//! runs a map action on the script's first step (what the C# game's debug
//! server does with `/run?cmd=NAME`), e.g. an AI's `MoveCargoOrder`.

use bevy::prelude::*;

use crate::car::drive_cars;
use crate::controls::{LocalPlayer, NoInput, PlayerIntent};
use crate::game::GameState;

/// The buttons a script step holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buttons {
    pub gadget: bool,
    pub fire: bool,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct DriveScript {
    /// Stick, buttons held, steps.
    steps: Vec<(Vec2, Buttons, u32)>,
    tick: u32,
    /// The trigger was held at the last step (an accurate shot is asked
    /// for when it was not).
    fire_held: bool,
    /// Map ids to log every 0.1 s (`--trace`), as the C# game's `/trace`.
    pub trace: Vec<i64>,
    /// Map actions to run on the script's first step (`--run`), as the C#
    /// game's debug server `/run?cmd=`.
    pub run: Vec<String>,
}

impl DriveScript {
    pub fn parse(s: &str) -> Option<Self> {
        let steps = s
            .split(';')
            .filter(|p| !p.trim().is_empty())
            .map(|part| {
                let (dir, secs) = part.split_once(':')?;
                let mut parts = dir.split(',');
                let (x, y) = (parts.next()?, parts.next()?);
                let mut buttons = Buttons::default();
                for flag in parts {
                    match flag.trim() {
                        "g" => buttons.gadget = true,
                        "f" => buttons.fire = true,
                        _ => return None,
                    }
                }
                let v = Vec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?);
                let secs: f32 = secs.trim().parse().ok()?;
                Some((v.clamp_length_max(1.0), buttons, (secs * 60.0).round() as u32))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(DriveScript { steps, tick: 0, fire_held: false, trace: Vec::new(), run: Vec::new() })
    }

    /// The stick and the buttons at the current tick; `None` once the
    /// script has ended.
    fn now(&self) -> Option<(Vec2, Buttons)> {
        let mut t = self.tick;
        for &(v, g, n) in &self.steps {
            if t < n {
                return Some((v, g));
            }
            t -= n;
        }
        None
    }
}

pub struct ScriptPlugin;

impl Plugin for ScriptPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            run_script
                .before(drive_cars)
                .before(crate::weapon::WeaponSystems)
                .before(crate::gadget::GadgetSystems)
                .run_if(in_state(GameState::Playing))
                .run_if(resource_exists::<DriveScript>),
        );
    }
}

type ScriptedCar<'a> = (&'a mut PlayerIntent, &'a Transform, &'a avian2d::prelude::LinearVelocity, Has<NoInput>);

fn run_script(
    mut script: ResMut<DriveScript>,
    mut run: MessageWriter<crate::actions::RunAction>,
    mut cars: Query<ScriptedCar, With<LocalPlayer>>,
    traced: Query<(&crate::game::MapId, &Transform, Option<&avian2d::prelude::LinearVelocity>, Option<&crate::explosion::Health>)>,
) {
    let Some((stick, buttons)) = script.now() else {
        return;
    };
    if script.tick == 0 {
        for name in &script.run {
            run.write(crate::actions::RunAction { name: name.clone(), actor: None });
        }
    }
    // `trace t id x y angle speed`, in the C# game's units and axes (y
    // down, angles the other way) for `tools/compare.py`.
    if script.tick.is_multiple_of(6) {
        // By map id: the query's order is the order things were made in.
        let mut lines: Vec<_> = traced.iter().filter(|(id, ..)| script.trace.contains(&id.0)).collect();
        lines.sort_by_key(|(id, ..)| id.0);
        for (id, t, v, health) in lines {
            let angle = t.rotation.to_euler(EulerRot::XYZ).2.to_degrees();
            info!(
                "trace {:.2} {} {:.3} {:.3} {:.2} {:.3}",
                script.tick as f32 / 60.0,
                id.0,
                t.translation.x,
                -t.translation.y,
                -angle,
                v.map_or(0.0, |v| v.0.length())
            );
            // The health too (the C# game's `/entities` has it, not its
            // `/trace`).
            if let Some(h) = health {
                info!("health {:.2} {} {}", script.tick as f32 / 60.0, id.0, h.hp);
            }
        }
    }
    // As the fire key: held, and accurate on the press
    // (`ScriptedCarControl`).
    let pulse = buttons.fire && !script.fire_held;
    script.fire_held = buttons.fire;
    for (mut intent, t, v, off) in &mut cars {
        // The script is the player: not while the car is not theirs.
        intent.move_dir = if off { Vec2::ZERO } else { stick };
        intent.gadget = !off && buttons.gadget;
        intent.gadget_at = None;
        intent.gadget_dir = None;
        intent.fire = !off && buttons.fire;
        if !off && pulse {
            intent.fire_accurate = true;
        }
        intent.aim_dir = Vec2::ZERO;
        intent.look = Vec2::ZERO;
        if script.tick.is_multiple_of(6) {
            let angle = t.rotation.to_euler(EulerRot::XYZ).2.to_degrees();
            info!(
                "drive t={:.2} pos=({:.3},{:.3}) angle={:.1} speed={:.3}",
                script.tick as f32 / 60.0,
                t.translation.x,
                t.translation.y,
                angle,
                v.0.length()
            );
        }
    }
    script.tick += 1;
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod tests;
