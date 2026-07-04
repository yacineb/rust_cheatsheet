//! drills_solved — reference answers for `drills.rs`. Every test here should pass:
//!
//!   cargo test -p tokio-streams-course --bin drills_solved
//!
//! Use it to check your approach or to confirm your toolchain runs the whole set green.
#![allow(dead_code)] // drill fns are exercised by the #[cfg(test)] tests below

fn main() {
    println!("Reference solutions. Run: cargo test -p tokio-streams-course --bin drills_solved");
}

async fn d1_sum_even(n: i64) -> i64 {
    use tokio_stream::StreamExt;
    tokio_stream::iter(0..n)
        .filter(|x| x % 2 == 0)
        .fold(0, |acc, x| acc + x)
        .await
}

async fn d2_async_double(input: Vec<i32>) -> Vec<i32> {
    use tokio_stream::StreamExt;
    async fn double(x: i32) -> i32 {
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
        x * 2
    }
    tokio_stream::iter(input).then(double).collect().await
}

async fn d3_drain_channel() -> Vec<i32> {
    use tokio_stream::wrappers::ReceiverStream;
    use tokio_stream::StreamExt;
    let (tx, rx) = tokio::sync::mpsc::channel::<i32>(4);
    tokio::spawn(async move {
        for i in 0..5 {
            if tx.send(i).await.is_err() {
                break;
            }
        }
        // tx dropped here -> stream ends
    });
    ReceiverStream::new(rx).collect().await
}

async fn d4_merge_sorted() -> Vec<i32> {
    use tokio_stream::StreamExt;
    let a = tokio_stream::iter(vec![0, 2, 4]);
    let b = tokio_stream::iter(vec![1, 3, 5]);
    let mut v: Vec<i32> = a.merge(b).collect().await;
    v.sort();
    v
}

async fn d5_stream_map_counts() -> (u32, u32) {
    use tokio_stream::{StreamExt, StreamMap};
    let mut map = StreamMap::new();
    map.insert("a", tokio_stream::iter(vec![1, 2, 3]));
    map.insert("b", tokio_stream::iter(vec![10, 20]));
    let (mut a, mut b) = (0u32, 0u32);
    while let Some((key, _v)) = map.next().await {
        match key {
            "a" => a += 1,
            "b" => b += 1,
            _ => {}
        }
    }
    (a, b)
}

async fn d6_concurrent_squares(inputs: Vec<u64>) -> Vec<u64> {
    use futures::StreamExt;
    async fn square(x: u64) -> u64 {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        x * x
    }
    let mut v: Vec<u64> = futures::stream::iter(inputs)
        .map(square)
        .buffer_unordered(4)
        .collect()
        .await;
    v.sort();
    v
}

async fn d7_drain_take_until(values: Vec<i32>, stop_now: bool) -> Vec<i32> {
    use futures::StreamExt;
    let s = tokio_stream::iter(values);
    if stop_now {
        // A future that's already ready -> take_until ends the stream immediately.
        s.take_until(async {}).collect().await
    } else {
        // A future that never completes -> take_until never triggers; full drain.
        s.take_until(std::future::pending::<()>()).collect().await
    }
}

async fn d8_fizzbuzz(n: u32) -> Vec<String> {
    use futures::StreamExt;
    fn fb(i: u32) -> String {
        match (i % 3, i % 5) {
            (0, 0) => "FizzBuzz".into(),
            (0, _) => "Fizz".into(),
            (_, 0) => "Buzz".into(),
            _ => i.to_string(),
        }
    }
    let s = async_stream::stream! {
        for i in 1..=n {
            yield fb(i);
        }
    };
    s.collect().await
}

async fn d9_powers_of_two(count: usize) -> Vec<u64> {
    use futures::StreamExt;
    futures::stream::unfold((1u64, 0usize), move |(val, i)| async move {
        if i == count {
            None
        } else {
            Some((val, (val * 2, i + 1)))
        }
    })
    .collect()
    .await
}

async fn d10_try_sum(items: Vec<Result<i64, String>>) -> Result<i64, String> {
    use futures::TryStreamExt;
    futures::stream::iter(items)
        .try_fold(0, |acc, n| async move { Ok(acc + n) })
        .await
}

async fn d11_parse_all(inputs: Vec<&'static str>) -> Result<Vec<i64>, String> {
    use futures::{StreamExt, TryStreamExt};
    async fn parse(s: &str) -> Result<i64, String> {
        s.parse::<i64>().map_err(|_| format!("bad: {s}"))
    }
    futures::stream::iter(inputs).then(parse).try_collect().await
}

async fn d12_partition(items: Vec<Result<i64, String>>) -> (Vec<i64>, usize) {
    use futures::StreamExt;
    let results: Vec<Result<i64, String>> = futures::stream::iter(items).collect().await;
    let mut oks = Vec::new();
    let mut nerr = 0;
    for r in results {
        match r {
            Ok(v) => oks.push(v),
            Err(_) => nerr += 1,
        }
    }
    (oks, nerr)
}

async fn d13_double_or_fail(inputs: Vec<i64>) -> Result<Vec<i64>, String> {
    use async_stream::try_stream;
    use futures::TryStreamExt;
    let s = try_stream! {
        for x in inputs {
            // `Ok(x * 2)` fixes the Ok type to i64, so `Err(String)` unifies without turbofish.
            let doubled = if x < 0 { Err(format!("negative: {x}")) } else { Ok(x * 2) }?;
            yield doubled;
        }
    };
    tokio::pin!(s);
    s.try_collect().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn t1() {
        assert_eq!(d1_sum_even(10).await, 20);
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
        assert_eq!(d6_concurrent_squares(vec![1, 2, 3, 4, 5]).await, vec![1, 4, 9, 16, 25]);
    }
    #[tokio::test]
    async fn t7() {
        assert_eq!(d7_drain_take_until(vec![1, 2, 3], false).await, vec![1, 2, 3]);
        assert_eq!(d7_drain_take_until(vec![1, 2, 3], true).await, Vec::<i32>::new());
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
        assert_eq!(d11_parse_all(vec!["1", "x", "3"]).await, Err("bad: x".to_string()));
    }
    #[tokio::test]
    async fn t12() {
        let (oks, nerr) =
            d12_partition(vec![Ok(1), Err("a".into()), Ok(3), Err("b".into())]).await;
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
