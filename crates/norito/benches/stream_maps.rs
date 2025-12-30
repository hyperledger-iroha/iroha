use std::collections::{BTreeMap, HashMap};

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use norito::{
    Compression, StreamMapIter, decode_from_bytes, serialize_into,
    stream_btreemap_collect_from_reader, stream_hashmap_collect_from_reader,
};
use rand::{Rng, SeedableRng, rngs::StdRng};

fn gen_maps(n: usize) -> (HashMap<String, u32>, BTreeMap<u64, String>) {
    let mut rng = StdRng::seed_from_u64(42);
    let mut hm = HashMap::with_capacity(n);
    let mut bm = BTreeMap::new();
    for i in 0..n as u64 {
        let k = format!("k-{}-{}", i, rng.random::<u32>());
        hm.insert(k, (i as u32).wrapping_mul(3) + 1);
        bm.insert(i * 7 + 1, format!("v-{}-{}", i, rng.random::<u64>()));
    }
    (hm, bm)
}

fn bench_stream_maps(c: &mut Criterion) {
    let (hm, bm) = gen_maps(50_000);

    // Uncompressed HashMap bytes
    let mut hm_bytes = Vec::new();
    // In benches, avoid panicking on encoding errors; skip work if it fails
    if let Err(_e) = serialize_into(&mut hm_bytes, &hm, Compression::None) {
        return;
    }

    // Compressed BTreeMap bytes
    let mut bm_bytes = Vec::new();
    if let Err(_e) = serialize_into(&mut bm_bytes, &bm, Compression::Zstd) {
        return;
    }

    c.bench_function("hashmap_decode_from_bytes", |b| {
        b.iter_batched(
            || hm_bytes.as_slice(),
            |bytes| {
                let out: Result<HashMap<String, u32>, _> =
                    decode_from_bytes(std::hint::black_box(bytes));
                std::hint::black_box(out.map(|m| m.len()).unwrap_or(0))
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("hashmap_stream_collect", |b| {
        b.iter_batched(
            || hm_bytes.as_slice(),
            |bytes| {
                let out = stream_hashmap_collect_from_reader::<_, String, u32>(
                    std::hint::black_box(bytes),
                )
                .map(|m| m.len())
                .unwrap_or(0);
                std::hint::black_box(out)
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("hashmap_stream_iter", |b| {
        b.iter_batched(
            || hm_bytes.clone(),
            |bytes| {
                let it = StreamMapIter::<String, u32>::new_hash(std::io::Cursor::new(
                    std::hint::black_box(bytes),
                ));
                let mut s: u64 = 0;
                if let Ok(mut it) = it {
                    for kv in it.by_ref() {
                        if let Ok((_k, v)) = kv {
                            s = s.wrapping_add(v as u64);
                        } else {
                            break;
                        }
                    }
                }
                std::hint::black_box(s)
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("btreemap_decode_from_bytes", |b| {
        b.iter_batched(
            || bm_bytes.as_slice(),
            |bytes| {
                let out: Result<BTreeMap<u64, String>, _> =
                    decode_from_bytes(std::hint::black_box(bytes));
                std::hint::black_box(out.map(|m| m.len()).unwrap_or(0))
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("btreemap_stream_collect", |b| {
        b.iter_batched(
            || bm_bytes.as_slice(),
            |bytes| {
                let out = stream_btreemap_collect_from_reader::<_, u64, String>(
                    std::hint::black_box(bytes),
                )
                .map(|m| m.len())
                .unwrap_or(0);
                std::hint::black_box(out)
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("btreemap_stream_iter", |b| {
        b.iter_batched(
            || bm_bytes.clone(),
            |bytes| {
                let it = StreamMapIter::<u64, String>::new_btree(std::io::Cursor::new(
                    std::hint::black_box(bytes),
                ));
                let mut s: usize = 0;
                if let Ok(mut it) = it {
                    for kv in it.by_ref() {
                        if let Ok((_k, v)) = kv {
                            s = s.wrapping_add(v.len());
                        } else {
                            break;
                        }
                    }
                }
                std::hint::black_box(s)
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_stream_maps);
criterion_main!(benches);
