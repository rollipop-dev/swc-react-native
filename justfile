# List available recipes
default:
    @just --list

# Build
build:
    cargo build

# Build release
build-release:
    cargo build --release

# Run tests
test:
    cargo test

# Lint
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Format check
fmt-check:
    cargo fmt --all -- --check

# Format (apply)
fmt:
    cargo fmt --all

# Run all checks (lint + fmt + test)
check: lint fmt-check test

# Review snapshots
snapshot-review:
    cargo insta review
