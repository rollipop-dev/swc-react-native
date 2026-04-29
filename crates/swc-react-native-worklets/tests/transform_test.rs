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
