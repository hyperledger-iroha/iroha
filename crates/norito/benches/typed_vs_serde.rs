//! Compare typed encode/decode across Norito's JSON helpers (generic vs fast paths).

#[cfg(all(feature = "json", feature = "bench-internal"))]
use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

#[cfg(all(feature = "json", feature = "bench-internal"))]
#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct Inner {
    a: u64,
    name: String,
    tag: Option<String>,
}

#[cfg(all(feature = "json", feature = "bench-internal"))]
#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct Outer {
    id: u64,
    title: String,
    flag: bool,
    inner: Vec<Inner>,
}

#[cfg(all(feature = "json", feature = "bench-internal"))]
fn make_outer(n_inner: usize, payload: &str) -> Outer {
    let mut inners = Vec::with_capacity(n_inner);
    for i in 0..n_inner {
        inners.push(Inner {
            a: i as u64,
            name: format!("{payload}-{i}"),
            tag: (i % 3 == 0).then(|| "tag".to_string()),
        });
    }
    Outer {
        id: 42,
        title: payload.to_string(),
        flag: true,
        inner: inners,
    }
}

#[cfg(all(feature = "json", feature = "bench-internal"))]
fn bench_typed(c: &mut Criterion) {
    let mut group = c.benchmark_group("typed_vs_generic");
    for &n in &[4usize, 16, 64] {
        let v = make_outer(n, "payload");

        group.bench_function(BenchmarkId::new("encode_generic", n), |b| {
            b.iter_batched(
                || v.clone(),
                |vv| norito::json::to_json(&vv).unwrap(),
                BatchSize::SmallInput,
            )
        });

        let s_generic = norito::json::to_json(&v).unwrap();

        group.bench_function(BenchmarkId::new("decode_generic", n), |b| {
            b.iter(|| norito::json::from_json::<Outer>(&s_generic).unwrap())
        });

        // Fast-path encode/decode benches removed for Clippy build.
    }
    group.finish();
}

#[cfg(all(feature = "json", feature = "bench-internal"))]
criterion_group!(benches, bench_typed);

#[cfg(all(feature = "json", feature = "bench-internal"))]
criterion_main!(benches);

#[cfg(not(all(feature = "json", feature = "bench-internal")))]
fn main() {
    eprintln!("Enable `json` and `bench-internal` features to run this benchmark.");
}
