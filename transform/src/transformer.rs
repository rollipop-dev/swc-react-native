use swc_core::ecma::{
    ast::*,
    visit::{noop_visit_mut_type, VisitMut, VisitMutWith},
};

pub struct ReactNativeCodegenTransformer {
    filename: String,
}

impl ReactNativeCodegenTransformer {
    pub fn new(filename: String) -> Self {
        Self { filename }
    }
}

impl VisitMut for ReactNativeCodegenTransformer {
    noop_visit_mut_type!();

    fn visit_mut_script(&mut self, script: &mut Script) {
        script.visit_mut_children_with(self);
    }

    fn visit_mut_module(&mut self, module: &mut Module) {
        module.visit_mut_children_with(self);
    }
}
