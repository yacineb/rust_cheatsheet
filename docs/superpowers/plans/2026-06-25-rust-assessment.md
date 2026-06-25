# Rust Knowledge Assessment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a static webpage that quizzes a user on Rust (Basic→Expert) with ~140 questions and shows an on-page score plus a strengths/weaknesses summary by topic and difficulty.

**Architecture:** Pure static single-page app. SurveyJS (vanilla JS bundle) loaded from CDN renders a survey built from per-topic question files. Pure scoring functions grade answers and a custom results panel renders the breakdown. No backend, no build step, no fetch (question files are plain `<script>` files that register into a global array, so it works from `file://` and GitHub Pages alike).

**Tech Stack:** HTML/CSS/vanilla JS, SurveyJS Form Library (`survey-core` + `survey-js-ui` via CDN, MIT core), Node's built-in test runner (`node:test`) for unit tests. No npm dependencies.

## Global Constraints

- No backend, no email, no npm/build step. Everything runs by opening `assessment/index.html` (or serving the folder statically).
- No runtime dependencies except SurveyJS loaded from CDN `<script>` tags.
- All app source lives under `assessment/`, isolated from the Cargo workspace.
- Difficulty values are exactly one of: `Basic`, `Intermediate`, `Advanced`, `Expert`.
- Question `type` values are exactly one of: `single`, `multi`, `boolean`, `code`.
- 14 topics, ~10 questions each (≈140 total), difficulty spread per topic ≈ Basic 2 / Intermediate 3 / Advanced 3 / Expert 2.
- Scoring: single/boolean/code = 1pt if exact match; multi = all-or-nothing (exact set match).
- Level thresholds on overall %: `<40` Beginner, `40–64` Intermediate, `65–84` Advanced, `85+` Expert. Expert additionally requires ≥50% on Expert-band questions, else capped at Advanced.
- Strength markers: ≥75% strong (✅), 50–74% ok (⚠), <50% weak (❌).
- Content sources: repo `README.md` and <https://cheats.rs/>.
- Tests run with: `node --test assessment/`.

## Data Shapes (used across tasks)

**Authoring question** (what topic files contain):

```js
{
  id: 'own-1',                 // unique string id
  difficulty: 'Basic',         // Basic | Intermediate | Advanced | Expert
  type: 'single',              // single | multi | boolean | code
  title: 'What does &T allow?',// plain text or HTML
  code: 'let r = &s;',         // optional; only for type 'code' (rust snippet)
  choices: ['a', 'b', 'c'],    // omit for type 'boolean'
  correct: 'b',                // string (single/code), ['a','c'] (multi), true/false (boolean)
  studyRef: { readme: '#borrowing', cheats: 'https://cheats.rs/#ownership' } // optional
}
```

**Topic registration** (each topic file calls this):

```js
window.registerTopic('Ownership, Borrowing & Moves', [ /* array of authoring questions */ ]);
```

**Normalized question** (produced by the converter, consumed by scoring):

```js
{ name: 'own-1', topic: 'Ownership, Borrowing & Moves', difficulty: 'Basic', type: 'single', correct: 'b' }
```

**Answers map** (from SurveyJS `survey.data`): `{ [questionName]: value }` where value is a string (single/code/boolean) or array (multi).

---

### Task 1: Scoring functions (`scoring.js`)

**Files:**
- Create: `assessment/scoring.js`
- Test: `assessment/scoring.test.js`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `setsEqual(a, b)` → `boolean` (order-independent array equality)
  - `isCorrect(question, answer)` → `boolean` (question is a normalized question; answer is the raw SurveyJS value or `undefined` when skipped)
  - `classifyLevel(overallPct, expertPct)` → `'Beginner'|'Intermediate'|'Advanced'|'Expert'`
  - `gradeSurvey(questions, answers)` → `{ overallPct, correct, total, byTopic, byDifficulty, level }` where `byTopic`/`byDifficulty` are `{ [key]: { correct, total, pct } }`
  - Module is dual-export: `module.exports` for Node, `window.Scoring` for browser.

