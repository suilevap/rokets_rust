//! `HighlightController`: its templates, what it sets, and the pictures
//! that pulse for it on each device.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, ControlHighlight, CurrentActions, run};
use tilerider::content::map::MapData;
use tilerider::control_highlight::*;
use tilerider::controls::ActiveScheme;
use tilerider::game::GameState;

fn app() -> App {
    let map = MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>
             <Item Type="GameLibrary.HighlightController"><Name>G</Name>
               <GadgetControl>true</GadgetControl><WeaponControl>false</WeaponControl>
               <MovementControl>false</MovementControl></Item>
             <Item Type="GameLibrary.HighlightController"><Name>WM</Name>
               <GadgetControl>false</GadgetControl><WeaponControl>true</WeaponControl>
               <MovementControl>true</MovementControl></Item>
           </Actions></Asset></XnaContent>"#,
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, ActionsPlugin)).insert_resource(CurrentActions(map.actions));
    app
}

fn flags(app: &App) -> (bool, bool, bool) {
    let h = app.world().resource::<ControlHighlight>();
    (h.gadget, h.weapon, h.movement)
}

#[test]
fn each_run_replaces_all_three_flags() {
    let mut app = app();
    assert_eq!(flags(&app), (false, false, false));
    assert!(run(app.world_mut(), "G", None, 0));
    assert_eq!(flags(&app), (true, false, false));
    assert!(run(app.world_mut(), "WM", None, 0));
    assert_eq!(flags(&app), (false, true, true));
}

#[test]
fn the_templates_set_one_flag_or_none() {
    let mut app = app();
    for (template, want) in [
        ("HighlightControllerGadget()", (true, false, false)),
        ("HighlightControllerWeapon()", (false, true, false)),
        ("HighlightControllerMovement()", (false, false, true)),
        ("HighlightControllerOff()", (false, false, false)),
    ] {
        assert!(run(app.world_mut(), template, None, 0), "{template}");
        assert_eq!(flags(&app), want, "{template}");
    }
}

#[test]
fn a_map_ending_turns_it_off() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin, ActionsPlugin))
        .init_state::<GameState>()
        .insert_resource(CurrentActions(Default::default()));
    *app.world_mut().resource_mut::<ControlHighlight>() = ControlHighlight { gadget: true, weapon: true, movement: true };
    // (`OnEnter` clears it the same way, but also needs a loaded map.)
    app.world_mut().run_schedule(OnExit(GameState::Playing));
    assert_eq!(flags(&app), (false, false, false));
}

const SCREEN: Vec2 = Vec2::new(800.0, 480.0);

fn size(name: &str) -> Vec2 {
    match name {
        PAD_BACK => Vec2::new(256.0, 160.0),
        CIRCLE => Vec2::splat(256.0),
        CIRCLE_GADGET | CIRCLE_ACCELERATOR | CIRCLE_GUN => Vec2::splat(200.0),
        _ => Vec2::splat(64.0),
    }
}

fn situation(scheme: ActiveScheme, highlight: ControlHighlight, car: Car) -> Situation {
    Situation { scheme, highlight, car, fingers: Fingers::default(), screen: SCREEN }
}

fn armed() -> Car {
    Car { gadget: Some(GadgetView::Magnet), weapon: true, ..Car::default() }
}

fn names(h: &[Hint]) -> Vec<&str> {
    h.iter().map(|h| h.sprite).collect()
}

const ALL: ControlHighlight = ControlHighlight { gadget: true, weapon: true, movement: true };
const NONE: ControlHighlight = ControlHighlight { gadget: false, weapon: false, movement: false };

#[test]
fn nothing_is_drawn_when_off() {
    for scheme in [ActiveScheme::Keyboard, ActiveScheme::Gamepad, ActiveScheme::Touch] {
        assert!(hints(&situation(scheme, NONE, armed()), size).is_empty());
    }
}

#[test]
fn the_keyboard_shows_the_mouse_buttons_and_the_arrow_keys() {
    let h = hints(&situation(ActiveScheme::Keyboard, ALL, armed()), size);
    assert_eq!(names(&h), [MOUSE_RIGHT, MOUSE_LEFT, KEYS_LEFT_RIGHT, KEYS_UP_DOWN]);
    assert_eq!(h[0].center, Vec2::new(672.0, 128.0));
    assert_eq!(h[1].center, Vec2::new(672.0, 352.0));
    assert!(h.iter().all(|h| h.pulse && !h.red));
}

