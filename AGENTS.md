# React Native Transform Porting Project

This repository ports React Native code transformations to Rust on top of the `swc` ecosystem.
Worklets tracks the official OXC Rust plugin for Bundle Mode and shared analysis, with the
current Babel plugin as the Legacy Eval reference. Codegen tracks its upstream Babel plugin.
Each transform has its own crate and is re-exported by `swc_react_native` behind a feature flag.

## Project Structure

- Upstream sources live in GitHub submodules:
  - `react-native/` — Facebook's React Native repo. Babel plugins ported from
    here are under `<submodule>/packages/<plugin-name>`.
  - `react-native-reanimated/` — Software Mansion's reanimated repo. The
    official Rust plugin lives at `<submodule>/packages/react-native-worklets/plugin-oxc/`;
    the Legacy Eval reference lives at `<submodule>/packages/react-native-worklets/plugin/`.
- Existing upstream tests serve as the behavior reference: OXC/shared tests for Bundle Mode
  and analysis, Babel Legacy tests for Legacy Eval, and Babel codegen tests for codegen.

---

## Requirements

### Core Goal

Port React Native transforms using the `swc` ecosystem. Replace worklets analysis and Bundle
Mode logic from the official OXC Rust implementation; check Legacy Eval against Babel.

### Crate Layout

The project is organized by upstream package. Each transform maps to its own Rust crate;
`swc_react_native` is the umbrella crate that re-exports each transform behind a feature flag.

| Upstream package                                                 | Rust location                                  | Crate / module                                     | Umbrella feature    |
| ---------------------------------------------------------------- | ---------------------------------------------- | -------------------------------------------------- | ------------------- |
| `react-native/packages/babel-plugin-codegen/`                    | `crates/swc-react-native-codegen`              | crate `swc_react_native_codegen`                   | `codegen`           |
| `react-native/packages/react-native-codegen/`                    | `crates/swc-react-native-codegen/src/codegen/` | private module `swc_react_native_codegen::codegen` | — (internal helper) |
| `react-native-reanimated/packages/react-native-worklets/plugin-oxc/` (Bundle Mode), `plugin/` (Legacy Eval) | `crates/swc-react-native-worklets` | crate `swc_react_native_worklets` | `worklets` |
| —                                                                | `crates/swc-react-native`                      | crate `swc_react_native`                           | (umbrella)          |

#### Currently ported

- **`codegen`** — `@react-native/babel-plugin-codegen`
- **`worklets`** — official `react-native-worklets` OXC Rust plugin ported to SWC, with Babel-compatible Legacy Eval (from `react-native-reanimated`)

#### Planned

- Additional transforms will be ported into their own `swc-react-native-*` crates and exposed
  under new feature flags on the umbrella crate.

> **Note:** Each mode must preserve the behavioral contract of its upstream implementation.
> Worklets Bundle Mode is based on the official OXC Rust implementation; Legacy Eval is checked
> against Babel, including `no-worklet-closure` and `limit-init-data-hoisting` directives.
> `react-native-codegen` is a collection of codegen utilities; only the subset of logic actually
> used by a plugin needs to be ported. It lives as a private submodule of its consumer crate
> (e.g. `swc_react_native_codegen::codegen`) rather than as a standalone crate, since none of it
> is part of the public API.

### Public API

Each transform crate exposes a free function returning `impl Pass`, mirroring
the `swc_ecma_compat_*::private_in_object()` convention. Options are passed as
arguments; the underlying `VisitMut` type stays internal (kept `pub` but
`#[doc(hidden)]` so advanced users that need fine-grained access — e.g.
codegen's `into_result()` — still have an escape hatch).

```rust
pub fn transform_name(/* cm, options, ... */) -> impl Pass { ... }
```

The umbrella crate re-exports each transform under a module that matches its feature flag,
e.g. `swc_react_native::codegen::codegen(...)`. The default feature set is empty — consumers
must opt in explicitly. The `all` feature is a convenience that pulls in every transform; new
transforms should be added to the `all` aggregate when introduced.

---

### Testing

Derive tests from the matching upstream suite inside the submodule. Verify generated Bundle
Mode modules and serialized Legacy Eval functions by executing them, including actual runtime
initializers. Optional reference comparisons use independently built OXC and Babel plugins.

Snapshot output may not match OXC or Babel byte-for-byte due to differences in SWC emission:

- Mismatches caused by **code indentation** or **identifier naming conventions** are acceptable — update snapshots to reflect swc-based output.
- All **business logic behavior** must be 100% compatible with the original.

#### Test Infrastructure

Use a **fixture-based snapshot testing** approach that mirrors the structure of the original Babel test suite.

**Recommended crate:** [`insta`](https://crates.io/crates/insta) for snapshot management.

**Snapshot update policy:**

- Run `cargo insta review` to review and accept snapshot diffs interactively.
- Accept only diffs that are clearly cosmetic (indentation, identifier casing).
- Reject and fix any diffs that reflect logic differences.
- Accepted swc-based snapshots become the source of truth going forward.

**Error handling guidelines:**

- **Do not panic.** Surface recoverable failures through `Result` and SWC diagnostics without changing the existing `impl Pass` interface. Failed transforms leave the input unchanged.
- Reserve `unreachable!()` or `panic!()` only for invariants that are genuinely impossible to violate.

---

### Maintainability

Track the official OXC Rust worklets implementation, its Babel Legacy Eval adapter, and React
Native codegen as they evolve. Update submodules to the latest stable release and port behavior
changes from the source corresponding to each mode.

To keep this as straightforward as possible, **mirror the upstream structure as closely as Rust conventions allow**:

- Keep function names, file names, and module names aligned with the official Rust implementation for shared worklets logic. Keep Legacy-only adapters tied to their Babel counterparts.
- When the upstream adds, removes, or renames a function or file, the corresponding change in this crate should be easy to locate and apply.
- Where a direct mapping isn't possible (e.g. due to language differences), leave a comment referencing the upstream counterpart:

```rust
// Port of `make_worklet_factory` in plugin-oxc/src/worklet_factory.rs.
fn make_worklet_factory(...) { ... }
```

---

> **NOTE:** If there are any ambiguities or areas requiring clarification beyond what is specified here, please ask before proceeding.
