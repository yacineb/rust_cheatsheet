//! ex14 — Implementing a *fallible* stream with `try_stream!`.
//!
//!   cargo run -p tokio-streams-course --bin ex14_try_stream_macro
//!
//! Companion to ex10 (`stream!`) and ex13 (`TryStreamExt`). `async_stream::try_stream!`
//! lets you WRITE a `Stream<Item = Result<T, E>>` as if it were a fallible async fn:
//!   * `yield value;`  emits `Ok(value)`
//!   * `?`             on an `Err` emits `Err(e)` and then ENDS the stream
//! So a single `?` buys you "stop the stream on the first failure" for free — the exact
//! producer-side mirror of ex13's `try_collect`/`try_next` on the consumer side.
//!
//! The error type is pinned down by the function's return type:
//!   `-> impl Stream<Item = Result<T, E>>`.
//! Like `stream!`, the result is `!Unpin`, so `tokio::pin!` it before consuming.

use async_stream::try_stream;
use futures::{StreamExt, TryStreamExt};
use std::time::Duration;
use tokio_stream::Stream;

#[derive(Debug)]
#[allow(dead_code)] // fields read only via derived Debug
enum ReadError {
    Parse(String),
    TooBig(u64),
}

/// Parse each token into a u64 and yield it. A bad token turns into `yield Err(..)` and
/// the stream ends. Note how `?` (via `.map_err`) does all the short-circuiting.
fn parse_tokens(tokens: Vec<&'static str>) -> impl Stream<Item = Result<u64, ReadError>> {
    try_stream! {
        for tok in tokens {
            let n: u64 = tok.parse().map_err(|_| ReadError::Parse(tok.to_string()))?;
            yield n; // emits Ok(n)
        }
    }
}

/// A validation helper so we can use `?` on a real `Result` (avoids the bare-`Err(..)?`
/// type-inference snag). Returning `Result<(), E>` is the idiomatic "assert or bail".
fn ensure_small(value: u64) -> Result<(), ReadError> {
    if value > 100 {
        Err(ReadError::TooBig(value))
    } else {
        Ok(())
    }
}

/// Async fallible generator: "fetch pages" until one is too big. `?` mid-loop ends it.
fn fetch_pages(max_pages: u32) -> impl Stream<Item = Result<u64, ReadError>> {
    try_stream! {
        for page in 0..max_pages {
            tokio::time::sleep(Duration::from_millis(5)).await; // await freely inside
            let value = (page as u64 + 1) * 40;                 // 40, 80, 120, ...
            ensure_small(value)?;                               // bail -> yields Err, ends
            yield value;
        }
    }
}

#[tokio::main]
async fn main() {
    // (1) All tokens valid -> Ok stream, ends normally. Composes with ex13's try_collect.
    let s = parse_tokens(vec!["1", "2", "3"]);
    tokio::pin!(s);
    let collected: Result<Vec<u64>, ReadError> = s.try_collect().await;
    println!("parse_tokens(all good) -> {collected:?}");

    // (2) A bad token: values flow until it, then one Err, then the stream ENDS.
    //     Consume item-by-item to watch the termination (note "30" never appears).
    let s = parse_tokens(vec!["10", "20", "oops", "30"]);
    tokio::pin!(s);
    while let Some(item) = s.next().await {
        match item {
            Ok(n) => println!("  ok {n}"),
            Err(e) => println!("  err {e:?}  (stream ends here — '30' is never yielded)"),
        }
    }

    // (3) The same failure, collapsed by try_collect into a single Result.
    let s = parse_tokens(vec!["10", "oops", "30"]);
    tokio::pin!(s);
    let r: Result<Vec<u64>, ReadError> = s.try_collect().await;
    println!("parse_tokens(bad) via try_collect -> {r:?}\n");

    // (4) Async generator with a mid-stream `?` bail.
    let s = fetch_pages(10);
    tokio::pin!(s);
    while let Some(item) = s.next().await {
        println!("fetch_pages -> {item:?}");
    }
    println!("(stopped at the first page > 100)");

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ Write `read_lines(text: &'static str) -> impl Stream<Item =        │
    // │ Result<String, String>>` with `try_stream!` that yields each       │
    // │ non-empty line, but returns `Err("forbidden")?` if a line equals   │
    // │ "STOP". Drive it with both `.try_collect()` and a `while let`.      │
    // └──────────────────────────────────────────────────────────────────┘
}
