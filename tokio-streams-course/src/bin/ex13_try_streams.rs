//! ex13 — Error handling with `TryStreamExt`: streams of `Result<T, E>`.
//!
//!   cargo run -p tokio-streams-course --bin ex13_try_streams
//!
//! When `Item = Result<T, E>`, `TryStreamExt` (from `futures`) gives Result-aware
//! combinators that treat `E` as a short-circuit channel — the stream analogue of `?`.
//! Import it ALONGSIDE `StreamExt`; their method names don't collide (try_*/map_ok/
//! map_err/and_then vs map/then/collect/next).
//!
//! Two families:
//!   * Ok/Err transforms:  map_ok, map_err, and_then, or_else, inspect_ok/err, err_into
//!   * Fallible consumers (STOP at first Err): try_next, try_collect, try_fold,
//!     try_for_each, try_for_each_concurrent, try_buffer_unordered
//!
//! GOLDEN RULE: the try_* consumers **short-circuit** — on the first Err they stop and
//! return it; remaining items are NOT processed. If you need every outcome (collect all
//! errors), don't use try_*; map to Result and collect into a `Vec<Result<..>>` (see #7).

use futures::{StreamExt, TryStreamExt};
use std::time::Duration;

#[derive(Debug, Clone)]
#[allow(dead_code)] // fields are only read via the derived Debug, which the lint ignores
enum ParseError {
    Empty,
    NotANumber(String),
    Negative(i64),
}

/// Fallible async "parse": pretend each parse touches I/O.
async fn parse_positive(s: &str) -> Result<i64, ParseError> {
    tokio::time::sleep(Duration::from_millis(3)).await;
    if s.is_empty() {
        return Err(ParseError::Empty);
    }
    let n: i64 = s.parse().map_err(|_| ParseError::NotANumber(s.to_string()))?;
    if n < 0 {
        return Err(ParseError::Negative(n));
    }
    Ok(n)
}

/// (1) `try_next` returns `Result<Option<T>, E>`, so `?` works right in the loop head.
async fn sum_until_error(items: Vec<Result<i64, ParseError>>) -> Result<i64, ParseError> {
    let mut s = futures::stream::iter(items);
    let mut total = 0;
    while let Some(n) = s.try_next().await? {
        // ? bubbles the first Err out of the fn
        total += n;
    }
    Ok(total)
}

#[tokio::main]
async fn main() {
    // ── (1) try_next + `?`: a fallible consumer loop. ──
    let ok = sum_until_error(vec![Ok(1), Ok(2), Ok(3)]).await;
    let bad = sum_until_error(vec![Ok(1), Err(ParseError::Empty), Ok(99)]).await;
    println!("try_next:  ok={ok:?}   bad={bad:?}   (never reached the 99)\n");

    // ── (2) try_collect: Result<Vec<T>, E>, stopping at the first Err. ──
    let all_good: Result<Vec<i64>, ParseError> = futures::stream::iter(vec!["1", "2", "3"])
        .then(parse_positive) // Stream<Item = Result<i64, ParseError>>
        .try_collect()
        .await;
    println!("try_collect (all good) -> {all_good:?}");

    let with_bad: Result<Vec<i64>, ParseError> = futures::stream::iter(vec!["1", "x", "3", "-5"])
        .then(parse_positive)
        .try_collect()
        .await;
    println!("try_collect (has bad) -> {with_bad:?}   (short-circuits at \"x\")\n");

    // ── (3) map_ok / map_err: transform the two channels independently. ──
    let transformed: Vec<Result<String, String>> = futures::stream::iter(vec!["10", "-2", "20"])
        .then(parse_positive)
        .map_ok(|n| n * 100) // touch only the Ok values
        .map_err(|e| format!("parse failed: {e:?}")) // normalize the error type
        .map_ok(|n| format!("#{n}"))
        .collect() // keep every Result so we can see both channels
        .await;
    println!("map_ok/map_err -> {transformed:?}\n");

    // ── (4) and_then: chain another fallible async step; propagates existing Err. ──
    async fn ensure_even(n: i64) -> Result<i64, ParseError> {
        if n % 2 == 0 {
            Ok(n)
        } else {
            Err(ParseError::NotANumber(format!("{n} is odd")))
        }
    }
    let chained: Vec<Result<i64, ParseError>> = futures::stream::iter(vec!["2", "3", "4"])
        .then(parse_positive)
        .and_then(ensure_even) // runs only on Ok items
        .collect()
        .await;
    println!("and_then -> {chained:?}\n");

    // ── (5) try_for_each_concurrent: bounded concurrent work that aborts on first Err. ──
    async fn validate(n: i64) -> Result<(), ParseError> {
        tokio::time::sleep(Duration::from_millis(5)).await;
        if n == 3 {
            Err(ParseError::Negative(n)) // pretend 3 fails validation
        } else {
            Ok(())
        }
    }
    let outcome = futures::stream::iter(vec![1, 2, 3, 4, 5])
        .map(Ok::<i64, ParseError>) // make it a TryStream of Ok(n)
        .try_for_each_concurrent(4, |n| async move { validate(n).await })
        .await;
    println!("try_for_each_concurrent -> {outcome:?}   (aborted when 3 failed)\n");

    // ── (6) try_buffer_unordered: bounded concurrent FALLIBLE work over a TryStream. ──
    // It operates on a TryStream whose Ok item is itself a fallible future, so the natural
    // shape is: start from a stream of Results, `map_ok` each Ok into an async job, then
    // `try_buffer_unordered(n)` runs <= n jobs at once and short-circuits on ANY Err
    // (whether from the source stream or from a job).
    let squares: Result<Vec<i64>, ParseError> =
        futures::stream::iter(vec![Ok::<&str, ParseError>("1"), Ok("2"), Ok("3"), Ok("4")])
            .map_ok(|s| async move { parse_positive(s).await.map(|n| n * n) })
            .try_buffer_unordered(3)
            .try_collect()
            .await;
    println!("try_buffer_unordered (all ok) -> {squares:?}");

    let squares_bad: Result<Vec<i64>, ParseError> =
        futures::stream::iter(vec![Ok::<&str, ParseError>("2"), Ok("oops"), Ok("4")])
            .map_ok(|s| async move { parse_positive(s).await.map(|n| n * n) })
            .try_buffer_unordered(3)
            .try_collect()
            .await;
    println!("try_buffer_unordered (has bad) -> {squares_bad:?}\n");

    // ── (7) The OPPOSITE of short-circuit: keep EVERY outcome. ──
    // Don't use try_*; collect into a Vec<Result<..>> and partition yourself.
    let results: Vec<Result<i64, ParseError>> = futures::stream::iter(vec!["1", "", "3", "-9", "5"])
        .then(parse_positive)
        .collect()
        .await;
    let (oks, errs): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);
    let oks: Vec<i64> = oks.into_iter().map(Result::unwrap).collect();
    let errs: Vec<ParseError> = errs.into_iter().map(Result::unwrap_err).collect();
    println!("collect-all -> {} ok {oks:?}, {} err {errs:?}", oks.len(), errs.len());

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ 1. Rewrite (2) using `try_fold` to SUM the parsed values into a    │
    // │    Result<i64, ParseError> (stops at the first bad input).         │
    // │ 2. Add `.inspect_err(|e| eprintln!("skipping {e:?}"))` before a    │
    // │    `try_collect` and watch when it fires vs. doesn't.              │
    // │ 3. Give parse_positive a second error type and unify them in the   │
    // │    pipeline with `.map_err(Into::into)` + a `From` impl.           │
    // └──────────────────────────────────────────────────────────────────┘
}
