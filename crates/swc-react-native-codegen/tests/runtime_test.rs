mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_ast::{Decl, ModuleDecl, ModuleItem, Stmt};
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_parser::{parse_file_as_module, Syntax, TsSyntax};

fn runtime_fixture(typescript: bool) -> String {
    let props = r#"
  color?: ColorValue;
  colors?: ReadonlyArray<ColorValue>;
  processedColor?: ProcessedColorValue;
  processedColors?: ReadonlyArray<ProcessedColorValue>;
  aliasColor?: Tint;
  defaultColors?: WithDefault<Palette, null>;
  aliasImage?: Image;
  image?: ImageSource;
  point?: PointValue;
  insets?: EdgeInsetsValue;
  dimension?: DimensionValue;
  enabled?: boolean;
  title?: string;
  count?: Int32;
  onChange?: DirectEventHandler<null>;
  onBubble?: BubblingEventHandler<null, 'paperBubble'>;
"#;
    let props = if typescript {
        format!("interface NativeProps extends ViewProps {{ {props} }}")
    } else {
        format!("type NativeProps = Readonly<{{ ...ViewProps, {props} }}>;")
    };
    let source = format!(
        r#"
import type {{ViewProps, HostComponent}} from 'react-native';
import type {{ColorValue, ProcessedColorValue, ImageSource, PointValue, EdgeInsetsValue, DimensionValue,
  Int32, DirectEventHandler, BubblingEventHandler, WithDefault}} from 'CodegenTypes';
type Tint = ColorValue;
type Palette = ReadonlyArray<Tint>;
type Image = ImageSource;
{props}
interface NativeCommands {{
  update: (viewRef: React.ElementRef<HostComponent<NativeProps>>, count: Int32, enabled: boolean) => void;
  reset: (viewRef: React.ElementRef<HostComponent<NativeProps>>) => void;
}}
export const Commands = codegenNativeCommands<NativeCommands>({{
  supportedCommands: ['update', 'reset'],
}});
export default codegenNativeComponent<NativeProps>('RuntimeView', {{
  paperComponentNameDeprecated: 'LegacyRuntimeView',
}});
"#,
    );
    let filename = if typescript {
        "RuntimeNativeComponent.ts"
    } else {
        "RuntimeNativeComponent.js"
    };
    let transformed = common::transform_fixture(filename, &source).unwrap();

    // The transform preserves source types. Remove only the fixture's type
    // imports/declarations so Node executes the generated module unchanged.
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Custom(filename.into()).into(), transformed);
    let mut module = parse_file_as_module(
        &fm,
        Syntax::Typescript(TsSyntax::default()),
        Default::default(),
        None,
        &mut vec![],
    )
    .unwrap();
    module.body.retain(|item| {
        !matches!(
            item,
            ModuleItem::ModuleDecl(ModuleDecl::Import(_))
                | ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(_) | Decl::TsTypeAlias(_)))
        )
    });
    let mut output = Vec::new();
    Emitter {
        cfg: Default::default(),
        cm: cm.clone(),
        comments: None,
        wr: JsWriter::new(cm, "\n", &mut output, None),
    }
    .emit_module(&module)
    .unwrap();
    String::from_utf8(output).unwrap()
}

