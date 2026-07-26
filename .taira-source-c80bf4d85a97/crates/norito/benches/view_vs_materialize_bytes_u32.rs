//! Bench view-iterate vs materialize decode for `(u64, &[u8], u32, bool)`.
//!
//! Goal: exercise the bytes combo layout and typical u32-delta cases.
//! Run with: cargo bench -p norito --bench view_vs_materialize_bytes_u32

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn make_rows(n: usize) -> Vec<(u64, Vec<u8>, u32, bool)> {
    let mut rows = Vec::with_capacity(n);
    let mut val: u32 = 1000; // small deltas likely trigger u32-delta encoding
    for i in 0..n as u64 {
        // bytes: vary size/pattern a bit
        let bytes = if i % 16 == 0 {
            vec![0xFF]
        } else if i % 3 == 0 {
            vec![0x41, 0x42, 0x43]
        } else if i % 5 == 0 {
            vec![]
        } else {
            vec![(i as u8) & 0x7F]
        };
        val = val.wrapping_add((i as u32) % 3); // small delta steps 0..2
        rows.push((i, bytes, val, (i & 1) == 1));
    }
    rows
}

fn bench_view_vs_materialize_bytes_u32(c: &mut Criterion) {
    let mut group = c.benchmark_group("view_vs_materialize_u64_bytes_u32_bool");
    for &n in &[1_000usize, 10_000] {
        // Prepare dataset and encode as NCB
        let owned = make_rows(n);
        let borrowed: Vec<(u64, &[u8], u32, bool)> = owned
            .iter()
            .map(|(id, bs, v, b)| (*id, bs.as_slice(), *v, *b))
            .collect();
        let body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&borrowed);

        // Sanity: compute both accumulators once and ensure equality
        let acc_view = {
            let view = norito::columnar::view_ncb_u64_bytes_u32_bool(&body).expect("view");
            let mut acc: u64 = 0;
            for i in 0..view.len() {
                let id = view.id(i);
                let bytes_len = view.data(i).len() as u64;
                let v = view.val(i) as u64;
                let flag = view.flag(i) as u64;
                acc = acc.wrapping_add(id ^ bytes_len ^ v ^ flag);
            }
            acc
        };
        let acc_mat = {
            let view = norito::columnar::view_ncb_u64_bytes_u32_bool(&body).expect("view");
            let mut acc: u64 = 0;
            for i in 0..view.len() {
                let (id, data, v, flag) =
                    (view.id(i), view.data(i).to_vec(), view.val(i), view.flag(i));
                acc = acc.wrapping_add(id ^ (data.len() as u64) ^ (v as u64) ^ (flag as u64));
            }
            acc
        };
        assert_eq!(
            acc_view, acc_mat,
            "view/materialize accumulators must match"
        );

        group.bench_with_input(BenchmarkId::new("view_iter", n), &n, |b, &_n| {
            b.iter(|| {
                let view = norito::columnar::view_ncb_u64_bytes_u32_bool(&body).expect("view");
                let mut acc: u64 = 0;
                for i in 0..view.len() {
                    let id = view.id(i);
                    let bytes_len = view.data(i).len() as u64;
                    let v = view.val(i) as u64;
                    let flag = view.flag(i) as u64;
                    acc = acc.wrapping_add(id ^ bytes_len ^ v ^ flag);
                }
                std::hint::black_box(acc)
            })
        });

        group.bench_with_input(BenchmarkId::new("materialize", n), &n, |b, &_n| {
            b.iter(|| {
                let view = norito::columnar::view_ncb_u64_bytes_u32_bool(&body).expect("view");
                let mut vec_owned = Vec::with_capacity(view.len());
                for i in 0..view.len() {
                    let (id, data, val, flag) =
                        (view.id(i), view.data(i).to_vec(), view.val(i), view.flag(i));
                    vec_owned.push((id, data, val, flag));
                }
                let mut acc: u64 = 0;
                for (id, data, v, flag) in vec_owned {
                    acc = acc.wrapping_add(id ^ (data.len() as u64) ^ (v as u64) ^ (flag as u64));
                }
                std::hint::black_box(acc)
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_view_vs_materialize_bytes_u32);
criterion_main!(benches);
