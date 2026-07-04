//! ex12 — Capstone: a real pipeline tying every piece together.
//!
//!   cargo run -p tokio-streams-course --bin ex12_capstone
//!
//! Shape:
//!   many SOURCES  --(StreamMap fan-in, tagged by source)-->
//!   take_until(cancel)  --(graceful shutdown)-->
//!   filter (drop invalid)  --> map to async work --> buffer_unordered (bounded fan-out)
//!   --> sink (collect + report)
//!
//! Everything is one lazy stream expression, driven by a single consumer loop.

use futures::StreamExt; // map/filter_map/take_until/buffer_unordered/next all from here
use std::pin::Pin;
use std::time::{Duration, Instant};
use tokio_stream::wrappers::{IntervalStream, ReceiverStream};
use tokio_stream::{Stream, StreamMap};
use tokio_util::sync::CancellationToken;

type BoxedSource = Pin<Box<dyn Stream<Item = i64> + Send>>;

/// Simulated downstream work (I/O + compute). Returns a report line.
async fn process(source: &'static str, raw: i64) -> String {
    tokio::time::sleep(Duration::from_millis(20)).await;
    format!("[{source:>6}] {raw:>4} -> {}", raw * raw)
}

#[tokio::main]
async fn main() {
    let start = Instant::now();
    let cancel = CancellationToken::new();

    // ── SOURCES: fan them into one keyed stream. ──
    let mut sources: StreamMap<&'static str, BoxedSource> = StreamMap::new();

    // Source 1: a "sensor" ticking every 15ms, emitting rising readings.
    let sensor = IntervalStream::new(tokio::time::interval(Duration::from_millis(15)))
        .enumerate()
        .map(|(i, _)| i as i64 * 3);
    sources.insert("sensor", Box::pin(sensor));

    // Source 2: a "manual" feed over a channel, including one INVALID (negative) reading.
    let (tx, rx) = tokio::sync::mpsc::channel::<i64>(8);
    tokio::spawn(async move {
        for v in [7, -1, 11, 42, -5, 100] {
            tokio::time::sleep(Duration::from_millis(22)).await;
            if tx.send(v).await.is_err() {
                break;
            }
        }
    });
    sources.insert("manual", Box::pin(ReceiverStream::new(rx)));

    // ── SHUTDOWN: cancel the whole pipeline after 130ms. ──
    let shutdown = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(130)).await;
        shutdown.cancel();
    });

    // ── THE PIPELINE (lazy until consumed). ──
    let pipeline = sources
        .take_until(cancel.cancelled()) // graceful stop: end stream on cancel
        .filter_map(|(who, raw)| async move {
            // validate: drop negatives; keep (source, value) for the good ones
            if raw >= 0 {
                Some((who, raw))
            } else {
                println!("  dropped invalid reading {raw} from {who}");
                None
            }
        })
        .map(|(who, raw)| async move { process(who, raw).await }) // Stream<Item = Future>
        .buffer_unordered(3); // bounded fan-out: <=3 process() calls at once

    tokio::pin!(pipeline);

    // ── SINK. ──
    let mut processed = 0usize;
    while let Some(line) = pipeline.next().await {
        processed += 1;
        println!("{line}");
    }

    println!(
        "\npipeline drained: {processed} readings processed in {:?} (stopped by shutdown)",
        start.elapsed()
    );

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ 1. Add a third source "backup" (another channel) at runtime AFTER │
    // │    the first sensor reading arrives (call sources.insert inside a   │
    // │    variant of the loop — you'll need to restructure to select!).    │
    // │ 2. Replace buffer_unordered(3) with buffered(3) and observe how the │
    // │    output ordering and total time change.                           │
    // │ 3. Add a per-item timeout so a slow process() can't stall the sink. │
    // └──────────────────────────────────────────────────────────────────┘
}
