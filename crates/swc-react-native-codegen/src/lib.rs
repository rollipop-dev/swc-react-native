// Corresponds to `index.js` in react-native/packages/babel-plugin-codegen/

mod codegen;
mod options;
mod visitor;

use swc_common::{sync::Lrc, SourceMap};
use swc_ecma_ast::Pass;
use swc_ecma_visit::visit_mut_pass;

pub use options::CodegenOptions;

// Visitor type retained for advanced users that need fine-grained error
// access via `into_result()` — hidden from documentation; prefer `codegen()`.
#[doc(hidden)]
pub use visitor::CodegenVisitor;

/// Run the codegen transform as an SWC `Pass`.
pub fn codegen(cm: Lrc<SourceMap>, options: CodegenOptions) -> impl Pass {
    visit_mut_pass(visitor::CodegenVisitor::new(cm, options))
}
