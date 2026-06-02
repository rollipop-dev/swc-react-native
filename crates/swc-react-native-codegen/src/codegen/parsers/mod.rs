pub mod flow;
pub mod typescript;

use std::collections::{BTreeMap, HashSet};

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

/// Extract props and events from a type definition.
/// The props type is expected to be `$ReadOnly<{| ...ViewProps, prop1: Type, onEvent: EventHandler<...> |}>`
/// or its modern equivalent `Readonly<{ ...ViewProps, prop1: Type, onEvent: EventHandler<...> }>`.
// Corresponds to `getProps` in flow/parser.js
pub type PropsAndEvents = (
    Vec<ExtendsPropsShape>,
    Vec<NamedShape<PropTypeAnnotation>>,
    Vec<EventTypeShape>,
);

pub fn extract_props_and_events(module: &Module, props_type_name: &str) -> Result<PropsAndEvents> {
    let mut extends_props: Vec<ExtendsPropsShape> = Vec::new();
    let mut props: Vec<NamedShape<PropTypeAnnotation>> = Vec::new();
    let mut events: Vec<EventTypeShape> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();

    collect_props_and_events(
        module,
        props_type_name,
        &mut extends_props,
        &mut props,
        &mut events,
        &mut visited,
    )?;

    Ok((extends_props, props, events))
}

/// Recursively flatten the properties/events of `type_name`. Mirrors
/// `flattenProperties` + `extendsForProp` from
/// `@react-native/codegen/src/parsers/typescript/components/componentsUtils.js`:
/// any `extends` (or Flow-style spread) that resolves to a locally declared
/// type/interface is inlined into the property list, while built-in
/// `ViewProps` is recorded as an `ExtendsPropsShape` instead.
fn collect_props_and_events(
    module: &Module,
    type_name: &str,
    extends_props: &mut Vec<ExtendsPropsShape>,
    props: &mut Vec<NamedShape<PropTypeAnnotation>>,
    events: &mut Vec<EventTypeShape>,
    visited: &mut HashSet<String>,
) -> Result<()> {
    if !visited.insert(type_name.to_string()) {
        return Ok(());
    }

    // Type alias path (Flow `type X = $ReadOnly<{| ...Y, prop |}>` or
    // `type X = Readonly<{ ...Y, prop }>`).
    if let Some(type_alias) = find_type_alias(module, type_name) {
        let properties = extract_object_properties(&type_alias.type_ann)?;
        for prop_info in properties {
            handle_prop_info(prop_info, module, extends_props, props, events, visited)?;
        }
        return Ok(());
    }

    // Interface path (TS `interface X extends Y, Z { … }`).
    if let Some(interface) = find_interface(module, type_name) {
        for ext in &interface.extends {
            let ext_name = match &*ext.expr {
                Expr::Ident(id) => id.sym.to_string(),
                _ => continue,
            };
            handle_extends_name(&ext_name, module, extends_props, props, events, visited)?;
        }

        for prop_info in extract_interface_body_properties(&interface.body)? {
            handle_prop_info(prop_info, module, extends_props, props, events, visited)?;
        }
        return Ok(());
    }

    bail!("Type '{type_name}' not found");
}

/// Dispatch a single inherited type by name: locally declared → recurse,
/// `ViewProps` → record extends shape, anything else → silently ignore (we
/// can't resolve cross-module references and the upstream Babel plugin
/// throws here, but tolerating unknowns matches our looser stance).
fn handle_extends_name(
    name: &str,
    module: &Module,
    extends_props: &mut Vec<ExtendsPropsShape>,
    props: &mut Vec<NamedShape<PropTypeAnnotation>>,
    events: &mut Vec<EventTypeShape>,
    visited: &mut HashSet<String>,
) -> Result<()> {
    if find_interface(module, name).is_some() || find_type_alias(module, name).is_some() {
        return collect_props_and_events(module, name, extends_props, props, events, visited);
    }
    if name == "ViewProps" && !has_extends_shape(extends_props, "ReactNativeCoreViewProps") {
        extends_props.push(ExtendsPropsShape {
            type_: ExtendsPropsType::ReactNativeBuiltInType,
            known_type_name: "ReactNativeCoreViewProps".to_string(),
        });
    }
    Ok(())
}

