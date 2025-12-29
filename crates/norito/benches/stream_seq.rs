use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use norito::{
    Compression, decode_from_bytes, serialize_into, stream_seq_iter,
    stream_vec_collect_from_reader, stream_vec_fold_from_reader,
};

fn gen_vec(n: usize) -> Vec<u32> {
    (0..n as u32).map(|i| i.wrapping_mul(3) + 1).collect()
}

fn bench_stream_seq(c: &mut Criterion) {
    let v = gen_vec(200_000);

    // Uncompressed bytes
    let mut raw = Vec::new();
    serialize_into(&mut raw, &v, Compression::None).unwrap();

    // Compressed bytes
    let mut z = Vec::new();
    serialize_into(&mut z, &v, Compression::Zstd).unwrap();

    c.bench_function("vec_decode_from_bytes", |b| {
        b.iter_batched(
            || raw.as_slice(),
            |bytes| {
                let out: Vec<u32> = decode_from_bytes(std::hint::black_box(bytes)).unwrap();
                std::hint::black_box(out.len())
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("vec_stream_collect", |b| {
        b.iter_batched(
            || raw.as_slice(),
            |bytes| {
                let out: Vec<u32> =
                    stream_vec_collect_from_reader(std::hint::black_box(bytes)).unwrap();
                std::hint::black_box(out.len())
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("vec_stream_fold", |b| {
        b.iter_batched(
            || raw.as_slice(),
            |bytes| {
                let sum: u64 = stream_vec_fold_from_reader::<_, u32, u64, _>(
                    std::hint::black_box(bytes),
                    0,
                    |acc, v| acc + v as u64,
                )
                .unwrap();
                std::hint::black_box(sum)
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("vec_stream_iter", |b| {
        b.iter_batched(
            || raw.clone(),
            |bytes| {
                let mut it =
                    stream_seq_iter::<_, u32>(std::io::Cursor::new(std::hint::black_box(bytes)))
                        .unwrap();
                let mut sum: u64 = 0;
                for x in it.by_ref() {
                    sum = sum.wrapping_add(x.unwrap() as u64);
                }
                it.finish().unwrap();
                std::hint::black_box(sum)
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("vec_stream_iter_zstd", |b| {
        b.iter_batched(
            || z.clone(),
            |bytes| {
                let mut it =
                    stream_seq_iter::<_, u32>(std::io::Cursor::new(std::hint::black_box(bytes)))
                        .unwrap();
                let mut sum: u64 = 0;
                for x in it.by_ref() {
                    sum = sum.wrapping_add(x.unwrap() as u64);
                }
                it.finish().unwrap();
                std::hint::black_box(sum)
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_stream_seq);
criterion_main!(benches);
