//! The Smart scheme's hand on the magnet: taps on boxes and ground, the grab
//! and drop jobs over several frames (against a magnet that answers the
//! intent as `gadget::use_magnets` does, as `tests/ai_cargo.rs` has it),
//! the wait for the car to come in reach, and the assist in an app with
//! real magnets.

use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use tilerider::assist::magnet::{Aim, Finish, Handling, Job, PENDING_SECS, Phase, RELEASE_SECS, Scene, Spot, step};
use tilerider::assist::tap::{Pick, tap_pick};
use tilerider::assist::{Assist, AssistPlugin, Commanded, PlayerCommand, Tapped};
use tilerider::car::CarRadius;
use tilerider::car::control::{AutoAimVisible, Team};
use tilerider::controls::{LocalPlayer, NoInput, PlayerIntent};
use tilerider::explosion::{Explosion, Health};
use tilerider::gadget::apply::{ApplyGadget, apply_to_magnets};
use tilerider::gadget::handling::{HOLD_MAX, HOLD_MIN, NEAR};
use tilerider::gadget::static_cargo::wake_towed;
use tilerider::gadget::{CHARGE, Linkable, Magnet, REACH};
use tilerider::settings::{ControlScheme, Settings};
use tilerider::sound::PlaySound;

const DT: f32 = 1.0 / 60.0;
/// How fast the dragged cargo follows the aim, units a second.
const DRAG_SPEED: f32 = 4.0;

fn id(n: u32) -> Entity {
    Entity::from_raw_u32(n).expect("an entity")
}

const CARGO: u32 = 11;

// ---- the tap's pick (plain numbers) ----

#[test]
fn a_tap_points_at_the_nearest_enemy_or_thing_and_the_gun_wins_a_tie() {
    let enemies = [(id(1), Vec2::new(5.0, 0.0))];
    let things = [(id(2), Vec2::new(3.0, 0.0)), (id(3), Vec2::new(20.0, 0.0))];
    assert_eq!(tap_pick(enemies, things, Vec2::new(3.2, 0.0), 1.5), Pick::Thing(id(2)));
    assert_eq!(tap_pick(enemies, things, Vec2::new(4.8, 0.0), 1.5), Pick::Enemy(id(1)));
    assert_eq!(tap_pick(enemies, things, Vec2::new(4.0, 0.0), 1.5), Pick::Enemy(id(1)), "equally near: the enemy");
    assert_eq!(tap_pick(enemies, things, Vec2::new(10.0, 0.0), 1.5), Pick::None);
    assert_eq!(tap_pick([], [], Vec2::ZERO, 1.5), Pick::None);
}

// ---- a job against a simulated magnet ----

/// The car, the cargo and the magnet answering the intent the job writes.
struct Sim {
    job: Handling,
    car: Vec2,
    cargo: Vec2,
    towing: bool,
    /// The magnet can find the cargo (it is in the ray).
    finds: bool,
    /// Something else is what the magnet finds.
    finds_other: bool,
    gone: bool,
    wall: bool,
    manual: bool,
    time: f32,
    // the magnet
    held: f32,
    active: bool,
    dragging: bool,
    candidate: bool,
    // what the job wrote
    button: bool,
    aim: Vec2,
    cancels: usize,
    released_at: Vec<f32>,
    held_since: Option<f32>,
    ended: Option<Finish>,
}

impl Sim {
    fn new(job: Job, car: Vec2, cargo: Vec2) -> Sim {
        Sim {
            job: Handling::new(job),
            car,
            cargo,
            towing: false,
            finds: true,
            finds_other: false,
            gone: false,
            wall: false,
            manual: false,
            time: 0.0,
            held: 0.0,
            active: false,
            dragging: false,
            candidate: false,
            button: false,
            aim: Vec2::ZERO,
            cancels: 0,
            released_at: Vec::new(),
            held_since: None,
            ended: None,
        }
    }

