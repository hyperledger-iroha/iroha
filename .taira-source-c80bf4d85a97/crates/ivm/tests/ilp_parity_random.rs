use ivm::{IVM, encoding, instruction};
mod common;

fn wide_rr(op: u8, rd: u8, rs1: u8, rs2: u8) -> u32 {
    encoding::wide::encode_rr(op, rd, rs1, rs2)
}

fn random_block(seed: u64, len: usize) -> Vec<u32> {
    // Simple LCG for deterministic pseudo-randomness
    let mut s = seed;
    let mut out = Vec::with_capacity(len);
    let ops: &[u8] = &[
        instruction::wide::arithmetic::ADD,
        instruction::wide::arithmetic::SUB,
        instruction::wide::arithmetic::AND,
        instruction::wide::arithmetic::OR,
        instruction::wide::arithmetic::XOR,
        instruction::wide::arithmetic::SLL,
        instruction::wide::arithmetic::SRL,
        instruction::wide::arithmetic::SRA,
    ];
    for _ in 0..len {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
        let pick = (s >> 32) as usize % ops.len();
        // choose regs in 1..16 to avoid x0 writes; keep small to increase conflicts
        let r = |shift: u32| -> u8 { ((((s >> shift) as u32) % 15) + 1) as u8 };
        let rd = r(4);
        let rs1 = r(8);
        let rs2 = r(12);
        out.push(wide_rr(ops[pick], rd, rs1, rs2));
    }
    out
}

#[test]
fn ilp_matches_sequential_for_random_blocks() {
    // Generate a handful of random ALU/shift-only programs and compare ILP vs sequential
    let seeds = [
        0xC0FFEEu64,
        0xDEADBEEFu64,
        0xFACEB00Cu64,
        0x0123456789ABCDEFu64,
    ];
    for (i, &seed) in seeds.iter().enumerate() {
        let words = random_block(seed, 32 + i * 3);
        let mut code = Vec::new();
        for w in &words {
            code.extend_from_slice(&w.to_le_bytes());
        }
        code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
        let prog = common::assemble(&code);

        // ILP run (max_cycles == 0)
        let mut vm_ilp = IVM::new(50_000);
        // Seed some registers deterministically
        for r in 1..16 {
            vm_ilp.set_register(
                r,
                seed.wrapping_mul((r as u64).wrapping_mul(0x9E37_79B97F4A7C15)),
            );
        }
        vm_ilp.load_program(&prog).unwrap();
        vm_ilp.run().unwrap();
        let regs_ilp = (1..32).map(|r| vm_ilp.register(r)).collect::<Vec<_>>();

        // Sequential run (disable ILP by setting max_cycles)
        let mut prog_seq = prog.clone();
        prog_seq[8..16].copy_from_slice(&1024u64.to_le_bytes());
        let mut vm_seq = IVM::new(50_000);
        for r in 1..16 {
            vm_seq.set_register(
                r,
                seed.wrapping_mul((r as u64).wrapping_mul(0x9E37_79B97F4A7C15)),
            );
        }
        vm_seq.load_program(&prog_seq).unwrap();
        vm_seq.run().unwrap();
        let regs_seq = (1..32).map(|r| vm_seq.register(r)).collect::<Vec<_>>();

        assert_eq!(regs_ilp, regs_seq, "ILP parity mismatch for seed {seed:#x}");
    }
}
