//! ex03 — Channels ARE streams: bridging producers and consumers.
//!
//!   cargo run -p tokio-streams-course --bin ex03_channels
//!
//! The most common real-world stream isn't `iter(...)` — it's "a task produces values,
//! another consumes them". `tokio::sync::mpsc` + `ReceiverStream` is the bridge.
//! We also peek at `broadcast` (fan-out to many) and `watch` (latest value only).

use std::time::Duration;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream, WatchStream};
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    // ── mpsc: many producers, one consumer. The receiver becomes a Stream. ──
    let (tx, rx) = tokio::sync::mpsc::channel::<i32>(8); // bounded => backpressure (ex08)

    // Producer task: sends 5 values then drops tx (which ENDS the stream via None).
    tokio::spawn(async move {
        for i in 0..5 {
            if tx.send(i).await.is_err() {
                break; // receiver went away
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        // tx dropped here -> ReceiverStream will yield None and the loop below ends.
    });

    let mut stream = ReceiverStream::new(rx);
    let mut total = 0;
    while let Some(v) = stream.next().await {
        total += v;
        println!("mpsc consumer got {v}");
    }
    println!("mpsc closed, total = {total}\n");

    // ── broadcast: every subscriber gets every message (fan-out). Items are Results
    //    because a slow subscriber can lag and miss messages. ──
    let (btx, brx1) = tokio::sync::broadcast::channel::<&str>(16);
    let brx2 = btx.subscribe();
    btx.send("alpha").unwrap();
    btx.send("beta").unwrap();
    drop(btx); // close so the streams terminate

    let s1: Vec<_> = BroadcastStream::new(brx1)
        .filter_map(|r| r.ok()) // drop Lagged errors
        .collect()
        .await;
    let s2: Vec<_> = BroadcastStream::new(brx2).filter_map(|r| r.ok()).collect().await;
    println!("broadcast sub1 = {s1:?}");
    println!("broadcast sub2 = {s2:?}  (both saw every message)\n");

    // ── watch: consumers only care about the LATEST value (config, state). ──
    let (wtx, wrx) = tokio::sync::watch::channel("v0");
    tokio::spawn(async move {
        for v in ["v1", "v2", "v3"] {
            tokio::time::sleep(Duration::from_millis(5)).await;
            let _ = wtx.send(v);
        }
    });
    // WatchStream yields the current value and then each change. Take 2 and stop.
    let seen: Vec<_> = WatchStream::new(wrx).take(2).collect().await;
    println!("watch first two observed = {seen:?}");

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ Spawn TWO producer tasks that both hold a clone of the same mpsc │
    // │ `tx` (mpsc = multi-producer). Have them send 0..3 and 100..103.  │
    // │ Consume via one ReceiverStream and confirm you get all 6 values. │
    // │ Hint: clone `tx` before `move` into each task; drop the original.│
    // └──────────────────────────────────────────────────────────────────┘
}
