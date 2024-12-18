use std::path::PathBuf;

use react_native_plugin_codegen::transform_codegen;
use swc_ecma_parser::{Syntax, TsSyntax};
use swc_ecma_transforms_testing::test_fixture;

#[testing::fixture("tests/fixture/**/input.js")]
fn fixture(input: PathBuf) {
    let filename = input.to_string_lossy();
    let output = input.with_file_name("output.js");
    let phase = 0.0; // ModulePhase::Register

    test_fixture(
        Syntax::Typescript(TsSyntax {
            tsx: filename.ends_with(".tsx"),
            ..Default::default()
        }),
        &|_| transform_codegen(String::from("index.ts")),
        &input,
        &output,
        Default::default(),
    );
}