- [ ] **Step 1: Write the failing test**

Create `assessment/scoring.test.js`:

```js
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test assessment/scoring.test.js`
Expected: FAIL — `Cannot find module './scoring.js'`.

- [ ] **Step 3: Write minimal implementation**

Create `assessment/scoring.js`:

```js
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test assessment/scoring.test.js`
Expected: PASS (all 5 tests).

- [ ] **Step 5: Commit**

```bash
git add assessment/scoring.js assessment/scoring.test.js
git commit -m "feat(assessment): pure scoring functions with tests"
```

---

### Task 2: Question converter + registry (`questions.js`)

**Files:**
- Create: `assessment/questions.js`
- Test: `assessment/questions.test.js`

**Interfaces:**
- Consumes: authoring-question shape (see Data Shapes).
- Produces:
  - global `window.RUST_TOPICS` (array of `{ topic, questions }`) and `window.registerTopic(topic, questions)` that appends to it.
  - `toNormalized(topics)` → array of normalized questions `{ name, topic, difficulty, type, correct }`.
  - `toSurveyJson(topics)` → SurveyJS survey JSON: `{ showProgressBar, progressBarType, pages: [...] }`, one page per topic, each element a `radiogroup` (single/code/boolean) or `checkbox` (multi) with custom props `topic`, `difficulty`, `studyRef`, `correctAnswer`.
  - For `type: 'boolean'`, choices are `['True','False']` and `correct` `true`→`'True'`, `false`→`'False'` (so the stored answer is a string).
  - For `type: 'code'`, the element `title` is set with `html`-style markup: the question title followed by `<pre><code>{code}</code></pre>`; element `titleLocation` left default.
  - Dual-export like Task 1 (`module.exports` + attach `toNormalized`/`toSurveyJson` to `window.Questions`).

- [ ] **Step 1: Write the failing test**

Create `assessment/questions.test.js`:

```js
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test assessment/questions.test.js`
Expected: FAIL — `Cannot find module './questions.js'`.

- [ ] **Step 3: Write minimal implementation**

Create `assessment/questions.js`:

```js
function boolToStr(v) { return v === true ? 'True' : 'False'; }

function escapeHtml(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function toNormalized(topics) {
  const out = [];
  for (const t of topics) {
    for (const q of t.questions) {
      out.push({
        name: q.id,
        topic: t.topic,
        difficulty: q.difficulty,
        type: q.type,
        correct: q.type === 'boolean' ? boolToStr(q.correct) : q.correct,
      });
    }
  }
  return out;
}

function toElement(q) {
  const isMulti = q.type === 'multi';
  let title = q.title;
  if (q.type === 'code' && q.code) {
    title = `${q.title}<pre><code>${escapeHtml(q.code)}</code></pre>`;
  }
  const choices = q.type === 'boolean' ? ['True', 'False'] : q.choices;
  return {
    type: isMulti ? 'checkbox' : 'radiogroup',
    name: q.id,
    title,
    choices,
    isRequired: false,
    // custom props (registered in app.js via Serializer.addProperty)
    topic: undefined, // set by caller
    difficulty: q.difficulty,
    studyRef: q.studyRef || null,
    correctAnswer: q.type === 'boolean' ? boolToStr(q.correct) : q.correct,
  };
}

function toSurveyJson(topics) {
  const pages = topics.map((t) => ({
    name: t.topic,
    title: t.topic,
    questionsOrder: 'random',
    elements: t.questions.map((q) => {
      const el = toElement(q);
      el.topic = t.topic;
      return el;
    }),
  }));
  return { showProgressBar: 'top', progressBarType: 'pages', pages };
}

const api = { toNormalized, toSurveyJson };
if (typeof module !== 'undefined' && module.exports) module.exports = api;
if (typeof window !== 'undefined') {
  window.Questions = api;
  window.RUST_TOPICS = window.RUST_TOPICS || [];
  window.registerTopic = function (topic, questions) {
    window.RUST_TOPICS.push({ topic, questions });
  };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test assessment/questions.test.js`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add assessment/questions.js assessment/questions.test.js
