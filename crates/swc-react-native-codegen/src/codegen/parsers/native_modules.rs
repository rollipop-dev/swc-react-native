use std::collections::{BTreeMap, HashSet};

use anyhow::{bail, Result};
use swc_ecma_ast::*;

use crate::codegen::codegen_schema::*;
use crate::codegen::parsers::{find_interface, find_type_alias};

pub fn build_schema(module: &Module, filename: &str) -> Result<Option<SchemaType>> {
    let Some(module_spec) = find_module_spec(module)? else {
        return Ok(None);
    };

    let haste_module_name = extract_native_module_name(filename);
    let module_name = parse_module_name(module)?;
    let excluded_platforms = verify_platforms(&haste_module_name, &module_name);

    let mut ctx = NativeModuleParser::new(module);
    let mut methods = Vec::new();
    let mut event_emitters = Vec::new();

    for member in &module_spec.body.body {
        match ctx.parse_spec_member(member)? {
            Some(SpecMember::Method(method)) => methods.push(method),
            Some(SpecMember::EventEmitter(event_emitter)) => event_emitters.push(event_emitter),
            None => {}
        }
    }

    let schema = NativeModuleSchema {
        alias_map: ctx.alias_map,
        enum_map: ctx.enum_map,
        spec: NativeModuleSpec {
            event_emitters,
            methods,
        },
        module_name,
        excluded_platforms,
    };

    let mut modules = BTreeMap::new();
    modules.insert(haste_module_name, ModuleSchema::NativeModule(schema));
    Ok(Some(SchemaType { modules }))
}

enum SpecMember {
    Method(NativeModulePropertyShape),
    EventEmitter(NativeModuleEventEmitterShape),
}

struct NativeModuleParser<'a> {
    module: &'a Module,
    alias_map: BTreeMap<String, NativeModuleTypeAnnotation>,
    enum_map: BTreeMap<String, NativeModuleEnumDeclarationWithMembers>,
    resolving_aliases: HashSet<String>,
}

