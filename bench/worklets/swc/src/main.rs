use std::path::PathBuf;
use std::time::Instant;
use std::{env, fs};

use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_parser::{parse_file_as_module, Syntax, TsSyntax};
use swc_ecma_visit::VisitMutWith;
use swc_react_native_worklets::{WorkletsOptions, WorkletsVisitor};

struct Fixture {
    filename: String,
    code: String,
}

fn load_fixtures() -> Vec<Fixture> {
    let bench_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = bench_dir.parent().unwrap().join("fixtures");

    fs::read_dir(&fixtures_dir)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", fixtures_dir.display(), e))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let ext = path.extension()?;
            if ext != "ts" && ext != "tsx" {
                return None;
            }
            let filename = path.to_string_lossy().to_string();
            let code = fs::read_to_string(&path).ok()?;
            Some(Fixture { filename, code })
        })
        .collect()
}

fn transform_one(cm: &Lrc<SourceMap>, fixture: &Fixture) {
    let fm = cm.new_source_file(
        FileName::Custom(fixture.filename.clone()).into(),
        fixture.code.clone(),
    );

    let tsx = fixture.filename.ends_with(".tsx");
    let syntax = Syntax::Typescript(TsSyntax {
        tsx,
        ..Default::default()
    });

    let mut module = parse_file_as_module(&fm, syntax, Default::default(), None, &mut vec![])
        .expect("parse failed");

    let mut visitor = WorkletsVisitor::new(WorkletsOptions {
        filename: Some(fixture.filename.clone()),
        plugin_version: "bench".to_string(),
        ..Default::default()
    })
    .with_source_map(cm.clone());
    module.visit_mut_with(&mut visitor);

    let mut buf = vec![];
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = Emitter {
            cfg: swc_ecma_codegen::Config::default().with_minify(false),
            cm: cm.clone(),
            comments: None,
            wr: writer,
        };
        emitter.emit_module(&module).expect("emit failed");
    }
}

fn main() {
    let n: usize = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);

    let fixtures = load_fixtures();
    if fixtures.is_empty() {
        eprintln!("No fixtures found");
        std::process::exit(1);
    }

    let cm: Lrc<SourceMap> = Default::default();
    for fixture in &fixtures {
        transform_one(&cm, fixture);
    }

    let start = Instant::now();
    for _ in 0..(n / fixtures.len()) {
        for fixture in &fixtures {
            let cm: Lrc<SourceMap> = Default::default();
            transform_one(&cm, fixture);
        }
    }
    let elapsed = start.elapsed();

    let ms = elapsed.as_secs_f64() * 1000.0;
    println!(
        "{} transforms in {:.1}ms ({:.3}ms/op)",
        n,
        ms,
        ms / n as f64
    );
}
