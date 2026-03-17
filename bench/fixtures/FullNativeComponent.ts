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
