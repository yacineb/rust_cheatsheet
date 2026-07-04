//! ex08 — Backpressure, throttling, and batching.
//!
//!   cargo run -p tokio-streams-course --bin ex08_backpressure
//!
//! Backpressure = a slow consumer slows the producer, instead of an unbounded queue
//! growing forever. Bounded `mpsc::channel(cap)` gives it to you for free: once the
//! buffer is full, `tx.send().await` SUSPENDS until the consumer drains one slot.

use std::time::{Duration, Instant};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    // ── Backpressure demo: tiny buffer + slow consumer throttles a fast producer. ──
    let (tx, rx) = tokio::sync::mpsc::channel::<u64>(2); // capacity 2 only

    let producer = tokio::spawn(async move {
        let mut send_stalls = 0;
        for i in 0..8 {
            let t0 = Instant::now();
            tx.send(i).await.unwrap(); // BLOCKS here when the buffer is full
            if t0.elapsed() > Duration::from_millis(5) {
                send_stalls += 1; // we were made to wait -> backpressure in action
            }
        }
        send_stalls
    });

    // Slow consumer: 20ms per item. The producer can't outrun it by more than `cap`.
    let mut stream = ReceiverStream::new(rx);
    while let Some(v) = stream.next().await {
        tokio::time::sleep(Duration::from_millis(20)).await;
        println!("consumed {v}");
    }
    let stalls = producer.await.unwrap();
    println!("producer was back-pressured (send stalled) {stalls} times\n");

    // ── throttle: cap the RATE of a stream (drops nothing; spaces items out). ──
    let start = Instant::now();
    let ticks: Vec<u64> = tokio_stream::iter(0..5)
        .throttle(Duration::from_millis(30)) // >= 30ms between yielded items
        .collect()
        .await;
    println!(
        "throttle: emitted {} items over {:?} (~30ms apart)\n",
        ticks.len(),
        start.elapsed()
    );

    // ── chunks_timeout: batch up to N items, but don't wait longer than a timeout.
    //    Great for amortizing per-write overhead (DB inserts, network frames). ──
    let (tx, rx) = tokio::sync::mpsc::channel::<u64>(64);
    tokio::spawn(async move {
        for i in 0..10 {
            let _ = tx.send(i).await;
            // Irregular arrival: some bursts, some gaps.
            tokio::time::sleep(Duration::from_millis(if i % 3 == 0 { 40 } else { 5 })).await;
        }
    });
    // chunks_timeout wraps a Sleep -> !Unpin -> pin before .next().
    let batches = ReceiverStream::new(rx).chunks_timeout(4, Duration::from_millis(25));
    tokio::pin!(batches);
    while let Some(batch) = batches.next().await {
        println!("batch of {}: {batch:?}", batch.len());
    }

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ Repeat the backpressure demo with `unbounded_channel()`. Observe  │
    // │ the producer NEVER stalls (0 stalls) — the queue just grows. Then  │
    // │ argue why bounded is the safer default for production.             │
    // └──────────────────────────────────────────────────────────────────┘
}
