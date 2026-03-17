#!/usr/bin/env bash

set -euo pipefail

N="${1:-1000}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== Benchmark: Babel vs SWC ($N iterations) ==="
echo ""

# Check babel deps
if [ ! -f "$SCRIPT_DIR/babel/.pnp.loader.mjs" ]; then
  echo "Error: pnp manifest not found. Run 'just setup-bench' first."
  exit 1
fi

# Build SWC bench binary
echo "Building SWC bench binary (release)..."
cargo build --release --manifest-path "$SCRIPT_DIR/swc/Cargo.toml" 2>&1 | tail -1
echo ""

# Run Babel benchmark (yarn node enables PnP resolution)
echo "--- Babel ---"
(cd "$SCRIPT_DIR/babel" && yarn node run.cjs "$N")
echo ""

# Run SWC benchmark
echo "--- SWC ---"
"$SCRIPT_DIR/swc/target/release/bench_swc" "$N"
echo ""
