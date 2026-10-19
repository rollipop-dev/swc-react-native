//! Command metadata used by GenerateViewConfigJs.js.
//!
//! Shared SWC adaptation of upstream `{flow,typescript}/components/commands.js`.
//! The JS generator needs command and argument names, not the complete native
//! parameter schema. Function parameters are read from the original AST rather
//! than the prop/event summary in `type_info`.

use std::collections::HashSet;

use anyhow::{bail, Result};
use swc_ecma_ast::*;

use crate::codegen::codegen_schema::{CommandParam, CommandTypeAnnotation, NamedShape};
use crate::codegen::parsers::{find_interface, find_type_alias};

/// Extract commands from an interface declaration.
// Corresponds to `getCommands` / `buildCommandSchema` in
// `{flow,typescript}/components/commands.js`.
pub fn extract_commands(
    module: &Module,
    command_type_name: &str,
) -> Result<Vec<NamedShape<CommandTypeAnnotation>>> {
    let interface = find_interface(module, command_type_name)
        .ok_or_else(|| anyhow::anyhow!("Interface '{command_type_name}' not found"))?;

    let mut commands = Vec::new();

    for member in &interface.body.body {
        let (key, optional, params) = match member {
            TsTypeElement::TsPropertySignature(prop) => {
                let type_ann = prop.type_ann.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Native command requires a function type annotation")
                })?;
                (
                    &prop.key,
                    prop.optional,
                    resolve_command_params(module, &type_ann.type_ann)?,
                )
            }
            TsTypeElement::TsMethodSignature(method) => {
                (&method.key, method.optional, method.params.as_slice())
            }
            _ => continue,
        };
        let name = match &**key {
            Expr::Ident(id) => id.sym.to_string(),
            _ => continue,
        };

        commands.push(NamedShape {
            name,
            optional,
            type_annotation: CommandTypeAnnotation {
                params: extract_command_params(params),
            },
        });
    }

    Ok(commands)
}

// Mirrors `typescript/parseTopLevelType.js::parseTopLevelType` and
// `flow/utils.js::getValueFromTypes` as used by upstream component commands.
fn resolve_command_params<'a>(
    module: &'a Module,
    mut type_ann: &'a TsType,
) -> Result<&'a [TsFnParam]> {
    let mut visited = HashSet::new();
    loop {
        type_ann = match type_ann {
            TsType::TsFnOrConstructorType(TsFnOrConstructorType::TsFnType(function)) => {
                return Ok(&function.params);
            }
            TsType::TsParenthesizedType(parenthesized) => &parenthesized.type_ann,
            TsType::TsTypeRef(reference) => {
                let TsEntityName::Ident(name) = &reference.type_name else {
                    bail!("Native command requires a function type annotation");
                };
                if matches!(&*name.sym, "Readonly" | "$ReadOnly") {
                    reference
                        .type_params
                        .as_ref()
                        .and_then(|params| params.params.first())
                        .ok_or_else(|| anyhow::anyhow!("Readonly requires a type parameter"))?
                } else {
                    if !visited.insert(name.sym.clone()) {
                        bail!("Cyclic native command type alias '{}'", name.sym);
                    }
                    &find_type_alias(module, &name.sym)
                        .ok_or_else(|| {
                            anyhow::anyhow!("Native command type '{}' not found", name.sym)
                        })?
                        .type_ann
                }
            }
            TsType::TsUnionOrIntersectionType(TsUnionOrIntersectionType::TsUnionType(union)) => {
                let mut types = union.types.iter().filter(|ty| {
                    !matches!(
                        &***ty,
                        TsType::TsKeywordType(TsKeywordType {
                            kind: TsKeywordTypeKind::TsNullKeyword
                                | TsKeywordTypeKind::TsUndefinedKeyword
                                | TsKeywordTypeKind::TsVoidKeyword,
                            ..
                        })
                    )
                });
                let Some(inner) = types.next() else {
                    bail!("Native command requires a function type annotation");
                };
                if types.next().is_some() {
                    bail!("Native command requires a single function type annotation");
                }
                inner
            }
            _ => bail!("Native command requires a function type annotation"),
        };
    }
}

/// Extract command arguments, skipping the first parameter (viewRef).
fn extract_command_params(params: &[TsFnParam]) -> Vec<CommandParam> {
    params
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
