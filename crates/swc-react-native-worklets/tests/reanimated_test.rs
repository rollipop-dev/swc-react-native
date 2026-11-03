//! Reanimated 3 Babel contracts (tag 3.19.5, plugin/__tests__/plugin.test.ts).
mod common;
use common::{transform_fixture_resolved, transform_with_files};
use std::io::Write;
use std::process::{Command, Stdio};
use swc_react_native_worklets::{CommonOptions, Options, ReanimatedOptions};

fn options(is_release: bool) -> ReanimatedOptions {
    ReanimatedOptions {
        common: CommonOptions {
            disable_source_maps: true,
            is_release,
            ..Default::default()
        },
        reanimated_version: "3.19.5".into(),
        ..Default::default()
    }
}
fn execute(source: &str, probe: &str) {
    let script = r#"
const vm = require('node:vm');
const input = JSON.parse(require('node:fs').readFileSync(0, 'utf8'));
const context = vm.createContext({ __DEV__: true, _WORKLET: true, _toString: String });
context.global = context;
const uiContext = vm.createContext({ __DEV__: true, _WORKLET: true, _toString: String, ReanimatedError: Error });
uiContext.global = uiContext;
const cache = new WeakMap();
context.materialize = fn => {
  if (!fn.__initData) return fn;
  if (cache.has(fn)) return cache.get(fn);
  // RA3 shareables.ts inspects Object.keys(worklet.__closure) in development.
  if (!fn.__closure || Array.isArray(fn.__closure)) throw Error('RA3 requires an object closure');
  Object.keys(fn.__closure);
  const compiled = vm.runInContext('(' + fn.__initData.code + ')', uiContext);
  const callable = function(...args) { return compiled.apply(callable, args); };
  cache.set(fn, callable);
  callable.__closure = Object.fromEntries(Object.entries(fn.__closure).map(([key, value]) =>
    [key, typeof value === 'function' && value.__workletHash ? context.materialize(value) : value]));
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
        .expect("Node.js is required");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            serde_json::json!({"source": source, "probe": probe})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn version_discriminator_and_validation() {
    let legacy: Options = serde_json::from_str(r#"{"reanimatedVersion":"3.19.5"}"#).unwrap();
    assert!(matches!(legacy, Options::Reanimated(_)));
    assert!(legacy.validate().is_ok());
    for version in ["2.17.0", "4.0.0", "0.7.4", "3", "invalid", ""] {
        let legacy = Options::Reanimated(ReanimatedOptions {
            reanimated_version: version.into(),
            ..options(false)
        });
        assert!(legacy
            .validate()
            .unwrap_err()
            .contains("Unsupported Reanimated version"));
        assert!(transform_with_files(
            "unsupported.js",
            "function f() {'worklet'; return 1;}",
            legacy,
            None
        )
        .is_err());
    }
    let current: Options =
        serde_json::from_str(r#"{"pluginVersion":"","reanimatedVersion":"4.0.0"}"#).unwrap();
    assert!(matches!(current, Options::Worklets(_)));
    assert!(current.validate().is_ok());
    assert!(serde_json::from_str::<Options>("{}")
        .unwrap_err()
        .to_string()
        .contains("required"));
    assert!(matches!(Options::default(), Options::Worklets(_)));
    for source in [
        r#"{"pluginVersion":42,"reanimatedVersion":"3.19.5"}"#,
        r#"{"reanimatedVersion":42}"#,
    ] {
        assert!(serde_json::from_str::<Options>(source).is_err());
    }
}

#[test]
fn option_variants_roundtrip_flat_json_without_cross_runtime_fields() {
    for source in [
        r#"{"reanimatedVersion":"3.19.5","isRelease":true,"processNestedWorklets":true,"globals":["custom"]}"#,
        r#"{"pluginVersion":"0.7.4","isRelease":true,"bundleMode":true,"globals":["custom"]}"#,
    ] {
        let original: Options = serde_json::from_str(source).unwrap();
        let serialized = serde_json::to_value(&original).unwrap();
        assert!(serialized.get("common").is_none());
        assert_eq!(serialized["isRelease"], true);
        let decoded: Options = serde_json::from_value(serialized.clone()).unwrap();
        assert_eq!(original.is_reanimated(), decoded.is_reanimated());
        assert_eq!(original.version(), decoded.version());
        assert_eq!(decoded.common().globals, ["custom"]);
        assert_eq!(serde_json::to_value(decoded).unwrap(), serialized);
        if original.is_reanimated() {
            assert!(serialized.get("pluginVersion").is_none());
            assert!(serialized.get("bundleMode").is_none());
        } else {
            assert!(serialized.get("reanimatedVersion").is_none());
            assert!(serialized.get("processNestedWorklets").is_none());
        }
    }
}

#[test]
fn unsupported_legacy_extensions_are_reported() {
    let error =
        serde_json::from_str::<Options>(r#"{"reanimatedVersion":"3.19.5","bundleMode":true}"#)
            .unwrap_err();
    assert!(error.to_string().contains("bundleMode"));
    for field in ["extraPlugins", "extraPresets"] {
        let legacy: Options = serde_json::from_value(
            serde_json::json!({"reanimatedVersion":"3.19.5", field: ["custom-plugin"]}),
        )
        .unwrap();
        assert!(legacy.validate().unwrap_err().contains("extraPlugins"));
    }
}
#[test]
fn object_closures_and_version_metadata_match_ra3() {
    let source = "const offset = 40; function value(x) {'worklet'; return x + offset;} function empty() {'worklet'; return 42;}";
    for is_release in [false, true] {
        let output = transform_fixture_resolved("runtime.js", source, options(is_release));
        execute(
            &output,
            &format!(
                r#"
if (materialize(value)(2) !== 42 || materialize(empty)() !== 42) throw Error('execution');
if (value.__closure.offset !== 40 || Object.keys(empty.__closure).length !== 0) throw Error('closure');
if ('__pluginVersion' in value) throw Error('worklets metadata leaked');
if ({is_release} ? 'version' in value.__initData : value.__initData.version !== '3.19.5') throw Error('RA3 version');
"#
            ),
        );
    }
}
#[test]
fn runtime_globals_and_shadowed_globals_follow_ra3_policy() {
    let source = r#"
const ReanimatedError = function badCapture() { throw Error('captured runtime global'); };
function stringify(value) {'worklet'; return _toString(value);}
function fail() {'worklet'; throw new ReanimatedError('runtime error');}
function local(_toString) {'worklet'; return _toString + 1;}
"#;
    for is_release in [false, true] {
        let output = transform_fixture_resolved("runtime.js", source, options(is_release));
        execute(
            &output,
            r#"
globalThis.ReanimatedError = Error;
if (Object.keys(stringify.__closure).length || Object.keys(fail.__closure).length) throw Error('captured global');
if (materialize(stringify)(42) !== '42' || materialize(local)(41) !== 42) throw Error('global policy');
try { materialize(fail)(); throw Error('did not throw'); } catch (error) { if (error.message !== 'runtime error') throw error; }
"#,
        );
    }
}
#[test]
fn legacy_hooks_and_callback_positions_are_isolated_from_current_hooks() {
    let source = r#"
const useWorkletCallback = fn => fn;
const useAnimatedGestureHandler = handlers => handlers;
const withTiming = (...args) => args;
const scheduleOnUI = fn => fn;
const f = useWorkletCallback(() => 42);
const handlers = useAnimatedGestureHandler({onStart() { return 42; }});
const timing = withTiming(0, {}, () => 42, () => 0);
const currentOnly = scheduleOnUI(() => 42);
"#;
    let output = transform_fixture_resolved("hooks.js", source, options(false));
    execute(
        &output,
        r#"
if (materialize(f)() !== 42 || materialize(handlers.onStart)() !== 42 || materialize(timing[2])() !== 42) throw Error('legacy hooks');
if (timing[3].__workletHash || currentOnly.__workletHash) throw Error('current hook leaked');
"#,
    );
}
#[test]
fn recursion_and_spread_execute_without_external_helpers() {
    let source = r#"
const offset = 40;
function factorial(x) {'worklet'; return x < 2 ? 1 : x * factorial(x - 1);}
function spread(value) {'worklet'; return {...value, offset};}
"#;
    for is_release in [false, true] {
        let output = transform_fixture_resolved("spread.js", source, options(is_release));
        execute(&output, "if (materialize(factorial)(5) !== 120) throw Error('recursion'); if (materialize(spread)({x: 2}).offset !== 40 || Object.keys(spread.__closure).join() !== 'offset') throw Error('spread');");
    }
}
#[test]
fn nested_worklets_option_changes_serialized_parent() {
    let source = "function outer() {'worklet'; return () => {'worklet'; return 42;};}";
    for process_nested_worklets in [false, true] {
        let output = transform_fixture_resolved(
            "nested.js",
            source,
            ReanimatedOptions {
                process_nested_worklets,
                ..options(false)
            },
        );
        execute(
            &output,
            if process_nested_worklets {
                "const inner = materialize(outer)(); if (!inner.__workletHash || materialize(inner)() !== 42) throw Error('nested processing');"
            } else {
                "const inner = materialize(outer)(); if (inner.__workletHash || inner() !== 42) throw Error('default nested behavior');"
            },
        );
    }
}

#[test]
fn context_objects_recreate_methods_with_runtime_this() {
    for source in [
        "const offset = 40; const context = {__workletContextObject: true, value: offset, increment() {return ++this.value;}};",
        "'worklet'; const offset = 40; const context = {value: offset, increment() {return ++this.value;}};",
        "'worklet'; const offset = 40; const context = {value: offset, get increment() {return () => ++this.value;}};",
    ] {
        for is_release in [false, true] {
            let output = transform_fixture_resolved("context.js", source, options(is_release));
            execute(&output, r#"
if ('__workletContextObject' in context) throw Error('marker remains');
const factory = context.__workletContextObjectFactory;
if (!factory.__workletHash || factory.__closure.offset !== 40) throw Error('context factory');
const uiObject = materialize(factory)();
if (uiObject.increment() !== 41 || uiObject.increment() !== 42 || context.value !== 40) throw Error('context state');
"#);
        }
    }
}

#[test]
fn ra3_options_preserve_web_and_debug_behavior() {
    let source = "const offset = 42; const isWeb = () => false, shouldBeUseWeb = () => false; function value() {'worklet'; return isWeb() && shouldBeUseWeb() && offset;}";
    for version in ["3.0.0", "3.16.7", "3.19.5", "3.19.5+build.1", "3.20.0-rc.1"] {
        let output = transform_fixture_resolved(
            "runtime.js",
            source,
            ReanimatedOptions {
                common: CommonOptions {
                    substitute_web_platform_checks: true,
                    omit_native_only_data: true,
                    relative_source_location: true,
                    ..options(false).common
                },
                reanimated_version: version.into(),
                ..options(false)
            },
        );
        execute(&output, "if (value() !== 42 || value.__initData || Array.isArray(value.__closure)) throw Error('RA3 options');");
    }
}

fn babel_transform(source: &str, is_release: bool, process_nested_worklets: bool) -> String {
    let reference = std::env::var("REANIMATED_BABEL_REFERENCE")
        .expect("set REANIMATED_BABEL_REFERENCE to RA3 plugin/index.js");
    let script = r#"
const input = JSON.parse(require('node:fs').readFileSync(0, 'utf8'));
const babel = require('@babel/core');
const result = babel.transformSync(input.source, {
  filename: 'parity.js', babelrc: false, configFile: false,
  plugins: [[require(input.reference), {disableSourceMaps: true, processNestedWorklets: input.nested}]]
});
process.stdout.write(result.code);
"#;
    let mut child = Command::new("node")
        .args(["-e", script])
        .env(
            "NODE_ENV",
            if is_release {
                "production"
            } else {
                "development"
            },
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(serde_json::json!({"source": source, "reference": reference, "nested": process_nested_worklets}).to_string().as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[ignore = "requires REANIMATED_BABEL_REFERENCE and Babel dependencies in NODE_PATH"]
fn official_ra3_babel_runtime_parity() {
    let cases = [
        ("const offset = 40; function value(x) {'worklet'; return x + offset;} function empty() {'worklet'; return 42;}",
         "if (materialize(value)(2) !== 42 || materialize(empty)() !== 42 || Object.keys(empty.__closure).length) throw Error('closure parity');"),
        ("const useAnimatedGestureHandler = h => h, useWorkletCallback = f => f; const f = useWorkletCallback(() => 42); const h = useAnimatedGestureHandler({onStart() {return 42;}});",
         "if (materialize(f)() !== 42 || materialize(h.onStart)() !== 42) throw Error('hook parity');"),
        ("const n = 40; const context = {__workletContextObject: true, n, increment() {return ++this.n;}};",
         "const obj = materialize(context.__workletContextObjectFactory)(); if (obj.increment() !== 41 || obj.increment() !== 42) throw Error('context parity');"),
        ("'worklet'; const n = 40; const context = {n, increment() {return ++this.n;}};",
         "const obj = materialize(context.__workletContextObjectFactory)(); if (obj.increment() !== 41 || obj.increment() !== 42) throw Error('file parity');"),
        ("function factorial(x) {'worklet'; return x < 2 ? 1 : x * factorial(x - 1);} function stringify(value) {'worklet';return _toString({...value, x: 42}.x);}",
         "if (materialize(factorial)(5) !== 120 || materialize(stringify)({}) !== '42') throw Error('body parity');"),
    ];
    for is_release in [false, true] {
        for (source, probe) in cases {
            execute(&babel_transform(source, is_release, false), probe);
            execute(
                &transform_fixture_resolved("parity.js", source, options(is_release)),
                probe,
            );
        }
        let source = "function outer() {'worklet'; return () => {'worklet'; return 42;};}";
        for process_nested_worklets in [false, true] {
            let probe = if process_nested_worklets {
                "const inner = materialize(outer)(); if (!inner.__workletHash || materialize(inner)() !== 42) throw Error('nested parity');"
            } else {
                "const inner = materialize(outer)(); if (inner.__workletHash || inner() !== 42) throw Error('nested parity');"
            };
            execute(
                &babel_transform(source, is_release, process_nested_worklets),
                probe,
            );
            execute(
                &transform_fixture_resolved(
                    "parity.js",
                    source,
                    ReanimatedOptions {
                        process_nested_worklets,
                        ..options(is_release)
                    },
                ),
                probe,
            );
        }
    }
}

#[test]
fn web_substitution_preserves_original_ui_worklet_with_nested_option() {
    let source = "function isWeb(){'worklet';return false;} function shouldBeUseWeb(){'worklet';return false;} function value(){'worklet';return isWeb()&&shouldBeUseWeb();}";
    for process_nested_worklets in [false, true] {
        let output = transform_fixture_resolved(
            "web.js",
            source,
            ReanimatedOptions {
                common: CommonOptions {
                    substitute_web_platform_checks: true,
                    ..options(false).common
                },
                process_nested_worklets,
                ..options(false)
            },
        );
        execute(&output, "if (value() !== true || materialize(value)() !== false || !value.__closure.isWeb) throw Error('host/UI web separation');");
    }
}

#[test]
fn development_source_maps_follow_ra3_final_map_contract() {
    let source = "function value(x) {'worklet'; return x + 1;}";
    let output = transform_fixture_resolved(
        "source-map.js",
        source,
        ReanimatedOptions {
            common: CommonOptions {
                disable_source_maps: false,
                ..options(false).common
            },
            ..options(false)
        },
    );
    execute(
        &output,
        r#"
const map = JSON.parse(value.__initData.sourceMap);
if (map.version !== 3 || !map.mappings || !map.sources.includes('source-map.js')) throw Error('source map');
// RA3 workletStringCode.ts deletes sourcesContent from the final map.
if ('sourcesContent' in map) throw Error('unexpected source content');
"#,
    );
}
