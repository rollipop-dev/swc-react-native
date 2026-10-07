mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::{
    options_with_version, transform_fixture, transform_fixture_resolved, transform_with_files,
};
use swc_react_native_worklets::EmittedFile;

fn execute(code: &str, probe: &str) {
    execute_modules(code, &[], probe, serde_json::json!({}));
}

fn execute_modules(code: &str, files: &[EmittedFile], probe: &str, modules: serde_json::Value) {
    let script = r#"
const { createContext, runInContext, SourceTextModule } = require('node:vm');
const { readFileSync } = require('node:fs');
const { code, probe, files, modules } = JSON.parse(readFileSync(0, 'utf8'));
const context = createContext({ __DEV__: true });
context.global = context;
const materialized = new WeakMap();
context.materialize = fn => {
  if (!fn.__initData) return fn;
  if (materialized.has(fn)) return materialized.get(fn);
  const compiled = runInContext(fn.__initData.code, context);
  const callable = function(...args) { return compiled.apply(callable, args); };
  materialized.set(fn, callable);
  callable.__closure = fn.__closure?.map(value => typeof value === 'function' ? context.materialize(value) : value);
  callable._recur = callable;
  return callable;
};
(async () => {
  const registry = new Map();
  for (const { path, content } of files) registry.set(path, new SourceTextModule(content, { context, identifier: path }));
  for (const [path, content] of Object.entries(modules)) registry.set(path, new SourceTextModule(content, { context, identifier: path }));
  context.require = path => {
    const module = registry.get(path);
    if (!module || module.status !== 'evaluated') throw new Error('unexpected require: ' + path);
    return module.namespace;
  };
  const linker = path => {
    if (!registry.has(path)) throw new Error('unexpected import: ' + path);
    return registry.get(path);
  };
  for (const module of registry.values()) if (module.status === 'unlinked') await module.link(linker);
  for (const module of registry.values()) if (module.status !== 'evaluated') await module.evaluate();
  const entry = new SourceTextModule(code + '\n' + probe, { context });
  await entry.link(linker);
  await entry.evaluate();
})().catch(error => { console.error(error); process.exitCode = 1; });
"#;
    let mut child = Command::new("node")
        .args(["--experimental-vm-modules", "--no-warnings", "-e", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Node.js is required for worklet runtime tests");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::json!({ "code": code, "probe": probe, "files": files.iter().map(|file| serde_json::json!({ "path": file.path, "content": file.content })).collect::<Vec<_>>(), "modules": modules }).to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct TestPackage(std::path::PathBuf);
impl TestPackage {
    fn new() -> Self {
        static NUMBER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir()
            .join(format!(
                "swc-worklets-{}-{}",
                std::process::id(),
                NUMBER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ))
            .join("node_modules/react-native-worklets");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join("package.json"),
            r#"{"name":"react-native-worklets","version":"0.13.0"}"#,
        )
        .unwrap();
        Self(path)
    }
}
impl Drop for TestPackage {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.0.parent().unwrap().parent().unwrap()).unwrap();
    }
}

#[test]
fn worklets_013_actual_initializers_execute_in_legacy_and_bundle_modes() {
    // Unmodified Worklets 0.13.0 initializer sources from Reanimated 4.7.1.
    let fixtures = [
        (
            "synchronizableUnpacker.native.ts",
            include_str!("fixtures/synchronizableUnpacker.native.ts"),
            r#"
globalThis.__RUNTIME_KIND = 2;
globalThis.__serializer = value => value;
globalThis.__workletsModuleProxy = {
  synchronizableGetBlocking: ref => ref.value,
  synchronizableGetDirty: ref => ref.value,
  synchronizableSetBlocking: (ref, value) => { ref.value = value; },
  synchronizableLock() {}, synchronizableUnlock() {},
};
materialize(installSynchronizableUnpacker)();
const value = __synchronizableUnpacker({ value: 10 }, false);
value.setBlocking(previous => previous + 1);
if (value.getBlocking() !== 11) throw new Error('synchronizable initializer failed');
"#,
        ),
        (
            "shareableGuestUnpacker.native.ts",
            include_str!("fixtures/shareableGuestUnpacker.native.ts"),
            r#"
globalThis.__RUNTIME_KIND = 2;
globalThis.__serializer = value => value;
globalThis.__workletsModuleProxy = {
  runOnRuntimeSyncWithId: (_, worklet) => materialize(worklet).call(worklet),
  scheduleOnRuntimeWithId: (_, worklet) => materialize(worklet).call(worklet),
};
materialize(installShareableGuestUnpacker)();
const guest = __shareableGuestUnpacker(1, { value: 10 });
if (guest.getSync() !== 10) throw new Error('shareable getter failed');
guest.setSync(20);
guest.setSync(previous => previous + 2);
if (guest.getSync() !== 22) throw new Error('shareable setter failed');
"#,
        ),
    ];
    for bundle_mode in [false, true] {
        let package = TestPackage::new();
        for (filename, source, probe) in fixtures {
            let filename = package.0.join("src/memory").join(filename);
            let mut options = options_with_version();
            options.bundle_mode = bundle_mode;
            options.plugin_version = "0.13.0".to_string();
            let (code, files) = transform_with_files(
                filename.to_str().unwrap(),
                source,
                options,
                Some(&package.0),
            )
            .unwrap();
            let probe =
                format!("globalThis._WORKLETS_BUNDLE_MODE_ENABLED = {bundle_mode};\n{probe}");
            execute_modules(
                &code,
                &files,
                &probe,
                serde_json::json!({
                    "../src/memory/serializable": "export const createSerializable = value => value;",
                    "../src/memory/serializableMappingCache": "export const serializableMappingCache = { set() {} };",
                    "../src/runtimes": "export const runOnRuntimeSyncWithId = (_, fn, ...args) => fn(...args); export const scheduleOnRuntimeWithId = (_, fn, ...args) => fn(...args); export const runOnRuntimeAsyncWithId = (_, fn, ...args) => Promise.resolve(fn(...args));"
                }),
            );
        }
    }
}

