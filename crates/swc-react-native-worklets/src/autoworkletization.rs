// Port of plugin-oxc/src/autoworkletization.rs.
use rustc_hash::FxHashSet;
use swc_common::Span;
use swc_ecma_ast::*;
use swc_ecma_visit::{Visit, VisitMut, VisitMutWith, VisitWith};

use crate::ast::member_property;
use crate::directives::{add_worklet_directive, ensure_block_body};
use crate::gestures::{contains_gesture_obj, is_layout_anim_chain};
use crate::hooks::{
    function_hooks, is_object_hook, GESTURE_BUILDER_METHODS, LAYOUT_ANIM_CALLBACKS,
};
use crate::referenced_worklets::{Definitions, Property, Shape};

#[derive(Clone, Copy)]
struct Kinds {
    function: bool,
    object: bool,
}
const FUNCTION: Kinds = Kinds {
    function: true,
    object: false,
};
const BOTH: Kinds = Kinds {
    function: true,
    object: true,
};

pub fn add_directives_to_known_callbacks<N>(program: &mut N) -> Option<String>
where
    N: VisitWith<Definitions> + for<'a> VisitWith<Callbacks<'a>> + VisitMutWith<DirectiveInjector>,
{
    let mut definitions = Definitions::default();
    program.visit_with(&mut definitions);
    let mut callbacks = Callbacks {
        definitions: &definitions,
        sites: FxHashSet::default(),
        error: None,
    };
    program.visit_with(&mut callbacks);
    program.visit_mut_with(&mut DirectiveInjector {
        sites: callbacks.sites,
    });
    callbacks.error
}

pub(crate) struct Callbacks<'a> {
    definitions: &'a Definitions,
    sites: FxHashSet<Span>,
    error: Option<String>,
}

impl Callbacks<'_> {
    fn collect(
        &mut self,
        shape: &Shape,
        kinds: Kinds,
        follow_bindings: bool,
        seen: &mut FxHashSet<Id>,
    ) {
        match shape {
            Shape::Function(span) if kinds.function => {
                self.sites.insert(*span);
            }
            Shape::Object(properties) if kinds.object => {
                for property in properties {
                    match property {
                        Property::Method(span) => {
                            self.sites.insert(*span);
                        }
                        Property::Value(value) => self.collect(value, FUNCTION, true, seen),
                        Property::Unsupported(kind) => {
                            self.error.get_or_insert_with(|| format!("'{kind}' as to-be workletized argument is not supported for object hooks."));
                        }
                    }
                }
            }
            Shape::Alias(id) if follow_bindings => {
                if !seen.insert(id.clone()) || self.definitions.hand_written.contains(id) {
                    return;
                }
                if kinds.function {
                    if let Some(span) = self.definitions.function_declarations.get(id) {
                        self.sites.insert(*span);
                        return;
                    }
                }
                if self.definitions.rebound.contains(id) {
                    if let Some(shape) = self.definitions.assignments.get(id).and_then(|shapes| {
                        shapes.iter().rev().find(|shape| has_shape(shape, kinds))
                    }) {
                        self.collect(shape, kinds, false, seen);
                    }
                } else if let Some(shape) = self.definitions.declarators.get(id) {
                    self.collect(shape, kinds, true, seen);
                }
            }
            _ => {}
        }
    }
    fn collect_argument(&mut self, arg: &ExprOrSpread, kinds: Kinds, follow: bool) {
        if arg.spread.is_none() {
            if let Some(shape) = Definitions::describe(&arg.expr) {
                self.collect(&shape, kinds, follow, &mut FxHashSet::default());
            }
        }
    }
}

fn has_shape(shape: &Shape, kinds: Kinds) -> bool {
    match shape {
        Shape::Function(_) => kinds.function,
        Shape::Object(_) => kinds.object,
        Shape::Alias(_) => false,
    }
}

fn effective_callee(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(expr) => effective_callee(&expr.expr),
        Expr::Seq(sequence) => sequence
            .exprs
            .last()
            .map_or(expr, |expr| effective_callee(expr)),
        _ => expr,
    }
}

fn callee_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(id) => Some(&id.sym),
        Expr::Member(member) => member_property(member),
        _ => None,
    }
}

fn has_optional_chain(expr: &Expr) -> bool {
    match expr {
        Expr::OptChain(_) => true,
        Expr::Paren(expr) => has_optional_chain(&expr.expr),
        Expr::Member(member) => has_optional_chain(&member.obj),
        Expr::Call(call) => matches!(&call.callee, Callee::Expr(expr) if has_optional_chain(expr)),
        _ => false,
    }
}

impl Visit for Callbacks<'_> {
    fn visit_call_expr(&mut self, call: &CallExpr) {
        call.visit_children_with(self);
        let Callee::Expr(callee) = &call.callee else {
            return;
        };
        if has_optional_chain(callee) {
            return;
        }
        let callee = effective_callee(callee);
        if let Some(name) = callee_name(callee) {
            let function = function_hooks().iter().find(|(hook, _)| *hook == name);
            let object = is_object_hook(name);
            if function.is_some() || object {
                let indices = function.map_or(&[0][..], |(_, indices)| *indices);
                for arg in indices.iter().filter_map(|index| call.args.get(*index)) {
                    self.collect_argument(
                        arg,
                        Kinds {
                            function: function.is_some(),
                            object,
                        },
                        true,
                    );
                }
            }
        }
        if let Expr::Member(member) = callee {
            if member_property(member).is_some_and(|name| GESTURE_BUILDER_METHODS.contains(&name))
                && contains_gesture_obj(&member.obj)
            {
                for arg in &call.args {
                    self.collect_argument(arg, BOTH, true);
                }
            }
            if member_property(member).is_some_and(|name| LAYOUT_ANIM_CALLBACKS.contains(&name))
                && is_layout_anim_chain(&member.obj)
            {
                for arg in &call.args {
                    self.collect_argument(arg, FUNCTION, false);
                }
            }
        }
    }
}

pub(crate) struct DirectiveInjector {
    sites: FxHashSet<Span>,
}
impl VisitMut for DirectiveInjector {
    fn visit_mut_function(&mut self, function: &mut Function) {
        if self.sites.contains(&function.span) {
            if let Some(body) = &mut function.body {
                add_worklet_directive(body);
            }
        }
        function.visit_mut_children_with(self);
    }
    fn visit_mut_arrow_expr(&mut self, arrow: &mut ArrowExpr) {
        if self.sites.contains(&arrow.span) {
            add_worklet_directive(ensure_block_body(arrow));
        }
        arrow.visit_mut_children_with(self);
    }
}
