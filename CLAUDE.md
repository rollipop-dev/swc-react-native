# React Native Babel → SWC Porting Project

This repository ports React Native's Babel-based code transformations to Rust on top of the
`swc` ecosystem. Each upstream Babel plugin is ported to its own crate and re-exported through
the umbrella crate `swc_react_native` behind a feature flag.

## Project Structure

- The original implementations live in the GitHub submodule at `react-native/*`:
  - Source: `<submodule>/packages/<plugin-name>`
  - Existing Babel plugin tests are available and serve as the reference for expected behavior.

---

## Requirements

### Core Goal

Port React Native's Babel plugins to Rust using the `swc` ecosystem.

### Crate Layout

The project is organized by upstream package. Each Babel plugin maps to its own Rust crate;
`swc_react_native` is the umbrella crate that re-exports each transform behind a feature flag.

| Upstream package | Rust location | Crate / module | Umbrella feature |
|---|---|---|---|
| `<submodule>/packages/babel-plugin-codegen/` | `crates/swc-react-native-codegen` | crate `swc_react_native_codegen` | `codegen` |
| `<submodule>/packages/react-native-codegen/` | `crates/swc-react-native-codegen/src/codegen/` | private module `swc_react_native_codegen::codegen` | — (internal helper) |
| — | `crates/swc-react-native` | crate `swc_react_native` | (umbrella) |

#### Currently ported

- **`codegen`** — `@react-native/babel-plugin-codegen`

#### Planned

- Additional Babel plugins (e.g. worklet) will be ported into their own `swc-react-native-*`
  crates and exposed under new feature flags on the umbrella crate.

> **Note:** Each Babel plugin must be ported with **100% behavioral fidelity** to the original spec.
> `react-native-codegen` is a collection of codegen utilities; only the subset of logic actually
> used by a plugin needs to be ported. It lives as a private submodule of its consumer crate
> (e.g. `swc_react_native_codegen::codegen`) rather than as a standalone crate, since none of it
> is part of the public API.

### Public API

Each transform crate exposes an SWC `Visitor` (or equivalent entry point) following this shape:

```rust
pub fn transform() -> Result<T, Error> {
    // Code transformation via an SWC Visitor
}
```

The umbrella crate re-exports each transform under a module that matches its feature flag,
e.g. `swc_react_native::codegen`. The default feature set is empty — consumers must opt in
explicitly. The `all` feature is a convenience that pulls in every transform; new transforms
should be added to the `all` aggregate when introduced.

---

### Testing

Write tests to verify behavioral parity with the original implementation. Test cases should be derived from the existing test suite inside the submodule.

Snapshot output may not match the Babel plugin byte-for-byte due to inherent differences between Babel and swc:

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

- **Do not panic.** All recoverable error conditions must be surfaced via `Result`.
- Reserve `unreachable!()` or `panic!()` only for invariants that are genuinely impossible to violate.

---

### Maintainability

This project must remain sustainable as the upstream Babel plugins evolve. When upstream changes, those changes must be tracked and ported to Rust accordingly.

To keep this as straightforward as possible, **mirror the upstream structure as closely as Rust conventions allow**:

- Keep function names, file names, and module names aligned with the original implementation.
- When the upstream adds, removes, or renames a function or file, the corresponding change in this crate should be easy to locate and apply.
- Where a direct mapping isn't possible (e.g. due to language differences), leave a comment referencing the upstream counterpart:

```rust
// Corresponds to `myAwesomeFunction` in <upstream path>
fn my_awesome_function(...) { ... }
```

---

> **NOTE:** If there are any ambiguities or areas requiring clarification beyond what is specified here, please ask before proceeding.