git commit -m "feat(assessment): question registry + SurveyJS converter with tests"
```

---

### Task 3: Page scaffold + survey bootstrap (`index.html`, `styles.css`, `app.js`)

**Files:**
- Create: `assessment/index.html`
- Create: `assessment/styles.css`
- Create: `assessment/app.js`
- Create (placeholder): `assessment/questions/01-basics.js` (one real question so the page renders before the full bank exists)

**Interfaces:**
- Consumes: `window.Scoring`, `window.Questions` (`toNormalized`, `toSurveyJson`), `window.RUST_TOPICS`.
- Produces: `window.initAssessment()` which builds the survey, mounts it into `#surveyContainer`, and on complete calls `window.renderResults(grade, normalized, survey)` (defined in Task 4 — guarded so Task 3 runs standalone by logging the grade if `renderResults` is absent).

- [ ] **Step 1: Create the placeholder question file**

Create `assessment/questions/01-basics.js`:

```js
window.registerTopic('Language basics & syntax', [
  {
    id: 'basics-1',
    difficulty: 'Basic',
    type: 'single',
    title: 'Which keyword declares an immutable binding?',
    choices: ['let', 'var', 'const fn', 'mut'],
    correct: 'let',
    studyRef: { cheats: 'https://cheats.rs/#basic-types' },
  },
]);
```

- [ ] **Step 2: Create `index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Rust Knowledge Assessment</title>
    <link href="https://unpkg.com/survey-core/survey-core.min.css" rel="stylesheet" />
    <link rel="stylesheet" href="styles.css" />
  </head>
  <body>
    <header class="app-header">
      <h1>🦀 Rust Knowledge Assessment</h1>
      <p>Basic → Expert. ~140 questions across 14 topics.</p>
    </header>
    <main>
      <div id="surveyContainer"></div>
      <div id="resultsContainer" hidden></div>
    </main>

    <!-- SurveyJS vanilla bundle -->
    <script src="https://unpkg.com/survey-core/survey.core.min.js"></script>
    <script src="https://unpkg.com/survey-js-ui/survey-js-ui.min.js"></script>

    <!-- App logic -->
    <script src="scoring.js"></script>
    <script src="questions.js"></script>

    <!-- Question bank (one file per topic) -->
    <script src="questions/01-basics.js"></script>
    <!-- additional topic files added in Task 6 -->

    <script src="app.js"></script>
    <script>
      window.addEventListener('DOMContentLoaded', () => window.initAssessment());
    </script>
  </body>
</html>
```

- [ ] **Step 3: Create `styles.css`**

```css
:root { --bg: #0f1115; --fg: #e6e6e6; --accent: #ce4a1f; --ok: #2ecc71; --warn: #f1c40f; --bad: #e74c3c; }
* { box-sizing: border-box; }
body { margin: 0; font-family: system-ui, sans-serif; background: var(--bg); color: var(--fg); }
.app-header { padding: 1.5rem; text-align: center; border-bottom: 1px solid #222; }
.app-header h1 { margin: 0 0 .25rem; }
main { max-width: 860px; margin: 0 auto; padding: 1.5rem; }
pre { background: #1b1e24; padding: .75rem; border-radius: 6px; overflow-x: auto; }
code { font-family: ui-monospace, monospace; }
.results-headline { text-align: center; margin-bottom: 1.5rem; }
.results-headline .level { font-size: 2rem; color: var(--accent); }
.bar-row { display: flex; align-items: center; gap: .5rem; margin: .35rem 0; }
.bar-row .label { width: 230px; font-size: .9rem; }
.bar-track { flex: 1; background: #1b1e24; border-radius: 4px; height: 14px; overflow: hidden; }
.bar-fill { height: 100%; }
.bar-fill.strong { background: var(--ok); }
.bar-fill.ok { background: var(--warn); }
.bar-fill.weak { background: var(--bad); }
.bar-row .pct { width: 48px; text-align: right; font-variant-numeric: tabular-nums; }
.study-links a { color: var(--accent); margin-right: .75rem; }
button.retake { margin-top: 1.5rem; padding: .6rem 1.2rem; background: var(--accent); color: #fff; border: 0; border-radius: 6px; cursor: pointer; }
section.results-block { margin: 1.5rem 0; }
section.results-block h2 { border-bottom: 1px solid #222; padding-bottom: .3rem; }
```

