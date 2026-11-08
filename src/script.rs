//! Scripted driving, for comparing the car with the C# game: a sequence of
//! stick positions held for a time, run in the fixed 60 Hz steps, with the
//! car's position, angle and speed logged every 0.1 s.
//!
//! Format: `x,y:seconds;x,y:seconds` with the stick in world directions
//! (y up), e.g. `1,0:2;0,1:1;0,0:1` (right for 2 s, up for 1 s, let go).

use bevy::prelude::*;

use crate::car::{Car, drive_cars};
use crate::controls::{LocalPlayer, PlayerIntent};
use crate::game::GameState;

#[derive(Resource, Debug, Clone, Default)]
pub struct DriveScript {
    steps: Vec<(Vec2, u32)>,
    tick: u32,
}

impl DriveScript {
    pub fn parse(s: &str) -> Option<Self> {
        let steps = s
            .split(';')
            .filter(|p| !p.trim().is_empty())
            .map(|part| {
                let (dir, secs) = part.split_once(':')?;
                let (x, y) = dir.split_once(',')?;
                let v = Vec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?);
                let secs: f32 = secs.trim().parse().ok()?;
                Some((v.clamp_length_max(1.0), (secs * 60.0).round() as u32))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(DriveScript { steps, tick: 0 })
    }

    /// The stick at the current tick; `None` once the script has ended.
    fn current(&self) -> Option<Vec2> {
        let mut t = self.tick;
        for &(v, n) in &self.steps {
            if t < n {
                return Some(v);
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
                .run_if(in_state(GameState::Playing))
                .run_if(resource_exists::<DriveScript>),
        );
    }
}

type ScriptedCar<'a> = (&'a mut PlayerIntent, &'a Transform, &'a avian2d::prelude::LinearVelocity);

fn run_script(mut script: ResMut<DriveScript>, mut cars: Query<ScriptedCar, (With<LocalPlayer>, With<Car>)>) {
    let Some(stick) = script.current() else {
        return;
    };
    for (mut intent, t, v) in &mut cars {
        intent.move_dir = stick;
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
        assert_eq!(s.current(), Some(Vec2::X));
        s.tick = 30;
        assert_eq!(s.current(), Some(Vec2::Y));
        s.tick = 36;
        assert_eq!(s.current(), None);
        assert!(DriveScript::parse("oops").is_none());
    }
}
