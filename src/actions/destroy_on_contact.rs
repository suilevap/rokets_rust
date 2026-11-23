//! `AttachDestroyOnContact` (`ActionRunner.AttachDestroyOnContactRun`, the
//! `DestroyOnContact` entity component and the `DestroyOnContact(sub)`
//! template): an entity that goes when something touches it.
//!
//! The campaign uses it to make pick-ups out of boxes: a gear star
//! (`TheRoad`), a gun lying on the floor (`DoorsWorld`'s Minigun,
//! `Revenge`'s Railgun) or the magnet's chain (`Teleport`, `TheRoad`'s
//! chain that ends the level) is destroyed when the player's car drives
//! into it, and its `ActionOnDestroy` hands over the prize. The component
//! came with the railgun in 2015 (commit `50bc1195`), for exactly that.
//!
//! What counts: a contact begins between the entity and another map
//! entity (anything with a map id: a car, a box, a door, a sensor such as
//! a teleport; the walls are no entities in C# either) that is visible
//! (`Entity.Visible`: a car just through a teleport is hidden for half a
//! second, a deactivated entity is hidden and has no contacts) and, when
//! the action lists `Subscribers`, whose map id is one of them (a clone
//! counts as its source, `GetSourceCustomId`). No subscribers, or an empty
//! list, means anything. The entity is then destroyed as `DestroyEntity`
//! does (`SheduleDestroy` is `Entity.Destroy` on the next update): it runs
//! its `ActionOnDestroy`, and a bomb or red box would blow up. It does not
//! play a sound or run an action of its own; the contact does not destroy
//! the *other* entity.
//!
//! The contact begins are Avian's [`CollisionStart`]: Farseer called
//! `OnCollision` as a contact began too (sensors included).
//!
//! Not ported: the component being a no-op on an entity that is not a
//! physic entity (a body-less one here is refused, as the action fails);
//! rockets in flight touching it (C# rockets are entities that make
//! contacts, ours are not bodies, they hit through their own code).

use std::collections::HashSet;

use avian2d::prelude::*;
use bevy::prelude::*;

use super::{AddMapAction, CloneOf, Run, RunAction, source_id};
use crate::content::actions::defs::AttachDestroyOnContact;
use crate::game::{GameState, MapId};

/// Destroyed when something touches it.
#[derive(Component, Debug, Clone)]
pub struct DestroyOnContact {
    /// Who it goes for (map ids); `None`: anything.
    pub subscribers: Option<Vec<i64>>,
}

/// Whether a contact with something destroys the entity
/// (`DestroyOnContact.phEnt_OnAnyCollide`): `other` is the map id of what
/// touched it (the source's, for a clone), `None` when it has none.
pub fn destroys(subscribers: Option<&[i64]>, other: Option<i64>, visible: bool) -> bool {
    visible && subscribers.is_none_or(|subs| other.is_some_and(|id| subs.contains(&id)))
}

pub struct DestroyOnContactPlugin;

impl Plugin for DestroyOnContactPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<AttachDestroyOnContact, _>(attach).add_systems(
            FixedUpdate,
            destroy_on_contact
                .after(crate::explosion::ExplosionSystems)
                .before(super::ActionSystems)
                .run_if(in_state(GameState::Playing)),
        );
    }
}

/// `AttachDestroyOnContactRun`: no subscribers, or an empty list, is anyone.
fn attach(
    In(run): In<Run<AttachDestroyOnContact>>,
    ids: super::MapIds,
    bodies: Query<(), With<Collider>>,
    mut commands: Commands,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    if !bodies.contains(e) {
        return false;
    }
    let subscribers = Some(run.data.subscribers).filter(|s| !s.is_empty());
    commands.entity(e).insert((DestroyOnContact { subscribers }, CollisionEventsEnabled));
    true
}

#[allow(clippy::type_complexity)]
fn destroy_on_contact(
    mut commands: Commands,
    mut started: MessageReader<CollisionStart>,
    hosts: Query<(&DestroyOnContact, &MapId)>,
    others: Query<(&MapId, Option<&CloneOf>, Option<&Visibility>)>,
    mut run: MessageWriter<RunAction>,
) {
    let mut done = HashSet::new();
    for event in started.read() {
        for (host, other) in [(event.collider1, event.body2), (event.collider2, event.body1)] {
            let (Ok((component, id)), Some(other)) = (hosts.get(host), other) else {
                continue;
            };
            let Ok((other_id, clone, visibility)) = others.get(other) else {
                continue;
            };
            let visible = !matches!(visibility, Some(Visibility::Hidden));
            let source = Some(source_id(other_id, clone));
            if destroys(component.subscribers.as_deref(), source, visible) && done.insert(host) {
                commands.entity(host).remove::<DestroyOnContact>();
                run.write(RunAction {
                    name: format!("DestroyEntity({})", id.0),
                    actor: Some(other_id.0),
                });
            }
        }
    }
}
