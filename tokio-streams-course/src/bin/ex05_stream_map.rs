//! ex05 — `StreamMap`: dynamic, keyed fan-in.
//!
//!   cargo run -p tokio-streams-course --bin ex05_stream_map
//!
//! `merge` is fixed-arity and forgets the source. `StreamMap<K, S>` fixes both:
//!   * you learn WHICH stream produced each item — it yields `(K, Item)`
//!   * you can `insert`/`remove` sources at RUNTIME (subscriptions, connections, ...)
//! When a member stream ends, StreamMap drops it automatically. It yields None only
//! once ALL members are gone.

use std::time::Duration;
use tokio_stream::wrappers::{IntervalStream, ReceiverStream};
use tokio_stream::{StreamExt, StreamMap};

fn ticker(period_ms: u64) -> impl tokio_stream::Stream<Item = ()> {
    IntervalStream::new(tokio::time::interval(Duration::from_millis(period_ms))).map(|_| ())
}

#[tokio::main]
async fn main() {
    // Key by a &'static str name. Values must share a type, so we Box the streams
    // to unify their concrete types behind `Pin<Box<dyn Stream>>`.
    let mut map: StreamMap<&'static str, std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String>>>> =
        StreamMap::new();

    // Source A: a fast clock.
    map.insert("clock", Box::pin(ticker(30).map(|_| "tick".to_string()).take(6)));

    // Source B: a network-ish channel we feed from a task.
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(8);
    tokio::spawn(async move {
        for m in ["hello", "world", "bye"] {
            tokio::time::sleep(Duration::from_millis(45)).await;
            let _ = tx.send(m.to_string()).await;
        }
    });
    map.insert("net", Box::pin(ReceiverStream::new(rx)));

    // Consume the combined stream. We KNOW the source of every item.
    let mut net_msgs = 0;
    while let Some((who, msg)) = map.next().await {
        println!("[{who:>5}] {msg}   (live sources: {})", map.len());

        if who == "net" {
            net_msgs += 1;
            // Dynamically ATTACH a new source after the first net message.
            if net_msgs == 1 {
                map.insert(
                    "late",
                    Box::pin(tokio_stream::iter(vec!["A".to_string(), "B".to_string()])),
                );
                println!("   -> attached 'late' source at runtime");
            }
            // Dynamically DETACH the clock once net says goodbye.
            if msg == "bye" {
                if map.remove("clock").is_some() {
                    println!("   -> detached 'clock' source at runtime");
                }
            }
        }
    }
    println!("all sources drained");

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ Give each ticker a different rate and key ("t1"=20ms, "t2"=35ms).│
    // │ Keep a HashMap<&str, u32> counting items per key. After 300ms of  │
    // │ wall-clock (wrap the loop in tokio::time::timeout), print the      │
    // │ per-source counts. Which source produced more? Why?               │
    // └──────────────────────────────────────────────────────────────────┘
}
