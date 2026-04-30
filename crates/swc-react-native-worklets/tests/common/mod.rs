use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_ast::{Module, Program};
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_parser::{parse_file_as_module, Syntax, TsSyntax};
use swc_react_native_worklets::{worklets, WorkletsOptions};

pub fn transform_fixture(filename: &str, code: &str, options: WorkletsOptions) -> String {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Custom(filename.into()).into(), code.to_string());

    let syntax = Syntax::Typescript(TsSyntax {
        tsx: filename.ends_with(".tsx"),
        ..Default::default()
    });

    let module = parse_file_as_module(&fm, syntax, Default::default(), None, &mut vec![])
        .expect("failed to parse");

    let pass = worklets(WorkletsOptions {
        filename: Some(filename.to_string()),
        ..options
    });
    let Program::Module(module) = Program::Module(module).apply(pass) else {
        unreachable!()
    };

    emit(&cm, &module)
}

pub fn options_with_version() -> WorkletsOptions {
    WorkletsOptions {
        plugin_version: "test".to_string(),
        ..Default::default()
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