    fn tick(&mut self) {
        self.time += DT;
        if self.ended.is_some() {
            return;
        }
        let spot = Spot { pos: self.cargo, radius: 0.5 };
        let scene = Scene {
            from: self.car,
            towing: self.towing.then_some(id(CARGO)),
            candidate: if self.candidate {
                Some(if self.finds_other { id(99) } else { id(CARGO) })
            } else {
                None
            },
            cargo: match self.job.job {
                Job::Grab(_) => (!self.gone).then_some(spot),
                Job::Drop(_) => (self.towing && !self.gone).then_some(spot),
            },
            in_sight: !self.wall,
            manual: self.manual,
            dt: DT,
        };
        let out = step(&mut self.job, &scene);
        if let Some(hold) = out.hold {
            self.button = hold;
        }
        match out.aim {
            Some(Aim::Dir(d)) => self.aim = self.car + d * REACH,
            Some(Aim::At(p)) => self.aim = p,
            None => {}
        }
        if out.cancel {
            self.cancels += 1;
            self.button = false;
            self.towing = false;
            self.held = 0.0;
            self.active = false;
            self.dragging = false;
            self.candidate = false;
            self.held_since = None;
        }
        if out.finished.is_some() {
            self.ended = out.finished;
        }
        self.work_the_magnet();
    }

    /// `gadget::use_magnets`, the part the intent reaches.
    fn work_the_magnet(&mut self) {
        if self.manual {
            return;
        }
        if self.button {
            self.held += DT;
            if self.held < CHARGE {
                return;
            }
            if !self.active {
                self.active = true;
                self.dragging = self.towing;
                self.held_since.get_or_insert(self.time);
            }
            if self.dragging {
                let to = (self.aim.clamp_length_max(f32::MAX) - self.cargo).clamp_length_max(DRAG_SPEED * DT);
                self.cargo += to;
            } else {
                let to = self.cargo - self.car;
                self.candidate = (self.finds || self.finds_other) && !self.gone && to.length() <= REACH;
            }
        } else {
            if self.held >= CHARGE && self.active {
                self.towing = self.candidate && !self.dragging && !self.gone;
                self.released_at.push(self.time);
            }
            self.held = 0.0;
            self.active = false;
            self.dragging = false;
            self.candidate = false;
            self.held_since = None;
        }
    }

    fn run(&mut self, secs: f32) {
        for _ in 0..(secs / DT).ceil() as usize {
            self.tick();
        }
    }
}

#[test]
fn a_grab_in_reach_holds_the_button_a_second_then_lets_go_and_has_the_cargo() {
    let mut sim = Sim::new(Job::Grab(id(CARGO)), Vec2::ZERO, Vec2::new(5.0, 0.0));
    sim.run(0.1);
    assert!(sim.button, "the button is down at once");
    assert!(sim.aim.distance(Vec2::new(REACH, 0.0)) < 1e-4, "aimed a unit way at the cargo: {:?}", sim.aim);
    sim.run(3.0);
    assert_eq!(sim.ended, Some(Finish::Done));
    assert!(sim.towing);
    assert_eq!(sim.released_at.len(), 1, "let go once");
    assert!(sim.released_at[0] >= HOLD_MIN, "held at least {HOLD_MIN} s, let go at {}", sim.released_at[0]);
    assert_eq!(sim.cancels, 0);
}

#[test]
fn a_grab_out_of_reach_waits_for_the_car_to_come_and_then_goes() {
    let mut sim = Sim::new(Job::Grab(id(CARGO)), Vec2::ZERO, Vec2::new(NEAR + 3.0, 0.0));
    sim.run(2.0);
    assert!(!sim.button && sim.ended.is_none(), "pending, the button untouched");
    assert_eq!(sim.job.phase, Phase::Waiting);
    // The player drives in range.
    sim.car = Vec2::new(4.0, 0.0);
    sim.run(3.0);
    assert_eq!(sim.ended, Some(Finish::Done));
    assert!(sim.towing);
}

