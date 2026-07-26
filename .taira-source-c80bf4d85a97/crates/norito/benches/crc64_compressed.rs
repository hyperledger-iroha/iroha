//! Benchmark CRC64 over large payloads with zstd enabled (SIMD-accelerated CRC64).

use std::io::Cursor;

use crc64fast::Digest as Crc64Digest;
use criterion::{BenchmarkId, Criterion};

fn random_bytes(n: usize) -> Vec<u8> {
    // Deterministic pseudo-random bytes
    let mut v = Vec::with_capacity(n);
    let mut x: u64 = 0x0123_4567_89ab_cdef;
    for _ in 0..n {
        x ^= x << 7;
        x ^= x >> 9;
        v.push((x as u8).wrapping_add(0x3D));
    }
    v
}

fn bench_crc64_compressed(c: &mut Criterion) {
    let mut group = c.benchmark_group("crc64_compressed");
    for &mib in &[1usize, 4, 16, 64] {
        let n = mib * 1024 * 1024;
        let buf = random_bytes(n);
        // Ensure zstd path is linked and functional; keep compressed bytes around
        let compressed = zstd::encode_all(Cursor::new(&buf), 3).expect("zstd compress");
        let dec = zstd::decode_all(Cursor::new(&compressed)).expect("zstd decompress");
        assert_eq!(dec.len(), buf.len());

        group.bench_with_input(
            BenchmarkId::new("hardware", format!("{mib}MiB")),
            &dec,
            |b, data| {
                b.iter(|| {
                    let h = norito::hardware_crc64(std::hint::black_box(data));
                    std::hint::black_box(h)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("fallback", format!("{mib}MiB")),
            &dec,
            |b, data| {
                b.iter(|| {
                    let h = norito::crc64_fallback(std::hint::black_box(data));
                    std::hint::black_box(h)
                })
            },
        );

        // Optional: CRC over compressed bytes (for reference)
        group.bench_with_input(
            BenchmarkId::new("hardware_on_compressed", format!("{mib}MiB")),
            &compressed,
            |b, data| {
                b.iter(|| {
                    let h = norito::hardware_crc64(std::hint::black_box(data));
                    std::hint::black_box(h)
                })
            },
        );

        // Streaming over decompressed bytes via zstd::Decoder, updating crc64fast::Digest
        group.bench_with_input(
            BenchmarkId::new("stream_hardware", format!("{mib}MiB")),
            &compressed,
            |b, data| {
                b.iter(|| {
                    let mut dec = zstd::Decoder::new(Cursor::new(&data[..])).expect("decoder");
                    let mut digest = Crc64Digest::new();
                    let mut buf = [0u8; 64 * 1024];
                    loop {
                        let read = std::io::Read::read(&mut dec, &mut buf).expect("read");
                        if read == 0 {
                            break;
                        }
                        digest.write(&buf[..read]);
                    }
                    std::hint::black_box(digest.sum64())
                })
            },
        );
    }
    group.finish();
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_crc64_compressed(&mut c);
    c.final_summary();
}