#[test]
fn bundle_mode_loads_closure_factories_and_closure_free_exports() {
    let package = TestPackage::new();
    let mut options = options_with_version();
    options.bundle_mode = true;
    let (code, files) = transform_with_files(
        "/app/test.ts",
        r#"
const value = 40;
function captured() { 'worklet'; return value + 2; }
function closureFree() { 'worklet'; return __DEV__; }
"#,
        options,
        Some(&package.0),
    )
    .unwrap();
    assert_eq!(files.len(), 2);
    assert!(!code.contains("__initData"));
    assert!(files.iter().all(|file| std::fs::read_to_string(
        package
            .0
            .join(".worklets")
            .join(file.path.rsplit('/').next().unwrap())
    )
    .unwrap()
        == file.content));
    execute_modules(&code, &files, "if (captured() !== 42 || !closureFree()) throw new Error('bundle exports failed'); if (!Array.isArray(captured.__closure)) throw new Error('closure must be an array');", serde_json::json!({}));
}

#[test]
fn bundle_mode_forwards_allowed_imports_and_captures_namespace_imports() {
    let package = TestPackage::new();
    let mut options = options_with_version();
    options.bundle_mode = true;
    options.import_forwarding.module_names = vec!["my-library".to_string()];
    let (code, files) = transform_with_files(
        "/app/test.ts",
        r#"
import { helper as read } from 'my-library';
import * as namespace from 'my-library';
function forwarded() { 'worklet'; return read(); }
function captured() { 'worklet'; return namespace.helper(); }
"#,
        options,
        Some(&package.0),
    )
    .unwrap();
    assert!(files[0].content.contains("import { helper as read }"));
    assert!(!files[1].content.contains("import *"));
    execute_modules(
        &code,
        &files,
        "if (forwarded() !== 42 || captured() !== 42) throw new Error('imports failed');",
        serde_json::json!({ "my-library": "export const helper = () => 42;" }),
    );
}

#[test]
fn bundle_mode_rebases_relative_named_imports_and_requires() {
    let package = TestPackage::new();
    let filename = package.0.parent().unwrap().join("my-library/src/test.ts");
    let mut options = options_with_version();
    options.bundle_mode = true;
    options.import_forwarding.relative_paths = vec!["my-library".to_string()];
    let (code, files) = transform_with_files(filename.to_str().unwrap(), "import {read} from './dep';function imported(){'worklet';return read();}function required(){'worklet';return require('./dep').read();}", options, Some(&package.0)).unwrap();
    assert!(files
        .iter()
        .all(|file| file.content.contains("../../my-library/src/dep")));
    execute_modules(
        &code,
        &files,
        "if(imported()!==42 || required()!==42) throw Error('relative rebasing');",
        serde_json::json!({ "./dep": "export const read = () => 42;", "../../my-library/src/dep": "export const read = () => 42;" }),
    );
}

#[test]
fn bundle_mode_discovers_package_directory_from_filename() {
    let package = TestPackage::new();
    let filename = package.0.join("src/test.ts");
    let mut options = options_with_version();
    options.bundle_mode = true;
    let (code, files) = transform_with_files(
        filename.to_str().unwrap(),
        "function f(){'worklet';return 42;}",
        options,
        None,
    )
    .unwrap();
    assert_eq!(files.len(), 1);
    execute_modules(
        &code,
        &files,
        "if(f()!==42) throw Error('package discovery');",
        serde_json::json!({}),
    );
}

