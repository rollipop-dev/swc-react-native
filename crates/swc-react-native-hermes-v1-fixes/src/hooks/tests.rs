use std::process::Command;

use swc_common::{sync::Lrc, FileName, Globals, Mark, SourceMap, GLOBALS};
use swc_ecma_ast::{EsVersion, Pass, Program};
use swc_ecma_codegen::{text_writer::JsWriter, Config, Emitter};
use swc_ecma_hooks::{CompositeHook, VisitMutHook, VisitMutWithHook};
use swc_ecma_parser::{parse_file_as_program, EsSyntax, Syntax};
use swc_ecma_transformer::{hook_pass, transform_hook, Options};
use swc_ecma_transforms_base::{
    fixer::fixer,
    helpers::{inject_helpers, Helpers, HELPERS},
    hygiene::hygiene,
    resolver,
};
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::{
    async_arrow_non_simple_params, class_in_finally, hermes_v1_fixes_hook, super_in_object_accessor,
};

fn parse(cm: &Lrc<SourceMap>, source: &str) -> Program {
    let file = cm.new_source_file(
        FileName::Custom("input.js".into()).into(),
        source.to_string(),
    );
    parse_file_as_program(
        &file,
        Syntax::Es(EsSyntax {
            decorators: true,
            ..Default::default()
        }),
        EsVersion::default(),
        None,
        &mut Vec::new(),
    )
    .unwrap()
}

fn transform(source: &str, fused: bool, dummy_spans: bool, lower_async: bool) -> String {
    GLOBALS.set(&Globals::new(), || {
        let cm = Lrc::<SourceMap>::default();
        let mut program = parse(&cm, source);
        if dummy_spans {
            struct ClearSpans;
            impl VisitMut for ClearSpans {
                fn visit_mut_span(&mut self, span: &mut swc_common::Span) {
                    *span = swc_common::DUMMY_SP;
                }
            }
            program.visit_mut_with(&mut ClearSpans);
        }
        let unresolved_mark = Mark::new();
        resolver(unresolved_mark, Mark::new(), false).process(&mut program);
        HELPERS.set(&Helpers::new(false), || {
            let mut options = Options::default();
            options.unresolved_ctxt =
                swc_common::SyntaxContext::empty().apply_mark(unresolved_mark);
            options.env.es2017.async_to_generator = lower_async;
            if fused {
                hook_pass(CompositeHook {
                    first: hermes_v1_fixes_hook(),
                    second: transform_hook(options),
                })
                .process(&mut program);
            } else {
                (
                    async_arrow_non_simple_params(),
                    super_in_object_accessor(),
                    class_in_finally(),
                )
                    .process(&mut program);
                if lower_async {
                    hook_pass(transform_hook(options)).process(&mut program);
                }
            }
            inject_helpers(unresolved_mark).process(&mut program);
        });
        (hygiene(), fixer(None)).process(&mut program);
        let mut output = Vec::new();
        Emitter {
            cfg: Config::default(),
            cm: Lrc::clone(&cm),
            comments: None,
            wr: JsWriter::new(cm, "\n", &mut output, None),
        }
        .emit_program(&program)
        .unwrap();
        String::from_utf8(output).unwrap()
    })
}

#[test]
fn matches_existing_passes_including_shared_generated_spans() {
    let cases = [
        r"const _p = 'outer';
const read = async (value = 1, { name }, [first]) => value + name + first + _p;
const collect = async (...items) => items.length;
const nested = async (outer = async ({ value }) => value) => outer;",
        r"const obj = {
  get value() { return () => super.value; },
  set value(next) { super.value = next; },
  get outer() { return { get inner() { return super.inner; } }; },
  get isolated() { return function() { return { get value() { return super.value; } }; }; },
  method() { return super.value; }
};",
        r"try { class InTry {} } catch (error) { class InCatch {} } finally {
  class Decl extends Base { method() { class Local {} } }
  const Expr = class Named {};
  use(class {});
  function nested() { class Local {} try {} finally { class Inner {} } }
  const arrow = () => { class Local {} try {} finally { class Inner {} } };
  const obj = {
    method() { class Local {} },
    get value() { class Local {} return Local; },
    set value(next) { class Local {} }
  };
  try {} catch (error) { class NestedCatch {} } finally { class NestedFinally {} }
  class AfterNested {}
}
class AfterFinally {}",
        r"try {} finally {
  @decorate class Decorated {}
  class Outer extends (class Base {}) {
    field = class Field {};
    static { class StaticLocal {} }
  }
}",
        r"try {} finally {
  class Example {
    field = async ({ value } = { value: 1 }) => value;
    method() { return { get value() { return super.value; } }; }
  }
  const Expr = class { field = async (...values) => values; };
}",
        r"export const read = async ({ value } = { value: 1 }) => value;
