// Port of plugin-oxc/src/closure.rs. SWC resolver IDs replace OXC symbol IDs.
// Legacy Eval retains the upstream globals policy for unbound references.
use indexmap::IndexMap;
use rustc_hash::FxHashSet;
use swc_atoms::Atom;
use swc_ecma_ast::*;
use swc_ecma_visit::{Visit, VisitWith};

pub fn get_closure(
    params: &[Param],
    body: &FunctionBody,
    self_id: Option<&Ident>,
    bindings: &FxHashSet<Id>,
    globals: &FxHashSet<Atom>,
    strict_global: bool,
    bundle_mode: bool,
) -> Vec<Ident> {
    let mut bindings_collector = BindingCollector::default();
    body.visit_with(&mut bindings_collector);
    for param in params {
        param.visit_with(&mut bindings_collector);
    }
    let mut locals = bindings_collector.ids;
    if let Some(id) = self_id {
        locals.insert(id.to_id());
    }
    let mut collector = ReferenceCollector {
        locals,
        bindings,
        globals,
        strict_global,
        bundle_mode,
        references: IndexMap::new(),
        in_for_target: false,
    };
    for param in params {
        param.pat.visit_with(&mut collector);
    }
    body.visit_with(&mut collector);
    collector.references.into_values().collect()
}

#[derive(Default)]
pub struct BindingCollector {
    pub ids: FxHashSet<Id>,
}

impl Visit for BindingCollector {
    fn visit_binding_ident(&mut self, id: &BindingIdent) {
        self.ids.insert(id.to_id());
    }
    fn visit_fn_decl(&mut self, function: &FnDecl) {
        self.ids.insert(function.ident.to_id());
        function.function.visit_with(self);
    }
    fn visit_fn_expr(&mut self, function: &FnExpr) {
        if let Some(id) = &function.ident {
            self.ids.insert(id.to_id());
        }
        function.function.visit_with(self);
    }
    fn visit_class_decl(&mut self, class: &ClassDecl) {
        self.ids.insert(class.ident.to_id());
        class.class.visit_with(self);
    }
    fn visit_class_expr(&mut self, class: &ClassExpr) {
        if let Some(id) = &class.ident {
            self.ids.insert(id.to_id());
        }
        class.class.visit_with(self);
    }
    fn visit_import_decl(&mut self, import: &ImportDecl) {
        if !import.type_only {
            for specifier in &import.specifiers {
                if !matches!(specifier, ImportSpecifier::Named(named) if named.is_type_only) {
                    self.ids.insert(specifier.local().to_id());
                }
            }
        }
    }
    fn visit_assign_target(&mut self, _: &AssignTarget) {}
    fn visit_for_head(&mut self, head: &ForHead) {
        if !matches!(head, ForHead::Pat(_)) {
            head.visit_children_with(self);
        }
    }
    fn visit_ts_type(&mut self, _: &TsType) {}
    fn visit_ts_type_ann(&mut self, _: &TsTypeAnn) {}
}

struct ReferenceCollector<'a> {
    locals: FxHashSet<Id>,
    bindings: &'a FxHashSet<Id>,
    globals: &'a FxHashSet<Atom>,
    strict_global: bool,
    bundle_mode: bool,
    references: IndexMap<Atom, Ident>,
    in_for_target: bool,
}

