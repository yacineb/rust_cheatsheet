//! drills — YOUR practice arena. Replace every `todo!()` with real code, then run:
//!
//!   cargo test -p tokio-streams-course --bin drills
//!
//! Each drill has a `#[tokio::test]` that checks your answer. Green = correct.
//! Tests are deterministic (values are sorted / counted, not timing-dependent).
//! Stuck? Peek at the matching function in `drills_solved.rs`.
//!
//! Tip on imports: there are two `StreamExt` traits. Rather than import one globally,
//! each drill `use`s exactly the trait it needs — note which methods come from where.
#![allow(unused)] // stub file: todo!() bodies leave imports/params "unused" until you fill them

use tokio::sync::watch::Receiver;

// `main` is unused; drills run via `cargo test`. This silences the "no main output" noise.
fn main() {
    println!("Run the drills with: cargo test -p tokio-streams-course --bin drills");
}

// ── D1 (basics) ────────────────────────────────────────────────────────────
// Stream the range 0..n, keep even numbers, and return their sum.
async fn d1_sum_even(n: i64) -> i64 {
    use tokio_stream::StreamExt;
    // Hint: tokio_stream::iter(0..n).filter(..).fold(0, ..).await
    let stream = tokio_stream::iter(1..n).filter(|n| n % 2 == 0);
    stream.fold(0, |acc, item| acc + item).await
}

// ── D2 (async transform) ───────────────────────────────────────────────────
// For each input, await an async doubling, and collect results IN ORDER.
async fn d2_async_double(input: Vec<i32>) -> Vec<i32> {
    use tokio_stream::StreamExt;
    async fn double(x: i32) -> i32 {
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
        x * 2
    }
    // Hint: tokio_stream::iter(input).then(double).collect().await

    tokio_stream::iter(input).then(double).collect().await
}

// ── D3 (channels) ──────────────────────────────────────────────────────────
// Spawn a producer that sends 0..5 over a bounded mpsc channel, then consume the
// receiver AS A STREAM and return everything you received.
async fn d3_drain_channel() -> Vec<i32> {
    use tokio_stream::wrappers::ReceiverStream;
    use tokio_stream::StreamExt;

    let (tx, rx) = tokio::sync::mpsc::channel(10);

    tokio::spawn(async move {
        for i in 0..5 {
            if tx.send(i).await.is_err() {
                return;
            }
        }
    });

    tokio_stream::wrappers::ReceiverStream::new(rx)
        .collect()
        .await
}

// ── D4 (merge / fan-in) ────────────────────────────────────────────────────
// Merge two streams (evens 0,2,4 and odds 1,3,5) into one, collect, and return
// the results SORTED (so the test is order-independent).
async fn d4_merge_sorted() -> Vec<i32> {
    use tokio_stream::StreamExt;

    let evens = tokio_stream::iter(0..6).filter(|x| x % 2 == 0);
    let odds = tokio_stream::iter(0..6).filter(|x| x % 2 == 1);

    let mut merged: Vec<_> = evens.merge(odds).collect().await;
    merged.sort();
    merged
}

// ── D5 (StreamMap / keyed fan-in) ──────────────────────────────────────────
// Put two sources in a StreamMap under keys "a" (yields 3 items) and "b" (yields 2),
// consume it, and return (count_from_a, count_from_b).
async fn d5_stream_map_counts() -> (u32, u32) {
    use tokio_stream::{StreamExt, StreamMap};

    let mut map = StreamMap::new();
    map.insert("a", tokio_stream::iter(1..=3));
    map.insert("b", tokio_stream::iter(1..=2));

    let (mut count_a, mut count_b) = (0, 0);
    while let Some((key, value)) = map.next().await {
        if key == "a" {
            count_a += 1;
        }
        if key == "b" {
            count_b += 1;
        }
    }

    (count_a, count_b)
}

