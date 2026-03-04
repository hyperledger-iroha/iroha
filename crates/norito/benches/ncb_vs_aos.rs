//! Benchmark NCB (columnar) vs AoS (Vec of structs/tuples) encoding/decoding.
//!
//! Focuses on a representative shape: (u64, String, bool) with varying batch sizes.
//! Uses bare payloads (no Norito header) for apples-to-apples comparison.

#[cfg(feature = "parity-scale")]
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
#[cfg(feature = "parity-scale")]
use norito::NoritoSerialize as _;
#[cfg(feature = "parity-scale")]
use norito::codec::{Decode as _, Encode as _};
#[cfg(feature = "parity-scale")]
use norito::columnar as ncb;
#[cfg(feature = "parity-scale")]
use parity_scale_codec as scale;

#[cfg(feature = "parity-scale")]
fn make_rows(n: usize) -> Vec<(u64, String, bool)> {
    // Deterministic data with short and mid-sized names to exercise var-width
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let id = i as u64;
        let flag = (i % 3) == 0;
        let name = match i % 5 {
            0 => "alice".to_string(),
            1 => "bob".to_string(),
            2 => format!("carol{idx}", idx = i),
            3 => "dave".repeat(2),
            _ => "eve".repeat(3),
        };
        out.push((id, name, flag));
    }
    out
}

#[cfg(feature = "parity-scale")]
fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_u64_str_bool");
    for &n in &[1usize, 8, 64, 512, 4096] {
        let aos_owned = make_rows(n);
        let ncb_borrowed: Vec<(u64, &str, bool)> = aos_owned
            .iter()
            .map(|(id, s, b)| (*id, s.as_str(), *b))
            .collect();

        group.bench_with_input(BenchmarkId::new("AoS_encode", n), &n, |b, &_n| {
            b.iter(|| {
                let bytes = aos_owned.encode();
                std::hint::black_box(bytes)
            })
        });

        group.bench_with_input(BenchmarkId::new("AoS_encode_hint", n), &n, |b, &_n| {
            b.iter(|| {
                let hint = aos_owned.encoded_len_hint().unwrap_or(0);
                let mut buf = Vec::with_capacity(hint);
                aos_owned.serialize(&mut buf).unwrap();
                std::hint::black_box(buf)
            })
        });

        group.bench_with_input(BenchmarkId::new("NCB_offsets_encode", n), &n, |b, &_n| {
            b.iter(|| {
                let bytes = ncb::encode_ncb_u64_str_bool_no_dict(&ncb_borrowed);
                std::hint::black_box(bytes)
            })
        });

        group.bench_with_input(BenchmarkId::new("NCB_dict_encode", n), &n, |b, &_n| {
            b.iter(|| {
                let bytes = ncb::encode_ncb_u64_str_bool_force_dict(&ncb_borrowed);
                std::hint::black_box(bytes)
            })
        });

        group.bench_with_input(BenchmarkId::new("SCALE_encode", n), &n, |b, &_n| {
            b.iter(|| {
                let bytes = scale::Encode::encode(&aos_owned);
                std::hint::black_box(bytes)
            })
        });
    }
    group.finish();
}

