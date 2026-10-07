// Port of plugin-oxc/src/file_directive.rs.
use crate::directives::{add_worklet_directive, ensure_block_body, string_statement};
use swc_common::DUMMY_SP;
use swc_ecma_ast::*;

pub fn process_module(module: &mut Module, legacy: bool) {
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
            ModuleItem::Stmt(stmt) => inject_statement(stmt, legacy),
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                inject_declaration(&mut export.decl, legacy)
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(export)) => {
                match &mut export.decl {
                    DefaultDecl::Fn(function) => {
                        if let Some(body) = &mut function.function.body {
                            add_worklet_directive(body);
                        }
                    }
                    DefaultDecl::Class(class) if legacy => {
                        crate::legacy_classes::mark(&mut class.class)
                    }
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

pub fn process_script(script: &mut Script, legacy: bool) {
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
        inject_statement(stmt, legacy);
    }
}

fn inject_statement(stmt: &mut Stmt, legacy: bool) {
    if let Stmt::Decl(decl) = stmt {
        inject_declaration(decl, legacy);
    }
}
fn inject_declaration(decl: &mut Decl, legacy: bool) {
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
        Decl::Class(class) if legacy => crate::legacy_classes::mark(&mut class.class),
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

pub fn set_bundle_flag(body: &mut [Stmt], filename: &str, enabled: bool) {
    const PATHS: &[&str] = &[
        "react-native-worklets/src/index.ts",
        "react-native-worklets/src/debug/bundleMode.native.ts",
        "react-native-worklets/lib/module/index.js",
        "react-native-worklets/lib/module/debug/bundleMode.native.js",
        "react-native-worklets/bundleMode/polyfills/prepareBundleMode.js",
    ];
    if !PATHS.iter().any(|path| filename.ends_with(path)) {
        return;
    }
    for stmt in body {
        if let Stmt::Expr(stmt) = stmt {
            if let Expr::Assign(assign) = &mut *stmt.expr {
                if let AssignTarget::Simple(SimpleAssignTarget::Member(member)) = &assign.left {
                    if matches!(&*member.obj, Expr::Ident(id) if id.sym == "globalThis")
                        && matches!(&member.prop, MemberProp::Ident(id) if id.sym == "_WORKLETS_BUNDLE_MODE_ENABLED")
                    {
                        *assign.right = Expr::Lit(Lit::Bool(Bool {
                            span: DUMMY_SP,
                            value: enabled,
                        }));
                    }
                }
            }
        }
    }
}
