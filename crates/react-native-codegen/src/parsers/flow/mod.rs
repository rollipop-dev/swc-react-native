// Corresponds to `parsers/flow/parser.js` in react-native/packages/react-native-codegen/src/parsers/flow/

use anyhow::Result;
use swc_ecma_ast::Module;

use crate::codegen_schema::SchemaType;
use crate::parsers::{
    extract_commands, extract_component_info, extract_props_and_events, extract_props_type_name,
    find_codegen_native_component, wrap_component_schema, ComponentBuildResult,
};

/// Build a SchemaType from a parsed Flow module.
/// This is the Rust equivalent of `FlowParser.parseString()`.
pub fn build_schema(module: &Module, command_type_name: Option<&str>) -> Result<SchemaType> {
    // Find the codegenNativeComponent call in the module's default export
    let (call, _) = find_default_export_component(module)
        .ok_or_else(|| anyhow::anyhow!("No codegenNativeComponent found in default export"))?;

    // Extract component name and options
    let (component_name, options) = extract_component_info(call)?;

    // Extract props type name from type arguments
    let props_type_name = extract_props_type_name(call)?;

    // Extract props, extends, and events from the type definition
    let (extends_props, props, events) = extract_props_and_events(module, &props_type_name)?;

    // Extract commands if a command type name is provided
    let commands = if let Some(cmd_type_name) = command_type_name {
        extract_commands(module, cmd_type_name)?
    } else {
        vec![]
    };

    let result = ComponentBuildResult {
        component_name,
        options,
        extends_props,
        events,
        props,
        commands,
    };

    Ok(wrap_component_schema(result))
}

/// Find the codegenNativeComponent call in the module's default export.
fn find_default_export_component(
    module: &Module,
) -> Option<(&swc_ecma_ast::CallExpr, &swc_ecma_ast::Expr)> {
    use swc_ecma_ast::*;

    for item in &module.body {
        match item {
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(ExportDefaultExpr {
                expr,
                ..
            })) => {
                if let Some(result) = find_codegen_native_component(expr) {
                    return Some(result);
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(ExportDefaultDecl {
                decl,
                ..
            })) => {
                // Handle `export default class/function` — unlikely for codegen but be safe
                let _ = decl;
            }
            _ => {}
        }
    }
    None
}
