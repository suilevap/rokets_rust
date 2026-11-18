//! `SetBodyRestrictions`: lock a body's movement along x, y or its
//! rotation (`Entity.SetRestrictions`); locked every way, it stands still
//! for good (a static body), as C# made it.

use super::{AddMapAction, MapIds, Run};
use crate::content::actions::defs::SetBodyRestrictions;
use avian2d::prelude::*;
use bevy::prelude::*;

pub struct BodyActionsPlugin;

impl Plugin for BodyActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<SetBodyRestrictions, _>(set_body_restrictions);
    }
}

fn set_body_restrictions(
    In(run): In<Run<SetBodyRestrictions>>,
    ids: MapIds,
    mut commands: Commands,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    let d = &run.data;
    if d.restrict_x && d.restrict_y && d.restrict_rotation {
        commands.entity(e).insert(RigidBody::Static);
        return true;
    }
    let mut axes = LockedAxes::new();
    if d.restrict_x {
        axes = axes.lock_translation_x();
    }
    if d.restrict_y {
        axes = axes.lock_translation_y();
    }
    if d.restrict_rotation {
        axes = axes.lock_rotation();
    }
    commands.entity(e).insert((RigidBody::Dynamic, axes));
    true
}
