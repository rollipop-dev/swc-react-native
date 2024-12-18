import assert from 'node:assert';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

const wasmPath = join(
  import.meta.dirname,
  'target/wasm32-wasip1/release/swc_plugin_codegen.wasm'
);

assert(existsSync(wasmPath), `wasm binary not found: ${wasmPath}`);

export default wasmPath;