impl Visit for ReferenceCollector<'_> {
    fn visit_ident(&mut self, id: &Ident) {
        if self.locals.contains(&id.to_id()) {
            return;
        }
        let bound = self.bindings.contains(&id.to_id());
        if !bound
            && !is_synthesized_init_data(&id.sym)
            && (self.bundle_mode || self.strict_global || self.globals.contains(&id.sym))
        {
            return;
        }
        self.references
            .entry(id.sym.clone())
            .or_insert_with(|| id.clone());
    }
    fn visit_binding_ident(&mut self, _: &BindingIdent) {}
    fn visit_fn_expr(&mut self, function: &FnExpr) {
        function.function.visit_with(self);
    }
    fn visit_fn_decl(&mut self, function: &FnDecl) {
        function.function.visit_with(self);
    }
    fn visit_class_decl(&mut self, class: &ClassDecl) {
        class.class.visit_with(self);
    }
    fn visit_class_expr(&mut self, class: &ClassExpr) {
        class.class.visit_with(self);
    }
    fn visit_labeled_stmt(&mut self, stmt: &LabeledStmt) {
        stmt.body.visit_with(self);
    }
    fn visit_break_stmt(&mut self, _: &BreakStmt) {}
    fn visit_continue_stmt(&mut self, _: &ContinueStmt) {}
    fn visit_assign_target(&mut self, target: &AssignTarget) {
        if !self.in_for_target
            && matches!(target, AssignTarget::Simple(SimpleAssignTarget::Ident(_)))
        {
            return;
        }
        target.visit_children_with(self);
    }
    fn visit_assign_pat_prop(&mut self, prop: &AssignPatProp) {
        if self.in_for_target {
            self.visit_ident(&prop.key.id);
        }
        prop.value.visit_with(self);
    }
    fn visit_simple_assign_target(&mut self, target: &SimpleAssignTarget) {
        if let SimpleAssignTarget::Ident(id) = target {
            if self.in_for_target {
                self.visit_ident(&id.id);
            }
        } else {
            target.visit_children_with(self);
        }
    }
    fn visit_for_in_stmt(&mut self, stmt: &ForInStmt) {
        self.in_for_target =
            matches!(stmt.left, ForHead::Pat(ref pat) if matches!(&**pat, Pat::Ident(_)));
        if self.in_for_target {
            if let ForHead::Pat(pat) = &stmt.left {
                if let Pat::Ident(id) = &**pat {
                    self.visit_ident(&id.id);
                }
            }
        } else {
            stmt.left.visit_with(self);
        }
        self.in_for_target = false;
        stmt.right.visit_with(self);
        stmt.body.visit_with(self);
    }
    fn visit_for_of_stmt(&mut self, stmt: &ForOfStmt) {
        self.in_for_target =
            matches!(stmt.left, ForHead::Pat(ref pat) if matches!(&**pat, Pat::Ident(_)));
        if self.in_for_target {
            if let ForHead::Pat(pat) = &stmt.left {
                if let Pat::Ident(id) = &**pat {
                    self.visit_ident(&id.id);
                }
            }
        } else {
            stmt.left.visit_with(self);
        }
        self.in_for_target = false;
        stmt.right.visit_with(self);
        stmt.body.visit_with(self);
    }
    fn visit_ts_type(&mut self, _: &TsType) {}
    fn visit_ts_type_ann(&mut self, _: &TsTypeAnn) {}
    fn visit_ts_interface_decl(&mut self, _: &TsInterfaceDecl) {}
    fn visit_ts_type_alias_decl(&mut self, _: &TsTypeAliasDecl) {}
    fn visit_jsx_element_name(&mut self, node: &JSXElementName) {
        if !self.bundle_mode {
            return;
        }
        match node {
            JSXElementName::Ident(id) => self.visit_ident(id),
            JSXElementName::JSXMemberExpr(member) => self.visit_jsx_member_expr(member),
            JSXElementName::JSXNamespacedName(_) => {}
        }
    }
    fn visit_jsx_member_expr(&mut self, node: &JSXMemberExpr) {
        if !self.bundle_mode {
            return;
        }
        match &node.obj {
            JSXObject::Ident(id) => self.visit_ident(id),
            JSXObject::JSXMemberExpr(member) => self.visit_jsx_member_expr(member),
        }
    }
    fn visit_jsx_attr_name(&mut self, _: &JSXAttrName) {}
}

fn is_synthesized_init_data(name: &str) -> bool {
    name.strip_prefix("_worklet_")
        .and_then(|s| s.strip_suffix("_init_data"))
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}
