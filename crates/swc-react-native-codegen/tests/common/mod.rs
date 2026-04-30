use anyhow::Result;
use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_ast::Module;
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_parser::{parse_file_as_module, FlowSyntax, Syntax, TsSyntax};
use swc_ecma_visit::VisitMutWith;
use swc_react_native_codegen::{CodegenOptions, CodegenVisitor};

pub fn transform_fixture(filename: &str, code: &str) -> Result<String, String> {
    let syntax = if filename.ends_with(".ts") {
        Syntax::Typescript(TsSyntax {
            tsx: false,
            ..Default::default()
        })
    } else {
        Syntax::Flow(FlowSyntax {
            all: true,
            ..Default::default()
        })
    };

    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Custom(filename.into()).into(), code.to_string());
    let mut module = parse_file_as_module(&fm, syntax, Default::default(), None, &mut vec![])
        .expect("failed to parse");

    let mut visitor = CodegenVisitor::new(
        cm.clone(),
        CodegenOptions {
            filename: filename.to_string(),
        },
    );
    module.visit_mut_with(&mut visitor);

    match visitor.into_result() {
        Ok(()) => Ok(emit(&cm, &module)),
        Err(e) => Err(e.to_string()),
    }
}

fn emit(cm: &Lrc<SourceMap>, module: &Module) -> String {
    let mut buf = vec![];
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: swc_ecma_codegen::Config::default().with_minify(false),
            cm: cm.clone(),
            comments: None,
            wr: writer,
        };
        emitter.emit_module(module).expect("failed to emit module");
    }
    String::from_utf8(buf).expect("invalid utf8")
}
