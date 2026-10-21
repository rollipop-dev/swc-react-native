//! Version-specific Legacy Eval contracts, independent of Expo or React Native.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::transform_fixture_resolved;
use swc_react_native_worklets::WorkletsOptions;

fn execute(source: &str, probe: &str) {
    let script = r#"
const vm = require('node:vm');
const input = JSON.parse(require('node:fs').readFileSync(0, 'utf8'));
const context = vm.createContext({ __DEV__: true });
context.global = context;
const materialized = new WeakMap();
context.materialize = fn => {
  if (!fn.__initData) return fn;
  if (materialized.has(fn)) return materialized.get(fn);
  const compiled = vm.runInContext(fn.__initData.code, context);
  const callable = function(...args) { return compiled.apply(callable, args); };
  materialized.set(fn, callable);
  const copy = value => typeof value === 'function' ? context.materialize(value) : value;
  callable.__closure = Array.isArray(fn.__closure)
    ? fn.__closure.map(copy)
    : Object.fromEntries(Object.entries(fn.__closure || {}).map(([key, value]) => [key, copy(value)]));
  callable._recur = callable;
  return callable;
};
vm.runInContext(input.source + '\n' + input.probe, context);
"#;
    let mut child = Command::new("node")
        .args(["-e", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Node.js is required for worklet runtime tests");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            serde_json::json!({ "source": source, "probe": probe })
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}\nTransformed source:\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn transform(source: &str, version: &str, is_release: bool) -> String {
    transform_fixture_resolved(
        "runtime.js",
        source,
        WorkletsOptions {
            plugin_version: version.into(),
            disable_source_maps: true,
            is_release,
            ..Default::default()
        },
    )
}

#[test]
fn nested_dev_macro_and_captured_worklet_execute_without_expo() {
    let source = r#"
function make(worklet) {
  'worklet';
  if (__DEV__ && typeof worklet !== 'function') throw new Error('Expected worklet');
  return (...args) => { 'worklet'; return worklet(...args); };
}

function value(offset) { 'worklet'; return offset + 40; }
"#;
    for is_release in [false, true] {
        execute(
            &transform(source, "0.7.4", is_release),
            "if (materialize(materialize(make)(value))(2) !== 42) throw Error('nested closure');",
        );
    }
}

#[test]
fn registered_worklet_error_functions_are_not_class_factories() {
    // Worklets 0.7.4 and Reanimated 4.2 register these function constructors on
    // each UI runtime. Their Babel plugin deliberately skips bound captures.
    let source = r#"
function ReanimatedError(message) { return new Error(message); }
function WorkletsError(message) { return new Error(message); }
function fail(reanimated) {
  'worklet';
  if (reanimated) throw new ReanimatedError('reanimated proof');
  throw new WorkletsError('worklets proof');
}
"#;
    let probe = r#"
globalThis.ReanimatedError = Error;
globalThis.WorkletsError = Error;
for (const [flag, expected] of [[true, 'reanimated proof'], [false, 'worklets proof']]) {
  let actual;
  try { materialize(fail)(flag); } catch (error) { actual = error.message; }
  if (actual !== expected) throw Error(`expected ${expected}, got ${actual}`);
}
"#;
    for version in [
        "0.4.0",
        "0.5.2",
        "0.6.1",
        "0.7.4",
        "0.8.3",
        "0.7.4+build.42",
    ] {
        for is_release in [false, true] {
            let code = transform(source, version, is_release);
            execute(&code, probe);
        }
    }
}

#[test]
fn current_runtime_still_captures_bound_error_names() {
    let source = "const ReanimatedError = 40, WorkletsError = 2; function value() { 'worklet'; return ReanimatedError + WorkletsError; }";
    let current = transform(source, "unknown", true);
    for version in [
        "0.3.0",
        "0.9.0",
        "0.9.1",
        "0.13.0",
        "1.0.0",
        "0.8.0-rc.0",
        "0.7.4-nightly.20261021",
        "0.8.0-bundle-mode-preview-1",
        "unknown",
    ] {
        for is_release in [false, true] {
            let code = transform(source, version, is_release);
            if is_release {
                assert_eq!(code, current, "version {version}");
            }
            execute(
                &code,
                "if (materialize(value)() !== 42 || value.__closure.length !== 2) throw Error('current closure policy');",
            );
        }
    }
}

#[test]
fn legacy_error_policy_preserves_local_bindings() {
    let source = "function value(WorkletsError) { 'worklet'; const ReanimatedError = 40; return ReanimatedError + WorkletsError; }";
    for version in ["0.4.0", "0.7.4", "0.8.3"] {
        execute(
            &transform(source, version, false),
            "if (materialize(value)(2) !== 42) throw Error('local binding changed');",
        );
    }
}

#[test]
fn legacy_error_policy_preserves_unbound_captures() {
    let source = "globalThis.ReanimatedError = 40; globalThis.WorkletsError = 2; function value() { 'worklet'; return ReanimatedError + WorkletsError; }";
    for version in ["0.4.0", "0.7.4", "0.8.3"] {
        execute(
            &transform(source, version, false),
            "globalThis.ReanimatedError = globalThis.WorkletsError = 0; if (materialize(value)() !== 42 || value.__closure.length !== 2) throw Error('unbound capture changed');",
        );
    }
}

#[test]
fn current_runtime_preserves_error_class_factories() {
    let source = r#"
class ReanimatedError {
  __workletClass = true;
  constructor(value) { this.value = value; }
}
class WorkletsError {
  __workletClass = true;
  constructor(value) { this.value = value; }
}
function value() { 'worklet'; return new ReanimatedError(40).value + new WorkletsError(2).value; }
"#;
    for version in ["0.9.1", "0.13.0"] {
        for is_release in [false, true] {
            execute(
                &transform(source, version, is_release),
                "if (materialize(value)() !== 42) throw Error('current class factory changed');",
            );
        }
    }
}

#[test]
fn invalid_versions_keep_current_closure_rules() {
    let source = "const ReanimatedError = 40, WorkletsError = 2; function value() { 'worklet'; return ReanimatedError + WorkletsError; }";
    let current = transform(source, "unknown", true);
    for version in ["", "0.7", "0.7.4.1", "0.7.04", "v0.7.4", "0.7.4+", "0.7.4-"] {
        let code = transform(source, version, true);
        assert_eq!(code, current, "version {version}");
        execute(
            &code,
            "if (materialize(value)() !== 42) throw Error('invalid version activated compatibility');",
        );
    }
}

#[test]
fn worklets074_class_factories_still_execute() {
    let source = r#"
class Value {
  __workletClass = true;
  constructor(value) { this.value = value; }
  plus(value) { return this.value + value; }
}
function value() { 'worklet'; return new Value(40).plus(2); }
"#;
    for is_release in [false, true] {
        execute(
            &transform(source, "0.7.4", is_release),
            "if (materialize(value)() !== 42) throw Error('class factory');",
        );
    }
}
