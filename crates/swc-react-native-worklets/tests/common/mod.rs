#![allow(dead_code)]

use swc_common::{sync::Lrc, FileName, Globals, Mark, SourceMap, GLOBALS};
use swc_ecma_ast::{Module, Pass, Program};
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_parser::{parse_file_as_module, Syntax, TsSyntax};
use swc_ecma_transforms_base::resolver;
use swc_react_native_worklets::{
    worklets, CommonOptions, EmittedFile, Options, ReanimatedVisitor, WorkletsOptions,
    WorkletsVisitor,
};

pub fn transform_fixture(filename: &str, code: &str, options: impl Into<Options>) -> String {
    // Mark::new() (used by the body lowering passes) requires a swc_common
    // GLOBALS scope. Production callers (rolldown, etc.) already set one up;
    // tests need to do it explicitly.
    let mut options = options.into();
    options.common_mut().filename = Some(filename.to_string());
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

        let pass = worklets(cm.clone(), options);
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
pub fn transform_fixture_resolved(
    filename: &str,
    code: &str,
    options: impl Into<Options>,
) -> String {
    let mut options = options.into();
    options.common_mut().filename = Some(filename.to_string());
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

        let pass = worklets(cm.clone(), options);
        let Program::Module(module) = program.apply(pass) else {
            unreachable!()
        };

        emit(&cm, &module)
    })
}

pub fn options_with_version() -> WorkletsOptions {
    WorkletsOptions {
        common: CommonOptions {
            // Keep snapshot output stable across machines.
            disable_source_maps: true,
            ..Default::default()
        },
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

pub fn transform_with_files(
    filename: &str,
    code: &str,
    options: impl Into<Options>,
    package_dir: Option<&std::path::Path>,
) -> Result<(String, Vec<EmittedFile>), String> {
    let mut options = options.into();
    options.common_mut().filename = Some(filename.to_string());
    GLOBALS.set(&Globals::new(), || {
        let cm: Lrc<SourceMap> = Default::default();
        let file = cm.new_source_file(FileName::Custom(filename.into()).into(), code.to_string());
        let syntax = Syntax::Typescript(TsSyntax { tsx: filename.ends_with(".tsx"), ..Default::default() });
        let module = parse_file_as_module(&file, syntax, Default::default(), None, &mut vec![]).map_err(|error| format!("{error:?}"))?;
        let import_spans: Vec<_> = module.body.iter().filter_map(|item| match item { swc_ecma_ast::ModuleItem::ModuleDecl(swc_ecma_ast::ModuleDecl::Import(import)) if !import.specifiers.is_empty() => Some(import.span), _ => None }).collect();
        let mut program = Program::Module(module);
        let unresolved = Mark::new(); let top_level = Mark::new();
        resolver(unresolved, top_level, true).process(&mut program);
        swc_ecma_transforms_typescript::typescript(swc_ecma_transforms_typescript::Config { verbatim_module_syntax: true, ..Default::default() }, unresolved, top_level).process(&mut program);
        if let Program::Module(module) = &mut program { module.body.retain(|item| !matches!(item, swc_ecma_ast::ModuleItem::ModuleDecl(swc_ecma_ast::ModuleDecl::Import(import)) if import.specifiers.is_empty() && import_spans.contains(&import.span))); }
        let files = match options {
            Options::Worklets(options) => {
                let mut visitor = WorkletsVisitor::new(options).with_source_map(cm.clone());
                if let Some(package_dir) = package_dir { visitor = visitor.with_worklets_package_dir(package_dir); }
                swc_ecma_visit::VisitMutWith::visit_mut_with(&mut program, &mut visitor);
                visitor.into_result()?
            }
            Options::Reanimated(options) => {
                let mut visitor = ReanimatedVisitor::new(options).with_source_map(cm.clone());
                swc_ecma_visit::VisitMutWith::visit_mut_with(&mut program, &mut visitor);
                visitor.into_result()?
            }
        };
        let Program::Module(module) = program else { unreachable!() };
        Ok((emit(&cm, &module), files))
    })
}
