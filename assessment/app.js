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

  // Allow raw HTML in question titles (SurveyJS markdown hook)
  survey.onTextMarkdown.add((sender, options) => { options.html = options.text; });

  // DOM-level fallback: if SurveyJS escaped HTML in the title, fix it directly
  survey.onAfterRenderQuestion.add((sender, options) => {
    const raw = options.question.title;
    if (!raw || !/<[a-z]/i.test(raw)) return;
    // Try common SurveyJS title span selectors across versions
    const titleSpan =
      options.htmlElement.querySelector('.sd-element__title .sv-string-viewer') ||
      options.htmlElement.querySelector('.sv-title .sv-string-viewer') ||
      options.htmlElement.querySelector('h5 .sv-string-viewer');
    if (titleSpan && titleSpan.innerHTML.trim() !== raw.trim()) {
      titleSpan.innerHTML = raw;
    }
    // Syntax-highlight code blocks that landed inside this question
    if (window.Prism) {
      options.htmlElement.querySelectorAll('pre code').forEach((el) => {
        if (!el.className) el.className = 'language-rust';
        window.Prism.highlightElement(el);
      });
    }
  });

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
