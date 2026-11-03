// Port of Reanimated 3.19.5 plugin/src/workletFactory.ts.
use super::worklet_pass::WorkletsVisitor;
use crate::ast::{
    array_expr, array_pattern, assign_ident_member, const_decl, id, ident_expr, ident_name,
    num_lit, str_lit,
};
use crate::closure::get_closure;
use crate::hash::worklet_hash;
use crate::legacy_classes::{substitute_class_constructors, SUFFIX};
use crate::naming::make_worklet_name;
use crate::worklet_string_code::build_worklet_string;
use rustc_hash::FxHashMap;
use swc_common::{sync::Lrc, Mark, SourceMap, SyntaxContext, DUMMY_SP};
use swc_ecma_ast::*;
use swc_ecma_utils::{replace_ident, ExprFactory};
use swc_ecma_visit::{noop_visit_mut_type, Visit, VisitMut, VisitMutWith, VisitWith};

pub struct WorkletInput {
    pub params: Vec<Param>,
    pub body: FunctionBody,
    pub serialized_body: Option<FunctionBody>,
    pub self_id: Option<Ident>,
    pub name: Option<String>,
    pub is_async: bool,
    pub is_generator: bool,
}

pub fn make_worklet_factory(
    mut input: WorkletInput,
    state: &mut WorkletsVisitor,
) -> Result<Expr, String> {
    let body_to_serialize = input.serialized_body.as_ref().unwrap_or(&input.body);
    let mut closure = get_closure(
        &input.params,
        body_to_serialize,
        input.self_id.as_ref(),
        &state.bindings,
        &state.globals,
        false,
        false,
    );
    super::filter_closure(&mut closure, &state.globals);
    let (worklet_name, react_name) =
        make_worklet_name(input.name.as_deref(), &state.filename, state.worklet_number);
    state.worklet_number += 1;
    if closure.iter().any(|id| id.sym == react_name) {
        return Err(format!(
            "the `{react_name}` worklet shadows a captured binding of the same name"
        ));
    }
    let react_ident = input
        .self_id
        .clone()
        .filter(|id| id.sym == react_name)
        .unwrap_or_else(|| id(&react_name));
    let mut serialized = body_to_serialize.clone();
    super::remove_worklet_directive(&mut serialized);
    let mut serialized_params = input.params.clone();
    let recursive = input
        .self_id
        .as_ref()
        .is_some_and(|self_id| references_self(&input, &self_id.to_id()));
    let recursion_statement = if recursive {
        let self_id = input.self_id.as_ref().unwrap();
        let recursion_ident = Ident::new(worklet_name.clone().into(), self_id.span, self_id.ctxt);
        replace_ident(&mut serialized, self_id.to_id(), &recursion_ident);
        replace_ident(&mut serialized_params, self_id.to_id(), &recursion_ident);
        Some(const_decl(
            Pat::Ident(recursion_ident.into()),
            Expr::This(ThisExpr { span: DUMMY_SP })
                .make_member(ident_name("_recur"))
                .into(),
        ))
    } else {
        None
    };
    substitute_class_constructors(&mut serialized, &mut closure);
    super::prepend_closure(&mut serialized, &closure);
    if let Some(statement) = recursion_statement {
        serialized.stmts.insert(0, statement);
    }
    let cm = state
        .source_map
        .clone()
        .unwrap_or_else(|| Lrc::new(SourceMap::default()));
    let location = state.location_path();
    let (code, source_map) = build_worklet_string(
        FnExpr {
            ident: Some(id(&worklet_name)),
            function: Box::new(Function {
                params: serialized_params,
                body: Some(serialized),
                is_async: input.is_async,
                is_generator: input.is_generator,
                ..Default::default()
            }),
        },
        &cm,
        &location,
        true,
        !state.is_release && !state.options.common.disable_source_maps,
    )?;
    let hash = worklet_hash(&code);
    super::remove_worklet_directive(&mut input.body);
    let init_id = (!state.options.common.omit_native_only_data).then(|| state.fresh_init_id(hash));
    if let Some(init_id) = &init_id {
        let mut props = vec![key_value("code", str_lit(&code))];
        if !state.is_release {
            props.push(key_value("location", str_lit(&location)));
            append_init_data_version(&mut props, state.plugin_version());
        }
        if let Some(map) = source_map {
            props.push(key_value("sourceMap", str_lit(&map)));
        }
        let declaration = const_decl(
            Pat::Ident(init_id.clone().into()),
            Expr::Object(ObjectLit {
                span: DUMMY_SP,
                props,
            }),
        );
        state.hoist_init_data(declaration);
    }
    let mut statements = Vec::new();
    if !state.is_release {
        let offset = if closure.is_empty() {
            1.0
        } else {
            -(closure.len() as f64) - 1.0
        };
        statements.push(const_decl(
            Pat::Ident(id("_e").into()),
            array_expr([
                Expr::New(NewExpr {
                    span: DUMMY_SP,
                    ctxt: Default::default(),
                    callee: Box::new(Expr::Member(
                        ident_expr("global").make_member(ident_name("Error")),
                    )),
                    args: Some(vec![]),
                    type_args: None,
                }),
                num_lit(offset),
                num_lit(-27.0),
            ]),
        ));
    }
    let inner = Expr::Fn(FnExpr {
        ident: None,
        function: Box::new(Function {
            params: input.params,
            body: Some(input.body),
            is_async: input.is_async,
            is_generator: input.is_generator,
            ..Default::default()
        }),
    });
    statements.push(const_decl(Pat::Ident(react_ident.clone().into()), inner));
    let values: Vec<Expr> = closure
        .iter()
        .map(|variable| {
            if let Some(name) = variable.sym.strip_suffix(SUFFIX) {
                return Expr::Member(
                    Expr::Ident(Ident::new(name.into(), variable.span, variable.ctxt))
                        .make_member(IdentName::new(variable.sym.clone(), DUMMY_SP)),
                );
            }
            Expr::Ident(variable.clone())
        })
        .collect();
    statements.push(assign_ident_member(
        &react_ident,
        "__closure",
        closure_expression(&closure, values),
    ));
    statements.push(assign_ident_member(
        &react_ident,
        "__workletHash",
        num_lit(hash as f64),
    ));
    if let Some(init_id) = &init_id {
        statements.push(assign_ident_member(
            &react_ident,
            "__initData",
            Expr::Ident(init_id.clone()),
        ));
    }
    if !state.is_release {
        statements.push(assign_ident_member(
            &react_ident,
            "__stackDetails",
            ident_expr("_e"),
        ));
    }
    statements.push(Stmt::Return(ReturnStmt {
        span: DUMMY_SP,
        arg: Some(Box::new(Expr::Ident(react_ident))),
    }));
    let mut factory_params = Vec::new();
    if let Some(init_id) = &init_id {
        factory_params.push(init_id.clone());
    }
    factory_params.extend(closure.iter().map(|variable| {
        variable.sym.strip_suffix(SUFFIX).map_or_else(
            || variable.clone(),
            |name| Ident::new(name.into(), variable.span, variable.ctxt),
        )
    }));
    // Factory parameters introduce a scope. In particular, captured globals
    // must not retain their unresolved Id as a binding: downstream parameter
    // and block-scoping passes would rename the outer call arguments too.
    let factory_bindings: Vec<_> = factory_params
        .iter()
        .map(|original| {
            Ident::new(
                original.sym.clone(),
                original.span,
                SyntaxContext::empty().apply_mark(Mark::new()),
            )
        })
        .collect();
    let binding_contexts = factory_params
        .iter()
        .zip(&factory_bindings)
        .map(|(original, binding)| (original.to_id(), binding.ctxt))
        .collect();
    statements.visit_mut_with(&mut FactoryBindingContexts(&binding_contexts));
    let factory = FnExpr {
        ident: Some(id(&format!("{worklet_name}Factory"))),
        function: Box::new(Function {
            params: if factory_params.is_empty() {
                vec![]
            } else {
                vec![Param {
                    span: DUMMY_SP,
                    decorators: vec![],
                    pat: array_pattern(&factory_bindings),
                }]
            },
            body: Some(FunctionBody {
                span: DUMMY_SP,
                stmts: statements,
            }),
            ..Default::default()
        }),
    };
    let args = if factory_params.is_empty() {
        vec![]
    } else {
        vec![array_expr(factory_params.into_iter().map(Expr::Ident)).as_arg()]
    };
    Ok(Expr::Fn(factory).wrap_with_paren().as_call(DUMMY_SP, args))
}

