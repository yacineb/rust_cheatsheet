// Loads every topic file in a minimal browser shim, then validates the bank.
const fs = require('node:fs');
const path = require('node:path');
const { validateBank } = require('./validate.js');

global.window = { RUST_TOPICS: [] };
global.window.registerTopic = (topic, questions) => global.window.RUST_TOPICS.push({ topic, questions });

const dir = path.join(__dirname, 'questions');
for (const f of fs.readdirSync(dir).sort()) {
  if (f.endsWith('.js')) {
    // eslint-disable-next-line no-eval
    eval(fs.readFileSync(path.join(dir, f), 'utf8'));
  }
}

const topics = global.window.RUST_TOPICS;
const total = topics.reduce((n, t) => n + t.questions.length, 0);
const res = validateBank(topics);
console.log(`Topics: ${topics.length}, questions: ${total}`);
if (!res.ok) { console.error(res.errors.join('\n')); process.exit(1); }
console.log('Bank OK');
