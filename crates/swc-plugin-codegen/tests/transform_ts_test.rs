/// TypeScript transform tests — mirrors the Flow tests in transform_test.rs
/// with idiomatic TypeScript syntax (interface extends, `as` casts, etc.)
mod common;

use common::transform_fixture;

// ---------- Success cases ----------

#[test]
fn test_ts_not_a_native_component() {
    let code = r#"
const requireNativeComponent = require('requireNativeComponent').default;

export default 'Not a view config';
"#;
    let result = transform_fixture("NotANativeComponent.ts", code).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn test_ts_full_native_component() {
    let code = r#"
import type {
  Int32,
  BubblingEventHandler,
  DirectEventHandler,
  WithDefault,
} from 'CodegenTypes';
import type {HostComponent} from 'react-native';
import type {ViewProps} from 'ViewPropTypes';

const codegenNativeCommands = require('codegenNativeCommands');
const codegenNativeComponent = require('codegenNativeComponent');

export interface ModuleProps extends ViewProps {
  // Props
  boolean_default_true_optional_both?: WithDefault<boolean, true>;

  // Events
  onDirectEventDefinedInlineNull: DirectEventHandler<null>;
  onBubblingEventDefinedInlineNull: BubblingEventHandler<null>;
}

type NativeType = HostComponent<ModuleProps>;

interface NativeCommands {
  readonly hotspotUpdate: (viewRef: React.ComponentRef<NativeType>, x: Int32, y: Int32) => void;
  readonly scrollTo: (viewRef: React.ComponentRef<NativeType>, y: Int32, animated: boolean) => void;
}

export const Commands = codegenNativeCommands<NativeCommands>({
  supportedCommands: ['hotspotUpdate', 'scrollTo'],
});

export default codegenNativeComponent<ModuleProps>('Module', {
  interfaceOnly: true,
  paperComponentName: 'RCTModule',
}) as NativeType;
"#;
    let result = transform_fixture("FullNativeComponent.ts", code).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn test_ts_simple_component_no_commands() {
    let code = r#"
import type {ViewProps} from 'ViewPropTypes';
import type {HostComponent} from 'react-native';

const codegenNativeComponent = require('codegenNativeComponent');

export interface ModuleProps extends ViewProps {}

export default codegenNativeComponent<ModuleProps>('Module') as HostComponent<ModuleProps>;
"#;
    let result = transform_fixture("CommandsWithSimpleCoverageNativeComponent.ts", code).unwrap();
    insta::assert_snapshot!(result);
}

/// Regression test: qualified-name event handler types like
/// `CT.DirectEventHandler<E>` (where `CT` aliases `CodegenTypes` via
/// `import type { CodegenTypes as CT }`) must be recognised as event
/// handlers, not silently demoted to ordinary props.
///
/// Failure mode before the fix: `extract_type_info` returned `"unknown"`
/// for any `TsQualifiedName`, so `try_extract_event` never matched and
/// every handler was emitted under `validAttributes` only — leaving
/// `directEventTypes` / `bubblingEventTypes` empty. At runtime React
/// Native could not map the native event back to its `onX` prop and
/// threw `Unsupported top level event type "topX" dispatched`.
#[test]
fn test_ts_qualified_event_handler() {
    let code = r#"
import type { CodegenTypes as CT } from 'react-native';
import type { ViewProps } from 'ViewPropTypes';

const codegenNativeComponent = require('codegenNativeComponent');

type HeaderHeightChangeEvent = Readonly<{ headerHeight: CT.Double }>;
type ScreenEvent = Readonly<{}>;

export interface ModuleProps extends ViewProps {
  onHeaderHeightChange?: CT.DirectEventHandler<HeaderHeightChangeEvent>;
  onAppear?: CT.DirectEventHandler<ScreenEvent>;
  onPress?: CT.BubblingEventHandler<ScreenEvent>;
}

export default codegenNativeComponent<ModuleProps>('Module', {
  interfaceOnly: true,
});
"#;
    let result = transform_fixture("QualifiedEventHandlerNativeComponent.ts", code).unwrap();
    insta::assert_snapshot!(result);
}

/// Regression test: an interface that extends multiple LOCAL interfaces
/// (in addition to the built-in `ViewProps`) must flatten every
/// inherited property into the generated `validAttributes`.
///
/// Failure mode before the fix: `extract_interface_extends` only
/// recognised `ViewProps` and silently dropped every other extends
/// clause, so props inherited from local helper interfaces (e.g.
/// `SvgNodeCommonProps`, `SvgRenderableCommonProps` in
/// `react-native-svg`) never made it into the view config. The native
/// side then ignored those props and the component rendered as an
/// invisible 0×0 shape.
#[test]
fn test_ts_extends_local_interfaces() {
    let code = r#"
import type { CodegenTypes as CT } from 'react-native';
import type { ViewProps } from 'ViewPropTypes';

const codegenNativeComponent = require('codegenNativeComponent');

interface SvgNodeCommonProps {
  name?: string;
  opacity?: CT.Float;
  matrix?: ReadonlyArray<CT.Float>;
}

interface SvgRenderableCommonProps {
  fill?: string;
  fillOpacity?: CT.Float;
  stroke?: string;
  strokeWidth?: CT.Float;
}

interface ModuleProps
  extends ViewProps,
    SvgNodeCommonProps,
    SvgRenderableCommonProps {
  cx?: CT.Float;
  cy?: CT.Float;
  r?: CT.Float;
  onSvgLayout?: CT.DirectEventHandler<Readonly<{ width: CT.Int32 }>>;
}

export default codegenNativeComponent<ModuleProps>('RNSVGCircle', {
  interfaceOnly: true,
});
"#;
    let result = transform_fixture("LocalExtendsNativeComponent.ts", code).unwrap();
    insta::assert_snapshot!(result);
}

/// A locally extended interface may itself extend another local
/// interface — the flattening must recurse through the whole chain.
#[test]
fn test_ts_extends_recursive_chain() {
    let code = r#"
import type { ViewProps } from 'ViewPropTypes';

const codegenNativeComponent = require('codegenNativeComponent');

interface BaseProps {
  base?: boolean;
}

interface MidProps extends BaseProps {
  mid?: string;
}

interface ModuleProps extends ViewProps, MidProps {
  leaf?: number;
}

export default codegenNativeComponent<ModuleProps>('Module');
"#;
    let result = transform_fixture("RecursiveExtendsNativeComponent.ts", code).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn test_ts_commands_with_coverage() {
    let code = r#"
import type {ViewProps} from 'ViewPropTypes';
import type {HostComponent} from 'react-native';

const codegenNativeCommands = require('codegenNativeCommands');
const codegenNativeComponent = require('codegenNativeComponent');

export interface ModuleProps extends ViewProps {}

type NativeType = HostComponent<ModuleProps>;

interface NativeCommands {
  readonly pause: (viewRef: React.ComponentRef<NativeType>) => void;
  readonly play: (viewRef: React.ComponentRef<NativeType>) => void;
}

export const Commands = (cov_1234567890.s[0]++, codegenNativeCommands<NativeCommands>({
  supportedCommands: ['pause', 'play'],
}));

export default codegenNativeComponent<ModuleProps>('Module') as NativeType;
"#;
    let result = transform_fixture("CommandsWithCoverageNativeComponent.ts", code).unwrap();
    insta::assert_snapshot!(result);
}

// ---------- Failure cases ----------

#[test]
fn test_ts_commands_exported_with_different_name() {
    let code = r#"
import type {ViewProps} from 'ViewPropTypes';
import type {HostComponent} from 'react-native';

const codegenNativeComponent = require('codegenNativeComponent');

export interface ModuleProps extends ViewProps {}

type NativeType = HostComponent<ModuleProps>;

interface NativeCommands {
  readonly hotspotUpdate: (viewRef: React.ComponentRef<NativeType>) => void;
}

export const Foo = codegenNativeCommands<NativeCommands>();

export default codegenNativeComponent<ModuleProps>('Module') as NativeType;
"#;
    let result = transform_fixture("CommandsExportedWithDifferentNameNativeComponent.ts", code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Native commands must be exported with the name 'Commands'"));
}

#[test]
fn test_ts_other_commands_export() {
    let code = r#"
import type {ViewProps} from 'ViewPropTypes';
import type {HostComponent} from 'react-native';

const codegenNativeComponent = require('codegenNativeComponent');

export interface ModuleProps extends ViewProps {}

type NativeType = HostComponent<ModuleProps>;

export const Commands = 4;

export default codegenNativeComponent<ModuleProps>('Module') as NativeType;
"#;
    let result = transform_fixture("OtherCommandsExportNativeComponent.ts", code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("'Commands' is a reserved export"));
}

#[test]
fn test_ts_commands_exported_with_shorthand() {
    let code = r#"
import type {ViewProps} from 'ViewPropTypes';
import type {HostComponent} from 'react-native';

const codegenNativeComponent = require('codegenNativeComponent');

export interface ModuleProps extends ViewProps {}

type NativeType = HostComponent<ModuleProps>;

const Commands = 4;

export {Commands};

export default codegenNativeComponent<ModuleProps>('Module') as NativeType;
"#;
    let result = transform_fixture("CommandsExportedWithShorthandNativeComponent.ts", code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("'Commands' is a reserved export"));
}
