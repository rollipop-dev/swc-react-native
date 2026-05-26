use swc_common::{util::take::Take, SyntaxContext, DUMMY_SP};
use swc_ecma_ast::*;

pub(crate) fn block_from_arrow_body(body: BlockStmtOrExpr) -> BlockStmt {
    match body {
        BlockStmtOrExpr::BlockStmt(body) => body,
        BlockStmtOrExpr::Expr(expr) => BlockStmt {
            span: DUMMY_SP,
            ctxt: SyntaxContext::empty(),
            stmts: vec![Stmt::Return(ReturnStmt {
                span: DUMMY_SP,
                arg: Some(expr),
            })],
        },
    }
}

pub(crate) fn var_decl_stmt(name: Pat, init: Expr) -> Stmt {
    Stmt::Decl(Decl::Var(Box::new(VarDecl {
        span: DUMMY_SP,
        ctxt: SyntaxContext::empty(),
        kind: VarDeclKind::Var,
        declare: false,
        decls: vec![VarDeclarator {
            span: DUMMY_SP,
            name,
            init: Some(Box::new(init)),
            definite: false,
        }],
    })))
}

pub(crate) fn call_expr(callee: Expr) -> Expr {
    Expr::Call(CallExpr {
        span: DUMMY_SP,
        ctxt: SyntaxContext::empty(),
        callee: Callee::Expr(Box::new(Expr::Paren(ParenExpr {
            span: DUMMY_SP,
            expr: Box::new(callee),
        }))),
        args: vec![],
        type_args: None,
    })
}

pub(crate) fn take_arrow_body(arrow: &mut ArrowExpr) -> BlockStmtOrExpr {
    *arrow.body.take()
}
