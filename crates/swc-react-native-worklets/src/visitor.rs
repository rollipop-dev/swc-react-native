//! Runtime selection for the public pass. Typed visitors recurse directly.

use crate::compat::ra3;
use crate::options::Options;
use crate::worklet_pass;
use swc_common::{sync::Lrc, SourceMap};
use swc_ecma_ast::{Pass, Program};
use swc_ecma_visit::VisitMutWith;

enum RuntimePass {
    Reanimated(ra3::WorkletsVisitor),
    Worklets(worklet_pass::WorkletsVisitor),
}

#[inline]
pub(crate) fn worklets_pass(options: Options, cm: Lrc<SourceMap>) -> impl Pass {
    match options {
        Options::Reanimated(options) => {
            RuntimePass::Reanimated(ra3::WorkletsVisitor::new(options).with_source_map(cm))
        }
        Options::Worklets(options) => {
            RuntimePass::Worklets(worklet_pass::WorkletsVisitor::new(options).with_source_map(cm))
        }
    }
}

impl Pass for RuntimePass {
    #[inline]
    fn process(&mut self, program: &mut Program) {
        match self {
            Self::Reanimated(visitor) => program.visit_mut_with(visitor),
            Self::Worklets(visitor) => program.visit_mut_with(visitor),
        }
    }
}
