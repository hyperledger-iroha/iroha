//! Micro-benchmarks for NCB/AoS views iteration and access patterns.
//!
//! Benchmarks:
//! - Iterate ids+names+flags for (u64, str, bool)
//! - Iterate ids+bytes+u32+flags for (u64, bytes, u32, bool)
//!
//! Uses Criterion harness declared in workspace.

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

fn bench_str_bool_view(c: &mut Criterion) {
    // Build dataset: 10_000 rows with short names alternating flags
    let n = 10_000usize;
    let rows: Vec<(u64, String, bool)> = (0..n)
        .map(|i| (i as u64, format!("name{}", i % 10), i % 2 == 0))
        .collect();
    let borrowed: Vec<(u64, &str, bool)> = rows
        .iter()
        .map(|(id, s, b)| (*id, s.as_str(), *b))
        .collect();
    let ncb = norito::columnar::encode_ncb_u64_str_bool(&borrowed);
    c.bench_function("view_ncb_u64_str_bool_iter", |b| {
        b.iter_batched(
            || norito::columnar::view_ncb_u64_str_bool(&ncb).unwrap(),
            |view| {
                let mut sum: u64 = 0;
                for i in 0..view.len() {
                    sum = sum.wrapping_add(view.id(i));
                    let _ = std::hint::black_box(view.name(i).unwrap());
                    let _ = std::hint::black_box(view.flag(i));
                }
                std::hint::black_box(sum)
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_bytes_u32_bool_view(c: &mut Criterion) {
    let n = 10_000usize;
    let bytes_pool: Vec<Vec<u8>> = (0..100)
        .map(|i| vec![i as u8; (i % 16 + 1) as usize])
        .collect();
    let rows: Vec<(u64, &[u8], u32, bool)> = (0..n)
        .map(|i| {
            let id = i as u64;
            let b = &bytes_pool[i % bytes_pool.len()];
            let v = (i % 128) as u32;
            let f = i % 2 == 1;
            (id, b.as_slice(), v, f)
        })
        .collect();
    let ncb = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    c.bench_function("view_ncb_u64_bytes_u32_bool_iter", |b| {
        b.iter_batched(
            || norito::columnar::view_ncb_u64_bytes_u32_bool(&ncb).unwrap(),
            |view| {
                let mut sum: u64 = 0;
                for i in 0..view.len() {
                    sum = sum.wrapping_add(view.id(i));
                    let _ = std::hint::black_box(view.data(i));
                    let _ = std::hint::black_box(view.val(i));
                    let _ = std::hint::black_box(view.flag(i));
                }
                std::hint::black_box(sum)
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_str_bool_view, bench_bytes_u32_bool_view);
criterion_main!(benches);