- [ ] **Step 4: Create `app.js`**

```js
const STORAGE_KEY = 'rust-assessment-progress';

function registerCustomProps() {
  const S = window.Survey.Serializer;
  ['topic', 'difficulty'].forEach((p) => {
    if (!S.findProperty('question', p)) S.addProperty('question', { name: p, default: '' });
  });
  if (!S.findProperty('question', 'studyRef')) {
    S.addProperty('question', { name: 'studyRef', default: null });
  }
}

function initAssessment() {
  registerCustomProps();
  const topics = window.RUST_TOPICS || [];
  const normalized = window.Questions.toNormalized(topics);
  const json = window.Questions.toSurveyJson(topics);

  const survey = new window.Survey.Model(json);
  survey.showCompletedPage = false;

  // restore saved progress
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved) {
    try { survey.data = JSON.parse(saved); } catch (e) { /* ignore corrupt save */ }
  }
  survey.onValueChanged.add(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(survey.data));
  });

  survey.onComplete.add((sender) => {
    const grade = window.Scoring.gradeSurvey(normalized, sender.data);
    document.getElementById('surveyContainer').hidden = true;
    if (typeof window.renderResults === 'function') {
      window.renderResults(grade, normalized, sender);
    } else {
      console.log('GRADE', grade); // Task 4 replaces this
    }
  });

  survey.render(document.getElementById('surveyContainer'));
}

function clearProgress() { localStorage.removeItem(STORAGE_KEY); }

window.initAssessment = initAssessment;
window.clearProgress = clearProgress;
```

- [ ] **Step 5: Smoke test in a browser**

Run: `python3 -m http.server -d assessment 8000` then open `http://localhost:8000`.
Expected: the survey renders the single "Language basics" question with a progress bar; answering and pressing Complete logs `GRADE {...}` to the console with `overallPct` and `byTopic`. (Opening `assessment/index.html` directly via `file://` should also work since there is no `fetch`.)

- [ ] **Step 6: Commit**

```bash
git add assessment/index.html assessment/styles.css assessment/app.js assessment/questions/01-basics.js
git commit -m "feat(assessment): static page scaffold + SurveyJS bootstrap"
```

---

### Task 4: Results panel (`results.js`)

**Files:**
- Create: `assessment/results.js`
- Modify: `assessment/index.html` (add `<script src="results.js"></script>` before `app.js`)

**Interfaces:**
- Consumes: `grade` from `gradeSurvey`, `normalized` questions, the completed `survey`.
- Produces: `window.renderResults(grade, normalized, survey)` that fills `#resultsContainer` (and un-hides it) with: headline (level + overall %), per-topic bars, per-difficulty ladder, strengths/focus lists with study links, and a Retake button. Retake calls `window.clearProgress()` then `location.reload()`.
- Helper: `marker(pct)` → `'strong'|'ok'|'weak'`.

- [ ] **Step 1: Write the failing test**

Create `assessment/results.test.js`:

```js
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test assessment/results.test.js`
Expected: FAIL — `Cannot find module './results.js'`.

- [ ] **Step 3: Write minimal implementation**

Create `assessment/results.js`:

