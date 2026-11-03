//! Port of Reanimated 3.19.5 plugin/src/contextObject.ts.

use crate::ast::ident_name;
use swc_common::DUMMY_SP;
use swc_ecma_ast::*;

pub(super) const CONTEXT_MARKER: &str = "__workletContextObject";

pub(crate) fn is_context_object(object: &ObjectLit) -> bool {
    object.props.iter().any(|property| {
        matches!(property,
        PropOrSpread::Prop(property) if matches!(&**property,
            Prop::KeyValue(property) if matches!(&property.key,
                PropName::Ident(name) if name.sym == CONTEXT_MARKER)))
    })
}

// Port of plugin/src/contextObject.ts::processIfWorkletContextObject.
pub(crate) fn process_context_object(object: &mut ObjectLit) {
    if !is_context_object(object) {
        return;
    }
    object.props.retain(|property| {
        !matches!(property,
        PropOrSpread::Prop(property) if matches!(&**property,
            Prop::KeyValue(property) if matches!(&property.key,
                PropName::Ident(name) if name.sym == CONTEXT_MARKER)))
    });
    let mut body = FunctionBody {
        span: DUMMY_SP,
        stmts: vec![Stmt::Return(ReturnStmt {
            span: DUMMY_SP,
            arg: Some(Box::new(Expr::Object(object.clone()))),
        })],
    };
    crate::directives::add_worklet_directive(&mut body);
    object
        .props
        .push(PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
            key: PropName::Ident(ident_name("__workletContextObjectFactory")),
            value: Box::new(Expr::Fn(FnExpr {
                ident: None,
                function: Box::new(Function {
                    body: Some(body),
                    ..Default::default()
                }),
            })),
        }))));
}
