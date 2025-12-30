use ivm::{IVM, encoding, kotodama::wide};
mod common;

fn program_shift_ops() -> Vec<u8> {
    // Build: SLL x5 = x1 << x2; SRL x6 = x3 >> x2; SRA x7 = x4 >>> x2; HALT
    let sll = wide::encode_sll(5, 1, 2);
    let srl = wide::encode_srl(6, 3, 2);
    let sra = wide::encode_sra(7, 4, 2);
    let halt = encoding::wide::encode_halt();
    let mut code = Vec::new();
    code.extend_from_slice(&sll.to_le_bytes());
    code.extend_from_slice(&srl.to_le_bytes());
    code.extend_from_slice(&sra.to_le_bytes());
    code.extend_from_slice(&halt.to_le_bytes());
    common::assemble(&code)
}

#[test]
fn shift_ops_parallel_vs_sequential_match() {
    // Inputs
    let mut vm_ilp = IVM::new(10_000);
    vm_ilp.set_register(1, 1);
    vm_ilp.set_register(2, 4);
    vm_ilp.set_register(3, 0x80);
    vm_ilp.set_register(4, (-8i64) as u64);
    let prog = program_shift_ops();
    vm_ilp.load_program(&prog).unwrap();
    // ILP on (max_cycles == 0)
    vm_ilp.run().unwrap();
    let r5_ilp = vm_ilp.register(5);
    let r6_ilp = vm_ilp.register(6);
    let r7_ilp = vm_ilp.register(7);

    // Sequential path (disable ILP by setting max_cycles)
    let mut vm_seq = IVM::new(10_000);
    vm_seq.set_register(1, 1);
    vm_seq.set_register(2, 4);
    vm_seq.set_register(3, 0x80);
    vm_seq.set_register(4, (-8i64) as u64);
    let mut prog_seq = prog.clone();
    // bump max_cycles in header to non-zero to disable ILP
    let max_cycles = 1024u64.to_le_bytes();
    prog_seq[8..16].copy_from_slice(&max_cycles);
    vm_seq.load_program(&prog_seq).unwrap();
    vm_seq.run().unwrap();

    assert_eq!(r5_ilp, vm_seq.register(5));
    assert_eq!(r6_ilp, vm_seq.register(6));
    assert_eq!(r7_ilp, vm_seq.register(7));
    // Basic value smoke check
    assert_eq!(r5_ilp, 16);
    assert_eq!(r6_ilp, 0x8);
    assert_eq!(r7_ilp, 0xFFFF_FFFF_FFFF_FFFF);
}
