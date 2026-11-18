//! Floor buttons (`Entities/ButtonBase.cs`, `Button.cs`, made by
//! `CreateButton`).
//!
//! A button is a static sensor. It is pressed by standing on it for a
//! second: by one of its `Subscribers` (map ids), or by the player's car
//! when it lists none. It looks every quarter of a second, as C# did; four
//! lights fill up meanwhile, then `ActionPress` runs for whoever stood on
//! it. With an `ActionRelease`, leaving runs that and the button can be
//! pressed again; without one it stays pressed. Loose things (not cars)
//! on a button are drawn to its middle and held there, so a box pushed
//! onto it stays.
//!
//! Lights: one orange light going round while it waits; yellow ones out
//! of four, a sound each, while it charges; four white once pressed.
//!
//! Not ported: the dotted line along `Path` (to what it opens),
//! `LaserCollector` buttons (no lasers yet), navigation links.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::actions::{AddMapAction, CloneOf, Run, RunAction, source_id};
use crate::content::actions::defs::{ButtonTypeType, CreateButton};
use crate::controls::LocalPlayer;
use crate::game::{GameState, LoadedMap, MapEntity, MapId};
use crate::render::layers::Layer;
use crate::render::sprite::{SpriteInfo, SpriteTint, UnitQuad, sprite_bundle};
use crate::sound::PlaySound;

/// How often a button looks at what stands on it (`_periodic`).
const LOOK_EVERY: f32 = 0.25;
/// How long it must be stood on (`_activateButtonTimeDest`).
const PRESS_AFTER: f32 = 1.0;
/// The physics shape against the sprite (`GeometryScale`: the rim is not
/// the button).
const BODY_SCALE: f32 = 1.0 - 18.0 / 64.0;
/// The pull on loose things away from its middle (Farseer newtons).
const PULL: f32 = 8.0;

const LIGHT: &str = "Entities\\ButtonLight";
const SPRITES: &[&str] = &[
    "Entities\\Button",
    "Entities\\ButtonPressed",
    "Entities\\ButtonWithoutRelease",
    "Entities\\ButtonWithoutReleasePressed",
    LIGHT,
];

const ORANGE: [f32; 4] = [1.0, 165.0 / 255.0, 0.0, 1.0];
const ORANGE_RED: [f32; 4] = [1.0, 69.0 / 255.0, 0.0, 1.0];
const YELLOW: [f32; 4] = [1.0, 1.0, 0.0, 1.0];
const WHITE: [f32; 4] = [1.0; 4];

/// Whether a button is pressed, and how far (`ButtonBase.UpdateActivity`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Press {
    /// Seconds stood on so far.
    pub charge: f32,
    pub pressed: bool,
    /// Lights lit while charging, out of 4.
    pub lights: usize,
    /// Whether it releases when left (it has an `ActionRelease`).
    pub releases: bool,
    was_stood_on: bool,
    /// The map id of the last one to stand on it.
    last: Option<i64>,
}

/// What a look at a button does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pressing {
    /// Another light comes on (sound).
    Light(usize),
    /// Pressed: run `ActionPress` for this one.
    Press(Option<i64>),
    /// Left: run `ActionRelease` for the last one.
    Release(Option<i64>),
}

impl Press {
    pub fn new(releases: bool) -> Self {
        Press {
            releases,
            ..default()
        }
    }

    /// One look, a quarter of a second after the last: `standing` is who
    /// stands on it (`Some(map id)`), if anyone may and does.
    pub fn look(&mut self, standing: Option<Option<i64>>) -> Option<Pressing> {
        let Some(who) = standing else {
            let mut out = None;
            if self.was_stood_on {
                if self.releases && self.pressed {
                    out = Some(Pressing::Release(self.last));
                    self.pressed = false;
                }
                self.charge = 0.0;
                self.lights = 0;
                self.was_stood_on = false;
            }
            return out;
        };
        self.was_stood_on = true;
        self.last = who;
        if self.charge >= PRESS_AFTER {
            let first = !self.pressed;
            self.pressed = true;
            return first.then_some(Pressing::Press(who));
        }
        self.charge += LOOK_EVERY;
        let lit = (self.charge / PRESS_AFTER * 4.0) as usize;
        // Lights (and their sound) are for a button not yet pressed.
        let more = lit > self.lights && lit < 4 && !self.pressed;
        self.lights = lit;
        more.then_some(Pressing::Light(lit))
    }
}

