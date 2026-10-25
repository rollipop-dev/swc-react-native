// Shared-traversal port from rollipop-dev/rolldown#58 (c75017c1e).
mod async_arrow_non_simple_params;
mod class_in_finally;
mod super_in_object_accessor;

#[cfg(test)]
mod tests;

use swc_ecma_hooks::{CompositeHook, VisitMutHook};

pub(crate) fn hermes_v1_fixes_hook<C>() -> impl VisitMutHook<C> {
    CompositeHook {
        first: async_arrow_non_simple_params::AsyncArrowNonSimpleParamsHook,
        second: CompositeHook {
            first: super_in_object_accessor::SuperInObjectAccessorHook::default(),
            second: class_in_finally::ClassInFinallyHook::default(),
        },
    }
}
