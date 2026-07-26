use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use norito::columnar::{EnumBorrow, encode_ncb_u64_enum_bool, view_ncb_u64_enum_bool};

fn gen_enum_rows(n: usize) -> Vec<(u64, EnumBorrow<'static>, bool)> {
    let mut v = Vec::with_capacity(n);
    for i in 0..n as u64 {
        let en = match i % 3 {
            0 => EnumBorrow::Name("alice"),
            1 => EnumBorrow::Name("bob"),
            _ => EnumBorrow::Code((i % 100) as u32),
        };
        let flag = i % 5 == 0 || i % 7 == 0;
        v.push((i * 3 + 1, en, flag));
    }
    v
}

fn bench_enum_indexed(c: &mut Criterion) {
    let rows = gen_enum_rows(50_000);
    let bytes = encode_ncb_u64_enum_bool(&rows, false, false, false);
    let view = view_ncb_u64_enum_bool(&bytes).expect("view");

    c.bench_function("enum_names_flag_true_fast", |b| {
        b.iter_batched(
            || (),
            |_| {
                let mut cnt = 0usize;
                for _s in view.iter_names_flag_true_fast() {
                    cnt += 1;
                }
                std::hint::black_box(cnt)
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("enum_names_flag_true_indexed", |b| {
        b.iter_batched(
            || (),
            |_| {
                let mut cnt = 0usize;
                for _s in view.iter_names_flag_true_indexed() {
                    cnt += 1;
                }
                std::hint::black_box(cnt)
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("enum_codes_flag_true_fast", |b| {
        b.iter_batched(
            || (),
            |_| {
                let mut sum = 0u64;
                for v in view.iter_codes_flag_true_fast() {
                    sum = sum.wrapping_add(v as u64);
                }
                std::hint::black_box(sum)
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("enum_codes_flag_true_indexed", |b| {
        b.iter_batched(
            || (),
            |_| {
                let mut sum = 0u64;
                for v in view.iter_codes_flag_true_indexed() {
                    sum = sum.wrapping_add(v as u64);
                }
                std::hint::black_box(sum)
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_enum_indexed);
criterion_main!(benches);
