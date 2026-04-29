# swc-react-native

Collection of SWC(Rust) implementations for React Native.

| Feature flag | Sub-crate                   | Upstream package                                                                                                                       |
| ------------ | --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `codegen`    | `swc_react_native_codegen`  | [`@react-native/babel-plugin-codegen`](https://github.com/facebook/react-native/tree/main/packages/babel-plugin-codegen)               |
| `worklets`   | `swc_react_native_worklets` | [`react-native-worklets/plugin`](https://github.com/software-mansion/react-native-reanimated/tree/main/packages/react-native-worklets) |

`swc_react_native` is the umbrella crate. No features are enabled by default — pick what you need
(`features = ["codegen"]`) or turn on `all` for everything. Each sub-crate can also be depended on
directly.

## Prerequisites

- [mise](https://mise.jdx.dev/) for environment management

## Setup

```sh
mise install
```

This installs the Rust toolchain and [just](https://github.com/casey/just) task runner as defined in `mise.toml`.

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

Measured on Apple M1 Pro, 100 iterations over a `bench/fixtures` containing TypeScript/Flow native component definitions.

|                                    |      Total | Avg / transform |  Speedup |
| ---------------------------------- | ---------: | --------------: | -------: |
| @react-native/babel-plugin-codegen |   1641.7ms |         1.642ms |       1x |
| **swc_react_native::codegen**      | **35.7ms** |     **0.036ms** | **~46x** |

## Project Structure

```
crates/
  swc-react-native/                       # Umbrella crate — feature-gated re-exports of each transform
  swc-react-native-codegen/               # SWC visitor for the codegen transform
  swc-react-native-worklets/              # SWC visitor for the worklets transform
```

| Upstream package                     | Rust location                     |
| ------------------------------------ | --------------------------------- |
| `@react-native/babel-plugin-codegen` | crate `swc_react_native_codegen`  |
| `react-native-worklets` Babel plugin | crate `swc_react_native_worklets` |

## LICENSE

[MIT](./LICENSE)
