#!/usr/bin/env bash

set -euo pipefail

TARGET="${1:-codegen}"
N="${2:-1000}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TARGET_DIR="$SCRIPT_DIR/$TARGET"

if [ ! -d "$TARGET_DIR" ]; then
  echo "Error: unknown bench target '$TARGET'. Expected one of: $(ls "$SCRIPT_DIR" | grep -v '\.sh\|fixtures' | tr '\n' ' ')"
  exit 1
fi

echo "=== Benchmark: $TARGET — Babel vs SWC ($N iterations) ==="
echo ""

if [ ! -f "$TARGET_DIR/babel/.pnp.loader.mjs" ] && [ ! -d "$TARGET_DIR/babel/node_modules" ]; then
  echo "Error: babel deps for '$TARGET' not installed. Run 'just setup-bench $TARGET' first."
  exit 1
fi

echo "Building SWC bench binary for $TARGET (release)..."
cargo build --release --manifest-path "$TARGET_DIR/swc/Cargo.toml" 2>&1 | tail -1
echo ""

BIN_NAME="bench_${TARGET}_swc"

echo "--- Babel ---"
(cd "$TARGET_DIR/babel" && yarn node run.cjs "$N")
echo ""

echo "--- SWC ---"
"$TARGET_DIR/swc/target/release/$BIN_NAME" "$N"
echo ""
