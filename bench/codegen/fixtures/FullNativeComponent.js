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

export default (codegenNativeComponent<ModuleProps>('Module', {
  interfaceOnly: true,
  paperComponentName: 'RCTModule',
}): NativeType);
