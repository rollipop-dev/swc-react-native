//! Port of Reanimated 3.19.5 plugin/src/file.ts.

use super::context_object::{is_context_object, CONTEXT_MARKER};
use crate::ast::ident_name;
use swc_common::DUMMY_SP;
use swc_ecma_ast::*;

// Port of plugin/src/file.ts::isImplicitContextObject. Called only for
// entities in a file carrying the top-level 'worklet' directive.
pub(crate) fn mark_implicit_context_object(object: &mut ObjectLit) -> bool {
    use swc_ecma_visit::{Visit, VisitWith};
    struct ThisFinder(bool);
    impl Visit for ThisFinder {
        fn visit_this_expr(&mut self, _: &ThisExpr) {
            self.0 = true;
        }
    }
    let implicit = object.props.iter().any(|property| {
        let PropOrSpread::Prop(property) = property else {
            return false;
        };
        let function = match &**property {
            Prop::Method(method) => &method.function,
            Prop::Getter(getter) => &getter.function,
            Prop::Setter(setter) => &setter.function,
            _ => return false,
        };
        let mut finder = ThisFinder(false);
        function.visit_with(&mut finder);
        finder.0
    });
    if implicit && !is_context_object(object) {
        object
            .props
            .push(PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                key: PropName::Ident(ident_name(CONTEXT_MARKER)),
                value: Box::new(Expr::Lit(Lit::Bool(Bool {
                    span: DUMMY_SP,
                    value: true,
                }))),
            }))));
    }
    implicit || is_context_object(object)
}

// Port of plugin-oxc/src/file_directive.rs.
use crate::directives::{add_worklet_directive, ensure_block_body, string_statement};

pub fn process_module(module: &mut Module) {
    let end = module
        .body
        .iter()
        .take_while(
            |item| matches!(item, ModuleItem::Stmt(stmt) if string_statement(stmt).is_some()),
        )
        .count();
    if !module.body[..end].iter().any(
        |item| matches!(item, ModuleItem::Stmt(stmt) if string_statement(stmt) == Some("worklet")),
    ) {
        return;
    }
    let directives: Vec<_> = module.body.drain(..end).filter(|item| !matches!(item, ModuleItem::Stmt(stmt) if string_statement(stmt) == Some("worklet"))).collect();
    module.body.splice(..0, directives);
    let mut tail = Vec::new();
    module.body.retain(|item| {
        if matches!(item, ModuleItem::Stmt(stmt) if is_commonjs_export(stmt)) {
            tail.push(item.clone());
            false
        } else {
            true
        }
    });
    module.body.extend(tail);
    for item in &mut module.body {
        match item {
            ModuleItem::Stmt(stmt) => inject_statement(stmt),
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                inject_declaration(&mut export.decl)
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(export)) => {
                match &mut export.decl {
                    DefaultDecl::Fn(function) => {
                        if let Some(body) = &mut function.function.body {
                            add_worklet_directive(body);
                        }
                    }
                    DefaultDecl::Class(class) => crate::legacy_classes::mark(&mut class.class),
                    _ => {}
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(export)) => {
                inject_expression(&mut export.expr)
            }
            _ => {}
        }
    }
}

pub fn process_script(script: &mut Script) {
    let end = script
        .body
        .iter()
        .take_while(|stmt| string_statement(stmt).is_some())
        .count();
    if !script.body[..end]
        .iter()
        .any(|stmt| string_statement(stmt) == Some("worklet"))
    {
        return;
    }
    let directives: Vec<_> = script
        .body
        .drain(..end)
        .filter(|stmt| string_statement(stmt) != Some("worklet"))
        .collect();
    script.body.splice(..0, directives);
    let mut tail = Vec::new();
    script.body.retain(|stmt| {
        if is_commonjs_export(stmt) {
            tail.push(stmt.clone());
            false
        } else {
            true
        }
    });
    script.body.extend(tail);
    for stmt in &mut script.body {
        inject_statement(stmt);
    }
}

fn inject_statement(stmt: &mut Stmt) {
    if let Stmt::Decl(decl) = stmt {
        inject_declaration(decl);
    }
}
fn inject_declaration(decl: &mut Decl) {
    match decl {
        Decl::Fn(function) => {
            if let Some(body) = &mut function.function.body {
                add_worklet_directive(body);
            }
        }
        Decl::Var(var) => {
            for declarator in &mut var.decls {
                if let Some(init) = &mut declarator.init {
                    inject_expression(init);
                }
            }
        }
        Decl::Class(class) => crate::legacy_classes::mark(&mut class.class),
        _ => {}
    }
}
fn inject_expression(expr: &mut Expr) {
    match expr {
        Expr::Fn(function) => {
            if let Some(body) = &mut function.function.body {
                add_worklet_directive(body);
            }
        }
        Expr::Arrow(arrow) => add_worklet_directive(ensure_block_body(arrow)),
        Expr::Object(obj) => {
            if mark_implicit_context_object(obj) {
                return;
            }
            for prop in &mut obj.props {
                if let PropOrSpread::Prop(prop) = prop {
                    match &mut **prop {
                        Prop::Method(method) => {
                            if let Some(body) = &mut method.function.body {
                                add_worklet_directive(body);
                            }
                        }
                        Prop::Getter(method) => {
                            if let Some(body) = &mut method.function.body {
                                add_worklet_directive(body);
                            }
                        }
                        Prop::Setter(method) => {
                            if let Some(body) = &mut method.function.body {
                                add_worklet_directive(body);
                            }
                        }
                        Prop::KeyValue(prop) => inject_expression(&mut prop.value),
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
}
fn is_commonjs_export(stmt: &Stmt) -> bool {
    let Stmt::Expr(stmt) = stmt else { return false };
    let Expr::Assign(assign) = &*stmt.expr else {
        return false;
    };
    let AssignTarget::Simple(SimpleAssignTarget::Member(member)) = &assign.left else {
        return false;
    };
    fn target(member: &MemberExpr) -> bool {
        match &*member.obj {
            Expr::Ident(id) => {
                id.sym == "exports"
                    || (id.sym == "module"
                        && match &member.prop {
                            MemberProp::Ident(id) => id.sym == "exports",
                            MemberProp::Computed(expr) => {
                                matches!(&*expr.expr, Expr::Lit(Lit::Str(value)) if value.value == "exports")
                            }
                            _ => false,
                        })
            }
            Expr::Member(member) => target(member),
            _ => false,
        }
    }
    target(member)
}
