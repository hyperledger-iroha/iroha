//! Measure wall-clock time per gas unit for representative admitted opcodes.
use ivm::{IVM, ProgramMetadata, encoding, instruction};
use std::time::Instant;

fn program_for_repeated(instr: u32, reps: usize) -> Vec<u8> {
    let mut program = ProgramMetadata::default().encode();
    for _ in 0..reps {
        program.extend_from_slice(&instr.to_le_bytes());
    }
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    program
}

fn run_case(name: &str, instr: u32, reps: usize, rounds: usize) {
    let program = program_for_repeated(instr, reps);
    let mut total_ns = 0u128;
    let mut total_gas = 0u128;
    // One untimed round warms the prepared-program cache.
    for round in 0..=rounds {
        let mut vm = IVM::new(1_000_000_000);
        vm.load_program(&program).expect("load");
        // Nonzero operands avoid division traps; r4 addresses an aligned heap word.
        vm.set_register(1, 123);
        vm.set_register(2, 7);
        vm.set_register(4, ivm::Memory::HEAP_START);
        let start_gas = vm.remaining_gas();
        let started = Instant::now();
        vm.run().expect("run");
        if round != 0 {
            total_ns += started.elapsed().as_nanos();
            total_gas += u128::from(start_gas - vm.remaining_gas());
        }
    }
    let avg_ns = total_ns as f64 / rounds as f64;
    let avg_gas = total_gas as f64 / rounds as f64;
    let ns_per_gas = if avg_gas > 0.0 {
        avg_ns / avg_gas
    } else {
        f64::INFINITY
    };
    println!(
        "{name}: reps={reps}, rounds={rounds}, ns/op={:.2}, gas/op={:.3}, ns/gas={ns_per_gas:.2}",
        avg_ns / reps as f64,
        avg_gas / reps as f64,
    );
}

fn main() {
    use instruction::wide::{arithmetic, memory};
    let reps = 200_000;
    let rounds = 5;
    for (name, instr) in [
        ("ADD", encoding::wide::encode_rr(arithmetic::ADD, 3, 1, 2)),
        ("MUL", encoding::wide::encode_rr(arithmetic::MUL, 3, 1, 2)),
        ("DIV", encoding::wide::encode_rr(arithmetic::DIV, 3, 1, 2)),
        ("GCD", encoding::wide::encode_rr(arithmetic::GCD, 3, 1, 2)),
        (
            "LOAD64",
            encoding::wide::encode_load(memory::LOAD64, 3, 4, 0),
        ),
        (
            "STORE64",
            encoding::wide::encode_store(memory::STORE64, 4, 1, 0),
        ),
    ] {
        run_case(name, instr, reps, rounds);
    }
}
