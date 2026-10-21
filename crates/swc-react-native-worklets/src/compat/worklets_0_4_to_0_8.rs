// Port of the bound-error exceptions in Worklets 0.4–0.8's getClosure and
// globals.ts. These constructors are registered on the UI runtime, without
// __classFactory. The exceptions were removed in Worklets 0.9.1.
// https://github.com/software-mansion/react-native-reanimated/blob/worklets-0.8.3/packages/react-native-worklets/plugin/src/closure.ts

use rustc_hash::FxHashSet;
use swc_ecma_ast::{Id, Ident};

pub(super) fn filter_closure(closure: &mut Vec<Ident>, bindings: &FxHashSet<Id>) {
    closure.retain(|variable| {
        !bindings.contains(&variable.to_id())
            || !matches!(variable.sym.as_ref(), "ReanimatedError" | "WorkletsError")
    });
}