```js
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test assessment/results.test.js`
Expected: PASS (2 tests).

- [ ] **Step 5: Wire results.js into index.html**

In `assessment/index.html`, add immediately before `<script src="app.js"></script>`:

```html
    <script src="results.js"></script>
```

- [ ] **Step 6: Smoke test**

Run: `python3 -m http.server -d assessment 8000`, open `http://localhost:8000`, answer the question, press Complete.
Expected: survey hides, results panel shows level headline, a "By topic" bar, "By difficulty" bar, strengths/focus sections, and a working Retake button (which reloads with a cleared survey).

- [ ] **Step 7: Run the full test suite + commit**

Run: `node --test assessment/`
Expected: all tests pass.

```bash
git add assessment/results.js assessment/results.test.js assessment/index.html
git commit -m "feat(assessment): on-page results panel with topic/difficulty breakdown"
```

---

### Task 5: Question-bank validator (`validate.js`)

**Files:**
- Create: `assessment/validate.js`
- Test: `assessment/validate.test.js`

**Interfaces:**
- Consumes: array of `{ topic, questions }`.
- Produces: `validateBank(topics)` → `{ ok: boolean, errors: string[] }`. Checks: every question has unique `id`, valid `difficulty`, valid `type`; `single`/`multi`/`code` have non-empty `choices`; `single`/`code` `correct` is in `choices`; `multi` `correct` is a subset of `choices`; `boolean` `correct` is a boolean. Warns (as errors) if any topic has fewer than 8 questions.

- [ ] **Step 1: Write the failing test**

Create `assessment/validate.test.js`:

```js
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test assessment/validate.test.js`
Expected: FAIL — `Cannot find module './validate.js'`.

- [ ] **Step 3: Write minimal implementation**

Create `assessment/validate.js`:

```js
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test assessment/validate.test.js`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add assessment/validate.js assessment/validate.test.js
git commit -m "feat(assessment): question-bank validator with tests"
```

---

### Task 6: Author the full question bank (14 topic files)

**Files:**
- Create: `assessment/questions/01-basics.js` … `assessment/questions/14-tooling.js` (replace the placeholder from Task 3 with the full set; 01-basics gets expanded to ~10).
- Modify: `assessment/index.html` (add a `<script>` tag for each topic file, in order, after the existing 01-basics tag).
- Create: `assessment/bank-check.js` (Node script that loads all topic files in a shim and runs `validateBank`).

**Interfaces:**
- Consumes: `window.registerTopic` (Task 2), `validateBank` (Task 5).
- Produces: `window.RUST_TOPICS` fully populated with ~140 questions.

**Authoring rules (apply to every topic):**
- Exactly the topic name strings below (must match the spec's 14 topics).
- ~10 questions each, difficulty spread ≈ Basic 2 / Intermediate 3 / Advanced 3 / Expert 2.
- Mix formats: mostly `single`, with at least one `multi`, one `boolean`, and one `code` per topic.
- `id` prefix per topic (e.g. `own-1`…`own-10`) so ids are globally unique.
- Add `studyRef` where the repo README or cheats.rs covers it. README anchors are GitHub-style slugs of the headings in `README.md` (e.g. `### Borrowing` → `#borrowing`, `### Smart Pointers` → `#smart-pointers`). cheats.rs links use its section anchors.
- Source the content from `README.md` and <https://cheats.rs/>. Use README's advanced material (variance, lifetimes `'b: 'a`, ZST list, `Rc`/`Arc` Send/Sync, mutex poisoning, `compare_exchange_weak`, `NonNull`, `#[noalias]`/`MaybeUninit`, panic=abort/wasm, `Cow`, `target-cpu=native`, etc.) for Advanced/Expert bands.

**Topic → id-prefix → primary sources:**

