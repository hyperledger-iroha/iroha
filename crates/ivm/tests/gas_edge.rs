use ivm::{IVM, VMError};

fn halt16() -> [u8; 2] {
    0u16.to_le_bytes()
}
fn add16(rd: u16, rs: u16, rt: u16) -> [u8; 2] {
    let word = ((0x1u16) << 12) | ((rd & 0xf) << 8) | ((rs & 0xf) << 4) | (rt & 0xf);
    word.to_le_bytes()
}

#[test]
fn out_of_gas_traps() {
    // Program: two simple instructions so with gas=1 execution must trap.
    let mut code = Vec::new();
    code.extend_from_slice(&add16(3, 0, 0));
    code.extend_from_slice(&halt16());
    let mut vm = IVM::new(1);
    vm.load_code(&code).unwrap();
    match vm.run_simple() {
        Err(VMError::OutOfGas) => {}
        other => panic!("expected OutOfGas, got {other:?}"),
    }
}
