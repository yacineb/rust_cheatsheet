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
