//! VM-level AESENC stress benchmark: builds a program with repeated AESENC rounds.
use criterion::Criterion;
use ivm::{IVM, ProgramMetadata, encoding, instruction, ivm_mode};

fn assemble(mut body: Vec<u32>) -> Vec<u8> {
    let mut metadata = ProgramMetadata::default();
    metadata.mode |= ivm_mode::VECTOR;
    let mut v = metadata.encode();
    // Encode body as little-endian u32 words
    for w in body.drain(..) {
        v.extend_from_slice(&w.to_le_bytes());
    }
    v
}

fn program_repeated_aesenc(reps: usize) -> Vec<u8> {
    let mut code = Vec::with_capacity(reps + 1);
    let instr = encoding::wide::encode_rr(instruction::wide::crypto::AESENC, 5, 1, 3);
    for _ in 0..reps {
        code.push(instr);
    }
    code.push(encoding::wide::encode_halt());
    assemble(code)
}

fn bench_vm_aesenc(c: &mut Criterion) {
    let reps = 200_000usize;
    let prog = program_repeated_aesenc(reps);
    c.bench_function("vm_aesenc_repeated", |b| {
        b.iter(|| {
            let mut vm = IVM::new(1_000_000_000);
            vm.load_program(&prog).expect("load");
            // Set state in x1..x2, round key in x3..x4
            let state_lo: u64 = 0x7766554433221100;
            let state_hi: u64 = 0xffeeddccbbaa9988;
            let rk_lo: u64 = 0xc971150f59e8d947;
            let rk_hi: u64 = 0x98_67_7f_af_d6_ad_b7_0c; // just an example pattern
            vm.set_register(1, state_lo);
            vm.set_register(2, state_hi);
            vm.set_register(3, rk_lo);
            vm.set_register(4, rk_hi);
            vm.run().expect("run");
        })
    });
}

fn program_repeated_aesdec(reps: usize) -> Vec<u8> {
    let mut code = Vec::with_capacity(reps + 1);
    let instr = encoding::wide::encode_rr(instruction::wide::crypto::AESDEC, 5, 1, 3);
    for _ in 0..reps {
        code.push(instr);
    }
    code.push(encoding::wide::encode_halt());
    assemble(code)
}

fn bench_vm_aesdec(c: &mut Criterion) {
    let reps = 200_000usize;
    let prog = program_repeated_aesdec(reps);
    c.bench_function("vm_aesdec_repeated", |b| {
        b.iter(|| {
            let mut vm = IVM::new(1_000_000_000);
            vm.load_program(&prog).expect("load");
            let state_lo: u64 = 0x7766554433221100;
            let state_hi: u64 = 0xffeeddccbbaa9988;
            let rk_lo: u64 = 0xc971150f59e8d947;
            let rk_hi: u64 = 0x98_67_7f_af_d6_ad_b7_0c;
            vm.set_register(1, state_lo);
            vm.set_register(2, state_hi);
            vm.set_register(3, rk_lo);
            vm.set_register(4, rk_hi);
            vm.run().expect("run");
        })
    });
}

fn program_mixed_aes(reps: usize) -> Vec<u8> {
    let mut code = Vec::with_capacity(2 * reps + 1);
    let enc = encoding::wide::encode_rr(instruction::wide::crypto::AESENC, 5, 1, 3);
    let dec = encoding::wide::encode_rr(instruction::wide::crypto::AESDEC, 5, 1, 3);
    for _ in 0..reps {
        code.push(enc);
        code.push(dec);
    }
    code.push(encoding::wide::encode_halt());
    assemble(code)
}

fn bench_vm_aes_mix(c: &mut Criterion) {
    let reps = 100_000usize; // 200k ops total (enc+dec)
    let prog = program_mixed_aes(reps);
    c.bench_function("vm_aes_mixed_enc_dec", |b| {
        b.iter(|| {
            let mut vm = IVM::new(1_000_000_000);
            vm.load_program(&prog).expect("load");
            let state_lo: u64 = 0x7766554433221100;
            let state_hi: u64 = 0xffeeddccbbaa9988;
            let rk_lo: u64 = 0xc971150f59e8d947;
            let rk_hi: u64 = 0x98_67_7f_af_d6_ad_b7_0c;
            vm.set_register(1, state_lo);
            vm.set_register(2, state_hi);
            vm.set_register(3, rk_lo);
            vm.set_register(4, rk_hi);
            vm.run().expect("run");
        })
    });
}

fn main() {
    ivm::set_banner_enabled(false);
    let mut c = Criterion::default().configure_from_args();
    bench_vm_aesenc(&mut c);
    bench_vm_aesdec(&mut c);
    bench_vm_aes_mix(&mut c);
    c.final_summary();
}
