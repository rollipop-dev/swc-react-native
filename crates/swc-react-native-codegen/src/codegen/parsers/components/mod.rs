//! Shared component parsing for SWC's normalized Flow/TypeScript AST.
//!
//! Prop and event rules below follow upstream `components/componentsUtils.js`
//! and `components/events.js`; each mapping names its upstream functions.
//! `type_info` contains the local AST adapter, and `commands` follows upstream
//! `components/commands.js`. RN runtime-version fallbacks belong in the generator.

mod commands;
mod type_info;

pub use commands::extract_commands;

use std::collections::HashSet;

use anyhow::{bail, Result};
use swc_ecma_ast::*;

use crate::codegen::codegen_schema::*;
use crate::codegen::parsers::{find_interface, find_type_alias};

use self::type_info::{extract_type_info, ts_entity_name_leaf, TypeInfo};

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
/// `flattenProperties` in `{flow,typescript}/components/componentsUtils.js`
/// and extends handling in `parsers-commons.js::extendsForProp` (TypeScript)
/// / `flow/parser.js::extendsForProp`:
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
        let properties = extract_object_properties(module, &type_alias.type_ann)?;
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

        for prop_info in extract_interface_body_properties(module, &interface.body)? {
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
            type_info,
        } => {
            let optional = optional || type_info.optional;
            if let Some(event) = try_extract_event(&name, optional, &type_info) {
                if !events.iter().any(|e| e.name == event.name) {
                    events.push(event);
                }
            } else if !props.iter().any(|p| p.name == name) {
                let type_annotation = resolve_prop_type(&type_info);
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
fn extract_interface_body_properties(
    module: &Module,
    body: &TsInterfaceBody,
) -> Result<Vec<PropInfo>> {
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
            let type_info = if let Some(type_ann) = &prop.type_ann {
                extract_type_info(module, &type_ann.type_ann)?
            } else {
                TypeInfo::new("unknown")
            };
            result.push(PropInfo::Property {
                name,
                optional: prop.optional,
                type_info,
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
        type_info: TypeInfo,
    },
}

/// Unwrap nested type structures like `$ReadOnly<{| ... |}>` (legacy Flow) or
/// `Readonly<{ ... }>` (modern Flow / TS) to get to the object type.
///
/// React Native is migrating spec files from Flow's `$ReadOnly` utility type to
/// `Readonly` (see facebook/react-native#55066). Accept both spellings so
/// component schemas continue to parse across RN versions.
fn extract_object_properties(module: &Module, ty: &TsType) -> Result<Vec<PropInfo>> {
    match ty {
        // $ReadOnly<{| ... |}> / Readonly<{ ... }> → TsTypeRef with type_params
        // containing the object type
        TsType::TsTypeRef(TsTypeRef {
            type_name: TsEntityName::Ident(id),
            type_params: Some(params),
            ..
        }) if matches!(&*id.sym, "$ReadOnly" | "Readonly") => {
            if let Some(inner) = params.params.first() {
                return extract_object_properties(module, inner);
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
                                match &*type_ann.type_ann {
                                    TsType::TsTypeRef(reference) => {
                                        ts_entity_name_leaf(&reference.type_name)
                                    }
                                    _ => "unknown".to_string(),
                                }
                            } else {
                                "unknown".to_string()
                            };
                            result.push(PropInfo::Spread(spread_type_name));
                            continue;
                        }

                        let type_info = if let Some(type_ann) = &prop.type_ann {
                            extract_type_info(module, &type_ann.type_ann)?
                        } else {
                            TypeInfo::new("unknown")
                        };
                        result.push(PropInfo::Property {
                            name,
                            optional: prop.optional,
                            type_info,
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
                result.extend(extract_object_properties(module, ty)?);
            }
            Ok(result)
        }
        _ => Ok(vec![]),
    }
}

/// ViewConfig metadata from `buildEventSchema` / `findEventArgumentsAndType`
/// in `{flow,typescript}/components/events.js`. Event payload schemas are not
/// needed by GenerateViewConfigJs.js.
fn try_extract_event(name: &str, optional: bool, type_info: &TypeInfo) -> Option<EventTypeShape> {
    if type_info.is_literal {
        return None;
    }
    if matches!(type_info.name.as_str(), "Readonly" | "$ReadOnly") {
        return type_info
            .params
            .first()
            .and_then(|inner| try_extract_event(name, optional || inner.optional, inner));
    }
    let bubbling_type = match type_info.name.as_str() {
        "DirectEventHandler" => BubblingType::Direct,
        "BubblingEventHandler" => BubblingType::Bubble,
        _ => return None,
    };

    Some(EventTypeShape {
        name: name.to_string(),
        bubbling_type,
        optional,
        paper_top_level_name_deprecated: type_info
            .params
            .get(1)
            .map(|param| &param.name)
            .filter(|name| !matches!(name.as_str(), "" | "unknown"))
            .cloned(),
    })
}

/// ViewConfig-relevant subset of `getTypeAnnotation` / `getTypeAnnotationForArray`
/// in `flow/components/componentsUtils.js` and `getCommonTypeAnnotation` /
/// `getTypeAnnotation` / `detectArrayType` in its TypeScript counterpart.
fn resolve_prop_type(type_info: &TypeInfo) -> PropTypeAnnotation {
    if type_info.is_literal {
        return PropTypeAnnotation::MixedTypeAnnotation;
    }
    match type_info.name.as_str() {
        "WithDefault" | "Readonly" | "$ReadOnly" => {
            // Default and readonly wrappers preserve the inner runtime type.
            if let Some(inner) = type_info.params.first() {
                resolve_prop_type(inner)
            } else {
                PropTypeAnnotation::MixedTypeAnnotation
            }
        }
        "boolean" | "TsBooleanKeyword" => PropTypeAnnotation::BooleanTypeAnnotation,
        "string" | "TsStringKeyword" => PropTypeAnnotation::StringTypeAnnotation,
        "Int32" => PropTypeAnnotation::Int32TypeAnnotation,
        "Double" => PropTypeAnnotation::DoubleTypeAnnotation,
        "Float" => PropTypeAnnotation::FloatTypeAnnotation,
        // Both RN 0.84's `$ReadOnlyArray` and RN 0.85's `ReadonlyArray` specs
        // need the same array processors as TS's `Array<T>` / `T[]` forms.
        "Array" | "ReadonlyArray" | "$ReadOnlyArray" => PropTypeAnnotation::ArrayTypeAnnotation {
            element_type: Box::new(
                type_info
                    .params
                    .first()
                    .map(resolve_prop_type)
                    .unwrap_or(PropTypeAnnotation::MixedTypeAnnotation),
            ),
        },
        "ImageSource" => PropTypeAnnotation::ReservedPropTypeAnnotation {
            name: "ImageSourcePrimitive".to_string(),
        },
        "ColorValue" | "ProcessedColorValue" => PropTypeAnnotation::ReservedPropTypeAnnotation {
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
