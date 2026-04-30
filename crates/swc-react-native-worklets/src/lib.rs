// Port of react-native-reanimated/packages/react-native-worklets/plugin/.
// Each module references the upstream `plugin/src/*.ts` file(s) it covers.

mod closure;
mod factory;
mod gestures;
mod globals;
mod hash;
mod hooks;
mod inline_style;
mod options;
mod visitor;

use swc_common::{sync::Lrc, SourceMap};
use swc_ecma_ast::Pass;
use swc_ecma_visit::visit_mut_pass;

#[doc(hidden)]
pub use visitor::WorkletsVisitor;

pub use options::WorkletsOptions;

pub fn worklets(cm: Lrc<SourceMap>, options: WorkletsOptions) -> impl Pass {
    visit_mut_pass(visitor::WorkletsVisitor::new(options).with_source_map(cm))
}
