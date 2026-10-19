//! SWC-specific type summary for JavaScript ViewConfig generation.
//!
//! SWC represents Flow and TypeScript types with the same `TsType` nodes. This
//! adapter retains only the names, parameters, literals and nullability needed
//! to select prop processors and event registrations. It is not a full type
//! schema or a direct port of upstream's default/union validation.
//!
//! Alias and wrapper references: `flow/utils.js::getValueFromTypes` and
//! `typescript/parseTopLevelType.js::{getValueFromTypes, parseTopLevelType}`.
//! React Native prop/event interpretation stays in the parent module; runtime
//! compatibility fallbacks stay in `generators/runtime/legacy.rs`.

use std::collections::HashSet;

use anyhow::{bail, Result};
use swc_ecma_ast::*;

use crate::codegen::parsers::{find_type_alias, str_value};

#[derive(Debug)]
pub(super) struct TypeInfo {
    pub(super) name: String,
    pub(super) params: Vec<TypeInfo>,
    pub(super) optional: bool,
    pub(super) is_literal: bool,
}

impl TypeInfo {
    pub(super) fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            params: vec![],
            optional: false,
            is_literal: false,
        }
    }

    fn literal(value: impl Into<String>) -> Self {
        Self {
            is_literal: true,
            ..Self::new(value)
        }
    }
}

/// Resolve local aliases while retaining nested wrapper/array parameters.
pub(super) fn extract_type_info(module: &Module, ty: &TsType) -> Result<TypeInfo> {
    extract_type_info_inner(module, ty, &mut HashSet::new())
}

/// Resolve a `TsEntityName` to its rightmost identifier symbol. This lets
/// qualified references like `CT.DirectEventHandler` (used after
/// `import { CodegenTypes as CT } from 'react-native'`) match the same
/// downstream lookup as a bare `DirectEventHandler` identifier.
pub(super) fn ts_entity_name_leaf(name: &TsEntityName) -> String {
    match name {
        TsEntityName::Ident(id) => id.sym.to_string(),
        TsEntityName::TsQualifiedName(qual) => qual.right.sym.to_string(),
    }
}

fn extract_type_info_inner(
    module: &Module,
    ty: &TsType,
    aliases: &mut HashSet<String>,
) -> Result<TypeInfo> {
    Ok(match ty {
        TsType::TsTypeRef(TsTypeRef {
            type_name,
            type_params,
            ..
        }) => {
            if let TsEntityName::Ident(id) = type_name {
                if let Some(alias) = find_type_alias(module, &id.sym) {
                    let name = id.sym.to_string();
                    if !aliases.insert(name.clone()) {
                        bail!("Circular type alias '{name}' in component props");
                    }
                    let result = extract_type_info_inner(module, &alias.type_ann, aliases);
                    aliases.remove(&name);
                    return result;
                }
            }
            let name = ts_entity_name_leaf(type_name);
            let params = type_params
                .as_ref()
                .map(|p| {
                    p.params
                        .iter()
                        .map(|param| extract_type_info_inner(module, param, aliases))
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?
                .unwrap_or_default();
            TypeInfo {
                name,
                params,
                optional: false,
                is_literal: false,
            }
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
            TypeInfo::new(name)
        }
        TsType::TsLitType(TsLitType {
            lit: TsLit::Bool(value),
            ..
        }) => TypeInfo::literal(value.value.to_string()),
        TsType::TsLitType(TsLitType {
            lit: TsLit::Str(value),
            ..
        }) => TypeInfo::literal(str_value(value)),
        TsType::TsOptionalType(optional) => {
            let mut info = extract_type_info_inner(module, &optional.type_ann, aliases)?;
            info.optional = true;
            info
        }
        TsType::TsArrayType(array) => TypeInfo {
            name: "Array".to_string(),
            params: vec![extract_type_info_inner(module, &array.elem_type, aliases)?],
            optional: false,
            is_literal: false,
        },
        TsType::TsTypeOperator(operator) if operator.op == TsTypeOperatorOp::ReadOnly => {
            extract_type_info_inner(module, &operator.type_ann, aliases)?
        }
        TsType::TsParenthesizedType(parenthesized) => {
            extract_type_info_inner(module, &parenthesized.type_ann, aliases)?
        }
        TsType::TsUnionOrIntersectionType(TsUnionOrIntersectionType::TsUnionType(union)) => {
            let mut optional = false;
            let mut selected = None;
            for ty in &union.types {
                let info = extract_type_info_inner(module, ty, aliases)?;
                let nullish =
                    !info.is_literal && matches!(info.name.as_str(), "null" | "void" | "undefined");
                optional |= info.optional || nullish;
                if selected.is_none() && !nullish && (info.is_literal || info.name != "unknown") {
                    selected = Some(info);
                }
            }
            let mut info = selected.unwrap_or_else(|| TypeInfo::new("unknown"));
            info.optional = optional;
            info
        }
        _ => TypeInfo::new("unknown"),
    })
}
