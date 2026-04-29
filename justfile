# List available recipes
default:
    @just --list

# Install babel deps for a bench target (codegen | worklets)
setup-bench target="codegen":
    cd bench/{{target}}/babel && yarn install --immutable

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
roll: lint fmt-check test

# Review snapshots
snapshot-review:
    cargo insta review

# Run benchmark for a target (codegen | worklets)
bench target="codegen" n="1000":
    ./bench/run.sh {{target}} {{n}}