impl<'a> NativeModuleParser<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            alias_map: BTreeMap::new(),
            enum_map: BTreeMap::new(),
            resolving_aliases: HashSet::new(),
        }
    }

    fn parse_spec_member(&mut self, member: &TsTypeElement) -> Result<Option<SpecMember>> {
        match member {
            TsTypeElement::TsPropertySignature(prop) => {
                let name = property_key_name(&prop.key)
                    .ok_or_else(|| anyhow::anyhow!("NativeModule property must be named"))?;
                let type_ann = prop.type_ann.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("NativeModule property '{name}' needs a type")
                })?;
                let ty = &type_ann.type_ann;

                if let Some(event_type) = self.parse_event_emitter_type(ty)? {
                    return Ok(Some(SpecMember::EventEmitter(NamedShape {
                        name,
                        optional: prop.optional,
                        type_annotation: NativeModuleTypeAnnotation::EventEmitterTypeAnnotation {
                            type_annotation: Box::new(event_type),
                        },
                    })));
                }

                let type_annotation = self.translate_type(ty)?;
                if !matches!(
                    strip_nullable(&type_annotation),
                    NativeModuleTypeAnnotation::FunctionTypeAnnotation { .. }
                ) {
                    bail!("NativeModule property '{name}' must be a function or EventEmitter type");
                }

                Ok(Some(SpecMember::Method(NamedShape {
                    name,
                    optional: prop.optional,
                    type_annotation,
                })))
            }
            TsTypeElement::TsMethodSignature(method) => {
                let name = property_key_name(&method.key)
                    .ok_or_else(|| anyhow::anyhow!("NativeModule method must be named"))?;
                let type_annotation = self.translate_method_signature(method)?;
                Ok(Some(SpecMember::Method(NamedShape {
                    name,
                    optional: method.optional,
                    type_annotation,
                })))
            }
            _ => Ok(None),
        }
    }

    fn parse_event_emitter_type(
        &mut self,
        ty: &TsType,
    ) -> Result<Option<NativeModuleTypeAnnotation>> {
        let TsType::TsTypeRef(type_ref) = unwrap_readonly_type(ty) else {
            return Ok(None);
        };

        if type_name_leaf(&type_ref.type_name) != "EventEmitter" {
            return Ok(None);
        }

        let event_type = one_type_param(type_ref, "EventEmitter")?;
        Ok(Some(self.translate_type(event_type)?))
    }

    fn translate_method_signature(
        &mut self,
        method: &TsMethodSignature,
    ) -> Result<NativeModuleTypeAnnotation> {
        let return_type = if let Some(type_ann) = &method.type_ann {
            self.translate_type(&type_ann.type_ann)?
        } else {
            NativeModuleTypeAnnotation::VoidTypeAnnotation
        };
        self.validate_function_return_type(&return_type)?;

        let params = self.translate_params(&method.params)?;
        Ok(NativeModuleTypeAnnotation::FunctionTypeAnnotation {
            params,
            return_type_annotation: Box::new(return_type),
        })
    }

    fn translate_type(&mut self, ty: &TsType) -> Result<NativeModuleTypeAnnotation> {
        let (annotation, nullable) = self.translate_type_inner(ty)?;
        Ok(wrap_nullable(nullable, annotation))
    }

    fn translate_type_inner(&mut self, ty: &TsType) -> Result<(NativeModuleTypeAnnotation, bool)> {
        match ty {
            TsType::TsKeywordType(keyword) => Ok((
                match keyword.kind {
                    TsKeywordTypeKind::TsBooleanKeyword => {
                        NativeModuleTypeAnnotation::BooleanTypeAnnotation
                    }
                    TsKeywordTypeKind::TsStringKeyword => {
                        NativeModuleTypeAnnotation::StringTypeAnnotation
                    }
                    TsKeywordTypeKind::TsNumberKeyword => {
                        NativeModuleTypeAnnotation::NumberTypeAnnotation
                    }
                    TsKeywordTypeKind::TsVoidKeyword => {
                        NativeModuleTypeAnnotation::VoidTypeAnnotation
                    }
                    TsKeywordTypeKind::TsAnyKeyword
                    | TsKeywordTypeKind::TsUnknownKeyword
                    | TsKeywordTypeKind::TsObjectKeyword => {
                        NativeModuleTypeAnnotation::GenericObjectTypeAnnotation {
                            dictionary_value_type: None,
                        }
                    }
                    TsKeywordTypeKind::TsNullKeyword | TsKeywordTypeKind::TsUndefinedKeyword => {
                        NativeModuleTypeAnnotation::VoidTypeAnnotation
                    }
                    _ => bail!("Unsupported NativeModule keyword type {:?}", keyword.kind),
                },
                matches!(
                    keyword.kind,
                    TsKeywordTypeKind::TsNullKeyword | TsKeywordTypeKind::TsUndefinedKeyword
                ),
            )),
            TsType::TsLitType(lit) => Ok((translate_literal_type(lit)?, false)),
            TsType::TsFnOrConstructorType(TsFnOrConstructorType::TsFnType(fn_type)) => {
                Ok((self.translate_function_type(fn_type)?, false))
            }
            TsType::TsFnOrConstructorType(TsFnOrConstructorType::TsConstructorType(_)) => {
                bail!("Construct signatures are not supported in NativeModule specs")
            }
            TsType::TsTypeRef(type_ref) => self.translate_type_ref(type_ref),
            TsType::TsTypeLit(type_lit) => Ok((self.translate_type_lit(type_lit, None)?, false)),
            TsType::TsArrayType(array_type) => {
                Ok((self.translate_array_type(&array_type.elem_type)?, false))
            }
            TsType::TsTupleType(_) => Ok((NativeModuleTypeAnnotation::AnyTypeAnnotation, false)),
            TsType::TsOptionalType(optional) => {
                let (annotation, _) = self.translate_type_inner(&optional.type_ann)?;
                Ok((annotation, true))
            }
            TsType::TsParenthesizedType(parenthesized) => {
                self.translate_type_inner(&parenthesized.type_ann)
            }
            TsType::TsTypeOperator(type_operator)
                if type_operator.op == TsTypeOperatorOp::ReadOnly =>
            {
                self.translate_type_inner(&type_operator.type_ann)
            }
            TsType::TsUnionOrIntersectionType(TsUnionOrIntersectionType::TsUnionType(union)) => {
                self.translate_union_type(union)
            }
            TsType::TsUnionOrIntersectionType(TsUnionOrIntersectionType::TsIntersectionType(
                intersection,
            )) => Ok((self.translate_intersection_type(intersection)?, false)),
            _ => bail!("Unsupported NativeModule type annotation"),
        }
    }

    fn translate_type_ref(
        &mut self,
        type_ref: &TsTypeRef,
    ) -> Result<(NativeModuleTypeAnnotation, bool)> {
        let name = type_name_leaf(&type_ref.type_name);
        match name.as_str() {
            "Promise" => {
                let element_type = one_type_param(type_ref, "Promise")?;
                Ok((
                    NativeModuleTypeAnnotation::PromiseTypeAnnotation {
                        element_type: Box::new(self.translate_type(element_type)?),
                    },
                    false,
                ))
            }
            "Array" | "ReadonlyArray" | "$ReadOnlyArray" => {
                let element_type = one_type_param(type_ref, &name)?;
                Ok((self.translate_array_type(element_type)?, false))
            }
            "Readonly" | "$ReadOnly" => {
                let inner_type = one_type_param(type_ref, &name)?;
                self.translate_type_inner(inner_type)
            }
            "Partial" | "$Partial" => {
                let inner_type = one_type_param(type_ref, &name)?;
                Ok((self.translate_partial_type(inner_type)?, false))
            }
            "RootTag" => Ok((
                NativeModuleTypeAnnotation::ReservedTypeAnnotation {
                    name: "RootTag".to_string(),
                },
                false,
            )),
            "Int32" => Ok((NativeModuleTypeAnnotation::Int32TypeAnnotation, false)),
            "Double" => Ok((NativeModuleTypeAnnotation::DoubleTypeAnnotation, false)),
            "Float" => Ok((NativeModuleTypeAnnotation::FloatTypeAnnotation, false)),
            "Stringish" => Ok((NativeModuleTypeAnnotation::StringTypeAnnotation, false)),
            "ArrayBuffer" => Ok((NativeModuleTypeAnnotation::ArrayBufferTypeAnnotation, false)),
            "Object" | "UnsafeObject" => Ok((
                NativeModuleTypeAnnotation::GenericObjectTypeAnnotation {
                    dictionary_value_type: None,
                },
                false,
            )),
            "unknown" | "UnsafeMixed" => {
                Ok((NativeModuleTypeAnnotation::MixedTypeAnnotation, false))
            }
            _ => self.resolve_named_type(&name),
        }
    }

    fn resolve_named_type(&mut self, name: &str) -> Result<(NativeModuleTypeAnnotation, bool)> {
        if let Some(enum_decl) = find_enum(self.module, name).cloned() {
            let member_type = self.register_enum(name, &enum_decl)?;
            return Ok((
                NativeModuleTypeAnnotation::EnumDeclaration {
                    name: name.to_string(),
                    member_type,
                },
                false,
            ));
        }

        if let Some(interface) = find_interface(self.module, name).cloned() {
            let object_annotation = self.translate_interface_object(name, &interface)?;
            self.alias_map.insert(name.to_string(), object_annotation);
            return Ok((
                NativeModuleTypeAnnotation::TypeAliasTypeAnnotation {
                    name: name.to_string(),
                },
                false,
            ));
        }

        if let Some(type_alias) = find_type_alias(self.module, name).cloned() {
            return self.resolve_type_alias(name, &type_alias);
        }

        bail!("Unsupported NativeModule type reference '{name}'")
    }

    fn resolve_type_alias(
        &mut self,
        name: &str,
        type_alias: &TsTypeAliasDecl,
    ) -> Result<(NativeModuleTypeAnnotation, bool)> {
        if self.resolving_aliases.contains(name) {
            return Ok((
                NativeModuleTypeAnnotation::TypeAliasTypeAnnotation {
                    name: name.to_string(),
                },
                false,
            ));
        }

        self.resolving_aliases.insert(name.to_string());
        let resolved = self.translate_type(&type_alias.type_ann);
        self.resolving_aliases.remove(name);

        let annotation = resolved?;
        match strip_nullable(&annotation) {
            NativeModuleTypeAnnotation::ObjectTypeAnnotation { .. } => {
                self.alias_map.insert(name.to_string(), annotation);
                Ok((
                    NativeModuleTypeAnnotation::TypeAliasTypeAnnotation {
                        name: name.to_string(),
                    },
                    false,
                ))
            }
            _ => Ok((annotation, false)),
        }
    }

    fn translate_interface_object(
        &mut self,
        _name: &str,
        interface: &TsInterfaceDecl,
    ) -> Result<NativeModuleTypeAnnotation> {
        let mut properties = Vec::new();
        let mut base_types = Vec::new();

        for ext in &interface.extends {
            let ext_name = match &*ext.expr {
                Expr::Ident(id) => id.sym.to_string(),
                _ => continue,
            };
            if ext_name == "TurboModule" {
                continue;
            }

            base_types.push(ext_name.clone());
            if let Some(base_interface) = find_interface(self.module, &ext_name).cloned() {
                let base_object = self.translate_interface_object(&ext_name, &base_interface)?;
                self.alias_map.insert(ext_name, base_object);
            } else if let Some(base_alias) = find_type_alias(self.module, &ext_name).cloned() {
                let _ = self.resolve_type_alias(&ext_name, &base_alias)?;
            }
        }

        for member in &interface.body.body {
            if let Some(property) = self.translate_object_member(member)? {
                properties.push(property);
            }
        }

        Ok(NativeModuleTypeAnnotation::ObjectTypeAnnotation {
            properties,
            base_types: if base_types.is_empty() {
                None
            } else {
                Some(base_types)
            },
        })
    }

    fn translate_type_lit(
        &mut self,
        type_lit: &TsTypeLit,
        base_types: Option<Vec<String>>,
    ) -> Result<NativeModuleTypeAnnotation> {
        let mut properties = Vec::new();
        let mut index_signatures = Vec::new();

        for member in &type_lit.members {
            match member {
                TsTypeElement::TsIndexSignature(index) => index_signatures.push(index),
                _ => {
                    if let Some(property) = self.translate_object_member(member)? {
                        properties.push(property);
                    }
                }
            }
        }

        if !index_signatures.is_empty() && !properties.is_empty() {
            bail!("ObjectTypeAnnotation cannot contain both an indexer and properties");
        }

        if let Some(index) = index_signatures.first() {
            let value_type = if let Some(type_ann) = &index.type_ann {
                Some(Box::new(self.translate_type(&type_ann.type_ann)?))
            } else {
                None
            };
            return Ok(NativeModuleTypeAnnotation::GenericObjectTypeAnnotation {
                dictionary_value_type: value_type,
            });
        }

        Ok(NativeModuleTypeAnnotation::ObjectTypeAnnotation {
            properties,
            base_types,
        })
    }

    fn translate_object_member(
        &mut self,
        member: &TsTypeElement,
    ) -> Result<Option<NamedShape<NativeModuleTypeAnnotation>>> {
        let TsTypeElement::TsPropertySignature(prop) = member else {
            return Ok(None);
        };
        if prop.computed {
            return Ok(None);
        }

        let name = property_key_name(&prop.key)
            .ok_or_else(|| anyhow::anyhow!("NativeModule object property must be named"))?;
        let type_ann = prop
            .type_ann
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NativeModule object property '{name}' needs a type"))?;
        let type_annotation = self.translate_type(&type_ann.type_ann)?;

        match strip_nullable(&type_annotation) {
            NativeModuleTypeAnnotation::FunctionTypeAnnotation { .. }
            | NativeModuleTypeAnnotation::PromiseTypeAnnotation { .. }
            | NativeModuleTypeAnnotation::VoidTypeAnnotation
            | NativeModuleTypeAnnotation::ArrayBufferTypeAnnotation => {
                bail!(
                    "Object property '{name}' cannot have type '{:?}'",
                    strip_nullable(&type_annotation)
                );
            }
            _ => {}
        }

        Ok(Some(NamedShape {
            name,
            optional: prop.optional,
            type_annotation,
        }))
    }

    fn translate_function_type(
        &mut self,
        fn_type: &TsFnType,
    ) -> Result<NativeModuleTypeAnnotation> {
        let params = self.translate_params(&fn_type.params)?;
        let return_type = self.translate_type(&fn_type.type_ann.type_ann)?;
        self.validate_function_return_type(&return_type)?;

        Ok(NativeModuleTypeAnnotation::FunctionTypeAnnotation {
            params,
            return_type_annotation: Box::new(return_type),
        })
    }

    fn translate_params(
        &mut self,
        params: &[TsFnParam],
    ) -> Result<Vec<NamedShape<NativeModuleTypeAnnotation>>> {
        let mut result = Vec::new();
        for param in params {
            let TsFnParam::Ident(ident) = param else {
                bail!("All NativeModule function parameters must be named");
            };
            let type_ann = ident.type_ann.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "NativeModule function parameter '{}' needs a type",
                    ident.id.sym
                )
            })?;
            let type_annotation = self.translate_type(&type_ann.type_ann)?;
            self.validate_function_param_type(&ident.id.sym, &type_annotation)?;
            result.push(NamedShape {
                name: ident.id.sym.to_string(),
                optional: false,
                type_annotation,
            });
        }
        Ok(result)
    }

    fn translate_array_type(
        &mut self,
        element_type: &TsType,
    ) -> Result<NativeModuleTypeAnnotation> {
        let element_type = match self.translate_type(element_type) {
            Ok(annotation) if is_supported_array_element(&annotation) => annotation,
            _ => NativeModuleTypeAnnotation::AnyTypeAnnotation,
        };
        Ok(NativeModuleTypeAnnotation::ArrayTypeAnnotation {
            element_type: Box::new(element_type),
        })
    }

    fn translate_union_type(
        &mut self,
        union: &TsUnionType,
    ) -> Result<(NativeModuleTypeAnnotation, bool)> {
        let mut nullable = false;
        let mut types = Vec::new();

        for ty in &union.types {
            if is_nullish_type(ty) {
                nullable = true;
                continue;
            }

            let annotation = self.translate_type(ty)?;
            let (annotation, nested_nullable) = take_nullable(annotation);
            nullable |= nested_nullable;
            types.push(annotation);
        }

        match types.len() {
            0 => Ok((NativeModuleTypeAnnotation::VoidTypeAnnotation, nullable)),
            1 => Ok((types.remove(0), nullable)),
            _ => Ok((
                NativeModuleTypeAnnotation::UnionTypeAnnotation { types },
                nullable,
            )),
        }
    }

    fn translate_intersection_type(
        &mut self,
        intersection: &TsIntersectionType,
    ) -> Result<NativeModuleTypeAnnotation> {
        let mut properties = Vec::new();
        for ty in &intersection.types {
            let annotation = self.translate_type(ty)?;
            match take_nullable(annotation).0 {
                NativeModuleTypeAnnotation::ObjectTypeAnnotation {
                    properties: mut object_properties,
                    ..
                } => properties.append(&mut object_properties),
                _ => bail!("NativeModule intersection types must contain object types"),
            }
        }

        Ok(NativeModuleTypeAnnotation::ObjectTypeAnnotation {
            properties,
            base_types: None,
        })
    }

    fn translate_partial_type(&mut self, ty: &TsType) -> Result<NativeModuleTypeAnnotation> {
        let annotation = self.translate_type(ty)?;
        let object = match strip_nullable(&annotation) {
            NativeModuleTypeAnnotation::TypeAliasTypeAnnotation { name } => {
                self.alias_map.get(name).cloned()
            }
            NativeModuleTypeAnnotation::ObjectTypeAnnotation { .. } => Some(annotation),
            _ => None,
        };

        let Some(NativeModuleTypeAnnotation::ObjectTypeAnnotation {
            mut properties,
            base_types,
        }) = object
        else {
            bail!("Partial<T> must annotate an object type");
        };

        for property in &mut properties {
            property.optional = true;
        }

        Ok(NativeModuleTypeAnnotation::ObjectTypeAnnotation {
            properties,
            base_types,
        })
    }

    fn register_enum(
        &mut self,
        name: &str,
        enum_decl: &TsEnumDecl,
    ) -> Result<NativeModuleEnumMemberType> {
        if let Some(existing) = self.enum_map.get(name) {
            return Ok(existing.member_type.clone());
        }

        if enum_decl.members.is_empty() {
            bail!("Enums should have at least one member");
        }

        let member_type = enum_decl
            .members
            .iter()
            .find_map(|member| match &member.init {
                Some(expr) => enum_expr_member_type(expr),
                None => Some(NativeModuleEnumMemberType::StringTypeAnnotation),
            })
            .ok_or_else(|| anyhow::anyhow!("Unsupported NativeModule enum member initializer"))?;

        let mut members = Vec::new();
        for member in &enum_decl.members {
            let member_name = enum_member_name(&member.id);
            let value = match (&member_type, &member.init) {
                (NativeModuleEnumMemberType::StringTypeAnnotation, Some(expr)) => {
                    NativeModuleEnumMemberValue::StringLiteralTypeAnnotation {
                        value: string_literal_expr_value(expr).ok_or_else(|| {
                            anyhow::anyhow!("String enum values must be string literals")
                        })?,
                    }
                }
                (NativeModuleEnumMemberType::StringTypeAnnotation, None) => {
                    NativeModuleEnumMemberValue::StringLiteralTypeAnnotation {
                        value: member_name.clone(),
                    }
                }
                (NativeModuleEnumMemberType::NumberTypeAnnotation, Some(expr)) => {
                    NativeModuleEnumMemberValue::NumberLiteralTypeAnnotation {
                        value: number_literal_expr_value(expr).ok_or_else(|| {
                            anyhow::anyhow!("Number enum values must be number literals")
                        })?,
                    }
                }
                (NativeModuleEnumMemberType::NumberTypeAnnotation, None) => {
                    bail!("Number enum members must have explicit values")
                }
            };

            members.push(NativeModuleEnumMember {
                name: member_name,
                value,
            });
        }

        self.enum_map.insert(
            name.to_string(),
            NativeModuleEnumDeclarationWithMembers {
                name: name.to_string(),
                member_type: member_type.clone(),
                members,
            },
        );

        Ok(member_type)
    }

    fn validate_function_param_type(
        &self,
        name: &str,
        annotation: &NativeModuleTypeAnnotation,
    ) -> Result<()> {
        match strip_nullable(annotation) {
            NativeModuleTypeAnnotation::VoidTypeAnnotation
            | NativeModuleTypeAnnotation::PromiseTypeAnnotation { .. }
            | NativeModuleTypeAnnotation::EventEmitterTypeAnnotation { .. } => {
                bail!(
                    "NativeModule function parameter '{name}' has unsupported type '{:?}'",
                    strip_nullable(annotation)
                )
            }
            _ => Ok(()),
        }
    }

    fn validate_function_return_type(&self, annotation: &NativeModuleTypeAnnotation) -> Result<()> {
        match strip_nullable(annotation) {
            NativeModuleTypeAnnotation::FunctionTypeAnnotation { .. }
            | NativeModuleTypeAnnotation::EventEmitterTypeAnnotation { .. } => {
                bail!(
                    "NativeModule function return has unsupported type '{:?}'",
                    strip_nullable(annotation)
                )
            }
            _ => Ok(()),
        }
    }
}

