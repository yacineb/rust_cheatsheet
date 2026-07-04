# Tokio Streams — Crash Course

A stream is **an async iterator**: a sequence of values produced *over time*, where
pulling the next value may need to `.await`. If you understand `Iterator`, you already
understand 80% of `Stream`. This doc gives you the model; the `src/bin/ex*.rs` files
let you run it; `src/bin/drills.rs` makes you write it.

---

## 1. The one analogy that unlocks everything

| Sync world | Async world | Produces |
|---|---|---|
| `fn() -> T` | `async fn() -> T` / `Future<Output = T>` | **one** value, later |
| `Iterator<Item = T>` (`next() -> Option<T>`) | `Stream<Item = T>` (`poll_next() -> Poll<Option<T>>`) | **many** values, over time |

- A `Future` is a stream that yields exactly one item then ends.
- `Some(x)` = "here's an item", `None` = "stream is finished, stop calling me".
- The difference from `Iterator`: getting the next item might not be ready *yet*, so
  instead of blocking, the stream returns `Poll::Pending` and arranges to be polled
  again when progress is possible.

```rust
// The trait, essentially:
pub trait Stream {
    type Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>;
    //                                                          ^^^^ Ready(Some) | Ready(None) | Pending
}
```

You rarely call `poll_next` yourself. You use **`StreamExt`**, the extension trait full
of combinators, exactly like `Iterator`'s adapters.

```rust
use tokio_stream::StreamExt;      // brings .next(), .map(), .filter(), .merge(), ...
let mut s = tokio_stream::iter(vec![1, 2, 3]);
while let Some(n) = s.next().await {   // <-- the async `for`
    println!("{n}");
}
```

> ⚠️ **Streams are lazy.** A combinator chain does *nothing* until something consumes it
> (`.next().await`, `while let`, `.collect().await`, `for_each`). Same as iterators.

> ⚠️ **You must import a `StreamExt`** for the methods to exist. There are two:
> `tokio_stream::StreamExt` (has `merge`, `timeout`, `throttle`, `chunks_timeout`) and
> `futures::StreamExt` (has `buffered`, `buffer_unordered`, `for_each_concurrent`).
> Importing both at once makes shared methods like `.next()` ambiguous — pick one per
> file, or disambiguate with `StreamExt::next(&mut s)`.

---

## 2. Where streams come from

**Ready-made:**
```rust
tokio_stream::iter(vec![1,2,3]);   // from any IntoIterator
tokio_stream::once(42);            // exactly one item
tokio_stream::empty::<i32>();      // zero items
tokio_stream::pending::<i32>();    // never yields (useful as a "disabled" branch)
```

**From channels (the workhorse):** wrap a receiver so a producer task feeds a consumer stream.
```rust
use tokio_stream::wrappers::ReceiverStream;
let (tx, rx) = tokio::sync::mpsc::channel(16);
let stream = ReceiverStream::new(rx);           // now it's a Stream
// also: BroadcastStream (fan-out), WatchStream (latest-value), IntervalStream (ticks)
```

**Built from a closure / async block:**
```rust
// stream::unfold: carry state, return Option<(item, next_state)>
futures::stream::unfold(0u64, |n| async move { Some((n, n + 1)) }); // 0,1,2,...

// async-stream: write it like an async fn with `yield`
async_stream::stream! { for i in 0..3 { yield i; } };
```

---

## 3. Transforming — the combinator toolbox

Just like iterators, but the mapping closures can be async and adapters can await.

```rust
use tokio_stream::StreamExt;
let out: Vec<_> = tokio_stream::iter(0..10)
    .map(|x| x * 2)              // sync transform
    .filter(|x| x % 3 == 0)      // keep some
    .filter_map(|x| Some(x))     // transform+drop in one
    .take(3)                     // stop early -> stream ends after 3
    .collect().await;            // drain into a collection

// .then(f) runs an async fn per item, in order (awaits each before the next):
stream.then(|x| async move { fetch(x).await });
// .fold(init, f) reduces to one value; .scan carries running state emitting items.
```