#[test]
fn only_what_the_car_has_is_shown() {
    let bare = Car::default();
    let h = hints(&situation(ActiveScheme::Keyboard, ALL, bare), size);
    assert_eq!(names(&h), [KEYS_LEFT_RIGHT, KEYS_UP_DOWN], "no gadget, no gun");
    let h = hints(&situation(ActiveScheme::Keyboard, ALL, Car { restrict_x: true, ..bare }), size);
    assert_eq!(names(&h), [KEYS_UP_DOWN]);
    let h = hints(&situation(ActiveScheme::Keyboard, ALL, Car { restrict_y: true, ..bare }), size);
    assert_eq!(names(&h), [KEYS_LEFT_RIGHT]);
}

#[test]
fn the_gamepad_draws_its_picture_under_the_pulsing_parts() {
    let weapon = ControlHighlight { weapon: true, ..NONE };
    let h = hints(&situation(ActiveScheme::Gamepad, weapon, armed()), size);
    assert_eq!(names(&h), [PAD_BACK, PAD_TRIGGER, PAD_STICK]);
    assert!(!h[0].pulse && h[0].alpha == 0.8);
    assert!(h[1].pulse && h[2].pulse);
    let moving = ControlHighlight { movement: true, ..NONE };
    let h = hints(&situation(ActiveScheme::Gamepad, moving, armed()), size);
    assert_eq!(names(&h), [PAD_BACK, PAD_STICK], "the left stick only");
    let gadget = ControlHighlight { gadget: true, ..NONE };
    let h = hints(&situation(ActiveScheme::Gamepad, gadget, Car::default()), size);
    assert!(h.is_empty(), "no gadget: not even the pad");
}

#[test]
fn touch_circles_sit_in_the_zones_and_the_gun_is_red() {
    let h = hints(&situation(ActiveScheme::Touch, ALL, armed()), size);
    assert_eq!(names(&h), [CIRCLE_GADGET, CIRCLE_GUN, CIRCLE]);
    let r = 100.8;
    let scale = r / 128.0;
    assert!(h.iter().all(|h| (h.scale - scale).abs() < 1e-4), "all at BigCircle's scale");
    assert!((h[0].center - Vec2::new(800.0 - r * 1.25, 240.0 - r * 1.25)).length() < 1e-3);
    assert!((h[1].center - Vec2::new(800.0 - r * 1.25, 480.0 - r * 1.25)).length() < 1e-3);
    assert!((h[2].center - Vec2::new(r * 1.25, 480.0 - r * 1.25)).length() < 1e-3);
    assert!(h[1].red && !h[0].red && !h[2].red);
}

#[test]
fn a_finger_in_a_zone_hides_its_highlight() {
    let s = |fingers| Situation { fingers, ..situation(ActiveScheme::Touch, ALL, armed()) };
    let h = hints(&s(Fingers { gadget: true, ..Fingers::default() }), size);
    assert_eq!(names(&h), [CIRCLE], "the gadget's finger hides the gadget and the gun");
    let h = hints(&s(Fingers { fire: true, ..Fingers::default() }), size);
    assert_eq!(names(&h), [CIRCLE_GADGET, CIRCLE]);
    let h = hints(&s(Fingers { stick: true, ..Fingers::default() }), size);
    assert_eq!(names(&h), [CIRCLE_GADGET, CIRCLE_GUN]);
}

#[test]
fn the_accelerator_shows_only_when_ready_and_a_static_car_has_no_stick() {
    let gadget = ControlHighlight { gadget: true, movement: true, ..NONE };
    let car = |ready, is_static| Car { gadget: Some(GadgetView::Accelerator { ready }), is_static, ..Car::default() };
    let h = hints(&situation(ActiveScheme::Touch, gadget, car(true, false)), size);
    assert_eq!(names(&h), [CIRCLE_ACCELERATOR, CIRCLE]);
    let h = hints(&situation(ActiveScheme::Touch, gadget, car(false, true)), size);
    assert!(h.is_empty());
}

#[test]
fn the_pictures_grow_with_the_window() {
    let big = Situation { screen: Vec2::new(1600.0, 960.0), ..situation(ActiveScheme::Keyboard, ALL, armed()) };
    let h = hints(&big, size);
    assert_eq!(h[0].scale, 2.0);
    assert_eq!(h[0].center, Vec2::new(1600.0 - 256.0, 256.0));
}

#[test]
fn the_pulse_goes_up_and_down_once_a_second() {
    assert_eq!(triangle(0.0), 0.0);
    assert_eq!(triangle(0.5), 1.0);
    assert!((triangle(0.75) - 0.5).abs() < 1e-6);
    assert!((triangle(1.25) - 0.5).abs() < 1e-6);
}