// ── D6 (fan-out / bounded concurrency) ─────────────────────────────────────
// Square each input via an async fn, running up to 4 concurrently, and return the
// results SORTED. NOTE: buffer_unordered comes from `futures::StreamExt`.
async fn d6_concurrent_squares(inputs: Vec<u64>) -> Vec<u64> {
    use futures::StreamExt; // buffer_unordered / map / collect (all from futures here)
    async fn square(x: u64) -> u64 {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        x * x
    }
    let mut stream: Vec<_> = futures::stream::iter(inputs)
        .map(square)
        .buffer_unordered(4)
        .collect()
        .await;
    stream.sort();
    stream
}

// ── D7 (cancellation semantics) ────────────────────────────────────────────
// Return all items from `values`, UNLESS `stop_now` is true, in which case the stop
// future is already resolved so the stream should yield nothing.
// (Teaches `take_until`: once the future resolves, the stream ends.)
async fn d7_drain_take_until(values: Vec<i32>, stop_now: bool) -> Vec<i32> {
    use futures::StreamExt; // take_until lives on futures' StreamExt
                            // Hint: if stop_now, use `async {}` (ready now) as the stop future;
                            //       else use `std::future::pending::<()>()` (never). Then .collect().await.
    let mut stream = tokio_stream::iter(values);

    if stop_now {
        stream.take_until(async {}).collect().await
    } else {
        stream
            .take_until(std::future::pending::<()>())
            .collect()
            .await
    }
}

// ── D8 (implement a stream via async-stream) ───────────────────────────────
// Using `async_stream::stream!`, produce the FizzBuzz strings for 1..=n and collect.
// (1->"1", 3->"Fizz", 5->"Buzz", 15->"FizzBuzz", ...)
async fn d8_fizzbuzz(n: u32) -> Vec<String> {
    use futures::StreamExt;
    // Hint: let s = async_stream::stream! { for i in 1..=n { yield fizzbuzz(i); } };
    //       tokio::pin!(s); collect by looping or s.collect().await
    todo!("build a stream! of fizzbuzz strings and collect")
}

// ── D9 (implement a stream via unfold) ─────────────────────────────────────
// Using `futures::stream::unfold`, yield the first `count` powers of two starting at
// 1 (1, 2, 4, 8, ...) and collect them.
async fn d9_powers_of_two(count: usize) -> Vec<u64> {
    use futures::StreamExt;
    // Hint: unfold((1u64, 0usize), |(val, i)| async move {
    //           if i == count { None } else { Some((val, (val*2, i+1))) } })
    todo!("generate powers of two with unfold")
}

// ── D10 (TryStreamExt: fallible reduce) ────────────────────────────────────
// Sum a stream of Result<i64, String>, STOPPING at the first Err and returning it.
async fn d10_try_sum(items: Vec<Result<i64, String>>) -> Result<i64, String> {
    use futures::TryStreamExt;
    // Hint: futures::stream::iter(items)
    //           .try_fold(0, |acc, n| async move { Ok(acc + n) }).await
    todo!("try_fold that short-circuits on the first Err")
}

// ── D11 (TryStreamExt: try_collect) ────────────────────────────────────────
// Parse every &str to i64. Return Ok(all values) or the FIRST error ("bad: <s>").
async fn d11_parse_all(inputs: Vec<&'static str>) -> Result<Vec<i64>, String> {
    use futures::{StreamExt, TryStreamExt};
    async fn parse(s: &str) -> Result<i64, String> {
        s.parse::<i64>().map_err(|_| format!("bad: {s}"))
    }
    // Hint: futures::stream::iter(inputs).then(parse).try_collect().await
    todo!("then(parse) + try_collect")
}

// ── D12 (collect-all: the OPPOSITE of short-circuit) ───────────────────────
// Keep EVERY outcome: return (all Ok values, count of Errs). Do NOT use try_*.
async fn d12_partition(items: Vec<Result<i64, String>>) -> (Vec<i64>, usize) {
    use futures::StreamExt;
    // Hint: collect into a Vec<Result<..>> first, then split oks from errs yourself.
    todo!("collect all results, then partition into (oks, error_count)")
}

