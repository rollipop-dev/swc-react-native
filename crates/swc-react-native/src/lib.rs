//! Collection of SWC implementations for React Native.
//!
//! Each transform is gated behind a feature flag and re-exported as a submodule.

#[cfg(feature = "codegen")]
pub use swc_react_native_codegen::{codegen, CodegenOptions, CodegenVisitor};

#[cfg(feature = "hermes-v1-fixes")]
pub use swc_react_native_hermes_v1_fixes::{
    async_arrow_non_simple_params, class_in_finally, super_in_object_accessor,
    AsyncArrowNonSimpleParamsVisitor, ClassInFinallyVisitor, SuperInObjectAccessorVisitor,
};

#[cfg(feature = "worklets")]
pub use swc_react_native_worklets::{worklets, WorkletsOptions, WorkletsVisitor};
