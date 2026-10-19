//! Shared schema assembly and declaration lookup for the Flow/TypeScript entry points.
//! Component parsing lives in `components`; NativeModule parsing stays in `native_modules`.

mod components;

pub use components::{extract_commands, extract_props_and_events};

pub mod flow;
pub mod native_modules;
pub mod typescript;

use std::collections::BTreeMap;

use anyhow::{bail, Result};
use swc_ecma_ast::*;

use crate::codegen::codegen_schema::*;

/// Helper to extract a String from Str literal (Wtf8Atom → String).
fn str_value(s: &Str) -> String {
    s.value.as_str().unwrap_or_default().to_string()
}

/// Configuration extracted from parsing a component file.
#[derive(Debug)]
pub struct ComponentConfig {
    pub component_name: String,
    pub props_type_name: String,
    pub options: OptionsShape,
    pub command_type_name: Option<String>,
}

/// Wrap a component config into a full SchemaType.
// Corresponds to `wrapComponentSchema` in parsers-commons.js
pub fn wrap_component_schema(config: ComponentBuildResult) -> SchemaType {
    let module_name = format!("{}NativeComponent", config.component_name);
    let mut components = BTreeMap::new();
    components.insert(
        config.component_name.clone(),
        ComponentShape {
            extends_props: config.extends_props,
            events: config.events,
            props: config.props,
            commands: config.commands,
            paper_component_name: config.options.paper_component_name,
            paper_component_name_deprecated: config.options.paper_component_name_deprecated,
            interface_only: config.options.interface_only,
            excluded_platforms: config.options.excluded_platforms,
        },
    );

    let mut modules = BTreeMap::new();
    modules.insert(
        module_name,
        ModuleSchema::Component(ComponentModule { components }),
    );

    SchemaType { modules }
}

/// Result of building component schema from AST.
#[derive(Debug)]
pub struct ComponentBuildResult {
    pub component_name: String,
    pub options: OptionsShape,
    pub extends_props: Vec<ExtendsPropsShape>,
    pub events: Vec<EventTypeShape>,
    pub props: Vec<NamedShape<PropTypeAnnotation>>,
    pub commands: Vec<NamedShape<CommandTypeAnnotation>>,
}

/// Find `codegenNativeComponent<PropsType>('Name', options)` in default export.
// Corresponds to `findNativeComponentType` in parsers-commons.js
pub fn find_codegen_native_component(expr: &Expr) -> Option<(&CallExpr, &Expr)> {
    match expr {
        // Direct call: codegenNativeComponent(...)
        Expr::Call(call) if is_codegen_native_component_call(call) => {
            return Some((call, expr));
        }
        // Paren + TsAs: (codegenNativeComponent(...): Type) — Flow type cast
        Expr::Paren(ParenExpr { expr: inner, .. }) => {
            return find_codegen_native_component(inner);
        }
        // TsAs: codegenNativeComponent(...) as Type
        Expr::TsAs(TsAsExpr { expr: inner, .. }) => {
            return find_codegen_native_component(inner);
        }
        // TsTypeAssertion: <Type>codegenNativeComponent(...)
        Expr::TsTypeAssertion(TsTypeAssertion { expr: inner, .. }) => {
            return find_codegen_native_component(inner);
        }
        _ => {}
    }
    None
}

fn is_codegen_native_component_call(call: &CallExpr) -> bool {
    matches!(&call.callee, Callee::Expr(expr) if matches!(&**expr, Expr::Ident(id) if &*id.sym == "codegenNativeComponent"))
}

/// Extract component name (first string arg) and options (second obj arg) from call.
pub fn extract_component_info(call: &CallExpr) -> Result<(String, OptionsShape)> {
    let component_name = call
        .args
        .first()
        .and_then(|arg| match &*arg.expr {
            Expr::Lit(Lit::Str(s)) => Some(str_value(s)),
            _ => None,
        })
        .ok_or_else(|| {
            anyhow::anyhow!("codegenNativeComponent requires a string literal as first argument")
        })?;

    let options = if let Some(arg) = call.args.get(1) {
        extract_options(&arg.expr)?
    } else {
        OptionsShape::default()
    };

    Ok((component_name, options))
}

