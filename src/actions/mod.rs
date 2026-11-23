//! Map actions: what triggers, sequences and `RunOnMapStart` set off.
//!
//! C# origin: `ActionRunner` (a registry of handlers by action type),
//! `ActionTemplates` and `TriggerManager.RunAction` (2012 on).
//!
//! **The core knows no action.** Action types are data, defined apart
//! from the game (`content::actions::defs`: fields, what an editor needs
//! of them, templates). The game gives a type a Bevy system that carries
//! it out, returning whether it succeeded, in one line:
//!
//! ```ignore
//! app.add_map_action::<defs::ApplyImpulse, _>(apply_impulse);
//! ```
//!
//! A new action is its definition, a new file with its system and that
//! line (see `entity.rs` for small examples); nothing here changes.
//!
//! The runner: a [`RunAction`] message names an action (a map action, or
//! a template such as `ActivateEntity(1001)`) and the map id of the entity
//! that set it off (`itemId`). The map actions of that name run in order
//! until one succeeds (as C#'s `result || action.Run()`), followed by its
//! `NextAction`. Handlers run
//! one after another with the world to themselves, so a sequence's later
//! steps see what the earlier ones did, as in C#. Action types the port
//! does not have yet are logged once per map.
//!
//! Built in here: `ActionSequence` / `Seq(…)` (now, or `TimeBetweenInSec`
//! apart; `na` steps skipped) and `RunOnMapStart` (variables first, as
//! C#'s `InitAllVariables`, then the rest in map order).

pub mod area;
mod body;
mod camera_path;
mod clone;
mod control;
pub mod destroy_on_contact;
mod entity;
mod focus;
mod highlight;
mod immortal;
mod level;
mod lifecycle;
mod path;
mod silent;
pub mod splash;
pub mod variables;
mod view;

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::content::actions::defs::ActionSequence;
use crate::content::actions::{ActionDef, ActionRef, MapActions, RawAction, TemplateArgs};
use crate::content::node::Node;
use crate::game::{GameState, LoadedMap, MapId};

pub use clone::{CloneOf, along, source_id};
pub use path::{Leg, legs, offset, uv};
pub use camera_path::stop_camera_path;
pub use entity::Inactive;
pub use highlight::ControlHighlight;
pub use lifecycle::{OnDestroy, Respawned, destroy_action};
pub use splash::{SplashBindText, SplashCommand};
pub use variables::{Owner, Variables};

/// How deep sequences and `NextAction` chains may nest (a loop guard).
const MAX_DEPTH: usize = 16;

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
        target_id(item, self.actor)
    }
}

