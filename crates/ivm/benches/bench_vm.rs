//! Benchmarks for core IVM VM operations and Merkle utilities.
use criterion::{BatchSize, Criterion};
use ivm::{ByteMerkleTree, IVM, ProgramMetadata, encoding};
// The upper C10 registers above the call ABI are scratch, except for the stack pointer.
const SCRATCH_REGISTERS: std::ops::Range<usize> = 16..ivm::instruction::wide::REGISTER_COUNT - 1;

#[inline]
fn encode_addi_word(rd: u8, rs1: u8, imm: i16) -> u32 {
    kotodama_lang::compiler::encode_addi(rd, rs1, imm).expect("encode addi")
}
fn straight_line_program(instructions: usize) -> Vec<u8> {
    let mut bytes = ProgramMetadata::default().encode();
    for idx in 0..instructions {
        let rd = (SCRATCH_REGISTERS.start + idx % SCRATCH_REGISTERS.len()) as u8;
        let add = encode_addi_word(rd, rd, 1);
        bytes.extend_from_slice(&add.to_le_bytes());
    }
    bytes.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    bytes
}
fn bench_predecoded_runs(c: &mut Criterion) {
    use kotodama_lang::compiler::{Compiler, CompilerOptions};
    for (name, options) in [
        ("baseline", CompilerOptions::default()),
        ("c10", CompilerOptions::c10()),
    ] {
        let profile = options.target_profile().unwrap();
        let bytes = Compiler::new_with_options(options)
            .compile_source("誓約 Bench { view fn answer() authorize(anyone) -> bool { true } }")
            .unwrap();
        let budget = iroha_allocation::AllocationBudget::new(16 * 1024 * 1024);
        let prepare = || {
            ivm::prepare_contract_with_memory_budget_for_profile(&bytes, &budget, profile).unwrap()
        };
        let contract = prepare();
        let loaded = || {
            let mut vm = IVM::new(u64::MAX);
            vm.load_prepared(&contract).unwrap();
            vm.select_entrypoint("answer").unwrap();
            vm
        };
        let mut check = loaded();
        check.run().unwrap();
        assert_eq!(check.public_call_result_word(0).unwrap(), 1);
        let mut group = c.benchmark_group(format!("ivm_{name}"));
        group.bench_function("cold_contract_preparation", |b| {
            b.iter_batched(|| (), |()| prepare(), BatchSize::PerIteration)
        });
        let code = &bytes[ProgramMetadata::parse_for_profile(&bytes, profile)
            .unwrap()
            .code_offset..];
        let mut cache = ivm::ivm_cache::IvmCache::new(1);
        let primed = cache.get_or_predecode(code).unwrap();
        assert_eq!(
            primed.as_ptr(),
            cache.get_or_predecode(code).unwrap().as_ptr()
        );
        group.bench_function("decoded_cache_hit", |b| {
            b.iter(|| cache.get_or_predecode(code).unwrap())
        });
        group.bench_function("prepared_execution", |b| {
            b.iter_batched_ref(loaded, |vm| vm.run().unwrap(), BatchSize::PerIteration)
        });
        group.finish();
    }
}
fn bench_straight_line_runs(c: &mut Criterion) {
    for (name, instructions) in [
        ("ivm_run_straight_line_simple_8", 8usize),
        ("ivm_run_straight_line_simple_64", 64usize),
    ] {
        let program = straight_line_program(instructions);
        let mut check = IVM::new(u64::MAX);
        check.load_program(&program).unwrap();
        check.run().unwrap();
        for (offset, register) in SCRATCH_REGISTERS.enumerate() {
            let count = instructions / SCRATCH_REGISTERS.len()
                + usize::from(offset < instructions % SCRATCH_REGISTERS.len());
            assert_eq!(check.register(register), count as u64);
        }
        c.bench_function(name, |b| {
            b.iter_batched_ref(
                || {
                    let mut vm = IVM::new(u64::MAX);
                    vm.load_program(&program).unwrap();
                    vm
                },
                |vm| vm.run().unwrap(),
                BatchSize::PerIteration,
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
