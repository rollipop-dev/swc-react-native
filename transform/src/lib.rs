use swc_core::{
    common::collections::AHashMap,
    ecma::{
        ast::Pass,
        visit::{visit_mut_pass, VisitMut},
    },
};
use transformer::ReactNativeCodegenTransformer;

pub fn transform_codegen(filename: String) -> impl VisitMut + Pass {
    visit_mut_pass(ReactNativeCodegenTransformer::new(filename))
}

mod transformer;
