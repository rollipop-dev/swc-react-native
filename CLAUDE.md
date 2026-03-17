# React Native Codegen Plugin — Rust Porting Project

## Project Structure

- This is a fresh project. Start by creating a new Cargo library crate.
- The original plugin implementation lives in the GitHub submodule at `react-native/*`:
  - Source: `<submodule>/packages/babel-plugin-codegen`
  - Existing Babel plugin tests are available and serve as the reference for expected behavior.
- The swc Rust crates are sourced from a submodule (pre-release, not yet published to crates.io), located at `swc/*`:
  - This submodule tracks an upcoming swc release that includes Flow parsing support.

---

## Requirements

### Core Goal

Port the `@react-native/babel-plugin-codegen` Babel plugin to Rust using the `swc` ecosystem.

> **All swc crate versions must be pinned to the submodule version.**

### Crate Layout

The project is organized by upstream package:

| Upstream package | Rust crate path | Crate name |
|---|---|---|
| `<submodule>/packages/babel-plugin-codegen/` | `crates/swc-plugin-codegen` | `swc_plugin_codegen` |
| `<submodule>/packages/react-native-codegen/` | `crates/react-native-codegen` | `react_native_codegen` |

> **Note:** The Babel plugin must be ported with **100% behavioral fidelity** to the original spec. `react-native-codegen` is a collection of codegen utilities; only the subset of logic actually used by the plugin needs to be ported.

### Public API

The externally facing crate is `crates/swc-plugin-codegen`. Its top-level entry point should follow this shape:

```rust
pub fn transform() -> Result<T, Error> {
    // Code transformation via an SWC Visitor
}
```

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

This project must remain sustainable as the upstream Babel plugin evolves. When the upstream changes, those changes must be tracked and ported to Rust accordingly.

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