#[test]
fn a_request_out_of_reach_times_out() {
    let mut sim = Sim::new(Job::Grab(id(CARGO)), Vec2::ZERO, Vec2::new(30.0, 0.0));
    sim.run(PENDING_SECS - 0.5);
    assert_eq!(sim.ended, None);
    sim.run(1.0);
    assert_eq!(sim.ended, Some(Finish::TimedOut));
    assert!(!sim.button);
    assert_eq!(sim.cancels, 0, "nothing was held, nothing to cancel");
}

#[test]
fn a_wall_between_keeps_a_grab_pending() {
    let mut sim = Sim::new(Job::Grab(id(CARGO)), Vec2::ZERO, Vec2::new(4.0, 0.0));
    sim.wall = true;
    sim.run(3.0);
    assert!(!sim.button && sim.ended.is_none());
    sim.wall = false;
    sim.run(3.0);
    assert_eq!(sim.ended, Some(Finish::Done));
}

#[test]
fn a_magnet_that_finds_nothing_is_held_five_seconds_then_cancelled() {
    let mut sim = Sim::new(Job::Grab(id(CARGO)), Vec2::ZERO, Vec2::new(4.0, 0.0));
    sim.finds = false;
    sim.run(HOLD_MAX - 0.5);
    assert!(sim.button && sim.ended.is_none());
    sim.run(1.0);
    assert_eq!(sim.ended, Some(Finish::Failed));
    assert_eq!(sim.cancels, 1);
    assert!(!sim.button);
}

#[test]
fn a_grab_does_not_let_go_at_something_else_in_the_way() {
    // The AI lets go at any candidate; the assist waits for the cargo.
    let mut sim = Sim::new(Job::Grab(id(CARGO)), Vec2::ZERO, Vec2::new(4.0, 0.0));
    sim.finds = false;
    sim.finds_other = true;
    sim.run(HOLD_MAX - 0.5);
    assert!(sim.button, "still holding");
    assert!(!sim.towing);
    sim.run(1.0);
    assert_eq!(sim.ended, Some(Finish::Failed));
}

#[test]
fn a_cargo_that_vanishes_while_held_cancels_the_gadget() {
    let mut sim = Sim::new(Job::Grab(id(CARGO)), Vec2::ZERO, Vec2::new(4.0, 0.0));
    sim.finds = false;
    sim.run(0.5);
    assert!(sim.button);
    sim.gone = true;
    sim.run(0.1);
    assert_eq!(sim.ended, Some(Finish::Gone));
    assert_eq!(sim.cancels, 1);
}

#[test]
fn a_hand_on_the_button_is_left_alone() {
    let mut sim = Sim::new(Job::Grab(id(CARGO)), Vec2::ZERO, Vec2::new(4.0, 0.0));
    sim.manual = true;
    sim.run(2.0);
    assert!(!sim.button && sim.ended.is_none() && sim.cancels == 0, "the assist wrote nothing");
    // The player grabbed it by hand: the job is done.
    sim.towing = true;
    sim.run(0.1);
    assert_eq!(sim.ended, Some(Finish::Done));
}

#[test]
fn a_drop_drags_the_cargo_over_the_point_and_lets_it_go_there() {
    let point = Vec2::new(6.0, 2.0);
    let mut sim = Sim::new(Job::Drop(point), Vec2::ZERO, Vec2::new(1.0, 0.0));
    sim.towing = true;
    sim.run(0.1);
    assert!(sim.button);
    assert_eq!(sim.aim, point, "the drag aims at the point itself");
    sim.run(8.0);
    assert_eq!(sim.ended, Some(Finish::Done));
    assert!(!sim.towing, "dropped");
    assert!(sim.cargo.distance(point) < 0.5 + 0.75, "dropped near the point, at {:?}", sim.cargo);
    assert!(sim.released_at[0] >= HOLD_MIN);
    assert_eq!(sim.cancels, 0);
}

