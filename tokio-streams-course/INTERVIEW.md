# Tokio Streams — Interview Cram Sheet

The 10 questions an interviewer actually asks, with tight answers. Each links to the
worked example where you can prove it (`cargo run -p tokio-streams-course --bin <ex>`).
Read these the morning of; if you can say them cold, you're ready.

---

### 1. What is a `Stream`, and how does it relate to `Iterator` and `Future`? → `ex01`

An **async iterator**: a sequence of values produced over time where pulling the next one
may need to `.await`. The trait is `poll_next(self: Pin<&mut Self>, cx) -> Poll<Option<T>>`
— `Iterator::next() -> Option<T>` made poll-based so it can return `Poll::Pending` instead
of blocking. `Some(x)` = an item, `None` = finished. A **`Future` is a stream of one item**.
You use it through `StreamExt` combinators, exactly like iterator adapters.

---

### 2. Why does `poll_next` take `Pin<&mut Self>`? → `ex09`

Because async state machines (from `async` blocks / `stream!` generators) can be
**self-referential** — they store a value and a reference into it across an `.await`. `Pin`
guarantees the value won't move, so those internal pointers stay valid. Practical fallout:
most concrete ecosystem streams are `Unpin` and this never bites; **manual and generator
streams are `!Unpin`**, so you `tokio::pin!(s)` (stack) or `Box::pin(s)` (heap) before
`.next()`. `.next()` itself requires `Self: Unpin`, which is why pinning is what unblocks it.

---

### 3. What makes a future "cancellation-safe," and why does it matter in `select!`? → `ex07`

`select!` polls several futures and **drops the ones that didn't win**. A future is
cancel-safe if dropping it mid-flight loses no data and leaves no torn state — you can just
create it again next iteration. `stream.next()` **is** cancel-safe: the dropped `Next`
future held no buffered item; the stream keeps its state. **Not** cancel-safe: things that
pull data into a temporary that's lost on drop (e.g. a manual `read()` into a local buffer
that's already partially filled). Rule: **keep in-progress state inside the stream/struct,
never inside the future you park in a `select!` branch.**

```rust
loop { tokio::select! {
    maybe = stream.next() => match maybe { Some(x)=>handle(x), None=>break },
    _ = token.cancelled() => break,   // graceful stop; stream state survives
}}
```

---

### 4. How do you fan-in streams — `merge` vs `StreamMap` vs `select!`? → `ex04`, `ex05`

- **`a.merge(b)`** — fixed arity, same `Item` type, **forgets** the source, ends when *both*
  end. Good for a couple of same-typed sources.
- **`StreamMap<K, S>`** — **dynamic** set (`insert`/`remove` at runtime), yields `(key, item)`
  so you know the source, auto-drops finished members, ends when all are gone. Good for a
  changing/keyed set (subscriptions, connections).
- **`select!`** — manual per-iteration control; mix streams with non-stream events (timers,
  shutdown) and heterogeneous handling. Good when combining with cancellation.

One-liner: *merge = static & anonymous, StreamMap = dynamic & tagged, select! = manual & mixed.*

---

### 5. How do you run stream work concurrently and bound it — `buffer_unordered` vs spawning? → `ex06`

`map(|x| async {...})` turns items into a stream of futures. Then:
- **`buffered(n)`** — ≤`n` concurrent, results **in input order**.
- **`buffer_unordered(n)`** — ≤`n` concurrent, results **as they finish** (faster, reordered).
- **`for_each_concurrent(n, f)`** — bounded concurrency, side effects only.

`n` is the **concurrency bound** — the knob that stops you launching 10k tasks. vs
`tokio::spawn`/`JoinSet`: spawning hands work to the scheduler (may run on other worker
threads, requires `Send + 'static`) — best for independent, long, or CPU-ish jobs.
`buffer_unordered` keeps the work in *this* task, driven by the consumer — natural
backpressure, no `'static` bound — best when inputs arrive **as a stream** and the consumer
should pace them. **Perf:** `buffer_unordered` bounds peak memory to `n` in-flight; unbounded
`spawn` in a loop does not.

---

### 6. What is backpressure and how do streams provide it? → `ex08`

Backpressure = a slow consumer **slows the producer** instead of an unbounded queue growing.
Bounded `mpsc::channel(cap)` gives it for free: `tx.send().await` **suspends** when the
buffer is full until the consumer drains a slot. `unbounded_channel` has **none** — memory
can blow up under load. Consumer-side knobs: `buffer_unordered(n)`/`for_each_concurrent(n)`
bound in-flight work; `throttle` caps rate; `chunks_timeout` batches. **Default to bounded**;
picking `cap`/`n` is an explicit latency-vs-memory decision (a real perf lever).

---

### 7. Why does a combinator chain run nothing until consumed? (Laziness) → `ex01`

