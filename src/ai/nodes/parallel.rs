//! `ParallelAllActionNode`.

use flatbt::BtAction;

use crate::ai::blackboard::AiBlackboard;
use crate::ai::orders::Orders;

/// `BTLib.ParallelAllActionNode` (`bt.All`): actions that run together as
/// one. It starts when every child starts (the first that does not stops the
/// rest, as `Enumerable.All` does), goes on while every child is in progress,
/// ticks all of them in order (the later one's orders over the earlier's) and
/// completes `Ok` when every child does.
///
/// Children are tuples of up to four actions; wrap it in `flatbt::action`.
pub struct All<Children>(Children);

/// [`All`] of `children`.
pub fn all<Children>(children: Children) -> All<Children> {
    All(children)
}

macro_rules! all_for_tuples {
    ($($child:ident $index:tt),+) => {
        impl<$($child: BtAction<AiBlackboard, Orders>),+> BtAction<AiBlackboard, Orders> for All<($($child,)+)> {
            type State = ($($child::State,)+);

            fn start(&self, ctx: &mut AiBlackboard, _: ()) -> Option<Self::State> {
                Some(($(self.0.$index.start(ctx, ())?,)+))
            }

            fn is_in_progress(&self, state: &Self::State, ctx: &AiBlackboard, _: ()) -> bool {
                true $(&& self.0.$index.is_in_progress(&state.$index, ctx, ()))+
            }

            fn tick(&self, state: &mut Self::State, ctx: &mut AiBlackboard, _: ()) -> Orders {
                let mut orders = Orders::default();
                $(orders = orders.then(self.0.$index.tick(&mut state.$index, ctx, ()));)+
                orders
            }

            fn complete(&self, state: &mut Self::State, ctx: &mut AiBlackboard, _: ()) -> bool {
                true $(&& self.0.$index.complete(&mut state.$index, ctx, ()))+
            }
        }
    };
}

all_for_tuples!(A 0, B 1);
all_for_tuples!(A 0, B 1, C 2);
all_for_tuples!(A 0, B 1, C 2, D 3);
