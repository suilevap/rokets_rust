//! Map actions: what triggers, sequences and `RunOnMapStart` set off.
//!
//! C# origin: `TriggerManager.RunAction` and `ActionRunner` (2012 on). A
//! [`RunAction`] message names an action (or a template such as
//! `ActivateEntity(1001)`) and the entity that set it off. The runner
//! resolves it (`content::actions::resolve`), runs sequences (now, or
//! `TimeBetweenInSec` apart), follows `NextAction`, and turns each action
//! into a message for the system that carries it out:
//!
//! - [`SetEntityActive`]: `Entity.Activate` / `Deactivate`: hidden, out of
//!   the physics, back again.
//! - [`DestroyEntity`]: `Entity.Destroy`: bombs and red boxes blow up, the
//!   player's car wrecks, anything else goes.
//! - triggers armed and disarmed (`triggers`), impulses (a change of speed
//!   as C#'s `mass × Impulse`), damage and healing (`Hit`), level end and
//!   restart.
//!
//! Actions the port does not have yet are logged once per map.

use std::collections::BTreeSet;

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::Rng;
use rand::rngs::SmallRng;

use crate::content::actions::{Act, resolve};
use crate::explosion::{Destroyed, Explosion, Explosive, Health, Hit};
use crate::game::{ChangeMap, GameState, LoadedMap, MapId};

/// How deep sequences and `NextAction` chains may nest (a loop guard).
const MAX_DEPTH: usize = 16;