#[derive(Component, Debug)]
pub struct Button {
    press: Option<String>,
    release: Option<String>,
    /// Who may press it; `None`: the player's car.
    subscribers: Option<Vec<i64>>,
    pub state: Press,
    /// What stands on it now, if loose: drawn to the middle.
    holding: Option<Entity>,
    until_look: f32,
    /// The waiting light's quarter (`_currentPhase`).
    phase: usize,
    released_view: Entity,
    pressed_view: Entity,
    lights: [Entity; 4],
    radius: f32,
}

pub struct ButtonPlugin;

impl Plugin for ButtonPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<CreateButton, _>(create_button)
            .add_action_sprites::<CreateButton>(SPRITES)
            .add_systems(Update, (look, show).chain().run_if(in_state(GameState::Playing)))
            .add_systems(FixedUpdate, hold.run_if(in_state(GameState::Playing)));
    }
}

fn create_button(
    In(run): In<Run<CreateButton>>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) -> bool {
    let d = run.data;
    if d.button_type == ButtonTypeType::LaserCollector {
        info!("LaserCollector buttons are not ported yet");
        return false;
    }
    let plain = d.action_release.as_deref().is_none_or(|r| r.trim().is_empty());
    let (released, pressed) = if plain {
        ("Entities\\ButtonWithoutRelease", "Entities\\ButtonWithoutReleasePressed")
    } else {
        ("Entities\\Button", "Entities\\ButtonPressed")
    };
    let (Some(released), Some(pressed), Some(light)) =
        (map.sprites.get(released), map.sprites.get(pressed), map.sprites.get(LIGHT))
    else {
        warn!("button sprites missing");
        return false;
    };
    let lit = SpriteTint {
        color: WHITE,
        lit: true,
    };
    let glow = SpriteTint {
        color: ORANGE,
        lit: false,
    };
    let mut view = |info: &SpriteInfo, scale: f32, layer: Layer, tint: SpriteTint, turn: f32| {
        let mut sprite = sprite_bundle(info, scale, layer, tint, &quad, &mut materials);
        sprite.2.rotation = Quat::from_rotation_z(turn);
        if layer == Layer::Glow
            && let Some(mut m) = materials.get_mut(&sprite.1.0)
        {
            m.alpha_mode = AlphaMode::Add;
        }
        commands.spawn(sprite).id()
    };
    let released_view = view(released, d.scale, Layer::GroundDecor, lit, 0.0);
    let pressed_view = view(pressed, d.scale, Layer::GroundDecor, lit, 0.0);
    // Light `i` points a quarter turn on from the last (C#'s angles turn
    // the other way: y down).
    let lights = [0, 1, 2, 3].map(|i| view(light, d.scale * 2.0, Layer::Glow, glow, -(i as f32) * std::f32::consts::FRAC_PI_2));
    let size = released.size * crate::units::px(1.0) * d.scale * BODY_SCALE;
    let subscribers = d.subscribers.clone();
    let at = map.world(d.position.x, d.position.y);
    commands
        .spawn((
            Name::new(format!("Button {}", d.id)),
            MapEntity,
            MapId(d.id),
            Transform::from_translation(at.extend(0.0))
                .with_rotation(Quat::from_rotation_z(crate::units::map_degrees_to_world(d.direction_deg))),
            Visibility::default(),
            RigidBody::Static,
            Collider::rectangle(size.x, size.y),
            Sensor,
            CollidingEntities::default(),
            Button {
                press: d.action_press.filter(|a| !a.trim().is_empty()),
                release: d.action_release.filter(|a| !a.trim().is_empty()),
                subscribers,
                state: Press::new(!plain),
                holding: None,
                until_look: LOOK_EVERY,
                phase: 0,
                released_view,
                pressed_view,
                lights,
                radius: size.max_element() / 2.0,
            },
        ))
        .add_children(&[released_view, pressed_view])
        .add_children(&lights);
    true
}