Time-aware (need tokio time):
```rust
stream.throttle(Duration::from_millis(100));  // rate-limit
stream.timeout(Duration::from_secs(1));       // Item becomes Result<T, Elapsed>
stream.chunks_timeout(100, Duration::from_millis(50)); // batch up to N or until timeout
```

---

## 4. Combining streams — fan-in

**`merge` — two streams into one** (yields items as they arrive from *either*; ends when
*both* end):
```rust
use tokio_stream::StreamExt;
let a = tokio_stream::iter(vec![1, 3, 5]);
let b = tokio_stream::iter(vec![2, 4, 6]);
let mut merged = a.merge(b);        // interleaved by readiness
```

**`StreamMap` — dynamic, keyed fan-in.** Merge *N* streams, add/remove them at runtime,
and learn *which* stream produced each item (you get `(key, item)`). This is the tool
for "I have a changing set of sources."
```rust
use tokio_stream::StreamMap;
let mut map = StreamMap::new();
map.insert("clock", IntervalStream::new(interval).map(|_| "tick"));
map.insert("net",   ReceiverStream::new(rx));
while let Some((who, msg)) = map.next().await {  // know the source
    if done { map.remove("net"); }               // detach a source live
}
```
`merge` is fixed-arity and forgets the source; `StreamMap` is dynamic and tags the source.

---

## 5. Concurrency over a stream — fan-out

Sometimes each item kicks off async work and you want *many in flight at once* (bounded).

- **`buffered(n)`** — stream of futures → run up to `n` concurrently, emit results **in order**.
- **`buffer_unordered(n)`** — same, but emit results **as they finish** (faster, reordered).
- **`for_each_concurrent(n, f)`** — run `f` on each item, up to `n` at a time, no results out.

```rust
use futures::StreamExt;             // these live on futures' StreamExt
let results: Vec<_> = futures::stream::iter(urls)
    .map(|u| async move { fetch(u).await })   // Stream<Item = Future>
    .buffer_unordered(8)                        // <= 8 fetches at once
    .collect().await;
```
`n` is your **concurrency limit** — the backpressure knob that stops you launching 10k
tasks at once. (For CPU-bound or fully independent jobs, `tokio::spawn` + a `JoinSet` is
the alternative; streams shine when the *inputs* themselves arrive as a stream.)

---

## 6. Backpressure

Backpressure = a slow consumer *slows the producer* instead of letting an unbounded queue
grow. You get it for free with **bounded** channels: `mpsc::channel(cap)` — `tx.send().await`
*suspends* when the buffer is full until the consumer drains one. (`unbounded_channel` has
no backpressure — the queue can grow without limit. Prefer bounded.)

On the consuming side, `buffer_unordered(n)` / `for_each_concurrent(n)` bound in-flight work;
`throttle` and `chunks_timeout` shape rate and batching.

---

## 7. Cancellation & shutdown

A stream stops when it yields `None`, or when you simply **drop it**. To stop early on a
signal, the main tools are:

- **`take_until(fut)`** — yield items until `fut` completes, then end the stream.
  ```rust
  let mut s = source.take_until(tokio::time::sleep(Duration::from_secs(5)));
  ```
- **`tokio_util::sync::CancellationToken`** — a shared, cloneable cancel signal.
  ```rust
  let token = CancellationToken::new();
  let child = token.clone();
  // in a task: use token.cancelled() as a future, e.g. with take_until or select!
  token.cancel(); // flips every clone
  ```
- **`tokio::select!`** — race the next item against a cancel/timeout branch:
  ```rust
  loop {
      tokio::select! {
          maybe = stream.next() => match maybe { Some(x) => handle(x), None => break },
          _ = token.cancelled() => break,   // graceful stop
      }
  }
  ```

> ⚠️ **`select!` drops the not-chosen futures.** With `stream.next()` this is fine (the
> stream keeps its state; you re-poll next loop). But do **not** put a non-cancel-safe
> future (e.g. one holding a half-read buffer) directly in a `select!` branch — wrap
> long-lived state in the stream itself. This "cancellation safety" question is a classic
> interview topic.