fn execute(typescript: bool, versions: &[&str], available: Option<&str>) {
    // Runtime contracts follow React Native v0.84.0/v0.84.1, v0.85.3, and the
    // pinned upstream v0.87.1. Mocks isolate Metro and native dependencies.
    let script = r#"
const assert = require('node:assert/strict');
const {readFileSync} = require('node:fs');
const {createContext, SourceTextModule} = require('node:vm');
const {code, versions, available} = JSON.parse(readFileSync(0, 'utf8'));
const plain = value => JSON.parse(JSON.stringify(value));
(async () => {
  const scenarios = versions.flatMap(version =>
    ['ios', 'android'].map(platform => ({version, platform})));
  for (const {version, platform} of scenarios) {
    const required = [];
    const calls = [];
    const managers = [];
    const registered = [];
    const wrap = value => version.endsWith('default') ? {default: value} : value;
    const color = value => {calls.push(['color', value]); return `color:${value}`;};
    const colors = value => {calls.push(['colors', value]); return value.map(color);};
    const image = value => {calls.push(['image', value]); return {uri: `asset:${value}`};};
    const point = (a, b) => {calls.push(['point', a, b]); return a.x !== b.x || a.y !== b.y;};
    const insets = (a, b) => {calls.push(['insets', a, b]); return a.top !== b.top;};
    const colorAttribute = version.endsWith('-native') ? true : {process: color};
    const modules = {
      'react-native': {UIManager: {
        hasViewManagerConfig: name => {managers.push(name); return name === available;},
      }},
      'react-native/Libraries/NativeComponent/NativeComponentRegistry': {
        get: (name, getConfig) => {
          const component = {name, getConfig};
          registered.push(component);
          return component;
        },
      },
      'react-native/Libraries/NativeComponent/ViewConfigIgnore': {
        ConditionallyIgnoredEventHandlers: events => {
          calls.push(['events', events]);
          return platform === 'ios' ? events : undefined;
        },
      },
      'react-native/Libraries/ReactNative/RendererProxy': {
        dispatchCommand: (...args) => calls.push(['command', ...args]),
      },
      'react-native/Libraries/Components/View/ReactNativeStyleAttributes':
        version.startsWith('84') ? wrap({color: {process: color}}) : {colorAttribute},
      'react-native/Libraries/StyleSheet/processColorArray': wrap(colors),
      'react-native/Libraries/Image/resolveAssetSource': wrap(image),
      'react-native/Libraries/Utilities/differ/pointsDiffer': wrap(point),
      'react-native/Libraries/Utilities/differ/insetsDiffer': wrap(insets),
    };
    if (version.startsWith('84')) {
      modules['react-native/Libraries/StyleSheet/processColor'] = wrap(color);
    }
    const module = new SourceTextModule(code, {context: createContext({
      require: name => {
        required.push(name);
        assert.ok(Object.hasOwn(modules, name), `unexpected require: ${name} (${version})`);
        return modules[name];
      },
    })});
    await module.link(name => {throw new Error(`unexpected import: ${name}`);});
    if (available === null) {
      await assert.rejects(module.evaluate(), /Failed to find native component/);
      assert.deepEqual(managers, ['RuntimeView', 'LegacyRuntimeView']);
      assert.equal(registered.length, 0);
      continue;
    }
    await module.evaluate();
    const {__INTERNAL_VIEW_CONFIG: config, default: component, Commands} = module.namespace;
    assert.equal(registered.length, 1);
    assert.equal(component, registered[0]);
    assert.equal(component.name, available);
    assert.equal(component.getConfig(), config);
    assert.equal(config.uiViewClassName, 'RuntimeView');
    assert.deepEqual(managers, available === 'RuntimeView' ? ['RuntimeView'] : ['RuntimeView', 'LegacyRuntimeView']);

    const attrs = config.validAttributes;
    if (!version.startsWith('84')) {
      assert.equal(attrs.color, colorAttribute);
      assert.equal(attrs.processedColor, colorAttribute);
      assert.equal(attrs.aliasColor, colorAttribute);
    }
    if (!version.endsWith('-native')) {
      assert.equal(attrs.color.process, color, `missing color processor on RN ${version}`);
      assert.equal(attrs.color.process('red'), 'color:red');
      assert.equal(attrs.processedColor.process, color);
      assert.equal(attrs.processedColor.process(123), 'color:123');
      assert.equal(attrs.aliasColor.process, color);
      assert.equal(attrs.aliasColor.process('blue'), 'color:blue');
    }
    assert.equal(required.includes('react-native/Libraries/StyleSheet/processColor'), version.startsWith('84'));
    assert.equal(attrs.colors.process, colors);
    assert.deepEqual(attrs.colors.process(['red', 'blue']), ['color:red', 'color:blue']);
    assert.equal(attrs.processedColors.process, colors);
    assert.deepEqual(attrs.processedColors.process([123, 456]), ['color:123', 'color:456']);
    assert.equal(attrs.defaultColors.process, colors);
    assert.deepEqual(attrs.defaultColors.process(['red']), ['color:red']);
    assert.equal(attrs.image.process, image);
    assert.deepEqual(attrs.image.process(42), {uri: 'asset:42'});
    assert.equal(attrs.aliasImage.process, image);
    assert.deepEqual(attrs.aliasImage.process(7), {uri: 'asset:7'});
    assert.equal(attrs.point.diff, point);
    assert.equal(attrs.point.diff({x: 1, y: 2}, {x: 1, y: 2}), false);
    assert.equal(attrs.point.diff({x: 1, y: 2}, {x: 2, y: 2}), true);
    assert.equal(attrs.insets.diff, insets);
    assert.equal(attrs.insets.diff({top: 1}, {top: 1}), false);
    assert.equal(attrs.insets.diff({top: 1}, {top: 2}), true);
    for (const prop of ['dimension', 'enabled', 'title', 'count']) {
      assert.equal(attrs[prop], true, prop);
    }
    for (const event of ['onChange', 'onBubble']) {
      assert.equal(Object.hasOwn(attrs, event), platform === 'ios', `${event} on ${platform}`);
      assert.equal(attrs[event], platform === 'ios' ? true : undefined);
    }
    assert.deepEqual(plain(config.directEventTypes), {topChange: {registrationName: 'onChange'}});
    assert.deepEqual(plain(config.bubblingEventTypes), {
      topPaperBubble: {phasedRegistrationNames: {captured: 'onBubbleCapture', bubbled: 'onBubble'}},
    });
    assert.deepEqual(plain(calls.find(call => call[0] === 'events')), ['events', {onChange: true, onBubble: true}]);
    const ref = {nativeTag: 7};
    Commands.update(ref, 3, false);
    Commands.reset(ref);
    assert.equal(calls.at(-2)[1], ref);
    assert.deepEqual(plain(calls.slice(-2)), [
      ['command', ref, 'update', [3, false]],
      ['command', ref, 'reset', []],
    ]);
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
"#;
    let mut child = Command::new("node")
        .args(["--experimental-vm-modules", "--no-warnings", "-e", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Node.js is required for codegen runtime tests");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            serde_json::json!({
                "code": runtime_fixture(typescript),
                "versions": versions,
                "available": available,
            })
            .to_string()
            .as_bytes(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn flow_view_config_runs_on_rn84_through_current_upstream() {
    execute(
        false,
        &[
            "84-default",
            "84-commonjs",
            "85-default",
            "85-js",
            "85-native",
            "87-default",
            "87-js",
            "87-native",
        ],
        Some("RuntimeView"),
    );
}

#[test]
fn typescript_view_config_runs_on_rn84_through_current_upstream() {
    execute(
        true,
        &[
            "84-default",
            "84-commonjs",
            "85-default",
            "85-js",
            "85-native",
            "87-default",
            "87-js",
            "87-native",
        ],
        Some("RuntimeView"),
    );
}

#[test]
fn deprecated_component_name_fallback_and_missing_component() {
    for typescript in [false, true] {
        execute(
            typescript,
            &["84-default", "85-native", "87-native"],
            Some("LegacyRuntimeView"),
        );
        execute(typescript, &["84-default", "85-native", "87-native"], None);
    }
}
