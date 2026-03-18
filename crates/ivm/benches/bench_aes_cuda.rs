//! Benchmark AES round (CPU vs CUDA) when `--features cuda` is enabled.
//!
//! Notes:
//! - Kernel launches per round incur overhead; this measures end-to-end time.
//! - To see GPU speedups, consider batching rounds per launch in future work.
use criterion::Criterion;

fn data() -> ([u8; 16], [u8; 16]) {
    let state = [
        0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ];
    let rk = [
        0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf, 0x7f, 0x67,
        0x98,
    ];
    (state, rk)
}

fn bench_aesenc_cpu(c: &mut Criterion) {
    let (state, rk) = data();
    c.bench_function("aesenc_cpu_round", |b| {
        b.iter(|| {
            let out = ivm::aesenc_impl(std::hint::black_box(state), std::hint::black_box(rk));
            std::hint::black_box(out)
        })
    });
}

fn bench_aesdec_cpu(c: &mut Criterion) {
    let (state, rk) = data();
    let enc = ivm::aesenc_impl(state, rk);
    c.bench_function("aesdec_cpu_round", |b| {
        b.iter(|| {
            let out = ivm::aesdec_impl(std::hint::black_box(enc), std::hint::black_box(rk));
            std::hint::black_box(out)
        })
    });
}

fn bench_aesenc_cuda(c: &mut Criterion) {
    if !ivm::cuda_available() {
        eprintln!("CUDA not available; skipping aesenc_cuda benchmark");
        return;
    }
    let (state, rk) = data();
    c.bench_function("aesenc_cuda_round", |b| {
        b.iter(|| {
            let out = ivm::aesenc_cuda(std::hint::black_box(state), std::hint::black_box(rk))
                .expect("cuda result");
            std::hint::black_box(out)
        })
    });
}

fn bench_aesdec_cuda(c: &mut Criterion) {
    if !ivm::cuda_available() {
        eprintln!("CUDA not available; skipping aesdec_cuda benchmark");
        return;
    }
    let (state, rk) = data();
    let enc = ivm::aesenc_cuda(state, rk).expect("cuda enc");
    c.bench_function("aesdec_cuda_round", |b| {
        b.iter(|| {
            let out = ivm::aesdec_cuda(std::hint::black_box(enc), std::hint::black_box(rk))
                .expect("cuda result");
            std::hint::black_box(out)
        })
    });
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_aesenc_cpu(&mut c);
    bench_aesdec_cpu(&mut c);
    bench_aesenc_cuda(&mut c);
    bench_aesdec_cuda(&mut c);
    bench_aesenc_cuda_batch(&mut c);
    bench_aesdec_cuda_batch(&mut c);
    c.final_summary();
}

fn bench_aesenc_cuda_batch(c: &mut Criterion) {
    if !ivm::cuda_available() {
        eprintln!("CUDA not available; skipping aesenc_cuda_batch benchmark");
        return;
    }
    let (_state, rk) = data();
    let blocks = 16384usize; // 16k blocks per launch
    let mut states = vec![[0u8; 16]; blocks];
    for i in 0..blocks {
        states[i][0] = (i & 0xff) as u8;
    }
    c.bench_function("aesenc_cuda_batch_round", |b| {
        b.iter(|| {
            let out =
                ivm::aesenc_batch_cuda(std::hint::black_box(&states), std::hint::black_box(rk))
                    .expect("cuda batch");
            std::hint::black_box(out)
        })
    });
    c.bench_function("aesenc_cpu_loop_round", |b| {
        b.iter(|| {
            let out: Vec<[u8; 16]> = states
                .iter()
                .map(|&s| ivm::aesenc_impl(std::hint::black_box(s), std::hint::black_box(rk)))
                .collect();
            std::hint::black_box(out)
        })
    });
}

fn bench_aesdec_cuda_batch(c: &mut Criterion) {
    if !ivm::cuda_available() {
        eprintln!("CUDA not available; skipping aesdec_cuda_batch benchmark");
        return;
    }
    let (_state, rk) = data();
    let blocks = 16384usize;
    let mut states = vec![[0u8; 16]; blocks];
    for i in 0..blocks {
        states[i][0] = (i & 0xff) as u8;
    }
    // Pre-encode once via CUDA to simulate decode input
    let enc = ivm::aesenc_batch_cuda(&states, rk).expect("cuda batch enc");
    c.bench_function("aesdec_cuda_batch_round", |b| {
        b.iter(|| {
            let out = ivm::aesdec_batch_cuda(std::hint::black_box(&enc), std::hint::black_box(rk))
                .expect("cuda batch");
            std::hint::black_box(out)
        })
    });
    c.bench_function("aesdec_cpu_loop_round", |b| {
        b.iter(|| {
            let out: Vec<[u8; 16]> = enc
                .iter()
                .map(|&s| ivm::aesdec_impl(std::hint::black_box(s), std::hint::black_box(rk)))
                .collect();
            std::hint::black_box(out)
        })
    });
}