#[test]
fn a_drop_point_out_of_reach_waits_then_times_out() {
    let mut sim = Sim::new(Job::Drop(Vec2::new(30.0, 0.0)), Vec2::ZERO, Vec2::new(1.0, 0.0));
    sim.towing = true;
    sim.run(3.0);
    assert!(!sim.button && sim.ended.is_none(), "the cargo is not touched");
    sim.run(PENDING_SECS);
    assert_eq!(sim.ended, Some(Finish::TimedOut));
    assert!(sim.towing, "what is on the rope stays");
}

#[test]
fn a_drop_holds_five_seconds_at_most_if_the_cargo_cannot_get_there() {
    let mut sim = Sim::new(Job::Drop(Vec2::new(6.0, 0.0)), Vec2::ZERO, Vec2::new(1.0, 0.0));
    sim.towing = true;
    // The cargo is stuck.
    for _ in 0..(HOLD_MAX / DT) as usize + 5 {
        sim.cargo = Vec2::new(1.0, 0.0);
        sim.tick();
    }
    assert!(!sim.towing, "let go after at most {HOLD_MAX} s");
    assert_eq!(sim.job.phase, Phase::Releasing);
}

#[test]
fn a_rope_that_breaks_during_a_drop_ends_it() {
    let mut sim = Sim::new(Job::Drop(Vec2::new(6.0, 0.0)), Vec2::ZERO, Vec2::new(1.0, 0.0));
    sim.towing = true;
    sim.run(0.5);
    sim.towing = false;
    sim.run(0.1);
    assert_eq!(sim.ended, Some(Finish::Gone));
    assert_eq!(sim.cancels, 1);
}

#[test]
fn a_release_the_magnet_never_shows_fails_after_a_moment() {
    let mut h = Handling::new(Job::Grab(id(CARGO)));
    let scene = |candidate| Scene {
        from: Vec2::ZERO,
        towing: None,
        candidate,
        cargo: Some(Spot { pos: Vec2::new(4.0, 0.0), radius: 0.5 }),
        in_sight: true,
        manual: false,
        dt: 0.25,
    };
    // Held for a second with the cargo found, then let go.
    let mut last = step(&mut h, &scene(Some(id(CARGO))));
    for _ in 0..4 {
        last = step(&mut h, &scene(Some(id(CARGO))));
    }
    assert_eq!(h.phase, Phase::Releasing);
    assert_eq!(last.hold, Some(false));
    let mut ended = None;
    for _ in 0..((RELEASE_SECS / 0.25) as usize + 2) {
        ended = step(&mut h, &scene(None)).finished.or(ended);
    }
    assert_eq!(ended, Some(Finish::Failed));
}

// ---- the assist in an app, with real magnets ----

fn smart_app() -> App {
    let mut settings = Settings::default();
    settings.control_scheme = ControlScheme::Smart;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()))
        .insert_resource(Gravity(Vec2::ZERO))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)))
        .insert_resource(settings)
        .add_message::<ApplyGadget>()
        .add_message::<PlaySound>()
        .add_message::<Explosion>()
        .add_plugins(AssistPlugin)
        .add_systems(FixedUpdate, (apply_to_magnets, wake_towed).chain());
    app.finish();
    app.cleanup();
    app.update();
    app
}

/// The player's car with a magnet (and a body, for the rope's ray).
fn player(app: &mut App, magnet: bool) -> Entity {
    let car = app
        .world_mut()
        .spawn((
            LocalPlayer,
            PlayerIntent::default(),
            RigidBody::Dynamic,
            Collider::rectangle(1.0, 1.0),
            Position(Vec2::ZERO),
            Team(0),
            Health::full(100),
            AutoAimVisible(true),
            CarRadius(1.0),
            Assist::default(),
        ))
        .id();
    if magnet {
        app.world_mut().entity_mut(car).insert(Magnet::new(Vec2::ZERO));
    }
    car
}

