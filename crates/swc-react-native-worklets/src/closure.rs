// Corresponds to `closure.ts` in
// react-native-reanimated/packages/react-native-worklets/plugin/src/.

use indexmap::{IndexMap, IndexSet};
use rustc_hash::FxHashSet;
use swc_atoms::Atom;
use swc_ecma_ast::*;
use swc_ecma_visit::{Visit, VisitWith};

/// Capture rules for an identifier referenced inside a worklet body.
///
/// 1. Locally declared bindings → not captured.
/// 2. `force_skip_capture` set → never captured, regardless of any
///    file-level binding (mirrors the upstream babel plugin's
///    `outsideBindingsToCaptureFromGlobalScope` /
///    `internalBindingsToCaptureFromGlobalScope` precedence, which is
///    checked before scope-binding resolution).
/// 3. Anything else with any module-level binding → always captured.
/// 4. `strict_global` → globals not captured.
/// 5. Otherwise captured unless in `globals`.
pub struct ClosureCtx<'a> {
    pub globals: &'a FxHashSet<Atom>,
    pub file_bindings: &'a FxHashSet<Atom>,
    /// Identifiers registered onto `globalThis` at worklet runtime
    /// (e.g. `ReanimatedError`, `WorkletsError`). Must take precedence
    /// over `file_bindings` because their JS-thread bindings should
    /// not shadow the UI-side global registration.
    pub force_skip_capture: &'a FxHashSet<Atom>,
    pub strict_global: bool,
}

/// Collect closure variables. The first reference's `SyntaxContext` is
/// preserved so SWC's later ESM→CJS pass can still resolve imported
/// bindings emitted into the factory's IIFE argument.
pub fn collect_closure_vars(
    params: &[Param],
    body: &BlockStmt,
    ctx: &ClosureCtx<'_>,
) -> Vec<Ident> {
    let mut declared = IndexSet::new();
    let mut referenced = IndexMap::new();

    for param in params {
        collect_pat_bindings(&param.pat, &mut declared);
    }

    let mut decl_collector = DeclCollector {
        declared: &mut declared,
        depth: 0,
    };
    body.visit_with(&mut decl_collector);

    let mut ref_collector = RefCollector {
        declared: &declared,
        ctx,
        referenced: &mut referenced,
    };
    body.visit_with(&mut ref_collector);

    referenced.into_values().collect()
}

/// Same but for ArrowFunctionExpression with either a block or expression body.
pub fn collect_closure_vars_arrow(
    params: &[Pat],
    body: &BlockStmtOrExpr,
    ctx: &ClosureCtx<'_>,
) -> Vec<Ident> {
    let mut declared = IndexSet::new();
    let mut referenced = IndexMap::new();

    for pat in params {
        collect_pat_bindings(pat, &mut declared);
    }

    match body {
        BlockStmtOrExpr::BlockStmt(block) => {
            let mut decl_collector = DeclCollector {
                declared: &mut declared,
                depth: 0,
            };
            block.visit_with(&mut decl_collector);

            let mut ref_collector = RefCollector {
                declared: &declared,
                ctx,
                referenced: &mut referenced,
            };
            block.visit_with(&mut ref_collector);
        }
        BlockStmtOrExpr::Expr(expr) => {
            let mut ref_collector = RefCollector {
                declared: &declared,
                ctx,
                referenced: &mut referenced,
            };
            expr.visit_with(&mut ref_collector);
        }
    }

    referenced.into_values().collect()
}

pub(crate) fn collect_pat_bindings(pat: &Pat, out: &mut IndexSet<Atom>) {
    match pat {
        Pat::Ident(id) => {
            out.insert(id.id.sym.clone());
        }
        Pat::Array(arr) => {
            for elem in arr.elems.iter().flatten() {
                collect_pat_bindings(elem, out);
            }
        }
        Pat::Object(obj) => {
            for prop in &obj.props {
                match prop {
                    ObjectPatProp::KeyValue(kv) => collect_pat_bindings(&kv.value, out),
                    ObjectPatProp::Assign(a) => {
                        out.insert(a.key.sym.clone());
                    }
                    ObjectPatProp::Rest(r) => collect_pat_bindings(&r.arg, out),
                }
            }
        }
        Pat::Rest(r) => collect_pat_bindings(&r.arg, out),
        Pat::Assign(a) => collect_pat_bindings(&a.left, out),
        Pat::Expr(_) | Pat::Invalid(_) => {}
    }
}

/// Collects all declaration names within the target function scope.
/// Stops recursing into nested functions (they have their own scopes).
pub(crate) struct DeclCollector<'a> {
    pub(crate) declared: &'a mut IndexSet<Atom>,
    pub(crate) depth: usize,
}

