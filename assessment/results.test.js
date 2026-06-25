const test = require('node:test');
const assert = require('node:assert');
const { marker, buildStudyList, formatAnswer } = require('./results.js');

test('marker thresholds', () => {
  assert.strictEqual(marker(80), 'strong');
  assert.strictEqual(marker(60), 'ok');
  assert.strictEqual(marker(40), 'weak');
});

test('buildStudyList returns weak topics sorted ascending', () => {
  const byTopic = { A: { pct: 90 }, B: { pct: 30 }, C: { pct: 49 } };
  const weak = buildStudyList(byTopic);
  assert.deepStrictEqual(weak.map((x) => x.topic), ['B', 'C']);
});

test('formatAnswer formats skipped, arrays, and scalars', () => {
  const { formatAnswer } = require('./results.js');
  assert.strictEqual(formatAnswer('single', undefined), '(skipped)');
  assert.strictEqual(formatAnswer('boolean', null), '(skipped)');
  assert.strictEqual(formatAnswer('multi', []), '(skipped)');
  assert.strictEqual(formatAnswer('multi', ['a', 'c']), 'a, c');
  assert.strictEqual(formatAnswer('single', 'b'), 'b');
});
