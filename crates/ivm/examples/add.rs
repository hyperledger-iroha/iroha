//! Minimal example: encode and execute a simple add program on IVM.
use ivm::{IVM, encoding, instruction};

fn main() {
    // Initialize the VM
    let mut vm = IVM::new(u64::MAX);
    // Prepare registers with values to add
    let a: u64 = 2;
    let b: u64 = 3;
    vm.set_register(1, a);
    vm.set_register(2, b);
    // Program: ADD x3 = x1 + x2; HALT using the wide opcode helpers
    let add = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 3, 1, 2).to_le_bytes();
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let mut body = Vec::new();
    body.extend_from_slice(&add);
    body.extend_from_slice(&halt);
    let body = body;
    let mut program = ivm::ProgramMetadata::default().encode();
    program.extend_from_slice(&body);
    vm.load_program(&program).unwrap();
    // Run the VM
    vm.run().expect("VM execution failed");
    let result = vm.register(3);
    println!("{a} + {b} = {result}");
}
