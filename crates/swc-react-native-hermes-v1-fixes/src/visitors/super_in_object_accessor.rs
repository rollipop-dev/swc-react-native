use swc_ecma_ast::*;
use swc_ecma_visit::{Visit, VisitMut, VisitMutWith, VisitWith};

/// Source: https://github.com/expo/expo/blob/d427a802b11d79c8e45be8ceb582ca10fae103af/packages/babel-preset-expo/src/plugins/fix-hermes-v1-super-in-object-accessor.ts
pub struct SuperInObjectAccessorVisitor;

impl VisitMut for SuperInObjectAccessorVisitor {
    fn visit_mut_getter_prop(&mut self, prop: &mut GetterProp) {
        if prop
            .body
            .as_ref()
            .is_some_and(contains_direct_super_member_for_accessor)
        {
            make_accessor_key_computed_string(&mut prop.key);
        }

        prop.visit_mut_children_with(self);
    }

    fn visit_mut_setter_prop(&mut self, prop: &mut SetterProp) {
        if prop
            .body
            .as_ref()
            .is_some_and(contains_direct_super_member_for_accessor)
        {
            make_accessor_key_computed_string(&mut prop.key);
        }

        prop.visit_mut_children_with(self);
    }
}

fn contains_direct_super_member_for_accessor(body: &BlockStmt) -> bool {
    let mut finder = DirectSuperMemberFinder::default();
    body.visit_with(&mut finder);
    finder.found
}

#[derive(Default)]
struct DirectSuperMemberFinder {
    found: bool,
}

impl Visit for DirectSuperMemberFinder {
    fn visit_super_prop_expr(&mut self, _expr: &SuperPropExpr) {
        self.found = true;
    }

    fn visit_expr(&mut self, expr: &Expr) {
        if self.found {
            return;
        }
        if matches!(expr, Expr::SuperProp(_)) {
            self.found = true;
            return;
        }
        expr.visit_children_with(self);
    }

    fn visit_function(&mut self, _function: &Function) {}

    fn visit_class(&mut self, _class: &Class) {}

    fn visit_method_prop(&mut self, _prop: &MethodProp) {}

    fn visit_getter_prop(&mut self, _prop: &GetterProp) {}

    fn visit_setter_prop(&mut self, _prop: &SetterProp) {}
}

fn make_accessor_key_computed_string(key: &mut PropName) {
    match key {
        PropName::Ident(ident) => {
            let span = ident.span;
            let value = ident.sym.clone();
            *key = PropName::Computed(ComputedPropName {
                span,
                expr: Box::new(Expr::Lit(Lit::Str(Str {
                    span,
                    value: value.into(),
                    raw: None,
                }))),
            });
        }
        PropName::Str(str_lit) => {
            let span = str_lit.span;
            *key = PropName::Computed(ComputedPropName {
                span,
                expr: Box::new(Expr::Lit(Lit::Str(str_lit.clone()))),
            });
        }
        _ => {}
    }
}

// Hermes fix: https://github.com/facebook/hermes/commit/18a963465
// Covers direct, nested, and lexical-arrow `super.x` in object accessors.
#[cfg(test)]
mod tests {
    use super::SuperInObjectAccessorVisitor;
    use crate::test_utils::{assert_contains, assert_not_contains, transform_with};
    use swc_ecma_visit::visit_mut_pass;

    #[test]
    fn computes_getter_and_setter_keys() {
        let code = r#"
const obj = {
  get value() {
    return super.value;
  },
  set value(next) {
    super.value = next;
  },
  method() {
    return super.value;
  }
};
"#;

        let out = transform_with(
            "Sample.ts",
            code,
            visit_mut_pass(SuperInObjectAccessorVisitor),
        );

        assert_contains(&out, "get [\"value\"]");
        assert_contains(&out, "set [\"value\"]");
        assert_contains(&out, "method");
        insta::assert_snapshot!(out);
    }

    #[test]
    fn nested_object_accessor_does_not_rewrite_outer_accessor() {
        let code = r#"
const obj = {
  get outer() {
    return {
      get inner() {
        return super.inner;
      }
    };
  }
};
"#;

        let out = transform_with(
            "Sample.ts",
            code,
            visit_mut_pass(SuperInObjectAccessorVisitor),
        );

        assert_contains(&out, "get outer");
        assert_contains(&out, "get [\"inner\"]");
        assert_not_contains(&out, "get [\"outer\"]()");
        insta::assert_snapshot!(out);
    }

    #[test]
    fn accessor_arrow_body_still_rewrites_accessor_key() {
        let code = r#"
const obj = {
  get value() {
    return () => super.value;
  }
};
"#;

        let out = transform_with(
            "Sample.ts",
            code,
            visit_mut_pass(SuperInObjectAccessorVisitor),
        );

        assert_contains(&out, "get [\"value\"]");
        insta::assert_snapshot!(out);
    }
}
