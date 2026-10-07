//! SWC counterparts of plugin-oxc/src/ast.rs.

use swc_atoms::Atom;
use swc_common::DUMMY_SP;
use swc_ecma_ast::*;
use swc_ecma_utils::ExprFactory;

pub(crate) fn member_property(member: &MemberExpr) -> Option<&str> {
    match &member.prop {
        MemberProp::Ident(id) => Some(&id.sym),
        MemberProp::Computed(computed) => match &*computed.expr {
            Expr::Ident(id) => Some(&id.sym),
            _ => None,
        },
        MemberProp::PrivateName(_) => None,
    }
}

pub(crate) fn const_decl(pattern: Pat, value: Expr) -> Stmt {
    Stmt::Decl(Decl::Var(Box::new(
        value.into_var_decl(VarDeclKind::Const, pattern),
    )))
}

pub(crate) fn array_pattern(ids: &[Ident]) -> Pat {
    Pat::Array(ArrayPat {
        span: DUMMY_SP,
        elems: ids
            .iter()
            .map(|id| Some(Pat::Ident(id.clone().into())))
            .collect(),
        optional: false,
        type_ann: None,
    })
}

pub(crate) fn array_expr(values: impl IntoIterator<Item = Expr>) -> Expr {
    Expr::Array(ArrayLit {
        span: DUMMY_SP,
        elems: values.into_iter().map(|expr| Some(expr.as_arg())).collect(),
    })
}

#[inline]
pub(crate) fn id(name: &str) -> Ident {
    Ident::from(Atom::from(name))
}

#[inline]
pub(crate) fn ident_name(name: &str) -> IdentName {
    IdentName::from(Atom::from(name))
}

pub(crate) fn function_body_from_block(body: BlockStmt) -> FunctionBody {
    FunctionBody {
        span: body.span,
        stmts: body.stmts,
    }
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

pub(crate) fn assign_ident_member(target: &Ident, prop: &str, value: Expr) -> Stmt {
    let member = Expr::Ident(target.clone()).make_member(ident_name(prop));
    AssignExpr {
        span: DUMMY_SP,
        op: AssignOp::Assign,
        left: SimpleAssignTarget::Member(member).into(),
        right: Box::new(value),
    }
    .into_stmt()
}
