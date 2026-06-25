const test = require('node:test');
const assert = require('node:assert');
const { setsEqual, isCorrect, classifyLevel, gradeSurvey } = require('./scoring.js');

test('setsEqual ignores order and duplicates-free compares', () => {
  assert.strictEqual(setsEqual(['a', 'b'], ['b', 'a']), true);
  assert.strictEqual(setsEqual(['a'], ['a', 'b']), false);
  assert.strictEqual(setsEqual([], []), true);
});

test('isCorrect: single/code/boolean exact match', () => {
  assert.strictEqual(isCorrect({ type: 'single', correct: 'b' }, 'b'), true);
  assert.strictEqual(isCorrect({ type: 'single', correct: 'b' }, 'a'), false);
  assert.strictEqual(isCorrect({ type: 'code', correct: 'x' }, 'x'), true);
  assert.strictEqual(isCorrect({ type: 'boolean', correct: false }, false), true);
  assert.strictEqual(isCorrect({ type: 'boolean', correct: true }, undefined), false);
});

test('isCorrect: multi is all-or-nothing', () => {
  assert.strictEqual(isCorrect({ type: 'multi', correct: ['a', 'c'] }, ['c', 'a']), true);
  assert.strictEqual(isCorrect({ type: 'multi', correct: ['a', 'c'] }, ['a']), false);
  assert.strictEqual(isCorrect({ type: 'multi', correct: ['a', 'c'] }, undefined), false);
});

test('classifyLevel thresholds + expert guard', () => {
  assert.strictEqual(classifyLevel(30, 0), 'Beginner');
  assert.strictEqual(classifyLevel(50, 0), 'Intermediate');
  assert.strictEqual(classifyLevel(70, 0), 'Advanced');
  assert.strictEqual(classifyLevel(90, 60), 'Expert');
  assert.strictEqual(classifyLevel(90, 40), 'Advanced'); // expert guard caps it
});

test('gradeSurvey aggregates by topic and difficulty', () => {
  const questions = [
    { name: 'q1', topic: 'A', difficulty: 'Basic', type: 'single', correct: 'x' },
    { name: 'q2', topic: 'A', difficulty: 'Expert', type: 'single', correct: 'x' },
    { name: 'q3', topic: 'B', difficulty: 'Basic', type: 'multi', correct: ['x', 'y'] },
  ];
  const answers = { q1: 'x', q2: 'wrong', q3: ['y', 'x'] };
  const r = gradeSurvey(questions, answers);
  assert.strictEqual(r.total, 3);
  assert.strictEqual(r.correct, 2);
  assert.strictEqual(r.byTopic.A.pct, 50);
  assert.strictEqual(r.byTopic.B.pct, 100);
  assert.strictEqual(r.byDifficulty.Basic.pct, 100);
  assert.strictEqual(r.byDifficulty.Expert.pct, 0);
});
