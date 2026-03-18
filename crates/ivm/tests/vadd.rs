use ivm::{IVM, ProgramMetadata, VMError, ivm_mode, simple_instruction::Instruction};

fn vector_vm() -> IVM {
    let mut vm = IVM::new(u64::MAX);
    let mut meta = ProgramMetadata::default();
    meta.mode |= ivm_mode::VECTOR;
    meta.abi_version = 1;
    vm.load_program(&meta.encode()).unwrap();
    vm
}

const VECTOR_BASE: usize = 32;

fn set_vector(vm: &mut IVM, reg: usize, values: &[u64]) {
    let stride = vm.vector_length();
    assert_eq!(
        values.len(),
        stride,
        "values should match current vector length"
    );
    let start = VECTOR_BASE + reg * stride;
    for (idx, value) in values.iter().enumerate() {
        vm.set_register(start + idx, *value);
    }
}

fn vector_values(vm: &IVM, reg: usize) -> Vec<u64> {
    let stride = vm.vector_length();
    let start = VECTOR_BASE + reg * stride;
    (0..stride).map(|idx| vm.register(start + idx)).collect()
}

fn set_vector_tags(vm: &mut IVM, reg: usize, tags: &[bool]) {
    let stride = vm.vector_length();
    assert_eq!(
        tags.len(),
        stride,
        "tags should match current vector length"
    );
    let start = VECTOR_BASE + reg * stride;
    for (idx, tag) in tags.iter().enumerate() {
        vm.registers.set_tag(start + idx, *tag);
    }
}

#[test]
fn vadd_basic() {
    let mut vm = vector_vm();
    vm.execute_instruction(Instruction::SetVL { new_vl: 4 })
        .unwrap();
    set_vector(&mut vm, 10, &[1, 2, 3, 4]);
    set_vector(&mut vm, 20, &[10, 20, 30, 40]);
    vm.execute_instruction(Instruction::Vadd {
        rd: 30,
        rs: 10,
        rt: 20,
    })
    .unwrap();
    assert_eq!(vector_values(&vm, 30), vec![11, 22, 33, 44]);
}

#[test]
fn vadd_vl1() {
    let mut vm = vector_vm();
    vm.execute_instruction(Instruction::SetVL { new_vl: 1 })
        .unwrap();
    set_vector(&mut vm, 5, &[100]);
    set_vector(&mut vm, 6, &[23]);
    vm.execute_instruction(Instruction::Vadd {
        rd: 7,
        rs: 5,
        rt: 6,
    })
    .unwrap();
    assert_eq!(vector_values(&vm, 7), vec![123]);
}

#[test]
fn vadd_chunks() {
    let mut vm = vector_vm();
    vm.execute_instruction(Instruction::SetVL { new_vl: 6 })
        .unwrap();
    set_vector(&mut vm, 10, &[0, 1, 2, 3, 4, 5]);
    set_vector(&mut vm, 20, &[0, 2, 4, 6, 8, 10]);
    vm.execute_instruction(Instruction::Vadd {
        rd: 30,
        rs: 10,
        rt: 20,
    })
    .unwrap();
    assert_eq!(vector_values(&vm, 30), vec![0, 3, 6, 9, 12, 15]);
}

#[test]
fn vadd_privacy_violation() {
    let mut vm = vector_vm();
    vm.set_zk_mode(true);
    vm.execute_instruction(Instruction::SetVL { new_vl: 2 })
        .unwrap();
    set_vector(&mut vm, 10, &[1, 2]);
    set_vector(&mut vm, 20, &[3, 4]);
    set_vector_tags(&mut vm, 10, &[true, false]);
    set_vector_tags(&mut vm, 20, &[true, true]);
    let res = vm.execute_instruction(Instruction::Vadd {
        rd: 30,
        rs: 10,
        rt: 20,
    });
    assert!(matches!(res, Err(VMError::PrivacyViolation)));
}