impl Visit for DeclCollector<'_> {
    fn visit_var_declarator(&mut self, node: &VarDeclarator) {
        collect_pat_bindings(&node.name, self.declared);
        node.init.visit_with(self);
    }

    fn visit_fn_decl(&mut self, node: &FnDecl) {
        if self.depth == 0 {
            self.declared.insert(node.ident.sym.clone());
        }
    }

    fn visit_function(&mut self, node: &Function) {
        if self.depth > 0 {
            return;
        }
        let old = self.depth;
        self.depth += 1;
        let _ = node;
        self.depth = old;
    }

    fn visit_arrow_expr(&mut self, _: &ArrowExpr) {}

    fn visit_class_decl(&mut self, node: &ClassDecl) {
        if self.depth == 0 {
            self.declared.insert(node.ident.sym.clone());
        }
    }

    fn visit_catch_clause(&mut self, node: &CatchClause) {
        if let Some(param) = &node.param {
            collect_pat_bindings(param, self.declared);
        }
        node.body.visit_with(self);
    }

    fn visit_for_in_stmt(&mut self, node: &ForInStmt) {
        if let ForHead::VarDecl(decl) = &node.left {
            for d in &decl.decls {
                collect_pat_bindings(&d.name, self.declared);
            }
        }
        node.body.visit_with(self);
    }

    fn visit_for_of_stmt(&mut self, node: &ForOfStmt) {
        if let ForHead::VarDecl(decl) = &node.left {
            for d in &decl.decls {
                collect_pat_bindings(&d.name, self.declared);
            }
        }
        node.body.visit_with(self);
    }
}

/// Collects identifier references that should be captured in the closure.
struct RefCollector<'a> {
    declared: &'a IndexSet<Atom>,
    ctx: &'a ClosureCtx<'a>,
    referenced: &'a mut IndexMap<Atom, Ident>,
}

impl<'a> RefCollector<'a> {
    fn should_capture(&self, name: &Atom) -> bool {
        if self.declared.contains(name) {
            return false;
        }
        if name.as_ref() == "undefined" || name.as_ref() == "arguments" {
            return false;
        }
        // `force_skip_capture` overrides `file_bindings`: these
        // identifiers are registered onto `globalThis` at worklet
        // runtime by the worklets package itself, so they must resolve
        // through the global scope even when an import binding shadows
        // them on the JS thread.
        if self.ctx.force_skip_capture.contains(name) {
            return false;
        }
        // A file-level binding shadows any global of the same name, so we must
        // capture it regardless of the globals list or strict mode.
        if self.ctx.file_bindings.contains(name) {
            return true;
        }
        if self.ctx.strict_global {
            return false;
        }
        !self.ctx.globals.contains(name)
    }

    fn maybe_insert(&mut self, ident: &Ident) {
        if !self.should_capture(&ident.sym) {
            return;
        }
        self.referenced
            .entry(ident.sym.clone())
            .or_insert_with(|| ident.clone());
    }

    /// Recurse into a `Function` (class method / private method body) with
    /// its own scope. Shared between class methods and getters where the
    /// inner-scope dance is identical to `visit_function`'s top-level
    /// handler.
    fn visit_function_in_new_scope(&mut self, function: &Function) {
        let mut inner_declared = self.declared.clone();
        for param in &function.params {
            collect_pat_bindings(&param.pat, &mut inner_declared);
        }
        if let Some(body) = &function.body {
            let mut inner_decl = DeclCollector {
                declared: &mut inner_declared,
                depth: 0,
            };
            body.visit_with(&mut inner_decl);
            let mut inner_ref = RefCollector {
                declared: &inner_declared,
                ctx: self.ctx,
                referenced: self.referenced,
            };
            body.visit_with(&mut inner_ref);
        }
    }
}