fn handle_prop_info(
    prop_info: PropInfo,
    module: &Module,
    extends_props: &mut Vec<ExtendsPropsShape>,
    props: &mut Vec<NamedShape<PropTypeAnnotation>>,
    events: &mut Vec<EventTypeShape>,
    visited: &mut HashSet<String>,
) -> Result<()> {
    match prop_info {
        PropInfo::Spread(name) => {
            handle_extends_name(&name, module, extends_props, props, events, visited)
        }
        PropInfo::Property {
            name,
            optional,
            type_name,
            type_params,
        } => {
            if let Some(event) = try_extract_event(&name, optional, &type_name, &type_params) {
                if !events.iter().any(|e| e.name == event.name) {
                    events.push(event);
                }
            } else if !props.iter().any(|p| p.name == name) {
                let type_annotation = resolve_prop_type(&type_name, &type_params);
                props.push(NamedShape {
                    name,
                    optional,
                    type_annotation,
                });
            }
            Ok(())
        }
    }
}

fn has_extends_shape(extends_props: &[ExtendsPropsShape], known_type_name: &str) -> bool {
    extends_props
        .iter()
        .any(|e| e.known_type_name == known_type_name)
}

/// Extract properties from a TS interface body.
fn extract_interface_body_properties(body: &TsInterfaceBody) -> Result<Vec<PropInfo>> {
    let mut result = Vec::new();
    for member in &body.body {
        if let TsTypeElement::TsPropertySignature(prop) = member {
            if prop.computed {
                continue;
            }
            let name = match &*prop.key {
                Expr::Ident(id) => id.sym.to_string(),
                _ => continue,
            };
            let (type_name, type_params, type_optional) = if let Some(type_ann) = &prop.type_ann {
                extract_type_info(&type_ann.type_ann)
            } else {
                ("unknown".to_string(), vec![], false)
            };
            result.push(PropInfo::Property {
                name,
                optional: prop.optional || type_optional,
                type_name,
                type_params,
            });
        }
    }
    Ok(result)
}

#[derive(Debug)]
enum PropInfo {
    Spread(String),
    Property {
        name: String,
        optional: bool,
        type_name: String,
        type_params: Vec<String>,
    },
}

/// Unwrap nested type structures like `$ReadOnly<{| ... |}>` (legacy Flow) or
/// `Readonly<{ ... }>` (modern Flow / TS) to get to the object type.
///
/// React Native is migrating spec files from Flow's `$ReadOnly` utility type to
/// `Readonly` (see facebook/react-native#55066). Accept both spellings so
/// component schemas continue to parse across RN versions.
fn extract_object_properties(ty: &TsType) -> Result<Vec<PropInfo>> {
    match ty {
        // $ReadOnly<{| ... |}> / Readonly<{ ... }> → TsTypeRef with type_params
        // containing the object type
        TsType::TsTypeRef(TsTypeRef {
            type_name: TsEntityName::Ident(id),
            type_params: Some(params),
            ..
        }) if matches!(&*id.sym, "$ReadOnly" | "Readonly") => {
            if let Some(inner) = params.params.first() {
                return extract_object_properties(inner);
            }
            Ok(vec![])
        }
        // Object type literal {| ... |}
        TsType::TsTypeLit(TsTypeLit { members, .. }) => {
            let mut result = Vec::new();
            for member in members {
                match member {
                    // Spread: ...ViewProps
                    TsTypeElement::TsCallSignatureDecl(_) => {}
                    TsTypeElement::TsConstructSignatureDecl(_) => {}
                    TsTypeElement::TsPropertySignature(prop) => {
                        if prop.computed {
                            continue;
                        }
                        let name = match &*prop.key {
                            Expr::Ident(id) => id.sym.to_string(),
                            _ => continue,
                        };

                        // SWC represents Flow's `...ViewProps` as `__flow_spread: ViewProps`
                        if name == "__flow_spread" {
                            let spread_type_name = if let Some(type_ann) = &prop.type_ann {
                                let (tn, _, _) = extract_type_info(&type_ann.type_ann);
                                tn
                            } else {
                                "unknown".to_string()
                            };
                            result.push(PropInfo::Spread(spread_type_name));
                            continue;
                        }

                        let (type_name, type_params, type_optional) =
                            if let Some(type_ann) = &prop.type_ann {
                                extract_type_info(&type_ann.type_ann)
                            } else {
                                ("unknown".to_string(), vec![], false)
                            };
                        result.push(PropInfo::Property {
                            name,
                            optional: prop.optional || type_optional,
                            type_name,
                            type_params,
                        });
                    }
                    TsTypeElement::TsMethodSignature(_) => {}
                    TsTypeElement::TsIndexSignature(idx) => {
                        // Check if this is a spread-like pattern
                        // In SWC Flow AST, `...ViewProps` in an exact object type
                        // might be represented differently
                        let _ = idx;
                    }
                    TsTypeElement::TsGetterSignature(_) => {}
                    TsTypeElement::TsSetterSignature(_) => {}
                }
            }
            Ok(result)
        }
        // Union or intersection type
        TsType::TsUnionOrIntersectionType(union_or_intersection) => {
            let types = match union_or_intersection {
                TsUnionOrIntersectionType::TsIntersectionType(TsIntersectionType {
                    types, ..
                }) => types,
                _ => return Ok(vec![]),
            };
            let mut result = Vec::new();
            for ty in types {
                result.extend(extract_object_properties(ty)?);
            }
            Ok(result)
        }
        _ => Ok(vec![]),
    }
}

