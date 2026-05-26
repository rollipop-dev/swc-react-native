use swc_common::{sync::Lrc, FileName, Globals, SourceMap, GLOBALS};
use swc_ecma_ast::{Module, Pass, Program};
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_parser::{parse_file_as_module, Syntax, TsSyntax};

pub fn transform_with<P>(filename: &str, code: &str, mut pass: P) -> String
where
    P: Pass,
{
    let globals = Globals::new();
    GLOBALS.set(&globals, || {
        let cm: Lrc<SourceMap> = Default::default();
        let fm = cm.new_source_file(FileName::Custom(filename.into()).into(), code.to_string());

        let syntax = Syntax::Typescript(TsSyntax {
            tsx: filename.ends_with(".tsx"),
            decorators: true,
            ..Default::default()
        });

        let module = parse_file_as_module(&fm, syntax, Default::default(), None, &mut vec![])
            .expect("failed to parse");

        let mut program = Program::Module(module);
        pass.process(&mut program);

        let Program::Module(module) = program else {
            unreachable!("test pass swapped Program kind")
        };

        emit(&cm, &module)
    })
}

pub fn assert_contains(out: &str, needle: &str) {
    assert!(
        out.contains(needle),
        "expected output to contain {needle:?}, got:\n{out}"
    );
}

pub fn assert_not_contains(out: &str, needle: &str) {
    assert!(
        !out.contains(needle),
        "expected output to NOT contain {needle:?}, got:\n{out}"
    );
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