fn find_module_spec(module: &Module) -> Result<Option<TsInterfaceDecl>> {
    let module_specs = module
        .body
        .iter()
        .filter_map(module_item_interface)
        .filter(|interface| interface_extends_turbo_module(interface))
        .cloned()
        .collect::<Vec<_>>();

    match module_specs.len() {
        0 => Ok(None),
        1 => {
            let spec = module_specs.into_iter().next().expect("module spec exists");
            if &*spec.id.sym != "Spec" {
                bail!("NativeModule interface extending TurboModule must be named 'Spec'");
            }
            Ok(Some(spec))
        }
        _ => bail!("Every NativeModule spec file must declare exactly one TurboModule interface"),
    }
}

fn module_item_interface(item: &ModuleItem) -> Option<&TsInterfaceDecl> {
    match item {
        ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(interface))) => Some(interface),
        ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
            decl: Decl::TsInterface(interface),
            ..
        })) => Some(interface),
        _ => None,
    }
}

fn interface_extends_turbo_module(interface: &TsInterfaceDecl) -> bool {
    interface.extends.len() == 1
        && matches!(
            &*interface.extends[0].expr,
            Expr::Ident(id) if &*id.sym == "TurboModule"
        )
}

fn parse_module_name(module: &Module) -> Result<String> {
    let mut calls = Vec::new();

    for item in &module.body {
        if let ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(default_export)) = item {
            collect_module_registry_calls(&default_export.expr, &mut calls);
        }
    }

    match calls.len() {
        0 => bail!("No TurboModuleRegistry.get/getEnforcing call found"),
        1 => parse_module_registry_call(calls[0]),
        _ => bail!("NativeModule spec must contain exactly one TurboModuleRegistry call"),
    }
}

