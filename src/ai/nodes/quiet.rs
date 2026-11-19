//! `Context.Think` starts an action without ticking it.

use flatbt::BtAction;

use crate::ai::blackboard::AiBlackboard;
use crate::ai::orders::Orders;

/// An action whose first tick, the one in the update that started it, writes
/// nothing.
///
/// C# origin: `ActionNode.OnUpdate` (the thinking pass) calls `Start` and
/// `IsInProgress` but never `Tick`; only `Context.Run()` ticks, on the steps
/// after. flatbt's `action` ticks in the update that starts it, so an action
/// would give orders a step earlier than C#'s; wrapping it in `QuietStart`
/// makes that tick give none. Later ticks, resumed or evaluated, are the
/// inner action's.
pub struct QuietStart<A>(A);

/// [`QuietStart`] of `action`.
pub fn quiet_start<A>(action: A) -> QuietStart<A> {
    QuietStart(action)
}

impl<A: BtAction<AiBlackboard, Orders>> BtAction<AiBlackboard, Orders> for QuietStart<A> {
    /// Whether it has been ticked once, and the inner action's state.
    type State = (bool, A::State);

    fn start(&self, bb: &mut AiBlackboard, _: ()) -> Option<Self::State> {
        Some((false, self.0.start(bb, ())?))
    }

    fn is_in_progress(&self, state: &Self::State, bb: &AiBlackboard, _: ()) -> bool {
        self.0.is_in_progress(&state.1, bb, ())
    }

    fn tick(&self, state: &mut Self::State, bb: &mut AiBlackboard, _: ()) -> Orders {
        if !state.0 {
            state.0 = true;
            return Orders::default();
        }
        self.0.tick(&mut state.1, bb, ())
    }

    fn complete(&self, state: &mut Self::State, bb: &mut AiBlackboard, _: ()) -> bool {
        self.0.complete(&mut state.1, bb, ())
    }
}
