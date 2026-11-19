//! `DecoratorCondtion` (`While`) and the `Timer` made of it.

use flatbt::inspect::{Inspector, NodeInfo};
use flatbt::{BtNode, Entry, EntryMode, NodeResult};

use crate::ai::blackboard::AiBlackboard;

/// `BTLib.DecoratorCondtion` (`bt.While`): runs its child while a condition
/// says `Running`; `Ok` or `Fail` ends it with that result, the child
/// dropped. `init` runs when the node is entered afresh, before the first
/// condition.
///
/// BTLib asked the condition in the thinking pass (`Context.Think`) only:
/// `Context.Run()` goes straight to the running action. Here the same: on
/// `Evaluate` it runs `init` (when fresh) and the condition; on `Resume` of
/// a running one it passes to the child without asking, so a decorator's
/// condition changes what runs only when the tree is thought over.
pub struct DecoratorCondition<Init, Check, Child> {
    init: Init,
    check: Check,
    child: Child,
}

/// [`DecoratorCondition`]: `init` when entered afresh; `check` when thinking:
/// `Running` goes on with `child`, `Success` ends `Ok`, `Failure` `Fail`.
pub fn decorator_condition<Init, Check, Child>(
    init: Init,
    check: Check,
    child: Child,
) -> DecoratorCondition<Init, Check, Child> {
    DecoratorCondition { init, check, child }
}

/// The decorator's invocation: whether it was entered before, and its child's
/// state.
#[derive(Default)]
pub struct WhileState<Child> {
    entered: bool,
    child: Child,
}

impl<Context, Act, Init, Check, Child> BtNode<Context, Act> for DecoratorCondition<Init, Check, Child>
where
    Init: Fn(&mut Context),
    Check: Fn(&mut Context) -> NodeResult<()>,
    Child: BtNode<Context, Act>,
{
    type State = WhileState<Child::State>;
    type Memory = Child::Memory;
    const NODES: usize = 1 + Child::NODES;

    fn update(
        &self,
        state: &mut Self::State,
        memory: &mut Self::Memory,
        ctx: &mut Context,
        _: (),
        entry: Entry<'_>,
    ) -> NodeResult<Act> {
        if state.entered && entry.mode() == EntryMode::Resume {
            return entry.run(1, &self.child, &mut state.child, memory, ctx, ());
        }
        if !state.entered {
            (self.init)(ctx);
            state.entered = true;
        }
        match (self.check)(ctx) {
            NodeResult::Running(()) => entry.run(1, &self.child, &mut state.child, memory, ctx, ()),
            NodeResult::Success => NodeResult::Success,
            NodeResult::Failure => NodeResult::Failure,
        }
    }

    fn inspect(&self, state: Option<&Self::State>, memory: &Self::Memory, inspector: &mut dyn Inspector) {
        let node = NodeInfo::new("while", state.is_some());
        inspector.node(node, |inspector| {
            BtNode::<Context, Act>::inspect(&self.child, state.map(|s| &s.child), memory, inspector);
        });
    }
}

/// `BTActionFactory.Timer(timeSec, child)`: `While` the blackboard's timer
/// (started when the node is entered) is under `secs`, run `child`; then
/// `Ok`. A millionth of a second is spare for the float sums of the steps
/// (a step's rounding must not cost a whole think).
pub fn timer<Child>(
    secs: f32,
    child: Child,
) -> DecoratorCondition<impl Fn(&mut AiBlackboard), impl Fn(&mut AiBlackboard) -> NodeResult<()>, Child> {
    decorator_condition(
        |bb: &mut AiBlackboard| bb.start_timer(),
        move |bb: &mut AiBlackboard| {
            if bb.timer() < secs - 1e-6 { NodeResult::RUNNING } else { NodeResult::Success }
        },
        child,
    )
}