fn collect_module_registry_calls<'a>(expr: &'a Expr, calls: &mut Vec<&'a CallExpr>) {
    match expr {
        Expr::Call(call) if is_module_registry_call(call) => calls.push(call),
        Expr::Call(call) => {
            if let Callee::Expr(callee) = &call.callee {
                collect_module_registry_calls(callee, calls);
            }
            for arg in &call.args {
                collect_module_registry_calls(&arg.expr, calls);
            }
        }
        Expr::Paren(paren) => collect_module_registry_calls(&paren.expr, calls),
        Expr::TsAs(as_expr) => collect_module_registry_calls(&as_expr.expr, calls),
        Expr::TsTypeAssertion(type_assertion) => {
            collect_module_registry_calls(&type_assertion.expr, calls);
        }
        Expr::Seq(seq) => {
            for expr in &seq.exprs {
                collect_module_registry_calls(expr, calls);
            }
        }
        _ => {}
    }
}

fn is_module_registry_call(call: &CallExpr) -> bool {
    let Callee::Expr(callee) = &call.callee else {
        return false;
    };
    let Expr::Member(member) = &**callee else {
        return false;
    };
    matches!(&*member.obj, Expr::Ident(id) if &*id.sym == "TurboModuleRegistry")
        && matches!(
            &member.prop,
            MemberProp::Ident(prop)
                if &*prop.sym == "get" || &*prop.sym == "getEnforcing"
        )
}

