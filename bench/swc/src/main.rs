use std::path::PathBuf;
use std::time::Instant;
use std::{env, fs};

use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_parser::{parse_file_as_module, FlowSyntax, Syntax, TsSyntax};
use swc_ecma_visit::VisitMutWith;
use swc_plugin_codegen::CodegenVisitor;

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
            let entry = entry.ok()?;
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext == "js" || ext == "ts")
            {
                let filename = format!("/{}", path.file_name()?.to_string_lossy());
                let code = fs::read_to_string(&path).ok()?;
                Some(Fixture { filename, code })
            } else {
                None
            }
        })
        .collect()
}

fn transform_one(cm: &Lrc<SourceMap>, fixture: &Fixture) {
    let fm = cm.new_source_file(
        FileName::Custom(fixture.filename.clone()).into(),
        fixture.code.clone(),
    );

    let syntax = if fixture.filename.ends_with(".ts") {
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

    let mut module = parse_file_as_module(&fm, syntax, Default::default(), None, &mut vec![])
        .expect("parse failed");

    let mut visitor = CodegenVisitor::new(cm.clone(), &fixture.filename, &fixture.code);
    module.visit_mut_with(&mut visitor);
    visitor.into_result().expect("transform failed");

    // Emit to string (mirrors real usage)
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

    // Warmup
    let cm: Lrc<SourceMap> = Default::default();
    for fixture in &fixtures {
        transform_one(&cm, fixture);
    }

    // Benchmark
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