/// `UpdateActivity`, every quarter of a second.
#[allow(clippy::type_complexity)]
fn look(
    time: Res<Time>,
    mut buttons: Query<(&mut Button, &CollidingEntities, &Transform)>,
    who: Query<(Option<&MapId>, Has<LocalPlayer>, &RigidBody, Option<&CloneOf>)>,
    mut run: MessageWriter<RunAction>,
    mut sound: MessageWriter<PlaySound>,
) {
    for (mut b, touching, transform) in &mut buttons {
        b.until_look -= time.delta_secs();
        if b.until_look > 0.0 {
            continue;
        }
        b.until_look += LOOK_EVERY;
        b.phase += 1;
        let at = Some(transform.translation.truncate());
        let standing = touching.iter().copied().find(|&e| {
            who.get(e).is_ok_and(|(id, player, _, clone)| match &b.subscribers {
                Some(ids) => id.is_some_and(|id| ids.contains(&source_id(id, clone))),
                None => player,
            })
        });
        b.holding = standing.filter(|&e| who.get(e).is_ok_and(|(_, player, body, _)| !player && body.is_dynamic()));
        let id = standing.map(|e| who.get(e).ok().and_then(|(id, _, _, _)| id.map(|id| id.0)));
        match b.state.look(id) {
            Some(Pressing::Light(n)) => {
                sound.write(PlaySound { name: "GButtonLight", at, pitch: -0.5 + n as f32 / 4.0, volume: 0.6 });
            }
            Some(Pressing::Press(actor)) => {
                if let Some(press) = b.press.clone() {
                    sound.write(PlaySound { name: "GButtonActivate", at, pitch: 0.0, volume: 1.0 });
                    run.write(RunAction { name: press, actor });
                }
            }
            Some(Pressing::Release(actor)) => {
                sound.write(PlaySound { name: "GButtonRelease", at, pitch: -0.5, volume: 1.0 });
                if let Some(release) = b.release.clone() {
                    run.write(RunAction { name: release, actor });
                }
            }
            None => {}
        }
    }
}

/// `Button.Update`: draw a loose thing to the middle, and hold it there.
fn hold(buttons: Query<(&Button, &Transform)>, mut bodies: Query<(&Position, &mut LinearVelocity, &ComputedMass)>, time: Res<Time>) {
    for (b, transform) in &buttons {
        let Some(Ok((pos, mut v, mass))) = b.holding.map(|e| bodies.get_mut(e)) else {
            continue;
        };
        let to_middle = transform.translation.truncate() - pos.0;
        if to_middle.length() > b.radius / 2.0 {
            v.0 += to_middle.normalize() * PULL * mass.inverse() * time.delta_secs();
        } else {
            // C# damped it each frame at 60 frames a second.
            v.0 *= 0.8_f32.powf(time.delta_secs() * 60.0);
        }
    }
}

/// `ButtonBase.Draw` and `DrawLight`.
fn show(
    buttons: Query<&Button>,
    mut visibility: Query<&mut Visibility>,
    mut tints: Query<&mut SpriteTint>,
) {
    for b in &buttons {
        let st = &b.state;
        let down = st.charge > 0.0 || st.pressed;
        let set = |v: &mut Query<&mut Visibility>, e: Entity, on: bool| {
            if let Ok(mut v) = v.get_mut(e) {
                v.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
            }
        };
        set(&mut visibility, b.released_view, !down);
        set(&mut visibility, b.pressed_view, down);
        for (i, &light) in b.lights.iter().enumerate() {
            let color = if st.pressed {
                Some(WHITE)
            } else if st.charge > 0.0 {
                Some(if i < st.lights { YELLOW } else { ORANGE_RED })
            } else {
                (i == b.phase % 4).then_some(ORANGE)
            };
            set(&mut visibility, light, color.is_some());
            if let (Some(color), Ok(mut tint)) = (color, tints.get_mut(light)) {
                tint.color = color;
            }
        }
    }
}
