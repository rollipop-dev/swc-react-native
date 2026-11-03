// SWC port of react-native-worklets/plugin-oxc's official Rust implementation.
// Legacy Eval output is kept in sync with react-native-worklets/plugin.

mod ast;
mod autoworkletization;
mod closure;
mod compat;
mod directives;
mod file_directive;
mod gestures;
mod globals;
mod hash;
mod hooks;
mod imports;
mod inline_style;
mod legacy_classes;
mod naming;
mod options;
mod referenced_worklets;
mod transform;
mod visitor;
mod worklet_factory;
mod worklet_file;
mod worklet_pass;
mod worklet_string_code;

use swc_common::{sync::Lrc, SourceMap};
use swc_ecma_ast::Pass;

#[doc(hidden)]
pub use worklet_pass::WorkletsVisitor;

#[doc(hidden)]
pub use compat::ra3::WorkletsVisitor as ReanimatedVisitor;

pub use compat::ra3::ReanimatedOptions;
pub use options::{CommonOptions, HbcBinaryResolver, Options, WorkletsOptions};
pub use worklet_file::EmittedFile;

#[inline]
pub fn worklets(cm: Lrc<SourceMap>, options: Options) -> impl Pass {
    visitor::worklets_pass(options, cm)
}
