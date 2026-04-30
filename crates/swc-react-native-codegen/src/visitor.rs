// Corresponds to the visitor logic in `index.js` of react-native/packages/babel-plugin-codegen/

use anyhow::Result;
use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_ast::*;
use swc_ecma_parser::{parse_file_as_module, Syntax};
use swc_ecma_visit::VisitMut;

use crate::codegen::parsers;
use crate::options::CodegenOptions;

pub struct CodegenVisitor {
    cm: Lrc<SourceMap>,
    filename: String,
    error: Option<anyhow::Error>,
}

impl CodegenVisitor {
    pub fn new(cm: Lrc<SourceMap>, options: CodegenOptions) -> Self {
        Self {
            cm,
            filename: options.filename,
            error: None,
        }
    }

    pub fn into_result(self) -> Result<()> {
        match self.error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }
}

/// Check if a declaration expression is a `codegenNativeComponent` call.
// Corresponds to `isCodegenDeclaration` in index.js
fn is_codegen_declaration(expr: &Expr) -> bool {
    parsers::find_codegen_native_component(expr).is_some()
}

/// Check if an expression is a `codegenNativeCommands` call (possibly wrapped in
/// sequence expressions for coverage instrumentation or type casts).
// Corresponds to `isCodegenNativeCommandsDeclaration` in index.js
fn is_codegen_native_commands_declaration(expr: &Expr) -> bool {
    match expr {
        // Direct call: codegenNativeCommands(...)
        Expr::Call(call) => is_codegen_native_commands_call(call),
        // Sequence expression: (cov_xxx().s[0]++, codegenNativeCommands(...))
        Expr::Seq(SeqExpr { exprs, .. }) => {
            if let Some(last) = exprs.last() {
                is_codegen_native_commands_declaration(last)
            } else {
                false
            }
        }
        // Paren wrapping
        Expr::Paren(ParenExpr { expr: inner, .. }) => is_codegen_native_commands_declaration(inner),
        // Flow type cast: (codegenNativeCommands(): Type)
        Expr::TsAs(TsAsExpr { expr: inner, .. }) => is_codegen_native_commands_declaration(inner),
        // TS type assertion
        Expr::TsTypeAssertion(TsTypeAssertion { expr: inner, .. }) => {
            is_codegen_native_commands_declaration(inner)
        }
        _ => false,
    }
}

fn is_codegen_native_commands_call(call: &CallExpr) -> bool {
    matches!(
        &call.callee,
        Callee::Expr(expr) if matches!(
            &**expr,
            Expr::Ident(id) if &*id.sym == "codegenNativeCommands"
        )
    )
}

/// Find the `codegenNativeCommands` CallExpr inside potentially wrapped expressions.
fn find_codegen_native_commands_call(expr: &Expr) -> Option<&CallExpr> {
    match expr {
        Expr::Call(call) if is_codegen_native_commands_call(call) => Some(call),
        Expr::Seq(SeqExpr { exprs, .. }) => exprs
            .last()
            .and_then(|e| find_codegen_native_commands_call(e)),
        Expr::Paren(ParenExpr { expr: inner, .. }) => find_codegen_native_commands_call(inner),
        Expr::TsAs(TsAsExpr { expr: inner, .. }) => find_codegen_native_commands_call(inner),
        Expr::TsTypeAssertion(TsTypeAssertion { expr: inner, .. }) => {
            find_codegen_native_commands_call(inner)
        }
        _ => None,
    }
}

