const test = require('node:test');
const assert = require('node:assert');
const { validateBank } = require('./validate.js');

const good = [{ topic: 'A', questions: Array.from({ length: 8 }, (_, i) => ({
  id: `a${i}`, difficulty: 'Basic', type: 'single', title: 't', choices: ['x', 'y'], correct: 'x',
})) }];

test('valid bank passes', () => {
  const r = validateBank(good);
  assert.strictEqual(r.ok, true, r.errors.join('\n'));
});

test('catches bad difficulty, duplicate id, out-of-range correct, thin topic', () => {
  const bad = [{ topic: 'B', questions: [
    { id: 'd1', difficulty: 'Hard', type: 'single', title: 't', choices: ['x'], correct: 'x' },
    { id: 'd1', difficulty: 'Basic', type: 'single', title: 't', choices: ['x'], correct: 'z' },
  ] }];
  const r = validateBank(bad);
  assert.strictEqual(r.ok, false);
  assert.ok(r.errors.some((e) => /difficulty/i.test(e)));
  assert.ok(r.errors.some((e) => /duplicate/i.test(e)));
  assert.ok(r.errors.some((e) => /correct/i.test(e)));
  assert.ok(r.errors.some((e) => /fewer than 8/i.test(e)));
});
