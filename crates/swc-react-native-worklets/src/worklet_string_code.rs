// Port of plugin-oxc/src/worklet_string_code.rs, with Babel's Legacy Eval
// compatibility passes and source maps for serialized worklets.
use crate::ast::{array_pattern, const_decl, ident_name};
use swc_common::{sync::Lrc, BytePos, LineCol, SourceMap, DUMMY_SP};
use swc_ecma_ast::*;
use swc_ecma_codegen::{text_writer::JsWriter, Config, Emitter};
use swc_ecma_utils::ExprFactory;

pub fn prepend_closure(body: &mut FunctionBody, closure: &[Ident]) {
    if !closure.is_empty() {
        body.stmts.insert(
            0,
            const_decl(
                array_pattern(closure),
                Expr::This(ThisExpr { span: DUMMY_SP })
                    .make_member(ident_name("__closure"))
                    .into(),
            ),
        );
    }
}

pub fn emit_module(module: &Module, cm: &Lrc<SourceMap>, minify: bool) -> Result<String, String> {
    let mut bytes = Vec::new();
    Emitter {
        cfg: Config::default().with_minify(minify),
        cm: cm.clone(),
        comments: None,
        wr: JsWriter::new(cm.clone(), "\n", &mut bytes, None),
    }
    .emit_module(module)
    .map_err(|error| error.to_string())?;
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

pub fn build_worklet_string(
    function: FnExpr,
    cm: &Lrc<SourceMap>,
    location: &str,
    legacy: bool,
    include_source_map: bool,
) -> Result<(String, Option<String>), String> {
    let statement = if legacy {
        Stmt::Expr(ExprStmt {
            span: DUMMY_SP,
            expr: Box::new(Expr::Fn(function)),
        })
    } else {
        Stmt::Decl(Decl::Fn(FnDecl {
            ident: function.ident.expect("serialized worklet is named"),
            declare: false,
            function: function.function,
        }))
    };
    let module = Module {
        span: DUMMY_SP,
        body: vec![ModuleItem::Stmt(statement)],
        shebang: None,
    };
    let module = if legacy {
        crate::transform::transform_worklet(module)
    } else {
        module
    };
    let mut bytes = Vec::new();
    let mut mappings: Vec<(BytePos, LineCol)> = Vec::new();
    Emitter {
        cfg: Config::default().with_minify(!legacy),
        cm: cm.clone(),
        comments: None,
        wr: JsWriter::new(
            cm.clone(),
            "\n",
            &mut bytes,
            include_source_map.then_some(&mut mappings),
        ),
    }
    .emit_module(&module)
    .map_err(|error| error.to_string())?;
    let code = String::from_utf8(bytes).map_err(|error| error.to_string())?;
    let code = code.trim_end_matches([';', '\n', '\r', ' ']);
    let source_map = if include_source_map && !mappings.is_empty() {
        let map = cm.build_source_map(
            &mappings,
            None,
            WorkletSourceMapConfig {
                location: location.to_string(),
            },
        );
        let mut bytes = Vec::new();
        map.to_writer(&mut bytes)
            .map_err(|error| error.to_string())?;
        Some(String::from_utf8(bytes).map_err(|error| error.to_string())?)
    } else {
        None
    };
    Ok((
        if legacy {
            format!("({code})")
        } else {
            code.to_string()
        },
        source_map,
    ))
}

struct WorkletSourceMapConfig {
    location: String,
}
impl swc_common::source_map::SourceMapGenConfig for WorkletSourceMapConfig {
    fn file_name_to_source(&self, filename: &swc_common::FileName) -> String {
        if self.location.is_empty() {
            swc_common::source_map::DefaultSourceMapGenConfig.file_name_to_source(filename)
        } else {
            self.location.clone()
        }
    }
    fn inline_sources_content(&self, _: &swc_common::FileName) -> bool {
        false
    }
}
