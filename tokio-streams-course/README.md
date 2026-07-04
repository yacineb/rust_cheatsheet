# Tokio Streams — Hands-On Course

A self-contained crate to make you genuinely proficient with async streams in Tokio:
theory → runnable worked examples → fill-in-the-blank drills with tests.

## Layout

| File | What it is |
|---|---|
| `CRASH_COURSE.md` | Read this first. The mental model, the whole API surface, the gotchas. |
| `INTERVIEW.md` | The 10 questions interviewers ask + tight answers + a perf angle. Read last. |
| `src/bin/ex*.rs` | 14 runnable, heavily-commented worked examples. Run, read, tweak. |
| `src/bin/drills.rs` | 9 exercises with `todo!()` bodies + tests. **This is the practice.** |
| `src/bin/drills_solved.rs` | Reference answers (same tests). Peek only when stuck. |

## The 15-minute quick start

```bash
# 1. confirm everything builds + the reference drills pass (proves your toolchain works)
cargo test -p tokio-streams-course --bin drills_solved

# 2. run the first worked example and read its source side-by-side
cargo run -p tokio-streams-course --bin ex01_basics

# 3. start practicing: open src/bin/drills.rs, fill in d1, then
cargo test -p tokio-streams-course --bin drills t1
```

## Worked examples — the difficulty ramp

Run each with `cargo run -p tokio-streams-course --bin <name>`.

| # | Bin | Topic | New concepts |
|---|---|---|---|
| 01 | `ex01_basics` | Create & consume | `iter/once/empty`, `.next().await`, `while let`, laziness |
| 02 | `ex02_combinators` | Transform | `map/filter/then/take_while/fold/timeout`, pinning `!Unpin` streams |
| 03 | `ex03_channels` | Channels are streams | `ReceiverStream`, `BroadcastStream`, `WatchStream` |
| 04 | `ex04_merge` | Fan-in (fixed) | `.merge`, mapping to a common enum |
| 05 | `ex05_stream_map` | Fan-in (dynamic, keyed) | `StreamMap`, add/remove sources at runtime |
| 06 | `ex06_fan_out` | Fan-out (bounded) | `buffered` vs `buffer_unordered` vs `for_each_concurrent` |
| 07 | `ex07_cancellation` | Stopping cleanly | `take_until`, `CancellationToken`, `select!`, cancel-safety |
| 08 | `ex08_backpressure` | Flow control | bounded channels, `throttle`, `chunks_timeout` |
| 09 | `ex09_impl_manual` | Implement Stream (hard way) | hand-written `poll_next`, `Poll::Pending`, wakers, `Pin` |
| 10 | `ex10_impl_generators` | Implement Stream (easy way) | `stream::unfold`, `async_stream::stream!` |
| 11 | `ex11_impl_combinator` | Wrap a stream | custom combinator + ext trait, the Pin projection issue |
| 12 | `ex12_capstone` | Put it together | multi-source → filter → bounded fan-out → sink + shutdown |
| 13 | `ex13_try_streams` | Error handling | `TryStreamExt`: `try_next/try_collect/try_fold/map_ok/map_err/and_then/try_for_each_concurrent/try_buffer_unordered`, short-circuit vs collect-all |
| 14 | `ex14_try_stream_macro` | Implement a fallible stream | `try_stream!`: `yield` = `Ok`, `?` = emit `Err` + end; the producer-side mirror of ex13 |

Each example ends with a `YOUR TURN` comment — a small modification to try in place.

## Drills — the real practice

`src/bin/drills.rs` has 13 functions stubbed with `todo!()` and a test each:

```bash
cargo test -p tokio-streams-course --bin drills          # run all, see what's red
cargo test -p tokio-streams-course --bin drills t5       # run just drill 5's test
```

Fill them in until green. They cover: basics, async transform, channels, merge,
`StreamMap`, bounded fan-out, `take_until` cancellation, `stream!`, `unfold`,
error handling with `TryStreamExt` (`try_fold`/`try_collect` + collect-all), and
writing a fallible stream with `try_stream!`.

## Suggested path to proficiency

1. Read `CRASH_COURSE.md` once end to end.
2. Run `ex01` → `ex08`, reading each source and doing its `YOUR TURN`.
3. Do drills **d1–d7** (`cargo test --bin drills`).
4. Read `ex09` → `ex14` (implementing streams, the capstone, then error handling both ways).
5. Do drills **d8–d13**, then the harder `YOUR TURN`s in `ex11`/`ex13`/`ex14`.
6. Explain out loud: *why does `poll_next` take `Pin<&mut Self>`? What makes a future
   cancel-safe? When would you pick `StreamMap` over `merge`, or `buffer_unordered`
   over spawning tasks?* If you can answer those, you're proficient.
7. Final pass: read `INTERVIEW.md` and say each of the 10 answers cold. That's the exam.
