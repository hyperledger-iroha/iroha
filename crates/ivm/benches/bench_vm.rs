//! Benchmarks for core IVM VM operations and Merkle utilities.
use criterion::{BatchSize, Criterion};
use ivm::{ByteMerkleTree, IVM, ProgramMetadata, encoding};
#[inline]
fn encode_addi_word(rd: u8, rs1: u8, imm: i16) -> u32 {
    ivm::kotodama::compiler::encode_addi(rd, rs1, imm).expect("encode addi")
}
fn predecoded_program() -> Vec<u8> {
    let mut bytes = ProgramMetadata::default().encode();
    let add = encode_addi_word(1, 0, 1);
    bytes.extend_from_slice(&add.to_le_bytes());
    bytes.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    bytes
}
fn straight_line_program(instructions: usize) -> Vec<u8> {
    let mut bytes = ProgramMetadata::default().encode();
    for idx in 0..instructions {
        let rd = ((idx % 64) + 1) as u8;
        let add = encode_addi_word(rd, rd, 1);
        bytes.extend_from_slice(&add.to_le_bytes());
    }
    bytes.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    bytes
}
fn bench_predecoded_runs(c: &mut Criterion) {
    let program = predecoded_program();
    c.bench_function("ivm_run_cold_decode", |b| {
        b.iter_batched(
            || {
                let prog = program.clone();
                let mut vm = IVM::new(u64::MAX);
                vm.load_program(&prog).unwrap();
                vm
            },
            |mut vm| {
                vm.run().unwrap();
            },
            BatchSize::SmallInput,
        );
    });
    c.bench_function("ivm_run_warm_predecoded", |b| {
        b.iter_batched(
            || {
                let prog = program.clone();
                let mut vm = IVM::new(u64::MAX);
                vm.load_program(&prog).unwrap();
                vm.run().unwrap();
                vm.load_program(&prog).unwrap();
                vm
            },
            |mut vm| {
                vm.run().unwrap();
            },
            BatchSize::SmallInput,
        );
    });
}
fn bench_straight_line_runs(c: &mut Criterion) {
    for (name, instructions) in [
        ("ivm_run_straight_line_simple_8", 8usize),
        ("ivm_run_straight_line_simple_64", 64usize),
    ] {
        let program = straight_line_program(instructions);
        c.bench_function(name, |b| {
            b.iter_batched(
                || {
                    let prog = program.clone();
                    let mut vm = IVM::new(u64::MAX);
                    vm.load_program(&prog).unwrap();
                    vm
                },
                |mut vm| {
                    vm.run().unwrap();
                },
                BatchSize::SmallInput,
            );
        });
    }
}
fn bench_merkle_build(c: &mut Criterion) {
    let data = vec![0u8; 32 * 1024];
    c.bench_function("byte_tree_from_bytes", |b| {
        b.iter(|| {
            let _ = ByteMerkleTree::from_bytes(&data, 32).unwrap();
        })
    });
}
fn bench_merkle_update(c: &mut Criterion) {
    let tree = ByteMerkleTree::new(1024, 32).unwrap();
    let chunk = [1u8; 32];
    c.bench_function("byte_tree_update_leaf", |b| {
        b.iter(|| {
            tree.update_leaf(0, &chunk).unwrap();
        })
    });
}
/// Entry point for the benchmark binary.
fn main() {
    // Silence ASCII banner and feature selection in benches.
    ivm::set_banner_enabled(false);
    let mut c = Criterion::default().configure_from_args();
    bench_predecoded_runs(&mut c);
    bench_straight_line_runs(&mut c);
    bench_merkle_build(&mut c);
    bench_merkle_update(&mut c);
    c.final_summary();
}
