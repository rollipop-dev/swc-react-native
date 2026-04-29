'use strict';

const { transformSync } = require('@babel/core');
const { readFileSync, readdirSync } = require('node:fs');
const { join } = require('node:path');
const { performance } = require('node:perf_hooks');

const plugin = require('@react-native/babel-plugin-codegen');
const flowSyntax = require('@babel/plugin-syntax-flow');
const tsSyntax = require('@babel/plugin-syntax-typescript');

const N = parseInt(process.argv[2] || '1000', 10);
const fixturesDir = join(__dirname, '..', 'fixtures');

// Load all fixture files (.js and .ts)
const fixtures = readdirSync(fixturesDir)
  .filter((f) => f.endsWith('.js') || f.endsWith('.ts'))
  .map((f) => ({
    filename: `/${f}`,
    code: readFileSync(join(fixturesDir, f), 'utf8'),
    isTs: f.endsWith('.ts'),
  }));

if (fixtures.length === 0) {
  console.error('No fixtures found in', fixturesDir);
  process.exit(1);
}

function transformOne({ filename, code, isTs }) {
  transformSync(code, {
    babelrc: false,
    browserslistConfigFile: false,
    configFile: false,
    cwd: '/',
    filename,
    highlightCode: false,
    plugins: [isTs ? tsSyntax : flowSyntax, plugin],
  });
}

// Warmup
for (const fixture of fixtures) {
  transformOne(fixture);
}

// Benchmark
const start = performance.now();
for (let i = 0; i < Math.max(1, N / fixtures.length); i++) {
  for (const fixture of fixtures) {
    transformOne(fixture);
  }
}
const elapsed = performance.now() - start;

console.log(
  `${N} transforms in ${elapsed.toFixed(1)}ms (${(elapsed / N).toFixed(3)}ms/op)`
);
