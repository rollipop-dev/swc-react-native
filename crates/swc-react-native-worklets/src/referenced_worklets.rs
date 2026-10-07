// Port of plugin-oxc/src/referenced_worklets.rs.
use rustc_hash::{FxHashMap, FxHashSet};
use swc_common::Span;
use swc_ecma_ast::*;
use swc_ecma_visit::{Visit, VisitWith};

use crate::ast::member_property;

#[derive(Clone)]
pub enum Shape {
    Function(Span),
    Object(Vec<Property>),
    Alias(Id),
}

#[derive(Clone)]
pub enum Property {
    Method(Span),
    Value(Shape),
    Unsupported(&'static str),
}

#[derive(Default)]
pub struct Definitions {
    pub hand_written: FxHashSet<Id>,
    pub function_declarations: FxHashMap<Id, Span>,
    pub declarators: FxHashMap<Id, Shape>,
    pub assignments: FxHashMap<Id, Vec<Shape>>,
    pub rebound: FxHashSet<Id>,
    declarations: FxHashSet<Id>,
}

impl Definitions {
    pub fn describe(expr: &Expr) -> Option<Shape> {
        match expr {
            Expr::Paren(expr) => Self::describe(&expr.expr),
            Expr::TsAs(expr) => Self::describe(&expr.expr),
            Expr::TsNonNull(expr) => Self::describe(&expr.expr),
            Expr::Fn(expr) => Some(Shape::Function(expr.function.span)),
            Expr::Arrow(expr) => Some(Shape::Function(expr.span)),
            Expr::Ident(id) => Some(Shape::Alias(id.to_id())),
            Expr::Object(obj) => Some(Shape::Object(
                obj.props
                    .iter()
                    .filter_map(|prop| match prop {
                        PropOrSpread::Spread(_) => Some(Property::Unsupported("SpreadElement")),
                        PropOrSpread::Prop(prop) => match &**prop {
                            Prop::Method(method) => Some(Property::Method(method.function.span)),
                            Prop::Getter(method) => Some(Property::Method(method.function.span)),
                            Prop::Setter(method) => Some(Property::Method(method.function.span)),
                            Prop::KeyValue(prop) => {
                                Self::describe(&prop.value).map(Property::Value)
                            }
                            Prop::Shorthand(id) => Some(Property::Value(Shape::Alias(id.to_id()))),
                            Prop::Assign(_) => None,
                        },
                    })
                    .collect(),
            )),
            _ => None,
        }
    }
}

impl Visit for Definitions {
    fn visit_binding_ident(&mut self, id: &BindingIdent) {
        if !self.declarations.insert(id.to_id()) {
            self.rebound.insert(id.to_id());
        }
    }
    fn visit_member_expr(&mut self, member: &MemberExpr) {
        member.visit_children_with(self);
        if member_property(member) == Some("__workletHash") {
            if let Expr::Ident(id) = &*member.obj {
                self.hand_written.insert(id.to_id());
            }
        }
    }
    fn visit_fn_decl(&mut self, function: &FnDecl) {
        function.visit_children_with(self);
        self.function_declarations
            .insert(function.ident.to_id(), function.function.span);
    }
    fn visit_var_declarator(&mut self, declarator: &VarDeclarator) {
        declarator.visit_children_with(self);
        if let (Pat::Ident(id), Some(shape)) = (
            &declarator.name,
            declarator.init.as_deref().and_then(Self::describe),
        ) {
            self.declarators.insert(id.to_id(), shape);
        }
    }
    fn visit_assign_expr(&mut self, assignment: &AssignExpr) {
        assignment.visit_children_with(self);
        let ids = assignment_ids(&assignment.left);
        for id in ids {
            self.rebound.insert(id.clone());
            if let Some(shape) = Self::describe(&assignment.right) {
                self.assignments.entry(id).or_default().push(shape);
            }
        }
    }
    fn visit_update_expr(&mut self, update: &UpdateExpr) {
        update.visit_children_with(self);
        if let Expr::Ident(id) = &*update.arg {
            self.rebound.insert(id.to_id());
        }
    }
}

fn assignment_ids(target: &AssignTarget) -> Vec<Id> {
    struct Collector(Vec<Id>);
    impl Visit for Collector {
        fn visit_binding_ident(&mut self, id: &BindingIdent) {
            self.0.push(id.to_id());
        }
        fn visit_assign_pat_prop(&mut self, prop: &AssignPatProp) {
            self.0.push(prop.key.to_id());
        }
        fn visit_member_expr(&mut self, _: &MemberExpr) {}
        fn visit_expr(&mut self, _: &Expr) {}
    }
    let mut collector = Collector(Vec::new());
    target.visit_with(&mut collector);
    collector.0
}
