//! Smoke tests for the worklets transform.
//!
//! These cover the most common shapes — function declarations marked with
//! a `'worklet'` directive, hook callbacks, and pass-through code without
//! any worklet markers. The full upstream Babel test suite (~170 cases at
//! `react-native-reanimated/packages/react-native-worklets/__tests__/plugin.test.ts`)
//! is intended to be ported as fixtures over time.

mod common;

use common::{options_with_version, transform_fixture};

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
