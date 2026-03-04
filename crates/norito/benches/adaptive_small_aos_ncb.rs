//! Quick AoS vs NCB adaptive small-N benches for `(u64, &str, bool)`.
//!
//! For small inputs, the adaptive encoder does a two-pass probe and picks the
//! smaller of AoS and NCB. We craft two datasets:
//! - AoS-friendly: distinct short names → AoS typically wins.
//! - NCB-friendly: many repeated names → NCB with dictionary should win.
//!
//! We then benchmark view-iterate and materialize for each chosen layout.
//!
//! Run: cargo bench -p norito --bench adaptive_small_aos_ncb

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn rows_aos_friendly(n: usize) -> Vec<(u64, String, bool)> {
    (0..n as u64)
        .map(|i| (i, format!("s{i}"), (i & 1) == 0))
        .collect()
}

fn rows_ncb_friendly(n: usize) -> Vec<(u64, String, bool)> {
    (0..n as u64)
        .map(|i| {
            let name = if i % 3 == 0 { "alpha" } else { "beta" };
            (i, name.to_string(), (i & 1) == 1)
        })
        .collect()
}

fn bench_adaptive_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_small_u64_str_bool");
    for &n in &[8usize, 32, 64] {
        // AoS-friendly dataset
        let owned = rows_aos_friendly(n);
        let borrowed: Vec<(u64, &str, bool)> = owned
            .iter()
            .map(|(id, s, b)| (*id, s.as_str(), *b))
            .collect();
        let aos_bytes = norito::columnar::encode_rows_u64_str_bool_adaptive(&borrowed);
        assert_eq!(
            aos_bytes[0],
            norito::columnar::ADAPTIVE_TAG_AOS,
            "expected AoS tag for aos-friendly dataset"
        );
        let aos_body = &aos_bytes[1..];

        // Sanity: AoS view/materialize produce identical accumulators
        let aos_acc_view = {
            let view = norito::columnar::view_aos_u64_str_bool(aos_body).expect("view");
            let mut acc: u64 = 0;
            for i in 0..view.len() {
                let id = view.id(i);
                let name = view.name(i).expect("utf8");
                let flag = view.flag(i) as u64;
                acc = acc.wrapping_add(id ^ (name.len() as u64) ^ flag);
            }
            acc
        };
        let aos_acc_mat = {
            let view = norito::columnar::view_aos_u64_str_bool(aos_body).expect("view");
            let mut acc: u64 = 0;
            for i in 0..view.len() {
                let (id, name, flag) =
                    (view.id(i), view.name(i).unwrap().to_string(), view.flag(i));
                acc = acc.wrapping_add(id ^ (name.len() as u64) ^ (flag as u64));
            }
            acc
        };
        assert_eq!(aos_acc_view, aos_acc_mat, "AoS accumulators must match");

        group.bench_with_input(BenchmarkId::new("aos_view_iter", n), &n, |b, &_n| {
            b.iter(|| {
                let view = norito::columnar::view_aos_u64_str_bool(aos_body).expect("view");
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

        group.bench_with_input(BenchmarkId::new("aos_materialize", n), &n, |b, &_n| {
            b.iter(|| {
                let view = norito::columnar::view_aos_u64_str_bool(aos_body).expect("view");
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

        // NCB-friendly dataset
        let owned = rows_ncb_friendly(n);
        let borrowed: Vec<(u64, &str, bool)> = owned
            .iter()
            .map(|(id, s, b)| (*id, s.as_str(), *b))
            .collect();
        let ncb_bytes = norito::columnar::encode_rows_u64_str_bool_adaptive(&borrowed);
        assert_eq!(
            ncb_bytes[0],
            norito::columnar::ADAPTIVE_TAG_NCB,
            "expected NCB tag for ncb-friendly dataset"
        );
        let ncb_body = &ncb_bytes[1..];

        // Sanity: NCB view/materialize produce identical accumulators
        let ncb_acc_view = {
            let view = norito::columnar::view_ncb_u64_str_bool(ncb_body).expect("view");
            let mut acc: u64 = 0;
            for i in 0..view.len() {
                let id = view.id(i);
                let name = view.name(i).expect("utf8");
                let flag = view.flag(i) as u64;
                acc = acc.wrapping_add(id ^ (name.len() as u64) ^ flag);
            }
            acc
        };
        let ncb_acc_mat = {
            let view = norito::columnar::view_ncb_u64_str_bool(ncb_body).expect("view");
            let mut acc: u64 = 0;
            for i in 0..view.len() {
                let (id, name, flag) =
                    (view.id(i), view.name(i).unwrap().to_string(), view.flag(i));
                acc = acc.wrapping_add(id ^ (name.len() as u64) ^ (flag as u64));
            }
            acc
        };
        assert_eq!(ncb_acc_view, ncb_acc_mat, "NCB accumulators must match");

        group.bench_with_input(BenchmarkId::new("ncb_view_iter", n), &n, |b, &_n| {
            b.iter(|| {
                let view = norito::columnar::view_ncb_u64_str_bool(ncb_body).expect("view");
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

        group.bench_with_input(BenchmarkId::new("ncb_materialize", n), &n, |b, &_n| {
            b.iter(|| {
                let view = norito::columnar::view_ncb_u64_str_bool(ncb_body).expect("view");
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

criterion_group!(benches, bench_adaptive_small);
criterion_main!(benches);
