//! Bench view-iterate vs materialize decode for a representative NCB shape.
//!
//! Shape: (u64, &str, bool)
//! - view: iterate borrowed rows and compute a checksum-like accumulator
//! - materialize: build owned Vec<(u64, String, bool)> and perform the same work
//!
//! Run with: cargo bench -p norito --bench view_vs_materialize

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn make_rows(n: usize) -> Vec<(u64, String, bool)> {
    let mut rows = Vec::with_capacity(n);
    for i in 0..n as u64 {
        let name = if i % 10 == 0 {
            // induce some dictionary coding
            "alpha".to_string()
        } else if i % 3 == 0 {
            "beta".to_string()
        } else {
            format!("s{i}")
        };
        rows.push((i, name, (i & 1) == 0));
    }
    rows
}

fn bench_view_vs_materialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("view_vs_materialize_u64_str_bool");
    for &n in &[1_000usize, 10_000] {
        // Prepare dataset and encode as NCB
        let owned = make_rows(n);
        let borrowed: Vec<(u64, &str, bool)> = owned
            .iter()
            .map(|(id, s, b)| (*id, s.as_str(), *b))
            .collect();
        let body = norito::columnar::encode_ncb_u64_str_bool(&borrowed);

        // Sanity: compute both accumulators once and ensure equality
        let acc_view = {
            let view = norito::columnar::view_ncb_u64_str_bool(&body).expect("view");
            let mut acc: u64 = 0;
            for i in 0..view.len() {
                let id = view.id(i);
                let name = view.name(i).expect("utf8");
                let flag = view.flag(i) as u64;
                acc = acc.wrapping_add(id ^ (name.len() as u64) ^ flag);
            }
            acc
        };
        let acc_mat = {
            let view = norito::columnar::view_ncb_u64_str_bool(&body).expect("view");
            let mut acc: u64 = 0;
            for i in 0..view.len() {
                let (id, name, flag) =
                    (view.id(i), view.name(i).unwrap().to_string(), view.flag(i));
                acc = acc.wrapping_add(id ^ (name.len() as u64) ^ (flag as u64));
            }
            acc
        };
        assert_eq!(
            acc_view, acc_mat,
            "view/materialize accumulators must match"
        );

        group.bench_with_input(BenchmarkId::new("view_iter", n), &n, |b, &_n| {
            b.iter(|| {
                let view = norito::columnar::view_ncb_u64_str_bool(&body).expect("view");
                let mut acc: u64 = 0;
                for i in 0..view.len() {
                    let id = view.id(i);
                    let name = view.name(i).expect("utf8");
                    let flag = view.flag(i) as u64;
                    acc = acc.wrapping_add(id ^ (name.len() as u64) ^ flag);
                }
                std::hint::black_box(acc)
            })
        });

        group.bench_with_input(BenchmarkId::new("materialize", n), &n, |b, &_n| {
            b.iter(|| {
                let view = norito::columnar::view_ncb_u64_str_bool(&body).expect("view");
                let mut vec_owned = Vec::with_capacity(view.len());
                for i in 0..view.len() {
                    let (id, name, flag) =
                        (view.id(i), view.name(i).unwrap().to_string(), view.flag(i));
                    vec_owned.push((id, name, flag));
                }
                let mut acc: u64 = 0;
                for (id, name, flag) in vec_owned {
                    acc = acc.wrapping_add(id ^ (name.len() as u64) ^ (flag as u64));
                }
                std::hint::black_box(acc)
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_view_vs_materialize);
criterion_main!(benches);
