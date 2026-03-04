//! Enum-heavy dataset benches: AoS vs Norito NCB enum layout, plus projections.

#[cfg(all(feature = "bench-internal", feature = "parity-scale"))]
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
#[cfg(all(feature = "bench-internal", feature = "parity-scale"))]
use norito::codec::{Decode as _, Encode as _};
#[cfg(all(feature = "bench-internal", feature = "parity-scale"))]
use norito::columnar as ncb;
#[cfg(all(feature = "bench-internal", feature = "parity-scale"))]
use parity_scale_codec as scale;

#[cfg(all(feature = "bench-internal", feature = "parity-scale"))]
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
enum BenchEnum {
    Name(String),
    Code(u32),
}

#[cfg(all(feature = "bench-internal", feature = "parity-scale"))]
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct BenchRow {
    id: u64,
    payload: BenchEnum,
    flag: bool,
}

#[cfg(all(feature = "bench-internal", feature = "parity-scale"))]
fn make_data(n: usize) -> Vec<BenchRow> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let id = (i as u64) * 3;
        let flag = (i % 5) == 0;
        let payload = if i % 10 < 7 {
            // 70% Name, varied sizes
            let name = match i % 6 {
                0 => "alice".into(),
                1 => "bob".into(),
                2 => format!("n{}", i),
                3 => "zz".repeat(4),
                4 => "q".repeat(1 + (i % 8)),
                _ => "name".repeat(3),
            };
            BenchEnum::Name(name)
        } else {
            BenchEnum::Code((i * 11) as u32)
        };
        out.push(BenchRow { id, payload, flag });
    }
    out
}

#[cfg(all(feature = "bench-internal", feature = "parity-scale"))]
fn bench_enum_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("enum_encode");
    for &n in &[64usize, 512, 4096] {
        let aos = make_data(n);
        // Borrow for NCB
        let borrowed: Vec<(u64, ncb::EnumBorrow<'_>, bool)> = aos
            .iter()
            .map(|r| match &r.payload {
                BenchEnum::Name(s) => (r.id, ncb::EnumBorrow::Name(s.as_str()), r.flag),
                BenchEnum::Code(v) => (r.id, ncb::EnumBorrow::Code(*v), r.flag),
            })
            .collect();

        group.bench_with_input(BenchmarkId::new("AoS_Norito", n), &n, |b, _| {
            b.iter(|| std::hint::black_box(aos.encode()))
        });
        group.bench_with_input(BenchmarkId::new("SCALE", n), &n, |b, _| {
            b.iter(|| std::hint::black_box(scale::Encode::encode(&aos)))
        });
        group.bench_with_input(BenchmarkId::new("NCB_enum_offsets", n), &n, |b, _| {
            b.iter(|| {
                std::hint::black_box(ncb::encode_ncb_u64_enum_bool(
                    &borrowed, false, false, false,
                ))
            })
        });
        group.bench_with_input(BenchmarkId::new("NCB_enum_delta_ids", n), &n, |b, _| {
            b.iter(|| {
                std::hint::black_box(ncb::encode_ncb_u64_enum_bool(&borrowed, true, false, false))
            })
        });
        group.bench_with_input(BenchmarkId::new("NCB_enum_codes_delta", n), &n, |b, _| {
            b.iter(|| {
                std::hint::black_box(ncb::encode_ncb_u64_enum_bool(&borrowed, false, false, true))
            })
        });
        group.bench_with_input(BenchmarkId::new("NCB_enum_dict_names", n), &n, |b, _| {
            b.iter(|| {
                std::hint::black_box(ncb::encode_ncb_u64_enum_bool(&borrowed, false, true, false))
            })
        });
        group.bench_with_input(
            BenchmarkId::new("NCB_enum_dict_names_codes_delta", n),
            &n,
            |b, _| {
                b.iter(|| {
                    std::hint::black_box(ncb::encode_ncb_u64_enum_bool(
                        &borrowed, false, true, true,
                    ))
                })
            },
        );
    }
    group.finish();
}

