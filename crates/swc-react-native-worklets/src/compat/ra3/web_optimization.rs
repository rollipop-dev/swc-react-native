//! Port of Reanimated 3.19.5 plugin/src/substituteWebCallExpression.ts.

use swc_common::DUMMY_SP;
use swc_ecma_ast::*;

// RA3 substitutes web checks in the host AST after serialized worklet data
// has been generated. The UI implementation retains the original calls.
pub(crate) struct SubstituteWebChecks;
impl swc_ecma_visit::VisitMut for SubstituteWebChecks {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        use swc_ecma_visit::VisitMutWith;
        expr.visit_mut_children_with(self);
        if matches!(expr, Expr::Call(call) if matches!(&call.callee,
            Callee::Expr(callee) if matches!(&**callee, Expr::Ident(name)
                if matches!(name.sym.as_ref(), "isWeb" | "shouldBeUseWeb"))))
        {
            *expr = Expr::Lit(Lit::Bool(Bool {
                span: DUMMY_SP,
                value: true,
            }));
        }
    }
}