/// Extract props type name from type arguments of the call.
pub fn extract_props_type_name(call: &CallExpr) -> Result<String> {
    let type_args = call
        .type_args
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("codegenNativeComponent requires a type parameter"))?;

    match type_args.params.first() {
        Some(ty) => match &**ty {
            TsType::TsTypeRef(TsTypeRef {
                type_name: TsEntityName::Ident(id),
                ..
            }) => Ok(id.sym.to_string()),
            _ => bail!("Expected a type reference as type parameter"),
        },
        None => bail!("Expected at least one type parameter"),
    }
}

// Corresponds to `getOptions` in parsers-commons.js
fn extract_options(expr: &Expr) -> Result<OptionsShape> {
    let obj = match expr {
        Expr::Object(obj) => obj,
        _ => return Ok(OptionsShape::default()),
    };

    let mut options = OptionsShape::default();

    for prop in &obj.props {
        let kv = match prop {
            PropOrSpread::Prop(prop) => match &**prop {
                Prop::KeyValue(kv) => kv,
                _ => continue,
            },
            _ => continue,
        };

        let key = match &kv.key {
            PropName::Ident(id) => id.sym.to_string(),
            _ => continue,
        };

        match key.as_str() {
            "interfaceOnly" => {
                if let Expr::Lit(Lit::Bool(b)) = &*kv.value {
                    options.interface_only = Some(b.value);
                }
            }
            "paperComponentName" => {
                if let Expr::Lit(Lit::Str(s)) = &*kv.value {
                    options.paper_component_name = Some(str_value(s));
                }
            }
            "paperComponentNameDeprecated" => {
                if let Expr::Lit(Lit::Str(s)) = &*kv.value {
                    options.paper_component_name_deprecated = Some(str_value(s));
                }
            }
            "excludedPlatforms" => {
                if let Expr::Array(arr) = &*kv.value {
                    let platforms: Vec<String> = arr
                        .elems
                        .iter()
                        .filter_map(|elem| {
                            elem.as_ref().and_then(|e| match &*e.expr {
                                Expr::Lit(Lit::Str(s)) => Some(str_value(s)),
                                _ => None,
                            })
                        })
                        .collect();
                    options.excluded_platforms = Some(platforms);
                }
            }
            _ => {}
        }
    }

    Ok(options)
}

/// Find type alias declaration by name in the module body.
pub fn find_type_alias<'a>(module: &'a Module, name: &str) -> Option<&'a TsTypeAliasDecl> {
    for item in &module.body {
        match item {
            // Top-level type alias: type X = ...
            ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(decl))) if &*decl.id.sym == name => {
                return Some(decl);
            }
            // Exported type alias: export type X = ...
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
                decl: Decl::TsTypeAlias(decl),
                ..
            })) if &*decl.id.sym == name => {
                return Some(decl);
            }
            _ => {}
        }
    }
    None
}

/// Find interface declaration by name in the module body.
pub fn find_interface<'a>(module: &'a Module, name: &str) -> Option<&'a TsInterfaceDecl> {
    for item in &module.body {
        match item {
            ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(decl))) if &*decl.id.sym == name => {
                return Some(decl);
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
                decl: Decl::TsInterface(decl),
                ..
            })) if &*decl.id.sym == name => {
                return Some(decl);
            }
            _ => {}
        }
    }
    None
}

/// Extract command type name from `codegenNativeCommands<NativeCommands>(...)`.
pub fn extract_command_type_name(call: &CallExpr) -> Option<String> {
    call.type_args.as_ref().and_then(|type_args| {
        type_args.params.first().and_then(|ty| match &**ty {
            TsType::TsTypeRef(TsTypeRef {
                type_name: TsEntityName::Ident(id),
                ..
            }) => Some(id.sym.to_string()),
            _ => None,
        })
    })
}
