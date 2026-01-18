use ivm::{IVM, ProgramMetadata, encoding};

fn simple_prog(nops: usize) -> Vec<u8> {
    let mut bytes = ProgramMetadata::default().encode();
    for _ in 0..nops {
        let addi = ivm::kotodama::compiler::encode_addi(1, 1, 0)
            .expect("encode addi");
        bytes.extend_from_slice(&addi.to_le_bytes());
    }
    bytes.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    bytes
}

#[test]
fn predecode_counters_increase_via_vm_run() {
    let prog = simple_prog(32);
    // First run on fresh VM
    let mut vm1 = IVM::new(1000);
    vm1.load_program(&prog).unwrap();
    let _ = vm1.run();
    let (h1, m1, e1) = ivm::ivm_cache::global_counters();
    // Second run on a new VM should increase hits
    let mut vm2 = IVM::new(1000);
    vm2.load_program(&prog).unwrap();
    let _ = vm2.run();
    let (h2, m2, e2) = ivm::ivm_cache::global_counters();
    assert!(h2 > h1, "expected predecode cache hits to increase");
    assert!(m2 >= m1, "misses monotonic");
    assert!(e2 >= e1, "evictions monotonic");
}
