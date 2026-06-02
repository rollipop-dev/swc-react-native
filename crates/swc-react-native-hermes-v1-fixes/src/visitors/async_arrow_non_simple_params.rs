use std::collections::HashSet;

use swc_common::{SyntaxContext, DUMMY_SP};
use swc_ecma_ast::*;
use swc_ecma_utils::private_ident;
use swc_ecma_visit::{Visit, VisitMut, VisitMutWith, VisitWith};

use crate::ast::{block_from_arrow_body, call_expr, take_arrow_body, var_decl_stmt};

/// Source: https://github.com/expo/expo/blob/d427a802b11d79c8e45be8ceb582ca10fae103af/packages/babel-preset-expo/src/plugins/fix-hermes-v1-async-arrow-non-simple-params.ts
pub struct AsyncArrowNonSimpleParamsVisitor;

impl VisitMut for AsyncArrowNonSimpleParamsVisitor {
    fn visit_mut_arrow_expr(&mut self, arrow: &mut ArrowExpr) {
        if arrow.is_async && arrow.params.iter().any(|p| !matches!(p, Pat::Ident(_))) {
            rewrite_async_arrow_non_simple_params(arrow);
        }

        arrow.visit_mut_children_with(self);
    }
}

fn rewrite_async_arrow_non_simple_params(arrow: &mut ArrowExpr) {
    if arrow.params.iter().any(|p| matches!(p, Pat::Rest(_))) {
        let body = block_from_arrow_body(take_arrow_body(arrow));
        let inner_async = Expr::Arrow(ArrowExpr {
            span: DUMMY_SP,
            ctxt: arrow.ctxt,
            params: vec![],
            body: Box::new(BlockStmtOrExpr::BlockStmt(body)),
            is_async: true,
            is_generator: false,
            type_params: None,
            return_type: None,
        });

        arrow.is_async = false;
        *arrow.body = BlockStmtOrExpr::Expr(Box::new(call_expr(inner_async)));
        return;
    }

    let mut used_names = UsedNameCollector::default();
    arrow.params.visit_with(&mut used_names);
    arrow.body.visit_with(&mut used_names);

    let params = std::mem::take(&mut arrow.params);
    let mut new_params = Vec::with_capacity(params.len());
    let mut init_stmts = vec![];

    for param in params {
        match param {
            Pat::Ident(_) => new_params.push(param),
            Pat::Assign(assign) => {
                let sym = next_param_ident(&mut used_names.names);
                new_params.push(Pat::Ident(sym.clone().into()));
                init_stmts.push(var_decl_stmt(
                    *assign.left,
                    Expr::Cond(CondExpr {
                        span: DUMMY_SP,
                        test: Box::new(Expr::Bin(BinExpr {
                            span: DUMMY_SP,
                            op: BinaryOp::EqEqEq,
                            left: Box::new(Expr::Ident(sym.clone())),
                            right: Box::new(undefined_ident()),
                        })),
                        cons: assign.right,
                        alt: Box::new(Expr::Ident(sym)),
                    }),
                ));
            }
            other => {
                let sym = next_param_ident(&mut used_names.names);
                new_params.push(Pat::Ident(sym.clone().into()));
                init_stmts.push(var_decl_stmt(other, Expr::Ident(sym)));
            }
        }
    }

    let mut body = block_from_arrow_body(take_arrow_body(arrow));
    body.stmts.splice(0..0, init_stmts);
    arrow.params = new_params;
    *arrow.body = BlockStmtOrExpr::BlockStmt(body);
}

fn undefined_ident() -> Expr {
    Expr::Ident(Ident::new(
        "undefined".into(),
        DUMMY_SP,
        SyntaxContext::empty(),
    ))
}

fn next_param_ident(used_names: &mut HashSet<String>) -> Ident {
    let mut idx = 1;
    loop {
        let name = if idx == 1 {
            "_p".to_string()
        } else {
            format!("_p{idx}")
        };

        if used_names.insert(name.clone()) {
            return private_ident!(name);
        }
        idx += 1;
    }
}

#[derive(Default)]
struct UsedNameCollector {
    names: HashSet<String>,
}

impl Visit for UsedNameCollector {
    fn visit_ident(&mut self, ident: &Ident) {
        self.names.insert(ident.sym.to_string());
    }
}

// Hermes fix: https://github.com/facebook/hermes/commit/68bfb3a48b31a19ac904ce6d3174ab2698ffc5e9
// Covers async arrows with default/destructuring params and the rest-param wrapper path.
#[cfg(test)]
mod tests {
    use super::AsyncArrowNonSimpleParamsVisitor;
    use crate::test_utils::{assert_contains, assert_not_contains, transform_with};
    use swc_ecma_visit::visit_mut_pass;

    #[test]
    fn lowers_defaults_and_destructuring() {
        let code = r#"
const _p = "outer";
const fn = async (value = 1, { name }, [first]) => value + name + first + _p;
"#;

        let out = transform_with(
            "Sample.ts",
            code,
            visit_mut_pass(AsyncArrowNonSimpleParamsVisitor),
        );

        assert_contains(&out, "async (_p2, _p3, _p4)");
        assert_contains(&out, "var value = _p2 === undefined ? 1 : _p2;");
        assert_contains(&out, "var { name } = _p3;");
        assert_contains(&out, "var [first] = _p4;");
        insta::assert_snapshot!(out);
    }

    #[test]
    fn rest_param_wraps_in_sync_arrow() {
        let code = r#"
const collect = async (...items) => items.length;
"#;

        let out = transform_with(
            "Sample.ts",
            code,
            visit_mut_pass(AsyncArrowNonSimpleParamsVisitor),
        );

        assert_contains(&out, "const collect = (...items)=>(async ()=>");
        assert_not_contains(&out, "const collect = async (...items)");
        insta::assert_snapshot!(out);
    }
}