impl VisitMut for CodegenVisitor {
    fn visit_mut_module(&mut self, module: &mut Module) {
        // Phase 1: Find default export with codegenNativeComponent and
        //          named export with codegenNativeCommands
        let mut default_export_idx: Option<usize> = None;
        let mut commands_export_idx: Option<usize> = None;
        let mut command_type_name: Option<String> = None;

        for (idx, item) in module.body.iter().enumerate() {
            match item {
                // ExportDefaultDeclaration
                ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(ExportDefaultExpr {
                    expr,
                    ..
                })) => {
                    if is_codegen_declaration(expr) {
                        default_export_idx = Some(idx);
                    }
                }

                // ExportNamedDeclaration with variable declaration
                ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
                    decl: Decl::Var(var_decl),
                    span,
                    ..
                })) => {
                    if let Some(first_decl) = var_decl.decls.first() {
                        if let Pat::Ident(BindingIdent { id, .. }) = &first_decl.name {
                            let name = &*id.sym;
                            if let Some(init) = &first_decl.init {
                                let is_valid_commands =
                                    is_codegen_native_commands_declaration(init);

                                if is_valid_commands {
                                    if name != "Commands" {
                                        self.error = Some(anyhow::anyhow!(
                                            "{}: Native commands must be exported with the name 'Commands'",
                                            self.format_error_location(*span)
                                        ));
                                        return;
                                    }
                                    commands_export_idx = Some(idx);

                                    // Extract command type name from the call
                                    if let Some(call) = find_codegen_native_commands_call(init) {
                                        command_type_name =
                                            parsers::extract_command_type_name(call);
                                    }
                                } else if name == "Commands" {
                                    self.error = Some(anyhow::anyhow!(
                                        "{}: 'Commands' is a reserved export and may only be used to export the result of codegenNativeCommands.",
                                        self.format_error_location(*span)
                                    ));
                                    return;
                                }
                            } else if name == "Commands" {
                                self.error = Some(anyhow::anyhow!(
                                    "{}: 'Commands' is a reserved export and may only be used to export the result of codegenNativeCommands.",
                                    self.format_error_location(*span)
                                ));
                                return;
                            }
                        }
                    }
                }

                // ExportNamedDeclaration with specifiers: export { Commands }
                ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(NamedExport {
                    specifiers,
                    span,
                    ..
                })) => {
                    for spec in specifiers {
                        if let ExportSpecifier::Named(ExportNamedSpecifier {
                            orig: ModuleExportName::Ident(id),
                            ..
                        }) = spec
                        {
                            if &*id.sym == "Commands" {
                                self.error = Some(anyhow::anyhow!(
                                    "{}: 'Commands' is a reserved export and may only be used to export the result of codegenNativeCommands.",
                                    self.format_error_location(*span)
                                ));
                                return;
                            }
                        }
                    }
                }

                _ => {}
            }
        }

        // Phase 2: If we found a codegenNativeComponent, generate and replace
        if let Some(default_idx) = default_export_idx {
            match self.generate_and_replace(
                module,
                default_idx,
                commands_export_idx,
                command_type_name.as_deref(),
            ) {
                Ok(()) => {}
                Err(err) => {
                    self.error = Some(err);
                }
            }
        }
    }
}

impl CodegenVisitor {
    fn format_error_location(&self, _span: swc_common::Span) -> String {
        // Return filename for error messages
        self.filename.clone()
    }

    fn generate_and_replace(
        &self,
        module: &mut Module,
        default_export_idx: usize,
        commands_export_idx: Option<usize>,
        command_type_name: Option<&str>,
    ) -> Result<()> {
        // Generate the view config
        let view_config =
            crate::codegen::generate_view_config(&self.filename, module, command_type_name)?;

        // Parse the generated JS into AST statements
        let generated_stmts = self.parse_generated_code(&view_config)?;

        // Remove commands export first (if after default export, adjust index)
        if let Some(cmd_idx) = commands_export_idx {
            module.body.remove(cmd_idx);
            // Adjust default export index if commands was before it
            let adjusted_default_idx = if cmd_idx < default_export_idx {
                default_export_idx - 1
            } else {
                default_export_idx
            };
            // Replace default export with generated statements
            module
                .body
                .splice(adjusted_default_idx..=adjusted_default_idx, generated_stmts);
        } else {
            // Just replace the default export
            module
                .body
                .splice(default_export_idx..=default_export_idx, generated_stmts);
        }

        Ok(())
    }

    fn parse_generated_code(&self, code: &str) -> Result<Vec<ModuleItem>> {
        let fm = self.cm.new_source_file(
            FileName::Custom(format!("{}<generated>", self.filename)).into(),
            code.to_string(),
        );

        let parsed = parse_file_as_module(
            &fm,
            Syntax::Es(Default::default()),
            Default::default(),
            None,
            &mut vec![],
        )
        .map_err(|e| anyhow::anyhow!("Failed to parse generated view config: {e:?}"))?;

        // Set all spans to DUMMY_SP so source maps don't point to the generated code
        Ok(parsed.body)
    }
}
