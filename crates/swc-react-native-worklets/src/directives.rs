// Port of plugin-oxc/src/directives.rs.
use swc_common::{Spanned, DUMMY_SP};
use swc_ecma_ast::*;
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::ast::str_lit;

pub fn string_statement(stmt: &Stmt) -> Option<&str> {
    let Stmt::Expr(stmt) = stmt else { return None };
    let Expr::Lit(Lit::Str(value)) = &*stmt.expr else {
        return None;
    };
    value.value.as_str()
}

pub fn has_directive(body: &FunctionBody, value: &str) -> bool {
    body.stmts
        .iter()
        .take_while(|stmt| string_statement(stmt).is_some())
        .any(|stmt| string_statement(stmt) == Some(value))
}

pub fn add_worklet_directive(body: &mut FunctionBody) {
    if !has_directive(body, "worklet") {
        body.stmts.insert(
            0,
            Stmt::Expr(ExprStmt {
                span: DUMMY_SP,
                expr: Box::new(str_lit("worklet")),
            }),
        );
    }
}

pub fn strip_worklet_directives(body: &mut FunctionBody, keep_no_memo: bool) {
    let was_worklet = has_directive(body, "worklet");
    let has_no_memo = has_directive(body, "use no memo");
    let end = body
        .stmts
        .iter()
        .take_while(|stmt| string_statement(stmt).is_some())
        .count();
    let mut directives: Vec<_> = body.stmts.drain(..end).collect();
    directives.retain(|stmt| {
        keep_no_memo
            && !matches!(
                string_statement(stmt),
                Some("worklet" | "no-worklet-closure" | "limit-init-data-hoisting")
            )
    });
    if keep_no_memo && was_worklet && !has_no_memo {
        directives.push(Stmt::Expr(ExprStmt {
            span: DUMMY_SP,
            expr: Box::new(str_lit("use no memo")),
        }));
    }
    body.stmts.splice(..0, directives);
}

pub fn ensure_block_body(arrow: &mut ArrowExpr) -> &mut FunctionBody {
    if let ArrowFunctionBody::Expr(expr) = &*arrow.body {
        *arrow.body = ArrowFunctionBody::FunctionBody(FunctionBody {
            span: arrow.span,
            stmts: vec![Stmt::Return(ReturnStmt {
                span: expr.span(),
                arg: Some(expr.clone()),
            })],
        });
    }
    match &mut *arrow.body {
        ArrowFunctionBody::FunctionBody(body) => body,
        ArrowFunctionBody::Expr(_) => unreachable!("arrow body converted to block"),
    }
}

pub struct StripDirectivePrologues;

impl VisitMut for StripDirectivePrologues {
    fn visit_mut_function_body(&mut self, body: &mut FunctionBody) {
        strip_worklet_directives(body, false);
        body.visit_mut_children_with(self);
    }
}