---

## 8. Implementing your own stream (four ways, easiest → most control)

1. **`async_stream::stream!`** — write it like an async fn; `yield` items, `.await` freely.
   Ergonomic, handles the state machine for you. Result is `!Unpin`, so `tokio::pin!` it.
   ```rust
   fn ticks(n: u64) -> impl Stream<Item = u64> {
       async_stream::stream! { for i in 0..n { yield i; tokio::time::sleep(ms(10)).await; } }
   }
   ```
   For a **fallible** stream, use **`async_stream::try_stream!`** (the producer-side mirror of
   §9's `TryStreamExt`): the `Item` is `Result<T, E>`, `yield v` emits `Ok(v)`, and `?` on an
   `Err` emits `Err(e)` and *ends* the stream — one `?` gives you "stop on first failure".
   ```rust
   fn parse(tokens: Vec<&str>) -> impl Stream<Item = Result<u64, MyErr>> {
       async_stream::try_stream! {
           for t in tokens { let n = t.parse().map_err(|_| MyErr)?; yield n; }
       }
   }   // runnable: ex14_try_stream_macro
   ```
2. **`stream::unfold(state, f)`** — for a pure "state → Option<(item, next state)>" generator.
3. **Wrap an inner stream (a combinator)** — hold another stream, transform in `poll_next`.
   If the inner is `Unpin` you can `Pin::new(&mut self.inner).poll_next(cx)` with no unsafe.
   For the general `!Unpin` case, use the `pin-project` crate to project the pin safely.
4. **Manual `impl Stream` with a hand-written state machine** — full control. You write
   `poll_next(self: Pin<&mut Self>, cx)`, return `Ready(Some)/Ready(None)/Pending`, and
   when you're `Pending` you must ensure the waker gets called later (poll a child future,
   or `cx.waker().wake_by_ref()`). This teaches you what the runtime actually does.

**Pin in one paragraph:** `poll_next` takes `Pin<&mut Self>` because a stream that holds an
`.await` point across polls may contain self-references; `Pin` promises the value won't move
so those pointers stay valid. Practical rules: to consume a bare `impl Stream` value, pin it
with `tokio::pin!(s)` (stack) or `Box::pin(s)` (heap); most concrete streams from the
ecosystem are `Unpin` and Just Work; only manual/generator streams force you to think about it.

---

## 9. Error handling — streams of `Result<T, E>` with `TryStreamExt`