fn crate_at(app: &mut App, at: Vec2) -> Entity {
    app.world_mut().spawn((RigidBody::Dynamic, Collider::rectangle(1.0, 1.0), Position(at), Linkable)).id()
}

fn enemy_at(app: &mut App, at: Vec2, linkable: bool) -> Entity {
    let e = app.world_mut().spawn((Team(1), Position(at), AutoAimVisible(true), CarRadius(1.0), Health::full(10))).id();
    if linkable {
        app.world_mut().entity_mut(e).insert(Linkable);
    }
    e
}

/// A frame as the devices leave it: the gadget button up unless a hand holds it.
fn frame(app: &mut App, car: Entity) {
    let mut intent = app.world_mut().get_mut::<PlayerIntent>(car).unwrap();
    intent.gadget = false;
    intent.gadget_at = None;
    intent.gadget_dir = None;
    app.update();
}

fn tap(app: &mut App, at: Vec2) -> Vec<PlayerCommand> {
    app.world_mut().resource_mut::<Messages<Tapped>>().write(Tapped { ground: at });
    app.update();
    app.world_mut().resource_mut::<Messages<Commanded>>().drain().map(|c| c.command).collect()
}

fn job(app: &App, car: Entity) -> Option<Job> {
    app.world().get::<Assist>(car).unwrap().cargo.map(|h| h.job)
}

fn intent(app: &App, car: Entity) -> PlayerIntent {
    *app.world().get::<PlayerIntent>(car).unwrap()
}

/// Ties the crate to the car's rope (the map action's way in).
fn tow(app: &mut App, car: Entity, to: Vec2) {
    for _ in 0..3 {
        app.update();
    }
    app.world_mut().write_message(ApplyGadget { car, at: to });
    app.update();
}

#[test]
fn a_tap_on_a_box_is_a_grab_and_the_assist_takes_over_the_button() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    let cargo = crate_at(&mut app, Vec2::new(4.0, 0.0));
    let sent = tap(&mut app, Vec2::new(4.5, 0.5));
    assert!(sent.contains(&PlayerCommand::Grab(cargo)), "{sent:?}");
    assert_eq!(job(&app, car), Some(Job::Grab(cargo)));
    for _ in 0..5 {
        frame(&mut app, car);
    }
    let i = intent(&app, car);
    assert!(i.gadget, "the button is held");
    let d = i.gadget_dir.expect("aimed");
    assert!(d.distance(Vec2::X) < 1e-4, "straight at the box: {d:?}");
}

#[test]
fn a_tap_on_an_enemy_still_locks_even_where_the_magnet_could_take_it() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    let foe = enemy_at(&mut app, Vec2::new(4.0, 0.0), true);
    let sent = tap(&mut app, Vec2::new(4.0, 0.2));
    assert_eq!(sent, [PlayerCommand::LockTarget(foe)]);
    assert_eq!(app.world().get::<Assist>(car).unwrap().lock, Some(foe));
    assert_eq!(job(&app, car), None);
}

#[test]
fn a_dead_enemys_wreck_can_be_grabbed() {
    let mut app = smart_app();
    let _car = player(&mut app, true);
    let wreck = enemy_at(&mut app, Vec2::new(4.0, 0.0), true);
    app.world_mut().get_mut::<Health>(wreck).unwrap().hp = 0;
    assert_eq!(tap(&mut app, Vec2::new(4.0, 0.0)), [PlayerCommand::Grab(wreck)]);
}

#[test]
fn without_a_magnet_a_box_is_just_ground() {
    let mut app = smart_app();
    let car = player(&mut app, false);
    crate_at(&mut app, Vec2::new(4.0, 0.0));
    assert_eq!(tap(&mut app, Vec2::new(4.0, 0.0)), [PlayerCommand::ClearTarget]);
    assert_eq!(job(&app, car), None);
}

