// Corresponds to `index.js` in react-native/packages/babel-plugin-codegen/

mod codegen;
mod options;
mod visitor;

use swc_common::{sync::Lrc, SourceMap};
use swc_ecma_ast::Pass;
use swc_ecma_visit::visit_mut_pass;

#[doc(hidden)]
pub use visitor::CodegenVisitor;

#[doc(hidden)]
pub use codegen::codegen_schema;

pub use options::CodegenOptions;

pub fn codegen(cm: Lrc<SourceMap>, options: CodegenOptions) -> impl Pass {
    visit_mut_pass(visitor::CodegenVisitor::new(cm, options))
}

#[doc(hidden)]
pub fn parse_codegen_schema(
    filename: &str,
    module: &swc_ecma_ast::Module,
) -> anyhow::Result<codegen_schema::SchemaType> {
    codegen::parse_file(filename, module, None)
}
