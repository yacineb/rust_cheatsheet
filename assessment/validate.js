const DIFFS = ['Basic', 'Intermediate', 'Advanced', 'Expert'];
const TYPES = ['single', 'multi', 'boolean', 'code'];

function validateBank(topics) {
  const errors = [];
  const ids = new Set();
  for (const t of topics) {
    if (t.questions.length < 8) errors.push(`Topic "${t.topic}" has fewer than 8 questions (${t.questions.length})`);
    for (const q of t.questions) {
      const where = `[${t.topic}/${q.id}]`;
      if (ids.has(q.id)) errors.push(`${where} duplicate id`);
      ids.add(q.id);
      if (!DIFFS.includes(q.difficulty)) errors.push(`${where} invalid difficulty "${q.difficulty}"`);
      if (!TYPES.includes(q.type)) errors.push(`${where} invalid type "${q.type}"`);
      if (q.type === 'boolean') {
        if (typeof q.correct !== 'boolean') errors.push(`${where} boolean correct must be true/false`);
      } else {
        if (!Array.isArray(q.choices) || q.choices.length === 0) { errors.push(`${where} missing choices`); continue; }
        if (q.type === 'multi') {
          if (!Array.isArray(q.correct) || !q.correct.every((c) => q.choices.includes(c))) errors.push(`${where} multi correct not subset of choices`);
        } else if (!q.choices.includes(q.correct)) {
          errors.push(`${where} correct "${q.correct}" not in choices`);
        }
      }
    }
  }
  return { ok: errors.length === 0, errors };
}

const api = { validateBank };
if (typeof module !== 'undefined' && module.exports) module.exports = api;
if (typeof window !== 'undefined') window.Validate = api;