// ── D13 (implement a fallible stream via try_stream!) ──────────────────────
// Build a `try_stream!` that yields each input DOUBLED, but errors "negative: <x>" on the
// first negative input (and ends). Collect it with try_collect.
async fn d13_double_or_fail(inputs: Vec<i64>) -> Result<Vec<i64>, String> {
    use async_stream::try_stream;
    use futures::TryStreamExt;
    // Hint: let s = try_stream! {
    //           for x in inputs {
    //               let doubled = if x < 0 { Err(format!("negative: {x}")) } else { Ok(x * 2) }?;
    //               yield doubled;
    //           }
    //       };
    //       tokio::pin!(s); s.try_collect().await
    todo!("try_stream! that doubles values but bails with ? on a negative")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn t1() {
        assert_eq!(d1_sum_even(10).await, 20); // 0+2+4+6+8
    }
    #[tokio::test]
    async fn t2() {
        assert_eq!(d2_async_double(vec![1, 2, 3]).await, vec![2, 4, 6]);
    }
    #[tokio::test]
    async fn t3() {
        assert_eq!(d3_drain_channel().await, vec![0, 1, 2, 3, 4]);
    }
    #[tokio::test]
    async fn t4() {
        assert_eq!(d4_merge_sorted().await, vec![0, 1, 2, 3, 4, 5]);
    }
    #[tokio::test]
    async fn t5() {
        assert_eq!(d5_stream_map_counts().await, (3, 2));
    }
    #[tokio::test]
    async fn t6() {
        assert_eq!(
            d6_concurrent_squares(vec![1, 2, 3, 4, 5]).await,
            vec![1, 4, 9, 16, 25]
        );
    }
    #[tokio::test]
    async fn t7() {
        assert_eq!(
            d7_drain_take_until(vec![1, 2, 3], false).await,
            vec![1, 2, 3]
        );
        assert_eq!(
            d7_drain_take_until(vec![1, 2, 3], true).await,
            Vec::<i32>::new()
        );
    }
    #[tokio::test]
    async fn t8() {
        assert_eq!(d8_fizzbuzz(5).await, vec!["1", "2", "Fizz", "4", "Buzz"]);
        assert_eq!(d8_fizzbuzz(15).await[14], "FizzBuzz");
    }
    #[tokio::test]
    async fn t9() {
        assert_eq!(d9_powers_of_two(5).await, vec![1, 2, 4, 8, 16]);
    }
    #[tokio::test]
    async fn t10() {
        assert_eq!(d10_try_sum(vec![Ok(1), Ok(2), Ok(3)]).await, Ok(6));
        assert_eq!(
            d10_try_sum(vec![Ok(1), Err("boom".into()), Ok(99)]).await,
            Err("boom".to_string())
        );
    }
    #[tokio::test]
    async fn t11() {
        assert_eq!(d11_parse_all(vec!["1", "2", "3"]).await, Ok(vec![1, 2, 3]));
        assert_eq!(
            d11_parse_all(vec!["1", "x", "3"]).await,
            Err("bad: x".to_string())
        );
    }
    #[tokio::test]
    async fn t12() {
        let (oks, nerr) = d12_partition(vec![Ok(1), Err("a".into()), Ok(3), Err("b".into())]).await;
        assert_eq!(oks, vec![1, 3]);
        assert_eq!(nerr, 2);
    }
    #[tokio::test]
    async fn t13() {
        assert_eq!(d13_double_or_fail(vec![1, 2, 3]).await, Ok(vec![2, 4, 6]));
        assert_eq!(
            d13_double_or_fail(vec![1, -2, 3]).await,
            Err("negative: -2".to_string())
        );
    }
}
