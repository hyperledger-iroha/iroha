use ivm::{IVM, Memory, encoding, instruction};
mod common;

fn wide_rr(op: u8, rd: u8, rs1: u8, rs2: u8) -> u32 {
    encoding::wide::encode_rr(op, rd, rs1, rs2)
}

fn wide_load(op: u8, rd: u8, base: u8, imm: i8) -> u32 {
    encoding::wide::encode_load(op, rd, base, imm)
}

fn wide_store(op: u8, base: u8, rs: u8, imm: i8) -> u32 {
    encoding::wide::encode_store(op, base, rs, imm)
}

#[test]
fn ilp_matches_seq_for_load_store_disjoint_regions() {
    // Two disjoint bases in HEAP: region A at HEAP_START, region B at HEAP_START + 0x1000
    let base_a = Memory::HEAP_START as i32; // rs1=2
    let base_b = (Memory::HEAP_START + 0x1000) as i32; // rs1=3

    // Prologue: seed some values in registers and region B so loads read non-zero
    let mut code = vec![
        // Compute some register values: r4 = r1 + r1, r5 = r2 + r2 ... using ADD
        wide_rr(instruction::wide::arithmetic::ADD, 4, 1, 1),
        wide_rr(instruction::wide::arithmetic::ADD, 5, 2, 2),
        wide_rr(instruction::wide::arithmetic::ADD, 6, 3, 3),
        wide_rr(instruction::wide::arithmetic::ADD, 7, 4, 4),
        // Pre-store some values into region B at offsets 0, 8, 16, 24
        wide_store(instruction::wide::memory::STORE64, 3, 4, 0), // SD [r3+0] = r4
        wide_store(instruction::wide::memory::STORE64, 3, 5, 8), // SD [r3+8] = r5
        wide_store(instruction::wide::memory::STORE64, 3, 6, 16), // SD [r3+16] = r6
        wide_store(instruction::wide::memory::STORE64, 3, 7, 24), // SD [r3+24] = r7
    ];

    // Main block: intermix stores to region A and loads from region B (disjoint addresses)
    // SD [r2+0] = r4; LD r10 = [r3+0]
    code.push(wide_store(instruction::wide::memory::STORE64, 2, 4, 0));
    code.push(wide_load(instruction::wide::memory::LOAD64, 10, 3, 0));
    // SD [r2+8] = r5; LD r11 = [r3+8]
    code.push(wide_store(instruction::wide::memory::STORE64, 2, 5, 8));
    code.push(wide_load(instruction::wide::memory::LOAD64, 11, 3, 8));
    // SD [r2+16] = r6; LD r12 = [r3+16]
    code.push(wide_store(instruction::wide::memory::STORE64, 2, 6, 16));
    code.push(wide_load(instruction::wide::memory::LOAD64, 12, 3, 16));
    // SD [r2+24] = r7; LD r13 = [r3+24]
    code.push(wide_store(instruction::wide::memory::STORE64, 2, 7, 24));
    code.push(wide_load(instruction::wide::memory::LOAD64, 13, 3, 24));

    // HALT
    code.push(encoding::wide::encode_halt());

    // Assemble artifact header + code
    let prog = common::assemble(
        &code
            .into_iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>(),
    );

    // ILP VM
    let mut vm_ilp = IVM::new(10_000);
    vm_ilp.load_program(&prog).unwrap();
    // Seed base registers and small values
    vm_ilp.set_register(1, 1);
    vm_ilp.set_register(2, base_a as u64);
    vm_ilp.set_register(3, base_b as u64);
    vm_ilp.run().unwrap();
    let regs_ilp = (1..20).map(|r| vm_ilp.register(r)).collect::<Vec<_>>();

    // Sequential VM (disable ILP)
    let mut prog_seq = prog.clone();
    prog_seq[8..16].copy_from_slice(&1024u64.to_le_bytes());
    let mut vm_seq = IVM::new(10_000);
    vm_seq.load_program(&prog_seq).unwrap();
    vm_seq.set_register(1, 1);
    vm_seq.set_register(2, base_a as u64);
    vm_seq.set_register(3, base_b as u64);
    vm_seq.run().unwrap();
    let regs_seq = (1..20).map(|r| vm_seq.register(r)).collect::<Vec<_>>();

    assert_eq!(
        regs_ilp, regs_seq,
        "ILP parity mismatch for disjoint load/store regions"
    );
}

#[test]
fn ilp_matches_seq_for_step_aligned_interleave() {
    // Write to region A at 0,8,16,... and read from region B at 8,16,24,... step-aligned.
    let base_a = Memory::HEAP_START as i32; // rs1=2
    let base_b = (Memory::HEAP_START + 0x2000) as i32; // rs1=3

    let mut code = Vec::new();
    // Prime region B with a pattern
    for k in 0..8 {
        // r4 = r1 + k; SD [r3 + 8*k] = r4
        code.push(wide_rr(instruction::wide::arithmetic::ADD, 4, 1, 1));
        code.push(wide_store(
            instruction::wide::memory::STORE64,
            3,
            4,
            (8 * k) as i8,
        ));
    }
    // Interleave stores to A at 8*k and loads from B at 8*(k+1)
    for k in 0..8 {
        code.push(wide_store(
            instruction::wide::memory::STORE64,
            2,
            4,
            (8 * k) as i8,
        ));
        code.push(wide_load(
            instruction::wide::memory::LOAD64,
            10 + k as u8,
            3,
            (8 * (k + 1)) as i8,
        ));
    }
    code.push(encoding::wide::encode_halt());

    let prog = common::assemble(
        &code
            .into_iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>(),
    );

    let mut vm_ilp = IVM::new(10_000);
    vm_ilp.load_program(&prog).unwrap();
    vm_ilp.set_register(1, 7);
    vm_ilp.set_register(2, base_a as u64);
    vm_ilp.set_register(3, base_b as u64);
    vm_ilp.run().unwrap();
    let regs_ilp = (1..24).map(|r| vm_ilp.register(r)).collect::<Vec<_>>();

    let mut prog_seq = prog.clone();
    prog_seq[8..16].copy_from_slice(&2048u64.to_le_bytes());
    let mut vm_seq = IVM::new(10_000);
    vm_seq.load_program(&prog_seq).unwrap();
    vm_seq.set_register(1, 7);
    vm_seq.set_register(2, base_a as u64);
    vm_seq.set_register(3, base_b as u64);
    vm_seq.run().unwrap();
    let regs_seq = (1..24).map(|r| vm_seq.register(r)).collect::<Vec<_>>();

    assert_eq!(
        regs_ilp, regs_seq,
        "ILP parity mismatch for step-aligned interleave"
    );
}
