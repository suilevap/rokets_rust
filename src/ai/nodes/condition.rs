//! Timers that end at a think (`BTActionFactory.Timer`, `LeaderAlive`'s five
//! seconds) and `BTActions/WhileTargetAlive`.

use flatbt::inspect::{Inspector, NodeInfo};
use flatbt::nodes::IfElse;
use flatbt::{BtNode, Check, ControlNode, Entry, EntryMode, NodeResult, check, if_else};

use crate::ai::blackboard::AiBlackboard;

/// Runs its child for `secs` from when it is entered, then ends with
/// `expired`: `timer` (`Ok`) or `time_limit` (`Fail`). The child's own end
/// ends it earlier, with the child's result.
///
/// C# origin: `BTActionFactory.Timer` was a `DecoratorCondtion` (`bt.While`)
/// over the blackboard's one `Timer`, started by its `init`; BTLib asked it
/// in the thinking pass (`Context.Think`) only, so a timer ends at the first
/// think after its time (a 1 s timer after 1 to 1.25 s). The same here: the
/// time is looked at when the tree is evaluated, not on `Resume`. Each timer
/// keeps its start in its own state, not on the blackboard (C#'s one shared
/// `Timer` could have been restarted under another; no tree did that).
pub struct ThinkTimer<Child> {
    secs: f32,
    /// What it ends with when the time is up: `Ok`, or `Fail`.
    succeed_on_expiry: bool,
    child: Child,
}

/// [`ThinkTimer`]'s invocation: when it was entered, and its child's state.
#[derive(Default)]
pub struct TimerState<Child> {
    started: Option<f64>,
    child: Child,
}

/// `BTActionFactory.Timer(timeSec, child)`: `child` for `secs`, then `Ok`.
pub fn timer<Child>(secs: f32, child: Child) -> ThinkTimer<Child> {
    ThinkTimer { secs, succeed_on_expiry: true, child }
}

/// `child` for `secs`, then `Fail` (`LeaderAlive`'s `Timer < 5 ? Running : Fail`).
pub fn time_limit<Child>(secs: f32, child: Child) -> ThinkTimer<Child> {
    ThinkTimer { secs, succeed_on_expiry: false, child }
}

impl<Act, Child: BtNode<AiBlackboard, Act>> BtNode<AiBlackboard, Act> for ThinkTimer<Child> {
    type State = TimerState<Child::State>;
    type Memory = Child::Memory;
    const NODES: usize = 1 + Child::NODES;

    fn update(
        &self,
        state: &mut Self::State,
        memory: &mut Self::Memory,
        bb: &mut AiBlackboard,
        _: (),
        entry: Entry<'_>,
    ) -> NodeResult<Act> {
        if state.started.is_some() && entry.mode() == EntryMode::Resume {
            return entry.run(1, &self.child, &mut state.child, memory, bb, ());
        }
        let started = *state.started.get_or_insert(bb.now());
        // A millionth of a second is spare for the float sums of the steps (a
        // step's rounding must not cost a whole think).
        if ((bb.now() - started) as f32) < self.secs - 1e-6 {
            return entry.run(1, &self.child, &mut state.child, memory, bb, ());
        }
        if self.succeed_on_expiry { NodeResult::Success } else { NodeResult::Failure }
    }

    fn inspect(&self, state: Option<&Self::State>, memory: &Self::Memory, inspector: &mut dyn Inspector) {
        let node = NodeInfo::new("timer", state.is_some());
        inspector.node(node, |inspector| {
            inspector.field("secs", &self.secs);
            BtNode::<AiBlackboard, Act>::inspect(&self.child, state.map(|s| &s.child), memory, inspector);
        });
    }
}

/// `BTActions/WhileTargetAlive(child)`: `child` while the blackboard has a
/// target that is alive, asked at each think (a `DecoratorCondtion` with no
/// `init`: flatbt's `if_else` asks its condition when evaluated only); `Fail`
/// when it has none.
pub fn while_target_alive<Child>(child: Child) -> WhileTargetAlive<Child> {
    if_else(has_live_target as Condition, child, check(never as Condition))
}

/// A condition on the blackboard.
pub type Condition = fn(&AiBlackboard) -> bool;

/// [`while_target_alive`]'s node.
pub type WhileTargetAlive<Child> = ControlNode<IfElse<Condition>, (Child, Check<Condition>)>;

/// Never: the branch that fails.
pub fn never(_: &AiBlackboard) -> bool {
    false
}

/// The blackboard has a target, and it is alive.
pub fn has_live_target(bb: &AiBlackboard) -> bool {
    bb.live_target().is_some()
}