struct FactoryBindingContexts<'a>(&'a FxHashMap<Id, SyntaxContext>);

impl VisitMut for FactoryBindingContexts<'_> {
    noop_visit_mut_type!();

    fn visit_mut_ident(&mut self, ident: &mut Ident) {
        if let Some(context) = self.0.get(&ident.to_id()) {
            ident.ctxt = *context;
        }
    }
}

fn key_value(name: &str, value: Expr) -> PropOrSpread {
    PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        key: PropName::Ident(ident_name(name)),
        value: Box::new(value),
    })))
}

fn references_self(input: &WorkletInput, original: &Id) -> bool {
    struct Probe<'a> {
        original: &'a Id,
        found: bool,
    }
    impl Visit for Probe<'_> {
        fn visit_ident(&mut self, id: &Ident) {
            self.found |= &id.to_id() == self.original;
        }
        fn visit_binding_ident(&mut self, _: &BindingIdent) {}
        fn visit_fn_expr(&mut self, function: &FnExpr) {
            function.function.visit_with(self);
        }
        fn visit_fn_decl(&mut self, function: &FnDecl) {
            function.function.visit_with(self);
        }
    }
    let mut probe = Probe {
        original,
        found: false,
    };
    input.body.visit_with(&mut probe);
    input.params.visit_with(&mut probe);
    probe.found
}

fn closure_expression(closure: &[Ident], values: Vec<Expr>) -> Expr {
    Expr::Object(ObjectLit {
        span: DUMMY_SP,
        props: closure
            .iter()
            .zip(values)
            .map(|(variable, value)| key_value(variable.sym.as_ref(), value))
            .collect(),
    })
}

fn append_init_data_version(properties: &mut Vec<PropOrSpread>, version: &str) {
    properties.push(key_value("version", str_lit(version)));
}
