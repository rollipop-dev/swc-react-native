// Corresponds to `parsers/typescript/parser.js` in
// react-native/packages/react-native-codegen/src/parsers/typescript/
//
// In the upstream Babel implementation, Flow and TypeScript have completely separate
// parsers because Babel produces different AST node types for each:
//   - Flow: GenericTypeAnnotation, TypeAlias, NullableTypeAnnotation, ObjectTypeSpreadProperty
//   - TS:   TSTypeReference, TSTypeAliasDeclaration, TSUnionType(null), TSPropertySignature
//
// In SWC, both Flow and TypeScript are parsed into the same `Ts*` AST nodes
// (e.g. TsTypeRef, TsTypeAliasDecl, TsInterfaceDecl). The structural difference
// that remains is:
//   - Flow props: `type X = $ReadOnly<{| ...ViewProps, ... |}>` (legacy) or
//                 `type X = Readonly<{ ...ViewProps, ... }>` (modern) → TsTypeAliasDecl
//   - TS props:   `interface X extends ViewProps { ... }` → TsInterfaceDecl with extends
//
// The shared logic in `parsers/mod.rs` handles both patterns via
// `extract_props_and_events()` which tries type alias first, then interface.

use anyhow::Result;
use swc_ecma_ast::Module;

use crate::codegen::codegen_schema::SchemaType;
use crate::codegen::parsers::{
    extract_commands, extract_component_info, extract_props_and_events, extract_props_type_name,
    find_codegen_native_component, wrap_component_schema, ComponentBuildResult,
};

/// Build a SchemaType from a parsed TypeScript module.
/// This is the Rust equivalent of `TypeScriptParser.parseString()`.
pub fn build_schema(module: &Module, command_type_name: Option<&str>) -> Result<SchemaType> {
    // Find the codegenNativeComponent call in the module's default export
    let (call, _) = find_default_export_component(module)
        .ok_or_else(|| anyhow::anyhow!("No codegenNativeComponent found in default export"))?;

    // Extract component name and options
    let (component_name, options) = extract_component_info(call)?;

    // Extract props type name from type arguments
    let props_type_name = extract_props_type_name(call)?;

    // Extract props, extends, and events from the type definition
    // This handles both `type X = ...` and `interface X extends ViewProps { ... }`
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
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(_)) => {}
            _ => {}
        }
    }
    None
}
