//! Port of Reanimated 3.19.5 plugin/src/workletStringCode.ts.

use crate::ast::{const_decl, ident_name};
use swc_common::DUMMY_SP;
use swc_ecma_ast::*;
use swc_ecma_utils::ExprFactory;

pub(crate) fn prepend_closure(body: &mut FunctionBody, closure: &[Ident]) {
    if closure.is_empty() {
        return;
    }
    body.stmts.insert(
        0,
        const_decl(
            Pat::Object(ObjectPat {
                span: DUMMY_SP,
                props: closure
                    .iter()
                    .map(|variable| {
                        ObjectPatProp::KeyValue(KeyValuePatProp {
                            key: PropName::Ident(ident_name(variable.sym.as_ref())),
                            value: Box::new(Pat::Ident(variable.clone().into())),
                        })
                    })
                    .collect(),
                optional: false,
                type_ann: None,
            }),
            Expr::This(ThisExpr { span: DUMMY_SP })
                .make_member(ident_name("__closure"))
                .into(),
        ),
    );
}

// RA3 removes only its own worklet directive, preserving other prologues.
pub(crate) fn remove_worklet_directive(body: &mut FunctionBody) {
    let end = body
        .stmts
        .iter()
        .take_while(|stmt| crate::directives::string_statement(stmt).is_some())
        .count();
    let directives: Vec<_> = body
        .stmts
        .drain(..end)
        .filter(|stmt| crate::directives::string_statement(stmt) != Some("worklet"))
        .collect();
    body.stmts.splice(..0, directives);
}
