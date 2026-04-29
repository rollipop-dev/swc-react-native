//! Collection of SWC visitors and utilities for React Native code transformation.
//!
//! Each transform is gated behind a feature flag and re-exported as a submodule.

#[cfg(feature = "codegen")]
pub use swc_react_native_codegen as codegen;