/// `GetId`: an item id of 0 or less means the actor.
pub fn target_id(item: i64, actor: Option<i64>) -> Option<i64> {
    if item > 0 { Some(item) } else { actor }
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

/// The action types and templates the game carries out.
#[derive(Resource, Default)]
pub struct ActionRegistry {
    by_type: HashMap<&'static str, Handler<Node>>,
    sprites: HashMap<&'static str, &'static [&'static str]>,
    /// Fields whose value names a sprite to load (`Sprite`).
    sprite_fields: HashMap<&'static str, &'static str>,
    /// Types whose sprites depend on what the action says (`SetWeapon`'s
    /// guns), by the function that reads them off its node.
    sprite_fns: HashMap<&'static str, fn(&Node) -> Vec<String>>,
    /// The same for a template (`SetWeapon(Railgun)`), by name.
    template_sprites: HashMap<&'static str, fn(&TemplateArgs) -> Vec<String>>,
    /// By name, with how many arguments each takes (`NextMap()` and
    /// `NextMap(stars)` are two).
    by_template: HashMap<&'static str, Vec<(usize, Handler<TemplateArgs>)>>,
}

impl ActionRegistry {
    /// The sprites an action of type `kind` draws.
    pub fn sprites(&self, kind: &str) -> &'static [&'static str] {
        self.sprites.get(kind).copied().unwrap_or(&[])
    }

    /// The sprites one map action draws: its type's, and the one its
    /// sprite field names.
    pub fn sprites_of(&self, action: &RawAction) -> Vec<String> {
        let mut out: Vec<String> = self.sprites(&action.kind).iter().map(|s| s.to_string()).collect();
        if let Some(field) = self.sprite_fields.get(action.kind.as_str())
            && let Some(n) = action.node.child(field)
            && !n.text.trim().is_empty()
        {
            out.push(n.text.trim().to_string());
        }
        if let Some(of) = self.sprite_fns.get(action.kind.as_str()) {
            out.extend(of(&action.node));
        }
        // Templates named in its items (a sequence's steps).
        fn texts(node: &Node, out: &mut Vec<String>, registry: &ActionRegistry) {
            out.extend(registry.sprites_of_text(&node.text));
            for child in &node.children {
                texts(child, out, registry);
            }
        }
        texts(&action.node, &mut out, self);
        out
    }

    /// The sprites an action written as text (`SetWeapon(Railgun)`, as an
    /// `ActionOnCreate` or a sequence's step) draws.
    pub fn sprites_of_text(&self, text: &str) -> Vec<String> {
        let reference = ActionRef::parse(text);
        match (&reference.args, self.template_sprites.get(reference.name.as_str())) {
            (Some(args), Some(of)) => of(&TemplateArgs(args.clone())),
            _ => Vec::new(),
        }
    }

    /// Whether the game carries out actions of type `kind`.
    pub fn has(&self, kind: &str) -> bool {
        self.by_type.contains_key(kind)
    }

    /// The template of a name for `args`: the one taking that many, or
    /// else the first.
    fn template(&self, name: &str, args: usize) -> Option<Handler<TemplateArgs>> {
        let all = self.by_template.get(name)?;
        all.iter()
            .find(|(n, _)| *n == args)
            .or(all.first())
            .map(|(_, h)| h.clone())
    }
}

pub trait AddMapAction {
    /// Has actions of type `T` and its templates carried out by `system`
    /// (`In<Run<T>>` → whether it succeeded).
    fn add_map_action<T: ActionDef, M>(
        &mut self,
        system: impl IntoSystem<In<Run<T>>, bool, M> + 'static,
    ) -> &mut Self;

    /// The same with a function that has the world to itself, for actions
    /// that run other actions.
    fn add_map_action_fn<T: ActionDef>(&mut self, run: fn(&mut World, Run<T>) -> bool)
    -> &mut Self;

    /// Sprites actions of type `T` draw, loaded with any map that has one.
    fn add_action_sprites<T: ActionDef>(&mut self, sprites: &'static [&'static str]) -> &mut Self;

    /// The field of `T` that names a sprite each action draws.
    fn add_action_sprite_field<T: ActionDef>(&mut self, field: &'static str) -> &mut Self;

    /// The sprites an action of type `T` draws, as its own fields say
    /// (read off its XML node).
    fn add_action_sprites_by<T: ActionDef>(&mut self, of: fn(&Node) -> Vec<String>) -> &mut Self;

    /// The sprites a template draws, by its name, as its arguments say.
    fn add_template_sprites(&mut self, name: &'static str, of: fn(&TemplateArgs) -> Vec<String>) -> &mut Self;
}

impl AddMapAction for App {
    fn add_map_action<T: ActionDef, M>(
        &mut self,
        system: impl IntoSystem<In<Run<T>>, bool, M> + 'static,
    ) -> &mut Self {
        let id = self.world_mut().register_system(system);
        register::<T>(self, move |world, data, actor, depth| {
            world
                .run_system_with(id, Run { data, actor, depth })
                .unwrap_or(false)
        })
    }

    fn add_map_action_fn<T: ActionDef>(
        &mut self,
        run: fn(&mut World, Run<T>) -> bool,
    ) -> &mut Self {
        register::<T>(self, move |world, data, actor, depth| {
            run(world, Run { data, actor, depth })
        })
    }

    fn add_action_sprites<T: ActionDef>(&mut self, sprites: &'static [&'static str]) -> &mut Self {
        let mut registry = self.world_mut().get_resource_or_init::<ActionRegistry>();
        registry.sprites.insert(T::INFO.kind, sprites);
        self
    }

    fn add_action_sprite_field<T: ActionDef>(&mut self, field: &'static str) -> &mut Self {
        let mut registry = self.world_mut().get_resource_or_init::<ActionRegistry>();
        registry.sprite_fields.insert(T::INFO.kind, field);
        self
    }

    fn add_action_sprites_by<T: ActionDef>(&mut self, of: fn(&Node) -> Vec<String>) -> &mut Self {
        let mut registry = self.world_mut().get_resource_or_init::<ActionRegistry>();
        registry.sprite_fns.insert(T::INFO.kind, of);
        self
    }

    fn add_template_sprites(&mut self, name: &'static str, of: fn(&TemplateArgs) -> Vec<String>) -> &mut Self {
        let mut registry = self.world_mut().get_resource_or_init::<ActionRegistry>();
        registry.template_sprites.insert(name, of);
        self
    }
}

fn register<T: ActionDef>(
    app: &mut App,
    call: impl Fn(&mut World, T, Option<i64>, usize) -> bool + Send + Sync + 'static,
) -> &mut App {
    let call = Arc::new(call);
    let mut registry = app.world_mut().get_resource_or_init::<ActionRegistry>();
    let from_node = call.clone();
    registry.by_type.insert(
        T::INFO.kind,
        Arc::new(
            move |world, node: Node, actor, depth| match T::read(&node) {
                Ok(data) => from_node(world, data, actor, depth),
                Err(e) => {
                    warn!("{}: {e}", T::INFO.kind);
                    false
                }
            },
        ),
    );
    for (signature, build) in T::templates() {
        let (name, params) = signature.split_once('(').unwrap_or((signature, ")"));
        let arity = params
            .trim_end_matches(')')
            .split(',')
            .filter(|p| !p.trim().is_empty())
            .count();
        let call = call.clone();
        let handler: Handler<TemplateArgs> =
            Arc::new(move |world, args: TemplateArgs, actor, depth| {
                build(&args).is_some_and(|data| call(world, data, actor, depth))
            });
        registry
            .by_template
            .entry(name)
            .or_default()
            .push((arity, handler));
    }
    app
}

/// The actions of the map being played (set when it starts).
#[derive(Resource, Default)]
pub struct CurrentActions(pub MapActions);

/// The `RunOnMapStart` actions of the map just started, by index; run
/// on the first frame, once its entities exist.
#[derive(Resource, Default)]
struct StartQueue(Vec<usize>);

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
            .init_resource::<StartQueue>()
            .add_map_action_fn::<ActionSequence>(run_sequence)
            .add_plugins((
                entity::EntityActionsPlugin,
                body::BodyActionsPlugin,
                camera_path::CameraPathPlugin,
                clone::CloneActionsPlugin,
                control::ControlActionsPlugin,
                destroy_on_contact::DestroyOnContactPlugin,
                focus::FocusActionsPlugin,
                highlight::HighlightActionsPlugin,
                level::LevelActionsPlugin,
                lifecycle::LifecyclePlugin,
                path::PathActionsPlugin,
                silent::SilentActionsPlugin,
                splash::SplashActionsPlugin,
                variables::VariablesPlugin,
                view::ViewActionsPlugin,
            ))
            .add_plugins((area::AreaActionPlugin, immortal::ImmortalActionPlugin))
            .add_systems(OnEnter(GameState::Playing), run_on_map_start)
            // At the end of the fixed step: what the step set off runs in it.
            .configure_sets(FixedUpdate, ActionSystems.after(crate::explosion::ExplosionSystems))
            .add_systems(
                FixedUpdate,
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
    mut start: ResMut<StartQueue>,
) {
    current.0 = map.data.actions.clone();
    delayed.0.clear();
    reported.0.clear();
    start.0 = start_order(&current.0);
}

/// The `RunOnMapStart` actions by index, in the order they run: variables
/// first (`InitAllVariables`), so the rest see them, then map order. Each
/// runs itself, not by name: several may share one (eight
/// `CreateTeleport`s on Unboxing).
pub fn start_order(actions: &MapActions) -> Vec<usize> {
    let mut queue: Vec<usize> = (0..actions.0.len()).filter(|&i| actions.0[i].base.run_on_map_start).collect();
    queue.sort_by_key(|&i| !is_variable(&actions.0[i].kind));
    queue
}

/// C#'s `Variable` actions.
fn is_variable(kind: &str) -> bool {
    matches!(kind, "ChangeIntVariable" | "CheckIntVariable")
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
    let start = std::mem::take(&mut world.resource_mut::<StartQueue>().0);
    for i in start {
        let action = world.resource::<CurrentActions>().0.0[i].clone();
        run_one(world, action, None, 0);
    }
    let requests: Vec<RunAction> = world
        .resource_mut::<Messages<RunAction>>()
        .drain()
        .collect();
    for r in requests {
        run(world, &r.name, r.actor, 0);
    }
}

/// Runs an action `seconds` from now.
pub fn run_later(world: &mut World, seconds: f32, action: RunAction) {
    world.resource_mut::<Delayed>().0.push((seconds, action));
}

/// Runs an action reference now; whether it succeeded
/// (`TriggerManager.RunAction`).
pub fn run(world: &mut World, reference: &str, actor: Option<i64>, depth: usize) -> bool {
    debug!("run {reference} for {actor:?}");
    if depth > MAX_DEPTH || reference.trim().is_empty() {
        return false;
    }
    let r = ActionRef::parse(reference);
    if let Some(args) = r.args {
        let handler = world
            .resource::<ActionRegistry>()
            .template(&r.name, args.len());
        return match handler {
            Some(h) => h(world, TemplateArgs(args), actor, depth),
            None => {
                report(world, &format!("{}()", r.name));
                false
            }
        };
    }
    let actions: Vec<_> = world
        .resource::<CurrentActions>()
        .0
        .named(&r.name)
        .cloned()
        .collect();
    // `RunActions`: `result = result || action.Run(itemId)`, so once one
    // succeeds the others of the name are skipped. The maps rely on it
    // (JunkYard's two `CloneObj3007_3` made one clone).
    for a in actions {
        if run_one(world, a, actor, depth) {
            return true;
        }
    }
    false
}

/// Runs one map action, then its `NextAction` if it succeeded.
fn run_one(world: &mut World, a: RawAction, actor: Option<i64>, depth: usize) -> bool {
    let handler = world
        .resource::<ActionRegistry>()
        .by_type
        .get(a.kind.as_str())
        .cloned();
    let ok = match handler {
        Some(h) => h(world, a.node, actor, depth),
        None => {
            report(world, &a.kind);
            false
        }
    };
    match (a.base.next(), ok) {
        (Some(next), true) => run(world, next, actor, depth + 1),
        _ => ok,
    }
}

fn report(world: &mut World, kind: &str) {
    if world.resource_mut::<Reported>().0.insert(kind.to_string()) {
        info!("action {kind} is not ported yet");
    }
}

/// `ActionSequence`, `Seq(a b c, seconds)`.
fn run_sequence(world: &mut World, run_: Run<ActionSequence>) -> bool {
    let step = run_.data.time_between_in_sec;
    for (i, name) in run_.data.actions.iter().enumerate() {
        if name.eq_ignore_ascii_case("na") {
            continue;
        }
        if step > 0.0 && i > 0 {
            let later = RunAction {
                name: name.clone(),
                actor: run_.actor,
            };
            world
                .resource_mut::<Delayed>()
                .0
                .push((step * i as f32, later));
        } else {
            run(world, name, run_.actor, run_.depth + 1);
        }
    }
    true
}
