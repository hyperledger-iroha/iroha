//! Bench multi-round AES streaming helpers (CPU/Metal/CUDA under the hood).
use criterion::Criterion;

fn make_blocks(n: usize) -> Vec<[u8; 16]> {
    let mut v = vec![[0u8; 16]; n];
    for (i, blk) in v.iter_mut().enumerate() {
        blk[0] = (i & 0xff) as u8;
        blk[1] = ((i >> 8) & 0xff) as u8;
    }
    v
}

fn make_round_keys(n: usize) -> Vec<[u8; 16]> {
    let mut rks = Vec::with_capacity(n);
    let base = [
        0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf, 0x7f, 0x67,
        0x98,
    ];
    for i in 0..n {
        let mut rk = base;
        rk[0] ^= (i & 0xff) as u8;
        rks.push(rk);
    }
    rks
}

fn bench_aesenc_stream(c: &mut Criterion) {
    let blocks = 16384usize; // 16k blocks
    let rounds = 9usize; // stream of 9 rounds (not full AES)
    let states = make_blocks(blocks);
    let rks = make_round_keys(rounds);
    c.bench_function("aesenc_n_rounds_many_stream", |b| {
        b.iter(|| {
            let out = ivm::aesenc_n_rounds_many(
                std::hint::black_box(&states),
                std::hint::black_box(&rks),
            );
            std::hint::black_box(out)
        })
    });
    c.bench_function("aesenc_n_rounds_cpu_loop", |b| {
        b.iter(|| {
            let mut cur = states.clone();
            for rk in &rks {
                for blk in &mut cur {
                    *blk = ivm::aesenc_impl(*blk, *rk);
                }
            }
            std::hint::black_box(cur)
        })
    });
}

fn bench_aesdec_stream(c: &mut Criterion) {
    let blocks = 16384usize;
    let rounds = 9usize;
    let states = make_blocks(blocks);
    let rks = make_round_keys(rounds);
    c.bench_function("aesdec_n_rounds_many_stream", |b| {
        b.iter(|| {
            let out = ivm::aesdec_n_rounds_many(
                std::hint::black_box(&states),
                std::hint::black_box(&rks),
            );
            std::hint::black_box(out)
        })
    });
    c.bench_function("aesdec_n_rounds_cpu_loop", |b| {
        b.iter(|| {
            let mut cur = states.clone();
            for rk in &rks {
                for blk in &mut cur {
                    *blk = ivm::aesdec_impl(*blk, *rk);
                }
            }
            std::hint::black_box(cur)
        })
    });
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_aesenc_stream(&mut c);
    bench_aesdec_stream(&mut c);
    c.final_summary();
}
