//! How trees write an action: [`act`].

use flatbt::inspect::{Named, named, type_label};
use flatbt::nodes::ActionNode;
use flatbt::action;

/// An action node named in debug paths after the action's own type
/// (`MoveToPoint`, `FireToTarget`, `All`), the way trees write a `BTActions`
/// action.
///
/// C# origin: BTLib's `ActionNode`. Not ported: its thinking pass
/// (`Context.Think`) started an action without ticking it, so the action gave
/// its first orders one physics step (1/60 s) after it started; here it gives
/// them in the step that starts it.
pub fn act<A>(inner: A) -> Named<ActionNode<A>> {
    named(type_label::<A>(), action(inner))
}
