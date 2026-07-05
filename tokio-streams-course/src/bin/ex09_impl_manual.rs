//! ex09 — Implementing `Stream` by hand: the poll_next state machine.
//!
//!   cargo run -p tokio-streams-course --bin ex09_impl_manual
//!
//! This is the "under the hood" exercise. You write `poll_next` and return one of:
//!   Poll::Ready(Some(item))  — here's a value
//!   Poll::Ready(None)        — finished, don't poll me again
//!   Poll::Pending            — not ready; I've arranged (via a child future / the waker)
//!                              to be polled again when progress is possible
//!
//! RULE: if you return Pending you MUST ensure the waker gets called later, or your
//! stream hangs forever. The easy way: delegate to a child future's poll (as in Ticker),
//! which registers the waker for you.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{Instant, Sleep};
use tokio_stream::{Stream, StreamExt};

// ── (A) A synchronous generator: always Ready, never Pending. ──
struct Counter {
    current: u64,
    max: u64,
}

impl Stream for Counter {
    type Item = u64;
    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<u64>> {
        // Self is Unpin (plain fields), so `self.field` works through the Pin.
        if self.current >= self.max {
            return Poll::Ready(None); // exhausted
        }
        let v = self.current;
        self.current += 1;
        Poll::Ready(Some(v)) // no awaiting needed -> immediately Ready
    }
}

// ── (B) A time-driven stream: returns Pending and lets a child Sleep wake us. ──
struct Ticker {
    period: Duration,
    sleep: Pin<Box<Sleep>>, // boxed => Ticker itself is Unpin; we poll it via as_mut()
    remaining: u32,
}

impl Ticker {
    fn new(period: Duration, ticks: u32) -> Self {
        Ticker {
            period,
            sleep: Box::pin(tokio::time::sleep(period)),
            remaining: ticks,
        }
    }
}

impl Stream for Ticker {
    type Item = u32; // yields the tick index

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<u32>> {
        if self.remaining == 0 {
            // end of stream
            return Poll::Ready(None);
        }
        // Poll the child future. If it's Pending, IT registered our waker — so we just
        // propagate Pending and get re-polled when the timer fires.
        match self.sleep.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => {
                self.remaining -= 1;
                let idx = self.remaining; // just a number to emit
                                          // Re-arm the timer for the next tick.
                let next = Instant::now() + self.period;
                self.sleep.as_mut().reset(next);
                Poll::Ready(Some(idx))
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let nums: Vec<u64> = Counter { current: 0, max: 5 }.collect().await;
    println!("Counter -> {nums:?}"); // [0,1,2,3,4]

    // Because Counter is Unpin we can call .next() directly; still, collect works too.
    let mut c = Counter {
        current: 10,
        max: 13,
    };
    while let Some(n) = c.next().await {
        println!("counter tick {n}");
    }

    // Ticker actually spaces ticks out in real time.
    let start = Instant::now();
    let mut t = Ticker::new(Duration::from_millis(25), 4);
    while let Some(i) = t.next().await {
        println!("ticker fired, remaining-index={i} at {:?}", start.elapsed());
    }

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ Implement a `Fibonacci` stream (manual poll_next) that yields     │
    // │ 0,1,1,2,3,5,8,... and stops once the value exceeds a `max` field. │
    // │ It's synchronous like Counter — always Ready, never Pending.      │
    // └──────────────────────────────────────────────────────────────────┘
}
