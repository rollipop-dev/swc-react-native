use swc_ecma_ast::{
    Class, Function, FunctionBody, GetterProp, MethodProp, PropName, SetterProp, SuperPropExpr,
};
use swc_ecma_hooks::VisitMutHook;

use crate::visitors::super_in_object_accessor::make_accessor_key_computed_string;

#[derive(Default)]
pub(crate) struct SuperInObjectAccessorHook {
    direct_super: Option<bool>,
    scopes: Vec<Option<bool>>,
    accessors: Vec<Accessor>,
}

struct Accessor {
    body_address: Option<usize>,
    found: bool,
}

impl SuperInObjectAccessorHook {
    fn enter_scope(&mut self) {
        self.scopes.push(self.direct_super.take());
    }

    fn exit_scope(&mut self) {
        self.direct_super = self.scopes.pop().expect("balanced accessor scope hooks");
    }

    fn enter_accessor(&mut self, body: Option<&FunctionBody>) {
        self.enter_scope();
        self.accessors.push(Accessor {
            body_address: body.map(|body| std::ptr::from_ref(body).addr()),
            found: false,
        });
    }

    fn exit_accessor(&mut self, key: &mut PropName) {
        if self.accessors.pop().expect("balanced accessor hooks").found {
            make_accessor_key_computed_string(key);
        }
        self.exit_scope();
    }

    fn is_accessor_body(&self, body: &FunctionBody) -> bool {
        self.accessors
            .last()
            .is_some_and(|accessor| accessor.body_address == Some(std::ptr::from_ref(body).addr()))
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

impl<C> VisitMutHook<C> for SuperInObjectAccessorHook {
    fn enter_getter_prop(&mut self, prop: &mut GetterProp, _ctx: &mut C) {
        self.enter_accessor(prop.function.body.as_ref());
    }

    fn exit_getter_prop(&mut self, prop: &mut GetterProp, _ctx: &mut C) {
        self.exit_accessor(&mut prop.key);
    }

    fn enter_setter_prop(&mut self, prop: &mut SetterProp, _ctx: &mut C) {
        self.enter_accessor(prop.function.body.as_ref());
    }

    fn exit_setter_prop(&mut self, prop: &mut SetterProp, _ctx: &mut C) {
        self.exit_accessor(&mut prop.key);
    }

    fn enter_function_body(&mut self, body: &mut FunctionBody, _ctx: &mut C) {
        if self.is_accessor_body(body) {
            self.direct_super = Some(false);
        }
    }

    fn exit_function_body(&mut self, body: &mut FunctionBody, _ctx: &mut C) {
        if self.is_accessor_body(body) {
            self.accessors.last_mut().expect("accessor body").found =
                self.direct_super.take().unwrap_or(false);
        }
    }

    fn enter_super_prop_expr(&mut self, _expr: &mut SuperPropExpr, _ctx: &mut C) {
        if let Some(found) = &mut self.direct_super {
            *found = true;
        }
    }

    // Arrows inherit the accessor's super binding; other functions/classes do not.
    scope_hooks!(enter_function, exit_function, Function);
    scope_hooks!(enter_method_prop, exit_method_prop, MethodProp);
    scope_hooks!(enter_class, exit_class, Class);
}
