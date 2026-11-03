//! Collection of SWC implementations for React Native.
//!
//! Each transform is gated behind a feature flag and re-exported as a submodule.

#[cfg(feature = "codegen")]
pub use swc_react_native_codegen::{codegen, CodegenOptions, CodegenVisitor};

#[cfg(feature = "hermes-v1-fixes")]
pub use swc_react_native_hermes_v1_fixes::{
    async_arrow_non_simple_params as fix_async_arrow_non_simple_params,
    class_in_finally as fix_class_in_finally, hermes_v1_fixes_hook,
    super_in_object_accessor as fix_super_in_object_accessor,
    AsyncArrowNonSimpleParamsVisitor as FixAsyncArrowNonSimpleParamsVisitor,
    ClassInFinallyVisitor as FixClassInFinallyVisitor,
    SuperInObjectAccessorVisitor as FixSuperInObjectAccessorVisitor,
};

#[cfg(feature = "worklets")]
pub use swc_react_native_worklets::{
    worklets, CommonOptions, HbcBinaryResolver, Options, ReanimatedOptions, ReanimatedVisitor,
    WorkletsOptions, WorkletsVisitor,
};
