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
