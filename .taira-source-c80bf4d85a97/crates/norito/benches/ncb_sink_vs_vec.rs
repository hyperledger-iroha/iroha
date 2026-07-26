use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use norito::columnar;
use rand::{Rng, SeedableRng, rngs::StdRng};

fn gen_rows(n: usize) -> Vec<(u64, String, bool)> {
    let mut rng = StdRng::seed_from_u64(42);
    let mut rows = Vec::with_capacity(n);
    for i in 0..n as u64 {
        let len = rng.random_range(4..24);
        let s: String = (0..len)
            .map(|_| (b'a' + (rng.random::<u8>() % 26)) as char)
            .collect();
        let flag = rng.random::<bool>();
        rows.push((i * 3 + 1, s, flag));
    }
    rows
}

// Simplified previous Vec-based encoder for (u64, &str, bool)
fn encode_vec(rows: &[(u64, &str, bool)]) -> Vec<u8> {
    let n = rows.len() as u32;
    let mut buf = Vec::with_capacity(4 + 1 + rows.len() * (8 + 1 + 4) + 16);
    buf.extend_from_slice(&n.to_le_bytes());
    buf.push(0x13);
    // ids
    let mis = buf.len() & 7;
    if mis != 0 {
        buf.extend(std::iter::repeat_n(0u8, 8 - mis));
    }
    for (id, _, _) in rows {
        buf.extend_from_slice(&id.to_le_bytes());
    }
    // strings offsets + blob
    let mis4 = buf.len() & 3;
    if mis4 != 0 {
        buf.extend(std::iter::repeat_n(0u8, 4 - mis4));
    }
    let base = buf.len();
    buf.extend(std::iter::repeat_n(0u8, (rows.len() + 1) * 4));
    let mut acc: u32 = 0;
    let mut offs = Vec::with_capacity(rows.len() + 1);
    offs.push(0);
    for (_, s, _) in rows {
        let b = s.as_bytes();
        acc = acc.wrapping_add(b.len() as u32);
        offs.push(acc);
        buf.extend_from_slice(b);
    }
    for (i, v) in offs.iter().enumerate() {
        let p = base + i * 4;
        buf[p..p + 4].copy_from_slice(&v.to_le_bytes());
    }
    // flags
    let bit_bytes = rows.len().div_ceil(8);
    let start = buf.len();
    buf.resize(start + bit_bytes, 0);
    for (i, (_, _, b)) in rows.iter().enumerate() {
        if *b {
            buf[start + (i / 8)] |= 1u8 << (i % 8);
        }
    }
    buf
}

// Current ByteSink-based encoder (same as library implementation)
fn encode_sink(rows: &[(u64, &str, bool)]) -> Vec<u8> {
    columnar::encode_ncb_u64_str_bool(rows)
}

fn bench_ncb_sink_vs_vec(c: &mut Criterion) {
    let rows_owned = gen_rows(10_000);
    let rows_borrowed: Vec<(u64, &str, bool)> = rows_owned
        .iter()
        .map(|(id, s, b)| (*id, s.as_str(), *b))
        .collect();

    c.bench_function("ncb_vec_encode_10k", |b| {
        b.iter_batched(
            || rows_borrowed.clone(),
            |rows| {
                let _ = std::hint::black_box(encode_vec(&rows));
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("ncb_sink_encode_10k", |b| {
        b.iter_batched(
            || rows_borrowed.clone(),
            |rows| {
                let _ = std::hint::black_box(encode_sink(&rows));
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_ncb_sink_vs_vec);
criterion_main!(benches);
