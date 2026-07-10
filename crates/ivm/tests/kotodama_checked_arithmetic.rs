//! Checked and explicitly wrapping Kotodama `i64` arithmetic regressions.

use iroha_crypto::Hash;
use iroha_primitives::json::Json;
use ivm::{
    IVM, ProgramMetadata, VMError, host::DefaultHost, kotodama::compiler::Compiler,
    pointer_abi::PointerType,
};

fn compile(source: &str) -> Vec<u8> {
    Compiler::new()
        .compile_source(source)
        .expect("compile arithmetic contract")
}

fn entrypoint_pc(program: &[u8]) -> u64 {
    let parsed = ProgramMetadata::parse(program).expect("parse checked arithmetic artifact");
    let entrypoint = parsed
        .contract_interface
        .as_ref()
        .expect("embedded contract interface")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "run")
        .expect("run entrypoint");
    u64::try_from(parsed.prefix_len()).expect("prefix fits u64") + entrypoint.entry_pc
}

fn tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("test payload fits u32")
            .to_be_bytes(),
    );
    out.extend_from_slice(payload);
    out.extend_from_slice(Hash::new(payload).as_ref());
    out
}

fn install_argument_record(vm: &mut IVM, program: &[u8], payload: &Json) -> Result<(), VMError> {
    let parsed = ProgramMetadata::parse(program)?;
    let entrypoint = parsed
        .contract_interface
        .as_ref()
        .and_then(|interface| {
            interface
                .entrypoints
                .iter()
                .find(|entrypoint| entrypoint.name == "run")
        })
        .expect("run entrypoint descriptor");
    let schema = entrypoint
        .argument_schema
        .as_ref()
        .expect("parameterized run entrypoint schema");
    let record = ivm::encode_argument_record_from_json(schema, payload)?;
    let pointer = vm.alloc_input_tlv(&tlv(PointerType::NoritoBytes, &record))?;
    vm.set_register(10, pointer);
    Ok(())
}

fn run_binary(program: &[u8], left: i64, right: i64) -> Result<i64, VMError> {
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(program)?;
    vm.set_program_counter(entrypoint_pc(program))?;
    vm.set_host(DefaultHost::new());
    let payload = Json::from_str_norito(&format!(r#"{{"left":{left},"right":{right}}}"#))
        .expect("valid binary arguments");
    install_argument_record(&mut vm, program, &payload)?;
    vm.run()?;
    Ok(vm.register(10) as i64)
}

fn run_unary(program: &[u8], value: i64) -> Result<i64, VMError> {
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(program)?;
    vm.set_program_counter(entrypoint_pc(program))?;
    vm.set_host(DefaultHost::new());
    let payload =
        Json::from_str_norito(&format!(r#"{{"value":{value}}}"#)).expect("valid unary arguments");
    install_argument_record(&mut vm, program, &payload)?;
    vm.run()?;
    Ok(vm.register(10) as i64)
}

#[test]
fn ordinary_addition_and_subtraction_trap_at_i64_boundaries() {
    let add = compile(
        "seiyaku CheckedAdd { view fn run(left: i64, right: i64) -> i64 { return left + right; } }",
    );
    assert_eq!(run_binary(&add, i64::MAX, 0).unwrap(), i64::MAX);
    assert_eq!(run_binary(&add, i64::MIN, 0).unwrap(), i64::MIN);
    assert!(matches!(
        run_binary(&add, i64::MAX, 1),
        Err(VMError::AssertionFailed)
    ));
    assert!(matches!(
        run_binary(&add, i64::MIN, -1),
        Err(VMError::AssertionFailed)
    ));

    let sub = compile(
        "seiyaku CheckedSub { view fn run(left: i64, right: i64) -> i64 { return left - right; } }",
    );
    assert_eq!(run_binary(&sub, i64::MIN, 0).unwrap(), i64::MIN);
    assert_eq!(run_binary(&sub, i64::MAX, 0).unwrap(), i64::MAX);
    assert!(matches!(
        run_binary(&sub, i64::MIN, 1),
        Err(VMError::AssertionFailed)
    ));
    assert!(matches!(
        run_binary(&sub, i64::MAX, -1),
        Err(VMError::AssertionFailed)
    ));
}

#[test]
fn ordinary_multiplication_and_negation_trap_at_i64_boundaries() {
    let mul = compile(
        "seiyaku CheckedMul { view fn run(left: i64, right: i64) -> i64 { return left * right; } }",
    );
    assert_eq!(run_binary(&mul, i64::MAX, 1).unwrap(), i64::MAX);
    assert_eq!(run_binary(&mul, i64::MIN, 1).unwrap(), i64::MIN);
    assert!(matches!(
        run_binary(&mul, i64::MAX, 2),
        Err(VMError::AssertionFailed)
    ));
    assert!(matches!(
        run_binary(&mul, i64::MIN, -1),
        Err(VMError::AssertionFailed)
    ));

    let neg = compile("seiyaku CheckedNeg { view fn run(value: i64) -> i64 { return -value; } }");
    assert_eq!(run_unary(&neg, i64::MAX).unwrap(), -i64::MAX);
    assert_eq!(run_unary(&neg, 0).unwrap(), 0);
    assert!(matches!(
        run_unary(&neg, i64::MIN),
        Err(VMError::AssertionFailed)
    ));
}

#[test]
fn constant_folding_uses_checked_i64_rules() {
    let safe = compile(
        "seiyaku CheckedConstant { view fn run() -> i64 { return (9223372036854775807 - 1) + 1; } }",
    );
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&safe).unwrap();
    vm.set_program_counter(entrypoint_pc(&safe)).unwrap();
    vm.run().unwrap();
    assert_eq!(vm.register(10) as i64, i64::MAX);

    for source in [
        "seiyaku OverflowAdd { view fn run() -> i64 { return 9223372036854775807 + 1; } }",
        "seiyaku OverflowNeg { view fn run() -> i64 { return -(-9223372036854775808); } }",
    ] {
        let error = Compiler::new()
            .compile_source(source)
            .expect_err("constant overflow must fail compilation");
        assert!(
            error.contains("E_INT_OVERFLOW"),
            "unexpected error: {error}"
        );
    }
}

#[test]
fn wrapping_builtins_are_the_explicit_modular_opt_in() {
    let program = compile(
        r#"
seiyaku WrappingArithmetic {
  view fn run() -> (i64, i64, i64, i64) {
    return (
        math::wrapping_add(9223372036854775807, 1),
        math::wrapping_sub(-9223372036854775808, 1),
        math::wrapping_mul(9223372036854775807, 2),
        math::wrapping_neg(-9223372036854775808)
    );
  }
}
"#,
    );
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&program).unwrap();
    vm.set_program_counter(entrypoint_pc(&program)).unwrap();
    vm.run().unwrap();
    assert_eq!(vm.register(10) as i64, i64::MIN);
    assert_eq!(vm.register(11) as i64, i64::MAX);
    assert_eq!(vm.register(12) as i64, -2);
    assert_eq!(vm.register(13) as i64, i64::MIN);
}
