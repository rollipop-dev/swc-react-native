// Ports Expo's Hermes v1 preset fixes:
// https://github.com/expo/expo/blob/d427a802b11d79c8e45be8ceb582ca10fae103af/packages/babel-preset-expo/src/configs/hermes-v1.ts

mod ast;
mod hooks;
mod visitors;

#[cfg(test)]
mod test_utils;

use swc_ecma_ast::Pass;
use swc_ecma_hooks::VisitMutHook;
use swc_ecma_visit::visit_mut_pass;

#[doc(hidden)]
pub use visitors::async_arrow_non_simple_params::AsyncArrowNonSimpleParamsVisitor;
#[doc(hidden)]
pub use visitors::class_in_finally::ClassInFinallyVisitor;
#[doc(hidden)]
pub use visitors::super_in_object_accessor::SuperInObjectAccessorVisitor;

pub fn async_arrow_non_simple_params() -> impl Pass {
    visit_mut_pass(visitors::async_arrow_non_simple_params::AsyncArrowNonSimpleParamsVisitor)
}

pub fn super_in_object_accessor() -> impl Pass {
    visit_mut_pass(visitors::super_in_object_accessor::SuperInObjectAccessorVisitor)
}

pub fn class_in_finally() -> impl Pass {
    visit_mut_pass(visitors::class_in_finally::ClassInFinallyVisitor::default())
}

/// Combines all three Hermes V1 fixes into one composable traversal hook.
///
/// Compose this before SWC's async lowering so non-simple arrow parameters are
/// fixed before lowering and accessor `super` references are collected before
/// their enclosing bodies are moved. Existing individual Pass/Visitor APIs are
/// unchanged.
///
/// ```
/// use swc_ecma_hooks::CompositeHook;
/// use swc_ecma_transformer::{hook_pass, transform_hook, Options};
/// use swc_react_native_hermes_v1_fixes::hermes_v1_fixes_hook;
///
/// let mut options = Options::default();
/// options.env.es2017.async_to_generator = true;
/// let pass = hook_pass(CompositeHook {
///     first: hermes_v1_fixes_hook(),
///     second: transform_hook(options),
/// });
/// ```
pub fn hermes_v1_fixes_hook<C>() -> impl VisitMutHook<C> {
    hooks::hermes_v1_fixes_hook()
}
