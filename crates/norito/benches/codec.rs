//! Benchmarks for Norito serialization, compression, and comparisons.

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
use criterion::Criterion;
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
use norito::{self, CompressionConfig, NoritoDeserialize, NoritoSerialize};
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
use parity_scale_codec::{Decode, Encode};
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
use zstd::stream::encode_all;

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(Clone, NoritoSerialize, NoritoDeserialize, Encode, Decode)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct Sample {
    id: u64,
    name: String,
    values: Vec<u32>,
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
fn sample_data() -> Sample {
    Sample {
        id: 42,
        name: "benchmark".repeat(10),
        values: (0..100).collect(),
    }
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
fn bench_codec(c: &mut Criterion) {
    let sample = sample_data();
    // Pre-encode once for decode benches
    let norito_bytes = norito::to_bytes(&sample).unwrap();
    let scale_bytes = parity_scale_codec::Encode::encode(&sample);
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

    c.bench_function("scale_encode", |b| {
        b.iter(|| {
            let bytes = parity_scale_codec::Encode::encode(std::hint::black_box(&sample));
            std::hint::black_box(bytes);
        })
    });

    c.bench_function("scale_encode_compressed", |b| {
        b.iter(|| {
            let bytes = parity_scale_codec::Encode::encode(std::hint::black_box(&sample));
            let compressed = encode_all(bytes.as_slice(), 0).unwrap();
            std::hint::black_box(compressed);
        })
    });

    // Decode benches (uncompressed)
    c.bench_function("norito_decode", |b| {
        b.iter(|| {
            let val: Sample =
                norito::decode_from_bytes(std::hint::black_box(&norito_bytes)).unwrap();
            std::hint::black_box(val)
        })
    });

    c.bench_function("scale_decode", |b| {
        b.iter(|| {
            let val = <Sample as parity_scale_codec::Decode>::decode(
                &mut &std::hint::black_box(&scale_bytes)[..],
            )
            .unwrap();
            std::hint::black_box(val)
        })
    });

    // Decode benches (compressed)
    c.bench_function("norito_decode_compressed", |b| {
        b.iter(|| {
            let val: Sample =
                norito::decode_from_bytes(std::hint::black_box(&norito_zstd)).unwrap();
            std::hint::black_box(val)
        })
    });
}

/// Entry point for the benchmark binary.
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_codec(&mut c);
    c.final_summary();
}

#[cfg(not(all(feature = "parity-scale", feature = "bench-internal")))]
fn main() {
    eprintln!("Enable `parity-scale` and `bench-internal` features to run this benchmark.");
}
