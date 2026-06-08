//! Benchmarks for Norito serialization and compression.

#[cfg(feature = "bench-internal")]
use criterion::Criterion;
#[cfg(feature = "bench-internal")]
use norito::{self, CompressionConfig, NoritoDeserialize, NoritoSerialize};

#[cfg(feature = "bench-internal")]
#[derive(Clone, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct Sample {
    id: u64,
    name: String,
    values: Vec<u32>,
}

#[cfg(feature = "bench-internal")]
fn sample_data() -> Sample {
    Sample {
        id: 42,
        name: "benchmark".repeat(10),
        values: (0..100).collect(),
    }
}

#[cfg(feature = "bench-internal")]
fn bench_codec(c: &mut Criterion) {
    let sample = sample_data();
    let norito_bytes = norito::to_bytes(&sample).unwrap();
    let norito_zstd =
        norito::to_compressed_bytes(&sample, Some(CompressionConfig::default())).unwrap();

    c.bench_function("norito_encode", |b| {
        b.iter(|| {
            let bytes = norito::to_bytes(std::hint::black_box(&sample)).unwrap();
            std::hint::black_box(bytes);
        })
    });

    c.bench_function("norito_encode_compressed", |b| {
        b.iter(|| {
            let bytes = norito::to_compressed_bytes(
                std::hint::black_box(&sample),
                Some(CompressionConfig::default()),
            )
            .unwrap();
            std::hint::black_box(bytes);
        })
    });

    c.bench_function("norito_decode", |b| {
        b.iter(|| {
            let val: Sample =
                norito::decode_from_bytes(std::hint::black_box(&norito_bytes)).unwrap();
            std::hint::black_box(val)
        })
    });

    c.bench_function("norito_decode_compressed", |b| {
        b.iter(|| {
            let val: Sample =
                norito::decode_from_bytes(std::hint::black_box(&norito_zstd)).unwrap();
            std::hint::black_box(val)
        })
    });
}

/// Entry point for the benchmark binary.
#[cfg(feature = "bench-internal")]
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_codec(&mut c);
    c.final_summary();
}

#[cfg(not(feature = "bench-internal"))]
fn main() {
    eprintln!("Enable the `bench-internal` feature to run this benchmark.");
}
