//! Benchmarks for core IVM VM operations and Merkle utilities.
use criterion::{BatchSize, Criterion};
use ivm::{
    ByteMerkleTree, IVM, ProgramMetadata, encoding, instruction,
    kotodama::wide as kwide,
    parallel::{Block, State, StateAccessSet, Transaction},
};

fn loop_program() -> Vec<u8> {
    let mut prog = ProgramMetadata::default().encode();
    prog.extend_from_slice(&kwide::encode_addi(1, 0, 1).to_le_bytes());
    let branch = kwide::encode_branch_checked(instruction::wide::control::BLT, 1, 2, -1)
        .expect("loop branch");
    prog.extend_from_slice(&branch.to_le_bytes());
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    prog
}

fn loop_transaction(code: &[u8]) -> Transaction {
    Transaction {
        code: code.to_vec(),
        gas_limit: 0,
        access: StateAccessSet::new(),
    }
}

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

fn bench_loop(c: &mut Criterion) {
    let program = loop_program();
    let tx = loop_transaction(&program);
    let block = Block {
        transactions: vec![tx; 100],
    };
    let cores = num_cpus::get_physical();
    let mut ivm = IVM::new_with_options(Some(cores), State::new(), u64::MAX);
    c.bench_function("parallel_loop_block_100", |b| {
        b.iter(|| {
            ivm.execute_block(block.clone());
        })
    });
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

fn bench_merkle_build(c: &mut Criterion) {
    let data = vec![0u8; 32 * 1024];
    c.bench_function("byte_tree_from_bytes", |b| {
        b.iter(|| {
            let _ = ByteMerkleTree::from_bytes(&data, 32);
        })
    });
}

fn bench_merkle_update(c: &mut Criterion) {
    let tree = ByteMerkleTree::new(1024, 32);
    let chunk = [1u8; 32];
    c.bench_function("byte_tree_update_leaf", |b| {
        b.iter(|| {
            tree.update_leaf(0, &chunk);
        })
    });
}

/// Entry point for the benchmark binary.
fn main() {
    // Silence ASCII banner and feature selection in benches.
    ivm::set_banner_enabled(false);
    let mut c = Criterion::default().configure_from_args();
    bench_loop(&mut c);
    bench_predecoded_runs(&mut c);
    bench_merkle_build(&mut c);
    bench_merkle_update(&mut c);
    c.final_summary();
}