impl Visit for RefCollector<'_> {
    fn visit_ident(&mut self, node: &Ident) {
        self.maybe_insert(node);
    }

    fn visit_member_expr(&mut self, node: &MemberExpr) {
        node.obj.visit_with(self);
        if let MemberProp::Computed(c) = &node.prop {
            c.expr.visit_with(self);
        }
    }

    fn visit_object_lit(&mut self, node: &ObjectLit) {
        for prop in &node.props {
            match prop {
                PropOrSpread::Prop(p) => match p.as_ref() {
                    Prop::Shorthand(id) => {
                        self.maybe_insert(id);
                    }
                    Prop::KeyValue(kv) => {
                        if let PropName::Computed(c) = &kv.key {
                            c.expr.visit_with(self);
                        }
                        kv.value.visit_with(self);
                    }
                    Prop::Method(m) => {
                        if let PropName::Computed(c) = &m.key {
                            c.expr.visit_with(self);
                        }
                        // Recurse into the method body with a fresh inner scope
                        // so that free identifiers referenced inside it are
                        // captured by the enclosing worklet's closure.
                        m.function.visit_with(self);
                    }
                    Prop::Getter(g) => {
                        if let PropName::Computed(c) = &g.key {
                            c.expr.visit_with(self);
                        }
                        if let Some(body) = &g.body {
                            let inner_declared = self.declared.clone();
                            let mut inner_ref = RefCollector {
                                declared: &inner_declared,
                                ctx: self.ctx,
                                referenced: self.referenced,
                            };
                            body.visit_with(&mut inner_ref);
                        }
                    }
                    Prop::Setter(s) => {
                        if let PropName::Computed(c) = &s.key {
                            c.expr.visit_with(self);
                        }
                        if let Some(body) = &s.body {
                            let mut inner_declared = self.declared.clone();
                            collect_pat_bindings(&s.param, &mut inner_declared);
                            let mut inner_ref = RefCollector {
                                declared: &inner_declared,
                                ctx: self.ctx,
                                referenced: self.referenced,
                            };
                            body.visit_with(&mut inner_ref);
                        }
                    }
                    Prop::Assign(a) => {
                        a.value.visit_with(self);
                    }
                },
                PropOrSpread::Spread(s) => {
                    s.expr.visit_with(self);
                }
            }
        }
    }

    fn visit_function(&mut self, node: &Function) {
        // Nested function: build the child scope and recurse.
        let mut inner_declared = self.declared.clone();
        for param in &node.params {
            collect_pat_bindings(&param.pat, &mut inner_declared);
        }
        if let Some(body) = &node.body {
            let mut inner_decl = DeclCollector {
                declared: &mut inner_declared,
                depth: 0,
            };
            body.visit_with(&mut inner_decl);
        }
        let mut inner_ref = RefCollector {
            declared: &inner_declared,
            ctx: self.ctx,
            referenced: self.referenced,
        };
        if let Some(body) = &node.body {
            body.visit_with(&mut inner_ref);
        }
    }

    fn visit_fn_expr(&mut self, node: &FnExpr) {
        // A named function expression binds its own name inside its body
        // but NOT in the enclosing scope. The default `Visit` impl would
        // visit `node.ident` as a reference via `visit_ident`, which would
        // spuriously capture the function's own name as a closure var.
        // Instead, treat the name as declared for the body and skip it at
        // the outer scope. Without this, wrapping an inner worklet in a
        // `function factoryName(param) {...}({...})` IIFE leaks
        // `factoryName` into the enclosing worklet's closure destructuring
        // — producing a `ReferenceError: Property 'factoryName' doesn't
        // exist` at runtime when the outer worklet is materialized.
        let mut inner_declared = self.declared.clone();
        if let Some(ident) = &node.ident {
            inner_declared.insert(ident.sym.clone());
        }
        for param in &node.function.params {
            collect_pat_bindings(&param.pat, &mut inner_declared);
        }
        if let Some(body) = &node.function.body {
            let mut inner_decl = DeclCollector {
                declared: &mut inner_declared,
                depth: 0,
            };
            body.visit_with(&mut inner_decl);
        }
        let mut inner_ref = RefCollector {
            declared: &inner_declared,
            ctx: self.ctx,
            referenced: self.referenced,
        };
        if let Some(body) = &node.function.body {
            body.visit_with(&mut inner_ref);
        }
    }

    fn visit_arrow_expr(&mut self, node: &ArrowExpr) {
        let mut inner_declared = self.declared.clone();
        for pat in &node.params {
            collect_pat_bindings(pat, &mut inner_declared);
        }
        match &*node.body {
            BlockStmtOrExpr::BlockStmt(block) => {
                let mut inner_decl = DeclCollector {
                    declared: &mut inner_declared,
                    depth: 0,
                };
                block.visit_with(&mut inner_decl);
                let mut inner_ref = RefCollector {
                    declared: &inner_declared,
                    ctx: self.ctx,
                    referenced: self.referenced,
                };
                block.visit_with(&mut inner_ref);
            }
            BlockStmtOrExpr::Expr(expr) => {
                let mut inner_ref = RefCollector {
                    declared: &inner_declared,
                    ctx: self.ctx,
                    referenced: self.referenced,
                };
                expr.visit_with(&mut inner_ref);
            }
        }
    }

    // Class members each open their own scope: constructor and method
    // parameters bind locals that must shadow the enclosing worklet's
    // free-variable analysis. Without per-member scope handling the
    // default `Visit` recursion treats every parameter ident as a
    // reference (because `visit_ident` is invoked on binding sites
    // too), and every body identifier referring to a constructor param
    // is misclassified as a free variable — leaking constructor
    // parameters into the outer worklet's `__closure`.
    //
    // Corresponds to babel's per-scope `ReferencedIdentifier` traversal
    // — the upstream plugin relies on babel's binding tracker instead
    // of replicating SWC's per-visitor scoping.
    fn visit_class(&mut self, node: &Class) {
        // `super_class`, decorators, type params, and computed keys
        // evaluate in the class's enclosing scope; collect refs there.
        if let Some(super_class) = &node.super_class {
            super_class.visit_with(self);
        }
        for decorator in &node.decorators {
            decorator.visit_with(self);
        }
        for member in &node.body {
            match member {
                ClassMember::Constructor(ctor) => {
                    if let PropName::Computed(c) = &ctor.key {
                        c.expr.visit_with(self);
                    }
                    let mut inner_declared = self.declared.clone();
                    for param in &ctor.params {
                        match param {
                            ParamOrTsParamProp::Param(p) => {
                                collect_pat_bindings(&p.pat, &mut inner_declared);
                            }
                            ParamOrTsParamProp::TsParamProp(tp) => match &tp.param {
                                TsParamPropParam::Ident(id) => {
                                    inner_declared.insert(id.id.sym.clone());
                                }
                                TsParamPropParam::Assign(a) => {
                                    collect_pat_bindings(&a.left, &mut inner_declared);
                                }
                            },
                        }
                    }
                    if let Some(body) = &ctor.body {
                        let mut inner_decl = DeclCollector {
                            declared: &mut inner_declared,
                            depth: 0,
                        };
                        body.visit_with(&mut inner_decl);
                        let mut inner_ref = RefCollector {
                            declared: &inner_declared,
                            ctx: self.ctx,
                            referenced: self.referenced,
                        };
                        body.visit_with(&mut inner_ref);
                    }
                }
                ClassMember::Method(m) => {
                    if let PropName::Computed(c) = &m.key {
                        c.expr.visit_with(self);
                    }
                    self.visit_function_in_new_scope(&m.function);
                }
                ClassMember::PrivateMethod(m) => {
                    self.visit_function_in_new_scope(&m.function);
                }
                ClassMember::ClassProp(p) => {
                    if let PropName::Computed(c) = &p.key {
                        c.expr.visit_with(self);
                    }
                    // Class-property initializers run in the constructor's
                    // scope, but `this`-binding aside they're just
                    // expressions evaluated when an instance is built;
                    // resolve refs against the outer scope.
                    if let Some(value) = &p.value {
                        value.visit_with(self);
                    }
                }
                ClassMember::PrivateProp(p) => {
                    if let Some(value) = &p.value {
                        value.visit_with(self);
                    }
                }
                ClassMember::StaticBlock(b) => {
                    // Static blocks share `this`/`super` with the class
                    // but do not see constructor/method params — handle
                    // them as a fresh inner scope keyed off `declared`.
                    let mut inner_declared = self.declared.clone();
                    let mut inner_decl = DeclCollector {
                        declared: &mut inner_declared,
                        depth: 0,
                    };
                    b.body.visit_with(&mut inner_decl);
                    let mut inner_ref = RefCollector {
                        declared: &inner_declared,
                        ctx: self.ctx,
                        referenced: self.referenced,
                    };
                    b.body.visit_with(&mut inner_ref);
                }
                ClassMember::TsIndexSignature(_)
                | ClassMember::AutoAccessor(_)
                | ClassMember::Empty(_) => {}
            }
        }
    }

    fn visit_ts_type(&mut self, _: &TsType) {}
    fn visit_ts_type_ann(&mut self, _: &TsTypeAnn) {}
    fn visit_ts_type_param_decl(&mut self, _: &TsTypeParamDecl) {}
    fn visit_ts_type_param_instantiation(&mut self, _: &TsTypeParamInstantiation) {}

    fn visit_key_value_prop(&mut self, node: &KeyValueProp) {
        if let PropName::Computed(c) = &node.key {
            c.expr.visit_with(self);
        }
        node.value.visit_with(self);
    }

    // JSX-position identifiers (tag names like `<Foo />`, member chains like
    // `<Foo.Bar />`, attribute names like `onPress=`) are skipped — they do
    // not need to be captured into the worklet closure. Identifiers inside
    // JSX expression containers (`{expr}`) are still visited via the default
    // walk on `JSXExprContainer.expr`. Mirrors the upstream babel plugin's
    // `idPath.isJSXIdentifier() ⇒ return` in `closure.ts`.
    fn visit_jsx_element_name(&mut self, _: &JSXElementName) {}
    fn visit_jsx_closing_element(&mut self, _: &JSXClosingElement) {}
    fn visit_jsx_attr_name(&mut self, _: &JSXAttrName) {}
}

