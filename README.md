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
