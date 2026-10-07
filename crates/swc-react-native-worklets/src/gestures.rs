//! Pure AST predicates for gesture-handler / layout-animation member chains
//! (e.g. `Gesture.Pan().onUpdate(cb)`, `FadeIn.duration(100).withCallback(cb)`).
//!
//! Port of plugin-oxc/src/gesture_handler_autoworkletization.rs and
//! layout_animation_autoworkletization.rs.

use swc_ecma_ast::*;

use crate::ast::member_property;
use crate::hooks::{GESTURE_OBJECTS, LAYOUT_ANIMATIONS, LAYOUT_ANIM_CHAINABLE};

pub(crate) fn contains_gesture_obj(obj: &Expr) -> bool {
    contains_gesture_obj_with_depth(obj, 64)
}

fn contains_gesture_obj_with_depth(obj: &Expr, depth: u32) -> bool {
    if depth == 0 {
        return false;
    }
    if is_gesture_obj(obj) {
        return true;
    }
    if let Expr::Call(call) = obj {
        if let Callee::Expr(ce) = &call.callee {
            if let Expr::Member(me) = ce.as_ref() {
                return contains_gesture_obj_with_depth(&me.obj, depth - 1);
            }
        }
    }
    false
}

pub(crate) fn is_gesture_obj(expr: &Expr) -> bool {
    if let Expr::Call(call) = expr {
        if let Callee::Expr(ce) = &call.callee {
            if let Expr::Member(me) = ce.as_ref() {
                if let Expr::Ident(obj) = me.obj.as_ref() {
                    if obj.sym.as_ref() == "Gesture" {
                        return member_property(me)
                            .is_some_and(|name| GESTURE_OBJECTS.contains(&name));
                    }
                }
            }
        }
    }
    false
}

pub(crate) fn is_layout_anim_chain(expr: &Expr) -> bool {
    is_layout_anim_chain_with_depth(expr, 64)
}

fn is_layout_anim_chain_with_depth(expr: &Expr, depth: u32) -> bool {
    if depth == 0 {
        return false;
    }
    match expr {
        Expr::Ident(id) => LAYOUT_ANIMATIONS.contains(&id.sym.as_ref()),
        Expr::New(new_expr) => {
            if let Expr::Ident(id) = new_expr.callee.as_ref() {
                LAYOUT_ANIMATIONS.contains(&id.sym.as_ref())
            } else {
                false
            }
        }
        Expr::Call(call) => {
            if let Callee::Expr(ce) = &call.callee {
                if let Expr::Member(me) = ce.as_ref() {
                    if member_property(me).is_some_and(|name| LAYOUT_ANIM_CHAINABLE.contains(&name))
                    {
                        return is_layout_anim_chain_with_depth(&me.obj, depth - 1);
                    }
                }
            }
            false
        }
        _ => false,
    }
}
