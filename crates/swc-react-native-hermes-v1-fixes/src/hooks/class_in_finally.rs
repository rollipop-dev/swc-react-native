use swc_ecma_ast::{
    ArrowExpr, BlockStmt, ClassMember, Decl, Expr, Function, GetterProp, MethodProp, SetterProp,
    Stmt, TryStmt,
};
use swc_ecma_hooks::VisitMutHook;

use crate::visitors::class_in_finally::{
    class_decl_in_finalizer_stmt, class_expr_in_finalizer_expr,
};

#[derive(Default)]
pub(crate) struct ClassInFinallyHook {
    finalizer_depth: usize,
    scope_depths: Vec<usize>,
    // Addresses distinguish generated blocks that share DUMMY_SP. They are
    // compared only during the containing try statement's traversal.
    finalizers: Vec<Option<usize>>,
}

impl ClassInFinallyHook {
    fn enter_scope(&mut self) {
        self.scope_depths
            .push(std::mem::take(&mut self.finalizer_depth));
    }

    fn exit_scope(&mut self) {
        self.finalizer_depth = self.scope_depths.pop().expect("balanced scope hooks");
    }

    fn is_finalizer(&self, block: &BlockStmt) -> bool {
        self.finalizers
            .last()
            .is_some_and(|finalizer| *finalizer == Some(std::ptr::from_ref(block).addr()))
    }
}

macro_rules! scope_hooks {
    ($enter:ident, $exit:ident, $node:ty) => {
        fn $enter(&mut self, _node: &mut $node, _ctx: &mut C) {
            self.enter_scope();
        }
        fn $exit(&mut self, _node: &mut $node, _ctx: &mut C) {
            self.exit_scope();
        }
    };
}

impl<C> VisitMutHook<C> for ClassInFinallyHook {
    fn enter_try_stmt(&mut self, stmt: &mut TryStmt, _ctx: &mut C) {
        self.finalizers.push(
            stmt.finalizer
                .as_ref()
                .map(|block| std::ptr::from_ref(block).addr()),
        );
    }

    fn exit_try_stmt(&mut self, _stmt: &mut TryStmt, _ctx: &mut C) {
        self.finalizers.pop().expect("balanced try hooks");
    }

    fn enter_block_stmt(&mut self, block: &mut BlockStmt, _ctx: &mut C) {
        if self.is_finalizer(block) {
            self.finalizer_depth += 1;
        }
    }

    fn exit_block_stmt(&mut self, block: &mut BlockStmt, _ctx: &mut C) {
        if self.is_finalizer(block) {
            self.finalizer_depth -= 1;
        }
    }

    fn enter_stmt(&mut self, stmt: &mut Stmt, _ctx: &mut C) {
        if let Stmt::Decl(Decl::Class(class)) = stmt {
            if self.finalizer_depth > 0 && class.class.decorators.is_empty() {
                *stmt = class_decl_in_finalizer_stmt(class);
            }
        }
    }

    fn enter_expr(&mut self, expr: &mut Expr, _ctx: &mut C) {
        if let Expr::Class(class) = expr {
            if self.finalizer_depth > 0 && class.class.decorators.is_empty() {
                *expr = class_expr_in_finalizer_expr(class);
            }
        }
    }

    scope_hooks!(enter_function, exit_function, Function);
    scope_hooks!(enter_arrow_expr, exit_arrow_expr, ArrowExpr);
    scope_hooks!(enter_method_prop, exit_method_prop, MethodProp);
    scope_hooks!(enter_getter_prop, exit_getter_prop, GetterProp);
    scope_hooks!(enter_setter_prop, exit_setter_prop, SetterProp);
    scope_hooks!(enter_class_members, exit_class_members, Vec<ClassMember>);
}
