//! Reanimated 3.19.5 Babel compatibility adapters.
//!
//! Each module follows its upstream plugin/src counterpart. RA3 owns its
//! visitor and factory; only low-level AST analysis and emission are shared
//! with the current Worklets implementation.

mod autoworkletization;
mod context_object;
mod file;
mod globals;
mod options;
mod web_optimization;
mod worklet_factory;
mod worklet_pass;
mod worklet_string_code;

pub(crate) use context_object::process_context_object;
pub(crate) use globals::{filter_closure, GLOBALS};
pub(crate) use options::validate_options;
pub use options::ReanimatedOptions;
pub(crate) use web_optimization::SubstituteWebChecks;
pub(crate) use worklet_string_code::{prepend_closure, remove_worklet_directive};

pub use worklet_pass::WorkletsVisitor;