#[cfg(all(feature = "bench-internal", feature = "parity-scale"))]
fn bench_enum_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("enum_decode");
    for &n in &[64usize, 512, 4096] {
        let aos = make_data(n);
        let borrowed: Vec<(u64, ncb::EnumBorrow<'_>, bool)> = aos
            .iter()
            .map(|r| match &r.payload {
                BenchEnum::Name(s) => (r.id, ncb::EnumBorrow::Name(s.as_str()), r.flag),
                BenchEnum::Code(v) => (r.id, ncb::EnumBorrow::Code(*v), r.flag),
            })
            .collect();
        let aos_bytes = aos.encode();
        let scale_bytes = scale::Encode::encode(&aos);
        let ncb_bytes = ncb::encode_ncb_u64_enum_bool(&borrowed, false, false, false);
        let ncb_delta_bytes = ncb::encode_ncb_u64_enum_bool(&borrowed, true, false, false);
        let ncb_code_delta_bytes = ncb::encode_ncb_u64_enum_bool(&borrowed, false, false, true);
        let ncb_dict_bytes = ncb::encode_ncb_u64_enum_bool(&borrowed, false, true, false);
        let ncb_dict_code_delta_bytes = ncb::encode_ncb_u64_enum_bool(&borrowed, false, true, true);

        group.bench_with_input(BenchmarkId::new("AoS_decode_materialize", n), &n, |b, _| {
            b.iter(|| {
                let mut cur = std::io::Cursor::new(&aos_bytes);
                let decoded: Vec<BenchRow> = <_>::decode(&mut cur).unwrap();
                std::hint::black_box(decoded.len())
            })
        });

        group.bench_with_input(
            BenchmarkId::new("SCALE_decode_materialize", n),
            &n,
            |b, _| {
                b.iter(|| {
                    let decoded: Vec<BenchRow> =
                        scale::Decode::decode(&mut &scale_bytes[..]).unwrap();
                    std::hint::black_box(decoded.len())
                })
            },
        );

        group.bench_with_input(BenchmarkId::new("NCB_enum_view_iter", n), &n, |b, _| {
            b.iter(|| {
                let view = ncb::view_ncb_u64_enum_bool(&ncb_bytes).unwrap();
                let mut acc = 0u64;
                for i in 0..view.len() {
                    acc = acc.wrapping_add(view.id(i));
                    let _ = view.tag(i);
                    if view.flag(i) {
                        acc = acc.wrapping_add(1);
                    }
                }
                std::hint::black_box(acc)
            })
        });

        group.bench_with_input(
            BenchmarkId::new("NCB_enum_delta_view_iter", n),
            &n,
            |b, _| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_enum_bool(&ncb_delta_bytes).unwrap();
                    let mut acc = 0u64;
                    for i in 0..view.len() {
                        acc = acc.wrapping_add(view.id(i));
                        if let Ok(ncb::ColEnumRef::Code(v)) = view.payload(i) {
                            acc = acc.wrapping_add(v as u64);
                        }
                    }
                    std::hint::black_box(acc)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("NCB_enum_codes_delta_view_iter", n),
            &n,
            |b, _| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_enum_bool(&ncb_code_delta_bytes).unwrap();
                    let mut acc = 0u64;
                    for i in 0..view.len() {
                        if let Ok(ncb::ColEnumRef::Code(v)) = view.payload(i) {
                            acc = acc.wrapping_add(v as u64);
                        }
                    }
                    std::hint::black_box(acc)
                })
            },
        );

        // Projection-only scans: tags and ids
        group.bench_with_input(BenchmarkId::new("NCB_project_tags_ids", n), &n, |b, _| {
            b.iter(|| {
                let view = ncb::view_ncb_u64_enum_bool(&ncb_bytes).unwrap();
                let tags = view.tags_slice();
                let ids_sum = if let Some(ids) = view.ids_slice() {
                    ids.iter().copied().sum::<u64>()
                } else {
                    (0..view.len()).map(|i| view.id(i)).sum::<u64>()
                };
                std::hint::black_box(tags.len() ^ (ids_sum as usize))
            })
        });

        // Column-only scans: skip irrelevant variants entirely
        group.bench_with_input(BenchmarkId::new("NCB_names_column_only", n), &n, |b, _| {
            b.iter(|| {
                let view = ncb::view_ncb_u64_enum_bool(&ncb_bytes).unwrap();
                let mut sum = 0usize;
                for k in 0..view.names_count() {
                    sum += view.name_k(k).unwrap().len();
                }
                std::hint::black_box(sum)
            })
        });
        group.bench_with_input(BenchmarkId::new("NCB_codes_column_only", n), &n, |b, _| {
            b.iter(|| {
                let view = ncb::view_ncb_u64_enum_bool(&ncb_bytes).unwrap();
                let mut sum = 0u64;
                for k in 0..view.codes_count() {
                    sum = sum.wrapping_add(view.code_k(k).unwrap() as u64);
                }
                std::hint::black_box(sum)
            })
        });

        group.bench_with_input(
            BenchmarkId::new("NCB_names_column_only_dict", n),
            &n,
            |b, _| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_enum_bool(&ncb_dict_bytes).unwrap();
                    let mut sum = 0usize;
                    for k in 0..view.names_count() {
                        sum += view.name_k(k).unwrap().len();
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("NCB_enum_dict_codes_delta_view_iter", n),
            &n,
            |b, _| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_enum_bool(&ncb_dict_code_delta_bytes).unwrap();
                    let mut acc = 0u64;
                    for i in 0..view.len() {
                        if let Ok(ncb::ColEnumRef::Code(v)) = view.payload(i) {
                            acc = acc.wrapping_add(v as u64);
                        }
                    }
                    std::hint::black_box(acc)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("NCB_codes_column_only_dict_codes_delta", n),
            &n,
            |b, _| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_enum_bool(&ncb_dict_code_delta_bytes).unwrap();
                    let mut sum = 0u64;
                    for k in 0..view.codes_count() {
                        sum = sum.wrapping_add(view.code_k(k).unwrap() as u64);
                    }
                    std::hint::black_box(sum)
                })
            },
        );

        // Dense iterators over subcolumns
        group.bench_with_input(BenchmarkId::new("NCB_iter_names_dense", n), &n, |b, _| {
            b.iter(|| {
                let view = ncb::view_ncb_u64_enum_bool(&ncb_bytes).unwrap();
                let mut acc = 0usize;
                for s in view.iter_names_dense() {
                    acc = acc.wrapping_add(s.len());
                }
                std::hint::black_box(acc)
            })
        });

        group.bench_with_input(BenchmarkId::new("NCB_iter_codes_dense", n), &n, |b, _| {
            b.iter(|| {
                let view = ncb::view_ncb_u64_enum_bool(&ncb_bytes).unwrap();
                let mut acc = 0u64;
                for v in view.iter_codes_dense() {
                    acc = acc.wrapping_add(v as u64);
                }
                std::hint::black_box(acc)
            })
        });

        group.bench_with_input(
            BenchmarkId::new("NCB_iter_names_dense_dict", n),
            &n,
            |b, _| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_enum_bool(&ncb_dict_bytes).unwrap();
                    let mut acc = 0usize;
                    for s in view.iter_names_dense() {
                        acc = acc.wrapping_add(s.len());
                    }
                    std::hint::black_box(acc)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("NCB_iter_codes_dense_dict_codes_delta", n),
            &n,
            |b, _| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_enum_bool(&ncb_dict_code_delta_bytes).unwrap();
                    let mut acc = 0u64;
                    for v in view.iter_codes_dense() {
                        acc = acc.wrapping_add(v as u64);
                    }
                    std::hint::black_box(acc)
                })
            },
        );

        // Filtered scans on flag: names and ids using fast vs row-wise
        group.bench_with_input(
            BenchmarkId::new("NCB_enum_names_flagtrue_fast", n),
            &n,
            |b, _| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_enum_bool(&ncb_bytes).unwrap();
                    let mut sum = 0usize;
                    for s in view.iter_names_flag_true_fast() {
                        sum = sum.wrapping_add(s.len());
                    }
                    std::hint::black_box(sum)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("NCB_enum_names_flagtrue_rowwise", n),
            &n,
            |b, _| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_enum_bool(&ncb_bytes).unwrap();
                    let mut sum = 0usize;
                    for i in 0..view.len() {
                        if view.flag(i)
                            && let Ok(ncb::ColEnumRef::Name(s)) = view.payload(i)
                        {
                            sum = sum.wrapping_add(s.len());
                        }
                    }
                    std::hint::black_box(sum)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("NCB_enum_ids_flagtrue_fast", n),
            &n,
            |b, _| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_enum_bool(&ncb_bytes).unwrap();
                    let mut sum = 0u64;
                    for idx in view.iter_true_positions_fast() {
                        sum = sum.wrapping_add(view.id(idx));
                    }
                    std::hint::black_box(sum)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("NCB_enum_ids_flagtrue_rowwise", n),
            &n,
            |b, _| {
                b.iter(|| {
                    let view = ncb::view_ncb_u64_enum_bool(&ncb_bytes).unwrap();
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
    }
    group.finish();
}

#[cfg(all(feature = "bench-internal", feature = "parity-scale"))]
criterion_group!(benches, bench_enum_encode, bench_enum_decode);

#[cfg(all(feature = "bench-internal", feature = "parity-scale"))]
criterion_main!(benches);

#[cfg(not(all(feature = "bench-internal", feature = "parity-scale")))]
fn main() {
    eprintln!("Enable `bench-internal` and `parity-scale` features to run this benchmark.");
}
