// Corresponds to `globals.ts` in
// react-native-reanimated/packages/react-native-worklets/plugin/src/.

use std::collections::HashSet;

use once_cell::sync::Lazy;

/// Identifiers that are never captured as closure variables.
/// Based on MDN global objects list plus React Native / Hermes globals.
pub static DEFAULT_GLOBALS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    for g in GLOBAL_IDENTIFIERS {
        s.insert(*g);
    }
    s
});

/// Identifiers that must be resolved through the worklet runtime's
/// global scope even when a file-level binding of the same name exists.
/// These are the union of `outsideBindingsToCaptureFromGlobalScope`
/// (e.g. `ReanimatedError`) and `internalBindingsToCaptureFromGlobalScope`
/// (e.g. `WorkletsError`) from the upstream babel plugin
/// (`react-native-worklets/plugin/src/globals.ts`). The runtime
/// registers these names onto `globalThis` (see
/// `registerReanimatedError` in `react-native-reanimated/common/errors.ts`),
/// so capturing them into a worklet's `__closure` would shadow the
/// registered factory and break the runtime's expectations — most
/// visibly as `ReanimatedError is not a function` from any code that
/// `throw new ReanimatedError(...)` inside a worklet.
pub static FORCE_SKIP_CAPTURE: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    for g in OUTSIDE_BINDINGS_CAPTURED_FROM_GLOBAL_SCOPE {
        s.insert(*g);
    }
    for g in INTERNAL_BINDINGS_CAPTURED_FROM_GLOBAL_SCOPE {
        s.insert(*g);
    }
    s
});

/// `outsideBindingsToCaptureFromGlobalScope` in
/// `react-native-worklets/plugin/src/globals.ts`.
const OUTSIDE_BINDINGS_CAPTURED_FROM_GLOBAL_SCOPE: &[&str] = &["ReanimatedError"];

/// `internalBindingsToCaptureFromGlobalScope` in the upstream plugin.
const INTERNAL_BINDINGS_CAPTURED_FROM_GLOBAL_SCOPE: &[&str] = &["WorkletsError"];

const GLOBAL_IDENTIFIERS: &[&str] = &[
    // Value properties
    "globalThis",
    "Infinity",
    "NaN",
    "undefined",
    // Function properties
    "eval",
    "isFinite",
    "isNaN",
    "parseFloat",
    "parseInt",
    "decodeURI",
    "decodeURIComponent",
    "encodeURI",
    "encodeURIComponent",
    "escape",
    "unescape",
    // Fundamental objects
    "Object",
    "Function",
    "Boolean",
    "Symbol",
    // Error objects
    "Error",
    "AggregateError",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
    "InternalError",
    // Numbers and dates
    "Number",
    "BigInt",
    "Math",
    "Date",
    // Text processing
    "String",
    "RegExp",
    // Indexed collections
    "Array",
    "Int8Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "Int16Array",
    "Uint16Array",
    "Int32Array",
    "Uint32Array",
    "BigInt64Array",
    "BigUint64Array",
    "Float32Array",
    "Float64Array",
    // Keyed collections
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    // Structured data
    "ArrayBuffer",
    "SharedArrayBuffer",
    "DataView",
    "Atomics",
    "JSON",
    // Managing memory
    "WeakRef",
    "FinalizationRegistry",
    // Control abstraction objects
    "Iterator",
    "AsyncIterator",
    "Promise",
    "GeneratorFunction",
    "AsyncGeneratorFunction",
    "Generator",
    "AsyncGenerator",
    "AsyncFunction",
    // Reflection
    "Reflect",
    "Proxy",
    // Internationalization
    "Intl",
    // Other stuff
    "null",
    "this",
    "global",
    "window",
    "self",
    "console",
    "performance",
    "arguments",
    "require",
    "fetch",
    "XMLHttpRequest",
    "WebSocket",
    // Run loop
    "queueMicrotask",
    "requestAnimationFrame",
    "cancelAnimationFrame",
    "setTimeout",
    "clearTimeout",
    "setImmediate",
    "clearImmediate",
    "setInterval",
    "clearInterval",
    // Hermes
    "HermesInternal",
    // Worklets
    "_WORKLET",
    // Deprecated
    "_IS_FABRIC",
];
