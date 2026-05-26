use swc_common::{SyntaxContext, DUMMY_SP};
use swc_ecma_ast::*;
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::ast::{call_expr, var_decl_stmt};

/// Source: https://github.com/expo/expo/blob/d427a802b11d79c8e45be8ceb582ca10fae103af/packages/babel-preset-expo/src/plugins/fix-hermes-v1-class-in-finally.ts
#[derive(Default)]
pub struct ClassInFinallyVisitor {
    finalizer_depth: usize,
}

impl ClassInFinallyVisitor {
    fn in_finalizer(&self) -> bool {
        self.finalizer_depth > 0
    }

    fn reset_finalizer_depth<T>(&mut self, node: &mut T)
    where
        T: VisitMutWith<Self>,
    {
        let saved = self.finalizer_depth;
        self.finalizer_depth = 0;
        node.visit_mut_children_with(self);
        self.finalizer_depth = saved;
    }
}

impl VisitMut for ClassInFinallyVisitor {
    fn visit_mut_try_stmt(&mut self, stmt: &mut TryStmt) {
        stmt.block.visit_mut_with(self);
        stmt.handler.visit_mut_with(self);

        if let Some(finalizer) = &mut stmt.finalizer {
            self.finalizer_depth += 1;
            finalizer.visit_mut_with(self);
            self.finalizer_depth -= 1;
        }
    }

    fn visit_mut_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::Decl(Decl::Class(class_decl)) = stmt {
            if self.in_finalizer() && class_decl.class.decorators.is_empty() {
                *stmt = class_decl_in_finalizer_stmt(class_decl);
                return;
            }
        }

        stmt.visit_mut_children_with(self);
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Class(class_expr) = expr {
            if self.in_finalizer() && class_expr.class.decorators.is_empty() {
                *expr = call_expr(Expr::Arrow(ArrowExpr {
                    span: DUMMY_SP,
                    ctxt: SyntaxContext::empty(),
                    params: vec![],
                    body: Box::new(BlockStmtOrExpr::Expr(Box::new(Expr::Class(
                        class_expr.clone(),
                    )))),
                    is_async: false,
                    is_generator: false,
                    type_params: None,
                    return_type: None,
                }));
                return;
            }
        }

        expr.visit_mut_children_with(self);
    }

    fn visit_mut_function(&mut self, function: &mut Function) {
        self.reset_finalizer_depth(function);
    }

    fn visit_mut_arrow_expr(&mut self, arrow: &mut ArrowExpr) {
        self.reset_finalizer_depth(arrow);
    }

    fn visit_mut_method_prop(&mut self, prop: &mut MethodProp) {
        self.reset_finalizer_depth(prop);
    }

    fn visit_mut_getter_prop(&mut self, prop: &mut GetterProp) {
        self.reset_finalizer_depth(prop);
    }

    fn visit_mut_setter_prop(&mut self, prop: &mut SetterProp) {
        self.reset_finalizer_depth(prop);
    }

    fn visit_mut_class(&mut self, class: &mut Class) {
        class.decorators.visit_mut_with(self);
        class.super_class.visit_mut_with(self);

        let saved = self.finalizer_depth;
        self.finalizer_depth = 0;
        class.body.visit_mut_with(self);
        self.finalizer_depth = saved;
    }
}

fn class_decl_in_finalizer_stmt(class_decl: &ClassDecl) -> Stmt {
    let ident = class_decl.ident.clone();
    let mut inner_class = (*class_decl.class).clone();
    inner_class.decorators.clear();

    let inner = Stmt::Decl(Decl::Class(ClassDecl {
        ident: ident.clone(),
        declare: false,
        class: Box::new(inner_class),
    }));

    let arrow = Expr::Arrow(ArrowExpr {
        span: DUMMY_SP,
        ctxt: SyntaxContext::empty(),
        params: vec![],
        body: Box::new(BlockStmtOrExpr::BlockStmt(BlockStmt {
            span: DUMMY_SP,
            ctxt: SyntaxContext::empty(),
            stmts: vec![
                inner,
                Stmt::Return(ReturnStmt {
                    span: DUMMY_SP,
                    arg: Some(Box::new(Expr::Ident(ident.clone()))),
                }),
            ],
        })),
        is_async: false,
        is_generator: false,
        type_params: None,
        return_type: None,
    });

    var_decl_stmt(Pat::Ident(ident.into()), call_expr(arrow))
}

// Hermes fix: https://github.com/facebook/hermes/commit/1e94fbe0e
// Covers class declarations/expressions in finalizers and finalizer-scope boundaries.
#[cfg(test)]
mod tests {
    use super::ClassInFinallyVisitor;
    use crate::test_utils::{assert_contains, assert_not_contains, transform_with};
    use swc_ecma_visit::visit_mut_pass;

    #[test]
    fn class_declaration_wraps_legacy_class_binding() {
        let code = r#"
try {
  work();
} finally {
  class Foo extends Base {
    value() {
      return 1;
    }
  }
  use(Foo);
}
"#;

        let out = transform_with(
            "Sample.ts",
            code,
            visit_mut_pass(ClassInFinallyVisitor::default()),
        );

        assert_contains(&out, "var Foo = (()=>");
        assert_contains(&out, "class Foo extends Base");
        assert_contains(&out, "return Foo;");
        assert_contains(&out, "})();");
        insta::assert_snapshot!(out);
    }

    #[test]
    fn function_local_finally_is_still_visited() {
        let code = r#"
function nested() {
  try {
    work();
  } finally {
    class Inner {}
    return Inner;
  }
}
"#;

        let out = transform_with(
            "Sample.ts",
            code,
            visit_mut_pass(ClassInFinallyVisitor::default()),
        );

        assert_contains(&out, "function nested()");
        assert_contains(&out, "var Inner = (()=>");
        insta::assert_snapshot!(out);
    }

    #[test]
    fn class_expression_wraps_expression_site() {
        let code = r#"
try {
  work();
} finally {
  const Foo = class {
    value() {
      return 1;
    }
  };
  use(class Bar {});
}
"#;

        let out = transform_with(
            "Sample.ts",
            code,
            visit_mut_pass(ClassInFinallyVisitor::default()),
        );

        assert_contains(&out, "const Foo = (()=>class");
        assert_contains(&out, "use((()=>class Bar");
        insta::assert_snapshot!(out);
    }

    #[test]
    fn class_in_function_inside_finally_is_left_alone() {
        let code = r#"
try {
  work();
} finally {
  function nested() {
    class Inner {}
    return Inner;
  }
}
"#;

        let out = transform_with(
            "Sample.ts",
            code,
            visit_mut_pass(ClassInFinallyVisitor::default()),
        );

        assert_contains(&out, "function nested()");
        assert_contains(&out, "class Inner");
        assert_not_contains(&out, "var Inner =");
        insta::assert_snapshot!(out);
    }
}
