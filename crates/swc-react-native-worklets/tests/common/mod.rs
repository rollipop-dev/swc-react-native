use swc_common::{sync::Lrc, FileName, Globals, Mark, SourceMap, GLOBALS};
use swc_ecma_ast::{Module, Pass, Program};
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_parser::{parse_file_as_module, Syntax, TsSyntax};
use swc_ecma_transforms_base::resolver;
use swc_react_native_worklets::{worklets, WorkletsOptions};

pub fn transform_fixture(filename: &str, code: &str, options: WorkletsOptions) -> String {
    // Mark::new() (used by the body lowering passes) requires a swc_common
    // GLOBALS scope. Production callers (rolldown, etc.) already set one up;
    // tests need to do it explicitly.
    let globals = Globals::new();
    GLOBALS.set(&globals, || {
        let cm: Lrc<SourceMap> = Default::default();
        let fm = cm.new_source_file(FileName::Custom(filename.into()).into(), code.to_string());

        let syntax = Syntax::Typescript(TsSyntax {
            tsx: filename.ends_with(".tsx"),
            ..Default::default()
        });

        let module = parse_file_as_module(&fm, syntax, Default::default(), None, &mut vec![])
            .expect("failed to parse");

        let pass = worklets(
            cm.clone(),
            WorkletsOptions {
                filename: Some(filename.to_string()),
                ..options
            },
        );
        let Program::Module(module) = Program::Module(module).apply(pass) else {
            unreachable!()
        };

        emit(&cm, &module)
    })
}

/// Like [`transform_fixture`] but runs `resolver` before the worklets pass,
/// mirroring the real rolldown pipeline (resolver → worklets → lowering).
/// Resolver assigns real `SyntaxContext` marks, which is what surfaces
/// closure-capture / hygiene bugs that the bare path cannot reproduce.
pub fn transform_fixture_resolved(filename: &str, code: &str, options: WorkletsOptions) -> String {
    let globals = Globals::new();
    GLOBALS.set(&globals, || {
        let cm: Lrc<SourceMap> = Default::default();
        let fm = cm.new_source_file(FileName::Custom(filename.into()).into(), code.to_string());

        let syntax = Syntax::Typescript(TsSyntax {
            tsx: filename.ends_with(".tsx"),
            ..Default::default()
        });

        let module = parse_file_as_module(&fm, syntax, Default::default(), None, &mut vec![])
            .expect("failed to parse");

        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();

        let mut program = Program::Module(module);
        resolver(unresolved_mark, top_level_mark, false).process(&mut program);

        let pass = worklets(
            cm.clone(),
            WorkletsOptions {
                filename: Some(filename.to_string()),
                ..options
            },
        );
        let Program::Module(module) = program.apply(pass) else {
            unreachable!()
        };

        emit(&cm, &module)
    })
}

pub fn options_with_version() -> WorkletsOptions {
    WorkletsOptions {
        plugin_version: "test".to_string(),
        // Keep snapshot output stable across machines.
        disable_source_maps: true,
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