/// Pre-scan a module/script and collect *all* identifier declarations across
/// every scope. Used by `ClosureCtx::file_bindings` to decide whether a
/// referenced identifier has any binding in the file (in which case it must
/// be captured, as it shadows any global of the same name).
pub fn collect_file_bindings_module(module: &Module) -> FxHashSet<Atom> {
    let mut out = FxHashSet::default();
    let mut v = FileBindingCollector { out: &mut out };
    module.visit_with(&mut v);
    out
}

pub fn collect_file_bindings_script(script: &Script) -> FxHashSet<Atom> {
    let mut out = FxHashSet::default();
    let mut v = FileBindingCollector { out: &mut out };
    script.visit_with(&mut v);
    out
}

struct FileBindingCollector<'a> {
    out: &'a mut FxHashSet<Atom>,
}

impl<'a> FileBindingCollector<'a> {
    fn push_pat(&mut self, pat: &Pat) {
        // Walk the pattern directly into our set instead of going through a
        // throwaway IndexSet — file bindings don't need ordering.
        fn walk(pat: &Pat, out: &mut FxHashSet<Atom>) {
            match pat {
                Pat::Ident(id) => {
                    out.insert(id.id.sym.clone());
                }
                Pat::Array(arr) => {
                    for elem in arr.elems.iter().flatten() {
                        walk(elem, out);
                    }
                }
                Pat::Object(obj) => {
                    for prop in &obj.props {
                        match prop {
                            ObjectPatProp::KeyValue(kv) => walk(&kv.value, out),
                            ObjectPatProp::Assign(a) => {
                                out.insert(a.key.sym.clone());
                            }
                            ObjectPatProp::Rest(r) => walk(&r.arg, out),
                        }
                    }
                }
                Pat::Rest(r) => walk(&r.arg, out),
                Pat::Assign(a) => walk(&a.left, out),
                Pat::Expr(_) | Pat::Invalid(_) => {}
            }
        }
        walk(pat, self.out);
    }
}

