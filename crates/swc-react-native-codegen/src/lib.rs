// Corresponds to `index.js` in react-native/packages/babel-plugin-codegen/

mod codegen;
mod options;
mod visitor;

use swc_common::{sync::Lrc, SourceMap};
use swc_ecma_ast::Pass;
use swc_ecma_visit::visit_mut_pass;

#[doc(hidden)]
pub use visitor::CodegenVisitor;

pub use options::CodegenOptions;

pub fn codegen(cm: Lrc<SourceMap>, options: CodegenOptions) -> impl Pass {
    visit_mut_pass(visitor::CodegenVisitor::new(cm, options))
}
