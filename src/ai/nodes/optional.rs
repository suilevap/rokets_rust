//! `OptionalActionNode`: an action that is always in progress, whatever its
//! child does.

use flatbt::BtAction;

use crate::ai::blackboard::AiBlackboard;
use crate::ai::orders::Orders;

/// `BTLib.OptionalActionNode` (`bt.Optional`): runs inside an [`all`](super::all)
/// without holding it back. It starts even when the child does not (unless
/// the start is required), is always in progress, ticks the child while the
/// child *is* in progress, and completes `Ok` without completing the child.
///
/// A child that has not started (its start failed) is started again on
/// each tick until it does, then ticked while it is in progress: a gun
/// whose target comes into sight mid-chase opens fire. C# ticked a child
/// whose start failed all the same, with its fields as the last run left
/// them, which only worked because its nodes were objects that kept their
/// own state.
pub struct Optional<A> {
    child: A,
    start_required: bool,
}

/// `bt.Optional(child)`: starts whatever the child's start says.
pub fn optional<A>(child: A) -> Optional<A> {
    Optional { child, start_required: false }
}

/// `bt.Optional(child, startSuccessRequired: true)`: starts when the child
/// does.
pub fn optional_required<A>(child: A) -> Optional<A> {
    Optional { child, start_required: true }
}

impl<A> BtAction<AiBlackboard, Orders> for Optional<A>
where
    A: BtAction<AiBlackboard, Orders>,
{
    /// The child's state, `None` when its start failed.
    type State = Option<A::State>;

    fn start(&self, bb: &mut AiBlackboard, _: ()) -> Option<Self::State> {
        let started = self.child.start(bb, ());
        if self.start_required && started.is_none() {
            return None;
        }
        Some(started)
    }

    fn is_in_progress(&self, _: &Self::State, _: &AiBlackboard, _: ()) -> bool {
        true
    }

    fn tick(&self, state: &mut Self::State, bb: &mut AiBlackboard, _: ()) -> Orders {
        if state.is_none() {
            *state = self.child.start(bb, ());
        }
        match state {
            Some(child) if self.child.is_in_progress(child, bb, ()) => self.child.tick(child, bb, ()),
            _ => Orders::default(),
        }
    }
}