Streams are **poll-driven and lazy**: building `s.map(..).filter(..)` only wraps types — no
closure runs until something *polls* it (`.next().await`, `while let`, `.collect().await`, a
`for_each`). Identical to iterators. Corollary: side effects happen only on consumption, so
"I built the pipeline but nothing happened" almost always means you never awaited a consumer.

---

### 8. How do you cancel / shut a stream down cleanly? → `ex07`

Four tools, escalating control:
- **Drop it** — stops polling immediately.
- **`take_until(fut)`** — end when a future fires (a deadline `sleep`, or `token.cancelled()`).
- **`CancellationToken`** — cloneable shared signal; `token.cancel()` flips every clone;
  use `token.cancelled()` as the future.
- **`select!`** — race `next()` against `token.cancelled()` and decide per-iteration.

Server shutdown pattern: **one token, cloned into every connection task**, each stream
`take_until(token.cancelled())`; call `cancel()` once → all tasks wind down. Add
`timeout(dur, s.next())` if a single stalled item must not block the whole sink.

---

### 9. How do you handle errors in a stream? (`Result<T,E>` + `TryStreamExt`) → `ex13`, `ex14`

`Item = Result<T, E>`; `futures::TryStreamExt` treats `E` as a **short-circuit channel — the
`?` of streams**: `try_next()?` in a loop, `try_collect() -> Result<Vec, E>`, `try_fold`,
`try_for_each_concurrent(n)`, `try_buffer_unordered(n)`, plus `map_ok`/`map_err`/`and_then`.
**The trap they probe:** these `try_*` consumers **stop at the first `Err` and drop the
rest**. If you must see *every* outcome, don't use `try_*` — `collect()` into a
`Vec<Result<..>>` and partition. Producer side: **`try_stream!`** where `yield v` = `Ok(v)`
and `?` on an `Err` emits it and *ends* the stream.

---

### 10. How do you implement a `Stream`, and what's the `Poll::Pending` contract? → `ex09`–`ex11`, `ex14`

From easiest to most control:
1. **`async_stream::stream!` / `try_stream!`** — write it like an async fn with `yield`.
2. **`stream::unfold(state, f)`** — pure `state -> Option<(item, next_state)>` generator.
3. **Wrap an inner stream** (a combinator) — transform in `poll_next`; `Pin::new(&mut inner)`
   if it's `Unpin`, else use `pin-project`.
4. **Manual `poll_next`** — return `Ready(Some)/Ready(None)/Pending`.

**The contract:** if you return **`Poll::Pending` you must arrange for the waker to fire**
later — normally by delegating to a child future's `poll` (which registers `cx.waker()` for
you) or calling `cx.waker().wake_by_ref()`. **`Pending` without registering a wake = the task
sleeps forever.** After `None`, consumers should stop; use `fuse()` if a caller might poll past
the end.

---

## Rapid-fire follow-ups (know the one-liner)

- **Two `StreamExt`s imported → `.next()` is ambiguous.** `tokio_stream`'s has
  `merge/timeout/throttle/chunks_timeout`; `futures`' has `buffered/buffer_unordered/
  for_each_concurrent/enumerate/take_until`. Import one; qualify the other.
- **`merge` ends when both end; `select!` ends when you say.** Know which "done" you mean.
- **`buffered` preserves order, `buffer_unordered` doesn't.** Choose deliberately.
- **`IntervalStream`'s first tick fires immediately** — a `timeout` on it won't elapse.
- **Holding a `MutexGuard` across a stream `.await`** stalls everything / risks deadlock.
- **`broadcast` items are `Result`** because a lagging subscriber can miss messages.
- **`watch` gives you only the *latest* value**, not every change — good for config/state.

## Performance angle (Rerun-flavored)

- **Bound everything.** Unbounded channels and unbounded `spawn`-in-a-loop are the classic
  memory blowups; `mpsc::channel(cap)` + `buffer_unordered(n)` cap queue and in-flight work.
- **`Box<dyn Stream>` costs** a heap alloc + dynamic dispatch per poll. In hot paths prefer
  concrete types or generics; reach for `Pin<Box<dyn Stream>>` only when you truly need a
  heterogeneous/dynamic set (e.g. `StreamMap` values).
- **Batch to amortize.** `chunks_timeout(n, dt)` turns many tiny writes (DB rows, GPU
  uploads, network frames) into few big ones — often the single biggest throughput win.
- **Pick the reorder you can tolerate.** `buffer_unordered` finishes sooner but reorders;
  if downstream needs order, pay for `buffered` or re-sort — don't reorder by accident.
- **Cancellation is a latency feature.** Prompt `CancellationToken` shutdown + per-item
  `timeout` keep p99 bounded when one source stalls.
