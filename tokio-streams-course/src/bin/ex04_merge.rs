//! ex04 — Fan-in with `merge`: two streams into one.
//!
//!   cargo run -p tokio-streams-course --bin ex04_merge
//!
//! `a.merge(b)` polls both and yields items as they become ready from EITHER side.
//! It ends only when BOTH ends are exhausted. Items must share the same Item type.
//! Use it for a fixed, small number of sources; use StreamMap (ex05) when the set of
//! sources is dynamic or you need to know which source produced each item.

use std::time::Duration;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::{Stream, StreamExt};

/// A stream that yields `label` every `period_ms`, `count` times.
/// Note the return type is `impl Stream<...>` — `Stream` is the trait; `StreamExt` is
/// only the adapter methods bolted onto it.
fn ticker(label: &'static str, period_ms: u64, count: usize) -> impl Stream<Item = &'static str> {
    let interval = tokio::time::interval(Duration::from_millis(period_ms));
    IntervalStream::new(interval).map(move |_| label).take(count)
}

#[tokio::main]
async fn main() {
    // Two independent timers at different rates, merged into one stream.
    let fast = ticker("fast", 20, 5);
    let slow = ticker("slow", 50, 3);

    let mut merged = fast.merge(slow); // interleaved by arrival time
    let mut order = Vec::new();
    while let Some(who) = merged.next().await {
        println!("tick from {who}");
        order.push(who);
    }
    // The fast stream tends to dominate early; merge ends only after BOTH finish.
    println!("arrival order = {order:?}");

    // merge is also the idiomatic way to fold a "control" stream into a "data" stream
    // when they share a type. For DIFFERENT types, map both into a common enum first:
    #[derive(Debug)]
    enum Event {
        Data(i32),
        Stop,
    }
    let data = tokio_stream::iter(vec![1, 2, 3]).map(Event::Data);
    let ctrl = tokio_stream::once(()).map(|_| Event::Stop);
    let mut events = data.merge(ctrl);
    while let Some(ev) = events.next().await {
        match ev {
            Event::Data(n) => println!("event = data {n}"),
            Event::Stop => println!("event = Stop (in real code you'd break here)"),
        }
    }

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ Merge THREE tickers ("a" 15ms, "b" 25ms, "c" 40ms), each 4 ticks.│
    // │ merge is binary, so chain it: a.merge(b).merge(c). Count how many │
    // │ of each label you received (should be 4 each = 12 total).         │
    // └──────────────────────────────────────────────────────────────────┘
}
