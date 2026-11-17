//! Map actions: what triggers, sequences and `RunOnMapStart` set off.
//!
//! C# origin: `ActionRunner` (a registry of handlers by action type),
//! `ActionTemplates` and `TriggerManager.RunAction` (2012 on).
//!
//! **The core knows no action.** Each action type is a struct that reads
//! its own fields from the map's XML ([`MapAction`]), optionally made by
//! templates too, and a Bevy system that carries it out, returning
//! whether it succeeded. A plugin registers both in one line:
//!
//! ```ignore
//! app.add_map_action::<ApplyImpulse, _>(apply_impulse);
//! ```
//!
//! A new action is a new file and that line (see `entity.rs` for small
//! examples); nothing here changes.
//!
//! The runner: a [`RunAction`] message names an action (a map action, or
//! a template such as `ActivateEntity(1001)`) and the map id of the entity
//! that set it off (`itemId`). Every map action of that name runs in
//! order, each followed by its `NextAction` if it succeeded. Handlers run
//! one after another with the world to themselves, so a sequence's later
//! steps see what the earlier ones did, as in C#. Action types the port
//! does not have yet are logged once per map.
//!
//! Built in here: `ActionSequence` / `Seq(…)` (now, or `TimeBetweenInSec`
//! apart; `na` steps skipped) and `RunOnMapStart`.

mod body;
mod entity;
mod level;
mod silent;

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::content::actions::{ActionRef, MapActions, TemplateArgs};
use crate::content::xna::Items;
use crate::game::{GameState, LoadedMap, MapId};

pub use entity::Inactive;

/// How deep sequences and `NextAction` chains may nest (a loop guard).
const MAX_DEPTH: usize = 16;

/// An action type: its data, read from the map's XML (serde, field names
/// as in C#), and how templates make it.
pub trait MapAction: DeserializeOwned + Send + Sync + 'static {
    /// The C# class, without `GameLibrary.` (`ApplyImpulseActionData`).
    const TYPE: &'static str;

    /// Templates that make it: `("Impulse", |args| …)` for
    /// `Impulse(itemId,impulseX,impulseY)`.
    fn templates() -> Templates<Self> {
        Vec::new()
    }
}

