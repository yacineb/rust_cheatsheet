//! ex11 — Writing your own combinator that WRAPS another stream.
//!
//!   cargo run -p tokio-streams-course --bin ex11_impl_combinator
//!
//! A combinator (like `.map`, `.filter`) holds an inner stream and transforms it in
//! `poll_next`. The one wrinkle is Pin: to poll the inner stream you need a
//! `Pin<&mut Inner>`. Two roads:
//!   * If `Inner: Unpin` (very common), just `Pin::new(&mut self.inner)` — no unsafe.
//!   * For the general `!Unpin` case, use the `pin-project` crate to project the pin
//!     safely (out of scope here; we take the Unpin road and note where it matters).
//!
//! We build `dedup`: drops CONSECUTIVE duplicate items. It also shows the important
//! "loop until Ready" shape: one call to our poll_next may need several inner polls.

use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_stream::{Stream, StreamExt};

/// The combinator struct: owns the inner stream + a little state (the last item seen).
pub struct Dedup<S: Stream> {
    inner: S,
    last: Option<S::Item>,
}

impl<S> Stream for Dedup<S>
where
    S: Stream + Unpin,           // lets us Pin::new(&mut inner) without unsafe/pin-project
    S::Item: Clone + PartialEq + Unpin, // + Unpin so `Dedup<S>: Unpin` => we can touch fields
{
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        // We may have to skip several duplicates, so loop until we produce or stall.
        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(item)) => {
                    if self.last.as_ref() == Some(&item) {
                        continue; // consecutive duplicate -> pull the next one
                    }
                    self.last = Some(item.clone());
                    return Poll::Ready(Some(item));
                }
                Poll::Ready(None) => return Poll::Ready(None), // inner ended -> we end
                Poll::Pending => return Poll::Pending,         // inner not ready -> propagate
            }
        }
    }
}

/// Extension trait so it reads like a built-in: `stream.dedup()`.
pub trait DedupExt: Stream + Sized {
    fn dedup(self) -> Dedup<Self> {
        Dedup { inner: self, last: None }
    }
}
impl<S: Stream> DedupExt for S {}

#[tokio::main]
async fn main() {
    let input = tokio_stream::iter(vec![1, 1, 2, 2, 2, 3, 1, 1, 4]);
    let deduped: Vec<i32> = input.dedup().collect().await;
    println!("dedup -> {deduped:?}"); // [1, 2, 3, 1, 4]  (only *consecutive* dups removed)

    // Compose it with the built-in combinators — it's a first-class stream.
    let out: Vec<i32> = tokio_stream::iter(vec![5, 5, 6, 6, 7])
        .dedup()
        .map(|x| x * 10)
        .collect()
        .await;
    println!("dedup + map -> {out:?}"); // [50, 60, 70]

    // ┌─────────────────────────── YOUR TURN ───────────────────────────┐
    // │ Write a `StepBy` combinator + `.step_by_stream(n)` ext method that │
    // │ yields every n-th item (indices 0, n, 2n, ...). Keep an index in   │
    // │ the struct; in poll_next, loop pulling inner items and only return │
    // │ the ones whose index % n == 0. Test it on iter(0..20).step_by(3).  │
    // └──────────────────────────────────────────────────────────────────┘
}
