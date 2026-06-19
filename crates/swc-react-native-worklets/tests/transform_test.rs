//! Smoke tests for the worklets transform.
//!
//! These cover the most common shapes — function declarations marked with
//! a `'worklet'` directive, hook callbacks, and pass-through code without
//! any worklet markers. The full upstream Babel test suite (~170 cases at
//! `react-native-reanimated/packages/react-native-worklets/__tests__/plugin.test.ts`)
//! is intended to be ported as fixtures over time.

mod common;

use common::{options_with_version, transform_fixture, transform_fixture_resolved};

#[test]
fn no_worklet_directive_passes_through() {
    let code = r#"
function regular() {
  return 42;
}
export default regular;
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    insta::assert_snapshot!(out);
}

#[test]
fn function_with_worklet_directive() {
    let code = r#"
function add(a, b) {
  'worklet';
  return a + b;
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    insta::assert_snapshot!(out);
}

#[test]
fn worklet_arrow_assigned_to_variable() {
    let code = r#"
const square = (x) => {
  'worklet';
  return x * x;
};
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    insta::assert_snapshot!(out);
}

#[test]
fn use_animated_style_callback_is_workletized() {
    let code = r#"
import { useAnimatedStyle } from 'react-native-reanimated';

function Component() {
  const style = useAnimatedStyle(() => {
    return { opacity: 1 };
  });
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    insta::assert_snapshot!(out);
}

#[test]
fn worklet_captures_outer_variable() {
    let code = r#"
const outer = 10;
function scale(x) {
  'worklet';
  return x * outer;
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    insta::assert_snapshot!(out);
}

#[test]
fn strict_global_captures_unlisted_globals() {
    let code = r#"
function fn() {
  'worklet';
  return Math.random();
}
"#;
    let mut opts = options_with_version();
    opts.strict_global = true;
    let out = transform_fixture("Sample.ts", code, opts);
    insta::assert_snapshot!(out);
}

// -----------------------------------------------------------------------
// `__workletClass` marker — class factory rewrite.
//
// The babel plugin (`react-native-worklets/plugin/src/class.ts`) emits a
// sibling `<Name>__classFactory` worklet factory function for any class
// with `__workletClass = true`. The runtime hook `useWorkletClass` reads
// `WorkletClass[<Name>__classFactory]` to clone the class onto the UI
// thread. Our port mirrors that contract so the same runtime works.
// -----------------------------------------------------------------------

// Assertion helpers — keep tests honest by checking that the output
// actually contains (or omits) the expected shape, not just that the
// snapshot looks plausible.
fn assert_contains(out: &str, needle: &str) {
    assert!(
        out.contains(needle),
        "expected output to contain {needle:?}, got:\n{out}"
    );
}

fn assert_not_contains(out: &str, needle: &str) {
    assert!(
        !out.contains(needle),
        "expected output to NOT contain {needle:?}, got:\n{out}"
    );
}

#[test]
fn worklet_class_marker_emits_factory_wrapper() {
    let code = r#"
class Sky {
  __workletClass = true;
  draw() { this.x = 1; }
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());

    // Marker is consumed (not present in output).
    assert_not_contains(&out, "__workletClass");
    // Factory binding name follows the babel convention `<Name>__classFactory`.
    assert_contains(&out, "Sky__classFactory");
    // Factory is invoked once to bind the JS-thread copy of the class.
    assert_contains(&out, "Sky__classFactory()");
    // Self-reference assignment lets the runtime resolve
    // `Class[<Name>__classFactory]` on the UI thread.
    assert_contains(&out, "Sky.Sky__classFactory = Sky__classFactory");

    insta::assert_snapshot!(out);
}

#[test]
fn worklet_class_marker_on_method_with_reserved_keyword_name() {
    // Methods named with reserved keywords (`throw`, `delete`, ...) must
    // not be emitted as raw `var <keyword> = ...` bindings.
    let code = r#"
class Ball {
  __workletClass = true;
  throw(vx, vy) { this.vx += vx; }
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());

    assert_contains(&out, "Ball__classFactory");
    // Reserved-keyword method names get `_`-prefixed when reused as
    // bindings (per `sanitize_ident` + `is_reserved_word`).
    assert_not_contains(&out, "var throw =");
    assert_not_contains(&out, "function throw(");

    insta::assert_snapshot!(out);
}

#[test]
fn worklet_class_marker_preserves_named_export() {
    let code = r#"
export class Sky {
  __workletClass = true;
  draw() { this.x = 1; }
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());

    assert_contains(&out, "Sky__classFactory");
    // The class binding stays exported; only its initializer is rewritten.
    assert_contains(&out, "export const Sky = Sky__classFactory()");

    insta::assert_snapshot!(out);
}

#[test]
fn worklet_class_marker_preserves_default_export() {
    let code = r#"
export default class Sky {
  __workletClass = true;
  draw() { this.x = 1; }
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());

    assert_contains(&out, "Sky__classFactory");
    // `export default class X {}` becomes `const X = X__classFactory(); export default X;`.
    assert_contains(&out, "const Sky = Sky__classFactory()");
    assert_contains(&out, "export default Sky");

    insta::assert_snapshot!(out);
}

#[test]
fn worklet_class_marker_skipped_when_disabled() {
    let code = r#"
class Sky {
  __workletClass = true;
  draw() { this.x = 1; }
}
"#;
    let mut opts = options_with_version();
    opts.disable_worklet_classes = true;
    let out = transform_fixture("Sample.ts", code, opts);

    // Disable flag mirrors babel's `disableWorkletClasses` — the marker
    // is left in place and no factory is emitted.
    assert_not_contains(&out, "Sky__classFactory");
    assert_contains(&out, "__workletClass");

    insta::assert_snapshot!(out);
}

#[test]
fn worklet_class_marker_skipped_in_bundle_mode() {
    let code = r#"
class Sky {
  __workletClass = true;
  draw() { this.x = 1; }
}
"#;
    let mut opts = options_with_version();
    opts.bundle_mode = true;
    let out = transform_fixture("Sample.ts", code, opts);

    // Bundle mode mirrors babel's `bundleMode`: `processIfWorkletClass`
    // returns early, so the class is left fully untouched — marker
    // intact, no method-level workletization triggered by the marker,
    // no factory emitted.
    assert_not_contains(&out, "Sky__classFactory");
    assert_contains(&out, "__workletClass");
    // The plain `draw()` method must NOT have been workletized (no
    // factory call shape) because the marker pathway is the only thing
    // that should opt this class in.
    assert_not_contains(&out, "drawFactory");
    assert_not_contains(&out, "__workletHash");

    insta::assert_snapshot!(out);
}

#[test]
fn worklet_class_marker_without_methods_still_wraps() {
    // A class can opt-in to the worklet-class machinery without having
    // any methods of its own; the factory must still be emitted so the
    // runtime hook resolves successfully.
    let code = r#"
class Empty {
  __workletClass = true;
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());

    assert_contains(&out, "Empty__classFactory");
    assert_contains(&out, "const Empty = Empty__classFactory()");
    assert_not_contains(&out, "__workletClass");

    insta::assert_snapshot!(out);
}

#[test]
fn class_without_worklet_marker_passes_through() {
    let code = r#"
class Plain {
  draw() { this.x = 1; }
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());

    // No marker → no factory rewrite, no worklet machinery whatsoever.
    assert_not_contains(&out, "Plain__classFactory");
    assert_not_contains(&out, "__workletHash");
    assert_not_contains(&out, "__initData");

    insta::assert_snapshot!(out);
}

#[test]
fn generated_worklet_file_is_skipped() {
    let code = r#"
function fn() {
  'worklet';
  return 1;
}
"#;
    let out = transform_fixture(
        "/app/node_modules/react-native-worklets/.worklets/123.js",
        code,
        options_with_version(),
    );

    assert_contains(&out, "'worklet'");
    assert_not_contains(&out, "__workletHash");
    assert_not_contains(&out, "__initData");
}

#[test]
fn bundle_mode_omits_native_only_data_and_stack_details() {
    let code = r#"
function fn() {
  'worklet';
  return 1;
}
"#;
    let mut opts = options_with_version();
    opts.bundle_mode = true;
    let out = transform_fixture("Sample.ts", code, opts);

    assert_contains(&out, "__workletHash");
    assert_contains(&out, "__pluginVersion");
    assert_not_contains(&out, "_init_data");
    assert_not_contains(&out, "__initData");
    assert_not_contains(&out, "__stackDetails");
    assert_not_contains(&out, "new global.Error");
}

#[test]
fn worklet_class_constructor_params_are_not_captured_as_closure() {
    // Regression: constructor and method params open a new scope, so
    // references to them inside the body must not leak into the outer
    // `<Name>__classFactory` worklet's closure. Without per-member
    // scope tracking these idents are misclassified as free vars and
    // show up in the factory IIFE's destructuring argument — the
    // runtime then errors with "Property 'id' doesn't exist" because
    // the closure object never carried them in the first place.
    let code = r#"
const TOP = 10;

class Ball {
  __workletClass = true;
  constructor(id, x, y) {
    this.id = id;
    this.x = x;
    this.y = y;
  }
  scale(factor) { this.x = factor * TOP; }
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());

    // Examine only the factory IIFE's destructuring parameter — the
    // serialized worklet source embedded in `init_data.code` is a
    // string, so substring matches there are noise.
    let factory_iife_destructure = extract_factory_iife_destructure(&out, "Ball__classFactory")
        .expect("Ball__classFactory IIFE destructuring should be emitted");
    assert!(
        factory_iife_destructure.contains("TOP"),
        "factory closure should capture TOP, got: {factory_iife_destructure}"
    );
    for forbidden in ["id", "x", "y", "factor", "this"] {
        assert!(
            !ident_in_destructure(&factory_iife_destructure, forbidden),
            "factory closure must NOT capture {forbidden:?}, got: {factory_iife_destructure}"
        );
    }

    insta::assert_snapshot!(out);
}

// === Worklet body lowering ===
//
// Mirrors `extraPlugins` in `workletFactory.ts` (upstream babel plugin):
// the worklet body — serialized into `init_data.code` and `eval`-d on the
// UI runtime — must be lowered to ES5-friendly syntax so it survives any
// JS engine the worklet runtime might use. These tests inspect the
// serialized `code` string literal and verify the modern syntax is gone.

/// Pulls the value of `code:` (the serialized worklet body) out of the
/// first `_worklet_*_init_data` literal in the output.
fn extract_first_init_data_code(out: &str) -> Option<String> {
    let init_marker = "init_data = {";
    let init_pos = out.find(init_marker)?;
    let tail = &out[init_pos..];
    let code_pos = tail.find("code:")?;
    let after_code = &tail[code_pos + "code:".len()..].trim_start();
    let quote = after_code.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let body = &after_code[1..];
    // Find the closing quote, accounting for escaped occurrences.
    let mut chars = body.char_indices();
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            chars.next();
            continue;
        }
        if c == quote {
            return Some(body[..i].to_string());
        }
    }
    None
}

#[test]
fn worklet_body_shorthand_props_get_lowered() {
    let code = r#"
function fn(x, y) {
  'worklet';
  return { x, y };
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    let body = extract_first_init_data_code(&out).expect("init_data.code should be present");
    assert!(
        body.contains("x: x") && body.contains("y: y"),
        "shorthand props should be expanded inside init_data.code, got: {body}"
    );
}

#[test]
fn worklet_body_arrow_functions_get_lowered() {
    let code = r#"
function fn(arr) {
  'worklet';
  const inc = (x) => x + 1;
  return inc(arr);
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    let body = extract_first_init_data_code(&out).expect("init_data.code should be present");
    assert!(
        !body.contains("=>"),
        "arrow function should be lowered inside init_data.code, got: {body}"
    );
    assert!(
        body.contains("function"),
        "lowered arrow should become a function expression, got: {body}"
    );
}

#[test]
fn worklet_body_template_literals_get_lowered() {
    let code = r#"
function fn(name) {
  'worklet';
  return `hello ${name}`;
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    let body = extract_first_init_data_code(&out).expect("init_data.code should be present");
    assert!(
        !body.contains("`"),
        "template literal should be lowered inside init_data.code, got: {body}"
    );
    // swc lowers `` `hello ${name}` `` to `"hello ".concat(name)` in loose
    // mode (babel emits string concat with `+`). Both are valid ES5 — what
    // matters is the backtick is gone.
    assert!(
        body.contains("\"hello \""),
        "lowered template should preserve the literal segment, got: {body}"
    );
}

#[test]
fn worklet_body_optional_chaining_gets_lowered() {
    let code = r#"
function fn(obj) {
  'worklet';
  return obj?.foo;
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    let body = extract_first_init_data_code(&out).expect("init_data.code should be present");
    assert!(
        !body.contains("?."),
        "optional chaining should be lowered inside init_data.code, got: {body}"
    );
}

#[test]
fn worklet_body_nullish_coalescing_gets_lowered() {
    let code = r#"
function fn(a, b) {
  'worklet';
  return a ?? b;
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    let body = extract_first_init_data_code(&out).expect("init_data.code should be present");
    assert!(
        !body.contains("??"),
        "nullish coalescing should be lowered inside init_data.code, got: {body}"
    );
}

#[test]
fn worklet_body_lowering_runs_hygiene_to_avoid_temp_collisions() {
    // Regression: two adjacent `a?.b ?? c` expressions in the same scope
    // both alias their result to a fresh-mark `_ref` ident. Without
    // hygiene, the marks collapse onto the same source-level sym in the
    // emitted string, producing `var _ref, _ref;` and reading the wrong
    // slot at runtime. The HostFunction receiving that bogus value
    // throws "Value is undefined, expected a number".
    let code = r#"
const FALLBACK = { level: 'info', strict: false };

function fn(options) {
  'worklet';
  return {
    level: options?.level ?? FALLBACK.level,
    strict: options?.strict ?? FALLBACK.strict,
  };
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    let body = extract_first_init_data_code(&out).expect("init_data.code should be present");

    // Find the `var ... ;` declaration line and verify all declared idents
    // are distinct. Pre-hygiene output looked like `var _ref, _ref;`.
    let var_line = body
        .lines()
        .find(|l| l.trim_start().starts_with("var "))
        .unwrap_or("");
    let names: Vec<&str> = var_line
        .trim_start()
        .trim_start_matches("var ")
        .trim_end_matches(|c: char| c == ';' || c.is_whitespace())
        .split(',')
        .map(str::trim)
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        names.len(),
        sorted.len(),
        "hygiene should produce distinct temp idents in lowered worklet body, \
         got `var {}` in: {body}",
        names.join(", ")
    );
}

#[test]
fn worklet_closure_destructure_keeps_enclosing_locals_arrow() {
    // Mirrors `createWorkletRuntime`: an arrow worklet that captures both
    // imports and locals of an enclosing function. All captured names
    // must stay shorthand in the `this.__closure` destructure so the
    // body's calls resolve at runtime.
    let code = r#"
import { helper } from './helpers';

const createSerializable = (fn: any) => fn;

function outer(initializerFn: () => void) {
  const local = helper();
  return createSerializable(() => {
    'worklet';
    local();
    initializerFn();
  });
}
"#;
    let out = transform_fixture_resolved("Sample.ts", code, options_with_version());
    let body = extract_first_init_data_code(&out).expect("init_data.code should be present");
    assert!(
        !body.contains("local:") && !body.contains("initializerFn:"),
        "closure destructure must keep enclosing locals shorthand, got: {body}"
    );
}

#[test]
fn worklet_closure_destructure_keeps_imported_binding_name_arrow() {
    // Mirrors the real reanimated/worklets case: an arrow worklet passed
    // to `createSerializable(...)` that calls multiple imported helpers.
    // The destructure should stay shorthand so the body's `helper()`
    // calls still resolve in the runtime scope.
    let code = r#"
import { helper, other } from './helpers';

const createSerializable = (fn: any) => fn;

const runtime = createSerializable(() => {
  'worklet';
  helper();
  other();
});
"#;
    // Resolver-first pipeline: this is what assigns the import marks that
    // make the destructure/body ctxts diverge.
    let out = transform_fixture_resolved("Sample.ts", code, options_with_version());
    let body = extract_first_init_data_code(&out).expect("init_data.code should be present");

    assert!(
        !body.contains("helper:") && !body.contains("other:"),
        "arrow worklet closure destructure must not rename imports, got: {body}"
    );
}

#[test]
fn worklet_closure_destructure_keeps_imported_binding_name() {
    // Regression: when a worklet captures an imported identifier, the
    // synthesized `const { name } = this.__closure` shorthand must keep
    // the binding name in sync with the body's references. Earlier the
    // destructure ident was built with the default SyntaxContext while
    // the body refs kept the import ctxt — hygiene then disambiguated
    // them by renaming the destructure side (`{ name: name1 }`), so the
    // body's `name()` calls would resolve to nothing at runtime.
    let code = r#"
import { helper } from './helper';

function fn() {
  'worklet';
  helper();
  helper();
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    let body = extract_first_init_data_code(&out).expect("init_data.code should be present");

    assert!(
        body.contains("const { helper }") || body.contains("var { helper }"),
        "closure destructure should keep the unrenamed `helper` binding, got: {body}"
    );
    assert!(
        !body.contains("helper:"),
        "closure destructure must not be renamed to `helper: helperN`, got: {body}"
    );
}

#[test]
fn worklet_body_optional_chaining_nullish_keeps_precedence() {
    // Regression: `frame?.opacity ?? 0` lowers to a `_ref = <optchain>`
    // assignment nested inside the `<…> !== null && _ref !== void 0`
    // nullish test. The assignment has lower precedence than the binary
    // test, so it must be parenthesized — `(_ref = …) !== null …`.
    // Without a final `fixer()` pass swc emits `_ref = … !== null …`,
    // which makes the assignment swallow the whole ternary and reads
    // `_ref` before it is set (always `undefined`).
    let code = r#"
function fn(frame) {
  'worklet';
  return frame?.opacity ?? 0;
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    let body = extract_first_init_data_code(&out).expect("init_data.code should be present");
    assert!(
        body.contains("(_ref ="),
        "optional-chain assignment must stay parenthesized, got: {body}"
    );
}

#[test]
fn worklet_body_combined_modern_syntax_gets_lowered() {
    let code = r#"
function fn(obj, fallback) {
  'worklet';
  const make = (x) => ({ x, msg: `value=${x}` });
  return make(obj?.value ?? fallback);
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());
    let body = extract_first_init_data_code(&out).expect("init_data.code should be present");
    for forbidden in ["=>", "`", "?.", "??"] {
        assert!(
            !body.contains(forbidden),
            "modern syntax {forbidden:?} should be lowered inside init_data.code, got: {body}"
        );
    }
    // `make` should now be assigned a `function` expression after the
    // arrow lowering, and the `{ x, msg: ... }` object literal should
    // have its shorthand expanded.
    assert!(
        body.contains("x: x"),
        "shorthand props should be expanded, got: {body}"
    );
}

// === JSX-position identifiers ===
//
// Mirrors `closure.ts` in the upstream babel plugin, which calls
// `idPath.skip()` on JSXIdentifiers so JSX tag/attribute names never leak
// into the worklet closure. Regular references inside JSX expression
// containers (`{value}`) must still be captured.

#[test]
fn worklet_body_jsx_tag_is_not_captured() {
    let code = r#"
import { Foo } from './foo';
const outer = 10;

function fn(): any {
  'worklet';
  const x = outer;
  return <Foo>{x}</Foo>;
}
"#;
    let out = transform_fixture("Sample.tsx", code, options_with_version());

    let destructure = extract_factory_iife_destructure(&out, "fn")
        .expect("fn factory IIFE destructuring should be emitted");

    assert!(
        ident_in_destructure(&destructure, "outer"),
        "outer should be captured (positive control), got: {destructure}"
    );
    assert!(
        !ident_in_destructure(&destructure, "Foo"),
        "Foo (JSX tag) should not be captured, got: {destructure}"
    );
}

#[test]
fn worklet_body_jsx_member_chain_is_not_captured() {
    let code = r#"
import { Lib } from './lib';

function fn(): any {
  'worklet';
  return <Lib.View />;
}
"#;
    let out = transform_fixture("Sample.tsx", code, options_with_version());

    let destructure = extract_factory_iife_destructure(&out, "fn")
        .expect("fn factory IIFE destructuring should be emitted");

    assert!(
        !ident_in_destructure(&destructure, "Lib"),
        "Lib (JSX member chain root) should not be captured, got: {destructure}"
    );
}

#[test]
fn worklet_body_jsx_expr_container_is_captured() {
    // Identifiers inside `{ ... }` expression containers are regular
    // references, not JSX identifiers — they must still be captured.
    let code = r#"
import { Foo } from './foo';
const message = 'hi';

function fn(): any {
  'worklet';
  return <Foo>{message}</Foo>;
}
"#;
    let out = transform_fixture("Sample.tsx", code, options_with_version());

    let destructure = extract_factory_iife_destructure(&out, "fn")
        .expect("fn factory IIFE destructuring should be emitted");

    assert!(
        ident_in_destructure(&destructure, "message"),
        "message (JSX expr container ref) should be captured, got: {destructure}"
    );
    assert!(
        !ident_in_destructure(&destructure, "Foo"),
        "Foo (JSX tag) should not be captured, got: {destructure}"
    );
}

#[test]
fn bundle_mode_jsx_tag_is_captured() {
    let code = r#"
import { Foo } from './foo';

function fn(): any {
  'worklet';
  return <Foo />;
}
"#;
    let mut opts = options_with_version();
    opts.bundle_mode = true;
    let out = transform_fixture("Sample.tsx", code, opts);

    let destructure = extract_factory_iife_destructure(&out, "fn")
        .expect("fn factory IIFE destructuring should be emitted");

    assert!(
        ident_in_destructure(&destructure, "Foo"),
        "Foo (JSX tag) should be captured in bundle mode, got: {destructure}"
    );
}

#[test]
fn deprecated_fabric_global_is_captured() {
    let code = r#"
function fn() {
  'worklet';
  return _IS_FABRIC;
}
"#;
    let out = transform_fixture("Sample.ts", code, options_with_version());

    let destructure = extract_factory_iife_destructure(&out, "fn")
        .expect("fn factory IIFE destructuring should be emitted");

    assert!(
        ident_in_destructure(&destructure, "_IS_FABRIC"),
        "_IS_FABRIC should no longer be treated as a default global, got: {destructure}"
    );
}

#[test]
fn worklet_class_new_substitution_keeps_binding_name() {
    // Regression: a worklet class whose body does `new Other(...)` gets a
    // synthesized `const Other = Other__classFactory();` preamble. The
    // `Other` binding must share the body's `new Other(...)` ctxt — else
    // the final hygiene pass renames the binding (`const Other1 = …`)
    // while the `new Other(...)` calls keep `Other`, producing a runtime
    // "Property 'Other' doesn't exist".
    let code = r#"
class Other {
  __workletClass = true;
  value = 1;
}

class Owner {
  __workletClass = true;
  items = [];
  constructor() {
    this.items.push(new Other());
    this.items.push(new Other());
  }
}
"#;
    let out = transform_fixture_resolved("Sample.ts", code, options_with_version());

    // The Owner factory's init_data.code embeds the constructor worklet.
    let owner_code = out
        .split("code:")
        .find(|chunk| chunk.contains("new Other"))
        .expect("a worklet body should contain `new Other`");

    assert!(
        owner_code.contains("const Other = Other__classFactory()")
            || owner_code.contains("var Other = Other__classFactory()"),
        "class-new preamble must bind the unrenamed `Other`, got: {owner_code}"
    );
    assert!(
        !owner_code.contains("Other1 = Other__classFactory"),
        "class-new binding must not be hygiene-renamed, got: {owner_code}"
    );
}

/// Extract the destructuring parameter list of the named class-factory
/// IIFE, i.e. the `({ … })` immediately after the factory factory's
/// `})(…)`.
fn extract_factory_iife_destructure(out: &str, factory_name: &str) -> Option<String> {
    // The factory IIFE shape is:
    //   const <FactoryName> = (function <FactoryName>_<sourceTag>Factory({ ... }) {
    //     ...
    //   })({
    //     <closure args>
    //   });
    //   const <ClassName> = <FactoryName>();
    let factory_start = format!("const {factory_name} =");
    let start = out.find(&factory_start)?;
    let tail = &out[start..];
    // Find the `})(` that closes the factory factory and opens the IIFE call.
    let close_marker = "})(";
    let close = tail.find(close_marker)?;
    let after_open = close + close_marker.len();
    let end_marker = ");";
    let end_rel = tail[after_open..].find(end_marker)?;
    Some(tail[after_open..after_open + end_rel].to_string())
}

/// Whether `name` appears as a destructured property key (not as part of
/// another identifier or a value position) in the IIFE arg literal.
fn ident_in_destructure(destructure: &str, name: &str) -> bool {
    for line in destructure.lines() {
        let trimmed = line.trim().trim_end_matches([',', ' ']);
        let key = trimmed.split(':').next().unwrap_or("").trim();
        if key == name {
            return true;
        }
    }
    false
}
