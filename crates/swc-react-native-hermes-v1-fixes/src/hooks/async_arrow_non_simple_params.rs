use swc_ecma_ast::{ArrowExpr, Pat};
use swc_ecma_hooks::VisitMutHook;

use crate::visitors::async_arrow_non_simple_params::rewrite_async_arrow_non_simple_params;

pub(crate) struct AsyncArrowNonSimpleParamsHook;

impl<C> VisitMutHook<C> for AsyncArrowNonSimpleParamsHook {
    fn enter_arrow_expr(&mut self, arrow: &mut ArrowExpr, _ctx: &mut C) {
        if arrow.is_async
            && arrow
                .params
                .iter()
                .any(|param| !matches!(param, Pat::Ident(_)))
        {
            rewrite_async_arrow_non_simple_params(arrow);
        }
    }
}