fn parse_module_registry_call(call: &CallExpr) -> Result<String> {
    let Some(type_args) = &call.type_args else {
        bail!("TurboModuleRegistry call must be typed with <Spec>");
    };
    match type_args.params.first().map(|param| &**param) {
        Some(TsType::TsTypeRef(TsTypeRef {
            type_name: TsEntityName::Ident(id),
            ..
        })) if &*id.sym == "Spec" => {}
        _ => bail!("TurboModuleRegistry call must use <Spec>"),
    }

    let module_name = call
        .args
        .first()
        .and_then(|arg| match &*arg.expr {
            Expr::Lit(Lit::Str(s)) => Some(str_value(s)),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("TurboModuleRegistry call needs a string module name"))?;

    Ok(module_name)
}

fn extract_native_module_name(filename: &str) -> String {
    let basename = std::path::Path::new(filename)
        .file_name()
        .map(|file_name| file_name.to_string_lossy())
        .unwrap_or_default();

    basename.split('.').next().unwrap_or_default().to_string()
}

fn verify_platforms(haste_module_name: &str, module_name: &str) -> Option<Vec<String>> {
    let mut excluded = Vec::new();

    for name in [module_name, haste_module_name] {
        if name.ends_with("Android") {
            push_unique(&mut excluded, "iOS");
        } else if name.ends_with("IOS") {
            push_unique(&mut excluded, "android");
        } else if name.ends_with("Windows") || name.ends_with("Cxx") {
            push_unique(&mut excluded, "iOS");
            push_unique(&mut excluded, "android");
        }
    }

    if excluded.is_empty() {
        None
    } else {
        Some(excluded)
    }
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn find_enum<'a>(module: &'a Module, name: &str) -> Option<&'a TsEnumDecl> {
    for item in &module.body {
        match item {
            ModuleItem::Stmt(Stmt::Decl(Decl::TsEnum(decl))) if &*decl.id.sym == name => {
                return Some(decl);
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
                decl: Decl::TsEnum(decl),
                ..
            })) if &*decl.id.sym == name => {
                return Some(decl);
            }
            _ => {}
        }
    }
    None
}

