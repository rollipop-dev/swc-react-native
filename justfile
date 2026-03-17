# List available recipes
default:
    @just --list

# Setup submodules
setup-submodule:
  git submodule update --init

# Install bench/babel dependencies
setup-bench:
    cd bench/babel && yarn install --immutable

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

# Run benchmark (Babel vs SWC)
bench n="1000":
    ./bench/run.sh {{n}}
