//! Scripted driving, for comparing the car with the C# game: a sequence of
//! stick positions held for a time, run in the fixed 60 Hz steps, with the
//! car's position, angle and speed logged every 0.1 s.
//!
//! Format: `x,y:seconds;x,y:seconds` with the stick in world directions
//! (y up), e.g. `1,0:2;0,1:1;0,0:1` (right for 2 s, up for 1 s, let go).
//! A third value `g` holds the gadget button meanwhile, aimed ahead of the
//! car: `0,0,g:1` shoots the magnet out for a second.

use bevy::prelude::*;

use crate::car::{Car, drive_cars};
use crate::controls::{LocalPlayer, NoInput, PlayerIntent};
use crate::game::GameState;

#[derive(Resource, Debug, Clone, Default)]
pub struct DriveScript {
    /// Stick, gadget held, steps.
    steps: Vec<(Vec2, bool, u32)>,
    tick: u32,
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
                let gadget = match parts.next().map(str::trim) {
                    None => false,
                    Some("g") => true,
                    Some(_) => return None,
                };
                let v = Vec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?);
                let secs: f32 = secs.trim().parse().ok()?;
                Some((v.clamp_length_max(1.0), gadget, (secs * 60.0).round() as u32))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(DriveScript { steps, tick: 0 })
    }

    /// The stick and the gadget button at the current tick; `None` once
    /// the script has ended.
    fn now(&self) -> Option<(Vec2, bool)> {
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
                .before(crate::gadget::GadgetSystems)
                .run_if(in_state(GameState::Playing))
                .run_if(resource_exists::<DriveScript>),
        );
    }
}

type ScriptedCar<'a> = (&'a mut PlayerIntent, &'a Transform, &'a avian2d::prelude::LinearVelocity, Has<NoInput>);

fn run_script(mut script: ResMut<DriveScript>, mut cars: Query<ScriptedCar, (With<LocalPlayer>, With<Car>)>) {
    let Some((stick, gadget)) = script.now() else {
        return;
    };
    for (mut intent, t, v, off) in &mut cars {
        // The script is the player: not while the car is not theirs.
        intent.move_dir = if off { Vec2::ZERO } else { stick };
        intent.gadget = !off && gadget;
        intent.gadget_at = None;
        intent.gadget_dir = None;
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
mod tests {
    use super::*;

    #[test]
    fn parses_and_steps() {
        let mut s = DriveScript::parse("1,0:0.5;0,1:0.1").unwrap();
        assert_eq!(s.now(), Some((Vec2::X, false)));
        s.tick = 30;
        assert_eq!(s.now(), Some((Vec2::Y, false)));
        s.tick = 36;
        assert_eq!(s.now(), None);
        assert_eq!(DriveScript::parse("0,0,g:1").unwrap().now(), Some((Vec2::ZERO, true)));
        assert!(DriveScript::parse("oops").is_none());
    }
}