fn property_key_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(id) => Some(id.sym.to_string()),
        Expr::Lit(Lit::Str(s)) => Some(str_value(s)),
        _ => None,
    }
}

fn str_value(s: &Str) -> String {
    s.value.as_str().unwrap_or_default().to_string()
}

fn type_name_leaf(type_name: &TsEntityName) -> String {
    match type_name {
        TsEntityName::Ident(id) => id.sym.to_string(),
        TsEntityName::TsQualifiedName(qualified) => qualified.right.sym.to_string(),
    }
}

fn one_type_param<'a>(type_ref: &'a TsTypeRef, name: &str) -> Result<&'a TsType> {
    let params = type_ref
        .type_params
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Generic '{name}' must have type parameters"))?;
    if params.params.len() != 1 {
        bail!("Generic '{name}' must have exactly one type parameter");
    }
    Ok(&params.params[0])
}

fn unwrap_readonly_type(ty: &TsType) -> &TsType {
    match ty {
        TsType::TsTypeOperator(type_operator) if type_operator.op == TsTypeOperatorOp::ReadOnly => {
            unwrap_readonly_type(&type_operator.type_ann)
        }
        TsType::TsParenthesizedType(parenthesized) => unwrap_readonly_type(&parenthesized.type_ann),
        _ => ty,
    }
}

