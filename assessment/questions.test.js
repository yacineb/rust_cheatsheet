const test = require('node:test');
const assert = require('node:assert');
const { toNormalized, toSurveyJson } = require('./questions.js');

const topics = [
  {
    topic: 'Ownership',
    questions: [
      { id: 'o1', difficulty: 'Basic', type: 'single', title: 'T?', choices: ['a', 'b'], correct: 'a' },
      { id: 'o2', difficulty: 'Advanced', type: 'multi', title: 'All?', choices: ['a', 'b', 'c'], correct: ['a', 'c'] },
      { id: 'o3', difficulty: 'Basic', type: 'boolean', title: '&mut T is Copy?', correct: false },
      { id: 'o4', difficulty: 'Expert', type: 'code', title: 'Compiles?', code: 'drop(s);', choices: ['yes', 'no'], correct: 'no' },
    ],
  },
];

test('toNormalized flattens with metadata', () => {
  const n = toNormalized(topics);
  assert.strictEqual(n.length, 4);
  assert.deepStrictEqual(n[0], { name: 'o1', topic: 'Ownership', difficulty: 'Basic', type: 'single', correct: 'a' });
  assert.strictEqual(n[1].type, 'multi');
  assert.deepStrictEqual(n[1].correct, ['a', 'c']);
});

test('boolean correct normalizes to string', () => {
  const n = toNormalized(topics);
  assert.strictEqual(n[2].correct, 'False');
});

test('toSurveyJson builds one page per topic with custom props', () => {
  const j = toSurveyJson(topics);
  assert.strictEqual(j.pages.length, 1);
  assert.strictEqual(j.pages[0].name, 'Ownership');
  const e0 = j.pages[0].elements[0];
  assert.strictEqual(e0.type, 'radiogroup');
  assert.strictEqual(e0.name, 'o1');
  assert.strictEqual(e0.topic, 'Ownership');
  assert.strictEqual(e0.difficulty, 'Basic');
  const e1 = j.pages[0].elements[1];
  assert.strictEqual(e1.type, 'checkbox');
  const e2 = j.pages[0].elements[2];
  assert.deepStrictEqual(e2.choices, ['True', 'False']);
  const e3 = j.pages[0].elements[3];
  assert.match(e3.title, /<pre><code>/);
});
