//! Bench full AES-128 encrypt/decrypt using pre-expanded key schedules.
use criterion::Criterion;

fn make_blocks(n: usize) -> Vec<[u8; 16]> {
    let mut v = vec![[0u8; 16]; n];
    for (i, blk) in v.iter_mut().enumerate().take(n) {
        blk[0] = (i & 0xff) as u8;
        blk[1] = ((i >> 8) & 0xff) as u8;
    }
    v
}

fn make_key() -> [u8; 16] {
    [
        0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f,
        0x3c,
    ]
}

fn bench_aes128_encrypt(c: &mut Criterion) {
    let blocks = 16384usize; // 16k
    let states = make_blocks(blocks);
    let key = make_key();
    let rks = ivm::aes128_expand_key(key);
    c.bench_function("aes128_encrypt_many", |b| {
        b.iter(|| {
            let out =
                ivm::aes128_encrypt_many(std::hint::black_box(&states), std::hint::black_box(&rks));
            std::hint::black_box(out)
        })
    });
}

fn bench_aes128_decrypt(c: &mut Criterion) {
    let blocks = 16384usize; // 16k
    let states = make_blocks(blocks);
    let key = make_key();
    let rks = ivm::aes128_expand_key(key);
    let enc = ivm::aes128_encrypt_many(&states, &rks);
    c.bench_function("aes128_decrypt_many", |b| {
        b.iter(|| {
            let out =
                ivm::aes128_decrypt_many(std::hint::black_box(&enc), std::hint::black_box(&rks));
            std::hint::black_box(out)
        })
    });
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_aes128_encrypt(&mut c);
    bench_aes128_decrypt(&mut c);
    c.final_summary();
}