fn wrap_nullable(
    nullable: bool,
    annotation: NativeModuleTypeAnnotation,
) -> NativeModuleTypeAnnotation {
    if nullable {
        NativeModuleTypeAnnotation::NullableTypeAnnotation {
            type_annotation: Box::new(annotation),
        }
    } else {
        annotation
    }
}

fn take_nullable(annotation: NativeModuleTypeAnnotation) -> (NativeModuleTypeAnnotation, bool) {
    match annotation {
        NativeModuleTypeAnnotation::NullableTypeAnnotation { type_annotation } => {
            (*type_annotation, true)
        }
        other => (other, false),
    }
}

fn strip_nullable(annotation: &NativeModuleTypeAnnotation) -> &NativeModuleTypeAnnotation {
    match annotation {
        NativeModuleTypeAnnotation::NullableTypeAnnotation { type_annotation } => {
            strip_nullable(type_annotation)
        }
        other => other,
    }
}

fn is_nullish_type(ty: &TsType) -> bool {
    matches!(
        ty,
        TsType::TsKeywordType(TsKeywordType {
            kind: TsKeywordTypeKind::TsNullKeyword
                | TsKeywordTypeKind::TsUndefinedKeyword
                | TsKeywordTypeKind::TsVoidKeyword,
            ..
        })
    )
}

