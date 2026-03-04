//! Measure Norito JSON encode/decode strategies on nested payloads.

#[cfg(all(feature = "json", feature = "bench-internal"))]
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

#[cfg(all(feature = "json", feature = "bench-internal"))]
#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct Nested {
    id: u32,
    data: String,
}

#[cfg(all(feature = "json", feature = "bench-internal"))]
#[derive(
    Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
struct Sample {
    name: String,
    flag: bool,
    maybe: Option<u64>,
    nested: Vec<Nested>,
}

#[cfg(all(feature = "json", feature = "bench-internal"))]
fn make_samples(n: usize, nested: usize) -> Vec<Sample> {
    (0..n)
        .map(|i| Sample {
            name: format!("sample-{i}"),
            flag: i % 2 == 0,
            maybe: (i % 3 == 0).then_some(i as u64 * 7),
            nested: (0..nested)
                .map(|j| Nested {
                    id: j as u32,
                    data: format!("chunk-{i}-{j}-{}", i.wrapping_mul(j + 1)),
                })
                .collect(),
        })
        .collect()
}

#[cfg(all(feature = "json", feature = "bench-internal"))]
fn bench_json(c: &mut Criterion) {
    let mut group = c.benchmark_group("norito_json");
    for &(items, nested) in &[(32usize, 4usize), (64, 8), (128, 12)] {
        let payload = make_samples(items, nested);
        let json_generic = norito::json::to_json(&payload).unwrap();
        group.throughput(Throughput::Bytes(json_generic.len() as u64));

        group.bench_function(
            BenchmarkId::new("encode_generic", format!("{items}x{nested}")),
            |b| {
                b.iter_batched(
                    || payload.clone(),
                    |value| norito::json::to_json(&value).unwrap(),
                    BatchSize::SmallInput,
                )
            },
        );

        group.bench_function(
            BenchmarkId::new("decode_generic", format!("{items}x{nested}")),
            |b| b.iter(|| norito::json::from_json::<Vec<Sample>>(&json_generic).unwrap()),
        );

        // Fast-path benchmarks removed in Clippy build to avoid duplicate derives.
    }
    group.finish();
}

#[cfg(all(feature = "json", feature = "bench-internal"))]
criterion_group!(benches, bench_json);

#[cfg(all(feature = "json", feature = "bench-internal"))]
criterion_main!(benches);

#[cfg(not(all(feature = "json", feature = "bench-internal")))]
fn main() {
    eprintln!("Enable `json` and `bench-internal` features to run this benchmark.");
}