/// Run an action (a map action's name or a template) for `actor`, the map
/// id of the entity that set it off (`itemId`), if any.
#[derive(Message, Debug, Clone)]
pub struct RunAction {
    pub name: String,
    pub actor: Option<i64>,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct SetEntityActive {
    pub entity: Entity,
    pub active: bool,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct DestroyEntity(pub Entity);

#[derive(Message, Debug, Clone)]
pub struct SetTriggersActive {
    pub ids: Vec<i64>,
    pub active: bool,
}

/// The level is won (`MoveToNextMap`, `ShowSuccess`).
#[derive(Message, Debug, Clone, Copy)]
pub struct LevelComplete;

/// Out of the game for now (`Entity.Deactivate`).
#[derive(Component, Debug)]
pub struct Inactive;

/// Sequences waiting for their turn: seconds left, then the action.
#[derive(Resource, Default)]
struct Delayed(Vec<(f32, RunAction)>);

#[derive(Resource)]
struct ActionRng(SmallRng);

/// The actions of this map that were reported as not ported.
#[derive(Resource, Default)]
struct Reported(BTreeSet<String>);

/// The action runner and the systems that carry actions out.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActionSystems;

pub struct ActionsPlugin;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RunAction>()
            .add_message::<SetEntityActive>()
            .add_message::<DestroyEntity>()
            .add_message::<SetTriggersActive>()
            .add_message::<LevelComplete>()
            .init_resource::<Delayed>()
            .init_resource::<Reported>()
            .insert_resource(ActionRng(rand::SeedableRng::seed_from_u64(13)))
            .add_systems(OnEnter(GameState::Playing), run_on_map_start)
            .add_systems(
                Update,
                (
                    tick_delayed,
                    run_actions,
                    (set_active, destroy_entities),
                )
                    .chain()
                    .in_set(ActionSystems)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

/// `TriggerManager.MapLoaded`: actions marked `RunOnMapStart`.
fn run_on_map_start(
    map: Res<LoadedMap>,
    mut delayed: ResMut<Delayed>,
    mut reported: ResMut<Reported>,
    mut run: MessageWriter<RunAction>,
) {
    delayed.0.clear();
    reported.0.clear();
    for a in map.data.actions.iter().filter(|a| a.run_on_map_start) {
        if let Some(name) = &a.name {
            run.write(RunAction {
                name: name.clone(),
                actor: None,
            });
        }
    }
}

fn tick_delayed(time: Res<Time>, mut delayed: ResMut<Delayed>, mut run: MessageWriter<RunAction>) {
    let dt = time.delta_secs();
    let mut due = Vec::new();
    delayed.0.retain_mut(|(left, action)| {
        *left -= dt;
        if *left <= 0.0 {
            due.push(action.clone());
            false
        } else {
            true
        }
    });
    run.write_batch(due);
}

/// What the actions of one frame do, gathered by [`run_actions`].
#[derive(Default)]
struct Outcome {
    active: Vec<(i64, bool)>,
    destroy: Vec<i64>,
    triggers: Vec<SetTriggersActive>,
    impulses: Vec<(i64, Vec2)>,
    damage: Vec<(i64, i64, f32)>,
    complete: bool,
    restart: bool,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn run_actions(
    map: Res<LoadedMap>,
    mut requests: MessageReader<RunAction>,
    mut delayed: ResMut<Delayed>,
    mut reported: ResMut<Reported>,
    mut rng: ResMut<ActionRng>,
    ids: Query<(Entity, &MapId)>,
    mut bodies: Query<(&mut LinearVelocity, &RigidBody)>,
    mut health: Query<&mut Health>,
    mut writers: (
        MessageWriter<SetEntityActive>,
        MessageWriter<DestroyEntity>,
        MessageWriter<SetTriggersActive>,
        MessageWriter<Hit>,
        MessageWriter<LevelComplete>,
        MessageWriter<ChangeMap>,
    ),
) {
    let mut out = Outcome::default();
    let requests: Vec<RunAction> = requests.read().cloned().collect();
    for r in requests {
        let mut ctx = Ctx {
            map: &map,
            out: &mut out,
            delayed: &mut delayed,
            reported: &mut reported,
            rng: &mut rng.0,
        };
        ctx.run(&r.name, r.actor, 0);
    }
    let find = |id: i64| ids.iter().find(|(_, m)| m.0 == id).map(|(e, _)| e);
    let (active_w, destroy_w, triggers_w, hit_w, complete_w, change_w) = &mut writers;
    for (id, active) in out.active {
        if let Some(entity) = find(id) {
            active_w.write(SetEntityActive { entity, active });
        }
    }
    for id in out.destroy {
        if let Some(entity) = find(id) {
            destroy_w.write(DestroyEntity(entity));
        }
    }
    triggers_w.write_batch(out.triggers);
    for (id, change) in out.impulses {
        let Some(entity) = find(id) else { continue };
        if let Ok((mut v, RigidBody::Dynamic)) = bodies.get_mut(entity) {
            v.0 += change;
        }
    }
    for (id, amount, direction) in out.damage {
        let Some(entity) = find(id) else { continue };
        if amount >= 0 {
            // C#'s angle was -Direction in its y-down space: +Direction here.
            hit_w.write(Hit {
                target: entity,
                damage: amount as i32,
                from: direction.to_radians(),
            });
        } else if let Ok(mut h) = health.get_mut(entity) {
            h.hp = (h.hp - amount as i32).min(h.max);
        }
    }
    if out.complete {
        complete_w.write(LevelComplete);
    }
    if out.restart {
        change_w.write(ChangeMap::Restart);
    }
}

struct Ctx<'a> {
    map: &'a LoadedMap,
    out: &'a mut Outcome,
    delayed: &'a mut Delayed,
    reported: &'a mut Reported,
    rng: &'a mut SmallRng,
}

impl Ctx<'_> {
    /// Runs a reference and what follows it.
    fn run(&mut self, name: &str, actor: Option<i64>, depth: usize) {
        if depth > MAX_DEPTH || name.trim().is_empty() {
            return;
        }
        // `GetId`: an id of 0 or less means whoever set the action off.
        let target = |item: i64| if item > 0 { Some(item) } else { actor };
        for resolved in resolve(&self.map.data, name) {
            match resolved.act {
                Act::Sequence { actions, step } => {
                    for (i, a) in actions.iter().enumerate() {
                        if a.eq_ignore_ascii_case("na") {
                            continue;
                        }
                        if step > 0.0 && i > 0 {
                            let later = RunAction {
                                name: a.clone(),
                                actor,
                            };
                            self.delayed.0.push((step * i as f32, later));
                        } else {
                            self.run(a, actor, depth + 1);
                        }
                    }
                }
                Act::SetActive { item, active } => self.out.active.extend(target(item).map(|id| (id, active))),
                Act::Destroy { item } => self.out.destroy.extend(target(item)),
                Act::SetTriggers { ids, active } => self.out.triggers.push(SetTriggersActive { ids, active }),
                Act::Impulse { item, impulse, random } => {
                    let r = self.rng.random::<f32>();
                    // C# y points down.
                    let v = Vec2::new(impulse[0] + random[0] * r, -(impulse[1] + random[1] * r));
                    self.out.impulses.extend(target(item).map(|id| (id, v)));
                }
                Act::Damage { item, amount, direction } => {
                    self.out.damage.extend(target(item).map(|id| (id, amount, direction)));
                }
                Act::LevelComplete => self.out.complete = true,
                Act::Restart => self.out.restart = true,
                Act::Nothing => {}
                Act::Unsupported(kind) => {
                    if self.reported.0.insert(kind.clone()) {
                        info!("action {name:?}: {kind} is not ported yet");
                    }
                }
            }
            if let Some(next) = resolved.next {
                self.run(&next, actor, depth + 1);
            }
        }
    }
}

/// `Entity.Activate` / `Deactivate`.
fn set_active(mut commands: Commands, mut changes: MessageReader<SetEntityActive>) {
    for change in changes.read() {
        let Ok(mut e) = commands.get_entity(change.entity) else {
            continue;
        };
        if change.active {
            e.remove::<(Inactive, RigidBodyDisabled, ColliderDisabled)>()
                .insert(Visibility::Inherited);
        } else {
            e.insert((Inactive, RigidBodyDisabled, ColliderDisabled, Visibility::Hidden));
        }
    }
}

/// `Entity.Destroy`: explosives blow up, the player's car wrecks.
fn destroy_entities(
    mut commands: Commands,
    mut destroys: MessageReader<DestroyEntity>,
    things: Query<(&Transform, Option<&Explosive>, Option<&Health>)>,
    mut explosions: MessageWriter<Explosion>,
    mut destroyed: MessageWriter<Destroyed>,
) {
    for DestroyEntity(entity) in destroys.read() {
        let Ok((t, explosive, health)) = things.get(*entity) else {
            continue;
        };
        if health.is_some() {
            destroyed.write(Destroyed(*entity));
            continue;
        }
        if let Some(explosive) = explosive {
            explosions.write(Explosion {
                at: t.translation.truncate(),
                blast: explosive.blast,
            });
        }
        commands.entity(*entity).despawn();
    }
}