| # | Topic (exact string) | id prefix | README anchors / cheats.rs |
|---|---|---|---|
| 1 | `Language basics & syntax` | `basics` | cheats.rs #basic-types, #control |
| 2 | `Ownership, Borrowing & Moves` | `own` | README #borrowing, #copy-vs-clone; cheats.rs #ownership |
| 3 | `Copy / Clone / Drop / Default` | `ccd` | README #copy-vs-clone, #drop |
| 4 | `Lifetimes & Variance` | `lt` | README #variance, #lifetimes; cheats.rs #lifetimes |
| 5 | `Traits & Generics` | `tg` | README #generics, #rust-conversions; cheats.rs #traits |
| 6 | `Smart pointers` | `sp` | README #box, #smart-pointers |
| 7 | `Error handling` | `err` | cheats.rs #error-custom; Result/Option/`?`/panic |
| 8 | `Iterators & Collections` | `iter` | cheats.rs #iterators; README #rust-conversions (IntoIterator) |
| 9 | `Concurrency & Threading` | `conc` | README #smart-pointers (Send/Sync), #tricky-threading-cases |
| 10 | `Async` | `async` | README #overhead-of-async, #joinset-and-localset |
| 11 | `Atomics, Lock-free & Mutex` | `atom` | README #lock-free--wait-free, #mutex, #other-sync-primitives |
| 12 | `Unsafe, Raw pointers, Pinning & ZST` | `unsafe` | README #raw-pointers, #zst, #aliasing, #pinning-and-self-referential-structs |
| 13 | `Performance & Build` | `perf` | README #performance-optimization-hints, #reduce-binary-size, #speed-up-build, #heap-allocations |
| 14 | `Tooling & Ecosystem` | `tool` | README #bootstrap-environment, #profilers; cheats.rs #tooling |

**Worked example — `assessment/questions/02-ownership.js` (full file shape; author the remaining 9 likewise):**

```js
window.registerTopic('Ownership, Borrowing & Moves', [
  { id: 'own-1', difficulty: 'Basic', type: 'boolean',
    title: 'A shared reference <code>&T</code> is <code>Copy</code>.',
    correct: true,
    studyRef: { readme: '#borrowing' } },
  { id: 'own-2', difficulty: 'Basic', type: 'single',
    title: 'How many active <code>&mut T</code> to the same value may exist in one scope?',
    choices: ['Zero', 'Exactly one', 'Unlimited', 'One per thread'],
    correct: 'Exactly one',
    studyRef: { readme: '#borrowing' } },
  { id: 'own-3', difficulty: 'Intermediate', type: 'code',
    title: 'What happens when this is compiled?',
    code: 'let s = String::from("hi");\nlet r = &s;\ndrop(s);\nprintln!("{r}");',
    choices: ['Prints "hi"', 'Fails to compile (use after move/drop)', 'Panics at runtime', 'Prints garbage'],
    correct: 'Fails to compile (use after move/drop)',
    studyRef: { cheats: 'https://cheats.rs/#ownership' } },
  // own-4 … own-10: 2 more Intermediate, 3 Advanced, 2 Expert, including at least one `multi`.
]);
```

- [ ] **Step 1: Author topic files 1–7**

Create `assessment/questions/01-basics.js` (expand placeholder to ~10), `02-ownership.js`, `03-copy-clone-drop.js`, `04-lifetimes.js`, `05-traits-generics.js`, `06-smart-pointers.js`, `07-error-handling.js`. Each follows the authoring rules and the worked-example shape, drawing content from the sources in the table.

- [ ] **Step 2: Author topic files 8–14**

Create `08-iterators.js`, `09-concurrency.js`, `10-async.js`, `11-atomics-mutex.js`, `12-unsafe-zst.js`, `13-performance.js`, `14-tooling.js`. Same rules.

- [ ] **Step 3: Add a Node validator harness**

Create `assessment/bank-check.js`:

```js
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
```

- [ ] **Step 4: Run the validator**

