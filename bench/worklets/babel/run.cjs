'use strict';

const { transformSync } = require('@babel/core');
const { readFileSync, readdirSync } = require('node:fs');
const { join } = require('node:path');
const { performance } = require('node:perf_hooks');

const plugin = require('react-native-worklets/plugin');
const tsSyntax = require('@babel/plugin-syntax-typescript');

const N = parseInt(process.argv[2] || '1000', 10);
const fixturesDir = join(__dirname, '..', 'fixtures');

const fixtures = readdirSync(fixturesDir)
  .filter((f) => f.endsWith('.ts') || f.endsWith('.tsx'))
  .map((f) => ({
    filename: join(fixturesDir, f),
    code: readFileSync(join(fixturesDir, f), 'utf8'),
    isTsx: f.endsWith('.tsx'),
  }));

if (fixtures.length === 0) {
  console.error('No fixtures found in', fixturesDir);
  process.exit(1);
}

function transformOne({ filename, code, isTsx }) {
  transformSync(code, {
    babelrc: false,
    browserslistConfigFile: false,
    configFile: false,
    filename,
    highlightCode: false,
    plugins: [[tsSyntax, { isTSX: isTsx }], plugin],
  });
}

for (const fixture of fixtures) {
  transformOne(fixture);
}

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
