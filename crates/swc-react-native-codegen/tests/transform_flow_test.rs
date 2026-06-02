mod common;

use common::transform_fixture;

#[test]
fn test_not_a_native_component() {
    let code = r#"
const requireNativeComponent = require('requireNativeComponent').default;

export default 'Not a view config'
"#;
    let result = transform_fixture("NotANativeComponent.js", code).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn test_full_native_component() {
    let code = r#"
// @flow

const codegenNativeCommands = require('codegenNativeCommands');
const codegenNativeComponent = require('codegenNativeComponent');

import type {
  Int32,
  BubblingEventHandler,
  DirectEventHandler,
  WithDefault,
} from 'CodegenFlowtypes';
import type {NativeComponentType} from 'codegenNativeComponent';

import type {ViewProps} from 'ViewPropTypes';

type ModuleProps = $ReadOnly<{|
  ...ViewProps,

  // Props
  boolean_default_true_optional_both?: WithDefault<boolean, true>,

  // Events
  onDirectEventDefinedInlineNull: DirectEventHandler<null>,
  onBubblingEventDefinedInlineNull: BubblingEventHandler<null>,
|}>;

type NativeType = NativeComponentType<ModuleProps>;

interface NativeCommands {
  +hotspotUpdate: (viewRef: React.ElementRef<NativeType>, x: Int32, y: Int32) => void;
  +scrollTo: (viewRef: React.ElementRef<NativeType>, y: Int32, animated: boolean) => void;
}

export const Commands = codegenNativeCommands<NativeCommands>({
  supportedCommands: ['hotspotUpdate', 'scrollTo'],
});

export default codegenNativeComponent<ModuleProps>('Module', {
  interfaceOnly: true,
  paperComponentName: 'RCTModule',
});
"#;
    let result = transform_fixture("FullNativeComponent.js", code).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn test_full_typed_native_component() {
    let code = r#"
// @flow

const codegenNativeCommands = require('codegenNativeCommands');
const codegenNativeComponent = require('codegenNativeComponent');
import type {NativeComponentType} from 'codegenNativeComponent';

import type {
  Int32,
  BubblingEventHandler,
  DirectEventHandler,
  WithDefault,
} from 'CodegenFlowtypes';

import type {ViewProps} from 'ViewPropTypes';

type ModuleProps = $ReadOnly<{|
  ...ViewProps,

  // Props
  boolean_default_true_optional_both?: WithDefault<boolean, true>,

  // Events
  onDirectEventDefinedInlineNull: DirectEventHandler<null>,
  onBubblingEventDefinedInlineNull: BubblingEventHandler<null>,
|}>;

type NativeType = NativeComponentType<ModuleProps>;

interface NativeCommands {
  +hotspotUpdate: (viewRef: React.ElementRef<NativeType>, x: Int32, y: Int32) => void;
  +scrollTo: (viewRef: React.ElementRef<NativeType>, y: Int32, animated: boolean) => void;
}

export const Commands = codegenNativeCommands<NativeCommands>({
  supportedCommands: ['hotspotUpdate', 'scrollTo'],
});

export default (codegenNativeComponent<ModuleProps>('Module', {
  interfaceOnly: true,
  paperComponentName: 'RCTModule',
}): NativeType);
"#;
    let result = transform_fixture("FullTypedNativeComponent.js", code).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn test_commands_with_simple_coverage() {
    let code = r#"
// @flow

const codegenNativeCommands = require('codegenNativeCommands');
const codegenNativeComponent = require('codegenNativeComponent');

import type {ViewProps} from 'ViewPropTypes';
import type {NativeComponentType} from 'codegenNativeComponent';

type ModuleProps = $ReadOnly<{|
  ...ViewProps,
|}>;

type NativeType = NativeComponentType<ModuleProps>;

interface NativeCommands {
  +pause: (viewRef: React.ElementRef<NativeType>) => void;
  +play: (viewRef: React.ElementRef<NativeType>) => void;
}

export const Commands = (cov_1234567890.s[0]++, codegenNativeCommands<NativeCommands>({
  supportedCommands: ['pause', 'play'],
}));

export default codegenNativeComponent<ModuleProps>('Module');
"#;
    let result = transform_fixture("CommandsWithSimpleCoverageNativeComponent.js", code).unwrap();
    insta::assert_snapshot!(result);
}

// Failure tests

#[test]
fn test_commands_exported_with_different_name() {
    let code = r#"
// @flow

const codegenNativeComponent = require('codegenNativeComponent');

import type {ViewProps} from 'ViewPropTypes';
import type {NativeComponentType} from 'codegenNativeComponent';

type ModuleProps = $ReadOnly<{|
  ...ViewProps,
|}>;

type NativeType = NativeComponentType<ModuleProps>;

interface NativeCommands {
  +hotspotUpdate: (viewRef: React.ElementRef<NativeType>) => void;
}

export const Foo = codegenNativeCommands<NativeCommands>();

export default (codegenNativeComponent<ModuleProps>('Module'): NativeType);
"#;
    let result = transform_fixture("CommandsExportedWithDifferentNameNativeComponent.js", code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Native commands must be exported with the name 'Commands'"));
}

#[test]
fn test_commands_exported_with_shorthand() {
    let code = r#"
// @flow

const codegenNativeComponent = require('codegenNativeComponent');
import type {NativeComponentType} from 'codegenNativeComponent';

import type {ViewProps} from 'ViewPropTypes';

type ModuleProps = $ReadOnly<{|
  ...ViewProps,
|}>;

type NativeType = NativeComponentType<ModuleProps>;

interface NativeCommands {
  +hotspotUpdate: (viewRef: React.ElementRef<NativeType>) => void;
}

const Commands = 4;

export {Commands};

export default (codegenNativeComponent<ModuleProps>('Module'): NativeType);
"#;
    let result = transform_fixture("CommandsExportedWithShorthandNativeComponent.js", code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("'Commands' is a reserved export"));
}

#[test]
fn test_other_commands_export() {
    let code = r#"
// @flow

const codegenNativeComponent = require('codegenNativeComponent');

import type {ViewProps} from 'ViewPropTypes';
import type {NativeComponentType} from 'codegenNativeComponent';

type ModuleProps = $ReadOnly<{|
  ...ViewProps,
|}>;

type NativeType = NativeComponentType<ModuleProps>;

interface NativeCommands {
  +hotspotUpdate: (viewRef: React.ElementRef<NativeType>) => void;
}

export const Commands = 4;

export default (codegenNativeComponent<ModuleProps>('Module'): NativeType);
"#;
    let result = transform_fixture("OtherCommandsExportNativeComponent.js", code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("'Commands' is a reserved export"));
}

// Regression for facebook/react-native#55066, which migrated all
// `$ReadOnly<{| ... |}>` props in RN spec files to `Readonly<{ ... }>`.
// React Native 0.85+ ships specs in the new spelling, so the parser must
// accept both legacy and modern forms.

#[test]
fn test_modern_readonly_full_native_component() {
    let code = r#"
// @flow

const codegenNativeCommands = require('codegenNativeCommands');
const codegenNativeComponent = require('codegenNativeComponent');

import type {
  Int32,
  BubblingEventHandler,
  DirectEventHandler,
  WithDefault,
} from 'CodegenFlowtypes';
import type {NativeComponentType} from 'codegenNativeComponent';

import type {ViewProps} from 'ViewPropTypes';

type ModuleProps = Readonly<{
  ...ViewProps,

  // Props
  boolean_default_true_optional_both?: WithDefault<boolean, true>,

  // Events
  onDirectEventDefinedInlineNull: DirectEventHandler<null>,
  onBubblingEventDefinedInlineNull: BubblingEventHandler<null>,
}>;

type NativeType = NativeComponentType<ModuleProps>;

interface NativeCommands {
  +hotspotUpdate: (viewRef: React.ElementRef<NativeType>, x: Int32, y: Int32) => void;
  +scrollTo: (viewRef: React.ElementRef<NativeType>, y: Int32, animated: boolean) => void;
}

export const Commands = codegenNativeCommands<NativeCommands>({
  supportedCommands: ['hotspotUpdate', 'scrollTo'],
});

export default codegenNativeComponent<ModuleProps>('Module', {
  interfaceOnly: true,
  paperComponentName: 'RCTModule',
});
"#;
    let result = transform_fixture("ModernReadonlyFullNativeComponent.js", code).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn test_modern_readonly_minimal_view_props() {
    // Minimal repro for leegeunhyeok/rollipop#80: a spec that only spreads
    // ViewProps under `Readonly<...>`. The previous parser silently dropped
    // the extends, so the generator emitted `NativeComponentRegistry.get(...)`
    // without importing `NativeComponentRegistry`.
    let code = r#"
// @flow

import codegenNativeComponent from 'codegenNativeComponent';
import type {ViewProps} from 'ViewPropTypes';

type SwitchNativeProps = Readonly<{
  ...ViewProps,
}>;

export default codegenNativeComponent<SwitchNativeProps>('Switch');
"#;
    let result = transform_fixture("ModernReadonlyMinimalNativeComponent.js", code).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn test_flow_nullable_event_handlers() {
    let code = r#"
// @flow

import codegenNativeComponent from 'codegenNativeComponent';
import type {
  BubblingEventHandler,
  DirectEventHandler,
} from 'CodegenFlowtypes';
import type {CodegenTypes} from 'react-native';
import type {ViewProps} from 'ViewPropTypes';

type OrientationChangeEvent = Readonly<{
  orientation: 'portrait' | 'landscape',
}>;

type ModuleProps = Readonly<{
  ...ViewProps,

  onOrientationChange?: ?DirectEventHandler<OrientationChangeEvent>,
  onQualifiedOrientationChange?: ?CodegenTypes.DirectEventHandler<OrientationChangeEvent>,
  onDirectWithPaperName?: ?DirectEventHandler<
    null,
    'paperDirectWithPaperName',
  >,
  onBubbleWithPaperName?: ?BubblingEventHandler<
    null,
    'paperBubbleWithPaperName',
  >,
}>;

export default codegenNativeComponent<ModuleProps>('ModalHostView', {
  interfaceOnly: true,
  paperComponentName: 'RCTModalHostView',
});
"#;
    let result = transform_fixture("NullableEventHandlersNativeComponent.js", code).unwrap();
    insta::assert_snapshot!(result);
}
