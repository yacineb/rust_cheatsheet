//! ex07 — Cancellation & graceful shutdown.
//!
//!   cargo run -p tokio-streams-course --bin ex07_cancellation
//!
//! Three idioms, weakest to strongest control:
//!   1. `take_until(fut)`   — end the stream when a future fires (e.g. a deadline).
//!   2. `CancellationToken` — a cloneable, shared "stop now" signal for many tasks.
//!   3. `select!`           — race "next item" against "cancel" and decide per-iteration.
//!
//! Two things to notice:
//!  * `take_until` lives on `futures::StreamExt` (not tokio's), so this file imports that.
//!  * `take_until(sleep/cancelled())` wraps a !Unpin future, so we `tokio::pin!` the
//!    stream before calling `.next()` (which requires the stream be Unpin).
//!
//! Key subtlety (classic interview point): `select!` DROPS the branch futures it didn't
//! pick. `stream.next()` is cancel-safe (the stream keeps its state; we just re-poll it
//! next loop). Do NOT put a future holding fragile mid-operation state in a select branch.

use futures::StreamExt; // map, next, take_until
use std::time::Duration;
use tokio_stream::wrappers::IntervalStream;
use tokio_util::sync::CancellationToken;

fn clock(period_ms: u64) -> impl futures::Stream<Item = u64> {
    let mut n = 0u64;
    IntervalStream::new(tokio::time::interval(Duration::from_millis(period_ms))).map(move |_| {
        n += 1;
        n
    })
}

#[tokio::main]
async fn main() {
    // ── 1. take_until: stop after a deadline future completes. ──
    let deadline = tokio::time::sleep(Duration::from_millis(120));
    let ticks = clock(30).take_until(deadline);
    tokio::pin!(ticks); // !Unpin (holds a Sleep) -> pin before .next()
    let mut count = 0;
    while let Some(_n) = ticks.next().await {
        count += 1;
    }
    println!("take_until: got {count} ticks before the 120ms deadline\n");

    // ── 2. CancellationToken: one signal, many listeners. ──
    let token = CancellationToken::new();

    // A worker that stops when EITHER its stream ends OR the token is cancelled.
    let worker_token = token.clone();
    let worker = tokio::spawn(async move {
        // `take_until(token.cancelled())` ends the stream on cancel — clean and stateless.
        let s = clock(20).take_until(worker_token.cancelled());
        tokio::pin!(s);
        let mut seen = 0;
        while let Some(_n) = s.next().await {
            seen += 1;
        }
        seen
    });

    // Let it run, then pull the plug.
    tokio::time::sleep(Duration::from_millis(100)).await;
    token.cancel(); // flips every clone at once
    let seen = worker.await.unwrap();
    println!("CancellationToken: worker saw {seen} ticks, then cancel() stopped it\n");

    // ── 3. select!: full manual control — handle items and cancel in one loop. ──
    let token = CancellationToken::new();
    let ctrl = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(90)).await;
        ctrl.cancel();
    });

    let mut s = clock(25);
    let mut handled = 0;
    loop {
        tokio::select! {
            // `biased;` would poll branches top-to-bottom; default is random for fairness.
            maybe = s.next() => match maybe {
                Some(n) => { handled += 1; println!("select: handled tick {n}"); }
                None => break, // stream ended
            },
            _ = token.cancelled() => {
                println!("select: cancel received — draining & shutting down");
                break;
            }
        }
    }
    println!("select loop handled {handled} ticks before shutdown");

    // Bonus: a hard timeout on a single await (returns Err on elapse). We build a stream
    // whose first item takes 100ms (note: `interval`'s first tick fires *immediately*, so
    // it would NOT time out — a common surprise), so the 10ms timeout genuinely elapses.
    let slow = tokio_stream::iter(std::iter::repeat(0u64))
        .then(|_| async { tokio::time::sleep(Duration::from_millis(100)).await });
    tokio::pin!(slow);
    let one = tokio::time::timeout(Duration::from_millis(10), slow.next()).await;
    println!("timeout(10ms) on a slow next() -> elapsed? {}", one.is_err());

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ Spawn 3 workers that all share ONE CancellationToken clone and    │
    // │ each consume `clock(...)` via take_until(token.cancelled()).       │
    // │ Cancel once after 100ms and confirm ALL three stop. This is the   │
    // │ real shutdown pattern for a server with many connection tasks.    │
    // └──────────────────────────────────────────────────────────────────┘
}
