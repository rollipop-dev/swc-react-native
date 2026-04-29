//! AST builders. Centralizes all DUMMY_SP-spanned node construction so the
//! transform body can stay declarative.
//!
//! No upstream counterpart — the Babel plugin uses `@babel/types` directly.

use swc_atoms::Atom;
use swc_common::DUMMY_SP;
use swc_ecma_ast::*;
use swc_ecma_utils::ExprFactory;

#[inline]
pub(crate) fn id(name: &str) -> Ident {
    Ident::from(Atom::from(name))
}

#[inline]
pub(crate) fn ident_name(name: &str) -> IdentName {
    IdentName::from(Atom::from(name))
}

#[inline]
pub(crate) fn binding(name: &str) -> Pat {
    Pat::Ident(id(name).into())
}

#[inline]
pub(crate) fn ident_expr(name: &str) -> Expr {
    Expr::Ident(id(name))
}

pub(crate) fn str_lit(value: &str) -> Expr {
    Lit::Str(Str {
        span: DUMMY_SP,
        value: value.into(),
        raw: None,
    })
    .into()
}

pub(crate) fn num_lit(value: f64) -> Expr {
    Lit::Num(Number {
        span: DUMMY_SP,
        value,
        raw: None,
    })
    .into()
}

pub(crate) fn shorthand_prop(name: &str) -> PropOrSpread {
    PropOrSpread::Prop(Box::new(Prop::Shorthand(id(name))))
}

/// `<target>.<prop> = <value>;`
pub(crate) fn assign_member(target: &str, prop: &str, value: Expr) -> Stmt {
    let member = ident_expr(target).make_member(ident_name(prop));
    AssignExpr {
        span: DUMMY_SP,
        op: AssignOp::Assign,
        left: SimpleAssignTarget::Member(member).into(),
        right: Box::new(value),
    }
    .into_stmt()
}

/// `const <name> = function <name>(<params>) { <body> };`
pub(crate) fn const_named_fn(
    name: &str,
    params: Vec<Param>,
    body: BlockStmt,
    is_generator: bool,
    is_async: bool,
) -> Stmt {
    let func = Function {
        params,
        decorators: vec![],
        span: DUMMY_SP,
        ctxt: Default::default(),
        body: Some(body),
        is_generator,
        is_async,
        type_params: None,
        return_type: None,
    };
    let init = Expr::Fn(FnExpr {
        ident: Some(id(name)),
        function: Box::new(func),
    });
    Stmt::Decl(Decl::Var(Box::new(
        init.into_var_decl(VarDeclKind::Const, binding(name)),
    )))
}

/// `const <name> = this._recur;`
pub(crate) fn const_recur_decl(name: &str) -> Stmt {
    let init = Expr::This(ThisExpr { span: DUMMY_SP }).make_member(ident_name("_recur"));
    Stmt::Decl(Decl::Var(Box::new(
        Expr::from(init).into_var_decl(VarDeclKind::Const, binding(name)),
    )))
}
