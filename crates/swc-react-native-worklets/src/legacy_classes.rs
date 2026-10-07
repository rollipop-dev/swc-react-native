// Legacy Eval adapter for plugin/src/class.ts and workletStringCode.ts.
// Bundle Mode deliberately leaves marked classes alone, as plugin-oxc does.
use crate::ast::{assign_ident_member, const_decl, id, ident_expr};
use crate::directives::add_worklet_directive;
use rustc_hash::FxHashSet;
use swc_common::{Mark, DUMMY_SP};
use swc_ecma_ast::*;
use swc_ecma_compat_es2015::classes;
use swc_ecma_compat_es2022::{
    class_properties::{class_properties, Config},
    private_in_object, static_blocks,
};
use swc_ecma_transformer::{regexp_pass, RegExpOptions};
use swc_ecma_transforms_base::helpers::{inject_helpers, Helpers, HELPERS};
use swc_ecma_utils::ExprFactory;
use swc_ecma_visit::{Visit, VisitMut, VisitMutWith, VisitWith};

pub const SUFFIX: &str = "__classFactory";

pub fn mark(class: &mut Class) {
    if !marked(class) {
        class.body.push(ClassMember::ClassProp(ClassProp {
            span: DUMMY_SP,
            key: PropName::Ident(crate::ast::ident_name("__workletClass")),
            value: Some(Box::new(Expr::Lit(Lit::Bool(Bool {
                span: DUMMY_SP,
                value: true,
            })))),
            ..Default::default()
        }));
    }
}

pub fn process_module(module: &mut Module) {
    let mut body = Vec::new();
    for item in std::mem::take(&mut module.body) {
        let (class, export) = match item {
            ModuleItem::Stmt(Stmt::Decl(Decl::Class(class))) if marked(&class.class) => (class, 0),
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => match export.decl {
                Decl::Class(class) if marked(&class.class) => (class, 1),
                decl => {
                    body.push(ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
                        decl,
                        ..export
                    })));
                    continue;
                }
            },
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(export)) => match export.decl {
                DefaultDecl::Class(class) if class.ident.is_some() && marked(&class.class) => (
                    ClassDecl {
                        ident: class.ident.unwrap(),
                        declare: false,
                        class: class.class,
                    },
                    2,
                ),
                decl => {
                    body.push(ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(
                        ExportDefaultDecl { decl, ..export },
                    )));
                    continue;
                }
            },
            item => {
                body.push(item);
                continue;
            }
        };
        let (factory, call) = class_factory(class);
        body.push(ModuleItem::Stmt(factory));
        if export == 1 {
            let Stmt::Decl(decl) = call else {
                unreachable!()
            };
            body.push(ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
                span: DUMMY_SP,
                decl,
            })));
        } else if export == 2 {
            let Stmt::Decl(Decl::Var(var)) = &call else {
                unreachable!()
            };
            let Pat::Ident(name) = &var.decls[0].name else {
                unreachable!()
            };
            let export = ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(ExportDefaultExpr {
                span: DUMMY_SP,
                expr: Box::new(Expr::Ident(name.id.clone())),
            }));
            body.push(ModuleItem::Stmt(call));
            body.push(export);
        } else {
            body.push(ModuleItem::Stmt(call));
        }
    }
    module.body = body;
    module.visit_mut_with(&mut NestedClasses);
}

pub fn process_script(script: &mut Script) {
    script.visit_mut_with(&mut NestedClasses);
}

struct NestedClasses;
impl VisitMut for NestedClasses {
    fn visit_mut_stmts(&mut self, stmts: &mut Vec<Stmt>) {
        let mut out = Vec::new();
        for mut stmt in std::mem::take(stmts) {
            if let Stmt::Decl(Decl::Class(class)) = stmt {
                if marked(&class.class) {
                    let (mut factory, call) = class_factory(class);
                    factory.visit_mut_children_with(self);
                    out.extend([factory, call]);
                } else {
                    stmt = Stmt::Decl(Decl::Class(class));
                    stmt.visit_mut_children_with(self);
                    out.push(stmt);
                }
            } else {
                stmt.visit_mut_children_with(self);
                out.push(stmt);
            }
        }
        *stmts = out;
    }
}

