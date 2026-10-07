// Port of plugin-oxc/src/worklet_file.rs.
use crate::imports::{create_import_path, ImportInfo};
use crate::worklet_string_code::emit_module;
use std::path::Path;
use swc_common::{sync::Lrc, SourceMap, DUMMY_SP};
use swc_ecma_ast::*;

#[derive(Debug, Clone)]
pub struct EmittedFile {
    pub path: String,
    pub content: String,
}

pub fn generate_worklet_file(
    mut factory: FnExpr,
    imports: &[ImportInfo],
    filename: &str,
    package_dir: &Path,
    cm: &Lrc<SourceMap>,
) -> Result<String, String> {
    let mut body = Vec::new();
    for info in imports {
        if info.source.starts_with('.') && matches!(info.specifier, ImportSpecifier::Default(_)) {
            continue;
        }
        let source = if info.source.starts_with('.') {
            create_import_path(filename, &info.source, package_dir)
                .unwrap_or_else(|| info.source.clone())
        } else {
            info.source.clone()
        };
        body.push(ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
            span: DUMMY_SP,
            specifiers: vec![info.specifier.clone()],
            src: Box::new(Str {
                span: DUMMY_SP,
                value: source.into(),
                raw: None,
            }),
            type_only: false,
            with: None,
            phase: ImportPhase::Evaluation,
        })));
    }
    let expr = if factory.function.params.is_empty() {
        let factory_body = factory
            .function
            .body
            .as_mut()
            .expect("factory always has a body");
        let Some(Stmt::Return(ReturnStmt {
            arg: Some(returned),
            ..
        })) = factory_body.stmts.pop()
        else {
            unreachable!("factory always returns its worklet")
        };
        body.extend(
            std::mem::take(&mut factory_body.stmts)
                .into_iter()
                .map(ModuleItem::Stmt),
        );
        returned
    } else {
        Box::new(Expr::Fn(factory))
    };
    body.push(ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(
        ExportDefaultExpr {
            span: DUMMY_SP,
            expr,
        },
    )));
    let module = Module {
        span: DUMMY_SP,
        body,
        shebang: None,
    };
    let printed = emit_module(&module, cm, false)?;
    if std::env::var_os("WORKLETS_WRITE_ORIGIN").is_some() {
        Ok(format!("// __workletOrigin: {filename}\n{printed}"))
    } else {
        Ok(printed)
    }
}

pub fn write_worklet_file(package_dir: &Path, file: &EmittedFile) -> Result<(), String> {
    let dir = package_dir.join(".worklets");
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
    let path = dir.join(file.path.rsplit('/').next().unwrap_or(&file.path));
    std::fs::write(&path, &file.content)
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}
