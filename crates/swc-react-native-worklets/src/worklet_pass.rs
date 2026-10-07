// SWC adapter for plugin-oxc/src/plugin.rs, worklet_pass.rs and class_method.rs.
use crate::ast::const_decl;
use crate::autoworkletization::add_directives_to_known_callbacks;
use crate::directives::{has_directive, string_statement};
use crate::globals::DEFAULT_GLOBALS;
use crate::imports::{build_imports_index, resolve_package_dir, ImportInfo};
use crate::options::WorkletsOptions;
use crate::referenced_worklets::Definitions;
use crate::worklet_factory::{make_worklet_factory, WorkletInput};
use crate::worklet_file::{write_worklet_file, EmittedFile};
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use swc_atoms::Atom;
use swc_common::{errors::HANDLER, sync::Lrc, Mark, SourceMap, Span, SyntaxContext, DUMMY_SP};
use swc_ecma_ast::*;
use swc_ecma_transforms_base::resolver;
use swc_ecma_utils::ExprFactory;
use swc_ecma_visit::{VisitMut, VisitMutWith};

pub struct WorkletsVisitor {
    pub options: WorkletsOptions,
    pub filename: String,
    pub worklet_number: u32,
    pub is_release: bool,
    pub skip_file: bool,
    pub globals: FxHashSet<Atom>,
    pub file_bindings: FxHashSet<Atom>,
    pub source_map: Option<Lrc<SourceMap>>,
    pub(crate) bindings: FxHashSet<Id>,
    pub(crate) imports: FxHashMap<Id, ImportInfo>,
    pub(crate) rebound: FxHashSet<Id>,
    pub(crate) forwardable_modules: Vec<String>,
    pub(crate) forwardable_paths: Vec<String>,
    pub(crate) worklets_package_dir: Option<PathBuf>,
    pub(crate) emitted_files: Vec<EmittedFile>,
    error: Option<String>,
    pending_prepends: Vec<Stmt>,
    function_prepends: Vec<Vec<Stmt>>,
    parent_is_scopable: bool,
    used_names: FxHashSet<Atom>,
}

