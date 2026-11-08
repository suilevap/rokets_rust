//! Waves, colour tracks and pulses.

use tilerider::animation::{ColorTrack, GRAY, Pulse, WHITE, Wave, YELLOW};

#[test]
fn waves_repeat_over_game_time() {
    let saw = Wave::Sawtooth { period: 1.0 };
    assert!((saw.phase(0.25) - 0.25).abs() < 1e-6);
    assert!((saw.phase(3.75) - 0.75).abs() < 1e-5);
    let tri = Wave::Triangle { period: 1.0 };
    assert!((tri.phase(0.25) - 0.5).abs() < 1e-6);
    assert!((tri.phase(0.5) - 1.0).abs() < 1e-6);
    assert!((tri.phase(0.75) - 0.5).abs() < 1e-6);
}

#[test]
fn step_tracks_hold_and_smooth_tracks_blend() {
    let steps = ColorTrack::steps(&[(0.0, YELLOW), (0.4, GRAY)]);
    assert_eq!(steps.sample(0.39), YELLOW);
    assert_eq!(steps.sample(0.4), GRAY);
    assert_eq!(steps.sample(0.99), GRAY);
    let smooth = ColorTrack {
        keys: vec![(0.0, GRAY), (1.0, WHITE)],
        step: false,
    };
    let mid = smooth.sample(0.5);
    assert!((mid[0] - (GRAY[0] + WHITE[0]) / 2.0).abs() < 1e-6);
}

#[test]
fn pulses_blend_then_end() {
    let pulse = Pulse {
        start: WHITE,
        end: GRAY,
        duration: 1.0,
    };
    assert_eq!(pulse.sample(0.0), Some(WHITE));
    assert!(pulse.sample(0.5).is_some());
    assert_eq!(pulse.sample(1.0), None);
}
