use std::io::Write;
use std::process::{Command, Stdio};

use swc_common::{sync::Lrc, FileName, Globals, Mark, SourceMap, GLOBALS};
use swc_ecma_ast::{Pass, Program};
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_compat_es2015::{block_scoping, destructuring, parameters};
use swc_ecma_parser::{parse_file_as_module, Syntax};
use swc_ecma_transforms_base::{
    helpers::{inject_helpers, Helpers, HELPERS},
    hygiene::hygiene,
    resolver,
};
use swc_react_native_worklets::{worklets, WorkletsOptions};

fn check_captured_global(parameters: bool, destructuring: bool, lower_blocks: bool) {
    check_captured_source(
        "function probe() { 'worklet'; return __DEV__ ? 42 : 0; }",
        parameters,
        destructuring,
        lower_blocks,
    );
}

fn check_captured_source(source: &str, parameters: bool, destructuring: bool, lower_blocks: bool) {
    let script = r#"
const vm = require('node:vm');
const context = vm.createContext({ __DEV__: true });
context.global = context;
vm.runInContext(require('node:fs').readFileSync(0, 'utf8') +
  ';if(probe()!==42)throw Error("incorrect capture");', context);
"#;
    for version in ["0.7.4", "0.9.1", "0.13.0"] {
        let code = GLOBALS.set(&Globals::new(), || {
            let cm: Lrc<SourceMap> = Default::default();
            let file = cm.new_source_file(
                FileName::Custom("probe.js".into()).into(),
                source.to_string(),
            );
            let module = parse_file_as_module(
                &file,
                Syntax::default(),
                Default::default(),
                None,
                &mut vec![],
            )
            .unwrap();
            let unresolved_mark = Mark::new();
            let mut program = Program::Module(module);
            resolver(unresolved_mark, Mark::new(), false).process(&mut program);
            HELPERS.set(&Helpers::new(false), || {
                worklets(
                    cm.clone(),
                    WorkletsOptions {
                        filename: Some("probe.js".into()),
                        plugin_version: version.into(),
                        disable_source_maps: true,
                        ..Default::default()
                    },
                )
                .process(&mut program);
                if parameters {
                    parameters::parameters(Default::default(), unresolved_mark)
                        .process(&mut program);
                }
                if destructuring {
                    destructuring::destructuring(Default::default()).process(&mut program);
                }
                if lower_blocks {
                    block_scoping(unresolved_mark).process(&mut program);
                }
                inject_helpers(unresolved_mark).process(&mut program);
                hygiene().process(&mut program);
            });
            let mut bytes = vec![];
            Emitter {
                cfg: Default::default(),
                comments: None,
                cm: cm.clone(),
                wr: JsWriter::new(cm, "\n", &mut bytes, None),
            }
            .emit_program(&program)
            .unwrap();
            String::from_utf8(bytes).unwrap()
        });
        let mut child = Command::new("node")
            .args(["-e", script])
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(code.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "version {version}: {}\n{code}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn captured_globals_survive_parameters() {
    check_captured_global(true, false, false);
}

#[test]
fn captured_globals_survive_destructuring() {
    check_captured_global(false, true, false);
}

#[test]
fn captured_globals_survive_block_scoping() {
    check_captured_global(false, false, true);
}

#[test]
fn captured_globals_survive_parameters_then_block_scoping() {
    check_captured_global(true, false, true);
}

#[test]
fn captured_globals_survive_destructuring_then_block_scoping() {
    check_captured_global(false, true, true);
}

#[test]
fn captured_globals_survive_hermes_compat_sequence() {
    check_captured_global(true, true, true);
}

#[test]
fn captured_global_shorthand_survives_hermes_compat_sequence() {
    check_captured_source(
        "function probe() { 'worklet'; return { __DEV__ }.__DEV__ ? 42 : 0; }",
        true,
        true,
        true,
    );
}

#[test]
fn captured_locals_preserve_nested_shadowing() {
    check_captured_source(
        "const value = 40; function probe() { 'worklet'; function local(value) { return value; } return local(2) + value; }",
        true,
        true,
        true,
    );
}

#[test]
fn captured_locals_preserve_default_and_rest_parameters() {
    check_captured_source(
        "const captured = 40; function probe(value = 2, ...rest) { 'worklet'; return captured + value + rest.length; }",
        true,
        true,
        true,
    );
}

#[test]
fn nested_worklets_preserve_captured_globals() {
    check_captured_source(
        "function probe() { 'worklet'; const inner = () => { 'worklet'; return __DEV__ ? 42 : 0; }; return inner(); }",
        true,
        true,
        true,
    );
}