impl WorkletsVisitor {
    pub fn new(options: WorkletsOptions) -> Self {
        let filename = options
            .filename
            .as_deref()
            .unwrap_or_default()
            .replace('\\', "/");
        let globals = DEFAULT_GLOBALS
            .iter()
            .copied()
            .chain(options.globals.iter().map(String::as_str))
            .map(Atom::from)
            .collect();
        let mut forwardable_modules = vec![
            "react-native-worklets".to_string(),
            "react-native/Libraries/Core/setUpXHR".to_string(),
        ];
        forwardable_modules.extend(options.import_forwarding.module_names.clone());
        let mut forwardable_paths = vec!["react-native-worklets".to_string()];
        forwardable_paths.extend(options.import_forwarding.relative_paths.clone());
        let worklets_package_dir = if options.bundle_mode {
            resolve_package_dir(&filename, options.cwd.as_deref())
        } else {
            None
        };
        Self {
            is_release: options.is_release,
            skip_file: filename.contains("react-native-worklets/.worklets"),
            options,
            filename,
            worklet_number: 1,
            globals,
            file_bindings: FxHashSet::default(),
            source_map: None,
            bindings: FxHashSet::default(),
            imports: FxHashMap::default(),
            rebound: FxHashSet::default(),
            forwardable_modules,
            forwardable_paths,
            worklets_package_dir,
            emitted_files: vec![],
            error: None,
            pending_prepends: vec![],
            function_prepends: vec![],
            parent_is_scopable: true,
            used_names: FxHashSet::default(),
        }
    }
    pub fn with_source_map(mut self, cm: Lrc<SourceMap>) -> Self {
        self.source_map = Some(cm);
        self
    }
    /// Override package discovery for bundlers using virtual files or custom resolution.
    pub fn with_worklets_package_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.worklets_package_dir = Some(path.into());
        self
    }
    /// Generated Bundle Mode modules. The visitor also writes these to the package's `.worklets` directory.
    pub fn emitted_files(&self) -> &[EmittedFile] {
        &self.emitted_files
    }
    pub fn into_result(self) -> Result<Vec<EmittedFile>, String> {
        self.error.map_or(Ok(self.emitted_files), Err)
    }

    pub(crate) fn plugin_version(&self) -> &str {
        if self.options.plugin_version.is_empty() {
            "unknown"
        } else {
            &self.options.plugin_version
        }
    }
    pub(crate) fn location_path(&self) -> String {
        if !self.options.relative_source_location {
            return self.filename.clone();
        }
        let cwd = self
            .options
            .cwd
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok());
        cwd.and_then(|cwd| crate::imports::pathdiff(&cwd, Path::new(&self.filename)))
            .map_or_else(
                || self.filename.clone(),
                |relative| relative.to_string_lossy().replace('\\', "/"),
            )
    }
    pub(crate) fn fresh_init_id(&mut self, hash: u64) -> Ident {
        let base = format!("_worklet_{hash}_init_data");
        let mut name = base.clone();
        let mut number = 2;
        while self.used_names.contains(&Atom::from(name.as_str())) {
            name = format!("{base}{number}");
            number += 1;
        }
        self.used_names.insert(name.clone().into());
        let ident = Ident::new(
            name.into(),
            DUMMY_SP,
            SyntaxContext::empty().apply_mark(Mark::new()),
        );
        self.bindings.insert(ident.to_id());
        ident
    }
    pub(crate) fn hoist_init_data(
        &mut self,
        declaration: Stmt,
        limited: bool,
    ) -> Result<(), String> {
        if limited {
            self.function_prepends
                .last_mut()
                .ok_or_else(|| {
                    "limit-init-data-hoisting requires an enclosing function".to_string()
                })?
                .push(declaration);
        } else {
            self.pending_prepends.push(declaration);
        }
        Ok(())
    }
    fn report_error(&mut self, message: String, span: Span) {
        if self.error.is_some() {
            return;
        }
        if HANDLER.is_set() {
            HANDLER.with(|handler| {
                handler
                    .struct_span_err(span, &format!("[Worklets] {message}"))
                    .emit()
            });
        }
        self.error = Some(message);
    }
    fn finish_bundle_files(&mut self, span: Span) {
        if self.error.is_some() {
            return;
        }
        if let Some(package_dir) = &self.worklets_package_dir {
            for file in &self.emitted_files {
                if let Err(error) = write_worklet_file(package_dir, file) {
                    self.report_error(error, span);
                    break;
                }
            }
        }
    }
    fn transform_function(
        &mut self,
        function: &Function,
        name: Option<String>,
        self_id: Option<Ident>,
    ) -> Option<Expr> {
        let body = function.body.as_ref()?;
        if !has_directive(body, "worklet") || self.error.is_some() {
            return None;
        }
        match make_worklet_factory(
            WorkletInput {
                params: function.params.clone(),
                body: body.clone(),
                self_id,
                name,
                is_async: function.is_async,
                is_generator: function.is_generator,
            },
            self,
        ) {
            Ok(expr) => Some(expr),
            Err(error) => {
                self.report_error(error, function.span);
                None
            }
        }
    }
    fn function_body_prepends(&mut self, body: &mut FunctionBody) {
        let prepends = self
            .function_prepends
            .pop()
            .expect("function frame was pushed");
        let end = body
            .stmts
            .iter()
            .take_while(|stmt| string_statement(stmt).is_some())
            .count();
        body.stmts.splice(end..end, prepends);
    }
}