try {} finally { class Local {} use(Local); }",
    ];
    for source in cases {
        for dummy_spans in [false, true] {
            assert_eq!(
                transform(source, true, dummy_spans, false),
                transform(source, false, dummy_spans, false),
                "dummy spans: {dummy_spans}, source:\n{source}"
            );
        }
    }
}

#[test]
fn composes_with_async_lowering_inside_generated_class_wrappers() {
    let source = r"try {} finally {
  class Example {
    method() {
      return {
        get value() {
          return async (value = 1, { name }) => tag`result ${await value} ${super.value} ${name}`;
        }
      };
    }
  }
  use(Example);
}";
    for dummy_spans in [false, true] {
        let code = transform(source, true, dummy_spans, true);
        assert!(code.contains("var Example ="), "{code}");
        assert!(code.contains("return Example;"), "{code}");
        assert!(code.contains("get [\"value\"]"), "{code}");
        assert!(code.contains("var value ="), "{code}");
        assert!(code.contains("var { name }"), "{code}");
        assert!(!code.contains("async ("), "{code}");
        assert!(!code.contains("await value"), "{code}");
    }
}

#[test]
fn preserves_execution_when_composed_with_async_lowering() {
    let source = r"
async function main() {
  const params = async (value = 2, { offset } = { offset: 3 }) => (await value) + offset;
  const collect = async (...values) => await Promise.resolve(values.reduce((a, b) => a + b, 0));
  const object = {
    __proto__: { value: 11, set updated(value) { this._updated = value; } },
    get value() {
      const inherited = super.value;
      return async (value = 1, { offset } = { offset: 2 }) => inherited + (await value) + offset;
    },
    set updated(value) { super.updated = value; }
  };
  object.updated = 9;
  let Example;
  try {} finally {
    class Local {
      read() { return 7; }
      field = async (value = 4) => await value;
    }
    Example = Local;
  }
  const instance = new Example();
  class Base { async read(value) { return value + 10; } }
  class Derived extends Base {
    async read(value = 3) {
      const callbacks = [];
      for (let index = 0; index < 2; index++) {
        const result = await super.read(value + index);
        callbacks.push(() => result);
      }
      return callbacks.map(callback => callback());
    }
  }
  return [await params(), await collect(1, 2, 3), await object.value(), object._updated,
    instance.read(), await instance.field(), await new Derived().read()];
}
main().then(value => console.log(JSON.stringify(value))).catch(error => {
  console.error(error);
  process.exitCode = 1;
});
";
    fn execute(code: &str) -> String {
        let output = Command::new("node")
            .args(["--input-type=module", "--eval", code])
            .output()
            .expect("Node.js is required for runtime compatibility tests");
        assert!(
            output.status.success(),
            "{}\n{code}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
    let expected = "[5,6,14,9,7,4,[13,14]]\n";
    assert_eq!(execute(source), expected);
    for fused in [false, true] {
        for dummy_spans in [false, true] {
            assert_eq!(
                execute(&transform(source, fused, dummy_spans, true)),
                expected,
                "fused: {fused}, dummy spans: {dummy_spans}"
            );
        }
    }
}

#[test]
fn supports_custom_context_and_balances_scopes_across_programs() {
    struct CountClasses;
    impl VisitMutHook<usize> for CountClasses {
        fn enter_class(&mut self, _class: &mut swc_ecma_ast::Class, count: &mut usize) {
            *count += 1;
        }
    }
    GLOBALS.set(&Globals::new(), || {
        let cm = Lrc::<SourceMap>::default();
        let mut visitor = VisitMutWithHook {
            hook: CompositeHook {
                first: hermes_v1_fixes_hook(),
                second: CountClasses,
            },
            context: 0,
        };
        for source in [
            "try {} finally { class Example {} use(class {}); }",
            "class Outside {}",
        ] {
            parse(&cm, source).visit_mut_with(&mut visitor);
        }
        assert_eq!(visitor.context, 3);
    });
}
