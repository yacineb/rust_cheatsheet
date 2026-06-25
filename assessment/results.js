function marker(pct) {
  if (pct >= 75) return 'strong';
  if (pct >= 50) return 'ok';
  return 'weak';
}

function buildStudyList(byTopic) {
  return Object.keys(byTopic)
    .filter((t) => byTopic[t].pct < 50)
    .map((t) => ({ topic: t, pct: byTopic[t].pct }))
    .sort((a, b) => a.pct - b.pct);
}

function barRow(label, pct) {
  const cls = marker(pct);
  return `<div class="bar-row"><span class="label">${label}</span>` +
    `<span class="bar-track"><span class="bar-fill ${cls}" style="width:${pct}%"></span></span>` +
    `<span class="pct">${pct}%</span></div>`;
}

function renderResults(grade, normalized, survey) {
  const el = document.getElementById('resultsContainer');
  const difficultyOrder = ['Basic', 'Intermediate', 'Advanced', 'Expert'];

  const topicRows = Object.keys(grade.byTopic)
    .sort((a, b) => grade.byTopic[b].pct - grade.byTopic[a].pct)
    .map((t) => barRow(t, grade.byTopic[t].pct))
    .join('');

  const diffRows = difficultyOrder
    .filter((d) => grade.byDifficulty[d])
    .map((d) => barRow(d, grade.byDifficulty[d].pct))
    .join('');

  const strengths = Object.keys(grade.byTopic)
    .filter((t) => grade.byTopic[t].pct >= 75)
    .sort((a, b) => grade.byTopic[b].pct - grade.byTopic[a].pct);

  const weak = buildStudyList(grade.byTopic);

  // study links come from the first weak-topic question that carries a studyRef
  const refByTopic = {};
  for (const q of normalized) { /* normalized lacks studyRef; pull from survey questions */ }
  survey.getAllQuestions().forEach((qq) => {
    if (qq.studyRef && !refByTopic[qq.topic]) refByTopic[qq.topic] = qq.studyRef;
  });

  function links(topic) {
    const ref = refByTopic[topic];
    if (!ref) return '';
    const parts = [];
    if (ref.readme) parts.push(`<a href="../README.md${ref.readme}" target="_blank">README</a>`);
    if (ref.cheats) parts.push(`<a href="${ref.cheats}" target="_blank">cheats.rs</a>`);
    return `<span class="study-links">${parts.join('')}</span>`;
  }

  el.innerHTML =
    `<div class="results-headline"><div class="level">${grade.level}</div>` +
    `<div>Overall score: <strong>${grade.overallPct}%</strong> (${grade.correct}/${grade.total})</div></div>` +
    `<section class="results-block"><h2>By topic</h2>${topicRows}</section>` +
    `<section class="results-block"><h2>By difficulty</h2>${diffRows}</section>` +
    `<section class="results-block"><h2>Strengths</h2>` +
    (strengths.length ? `<ul>${strengths.map((t) => `<li>${t} (${grade.byTopic[t].pct}%)</li>`).join('')}</ul>` : '<p>Keep practicing to build clear strengths.</p>') +
    `</section>` +
    `<section class="results-block"><h2>Focus areas</h2>` +
    (weak.length ? `<ul>${weak.map((w) => `<li>${w.topic} (${w.pct}%) ${links(w.topic)}</li>`).join('')}</ul>` : '<p>No weak areas — excellent!</p>') +
    `</section>` +
    `<button class="retake">Retake assessment</button>`;

  el.hidden = false;
  el.querySelector('.retake').addEventListener('click', () => {
    window.clearProgress();
    location.reload();
  });
  window.scrollTo(0, 0);
}

const api = { marker, buildStudyList, renderResults };
if (typeof module !== 'undefined' && module.exports) module.exports = api;
if (typeof window !== 'undefined') {
  window.renderResults = renderResults;
  window.ResultsHelpers = api;
}
