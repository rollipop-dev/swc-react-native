import * as fs from 'node:fs';
import * as path from 'node:path';
import * as swc from '@swc/core';
import codegenPlugin from '../index.mjs';

const code = await fs.promises.readFile(
  path.join(import.meta.dirname, 'code.js'),
  {
    encoding: 'utf-8',
  }
);

const result = await swc.transform(code, {
  filename: 'test.js',
  jsc: {
    target: 'esnext',
    experimental: {
      plugins: [[codegenPlugin, {}]],
    },
  },
});

console.log(result.code);