Real pipelines fail. When `Item = Result<T, E>`, `futures::TryStreamExt` treats `E` as a
short-circuit channel — the stream analogue of `?`. Import it *alongside* `StreamExt`
(their method names don't collide: `try_*`/`map_ok`/`map_err`/`and_then` vs `map`/`then`).

**Ok/Err transforms** (keep streaming; touch one channel):
```rust
use futures::TryStreamExt;
stream                             // Stream<Item = Result<T, E>>
    .map_ok(|t| ...)               // transform only Ok values
    .map_err(|e| ...)              // transform / normalize the error type
    .and_then(|t| async { ... })   // chain a fallible async step (propagates existing Err)
    .or_else(|e| async { ... })    // recover from an error
    .inspect_ok(|t| ...).inspect_err(|e| ...); // logging taps
```

**Fallible consumers — these SHORT-CIRCUIT on the first `Err`:**
```rust
while let Some(item) = s.try_next().await? { ... }    // ? works: Result<Option<T>, E>
let v:  Result<Vec<T>, E> = s.try_collect().await;    // stops at first Err
let a:  Result<A, E>      = s.try_fold(init, |a, t| async { Ok(...) }).await;
let r:  Result<(), E>     = s.try_for_each(|t| async { Ok(()) }).await;
let r:  Result<(), E>     = s.try_for_each_concurrent(n, |t| async { Ok(()) }).await; // bounded
// bounded concurrent fallible work: map each Ok into a job, run <= n at once, short-circuit
let v:  Result<Vec<T>, E> = s.map_ok(|x| async move { work(x).await })
                             .try_buffer_unordered(n).try_collect().await;
```

**The trap:** `try_collect`/`try_for_each`/`try_*` drop the rest of the stream at the first
error. If you must see EVERY outcome (collect all errors, count failures), do NOT use `try_*`:
```rust
let all: Vec<Result<T, E>> = s.collect().await;       // keep every result
let (oks, errs): (Vec<_>, Vec<_>) = all.into_iter().partition(Result::is_ok);
```
(Bonus: `tokio_stream::StreamExt::collect` can target `Result<Vec<T>, E>` directly — like
`Iterator::collect::<Result<_,_>>()` — short-circuiting, without pulling in `TryStreamExt`.)

Runnable: `ex13_try_streams`. Drills: **d10–d12**.

---

## 10. Gotchas checklist (a.k.a. interview traps)

- **Nothing runs until polled.** Building a chain has no side effects; consume it.
- **Forgot `use ...StreamExt`** → "no method named `next`/`map`". Import the right one.
- **Two `StreamExt`s imported** → ambiguity on `.next()`. Import one; qualify the other.
- **`merge` ends when both end; `select!` you control.** Know which "done" you mean.
- **`buffered` preserves order; `buffer_unordered` doesn't.** Pick intentionally.
- **`unbounded_channel` = no backpressure.** Memory can blow up. Default to bounded.
- **`select!` = cancellation.** Only put cancel-safe futures in branches; keep state in streams.
- **Holding a `MutexGuard`/`RefCell` borrow across a stream `.await`** → deadlock/panic risk.
- **`impl Stream` from `async-stream`/manual is `!Unpin`** → `tokio::pin!` before `.next()`.
- **A manual `poll_next` that returns `Pending` without registering a waker** → hangs forever.
- **`try_collect`/`try_for_each` short-circuit** and drop the rest on first `Err`. Want all
  outcomes? `collect()` into a `Vec<Result<..>>` instead.

---

## 11. API cheat sheet

```text
Create:   tokio_stream::{iter, once, empty, pending}
          wrappers::{ReceiverStream, BroadcastStream, WatchStream, IntervalStream}
          futures::stream::{unfold, repeat, repeat_with}
          async_stream::{stream!, try_stream!}

Consume:  s.next().await            while let Some(x) = s.next().await
          s.collect().await         s.fold(a, f).await        s.all/any(f).await
          (count/for_each/take_until/enumerate/scan live on futures::StreamExt, not tokio's)

Transform (tokio_stream::StreamExt): map map_while filter filter_map then take take_while
          skip skip_while fold merge chain timeout throttle chunks_timeout
Transform (futures::StreamExt adds):  enumerate scan take_until zip flatten inspect ...

Concurrency (futures::StreamExt): buffered(n)  buffer_unordered(n)  for_each_concurrent(n, f)

Combine:  a.merge(b)               StreamMap::{new, insert, remove, keys, next}

Errors (futures::TryStreamExt on Stream<Item = Result<T, E>>):
          try_next try_collect try_fold try_for_each try_for_each_concurrent(n, f)
          try_buffer_unordered(n)  map_ok map_err and_then or_else inspect_ok inspect_err

Cancel:   s.take_until(fut)        CancellationToken::{new, clone, cancel, cancelled}
          tokio::select! { x = s.next() => .., _ = token.cancelled() => .. }
          tokio::time::timeout(dur, s.next())

Pin:      tokio::pin!(s)           Box::pin(s)          (needed for !Unpin streams)
```

---

## How to work through this course

1. **Read** this file top to bottom once (you just did).
2. **Run** the worked examples in order — they print what's happening:
   `cargo run -p tokio-streams-course --bin ex01_basics` … through `ex12_capstone`.
   Read the source alongside the output; each ends with a `YOUR TURN` tweak to try.
3. **Practice for real:** open `src/bin/drills.rs`, replace each `todo!()`, and run
   `cargo test -p tokio-streams-course --bin drills` until green. Peek at
   `drills_solved.rs` only when stuck.

See `README.md` for the exercise index and difficulty ramp.
