//! Inline-style warning. Rewrites every `sharedValue.value` read inside a
//! style object into a thunk that emits a dev warning before returning the
//! original value.
//!
//! Corresponds to `inlineStylesWarning.ts` in
//! react-native-reanimated/packages/react-native-worklets/plugin/src/.

use swc_common::DUMMY_SP;
use swc_ecma_ast::*;
use swc_ecma_utils::ExprFactory;

use crate::factory::{ident_expr, ident_name, str_lit};
use crate::visitor::prop_name_str;

pub(crate) fn warn_obj(obj: &mut ObjectLit) {
    for prop in &mut obj.props {
        if let PropOrSpread::Prop(p) = prop {
            if let Prop::KeyValue(kv) = p.as_mut() {
                let key = prop_name_str(&kv.key);
                if key == Some("transform") {
                    warn_transform(&mut kv.value);
                } else {
                    warn_value(&mut kv.value);
                }
            }
        }
    }
}

fn warn_transform(v: &mut Box<Expr>) {
    if let Expr::Array(arr) = v.as_mut() {
        for ExprOrSpread { expr, .. } in arr.elems.iter_mut().flatten() {
            if let Expr::Object(o) = expr.as_mut() {
                warn_obj(o);
            }
        }
    }
}

fn warn_value(v: &mut Box<Expr>) {
    if let Expr::Member(me) = v.as_ref() {
        if !me.prop.is_computed() {
            if let MemberProp::Ident(prop) = &me.prop {
                if prop.sym.as_ref() == "value" {
                    let orig = v.as_ref().clone();
                    **v = inline_style_warning(orig);
                }
            }
        }
    }
}

/// `(() => { console.warn(require("react-native-reanimated").getUseOfValueInStyleWarning()); return <orig>; })()`
fn inline_style_warning(orig: Expr) -> Expr {
    let require_reanimated =
        ident_expr("require").as_call(DUMMY_SP, vec![str_lit("react-native-reanimated").as_arg()]);
    let warning_msg = require_reanimated
        .make_member(ident_name("getUseOfValueInStyleWarning"))
        .as_call(DUMMY_SP, vec![]);
    let console_warn = ident_expr("console")
        .make_member(ident_name("warn"))
        .as_call(DUMMY_SP, vec![warning_msg.as_arg()]);

    let body = BlockStmt {
        span: DUMMY_SP,
        ctxt: Default::default(),
        stmts: vec![
            console_warn.into_stmt(),
            Stmt::Return(ReturnStmt {
                span: DUMMY_SP,
                arg: Some(Box::new(orig)),
            }),
        ],
    };
    let arrow = ArrowExpr {
        span: DUMMY_SP,
        ctxt: Default::default(),
        params: vec![],
        body: Box::new(BlockStmtOrExpr::BlockStmt(body)),
        is_async: false,
        is_generator: false,
        type_params: None,
        return_type: None,
    };
    Expr::Arrow(arrow).wrap_with_paren().as_iife().into()
}