#[cfg(feature = "parity-scale")]
fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_u64_str_bool");
    for &n in &[1usize, 8, 64, 512, 4096] {
        let aos_owned = make_rows(n);
        let ncb_borrowed: Vec<(u64, &str, bool)> = aos_owned
            .iter()
            .map(|(id, s, b)| (*id, s.as_str(), *b))
            .collect();

        // Pre-encode once per size
        let aos_bytes = aos_owned.encode();
        let ncb_bytes_offsets = ncb::encode_ncb_u64_str_bool_no_dict(&ncb_borrowed);
        let ncb_bytes_dict = ncb::encode_ncb_u64_str_bool_force_dict(&ncb_borrowed);
        let ncb_bytes_delta = ncb::encode_ncb_u64_str_bool_delta(&ncb_borrowed);
        let scale_bytes = scale::Encode::encode(&aos_owned);

        group.bench_with_input(
            BenchmarkId::new("AoS_decode_materialize", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let mut cur = std::io::Cursor::new(&aos_bytes);
                    let decoded: Vec<(u64, String, bool)> = <_>::decode(&mut cur).unwrap();
                    // Consume a few fields to prevent optimizing away
                    let sum = decoded
                        .iter()
                        .fold(0u64, |acc, (id, _s, _b)| acc.wrapping_add(*id));
                    std::hint::black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_decode_view_iter", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0u64;
                    for i in 0..view.len() {
                        // Touch all columns to simulate common accesses
                        sum = sum.wrapping_add(view.id(i));
                        sum = sum.wrapping_add(view.name(i).unwrap().len() as u64);
                        if view.flag(i) {
                            sum = sum.wrapping_add(1);
                        }
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        // Filtered scans: compare dense-iterator over flag==true vs row-wise filter
        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_names_flagtrue_dense", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0usize;
                    for s in view.iter_names_flag_true() {
                        sum = sum.wrapping_add(s.len());
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_names_flagtrue_rowwise", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0usize;
                    for i in 0..view.len() {
                        if view.flag(i) {
                            sum = sum.wrapping_add(view.name(i).unwrap().len());
                        }
                    }
                    std::hint::black_box(sum)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_names_flagtrue_dense_popcnt64_aligned", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0usize;
                    for s in view.iter_names_flag_true_popcount64_aligned() {
                        sum = sum.wrapping_add(s.len());
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_names_flagtrue_dense_popcnt", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0usize;
                    for s in view.iter_names_flag_true_popcount() {
                        sum = sum.wrapping_add(s.len());
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        // Filtered scans: compare dense-iterator over flag==true vs row-wise filter
        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_names_flagtrue_dense", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0usize;
                    for s in view.iter_names_flag_true() {
                        sum = sum.wrapping_add(s.len());
                    }
                    std::hint::black_box(sum)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_names_flagtrue_rowwise", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0usize;
                    for i in 0..view.len() {
                        if view.flag(i) {
                            sum = sum.wrapping_add(view.name(i).unwrap().len());
                        }
                    }
                    std::hint::black_box(sum)
                })
            },
        );
        // IDs filtered scans (offsets-based)
        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_ids_flagtrue_dense", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0u64;
                    for idx in view.iter_true_positions() {
                        sum = sum.wrapping_add(view.id(idx));
                    }
                    std::hint::black_box(sum)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_ids_flagtrue_dense_popcnt", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0u64;
                    for idx in view.iter_true_positions_popcount() {
                        sum = sum.wrapping_add(view.id(idx));
                    }
                    std::hint::black_box(sum)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_ids_flagtrue_dense_popcnt64_aligned", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0u64;
                    for idx in view.iter_true_positions_popcount64_aligned() {
                        sum = sum.wrapping_add(view.id(idx));
                    }
                    std::hint::black_box(sum)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_ids_flagtrue_rowwise", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0u64;
                    for i in 0..view.len() {
                        if view.flag(i) {
                            sum = sum.wrapping_add(view.id(i));
                        }
                    }
                    std::hint::black_box(sum)
                })
            },
        );
        // CPU-feature guided fast path
        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_names_flagtrue_fast", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0usize;
                    for s in view.iter_names_flag_true_fast() {
                        sum = sum.wrapping_add(s.len());
                    }
                    std::hint::black_box(sum)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_ids_flagtrue_fast", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let mut sum = 0u64;
                    for idx in view.iter_true_positions_fast() {
                        sum = sum.wrapping_add(view.id(idx));
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("NCB_offsets_decode_materialize", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_offsets).unwrap();
                    let rows = ncb::materialize_ncb(view).unwrap();
                    std::hint::black_box(rows)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("NCB_delta_decode_view_iter", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_delta).unwrap();
                    let mut sum = 0u64;
                    for i in 0..view.len() {
                        sum = sum.wrapping_add(view.id(i));
                        if view.flag(i) {
                            sum = sum.wrapping_add(1);
                        }
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("NCB_dict_decode_view_iter", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_dict).unwrap();
                    let mut sum = 0u64;
                    for i in 0..view.len() {
                        sum = sum.wrapping_add(view.id(i));
                        sum = sum.wrapping_add(view.name(i).unwrap().len() as u64);
                        if view.flag(i) {
                            sum = sum.wrapping_add(1);
                        }
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("NCB_dict_decode_materialize", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_str_bool(&ncb_bytes_dict).unwrap();
                    let rows = ncb::materialize_ncb(view).unwrap();
                    std::hint::black_box(rows)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("SCALE_decode_materialize", n),
            &n,
            |b, &_n| {
                b.iter(|| {
                    let decoded: Vec<(u64, String, bool)> =
                        scale::Decode::decode(&mut &scale_bytes[..]).unwrap();
                    let sum = decoded
                        .iter()
                        .fold(0u64, |acc, (id, _s, _b)| acc.wrapping_add(*id));
                    std::hint::black_box(sum)
                })
            },
        );
    }
    group.finish();
}

#[cfg(feature = "parity-scale")]
criterion_group!(benches, bench_encode, bench_decode);

#[cfg(feature = "parity-scale")]
criterion_main!(benches);

#[cfg(not(feature = "parity-scale"))]
fn main() {
    eprintln!("Enable the `parity-scale` feature to run this benchmark.");
}