fn marked(class: &Class) -> bool {
    class.body.iter().any(marker)
}
fn marker(member: &ClassMember) -> bool {
    matches!(member, ClassMember::ClassProp(prop) if matches!(&prop.key, PropName::Ident(id) if id.sym == "__workletClass"))
}

fn class_factory(mut class: ClassDecl) -> (Stmt, Stmt) {
    class.class.body.retain(|member| !marker(member));
    let name = class.ident.clone();
    let factory_name = format!("{}{SUFFIX}", name.sym);
    let factory_ident = id(&factory_name);
    let assignment = assign_ident_member(&name, &factory_name, Expr::Ident(factory_ident.clone()));
    let span = class.class.span;
    // Keep SWC's compatibility helpers inside the class factory. They stay
    // ordinary local functions, so their self-caching assignments and hoisting
    // survive serialization without needing independent worklet metadata.
    let mut stmts = HELPERS.set(&Helpers::new(false), || {
        let unresolved = Mark::new();
        let mut regex_options = RegExpOptions::default();
        regex_options.unicode_regex = true;
        let mut program = Program::Module(Module {
            span,
            body: vec![ModuleItem::Stmt(Stmt::Decl(Decl::Class(class)))],
            shebang: None,
        });
        (
            regexp_pass(regex_options),
            static_blocks(),
            class_properties(Config::default(), unresolved),
            private_in_object(),
            classes(Default::default()),
            inject_helpers(unresolved),
        )
            .process(&mut program);
        let Program::Module(module) = program else {
            unreachable!()
        };
        module
            .body
            .into_iter()
            .filter_map(|item| match item {
                ModuleItem::Stmt(stmt) => Some(stmt),
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    stmts.extend([
        assignment,
        Stmt::Return(ReturnStmt {
            span: DUMMY_SP,
            arg: Some(Box::new(Expr::Ident(name.clone()))),
        }),
    ]);
    let mut body = FunctionBody { span, stmts };
    add_worklet_directive(&mut body);
    let factory = Stmt::Decl(Decl::Fn(FnDecl {
        ident: factory_ident,
        declare: false,
        function: Box::new(Function {
            span,
            params: vec![],
            body: Some(body),
            ..Default::default()
        }),
    }));
    let call = const_decl(
        Pat::Ident(name.into()),
        ident_expr(&factory_name).as_call(DUMMY_SP, vec![]),
    );
    (factory, call)
}

pub fn substitute_class_constructors(body: &mut FunctionBody, closure: &mut Vec<Ident>) {
    struct ConstructorRefs {
        captured: FxHashSet<Id>,
        found: Vec<Id>,
    }
    impl Visit for ConstructorRefs {
        fn visit_new_expr(&mut self, expr: &NewExpr) {
            if let Expr::Ident(id) = &*expr.callee {
                if self.captured.contains(&id.to_id()) && !self.found.contains(&id.to_id()) {
                    self.found.push(id.to_id());
                }
            }
            expr.visit_children_with(self);
        }
    }
    let mut refs = ConstructorRefs {
        captured: closure.iter().map(Ident::to_id).collect(),
        found: vec![],
    };
    body.visit_with(&mut refs);
    for found in refs.found {
        let Some(index) = closure.iter().position(|id| id.to_id() == found) else {
            continue;
        };
        let original = closure.remove(index);
        let factory = Ident::new(
            format!("{}{SUFFIX}", original.sym).into(),
            original.span,
            original.ctxt,
        );
        body.stmts.insert(
            0,
            const_decl(
                Pat::Ident(original.into()),
                Expr::Ident(factory.clone()).as_call(DUMMY_SP, vec![]),
            ),
        );
        closure.push(factory);
    }
}