impl Visit for FileBindingCollector<'_> {
    fn visit_var_declarator(&mut self, n: &VarDeclarator) {
        self.push_pat(&n.name);
        n.init.visit_with(self);
    }
    fn visit_fn_decl(&mut self, n: &FnDecl) {
        self.out.insert(n.ident.sym.clone());
        n.function.visit_with(self);
    }
    fn visit_class_decl(&mut self, n: &ClassDecl) {
        self.out.insert(n.ident.sym.clone());
        n.class.visit_with(self);
    }
    fn visit_function(&mut self, n: &Function) {
        for p in &n.params {
            self.push_pat(&p.pat);
        }
        n.body.visit_with(self);
    }
    fn visit_arrow_expr(&mut self, n: &ArrowExpr) {
        for p in &n.params {
            self.push_pat(p);
        }
        n.body.visit_with(self);
    }
    fn visit_catch_clause(&mut self, n: &CatchClause) {
        if let Some(p) = &n.param {
            self.push_pat(p);
        }
        n.body.visit_with(self);
    }
    fn visit_for_in_stmt(&mut self, n: &ForInStmt) {
        if let ForHead::VarDecl(d) = &n.left {
            for dec in &d.decls {
                self.push_pat(&dec.name);
            }
        }
        n.right.visit_with(self);
        n.body.visit_with(self);
    }
    fn visit_for_of_stmt(&mut self, n: &ForOfStmt) {
        if let ForHead::VarDecl(d) = &n.left {
            for dec in &d.decls {
                self.push_pat(&dec.name);
            }
        }
        n.right.visit_with(self);
        n.body.visit_with(self);
    }
    fn visit_import_specifier(&mut self, n: &ImportSpecifier) {
        match n {
            ImportSpecifier::Named(s) => {
                self.out.insert(s.local.sym.clone());
            }
            ImportSpecifier::Default(s) => {
                self.out.insert(s.local.sym.clone());
            }
            ImportSpecifier::Namespace(s) => {
                self.out.insert(s.local.sym.clone());
            }
        }
    }
}
