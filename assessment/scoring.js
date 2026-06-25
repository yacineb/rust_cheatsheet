function setsEqual(a, b) {
  if (!Array.isArray(a) || !Array.isArray(b)) return false;
  if (a.length !== b.length) return false;
  const sa = new Set(a);
  return b.every((x) => sa.has(x));
}

function isCorrect(question, answer) {
  if (answer === undefined || answer === null) return false;
  if (question.type === 'multi') return setsEqual(question.correct, answer);
  return question.correct === answer;
}

function classifyLevel(overallPct, expertPct) {
  let level;
  if (overallPct < 40) level = 'Beginner';
  else if (overallPct < 65) level = 'Intermediate';
  else if (overallPct < 85) level = 'Advanced';
  else level = 'Expert';
  if (level === 'Expert' && expertPct < 50) level = 'Advanced';
  return level;
}

function bucket(map, key, ok) {
  if (!map[key]) map[key] = { correct: 0, total: 0, pct: 0 };
  map[key].total += 1;
  if (ok) map[key].correct += 1;
}

function finalizePct(map) {
  for (const k of Object.keys(map)) {
    const e = map[k];
    e.pct = e.total ? Math.round((e.correct / e.total) * 100) : 0;
  }
}

function gradeSurvey(questions, answers) {
  const byTopic = {};
  const byDifficulty = {};
  let correct = 0;
  for (const q of questions) {
    const ok = isCorrect(q, answers[q.name]);
    if (ok) correct += 1;
    bucket(byTopic, q.topic, ok);
    bucket(byDifficulty, q.difficulty, ok);
  }
  finalizePct(byTopic);
  finalizePct(byDifficulty);
  const total = questions.length;
  const overallPct = total ? Math.round((correct / total) * 100) : 0;
  const expertPct = byDifficulty.Expert ? byDifficulty.Expert.pct : 0;
  const level = classifyLevel(overallPct, expertPct);
  return { overallPct, correct, total, byTopic, byDifficulty, level };
}

const api = { setsEqual, isCorrect, classifyLevel, gradeSurvey };
if (typeof module !== 'undefined' && module.exports) module.exports = api;
if (typeof window !== 'undefined') window.Scoring = api;