Run: `node assessment/bank-check.js`
Expected: `Topics: 14, questions: ~140` then `Bank OK`. Fix any reported errors (bad difficulty/type, duplicate id, out-of-range correct, thin topic) until it passes.

- [ ] **Step 5: Wire all topic files into index.html**

In `assessment/index.html`, replace the single placeholder script line with all 14 in order:

```html
    <script src="questions/01-basics.js"></script>
    <script src="questions/02-ownership.js"></script>
    <script src="questions/03-copy-clone-drop.js"></script>
    <script src="questions/04-lifetimes.js"></script>
    <script src="questions/05-traits-generics.js"></script>
    <script src="questions/06-smart-pointers.js"></script>
    <script src="questions/07-error-handling.js"></script>
    <script src="questions/08-iterators.js"></script>
    <script src="questions/09-concurrency.js"></script>
    <script src="questions/10-async.js"></script>
    <script src="questions/11-atomics-mutex.js"></script>
    <script src="questions/12-unsafe-zst.js"></script>
    <script src="questions/13-performance.js"></script>
    <script src="questions/14-tooling.js"></script>
```

- [ ] **Step 6: Full smoke test**

Run: `python3 -m http.server -d assessment 8000`, open `http://localhost:8000`. Page through all 14 topics, complete, and confirm the results panel shows all topics with sensible bars, a level, strengths, and focus areas with study links.

- [ ] **Step 7: Commit**

```bash
git add assessment/questions/ assessment/bank-check.js assessment/index.html
git commit -m "feat(assessment): full ~140-question bank across 14 topics"
```

---

### Task 7: Project README + final verification

**Files:**
- Create: `assessment/README.md`

**Interfaces:** none.

- [ ] **Step 1: Write `assessment/README.md`**

````markdown
# Rust Knowledge Assessment

A static, no-build webpage that quizzes Rust knowledge (Basic→Expert) and shows
an on-page score with a strengths/weaknesses breakdown by topic and difficulty.

## Run

Open `index.html` directly, or serve the folder:

```bash
python3 -m http.server -d assessment 8000
# open http://localhost:8000
```

## Test

```bash
node --test assessment/        # unit tests
node assessment/bank-check.js  # validate the question bank
```

## Deploy (GitHub Pages)

Push the repo and enable Pages; point it at the `assessment/` folder (or copy
its contents to the Pages root). It is fully static.

## Add / edit questions

Edit the files in `questions/`. Each calls `registerTopic(name, questions)`.
See the authoring shape in `questions/02-ownership.js`. Run `bank-check.js` to
validate.
````

- [ ] **Step 2: Final verification**

Run: `node --test assessment/ && node assessment/bank-check.js`
Expected: all unit tests pass; bank reports 14 topics / ~140 questions / `Bank OK`.

- [ ] **Step 3: Commit**

```bash
git add assessment/README.md
git commit -m "docs(assessment): add project README"
```

---

## Self-Review Notes

- **Spec coverage:** static page (T3), SurveyJS lib (T2/T3), 14 topics ×10 (T6), topic+difficulty custom props (T2/T3), 4 formats (T2/T6), all-or-nothing multi + skipped=wrong (T1), per-topic/per-difficulty + level with Expert guard (T1), results panel with strengths/weaknesses + study links + retake + localStorage (T3/T4), file structure & testing (all). Email explicitly out of scope. ✓
- **Type consistency:** `gradeSurvey`/`isCorrect`/`classifyLevel` (T1), `toNormalized`/`toSurveyJson` (T2), `renderResults`/`marker`/`buildStudyList` (T4), `validateBank` (T5) used consistently downstream. Normalized question shape `{name,topic,difficulty,type,correct}` is identical in T1, T2, T6.
- **Deviation from spec:** question files are `.js` (register via `window.registerTopic`) instead of `.json` fetched at runtime — avoids `file://` CORS so the page works when opened directly, and still satisfies "one file per topic." Added `validate.js`/`bank-check.js` as testing aids for the large bank.
