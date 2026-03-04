//! Benchmarks for AES round helpers (scalar vs accelerated).
use criterion::Criterion;
use ivm::{aesdec, aesdec_impl, aesenc, aesenc_impl};

fn bench_aesenc(c: &mut Criterion) {
    let state = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ];
    let rk = [
        0x0f, 0x47, 0x0c, 0xaf, 0x15, 0xd9, 0xb7, 0x7f, 0x71, 0xe8, 0xad, 0x67, 0xc9, 0x59, 0xd6,
        0x98,
    ];
    // Scalar reference
    c.bench_function("aesenc_scalar", |b| {
        b.iter(|| {
            let out = aesenc_impl(std::hint::black_box(state), std::hint::black_box(rk));
            std::hint::black_box(out)
        })
    });
    // Auto (accelerated if available: CUDA/AES-NI/AESE)
    c.bench_function("aesenc_auto", |b| {
        b.iter(|| {
            let out = aesenc(std::hint::black_box(state), std::hint::black_box(rk));
            std::hint::black_box(out)
        })
    });
}

fn bench_aesdec(c: &mut Criterion) {
    // Use the output of one round as the input for dec-round
    let enc_in = [
        0x69, 0xc4, 0xe0, 0xd8, 0x6a, 0x7b, 0x04, 0x30, 0xd8, 0xcd, 0xb7, 0x80, 0x70, 0xb4, 0xc5,
        0x5a,
    ];
    let rk = [
        0x0f, 0x47, 0x0c, 0xaf, 0x15, 0xd9, 0xb7, 0x7f, 0x71, 0xe8, 0xad, 0x67, 0xc9, 0x59, 0xd6,
        0x98,
    ];
    c.bench_function("aesdec_scalar", |b| {
        b.iter(|| {
            let out = aesdec_impl(std::hint::black_box(enc_in), std::hint::black_box(rk));
            std::hint::black_box(out)
        })
    });
    c.bench_function("aesdec_auto", |b| {
        b.iter(|| {
            let out = aesdec(std::hint::black_box(enc_in), std::hint::black_box(rk));
            std::hint::black_box(out)
        })
    });
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_aesenc(&mut c);
    bench_aesdec(&mut c);
    c.final_summary();
}
