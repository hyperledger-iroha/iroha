use std::time::Instant;

use ivm::{IVM, ProgramMetadata, encoding, instruction, ivm_mode, kotodama::wide as kwide};

fn assemble(code: &[u8]) -> Vec<u8> {
    let mut meta = ProgramMetadata::default();
    meta.mode |= ivm_mode::VECTOR;
    let mut buf = meta.encode();
    buf.extend_from_slice(code);
    buf
}

fn program_for_repeated(instr: u32, reps: usize) -> Vec<u8> {
    let mut code = Vec::with_capacity(4 * (reps + 1));
    for _ in 0..reps {
        code.extend_from_slice(&instr.to_le_bytes());
    }
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    assemble(&code)
}

fn run_case(name: &str, instr: u32, reps: usize, rounds: usize) {
    // Warm-up
    {
        let code = program_for_repeated(instr, 1_000);
        let mut vm = IVM::new(1_000_000_000);
        vm.load_program(&code).expect("load");
        let _ = vm.run();
    }

    let mut total_ns = 0u128;
    let mut total_gas = 0u128;
    for _ in 0..rounds {
        let code = program_for_repeated(instr, reps);
        let mut vm = IVM::new(1_000_000_000);
        vm.load_program(&code).expect("load");
        // Initialize registers used by R-type ops to avoid div-by-zero etc.
        vm.set_register(1, 123);
        vm.set_register(2, 7);
        let start_gas = vm.remaining_gas();
        let t0 = Instant::now();
        vm.run().expect("run");
        let dur = t0.elapsed();
        let used = (start_gas - vm.remaining_gas()) as u128;
        total_ns += dur.as_nanos();
        total_gas += used;
    }
    let avg_ns = total_ns as f64 / rounds as f64;
    let avg_gas = total_gas as f64 / rounds as f64;
    let gas_per_op = avg_gas / reps as f64;
    let ns_per_op = avg_ns / reps as f64;
    let ns_per_gas = if avg_gas > 0.0 {
        avg_ns / avg_gas
    } else {
        f64::INFINITY
    };
    println!(
        "{name}: reps={reps}, rounds={rounds}, ns/op={ns_per_op:.2}, gas/op={gas_per_op:.3}, ns/gas={ns_per_gas:.2}"
    );
}

fn main() {
    // Ensure acceleration policy is applied consistently (defaults enable all backends
    // and auto-detect hardware; golden self-tests preserve determinism).
    ivm::set_acceleration_config(ivm::AccelerationConfig {
        enable_simd: true,
        enable_metal: true,
        enable_cuda: true,
        max_gpus: None,
        merkle_min_leaves_gpu: None,
        merkle_min_leaves_metal: None,
        merkle_min_leaves_cuda: None,
        prefer_cpu_sha2_max_leaves_aarch64: None,
        prefer_cpu_sha2_max_leaves_x86: None,
    });
    // Select representative ops
    // R-type ADD (classic 0x33: funct3=0x0, funct7=0x00)
    let add = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 3, 1, 2);
    // Vector ADD32 in the wide space
    let vadd32 = encoding::wide::encode_rr(instruction::wide::crypto::VADD32, 1, 2, 3);
    // SHA256BLOCK (wide opcode 0x80) uses vector registers in rd/rs1
    let sha256 = encoding::wide::encode_rr(instruction::wide::crypto::SHA256BLOCK, 1, 2, 0);
    // Use a 128-bit load to mimic vector fetches
    let ldv = kwide::encode_load128(4, 1, 2);

    let reps = 200_000usize;
    let rounds = 5usize;
    run_case("ADD", add, reps, rounds);

    // Bench VADD32 with zero-initialized vectors (safe baseline)
    run_case("VADD32", vadd32, reps, rounds);

    // Bench LOAD_VECTOR from HEAP_START (address in x2)
    {
        let code = program_for_repeated(ldv, reps);
        let mut total_ns = 0u128;
        let mut total_gas = 0u128;
        for _ in 0..rounds {
            let mut vm = IVM::new(1_000_000_000);
            let base = ivm::Memory::HEAP_START;
            let buf = [0u8; 16];
            vm.memory.store_bytes(base, &buf).expect("store bytes");
            vm.load_program(&code).expect("load");
            vm.set_register(4, base);
            let start_gas = vm.remaining_gas();
            let t0 = Instant::now();
            vm.run().expect("run");
            let dur = t0.elapsed();
            let used = (start_gas - vm.remaining_gas()) as u128;
            total_ns += dur.as_nanos();
            total_gas += used;
        }
        let avg_ns = total_ns as f64 / rounds as f64;
        let avg_gas = total_gas as f64 / rounds as f64;
        let gas_per_op = avg_gas / reps as f64;
        let ns_per_op = avg_ns / reps as f64;
        let ns_per_gas = if avg_gas > 0.0 {
            avg_ns / avg_gas
        } else {
            f64::INFINITY
        };
        println!(
            "LOAD_VECTOR: reps={reps}, rounds={rounds}, ns/op={ns_per_op:.2}, gas/op={gas_per_op:.3}, ns/gas={ns_per_gas:.2}"
        );
    }

    // Bench SHA256BLOCK over a zero block at HEAP_START (x2 = addr)
    {
        let code = program_for_repeated(sha256, reps);
        let mut total_ns = 0u128;
        let mut total_gas = 0u128;
        for _ in 0..rounds {
            let mut vm = IVM::new(1_000_000_000);
            let base = ivm::Memory::HEAP_START;
            let buf = [0u8; 64];
            vm.memory.store_bytes(base, &buf).expect("store bytes");
            vm.load_program(&code).expect("load");
            vm.set_register(2, base);
            let start_gas = vm.remaining_gas();
            let t0 = Instant::now();
            vm.run().expect("run");
            let dur = t0.elapsed();
            let used = (start_gas - vm.remaining_gas()) as u128;
            total_ns += dur.as_nanos();
            total_gas += used;
        }
        let avg_ns = total_ns as f64 / rounds as f64;
        let avg_gas = total_gas as f64 / rounds as f64;
        let gas_per_op = avg_gas / reps as f64;
        let ns_per_op = avg_ns / reps as f64;
        let ns_per_gas = if avg_gas > 0.0 {
            avg_ns / avg_gas
        } else {
            f64::INFINITY
        };
        println!(
            "SHA256BLOCK: reps={reps}, rounds={rounds}, ns/op={ns_per_op:.2}, gas/op={gas_per_op:.3}, ns/gas={ns_per_gas:.2}"
        );
    }
}
