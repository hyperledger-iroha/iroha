//! Microbenchmarks to relate IVM runtime latency with the gas schedule.
//!
//! For each selected instruction, this bench:
//! - Builds a tiny program that repeats the instruction `N` times then HALTs.
//! - Runs the program in the VM and measures elapsed time.
//! - Reads gas consumed from the VM and prints `ns/op`, `gas/op`, and `ns/gas`.
//!
//! Use output to calibrate a target `ns_per_gas` scalar for your baseline CPU and
//! to tune `ivm_gas_limit_per_block` for a desired block time (e.g., ~200 ms).

use criterion::{BatchSize, Criterion};
use ivm::{IVM, encoding, instruction};

// Assemble header + code (mode=0, max_cycles=0, abi=1) — copied from tests/common.rs
fn assemble(code: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"IVM\0");
    v.extend_from_slice(&[1, 0, 0, 0]); // version_major=1, minor=0, mode=0, vector=0(auto)
    v.extend_from_slice(&0u64.to_le_bytes()); // max_cycles = 0 (unspecified)
    v.push(1); // abi_version = 1 (baseline)
    v.extend_from_slice(code);
    v
}

fn program_for_repeated(instr: u32, reps: usize) -> Vec<u8> {
    let mut code = Vec::with_capacity(4 * (reps + 1));
    for _ in 0..reps {
        code.extend_from_slice(&instr.to_le_bytes());
    }
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    assemble(&code)
}

fn bench_instr(c: &mut Criterion, name: &str, instr: u32, reps: usize) {
    let code = program_for_repeated(instr, reps);
    let mut group = c.benchmark_group("ivm-gas-cal");
    group.bench_function(name, |b| {
        b.iter_batched(
            || {
                let mut vm = IVM::new(1_000_000_000);
                vm.load_program(&code).expect("load program");
                // Seed registers used by our wide templates (rs1=1, rs2=2) to
                // avoid traps like divide-by-zero in DIVU during warmup.
                vm.registers.set(1, 123);
                vm.registers.set(2, 7);
                vm
            },
            |mut vm| {
                let start_remaining = vm.remaining_gas();
                vm.run().expect("run");
                let used = start_remaining - vm.remaining_gas();
                let _gas_per_op = used as f64 / reps as f64;
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

fn run_benchmarks(c: &mut Criterion) {
    // Wide arithmetic with small register indices (rd=3, rs1=1, rs2=2)
    let add = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 3, 1, 2);
    let mul = encoding::wide::encode_rr(instruction::wide::arithmetic::MUL, 3, 1, 2);
    let divu = encoding::wide::encode_rr(instruction::wide::arithmetic::DIVU, 3, 1, 2);

    // Repeat count large enough to amortize overhead
    let reps = 50_000;
    bench_instr(c, "ADD", add, reps);
    bench_instr(c, "MUL", mul, reps);
    bench_instr(c, "DIVU", divu, reps);

    // You can extend with more: logic ops, shifts, branches, loads/stores, vector ops, crypto ops.
}

/// Entry point for the benchmark binary.
fn main() {
    // Silence ASCII banner and feature selection in benches.
    ivm::set_banner_enabled(false);
    let mut c = Criterion::default().configure_from_args();
    run_benchmarks(&mut c);
    c.final_summary();
}
