// SWC port of react-native-worklets/plugin-oxc's official Rust implementation.
// Legacy Eval output is kept in sync with react-native-worklets/plugin.

mod ast;
mod autoworkletization;
mod closure;
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
mod worklet_factory;
mod worklet_file;
mod worklet_pass;
mod worklet_string_code;

use swc_common::{sync::Lrc, SourceMap};
use swc_ecma_ast::Pass;
use swc_ecma_visit::visit_mut_pass;

#[doc(hidden)]
pub use worklet_pass::WorkletsVisitor;

pub use options::{HbcBinaryResolver, WorkletsOptions};
pub use worklet_file::EmittedFile;

pub fn worklets(cm: Lrc<SourceMap>, options: WorkletsOptions) -> impl Pass {
    visit_mut_pass(worklet_pass::WorkletsVisitor::new(options).with_source_map(cm))
}
