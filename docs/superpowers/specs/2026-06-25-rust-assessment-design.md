# Rust Knowledge Assessment — Design Spec

**Date:** 2026-06-25
**Status:** Approved (design) — pending implementation plan

## Goal

A static webpage that assesses a user's Rust knowledge from basic to very
advanced via an exhaustive questionnaire, then displays a score and a summary of
strengths and weaknesses **on the page** at completion. No backend, no email.

Content is sourced from this repo's `README.md` cheat sheet and from
<https://cheats.rs/>.

## 1. Architecture & Stack

- Pure **static single-page app** — no backend, no required build step.
- **SurveyJS Form Library** (MIT-licensed core) loaded via CDN `<script>` tags
  (`survey-core` + `survey-js-ui` vanilla JS bundle).
- Plain `index.html` + `styles.css` + `app.js` + `scoring.js`.
- Deployable as-is to GitHub Pages.
- Lives in a new top-level `assessment/` directory, isolated from the Rust
  Cargo workspace.
- Results render on the page at completion. No email delivery.

## 2. Content Model

- **14 topics**, each with **~10 questions** ≈ **140 total**:
  1. Language basics & syntax
  2. Ownership, Borrowing & Moves
  3. Copy / Clone / Drop / Default
  4. Lifetimes & Variance
  5. Traits & Generics
  6. Smart pointers (Box, Rc, RefCell, Cow)
  7. Error handling (Result, Option, `?`, panic)
  8. Iterators & Collections
  9. Concurrency & Threading (Arc, Send/Sync, channels)
  10. Async
  11. Atomics, Lock-free & Mutex internals
  12. Unsafe, Raw pointers, Pinning & ZST
  13. Performance & Build optimization
  14. Tooling & Ecosystem (cargo, clippy, wasm)

- Each question is tagged with:
  - `topic` (one of the 14 above)
  - `difficulty` — one of `Basic`, `Intermediate`, `Advanced`, `Expert`
  - optional `studyRef` — a README section anchor and/or cheats.rs link used in
    the results "what to study" links.
- Tags are stored as **SurveyJS custom properties** registered via
  `Serializer.addProperty`, so they can be read back during scoring.
- Per-topic difficulty spread ≈ **Basic 2 / Intermediate 3 / Advanced 3 /
  Expert 2** (10 per topic).

### Question formats

- **Single-answer multiple choice** (SurveyJS `radiogroup`) — the backbone.
- **True/False** (`boolean` or two-choice `radiogroup`).
- **Multi-select "select all that apply"** (`checkbox`).
- **Code-snippet** questions — a Rust snippet rendered in a `<pre><code>` block
  inside the question title (HTML enabled), asking e.g. "does this compile / what
  prints / what does this do".

### Authoring layout

- One JSON file per topic under `assessment/questions/` (e.g.
  `ownership.json`), each holding that topic's ~10 questions.
- `app.js` fetches and merges all topic files at load time.
- This keeps the 140-question bank reviewable and authorable in batches.

## 3. Survey Behavior

- Paged by topic: one topic ≈ one SurveyJS page, with a **progress bar**.
- `questionsOrder: random` within a page; topic (page) order fixed.
- Progress auto-saved to **localStorage** so a refresh does not lose answers.
- Questions are **not required** — a skipped question counts as wrong, so a user
  can complete the assessment without answering everything.

## 4. Scoring

Defined in `scoring.js` as pure, unit-testable functions.

- Single-choice / true-false / code-snippet: **1 point** if the answer matches
  the correct answer.
- Multi-select: **all-or-nothing** — exact match of the selected set against the
  correct set scores 1 point, otherwise 0.
- **Per-topic score** = correct / total for that topic, as a percentage.
- **Per-difficulty score** = correct / total within each difficulty band,
  aggregated across all topics.
- **Overall score** = correct / 140, as a percentage.
- **Overall level** from overall %:
  - `< 40` → Beginner
  - `40–64` → Intermediate
  - `65–84` → Advanced
  - `85+` → Expert
  - **Expert guard:** classification of "Expert" additionally requires ≥ 50%
    correct on Expert-band questions; otherwise capped at Advanced.

## 5. Results Panel

Rendered on `survey.onComplete`, replacing the survey UI. Contains:

- **Headline:** overall level + overall %.
- **Per-topic bar list** with markers:
  - ✅ strength: ≥ 75%
  - ⚠ ok: 50–74%
  - ❌ weak: < 50%
- **Per-difficulty ladder:** how far up Basic → Expert the user got.
- **Strengths** (top topics) and **Focus areas** (weak topics), each with study
  links (README section anchors + cheats.rs).
- **Retake** button (clears localStorage and restarts).
- **Review answers** toggle (SurveyJS read-only display mode showing correct vs.
  given answers).

## 6. File Structure

```
assessment/
  index.html        # loads SurveyJS from CDN, mounts survey + results container
  styles.css        # page + results styling
  app.js            # load+merge questions, register custom props, configure
                    # SurveyJS, wire scoring + results rendering
  scoring.js        # pure scoring functions (unit-testable)
  questions/
    01-basics.json
    02-ownership.json
    ... (14 topic files)
  README.md         # how to run locally and deploy to GitHub Pages
```

## 7. Testing

- `scoring.js` exposes pure functions tested via a small Node-runnable test
  file (`assessment/scoring.test.js`), covering:
  - per-topic percentage computation
  - multi-select all-or-nothing scoring
  - per-difficulty aggregation
  - overall level thresholds, including the Expert guard
- Manual smoke test: open `index.html` in a browser, complete a short run, and
  confirm the results panel renders correctly.

## Decisions & Defaults

- Multi-select scoring: all-or-nothing.
- Skipped question = wrong.
- Level thresholds: 40 / 65 / 85, with Expert guard (≥50% on Expert band).
- One page per topic; random question order within a page.
- Content sources: repo `README.md` + <https://cheats.rs/>.

## Out of Scope (YAGNI)

- Email delivery of results.
- Backend / server / persistence beyond localStorage.
- User accounts, leaderboards, or analytics.
- Subset/randomized question selection (full 140 served each run).
