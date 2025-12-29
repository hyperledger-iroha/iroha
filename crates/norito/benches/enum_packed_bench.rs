#[cfg(feature = "bench-internal")]
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
#[cfg(feature = "bench-internal")]
use norito::{NoritoDeserialize, NoritoSerialize, codec, decode_from_bytes, to_bytes};

#[cfg(feature = "bench-internal")]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct TuplePayload {
    value: u32,
    label: String,
    blob: Vec<u8>,
}

#[cfg(feature = "bench-internal")]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct NamedPayload {
    name: String,
    flag: bool,
}

#[cfg(feature = "bench-internal")]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct NestedPayload {
    maybe: Option<String>,
    res: Result<u32, String>,
}

#[cfg(feature = "bench-internal")]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
enum PackedEnum {
    Unit,
    Tuple(TuplePayload),
    Named(NamedPayload),
    Nested(NestedPayload),
}

#[cfg(feature = "bench-internal")]
fn make_dataset(n: usize) -> Vec<PackedEnum> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        match i % 4 {
            0 => out.push(PackedEnum::Unit),
            1 => out.push(PackedEnum::Tuple(TuplePayload {
                value: (i as u32) ^ 0xA5A5_5A5A,
                label: format!("name-{}", i % 128),
                blob: vec![((i * 31) % 251) as u8; 7 + (i % 5)],
            })),
            2 => out.push(PackedEnum::Named(NamedPayload {
                name: format!("key-{i:04}"),
                flag: (i & 1) == 0,
            })),
            _ => out.push(PackedEnum::Nested(NestedPayload {
                maybe: if (i & 3) == 0 {
                    None
                } else {
                    Some(format!("maybe-{i}"))
                },
                res: if (i & 7) < 3 {
                    Ok(((i * 17 + 13) % 10_000) as u32)
                } else {
                    Err(format!("err-{}", i % 256))
                },
            })),
        }
    }
    out
}

#[cfg(feature = "bench-internal")]
fn enum_packed_bench(c: &mut Criterion) {
    let n: usize = std::env::var("ENUM_BENCH_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    let data = make_dataset(n);

    let mut group = c.benchmark_group("enum_packed");
    group.throughput(Throughput::Elements(n as u64));

    // Encode (headered)
    group.bench_function("encode_headered_to_bytes", |b| {
        b.iter_batched(
            || data.clone(),
            |vals| {
                let mut total = 0usize;
                for v in vals {
                    let bytes = to_bytes(&v).unwrap();
                    total += bytes.len();
                    std::hint::black_box(bytes);
                }
                std::hint::black_box(total)
            },
            BatchSize::SmallInput,
        )
    });

    // Encode (bare SCALE-like)
    group.bench_function("encode_bare_codec", |b| {
        b.iter_batched(
            || data.clone(),
            |vals| {
                let mut total = 0usize;
                for v in vals {
                    let bytes = <PackedEnum as codec::Encode>::encode(&v);
                    total += bytes.len();
                    std::hint::black_box(bytes);
                }
                std::hint::black_box(total)
            },
            BatchSize::SmallInput,
        )
    });

    // Precompute bytes for decode benches
    let headered_bytes: Vec<Vec<u8>> = data.iter().map(|v| to_bytes(v).unwrap()).collect();
    let bare_bytes: Vec<Vec<u8>> = data
        .iter()
        .map(<PackedEnum as codec::Encode>::encode)
        .collect();

    // Decode (headered) + equality check
    group.bench_function("decode_headered_roundtrip_eq", |b| {
        b.iter(|| {
            for (i, bytes) in headered_bytes.iter().enumerate() {
                let out: PackedEnum = decode_from_bytes(bytes).unwrap();
                // Validate roundtrip correctness
                assert_eq!(out, data[i]);
                std::hint::black_box(&out);
            }
        })
    });

    // Decode (bare) + equality check
    group.bench_function("decode_bare_roundtrip_eq", |b| {
        b.iter(|| {
            for (i, bytes) in bare_bytes.iter().enumerate() {
                let mut cur = std::io::Cursor::new(bytes);
                let out: PackedEnum = <PackedEnum as codec::Decode>::decode(&mut cur).unwrap();
                assert_eq!(out, data[i]);
                std::hint::black_box(&out);
            }
        })
    });

    group.finish();
}

#[cfg(feature = "bench-internal")]
criterion_group!(benches, enum_packed_bench);

#[cfg(feature = "bench-internal")]
criterion_main!(benches);

#[cfg(not(feature = "bench-internal"))]
fn main() {
    eprintln!("Enable the `bench-internal` feature to run this benchmark.");
}
