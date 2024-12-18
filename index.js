const assert = require('node:assert');
const { existsSync } = require('node:fs');
const { join } = require('node:path');

const wasmPath = join(
  __dirname,
  'target/wasm32-wasip1/release/swc_plugin_codegen.wasm'
);

assert(existsSync(wasmPath), `wasm binary not found: ${wasmPath}`);

export default wasmPath;