/// The templates an action type answers to, each with how it reads its
/// arguments.
pub type Templates<T> = Vec<(&'static str, fn(&TemplateArgs) -> Option<T>)>;

/// What a handler gets: the action's data and who set it off.
pub struct Run<T> {
    pub data: T,
    /// The map id of the entity that set the action off (`itemId`).
    pub actor: Option<i64>,
    /// How deep in sequences this runs (for actions that run others).
    pub depth: usize,
}

impl<T> Run<T> {
    /// `GetId`: an action's `ItemId` of 0 or less means the actor.
    pub fn target(&self, item: i64) -> Option<i64> {
        if item > 0 { Some(item) } else { self.actor }
    }
}

/// Run an action (a map action's name or a template) for `actor`.
#[derive(Message, Debug, Clone)]
pub struct RunAction {
    pub name: String,
    pub actor: Option<i64>,
}

/// Map ids to entities, for handlers.
#[derive(SystemParam)]
pub struct MapIds<'w, 's> {
    ids: Query<'w, 's, (Entity, &'static MapId)>,
}

impl MapIds<'_, '_> {
    pub fn entity(&self, id: Option<i64>) -> Option<Entity> {
        let id = id?;
        self.ids.iter().find(|(_, m)| m.0 == id).map(|(e, _)| e)
    }
}

type Handler<A> = Arc<dyn Fn(&mut World, A, Option<i64>, usize) -> bool + Send + Sync>;
type Reader = fn(&str) -> Result<(), String>;

/// The action types and templates the port has.
#[derive(Resource, Default)]
pub struct ActionRegistry {
    by_type: HashMap<&'static str, Handler<String>>,
    readers: HashMap<&'static str, Reader>,
    by_template: HashMap<&'static str, Handler<TemplateArgs>>,
}

impl ActionRegistry {
    /// Whether an action of type `kind` reads from `xml`: `None` if the
    /// type is not ported.
    pub fn read(&self, kind: &str, xml: &str) -> Option<Result<(), String>> {
        self.readers.get(kind).map(|read| read(xml))
    }
}

pub trait AddMapAction {
    /// Registers an action type and its templates, carried out by `system`
    /// (`In<Run<T>>` → whether it succeeded).
    fn add_map_action<T: MapAction, M>(&mut self, system: impl IntoSystem<In<Run<T>>, bool, M> + 'static) -> &mut Self;

    /// The same with a function that has the world to itself, for actions
    /// that run other actions.
    fn add_map_action_fn<T: MapAction>(&mut self, run: fn(&mut World, Run<T>) -> bool) -> &mut Self;
}

impl AddMapAction for App {
    fn add_map_action<T: MapAction, M>(&mut self, system: impl IntoSystem<In<Run<T>>, bool, M> + 'static) -> &mut Self {
        let id = self.world_mut().register_system(system);
        register::<T>(self, move |world, data, actor, depth| {
            world.run_system_with(id, Run { data, actor, depth }).unwrap_or(false)
        })
    }

    fn add_map_action_fn<T: MapAction>(&mut self, run: fn(&mut World, Run<T>) -> bool) -> &mut Self {
        register::<T>(self, move |world, data, actor, depth| run(world, Run { data, actor, depth }))
    }
}

fn register<T: MapAction>(
    app: &mut App,
    call: impl Fn(&mut World, T, Option<i64>, usize) -> bool + Send + Sync + 'static,
) -> &mut App {
    let call = Arc::new(call);
    let mut registry = app.world_mut().get_resource_or_init::<ActionRegistry>();
    let from_xml = call.clone();
    registry.by_type.insert(
        T::TYPE,
        Arc::new(move |world, xml: String, actor, depth| match quick_xml::de::from_str::<T>(&xml) {
            Ok(data) => from_xml(world, data, actor, depth),
            Err(e) => {
                warn!("{}: {e}", T::TYPE);
                false
            }
        }),
    );
    registry
        .readers
        .insert(T::TYPE, |xml| quick_xml::de::from_str::<T>(xml).map(|_| ()).map_err(|e| e.to_string()));
    for (name, build) in T::templates() {
        let call = call.clone();
        registry.by_template.insert(
            name,
            Arc::new(move |world, args: TemplateArgs, actor, depth| {
                build(&args).is_some_and(|data| call(world, data, actor, depth))
            }),
        );
    }
    app
}

/// The actions of the map being played (set when it starts).
#[derive(Resource, Default)]
pub struct CurrentActions(pub MapActions);

/// Sequences waiting for their turn: seconds left, then the action.
#[derive(Resource, Default)]
struct Delayed(Vec<(f32, RunAction)>);

/// The action types of this map reported as not ported.
#[derive(Resource, Default)]
struct Reported(BTreeSet<String>);

/// The runner, for systems to run after.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActionSystems;

pub struct ActionsPlugin;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RunAction>()
            .init_resource::<ActionRegistry>()
            .init_resource::<Delayed>()
            .init_resource::<Reported>()
            .init_resource::<CurrentActions>()
            .add_map_action_fn::<Sequence>(run_sequence)
            .add_plugins((
                entity::EntityActionsPlugin,
                body::BodyActionsPlugin,
                level::LevelActionsPlugin,
                silent::SilentActionsPlugin,
            ))
            .add_systems(OnEnter(GameState::Playing), run_on_map_start)
            .add_systems(
                Update,
                (tick_delayed, run_actions)
                    .chain()
                    .in_set(ActionSystems)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

/// `TriggerManager.MapLoaded`: actions marked `RunOnMapStart`.
fn run_on_map_start(
    map: Res<LoadedMap>,
    mut current: ResMut<CurrentActions>,
    mut delayed: ResMut<Delayed>,
    mut reported: ResMut<Reported>,
    mut run: MessageWriter<RunAction>,
) {
    current.0 = map.data.actions.clone();
    delayed.0.clear();
    reported.0.clear();
    for a in current.0.0.iter().filter(|a| a.run_on_map_start) {
        run.write(RunAction {
            name: a.name.clone(),
            actor: None,
        });
    }
}

fn tick_delayed(time: Res<Time>, mut delayed: ResMut<Delayed>, mut run: MessageWriter<RunAction>) {
    let dt = time.delta_secs();
    let mut due = Vec::new();
    delayed.0.retain_mut(|(left, action)| {
        *left -= dt;
        let keep = *left > 0.0;
        if !keep {
            due.push(action.clone());
        }
        keep
    });
    run.write_batch(due);
}

fn run_actions(world: &mut World) {
    let requests: Vec<RunAction> = world.resource_mut::<Messages<RunAction>>().drain().collect();
    for r in requests {
        run(world, &r.name, r.actor, 0);
    }
}

/// Runs an action reference now; whether it succeeded
/// (`TriggerManager.RunAction`).
pub fn run(world: &mut World, reference: &str, actor: Option<i64>, depth: usize) -> bool {
    if depth > MAX_DEPTH || reference.trim().is_empty() {
        return false;
    }
    let r = ActionRef::parse(reference);
    if let Some(args) = r.args {
        let handler = world.resource::<ActionRegistry>().by_template.get(r.name.as_str()).cloned();
        return match handler {
            Some(h) => h(world, TemplateArgs(args), actor, depth),
            None => {
                report(world, &format!("{}()", r.name));
                false
            }
        };
    }
    let actions: Vec<_> = world.resource::<CurrentActions>().0.named(&r.name).cloned().collect();
    let mut any = false;
    for a in actions {
        let handler = world.resource::<ActionRegistry>().by_type.get(a.kind.as_str()).cloned();
        let ok = match handler {
            Some(h) => h(world, a.xml, actor, depth),
            None => {
                report(world, &a.kind);
                false
            }
        };
        let ok = match (&a.next, ok) {
            (Some(next), true) => run(world, next, actor, depth + 1),
            _ => ok,
        };
        any |= ok;
    }
    any
}

fn report(world: &mut World, kind: &str) {
    if world.resource_mut::<Reported>().0.insert(kind.to_string()) {
        info!("action {kind} is not ported yet");
    }
}

/// `ActionSequence`, `Seq(a b c, seconds)`.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Sequence {
    #[serde(default)]
    actions: Items<String>,
    #[serde(default)]
    time_between_in_sec: f32,
}

impl MapAction for Sequence {
    const TYPE: &'static str = "ActionSequence";

    fn templates() -> Templates<Self> {
        vec![("Seq", |args| {
            Some(Sequence {
                actions: Items { items: args.words(0) },
                time_between_in_sec: args.num(1).unwrap_or(0.0),
            })
        })]
    }
}

fn run_sequence(world: &mut World, run_: Run<Sequence>) -> bool {
    let step = run_.data.time_between_in_sec;
    for (i, name) in run_.data.actions.items.iter().enumerate() {
        if name.eq_ignore_ascii_case("na") {
            continue;
        }
        if step > 0.0 && i > 0 {
            let later = RunAction {
                name: name.clone(),
                actor: run_.actor,
            };
            world.resource_mut::<Delayed>().0.push((step * i as f32, later));
        } else {
            run(world, name, run_.actor, run_.depth + 1);
        }
    }
    true
}
