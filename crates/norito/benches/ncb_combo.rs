//! Benches for combo NCB shapes: (u64, &str, u32, bool) and (u64, &[u8], u32, bool).
#![allow(clippy::type_complexity)]

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

mod ncb {
    pub use norito::columnar::{
        encode_ncb_u64_bytes_u32_bool, encode_ncb_u64_str_u32_bool,
        encode_rows_u64_bytes_u32_bool_adaptive, encode_rows_u64_str_u32_bool_adaptive,
        view_ncb_u64_bytes_u32_bool, view_ncb_u64_str_u32_bool,
    };
}

// Bring AoS encode trait into scope for `value.encode()` syntax
use norito::codec::Encode as _;

fn gen_str_u32_rows(n: usize) -> (Vec<(u64, String, u32, bool)>, Vec<(u64, String, u32, bool)>) {
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        rows.push((
            (i as u64) * 3,
            format!("user_{:04}", i % 37),
            1_000 + ((i * 7) as u32 % 113),
            i % 3 == 0,
        ));
    }
    (rows.clone(), rows)
}

fn gen_bytes_u32_rows(
    n: usize,
) -> (
    Vec<(u64, Vec<u8>, u32, bool)>,
    Vec<(u64, Vec<u8>, u32, bool)>,
) {
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        let len = 6 + (i % 5);
        rows.push((
            (i as u64) * 5,
            vec![(i % 251) as u8; len],
            2_000 + (i as u32 % 127),
            i % 2 == 1,
        ));
    }
    (rows.clone(), rows)
}

fn bench_combo(c: &mut Criterion) {
    let sizes = [256usize, 1024usize];

    // u64, &str, u32, bool
    let mut g = c.benchmark_group("combo_u64_str_u32_bool");
    for n in sizes {
        g.bench_with_input(BenchmarkId::new("AoS_encode", n), &n, |b, &n| {
            b.iter_batched(
                || {
                    let (owned, _dup) = gen_str_u32_rows(n);
                    owned
                },
                |owned| {
                    let bytes = owned.encode();
                    std::hint::black_box(bytes)
                },
                BatchSize::SmallInput,
            )
        });
        g.bench_with_input(BenchmarkId::new("NCB_encode", n), &n, |b, &n| {
            b.iter_batched(
                || gen_str_u32_rows(n).1,
                |src| {
                    let borrowed: Vec<(u64, &str, u32, bool)> = src
                        .iter()
                        .map(|(id, s, v, f)| (*id, s.as_str(), *v, *f))
                        .collect();
                    let bytes = ncb::encode_ncb_u64_str_u32_bool(&borrowed);
                    std::hint::black_box(bytes)
                },
                BatchSize::SmallInput,
            )
        });
        g.bench_with_input(BenchmarkId::new("NCB_decode_view_iter", n), &n, |b, &n| {
            let (_owned, src) = gen_str_u32_rows(n);
            let borrowed: Vec<(u64, &str, u32, bool)> = src
                .iter()
                .map(|(id, s, v, f)| (*id, s.as_str(), *v, *f))
                .collect();
            let bytes = ncb::encode_ncb_u64_str_u32_bool(&borrowed);
            b.iter(|| {
                let view = ncb::view_ncb_u64_str_u32_bool(std::hint::black_box(&bytes)).unwrap();
                let mut sum = 0u64;
                for i in 0..view.len() {
                    // touch all columns
                    sum = sum
                        .wrapping_add(view.id(i))
                        .wrapping_add(view.val(i) as u64);
                    let _s = view.name(i).unwrap();
                    if view.flag(i) {
                        sum = sum.wrapping_add(1);
                    }
                }
                std::hint::black_box(sum)
            })
        });
        g.bench_with_input(BenchmarkId::new("adaptive_encode", n), &n, |b, &n| {
            b.iter_batched(
                || gen_str_u32_rows(n).1,
                |src| {
                    let rows: Vec<(u64, &str, u32, bool)> = src
                        .iter()
                        .map(|(id, s, v, f)| (*id, s.as_str(), *v, *f))
                        .collect();
                    let bytes = ncb::encode_rows_u64_str_u32_bool_adaptive(&rows);
                    std::hint::black_box(bytes)
                },
                BatchSize::SmallInput,
            )
        });
    }
    g.finish();

    // u64, &[u8], u32, bool
    let mut g2 = c.benchmark_group("combo_u64_bytes_u32_bool");
    for n in sizes {
        g2.bench_with_input(BenchmarkId::new("AoS_encode", n), &n, |b, &n| {
            b.iter_batched(
                || gen_bytes_u32_rows(n).0,
                |owned| {
                    let bytes = owned.encode();
                    std::hint::black_box(bytes)
                },
                BatchSize::SmallInput,
            )
        });
        g2.bench_with_input(BenchmarkId::new("NCB_encode", n), &n, |b, &n| {
            b.iter_batched(
                || gen_bytes_u32_rows(n).1,
                |src| {
                    let rows: Vec<(u64, &[u8], u32, bool)> = src
                        .iter()
                        .map(|(id, b, v, f)| (*id, b.as_slice(), *v, *f))
                        .collect();
                    let bytes = ncb::encode_ncb_u64_bytes_u32_bool(&rows);
                    std::hint::black_box(bytes)
                },
                BatchSize::SmallInput,
            )
        });
        g2.bench_with_input(BenchmarkId::new("NCB_decode_view_iter", n), &n, |b, &n| {
            let (_owned, src) = gen_bytes_u32_rows(n);
            let borrowed: Vec<(u64, &[u8], u32, bool)> = src
                .iter()
                .map(|(id, b, v, f)| (*id, b.as_slice(), *v, *f))
                .collect();
            let bytes = ncb::encode_ncb_u64_bytes_u32_bool(&borrowed);
            b.iter(|| {
                let view = ncb::view_ncb_u64_bytes_u32_bool(std::hint::black_box(&bytes)).unwrap();
                let mut sum = 0u64;
                for i in 0..view.len() {
                    sum = sum
                        .wrapping_add(view.id(i))
                        .wrapping_add(view.val(i) as u64);
                    let _b = view.data(i);
                    if view.flag(i) {
                        sum = sum.wrapping_add(1);
                    }
                }
                std::hint::black_box(sum)
            })
        });
        g2.bench_with_input(BenchmarkId::new("adaptive_encode", n), &n, |b, &n| {
            b.iter_batched(
                || gen_bytes_u32_rows(n).1,
                |src| {
                    let rows: Vec<(u64, &[u8], u32, bool)> = src
                        .iter()
                        .map(|(id, b, v, f)| (*id, b.as_slice(), *v, *f))
                        .collect();
                    let bytes = ncb::encode_rows_u64_bytes_u32_bool_adaptive(&rows);
                    std::hint::black_box(bytes)
                },
                BatchSize::SmallInput,
            )
        });
    }
    g2.finish();
}

criterion_group!(benches, bench_combo);
criterion_main!(benches);
