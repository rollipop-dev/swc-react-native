use react_native_plugin_codegen::transform_codegen;
use swc_core::{
    ecma::{ast::Program, visit::VisitMutWith},
    plugin::{
        metadata::TransformPluginMetadataContextKind, plugin_transform,
        proxies::TransformPluginProgramMetadata,
    },
};

#[plugin_transform]
pub fn codegen_plugin(mut program: Program, metadata: TransformPluginProgramMetadata) -> Program {
    program.visit_mut_with(&mut transform_codegen(
        metadata
            .get_context(&TransformPluginMetadataContextKind::Filename)
            .expect("filename is required"),
    ));

    program
}
