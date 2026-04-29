// Port of react-native-reanimated/packages/react-native-worklets/plugin/.
// Each module references the upstream `plugin/src/*.ts` file(s) it covers.

mod closure;
mod factory;
mod gestures;
mod globals;
mod hash;
mod hooks;
mod inline_style;
mod options;
mod visitor;

pub use options::WorkletsOptions;
pub use visitor::WorkletsVisitor;
