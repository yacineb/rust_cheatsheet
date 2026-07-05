//! ex02 — Combinators: map / filter / then / take / take_while / fold / timeout.
//!
//!   cargo run -p tokio-streams-course --bin ex02_combinators
//!
//! The mapping closures may be async (`then`), and adapters may await internally
//! (`timeout`, `throttle`). Order still flows left-to-right like iterators.
//!
//! NOTE: `tokio_stream::StreamExt` is a *subset* of iterator adapters — it has
//! map/filter/then/take/take_while/fold/timeout/throttle but NOT enumerate/count/scan.
//! Those extras live on `futures::StreamExt` (see ex06). Know which trait you imported.

use std::time::Duration;
use tokio::time::error::Elapsed;
use tokio_stream::StreamExt;

async fn double_slowly(x: i32) -> i32 {
    tokio::time::sleep(Duration::from_millis(20)).await; // pretend I/O
    x * 2
}

#[tokio::main]
async fn main() {
    // Sync pipeline: transform, drop, stop early.
    let out: Vec<i32> = tokio_stream::iter(0..10)
        .filter(|x| x % 2 == 0) // 0 2 4 6 8
        .map(|x| x + 1) //          1 3 5 7 9
        .take(3) //           stop after 3 -> the stream ends
        .collect()
        .await;
    println!("pipeline -> {out:?}"); // [1, 3, 5]

    // `.then` runs an ASYNC fn per item, awaiting each before the next (ordered, serial).
    let doubled: Vec<i32> = tokio_stream::iter(vec![1, 2, 3])
        .then(double_slowly) // each awaits ~20ms, in sequence -> ~60ms total
        .collect()
        .await;
    println!("then -> {doubled:?}");

    // `.fold` reduces the whole stream to one value.
    let sum = tokio_stream::iter(1..=100).fold(0, |acc, x| acc + x).await;
    println!("fold sum 1..=100 = {sum}");

    // `.take_while` stops as soon as the predicate is false (unlike filter, which skips).
    let small: Vec<i32> = tokio_stream::iter(vec![1, 2, 3, 99, 4])
        .take_while(|&x| x < 10)
        .collect()
        .await;
    println!("take_while<10 -> {small:?}"); // [1, 2, 3]  (stops at 99)

    // `.timeout` gives each item a deadline: Item becomes Result<T, Elapsed>.
    // This stream wraps a Sleep, which is !Unpin, so we must pin it before `.next()`.
    let slow = tokio_stream::iter(vec![1, 2])
        .then(|x| async move {
            let ms = if x == 2 { 200 } else { 5 };
            tokio::time::sleep(Duration::from_millis(ms)).await;
            x
        })
        .timeout(Duration::from_millis(50));
    tokio::pin!(slow); // <-- required: pin the !Unpin stream on the stack
    while let Some(res) = slow.next().await {
        match res {
            Ok(v) => println!("timeout branch: ok {v}"),
            Err(_) => println!("timeout branch: item took too long"),
        }
    }

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ From 0..1000 keep multiples of 7, square them, and use           │
    // │ `.take_while(|&x| x < 10_000)` to stop once squares exceed 10k.  │
    // │ Collect and print how many survived.                             │
    // └──────────────────────────────────────────────────────────────────┘
}
