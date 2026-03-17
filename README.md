# swc-plugin-codegen

> [!WARNING]
> This is an **in-development** Rust port of `@react-native/babel-plugin-codegen`, built on a **pre-release** version of SWC with Flow parsing support. It is not yet published to crates.io or ready for production use.

Rust/SWC port of [`@react-native/babel-plugin-codegen`](https://github.com/facebook/react-native/tree/main/packages/babel-plugin-codegen) from the [react-native](https://github.com/facebook/react-native) repository.

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

## Project Structure

```
crates/
  react-native-codegen/   # Schema types, parsers (Flow/TS), view config generator
  swc-plugin-codegen/     # SWC visitor — public transform() entry point
swc/                      # SWC submodule (pre-release, path dependencies)
react-native/             # React Native submodule (upstream reference)
```

| Upstream package | Rust crate |
|---|---|
| `@react-native/babel-plugin-codegen` | `swc_plugin_codegen` |
| `@react-native/codegen` | `react_native_codegen` |

## LICENSE

[MIT](./LICENSE)
