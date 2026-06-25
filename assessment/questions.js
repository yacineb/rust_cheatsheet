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