fn is_supported_array_element(annotation: &NativeModuleTypeAnnotation) -> bool {
    !matches!(
        strip_nullable(annotation),
        NativeModuleTypeAnnotation::FunctionTypeAnnotation { .. }
            | NativeModuleTypeAnnotation::VoidTypeAnnotation
            | NativeModuleTypeAnnotation::PromiseTypeAnnotation { .. }
            | NativeModuleTypeAnnotation::ArrayBufferTypeAnnotation
    )
}

fn translate_literal_type(lit: &TsLitType) -> Result<NativeModuleTypeAnnotation> {
    match &lit.lit {
        TsLit::Str(s) => Ok(NativeModuleTypeAnnotation::StringLiteralTypeAnnotation {
            value: str_value(s),
        }),
        TsLit::Number(n) => {
            Ok(NativeModuleTypeAnnotation::NumberLiteralTypeAnnotation { value: n.value })
        }
        TsLit::Bool(b) => {
            Ok(NativeModuleTypeAnnotation::BooleanLiteralTypeAnnotation { value: b.value })
        }
        _ => bail!("Unsupported NativeModule literal type"),
    }
}

fn enum_expr_member_type(expr: &Expr) -> Option<NativeModuleEnumMemberType> {
    if string_literal_expr_value(expr).is_some() {
        return Some(NativeModuleEnumMemberType::StringTypeAnnotation);
    }
    if number_literal_expr_value(expr).is_some() {
        return Some(NativeModuleEnumMemberType::NumberTypeAnnotation);
    }
    None
}

fn enum_member_name(id: &TsEnumMemberId) -> String {
    match id {
        TsEnumMemberId::Ident(id) => id.sym.to_string(),
        TsEnumMemberId::Str(s) => str_value(s),
    }
}

fn string_literal_expr_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(Lit::Str(s)) => Some(str_value(s)),
        _ => None,
    }
}

fn number_literal_expr_value(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Lit(Lit::Num(n)) => Some(n.value),
        Expr::Unary(UnaryExpr {
            op: UnaryOp::Minus,
            arg,
            ..
        }) => match &**arg {
            Expr::Lit(Lit::Num(n)) => Some(-n.value),
            _ => None,
        },
        _ => None,
    }
}
