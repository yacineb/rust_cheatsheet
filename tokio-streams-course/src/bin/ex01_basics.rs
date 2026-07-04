//! ex01 — Stream basics: creating and consuming.
//!
//! A `Stream` is an async `Iterator`. Everything here mirrors iterators, except we
//! `.await` when pulling the next item. Run me:
//!   cargo run -p tokio-streams-course --bin ex01_basics
//!
//! Concepts: tokio_stream::{iter, once, empty}, `.next().await`, the `while let` loop,
//! and the fact that streams are LAZY (do nothing until consumed).

use tokio_stream::StreamExt; // brings `.next()`, `.map()`, `.count()`, ... into scope

#[tokio::main]
async fn main() {
    // (1) Build a stream from anything iterable.
    let mut s = tokio_stream::iter(vec![10, 20, 30]);

    // The async equivalent of a `for` loop. `.next()` returns a Future<Output=Option<T>>.
    while let Some(n) = s.next().await {
        println!("got {n}");
    }

    // (2) A single-item stream and an empty stream. (tokio_stream::StreamExt has no
    // `.count()`, so we fold to count — a good reminder that combinators are just folds.)
    let one = tokio_stream::once("hello");
    let none = tokio_stream::empty::<&str>();
    println!("once yields {} item(s)", one.fold(0, |n, _| n + 1).await); // 1
    println!("empty yields {} item(s)", none.fold(0, |n, _| n + 1).await); // 0

    // (3) LAZINESS: building a chain runs no code. The closure only fires on consumption.
    let lazy = tokio_stream::iter(0..3).map(|x| {
        println!("  ...mapping {x}"); // notice: nothing prints until we consume below
        x * x
    });
    println!("built the chain, nothing mapped yet");
    let squares: Vec<i32> = lazy.collect().await; // NOW the closure runs
    println!("squares = {squares:?}");

    // (4) `.next()` on a fused, finished stream keeps returning None.
    let mut done = tokio_stream::iter(std::iter::empty::<i32>());
    assert_eq!(done.next().await, None);

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ Build a stream of 1..=5, keep only even numbers, and print their │
    // │ running sum (2, 6). Hint: `.filter(|x| x % 2 == 0)` then a       │
    // │ `while let` accumulating into a mutable `sum`. Or try `.fold`.    │
    // └──────────────────────────────────────────────────────────────────┘
}
