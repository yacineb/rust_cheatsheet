//! ex10 — Building streams without hand-writing poll_next: `unfold` and `stream!`.
//!
//!   cargo run -p tokio-streams-course --bin ex10_impl_generators
//!
//! 99% of the time you don't need a manual `impl Stream` (ex09). Reach for these:
//!   * `futures::stream::unfold(state, f)` — pure "state -> Option<(item, next_state)>".
//!     f may be async, so you can await I/O between items.
//!   * `async_stream::stream! { ... yield x; ... }` — write it like an async fn: loops,
//!     `.await`, `?`, early return all work; `yield` emits an item. Most readable option.
//!
//! Both produce `!Unpin` streams, so pin them (`tokio::pin!`) before `.next()`.

use async_stream::stream;
use futures::StreamExt; // unfold lives in futures; use its StreamExt here too
use std::time::Duration;
use tokio_stream::Stream;

/// unfold version: an async counter that "loads" each value with a delay.
fn counter_unfold(max: u64) -> impl Stream<Item = u64> {
    futures::stream::unfold(0u64, move |n| async move {
        if n >= max {
            None // ending the stream
        } else {
            tokio::time::sleep(Duration::from_millis(5)).await; // await between items is fine
            Some((n, n + 1)) // (item to yield, next state)
        }
    })
}

/// stream! version of the SAME thing — reads top to bottom like normal async code.
fn counter_macro(max: u64) -> impl Stream<Item = u64> {
    stream! {
        for n in 0..max {
            tokio::time::sleep(Duration::from_millis(5)).await;
            yield n;
        }
    }
}

/// stream! shines when the logic is non-trivial: here, a retrying "poller" that
/// yields a value, occasionally "fails" and retries, and stops after N successes.
fn flaky_poller(successes: u32) -> impl Stream<Item = String> {
    stream! {
        let mut done = 0;
        let mut attempt = 0u32;
        while done < successes {
            attempt += 1;
            if attempt % 3 == 0 {
                // pretend a transient failure -> log & retry, DON'T yield
                yield format!("attempt {attempt}: transient error, retrying");
                continue;
            }
            done += 1;
            yield format!("attempt {attempt}: success #{done}");
        }
    }
}

#[tokio::main]
async fn main() {
    let a = counter_unfold(4);
    tokio::pin!(a);
    let mut va = Vec::new();
    while let Some(n) = a.next().await {
        va.push(n);
    }
    println!("unfold  -> {va:?}");

    let b = counter_macro(4);
    tokio::pin!(b);
    let mut vb = Vec::new();
    while let Some(n) = b.next().await {
        vb.push(n);
    }
    println!("stream! -> {vb:?}  (identical)\n");

    let poller = flaky_poller(3);
    tokio::pin!(poller);
    while let Some(line) = poller.next().await {
        println!("{line}");
    }

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ Write `lines_of(text: &'static str) -> impl Stream<Item=String>`  │
    // │ using `stream!` that yields each line, but SKIPS blank lines and   │
    // │ stops at a line equal to "END". Then collect and print the result. │
    // └──────────────────────────────────────────────────────────────────┘
}
