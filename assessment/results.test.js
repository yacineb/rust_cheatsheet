const test = require('node:test');
const assert = require('node:assert');
const { marker, buildStudyList } = require('./results.js');

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