#[test]
fn empty_ground_clears_the_lock_and_gives_up_a_pending_grab() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    let far = crate_at(&mut app, Vec2::new(20.0, 0.0));
    tap(&mut app, Vec2::new(20.0, 0.0));
    assert_eq!(job(&app, car), Some(Job::Grab(far)), "out of reach: pending");
    for _ in 0..30 {
        frame(&mut app, car);
    }
    assert!(!intent(&app, car).gadget, "the button is not touched while waiting");
    assert_eq!(tap(&mut app, Vec2::new(0.0, 10.0)), [PlayerCommand::ClearTarget, PlayerCommand::CancelCargo]);
    assert_eq!(job(&app, car), None);
}

#[test]
fn a_pending_grab_times_out_and_the_marker_data_goes() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    let far = crate_at(&mut app, Vec2::new(20.0, 0.0));
    tap(&mut app, Vec2::new(20.0, 0.0));
    assert_eq!(job(&app, car), Some(Job::Grab(far)));
    for _ in 0..((PENDING_SECS - 1.0) / DT) as usize {
        frame(&mut app, car);
    }
    assert_eq!(job(&app, car), Some(Job::Grab(far)), "still pending");
    for _ in 0..(2.0 / DT) as usize {
        frame(&mut app, car);
    }
    assert_eq!(job(&app, car), None, "given up");
}

#[test]
fn a_tap_on_the_ground_while_towing_is_a_drop_there_and_the_assist_aims_at_it() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    let cargo = crate_at(&mut app, Vec2::new(3.0, 0.0));
    tow(&mut app, car, Vec2::new(3.0, 0.0));
    assert_eq!(app.world().get::<Magnet>(car).unwrap().towing(), Some(cargo), "the rope has it");
    let point = Vec2::new(0.0, 5.0);
    assert_eq!(tap(&mut app, point), [PlayerCommand::DropAt(point)]);
    assert_eq!(job(&app, car), Some(Job::Drop(point)));
    for _ in 0..5 {
        frame(&mut app, car);
    }
    let i = intent(&app, car);
    assert!(i.gadget);
    assert_eq!(i.gadget_at, Some(point));
    // A tap on another box (or the cargo) while towing is a drop too, and an
    // enemy still locks.
    let other = crate_at(&mut app, Vec2::new(-4.0, 0.0));
    let sent = tap(&mut app, Vec2::new(-4.0, 0.0));
    assert_eq!(sent, [PlayerCommand::DropAt(Vec2::new(-4.0, 0.0))], "not a grab of {other:?}");
    let foe = enemy_at(&mut app, Vec2::new(0.0, -6.0), false);
    assert_eq!(tap(&mut app, Vec2::new(0.0, -6.0)), [PlayerCommand::LockTarget(foe)]);
}

#[test]
fn commands_that_make_no_sense_are_ignored() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    let bare = player(&mut app, false);
    app.world_mut().entity_mut(bare).remove::<(Collider, RigidBody)>();
    let cargo = crate_at(&mut app, Vec2::new(3.0, 0.0));
    let plain = app.world_mut().spawn(Position(Vec2::new(1.0, 1.0))).id();
    let send = |app: &mut App, car, command| {
        app.world_mut().resource_mut::<Messages<Commanded>>().write(Commanded { car, command });
        app.update();
    };
    send(&mut app, car, PlayerCommand::DropAt(Vec2::X));
    assert_eq!(job(&app, car), None, "nothing to drop");
    send(&mut app, car, PlayerCommand::Grab(plain));
    assert_eq!(job(&app, car), None, "the magnet cannot take it");
    send(&mut app, bare, PlayerCommand::Grab(cargo));
    assert_eq!(job(&app, bare), None, "no magnet");
    send(&mut app, car, PlayerCommand::Grab(car));
    assert_eq!(job(&app, car), None, "not itself");
    send(&mut app, car, PlayerCommand::Grab(cargo));
    assert_eq!(job(&app, car), Some(Job::Grab(cargo)));
    // Towing: no new grab.
    tow(&mut app, car, Vec2::new(3.0, 0.0));
    assert!(app.world().get::<Magnet>(car).unwrap().towing().is_some(), "the test's rope has the crate");
    let second = crate_at(&mut app, Vec2::new(0.0, 4.0));
    send(&mut app, car, PlayerCommand::Grab(second));
    assert_ne!(job(&app, car), Some(Job::Grab(second)), "something is on the rope already");
}