#[test]
fn bundle_mode_file_write_failure_returns_error() {
    let package = TestPackage::new();
    std::fs::write(package.0.join(".worklets"), "not a directory").unwrap();
    let mut options = options_with_version();
    options.bundle_mode = true;
    let error = transform_with_files(
        "test.ts",
        "function f(){'worklet';return 42;}",
        options,
        Some(&package.0),
    )
    .unwrap_err();
    assert!(error.contains("could not create"), "{error}");
}

#[test]
fn worklet_accessors_and_object_hook_spreads_return_errors() {
    for (source, expected) in [
        (
            "const obj={get value(){'worklet';return 42;}};",
            "getter cannot be a worklet",
        ),
        (
            "const obj={set value(value){'worklet';}};",
            "setter cannot be a worklet",
        ),
        ("useAnimatedScrollHandler({...handlers});", "SpreadElement"),
    ] {
        let error =
            transform_with_files("test.ts", source, options_with_version(), None).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn runtime_bundle_flag_matches_mode_only_in_toggle_targets() {
    let package = TestPackage::new();
    let filename = package.0.join("src/index.ts");
    for bundle_mode in [false, true] {
        let mut options = options_with_version();
        options.bundle_mode = bundle_mode;
        let (code, _) = transform_with_files(
            filename.to_str().unwrap(),
            "globalThis._WORKLETS_BUNDLE_MODE_ENABLED = false;",
            options.clone(),
            Some(&package.0),
        )
        .unwrap();
        assert!(code.contains(&format!("= {bundle_mode};")));
        let (other, _) = transform_with_files(
            "/app/other.ts",
            "globalThis._WORKLETS_BUNDLE_MODE_ENABLED = false;",
            options,
            Some(&package.0),
        )
        .unwrap();
        assert!(other.contains("= false;"));
    }
}

#[test]
fn legacy_web_substitution_and_release_metadata_follow_options() {
    let mut options = options_with_version();
    options.substitute_web_platform_checks = true;
    options.is_release = true;
    let (code, _) = transform_with_files(
        "test.ts",
        "function f(){'worklet';return isWeb()&&shouldBeUseWeb();}",
        options,
        None,
    )
    .unwrap();
    assert!(!code.contains("__pluginVersion"));
    assert!(!code.contains("__stackDetails"));
    assert!(!code.contains("location:"));
    execute(
        &code,
        "if(!materialize(f)()) throw Error('web substitution');",
    );
}

#[test]
#[ignore = "requires WORKLETS_BABEL_REFERENCE and Babel dependencies in NODE_PATH"]
fn official_babel_legacy_runtime_parity() {
    let plugin = std::env::var("WORKLETS_BABEL_REFERENCE")
        .expect("set WORKLETS_BABEL_REFERENCE to the upstream Babel plugin");
    legacy_runtime_cases(Some(&plugin));
}

#[test]
fn legacy_upstream_cases_execute() {
    legacy_runtime_cases(None);
}

fn legacy_runtime_cases(plugin: Option<&str>) {
    let cases = [
        ("const x=20,y=22;function f(){'worklet';return x+y;}", "if(materialize(f)()!==42) throw Error('closure');"),
        ("function f(){'worklet';'no-worklet-closure';return __DEV__;}", "if(!materialize(f)()) throw Error('closure free');"),
        ("const x=42;function f(value=x){'worklet';return value;}", "let failed=false;try{materialize(f)();}catch(error){failed=error instanceof ReferenceError;}if(!failed) throw Error('legacy default expression must match upstream limitation');"),
        ("const x=42;function f(){'worklet';{let x=10;}return x;}", "if(materialize(f)()!==42) throw Error('block shadowing');"),
        ("function f(n){'worklet';return n<2?1:n*f(n-1);}", "if(materialize(f)(5)!==120) throw Error('recursion');"),
        ("const f=function factorial(n){'worklet';return n<2?1:n*factorial(n-1);};", "if(materialize(f)(5)!==120) throw Error('named recursion');"),
        ("function f(obj){'worklet';const get=x=>`value=${x}`;return get(obj?.value??42);}", "if(materialize(f)(null)!=='value=42') throw Error('compat passes');"),
        ("function runOnUI(fn){return fn;}const cb=()=>42;const alias=cb;const f=runOnUI(alias);", "if(materialize(f)()!==42||!f.__initData) throw Error('alias');"),
        ("function runOnUI(fn){return fn;}let cb=()=>0;cb=()=>42;const f=runOnUI(cb);", "if(materialize(f)()!==42||!f.__initData) throw Error('assignment');"),
        ("class C{__workletClass=true;value=40;get result(){return this.value+2;}}", "const Copied=materialize(C.C__classFactory)();if(new Copied().result!==42) throw Error('class fields and getter');"),
        ("'worklet';class C{value=42;}function f(){return new C().value;}", "if(materialize(f)()!==42) throw Error('file class');"),
        ("function initialize(){'worklet';'no-worklet-closure';globalThis.make=function(value){const get=()=>{'worklet';'limit-init-data-hoisting';return value;};return get;};}", "materialize(initialize)();if(materialize(make(42))()!==42) throw Error('nested hoisting');"),
    ];
    for (source, probe) in cases {
        let (code, _) =
            transform_with_files("/app/test.js", source, options_with_version(), None).unwrap();
        execute(&code, probe);
        let Some(plugin) = plugin else { continue };
        let mut child = Command::new("node").args(["-e", "const fs=require('node:fs');const i=JSON.parse(fs.readFileSync(0,'utf8'));const b=require('@babel/core');const result=b.transformSync(i.source,{filename:'/app/test.js',babelrc:false,configFile:false,envName:'development',plugins:[[require(i.plugin),{disableSourceMaps:true}]]});process.stdout.write(JSON.stringify(result.code));"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                serde_json::json!({ "plugin": plugin, "source": source })
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "source: {source}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let code: String = serde_json::from_slice(&output.stdout).unwrap();
        execute(&code, probe);
    }
}

#[test]
fn legacy_worklet_class_inheritance_and_getters_survive_serialization() {
    let (code, _) = transform_with_files("/app/test.js", "class Base{__workletClass=true;value=40;}class C extends Base{__workletClass=true;get result(){return this.value+2;}}", options_with_version(), None).unwrap();
    execute(&code, "const Copied=materialize(C.C__classFactory)();if(new Copied().result!==42) throw Error('class inheritance');");
    let (code, _) = transform_with_files(
        "/app/test.js",
        "class C{__workletClass=true;matches(value){return /\\u{1F30D}/u.test(value);}}",
        options_with_version(),
        None,
    )
    .unwrap();
    execute(&code, "const Copied=materialize(C.C__classFactory)();if(!new Copied().matches('\u{1F30D}')) throw Error('class unicode regex');");
}

#[test]
fn legacy_tagged_templates_include_helpers_and_preserve_template_identity() {
    let (code, _) = transform_with_files("test.ts", "function tag(strings,value){'worklet';return [strings,value];}function f(value){'worklet';return tag`hello ${value}`;}", options_with_version(), None).unwrap();
    execute(&code, "const compiled=materialize(f);const first=compiled(1),second=compiled(2);if(first[0]!==second[0]||second[1]!==2||first[0].raw[0]!=='hello ') throw Error('tagged template semantics');");
}

#[test]
#[ignore = "requires WORKLETS_OXC_REFERENCE pointing to a built plugin-oxc native addon"]
fn official_oxc_bundle_runtime_parity() {
    let reference_path = std::env::var("WORKLETS_OXC_REFERENCE")
        .expect("set WORKLETS_OXC_REFERENCE to the built OXC plugin");
    bundle_runtime_cases(Some(&reference_path));
}

#[test]
fn bundle_upstream_cases_execute() {
    bundle_runtime_cases(None);
}

fn bundle_runtime_cases(reference_path: Option<&str>) {
    let cases = [
        ("const x=20,y=22; function f(){'worklet';return x+y;}", "if(f()!==42 || !Array.isArray(f.__closure)) throw Error('multiple captures');"),
        ("const Math={value:42}; function f(){'worklet';return Math.value;}", "if(f()!==42) throw Error('shadowed global');"),
        ("const x=42; function f(value=x){'worklet';return value;}", "if(f()!==42) throw Error('parameter default');"),
        ("const x=42; function f(){'worklet';{let x=10;}return x;}", "if(f()!==42) throw Error('block scope');"),
        ("const f=function factorial(n){'worklet';return n<2?1:n*factorial(n-1);};", "if(f(5)!==120) throw Error('named expression recursion');"),
        ("function f(n){'worklet';return n<2?1:n*f(n-1);}", "if(f(5)!==120) throw Error('declaration recursion');"),
        ("function runOnUI(fn){return fn;} const original=()=>42;const alias=original;const f=runOnUI(alias);", "if(f()!==42 || !f.__workletHash) throw Error('alias callback');"),
        ("function runOnUI(fn){return fn;} let cb=()=>0;cb=()=>42;const f=runOnUI(cb);", "if(f()!==42 || !f.__workletHash) throw Error('rebound callback');"),
        ("function runOnUI(fn){return fn;} const f=runOnUI?.(()=>42);", "if(f()!==42 || f.__workletHash) throw Error('optional call');"),
        ("function useAnimatedScrollHandler(o){return o;} const f=useAnimatedScrollHandler({onScroll(){return 42;}});", "if(f.onScroll()!==42 || !f.onScroll.__workletHash) throw Error('object callback');"),
        ("const Gesture={Pan:()=>({onEnd:fn=>fn})}; const f=Gesture.Pan().onEnd(()=>42);", "if(f()!==42 || !f.__workletHash) throw Error('gesture callback');"),
        ("const FadeIn={duration:()=>({withCallback:fn=>fn})}; const f=FadeIn.duration(1).withCallback(()=>42);", "if(f()!==42 || !f.__workletHash) throw Error('layout callback');"),
        ("const withTiming=(a,b,c,d)=>d;const f=withTiming(1,{},undefined,()=>42);", "if(f()!==42 || !f.__workletHash) throw Error('fourth timing argument');"),
        ("'worklet';const f=()=>42;", "if(f()!==42 || !f.__workletHash) throw Error('file directive');"),
        ("'worklet';const table={f(){return 42;},nested:{g:()=>42}};", "if(table.f()!==42 || !table.nested.g.__workletHash) throw Error('file object');"),
        ("function runOnUI(fn){return fn;} const f=()=>42;f.__workletHash=1;runOnUI(f);", "if(f()!==42 || f.__workletHash!==1) throw Error('handwritten worklet');"),
        ("let key;const obj={a:42};function f(){'worklet';for(key in obj){return obj[key];}}", "if(f()!==42) throw Error('for target');"),
        ("const value=42;function f(){'worklet';'no-worklet-closure';return value;}", "if(f()!==42 || !Array.isArray(f.__closure)) throw Error('bundle ignores closure directive');"),
        ("class C{method(){'worklet';return 42;}} const f=new C().method;", "if(f()!==42 || !f.__workletHash) throw Error('class method');"),
        ("function f(){'worklet';return 'utf16: \u{1F30D}';}", "if(f()!=='utf16: \u{1F30D}') throw Error('utf16 hash input');"),
    ];
    for (source, probe) in cases {
        let package = TestPackage::new();
        let mut options = options_with_version();
        options.bundle_mode = true;
        let (code, files) =
            transform_with_files("/app/test.js", source, options, Some(&package.0)).unwrap();
        execute_modules(&code, &files, probe, serde_json::json!({}));
        let Some(reference_path) = reference_path else {
            continue;
        };
        let mut child = Command::new("node").args(["-e", "const fs=require('node:fs');const input=JSON.parse(fs.readFileSync(0,'utf8'));process.stdout.write(JSON.stringify(require(input.plugin).transform(input.source,input.filename,input.options)));"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(serde_json::json!({ "plugin": reference_path, "source": source, "filename": "/app/test.js", "options": { "pluginVersion": "test", "envName": "development" } }).to_string().as_bytes()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "source: {source}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let reference_files: Vec<_> = result["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| EmittedFile {
                path: file["path"].as_str().unwrap().to_string(),
                content: file["content"].as_str().unwrap().to_string(),
            })
            .collect();
        assert_eq!(files.len(), reference_files.len(), "source: {source}");
        execute_modules(
            result["code"].as_str().unwrap(),
            &reference_files,
            probe,
            serde_json::json!({}),
        );
    }
}

#[test]
fn rollipop_202_closure_free_initializer_executes_without_this() {
    let code = transform_fixture(
        "repro.ts",
        "function initialize() { 'worklet'; 'no-worklet-closure'; globalThis.__runtimeProbe = __DEV__; }",
        options_with_version(),
    );
    execute(&code, "materialize(initialize)(); if (__runtimeProbe !== true) throw new Error('initializer failed');");
}

#[test]
fn rollipop_202_nested_init_data_stays_in_enclosing_function() {
    let code = transform_fixture_resolved(
        "repro.ts",
        r#"
function initialize() {
  'worklet';
  'no-worklet-closure';
  globalThis.makeGuest = function(value) {
    const get = () => {
      'worklet';
      'limit-init-data-hoisting';
      return value;
    };
    return get;
  };
}
"#,
        options_with_version(),
    );
    execute(&code, "materialize(initialize)(); const guest = makeGuest(42); if (materialize(guest).call(guest) !== 42) throw new Error('nested initializer failed');");
}
