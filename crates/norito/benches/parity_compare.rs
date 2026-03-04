//! Benchmarks comparing Norito with `parity-scale-codec` serialization.

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
use criterion::Criterion;
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
use norito::codec as ncodec;
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
use norito::{
    Compression, CompressionConfig, NoritoDeserialize, deserialize_from, from_compressed_bytes,
    serialize_into, to_compressed_bytes,
};
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
use parity_scale_codec::{Decode as _, Encode as _};

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
#[derive(
    Clone,
    Debug,
    PartialEq,
    norito::Encode,
    norito::Decode,
    parity_scale_codec::Encode,
    parity_scale_codec::Decode,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct BenchmarkData {
    numbers: Vec<u64>,
    text: String,
}

#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
fn parity_scale_comparison(c: &mut Criterion) {
    let sample = BenchmarkData {
        numbers: (0..16).collect(),
        text: "norito".repeat(8),
    };

    let scale_bytes = sample.encode();
    let scale_decoded = BenchmarkData::decode(&mut &scale_bytes[..]).expect("scale decode");
    assert_eq!(sample, scale_decoded);

    let mut norito_bytes = Vec::new();
    serialize_into(&mut norito_bytes, &sample, Compression::None).expect("norito encode");
    let norito_decoded: BenchmarkData =
        deserialize_from(&mut &norito_bytes[..]).expect("norito decode");
    assert_eq!(sample, norito_decoded);

    let compressed_bytes =
        to_compressed_bytes(&sample, Some(CompressionConfig::default())).expect("zstd encode");
    let archived = from_compressed_bytes::<BenchmarkData>(&compressed_bytes).expect("zstd decode");
    let compressed_decoded = BenchmarkData::deserialize(&archived);
    assert_eq!(sample, compressed_decoded);
    assert!(compressed_bytes.len() <= norito_bytes.len());

    // Norito bare (codec) payload for apples-to-apples with SCALE
    let norito_bare = ncodec::Encode::encode(&sample);
    let norito_bare_decoded: BenchmarkData =
        ncodec::Decode::decode(&mut &norito_bare[..]).expect("norito bare decode");
    assert_eq!(sample, norito_bare_decoded);
    println!(
        "sizes: scale={} B, norito_bare={} B, norito_headered={} B, norito+zstd={} B",
        scale_bytes.len(),
        norito_bare.len(),
        norito_bytes.len(),
        compressed_bytes.len()
    );

    // Optional: emit a machine-readable summary for CI when NORITO_BENCH_SUMMARY=1
    if std::env::var("NORITO_BENCH_SUMMARY").ok().as_deref() == Some("1") {
        let summary = format!(
            "{{\n  \"dataset\": \"BenchmarkData\",\n  \"sizes\": {{\n    \"scale\": {},\n    \"norito_bare\": {},\n    \"norito_headered\": {},\n    \"norito_zstd\": {}\n  }}\n}}\n",
            scale_bytes.len(),
            norito_bare.len(),
            norito_bytes.len(),
            compressed_bytes.len()
        );
        // Write under benches/artifacts; create directory if missing
        let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("benches")
            .join("artifacts");
        let _ = std::fs::create_dir_all(&out_dir);
        let out_path = out_dir.join("size_summary.json");
        std::fs::write(&out_path, summary).expect("write bench summary");
        println!("NORITO_BENCH_SUMMARY: wrote {:?}", out_path);
    }

    c.bench_function("scale_encode", |b| {
        b.iter(|| {
            let bytes = std::hint::black_box(&sample).encode();
            std::hint::black_box(bytes);
        })
    });

    c.bench_function("scale_decode", |b| {
        b.iter(|| {
            let value = BenchmarkData::decode(&mut &std::hint::black_box(&scale_bytes)[..])
                .expect("scale decode");
            std::hint::black_box(value);
        })
    });

    c.bench_function("norito_encode", |b| {
        b.iter(|| {
            let mut buf = Vec::new();
            serialize_into(&mut buf, std::hint::black_box(&sample), Compression::None)
                .expect("norito encode");
            std::hint::black_box(buf);
        })
    });

    c.bench_function("norito_decode", |b| {
        b.iter(|| {
            let value: BenchmarkData =
                deserialize_from(&mut &std::hint::black_box(&norito_bytes)[..])
                    .expect("norito decode");
            std::hint::black_box(value);
        })
    });

    c.bench_function("norito_codec_bare_encode", |b| {
        b.iter(|| {
            let bytes = ncodec::Encode::encode(std::hint::black_box(&sample));
            std::hint::black_box(bytes);
        })
    });

    c.bench_function("norito_codec_bare_decode", |b| {
        b.iter(|| {
            let value: BenchmarkData =
                ncodec::Decode::decode(&mut &std::hint::black_box(&norito_bare)[..])
                    .expect("norito bare decode");
            std::hint::black_box(value);
        })
    });

    c.bench_function("norito_zstd_encode", |b| {
        b.iter(|| {
            let bytes = to_compressed_bytes(
                std::hint::black_box(&sample),
                Some(CompressionConfig::default()),
            )
            .expect("zstd encode");
            std::hint::black_box(bytes);
        })
    });

    c.bench_function("norito_zstd_decode", |b| {
        b.iter(|| {
            let archived =
                from_compressed_bytes::<BenchmarkData>(std::hint::black_box(&compressed_bytes))
                    .expect("zstd decode");
            let value = BenchmarkData::deserialize(&archived);
            std::hint::black_box(&value);
            std::mem::forget(archived);
        })
    });
}

/// Entry point for the benchmark binary.
#[cfg(all(feature = "parity-scale", feature = "bench-internal"))]
fn main() {
    let mut c = Criterion::default().configure_from_args();
    parity_scale_comparison(&mut c);
    c.final_summary();
}

#[cfg(not(all(feature = "parity-scale", feature = "bench-internal")))]
fn main() {
    eprintln!("Enable `parity-scale` and `bench-internal` features to run this benchmark.");
}