#[test]
fn a_new_job_while_the_button_is_held_cancels_the_gadget_first() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    let cargo = crate_at(&mut app, Vec2::new(0.0, 4.0));
    app.world_mut().resource_mut::<Messages<Commanded>>().write(Commanded { car, command: PlayerCommand::Grab(cargo) });
    for _ in 0..10 {
        frame(&mut app, car);
    }
    assert!(intent(&app, car).gadget);
    app.world_mut().resource_mut::<Messages<Commanded>>().write(Commanded { car, command: PlayerCommand::CancelCargo });
    frame(&mut app, car);
    let i = intent(&app, car);
    assert!(i.gadget_cancel && !i.gadget, "the gadget is cancelled, the button up");
    assert_eq!(job(&app, car), None);
}

#[test]
fn a_manual_gadget_press_is_not_fought() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    let cargo = crate_at(&mut app, Vec2::new(0.0, 4.0));
    app.world_mut().resource_mut::<Messages<Commanded>>().write(Commanded { car, command: PlayerCommand::Grab(cargo) });
    for _ in 0..5 {
        let mut i = app.world_mut().get_mut::<PlayerIntent>(car).unwrap();
        // A device holds the button, aiming its own way.
        i.gadget = true;
        i.gadget_at = Some(Vec2::new(-5.0, 0.0));
        i.gadget_dir = None;
        app.update();
    }
    let i = intent(&app, car);
    assert_eq!(i.gadget_at, Some(Vec2::new(-5.0, 0.0)), "the manual aim stands");
    assert_eq!(app.world().get::<Assist>(car).unwrap().cargo.map(|h| h.phase), Some(Phase::Waiting));
}

#[test]
fn a_car_nobody_drives_takes_no_job_on() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    app.world_mut().entity_mut(car).insert(NoInput);
    let cargo = crate_at(&mut app, Vec2::new(0.0, 4.0));
    app.world_mut().resource_mut::<Messages<Commanded>>().write(Commanded { car, command: PlayerCommand::Grab(cargo) });
    for _ in 0..5 {
        frame(&mut app, car);
    }
    assert!(!intent(&app, car).gadget);
    assert_eq!(job(&app, car), None);
}

#[test]
fn the_classic_scheme_has_no_assist_so_taps_and_magnets_are_untouched() {
    let mut app = smart_app();
    app.insert_resource(Settings::default());
    let car = player(&mut app, true);
    app.world_mut().entity_mut(car).remove::<Assist>();
    crate_at(&mut app, Vec2::new(3.0, 0.0));
    app.update();
    assert!(app.world().get::<Assist>(car).is_none());
    app.world_mut().resource_mut::<Messages<Tapped>>().write(Tapped { ground: Vec2::new(3.0, 0.0) });
    for _ in 0..5 {
        frame(&mut app, car);
    }
    assert!(!intent(&app, car).gadget);
    assert!(app.world().get::<Assist>(car).is_none());
}

#[test]
fn a_double_tap_while_towing_is_a_drop_twice_and_never_a_drive() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    let cargo = crate_at(&mut app, Vec2::new(3.0, 0.0));
    tow(&mut app, car, Vec2::new(3.0, 0.0));
    assert_eq!(app.world().get::<Magnet>(car).unwrap().towing(), Some(cargo));
    let point = Vec2::new(0.0, 5.0);
    assert_eq!(tap(&mut app, point), [PlayerCommand::DropAt(point)]);
    assert_eq!(tap(&mut app, point), [PlayerCommand::DropAt(point)], "the second tap, a frame later: no GoTo");
    assert!(app.world().get::<Assist>(car).unwrap().goto.is_none());
}
