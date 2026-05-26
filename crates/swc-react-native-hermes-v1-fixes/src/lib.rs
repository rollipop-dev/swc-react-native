// Ports Expo's Hermes v1 preset fixes:
// https://github.com/expo/expo/blob/d427a802b11d79c8e45be8ceb582ca10fae103af/packages/babel-preset-expo/src/configs/hermes-v1.ts

mod ast;
mod visitors;

#[cfg(test)]
mod test_utils;

use swc_ecma_ast::Pass;
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
