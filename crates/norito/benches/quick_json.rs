//! Quick JSON benches exercising Norito's typed JSON pipelines.
#![cfg(feature = "json")]

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use norito::json::FastJsonWrite;

#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct Inner {
    a: u64,
    name: String,
}

#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct Outer {
    id: u64,
    title: String,
    flag: bool,
    inner: Vec<Inner>,
}

fn make_outer(n_inner: usize, payload: &str) -> Outer {
    let mut inners = Vec::with_capacity(n_inner);
    for i in 0..n_inner {
        inners.push(Inner {
            a: i as u64,
            name: format!("{payload}-{i}"),
        });
    }
    Outer {
        id: 42,
        title: payload.to_string(),
        flag: true,
        inner: inners,
    }
}

fn bench_quick(c: &mut Criterion) {
    let mut g = c.benchmark_group("quick_typed_encode_decode");
    g.sample_size(20);

    for &n in &[8usize, 32] {
        let v = make_outer(n, "payload");
        let s_generic = norito::json::to_json(&v).unwrap();
        let s_fast = norito::json::to_json_fast(&v).unwrap();

        g.bench_function(BenchmarkId::new("encode_generic", n), |b| {
            b.iter_batched(
                || v.clone(),
                |vv| norito::json::to_json(&vv).unwrap(),
                BatchSize::SmallInput,
            )
        });
        g.bench_function(BenchmarkId::new("encode_fast", n), |b| {
            b.iter_batched(
                || v.clone(),
                |vv| norito::json::to_json_fast(&vv).unwrap(),
                BatchSize::SmallInput,
            )
        });
        g.bench_function(BenchmarkId::new("encode_writer", n), |b| {
            b.iter_batched(
                || v.clone(),
                |vv| {
                    let mut s = String::new();
                    vv.write_json(&mut s);
                    s
                },
                BatchSize::SmallInput,
            )
        });
        g.bench_function(BenchmarkId::new("decode_generic", n), |b| {
            b.iter(|| norito::json::from_json::<Outer>(&s_generic).unwrap())
        });
        g.bench_function(BenchmarkId::new("decode_fast", n), |b| {
            b.iter(|| norito::json::from_json_fast_smart::<Outer>(&s_fast).unwrap())
        });
    }
    g.finish();

    // small-object break-even
    let mut small = c.benchmark_group("quick_small_break_even");
    small.sample_size(20);
    for &pad_len in &[32usize, 96, 224, 320] {
        let pad = "x".repeat(pad_len);
        let v = Outer {
            id: 1,
            title: pad,
            flag: true,
            inner: vec![],
        };
        let s_generic = norito::json::to_json(&v).unwrap();
        let s_fast = norito::json::to_json_fast(&v).unwrap();
        small.bench_function(BenchmarkId::new("small_generic", pad_len), |b| {
            b.iter(|| norito::json::from_json::<Outer>(&s_generic).unwrap())
        });
        small.bench_function(BenchmarkId::new("small_fast", pad_len), |b| {
            b.iter(|| norito::json::from_json_fast_smart::<Outer>(&s_fast).unwrap())
        });
    }
    small.finish();
}

criterion_group!(benches, bench_quick);
criterion_main!(benches);