/// Resolve a `TsEntityName` to its rightmost identifier symbol. This lets
/// qualified references like `CT.DirectEventHandler` (used after
/// `import { CodegenTypes as CT } from 'react-native'`) match the same
/// downstream lookup as a bare `DirectEventHandler` identifier.
fn ts_entity_name_leaf(name: &TsEntityName) -> String {
    match name {
        TsEntityName::Ident(id) => id.sym.to_string(),
        TsEntityName::TsQualifiedName(qual) => qual.right.sym.to_string(),
    }
}

/// Extract type name, type params, and optional/nullability from a type annotation.
fn extract_type_info(ty: &TsType) -> (String, Vec<String>, bool) {
    match ty {
        TsType::TsTypeRef(TsTypeRef {
            type_name,
            type_params,
            ..
        }) => {
            let name = ts_entity_name_leaf(type_name);
            let params = type_params
                .as_ref()
                .map(|p| {
                    p.params
                        .iter()
                        .map(|param| match &**param {
                            TsType::TsTypeRef(TsTypeRef { type_name, .. }) => {
                                ts_entity_name_leaf(type_name)
                            }
                            TsType::TsKeywordType(kw) => match kw.kind {
                                TsKeywordTypeKind::TsNullKeyword => "null".to_string(),
                                TsKeywordTypeKind::TsVoidKeyword => "void".to_string(),
                                TsKeywordTypeKind::TsUndefinedKeyword => "undefined".to_string(),
                                TsKeywordTypeKind::TsBooleanKeyword => "boolean".to_string(),
                                _ => format!("{:?}", kw.kind),
                            },
                            TsType::TsLitType(TsLitType {
                                lit: TsLit::Bool(b),
                                ..
                            }) => format!("{}", b.value),
                            TsType::TsLitType(TsLitType {
                                lit: TsLit::Str(s), ..
                            }) => s.value.as_str().unwrap_or_default().to_string(),
                            _ => "unknown".to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            (name, params, false)
        }
        TsType::TsKeywordType(kw) => {
            let name = match kw.kind {
                TsKeywordTypeKind::TsBooleanKeyword => "boolean",
                TsKeywordTypeKind::TsStringKeyword => "string",
                TsKeywordTypeKind::TsNumberKeyword => "number",
                TsKeywordTypeKind::TsNullKeyword => "null",
                TsKeywordTypeKind::TsVoidKeyword => "void",
                TsKeywordTypeKind::TsUndefinedKeyword => "undefined",
                _ => "unknown",
            };
            (name.to_string(), vec![], false)
        }
        TsType::TsOptionalType(optional) => {
            let (name, params, _) = extract_type_info(&optional.type_ann);
            (name, params, true)
        }
        TsType::TsParenthesizedType(parenthesized) => extract_type_info(&parenthesized.type_ann),
        TsType::TsUnionOrIntersectionType(TsUnionOrIntersectionType::TsUnionType(union)) => {
            let mut optional = false;
            let mut selected = None;
            for ty in &union.types {
                let (name, params, nested_optional) = extract_type_info(ty);
                optional |=
                    nested_optional || matches!(name.as_str(), "null" | "void" | "undefined");
                if selected.is_none()
                    && !matches!(name.as_str(), "null" | "void" | "undefined" | "unknown")
                {
                    selected = Some((name, params));
                }
            }
            if let Some((name, params)) = selected {
                return (name, params, optional);
            }
            ("unknown".to_string(), vec![], optional)
        }
        _ => ("unknown".to_string(), vec![], false),
    }
}

/// Try to extract an event from a property.
fn try_extract_event(
    name: &str,
    optional: bool,
    type_name: &str,
    type_params: &[String],
) -> Option<EventTypeShape> {
    let bubbling_type = match type_name {
        "DirectEventHandler" => BubblingType::Direct,
        "BubblingEventHandler" => BubblingType::Bubble,
        _ => return None,
    };

    Some(EventTypeShape {
        name: name.to_string(),
        bubbling_type,
        optional,
        paper_top_level_name_deprecated: type_params
            .get(1)
            .filter(|name| !matches!(name.as_str(), "" | "unknown"))
            .cloned(),
    })
}

/// Resolve a prop type annotation from its type name.
fn resolve_prop_type(type_name: &str, type_params: &[String]) -> PropTypeAnnotation {
    match type_name {
        "WithDefault" => {
            // WithDefault<Type, default> — resolve the inner type
            if let Some(inner) = type_params.first() {
                resolve_prop_type(inner, &[])
            } else {
                PropTypeAnnotation::MixedTypeAnnotation
            }
        }
        "boolean" | "TsBooleanKeyword" => PropTypeAnnotation::BooleanTypeAnnotation,
        "string" | "TsStringKeyword" => PropTypeAnnotation::StringTypeAnnotation,
        "Int32" => PropTypeAnnotation::Int32TypeAnnotation,
        "Double" => PropTypeAnnotation::DoubleTypeAnnotation,
        "Float" => PropTypeAnnotation::FloatTypeAnnotation,
        "ImageSource" => PropTypeAnnotation::ReservedPropTypeAnnotation {
            name: "ImageSourcePrimitive".to_string(),
        },
        "ColorValue" => PropTypeAnnotation::ReservedPropTypeAnnotation {
            name: "ColorPrimitive".to_string(),
        },
        "PointValue" => PropTypeAnnotation::ReservedPropTypeAnnotation {
            name: "PointPrimitive".to_string(),
        },
        "EdgeInsetsValue" => PropTypeAnnotation::ReservedPropTypeAnnotation {
            name: "EdgeInsetsPrimitive".to_string(),
        },
        _ => PropTypeAnnotation::MixedTypeAnnotation,
    }
}

/// Extract commands from an interface declaration.
// Corresponds to `getCommands` in parsers-commons.js
pub fn extract_commands(
    module: &Module,
    command_type_name: &str,
) -> Result<Vec<NamedShape<CommandTypeAnnotation>>> {
    let interface = find_interface(module, command_type_name)
        .ok_or_else(|| anyhow::anyhow!("Interface '{command_type_name}' not found"))?;

    let mut commands = Vec::new();

    for member in &interface.body.body {
        if let TsTypeElement::TsPropertySignature(prop) = member {
            let name = match &*prop.key {
                Expr::Ident(id) => id.sym.to_string(),
                _ => continue,
            };

            // Extract function parameters (skip first param which is viewRef)
            let params = extract_command_params(prop);

            commands.push(NamedShape {
                name,
                optional: false,
                type_annotation: CommandTypeAnnotation { params },
            });
        }
    }

    Ok(commands)
}

/// Extract command parameters from a property signature's function type.
fn extract_command_params(prop: &TsPropertySignature) -> Vec<CommandParam> {
    let type_ann = match &prop.type_ann {
        Some(ann) => &ann.type_ann,
        None => return vec![],
    };

    // The type should be a function type: (viewRef: ..., x: T, y: T) => void
    let fn_type = match &**type_ann {
        TsType::TsFnOrConstructorType(TsFnOrConstructorType::TsFnType(f)) => f,
        _ => return vec![],
    };

    // Skip the first parameter (viewRef)
    fn_type
        .params
        .iter()
        .skip(1)
        .filter_map(|param| {
            let name = match param {
                TsFnParam::Ident(id) => id.id.sym.to_string(),
                _ => return None,
            };
            Some(CommandParam { name })
        })
        .collect()
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
