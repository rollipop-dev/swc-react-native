# swc-react-native

Collection of SWC(Rust) implementations for React Native. The worklets transform
ports the official OXC Rust plugin to SWC for Bundle Mode and shared analysis;
Legacy Eval behavior follows the current Babel plugin.

| Feature flag | Sub-crate                   | Upstream package                                                                                                                       |
| ------------ | --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `codegen`    | `swc_react_native_codegen`  | [`@react-native/babel-plugin-codegen`](https://github.com/facebook/react-native/tree/main/packages/babel-plugin-codegen)               |
| `worklets`   | `swc_react_native_worklets` | [`react-native-worklets/plugin-oxc`](https://github.com/software-mansion/react-native-reanimated/tree/main/packages/react-native-worklets/plugin-oxc) (Bundle Mode), [`plugin`](https://github.com/software-mansion/react-native-reanimated/tree/main/packages/react-native-worklets/plugin) (Legacy Eval) |

`swc_react_native` is the umbrella crate. No features are enabled by default — pick what you need
(`features = ["codegen"]`) or turn on `all` for everything. Each sub-crate can also be depended on
directly.

Version-specific Worklets Legacy Eval adapters live in
[`compat/`](crates/swc-react-native-worklets/src/compat/). The installed
`plugin_version` selects compatibility once per transform using `semver`:
stable 0.4.x–0.8.x releases use their historical bound-error global rules.
Build metadata is accepted; prereleases, unknown or invalid versions, and
Bundle Mode keep the current upstream rules. Shared analysis and SWC scope
handling stay in the upstream-named modules.

Reanimated 3 uses a separate Babel-compatible adapter in
[`compat/ra3/`](crates/swc-react-native-worklets/src/compat/ra3/mod.rs), ported from
[`react-native-reanimated@3.19.5/plugin`](https://github.com/software-mansion/react-native-reanimated/tree/3.19.5/packages/react-native-reanimated/plugin).
The RA3 modules mirror the Babel plugin's globals, automatic callbacks,
context objects, file directives, web optimization, serialized functions and
factory metadata, with its own visitor and factory. The current Worklets visitor,
factory, callback discovery and file directives retain their original implementation
apart from option access. Runtime selection happens once at the visitor boundary;
AST recursion stays inside the selected implementation. Low-level callers retain
`WorkletsVisitor::new(WorkletsOptions)` and use
`ReanimatedVisitor::new(ReanimatedOptions)` for RA3. JSON options select the runtime
by version field:

```json
{ "pluginVersion": "0.13.0" }
```

```json
{ "reanimatedVersion": "3.19.5", "processNestedWorklets": true }
```

A present `pluginVersion` selects the existing Worklets path. Otherwise,
`reanimatedVersion` must be a valid major-3 version; missing or unsupported
versions report an error without transforming the input. RA3 uses object
closures (including empty closures), its own globals and automatic callback
rules, and development `__initData.version` metadata. Its
`processNestedWorklets` option defaults to `false`, matching the RA3 Babel plugin.
RA3 does not support Bundle Mode. Babel's `extraPlugins` and `extraPresets`
cannot run inside SWC and nonempty values report an error.

Rust callers select the runtime explicitly using `Options` (plural, to avoid
shadowing Rust's standard `Option<T>`):

```rust
use swc_react_native_worklets::{Options, ReanimatedOptions, WorkletsOptions};

let reanimated = Options::Reanimated(ReanimatedOptions {
    reanimated_version: "3.19.5".into(),
    ..Default::default()
});
let worklets = Options::Worklets(WorkletsOptions {
    plugin_version: "0.13.0".into(),
    ..Default::default()
});
```

Both payloads contain `common: CommonOptions` for globals, source locations and
build settings. JSON flattens these fields, preserving the existing configuration
shape. Runtime-specific fields belong only to their corresponding payload;
version fields are strings rather than optional discriminators. `Options::default()`
selects Worklets, and `Options::validate()` checks directly constructed Rust
options. JSON must provide a version field; `pluginVersion` takes precedence
when both are supplied.

## Hermes V1 Fixes

Enable `hermes-v1-fixes` for Expo's Hermes V1 runtime fixes. Existing
`fix_async_arrow_non_simple_params()`, `fix_super_in_object_accessor()` and
`fix_class_in_finally()` passes and their visitor types remain available.

`hermes_v1_fixes_hook()` combines the three fixes into a single traversal and
can share that traversal with SWC's compatibility transforms:

```rust
use swc_ecma_hooks::CompositeHook;
use swc_ecma_transformer::{hook_pass, transform_hook, Options};
use swc_react_native::hermes_v1_fixes_hook;

let mut options = Options::default();
options.env.es2017.async_to_generator = true;
let pass = hook_pass(CompositeHook {
    first: hermes_v1_fixes_hook(),
    second: transform_hook(options),
});
```

Keep the fixes before async lowering in the composition. Run any required
block-scoping transform afterwards, as a separate pass. The hook uses the
caller's context type and does not require `swc_ecma_transformer` as a runtime
dependency of the fixes crate.

## Prerequisites

- [mise](https://mise.jdx.dev/) for environment management

## Setup

```sh
mise install
```

This installs the Rust toolchain, Node.js for runtime tests, and [just](https://github.com/casey/just) task runner as defined in `mise.toml`.

## Development

All tasks are available via `just`:

```sh
just          # List all available recipes
```

### Build

```sh
just build          # Debug build
just build-release  # Release build
```

### Lint & Format

```sh
just lint       # Run clippy with strict warnings
just fmt        # Apply rustfmt
just fmt-check  # Check formatting without applying
```

### Test

```sh
just test             # Run all tests
just snapshot-review  # Interactively review snapshot diffs (cargo insta review)
```

### All-in-one

```sh
just roll  # lint + fmt-check + test
```

## Benchmark

Historical measurements before the OXC-based worklets rewrite,
on Apple M1 Pro, 200 iterations over the per-target fixtures in
`bench/<target>/fixtures/`.

| Target     | Babel total | Babel avg / op | SWC total | SWC avg / op | Speedup  |
| ---------- | ----------: | -------------: | --------: | -----------: | -------: |
| `codegen`  |    944.9ms  |       4.724ms  |   13.1ms  |     0.066ms  | **~72x** |
| `worklets` |   4554.4ms  |      22.772ms  |   61.3ms  |     0.307ms  | **~74x** |

Run with `just bench <target> [n]` (after `just setup-bench <target>` once).

## Project Structure

```
crates/
  swc-react-native/                       # Umbrella crate — feature-gated re-exports of each transform
  swc-react-native-codegen/               # SWC visitor for the codegen transform
  swc-react-native-worklets/              # Official OXC Rust worklets plugin ported to SWC, with Legacy Eval support
```

| Upstream package                     | Rust location                     |
| ------------------------------------ | --------------------------------- |
| `@react-native/babel-plugin-codegen` | crate `swc_react_native_codegen`  |
| `react-native-worklets` OXC Rust plugin (Bundle Mode), Babel plugin (Legacy Eval) | crate `swc_react_native_worklets` |

## LICENSE

[MIT](./LICENSE)
