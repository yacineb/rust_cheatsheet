//! ex06 — Fan-out: run async work for many items concurrently, but BOUNDED.
//!
//!   cargo run -p tokio-streams-course --bin ex06_fan_out
//!
//! Each item spawns async work (a "fetch"). We want several in flight at once — but
//! not 10,000. The knob is the concurrency limit `n`.
//!   * buffered(n)          -> results IN ORDER, up to n at a time
//!   * buffer_unordered(n)  -> results AS THEY FINISH (faster), reordered
//!   * for_each_concurrent(n, f) -> run side effects, no results collected
//!
//! These three live on `futures::StreamExt`, NOT tokio's — so this file imports
//! `futures::StreamExt` (and uses `futures::stream::iter`). Mixing both StreamExt
//! traits in one file makes `.next()` ambiguous, so we commit to one here.

use futures::stream::StreamExt; // buffered / buffer_unordered / for_each_concurrent
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Fake I/O whose latency depends on the input, so ordering differences are visible.
async fn fetch(id: u64) -> (u64, u64) {
    let latency = 60u64.saturating_sub(id * 10).max(10); // id 0 slow, id 5 fast
    tokio::time::sleep(Duration::from_millis(latency)).await;
    (id, latency)
}

#[tokio::main]
async fn main() {
    // buffered(3): keeps output order = input order, even though work overlaps.
    let start = Instant::now();
    let ordered: Vec<(u64, u64)> = futures::stream::iter(0..6)
        .map(fetch) // Stream<Item = Future<Output=(u64,u64)>>
        .buffered(3) // up to 3 concurrent, results emitted in input order
        .collect()
        .await;
    println!("buffered(3)  -> {ordered:?}  in {:?}", start.elapsed());

    // buffer_unordered(3): same concurrency, but fastest finishers come out first.
    let start = Instant::now();
    let unordered: Vec<(u64, u64)> = futures::stream::iter(0..6)
        .map(fetch)
        .buffer_unordered(3)
        .collect()
        .await;
    println!("buffer_unordered(3) -> {unordered:?}  in {:?}", start.elapsed());

    // for_each_concurrent(n, f): run side effects with bounded concurrency, no Vec out.
    let inflight = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (inflight_c, peak_c) = (inflight.clone(), peak.clone());
    futures::stream::iter(0..12)
        .for_each_concurrent(4, |id| {
            let (inflight, peak) = (inflight_c.clone(), peak_c.clone());
            async move {
                let now = inflight.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst); // track max concurrency reached
                let _ = fetch(id).await;
                inflight.fetch_sub(1, Ordering::SeqCst);
            }
        })
        .await;
    println!(
        "for_each_concurrent(4): peak simultaneous tasks = {} (never exceeds the limit)",
        peak.load(Ordering::SeqCst)
    );

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ Time `buffered(1)` vs `buffered(6)` over the same 0..6 inputs.    │
    // │ 1 is fully serial; 6 is fully parallel. Print both durations and  │
    // │ confirm the wall-clock difference matches the concurrency.        │
    // └──────────────────────────────────────────────────────────────────┘
}