impl VisitMut for WorkletsVisitor {
    fn visit_mut_module(&mut self, module: &mut Module) {
        if self.skip_file {
            return;
        }
        if self.options.hermes_bytecode
            && !self.options.bundle_mode
            && self.is_release
            && !self.options.omit_native_only_data
        {
            self.report_error("react-native-worklets hermesBytecode is not supported by swc-react-native-worklets".to_string(), module.span);
            return;
        }
        let original = module.clone();
        crate::file_directive::process_module(module, !self.options.bundle_mode);
        if !self.options.bundle_mode && !self.options.disable_worklet_classes {
            crate::legacy_classes::process_module(module);
        }
        module.visit_mut_with(&mut resolver(Mark::new(), Mark::new(), true));
        let mut bindings = crate::closure::BindingCollector::default();
        swc_ecma_visit::VisitWith::visit_with(module, &mut bindings);
        self.bindings = bindings.ids;
        self.file_bindings = self.bindings.iter().map(|(name, _)| name.clone()).collect();
        self.used_names = self.file_bindings.clone();
        self.imports = build_imports_index(module);
        let mut definitions = Definitions::default();
        swc_ecma_visit::VisitWith::visit_with(module, &mut definitions);
        self.rebound = definitions.rebound;
        if let Some(error) = add_directives_to_known_callbacks(module) {
            self.report_error(error, module.span);
        }
        let mut out = Vec::new();
        for mut item in std::mem::take(&mut module.body) {
            self.parent_is_scopable = true;
            if let ModuleItem::Stmt(stmt) = &mut item {
                crate::file_directive::set_bundle_flag(
                    std::slice::from_mut(stmt),
                    &self.filename,
                    self.options.bundle_mode,
                );
            }
            item.visit_mut_with(self);
            out.extend(self.pending_prepends.drain(..).map(ModuleItem::Stmt));
            out.push(item);
        }
        module.body = out;
        self.finish_bundle_files(module.span);
        if self.error.is_some() {
            *module = original;
            self.emitted_files.clear();
        }
    }
    fn visit_mut_script(&mut self, script: &mut Script) {
        if self.skip_file {
            return;
        }
        if self.options.hermes_bytecode
            && !self.options.bundle_mode
            && self.is_release
            && !self.options.omit_native_only_data
        {
            self.report_error("react-native-worklets hermesBytecode is not supported by swc-react-native-worklets".to_string(), script.span);
            return;
        }
        let original = script.clone();
        crate::file_directive::process_script(script, !self.options.bundle_mode);
        if !self.options.bundle_mode && !self.options.disable_worklet_classes {
            crate::legacy_classes::process_script(script);
        }
        script.visit_mut_with(&mut resolver(Mark::new(), Mark::new(), true));
        let mut bindings = crate::closure::BindingCollector::default();
        swc_ecma_visit::VisitWith::visit_with(script, &mut bindings);
        self.bindings = bindings.ids;
        self.file_bindings = self.bindings.iter().map(|(name, _)| name.clone()).collect();
        self.used_names = self.file_bindings.clone();
        let mut definitions = Definitions::default();
        swc_ecma_visit::VisitWith::visit_with(script, &mut definitions);
        self.rebound = definitions.rebound;
        if let Some(error) = add_directives_to_known_callbacks(script) {
            self.report_error(error, script.span);
        }
        crate::file_directive::set_bundle_flag(
            &mut script.body,
            &self.filename,
            self.options.bundle_mode,
        );
        let mut out = Vec::new();
        for mut stmt in std::mem::take(&mut script.body) {
            self.parent_is_scopable = true;
            stmt.visit_mut_with(self);
            out.append(&mut self.pending_prepends);
            out.push(stmt);
        }
        script.body = out;
        self.finish_bundle_files(script.span);
        if self.error.is_some() {
            *script = original;
            self.emitted_files.clear();
        }
    }
    fn visit_mut_function(&mut self, function: &mut Function) {
        self.function_prepends.push(Vec::new());
        function.visit_mut_children_with(self);
        if let Some(body) = &mut function.body {
            self.function_body_prepends(body);
        } else {
            self.function_prepends.pop();
        }
    }
    fn visit_mut_stmts(&mut self, stmts: &mut Vec<Stmt>) {
        let old = std::mem::replace(&mut self.parent_is_scopable, true);
        stmts.visit_mut_children_with(self);
        self.parent_is_scopable = old;
    }
    fn visit_mut_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::Decl(Decl::Fn(function)) = stmt {
            function.function.visit_mut_with(self);
            if let Some(expr) = self.transform_function(
                &function.function,
                Some(function.ident.sym.to_string()),
                Some(function.ident.clone()),
            ) {
                *stmt = if self.parent_is_scopable {
                    const_decl(Pat::Ident(function.ident.clone().into()), expr)
                } else {
                    expr.into_stmt()
                };
            }
        } else {
            let old = std::mem::replace(&mut self.parent_is_scopable, false);
            stmt.visit_mut_children_with(self);
            self.parent_is_scopable = old;
        }
    }
    fn visit_mut_module_item(&mut self, item: &mut ModuleItem) {
        match item {
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                if let Decl::Fn(function) = &mut export.decl {
                    function.function.visit_mut_with(self);
                    if let Some(expr) = self.transform_function(
                        &function.function,
                        Some(function.ident.sym.to_string()),
                        Some(function.ident.clone()),
                    ) {
                        let Stmt::Decl(decl) =
                            const_decl(Pat::Ident(function.ident.clone().into()), expr)
                        else {
                            unreachable!()
                        };
                        export.decl = decl;
                    }
                } else {
                    export.decl.visit_mut_with(self);
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(export)) => {
                if let DefaultDecl::Fn(function) = &mut export.decl {
                    function.function.visit_mut_with(self);
                    if let Some(expr) = self.transform_function(
                        &function.function,
                        function.ident.as_ref().map(|id| id.sym.to_string()),
                        function.ident.clone(),
                    ) {
                        *item = ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(
                            ExportDefaultExpr {
                                span: export.span,
                                expr: Box::new(expr),
                            },
                        ));
                    }
                } else {
                    item.visit_mut_children_with(self);
                }
            }
            _ => item.visit_mut_children_with(self),
        }
    }
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Fn(function) => {
                function.function.visit_mut_with(self);
                if let Some(replacement) = self.transform_function(
                    &function.function,
                    function.ident.as_ref().map(|id| id.sym.to_string()),
                    function.ident.clone(),
                ) {
                    *expr = replacement;
                }
            }
            Expr::Arrow(arrow) => {
                self.function_prepends.push(Vec::new());
                arrow.visit_mut_children_with(self);
                let is_worklet = matches!(&*arrow.body, ArrowFunctionBody::FunctionBody(body) if has_directive(body, "worklet"));
                if let ArrowFunctionBody::FunctionBody(body) = &mut *arrow.body {
                    self.function_body_prepends(body);
                } else {
                    self.function_prepends.pop();
                }
                if is_worklet && self.error.is_none() {
                    let ArrowFunctionBody::FunctionBody(body) = &*arrow.body else {
                        unreachable!()
                    };
                    let input = WorkletInput {
                        params: arrow
                            .params
                            .iter()
                            .cloned()
                            .map(|pat| Param {
                                span: DUMMY_SP,
                                decorators: vec![],
                                pat,
                            })
                            .collect(),
                        body: body.clone(),
                        self_id: None,
                        name: None,
                        is_async: arrow.is_async,
                        is_generator: false,
                    };
                    match make_worklet_factory(input, self) {
                        Ok(replacement) => *expr = replacement,
                        Err(error) => self.report_error(error, arrow.span),
                    }
                }
            }
            _ => {
                expr.visit_mut_children_with(self);
                if !self.options.bundle_mode
                    && self.options.substitute_web_platform_checks
                    && matches!(expr, Expr::Call(call) if matches!(&call.callee, Callee::Expr(expr) if matches!(&**expr, Expr::Ident(id) if matches!(id.sym.as_ref(), "isWeb" | "shouldBeUseWeb"))))
                {
                    *expr = Expr::Lit(Lit::Bool(Bool {
                        span: DUMMY_SP,
                        value: true,
                    }));
                }
            }
        }
    }
    fn visit_mut_prop(&mut self, prop: &mut Prop) {
        if let Prop::Method(method) = prop {
            method.key.visit_mut_with(self);
            method.function.visit_mut_with(self);
            let name = if let PropName::Ident(id) = &method.key {
                Some(id.sym.to_string())
            } else {
                None
            };
            if let Some(expr) = self.transform_function(&method.function, name, None) {
                *prop = Prop::KeyValue(KeyValueProp {
                    key: method.key.clone(),
                    value: Box::new(expr),
                });
            }
        } else if let Prop::Getter(getter) = prop {
            if getter
                .function
                .body
                .as_ref()
                .is_some_and(|body| has_directive(body, "worklet"))
            {
                self.report_error(
                    format!(
                        "the `{}` getter cannot be a worklet",
                        prop_name_str(&getter.key).unwrap_or("<computed>")
                    ),
                    getter.span,
                );
            } else {
                prop.visit_mut_children_with(self);
            }
        } else if let Prop::Setter(setter) = prop {
            if setter
                .function
                .body
                .as_ref()
                .is_some_and(|body| has_directive(body, "worklet"))
            {
                self.report_error(
                    format!(
                        "the `{}` setter cannot be a worklet",
                        prop_name_str(&setter.key).unwrap_or("<computed>")
                    ),
                    setter.span,
                );
            } else {
                prop.visit_mut_children_with(self);
            }
        } else {
            prop.visit_mut_children_with(self);
        }
    }
    fn visit_mut_class_member(&mut self, member: &mut ClassMember) {
        let ClassMember::Method(method) = member else {
            member.visit_mut_children_with(self);
            return;
        };
        method.visit_mut_children_with(self);
        if method.kind != MethodKind::Method {
            return;
        }
        let name = if let PropName::Ident(id) = &method.key {
            Some(id.sym.to_string())
        } else {
            None
        };
        if let Some(expr) = self.transform_function(&method.function, name, None) {
            *member = ClassMember::ClassProp(ClassProp {
                span: method.span,
                key: method.key.clone(),
                value: Some(Box::new(expr)),
                type_ann: None,
                is_static: method.is_static,
                decorators: vec![],
                accessibility: method.accessibility,
                is_abstract: false,
                is_optional: false,
                is_override: false,
                readonly: false,
                declare: false,
                definite: false,
            });
        }
    }
    fn visit_mut_jsx_attr(&mut self, attr: &mut JSXAttr) {
        attr.visit_mut_children_with(self);
        if self.options.bundle_mode
            || self.is_release
            || self.options.disable_inline_styles_warning
            || !matches!(&attr.name, JSXAttrName::Ident(id) if id.sym == "style")
        {
            return;
        }
        if let Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
            expr: JSXExpr::Expr(expr),
            ..
        })) = &mut attr.value
        {
            match &mut **expr {
                Expr::Object(obj) => crate::inline_style::warn_obj(obj),
                Expr::Array(array) => {
                    for element in array.elems.iter_mut().flatten() {
                        if let Expr::Object(obj) = &mut *element.expr {
                            crate::inline_style::warn_obj(obj);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn prop_name_str(name: &PropName) -> Option<&str> {
    match name {
        PropName::Ident(id) => Some(&id.sym),
        PropName::Str(value) => value.value.as_str(),
        _ => None,
    }
}
