// Port of plugin-oxc/src/worklet_factory.rs and factory_expression.rs.
// The Legacy Eval branch follows plugin/src/workletFactory.ts.
use crate::ast::{
    array_expr, array_pattern, assign_ident_member, const_decl, id, ident_expr, ident_name,
    num_lit, str_lit,
};
use crate::closure::get_closure;
use crate::directives::{has_directive, strip_worklet_directives, StripDirectivePrologues};
use crate::hash::worklet_hash;
use crate::imports::{split_forwarded_imports, update_relative_requires};
use crate::legacy_classes::{substitute_class_constructors, SUFFIX};
use crate::naming::make_worklet_name;
use crate::worklet_file::{generate_worklet_file, EmittedFile};
use crate::worklet_pass::WorkletsVisitor;
use crate::worklet_string_code::{build_worklet_string, prepend_closure};
use swc_common::{sync::Lrc, SourceMap, DUMMY_SP};
use swc_ecma_ast::*;
use swc_ecma_utils::{replace_ident, ExprFactory};
use swc_ecma_visit::{Visit, VisitMutWith, VisitWith};

pub struct WorkletInput {
    pub params: Vec<Param>,
    pub body: FunctionBody,
    pub self_id: Option<Ident>,
    pub name: Option<String>,
    pub is_async: bool,
    pub is_generator: bool,
}

pub fn make_worklet_factory(
    mut input: WorkletInput,
    state: &mut WorkletsVisitor,
) -> Result<Expr, String> {
    let bundle = state.options.bundle_mode;
    let include_closure = bundle || !has_directive(&input.body, "no-worklet-closure");
    let limited_hoisting = has_directive(&input.body, "limit-init-data-hoisting");
    let mut closure = if include_closure {
        get_closure(
            &input.params,
            &input.body,
            input.self_id.as_ref(),
            &state.bindings,
            &state.globals,
            state.options.strict_global,
            bundle,
        )
    } else {
        vec![]
    };
    let imports = if bundle {
        split_forwarded_imports(
            &mut closure,
            &state.imports,
            &state.rebound,
            &state.filename,
            &state.forwardable_modules,
            &state.forwardable_paths,
        )
    } else {
        vec![]
    };
    let (worklet_name, mut react_name) =
        make_worklet_name(input.name.as_deref(), &state.filename, state.worklet_number);
    state.worklet_number += 1;
    if bundle && closure.is_empty() {
        while imports
            .iter()
            .any(|import| import.specifier.local().sym == react_name)
        {
            react_name.insert(0, '_');
        }
    }
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
    let mut serialized = input.body.clone();
    if bundle {
        strip_worklet_directives(&mut serialized, false);
    } else {
        serialized.visit_mut_with(&mut StripDirectivePrologues);
    }
    let mut serialized_params = input.params.clone();
    let recursive = input
        .self_id
        .as_ref()
        .is_some_and(|self_id| references_self(&input, &self_id.to_id()));
    let recursion_statement = if recursive {
        let self_id = input.self_id.as_ref().unwrap();
        let recursion_ident = if bundle {
            self_id.clone()
        } else {
            Ident::new(worklet_name.clone().into(), self_id.span, self_id.ctxt)
        };
        if !bundle {
            replace_ident(&mut serialized, self_id.to_id(), &recursion_ident);
            replace_ident(&mut serialized_params, self_id.to_id(), &recursion_ident);
        }
        Some(const_decl(
            Pat::Ident(recursion_ident.into()),
            Expr::This(ThisExpr { span: DUMMY_SP })
                .make_member(ident_name("_recur"))
                .into(),
        ))
    } else {
        None
    };
    if !bundle && !state.options.disable_worklet_classes {
        substitute_class_constructors(&mut serialized, &mut closure);
    }
    prepend_closure(&mut serialized, &closure);
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
        !bundle,
        !bundle && !state.is_release && !state.options.disable_source_maps,
    )?;
    let hash = worklet_hash(&code);
    strip_worklet_directives(&mut input.body, true);
    let init_id =
        (!bundle && !state.options.omit_native_only_data).then(|| state.fresh_init_id(hash));
    if let Some(init_id) = &init_id {
        let mut props = vec![key_value("code", str_lit(&code))];
        if !state.is_release {
            props.push(key_value("location", str_lit(&location)));
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
        state.hoist_init_data(declaration, limited_hoisting)?;
    }
    let mut statements = Vec::new();
    if !bundle && !state.is_release {
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
    if !closure.is_empty() {
        statements.push(assign_ident_member(
            &react_ident,
            "__closure",
            array_expr(closure.iter().map(|variable| {
                if !bundle {
                    if let Some(name) = variable.sym.strip_suffix(SUFFIX) {
                        return Expr::Member(
                            Expr::Ident(Ident::new(name.into(), variable.span, variable.ctxt))
                                .make_member(IdentName::new(variable.sym.clone(), DUMMY_SP)),
                        );
                    }
                }
                Expr::Ident(variable.clone())
            })),
        ));
    }
    statements.push(assign_ident_member(
        &react_ident,
        "__workletHash",
        num_lit(hash as f64),
    ));
    if !state.is_release {
        statements.push(assign_ident_member(
            &react_ident,
            "__pluginVersion",
            str_lit(state.plugin_version()),
        ));
    }
    if let Some(init_id) = &init_id {
        statements.push(assign_ident_member(
            &react_ident,
            "__initData",
            Expr::Ident(init_id.clone()),
        ));
    }
    if !bundle && !state.is_release {
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
        if !bundle {
            variable.sym.strip_suffix(SUFFIX).map_or_else(
                || variable.clone(),
                |name| Ident::new(name.into(), variable.span, variable.ctxt),
            )
        } else {
            variable.clone()
        }
    }));
    let mut factory = FnExpr {
        ident: Some(id(&format!("{worklet_name}Factory"))),
        function: Box::new(Function {
            params: if factory_params.is_empty() {
                vec![]
            } else {
                vec![Param {
                    span: DUMMY_SP,
                    decorators: vec![],
                    pat: array_pattern(&factory_params),
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
    if bundle {
        let package_dir = state.worklets_package_dir.as_ref().ok_or_else(|| {
            "could not resolve react-native-worklets package directory for Bundle Mode".to_string()
        })?;
        update_relative_requires(
            factory.function.body.as_mut().unwrap(),
            &state.filename,
            &state.forwardable_paths,
            package_dir,
        );
        let content = generate_worklet_file(factory, &imports, &state.filename, package_dir, &cm)?;
        let path = format!("react-native-worklets/.worklets/{hash}.js");
        let required = ident_expr("require").as_call(DUMMY_SP, vec![str_lit(&path).as_arg()]);
        let exported = Expr::Member(required.make_member(ident_name("default")));
        state.emitted_files.push(EmittedFile { path, content });
        if args.is_empty() {
            Ok(exported)
        } else {
            Ok(exported.as_call(DUMMY_SP, args))
        }
    } else {
        Ok(Expr::Fn(factory).wrap_with_paren().as_call(DUMMY_SP, args))
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
