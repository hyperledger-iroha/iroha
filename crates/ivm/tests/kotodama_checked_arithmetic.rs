//! Checked and explicitly wrapping Kotodama `int` arithmetic regressions.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_data_model::prelude::Name;
use iroha_primitives::{bigint::BigInt, json::Json, numeric::Numeric, numeric_abi::DecimalValueV1};
use ivm::{
    IVM, ProgramMetadata, VMError, encoding, host::DefaultHost, kotodama::compiler::Compiler,
    numeric::NumericFaultV1, pointer_abi::PointerType, syscalls,
};
mod common;

const MAX_INT: &str = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047";
const MIN_INT: &str = "-6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042048";

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

fn argument_host(program: &[u8], payload: &Json) -> Result<DefaultHost, VMError> {
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
    let key: Name = "trigger_event_json".parse().expect("public input key");
    Ok(DefaultHost::new().with_public_inputs(BTreeMap::from([(
        key,
        tlv(PointerType::NoritoBytes, &record),
    )])))
}

fn run_binary(program: &[u8], left: &str, right: &str) -> Result<BigInt, VMError> {
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(program)?;
    vm.set_program_counter(entrypoint_pc(program))?;
    let payload = Json::from_str_norito(&format!(r#"{{"left":"{left}","right":"{right}"}}"#))
        .expect("valid binary arguments");
    vm.set_host(argument_host(program, &payload)?);
    vm.run()?;
    Ok(common::decode_int_register(&vm, 10))
}

fn run_unary(program: &[u8], value: &str) -> Result<BigInt, VMError> {
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(program)?;
    vm.set_program_counter(entrypoint_pc(program))?;
    let payload =
        Json::from_str_norito(&format!(r#"{{"value":"{value}"}}"#)).expect("valid unary arguments");
    vm.set_host(argument_host(program, &payload)?);
    vm.run()?;
    Ok(common::decode_int_register(&vm, 10))
}

fn run_mixed_int_decimal(program: &[u8], left: &str, right: &str) -> Result<Numeric, VMError> {
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(program)?;
    vm.set_program_counter(entrypoint_pc(program))?;
    let payload = Json::from_str_norito(&format!(r#"{{"left":"{left}","right":"{right}"}}"#))
        .expect("valid mixed numeric arguments");
    vm.set_host(argument_host(program, &payload)?);
    vm.run()?;
    let output = vm.validate_tlv(vm.register(10))?;
    assert_eq!(output.type_id, PointerType::Decimal);
    DecimalValueV1::decode_frame(output.payload)
        .map(DecimalValueV1::into_numeric)
        .map_err(|_| VMError::DecodeError)
}

fn bigint(value: &str) -> BigInt {
    value.parse().expect("parse bounded integer fixture")
}

#[derive(Debug, PartialEq, Eq)]
enum ArithmeticOutcome {
    Value(BigInt),
    MantissaOverflow,
    DivisionByZero,
}

fn classify_runtime(result: Result<BigInt, VMError>) -> ArithmeticOutcome {
    match result {
        Ok(value) => ArithmeticOutcome::Value(value),
        Err(VMError::NumericFault(NumericFaultV1::MantissaOverflow)) => {
            ArithmeticOutcome::MantissaOverflow
        }
        Err(VMError::NumericFault(NumericFaultV1::DivisionByZero)) => {
            ArithmeticOutcome::DivisionByZero
        }
        Err(error) => panic!("unexpected runtime arithmetic failure: {error:?}"),
    }
}

fn folded_outcome(expression: &str) -> ArithmeticOutcome {
    let source =
        format!("seiyaku FoldedArithmetic {{ view fn run() -> int {{ return {expression}; }} }}");
    let program = match Compiler::new().compile_source(&source) {
        Ok(program) => program,
        Err(error) if error.contains("E_INT_OVERFLOW") => {
            return ArithmeticOutcome::MantissaOverflow;
        }
        Err(error) if error.contains("E_DIVISION_BY_ZERO") => {
            return ArithmeticOutcome::DivisionByZero;
        }
        Err(error) => panic!("unexpected folded arithmetic failure: {error}"),
    };
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&program)
        .expect("load folded arithmetic program");
    vm.set_program_counter(entrypoint_pc(&program))
        .expect("select folded arithmetic entrypoint");
    vm.run().expect("execute folded arithmetic result");
    ArithmeticOutcome::Value(common::decode_int_register(&vm, 10))
}

#[test]
fn constant_folding_and_runtime_match_signed_512_bit_boundaries_and_failures() {
    for (operator, left, right) in [
        ("+", MAX_INT, "0"),
        ("+", MAX_INT, "1"),
        ("+", MIN_INT, "-1"),
        ("-", MIN_INT, "0"),
        ("-", MIN_INT, "1"),
        ("-", MAX_INT, "-1"),
        ("*", MAX_INT, "1"),
        ("*", MAX_INT, "2"),
        ("*", MIN_INT, "-1"),
        ("/", MIN_INT, "1"),
        ("/", MIN_INT, "-1"),
        ("/", "1", "0"),
        ("%", MIN_INT, "1"),
        ("%", MIN_INT, "-1"),
        ("%", "1", "0"),
    ] {
        let program = compile(&format!(
            "seiyaku RuntimeArithmetic {{ view fn run(int left, int right) -> int {{ return left {operator} right; }} }}"
        ));
        let runtime = classify_runtime(run_binary(&program, left, right));
        let folded = folded_outcome(&format!("({left}) {operator} ({right})"));
        assert_eq!(
            folded, runtime,
            "folded/runtime mismatch for ({left}) {operator} ({right})"
        );
    }

    let negation =
        compile("seiyaku RuntimeNegation { view fn run(int value) -> int { return -value; } }");
    for value in [MAX_INT, MIN_INT] {
        let runtime = classify_runtime(run_unary(&negation, value));
        let folded = folded_outcome(&format!("-({value})"));
        assert_eq!(folded, runtime, "folded/runtime mismatch for -({value})");
    }
}

#[test]
fn mixed_int_decimal_runtime_promotion_matches_folding_and_uses_decimal_from_int() {
    let runtime = compile(
        "seiyaku MixedRuntime { view fn run(int left, decimal right) -> decimal { return left + right; } }",
    );
    let metadata = ProgramMetadata::parse(&runtime).expect("parse mixed runtime artifact");
    let conversion =
        encoding::wide::encode_syscallx(syscalls::SYSCALL_DECIMAL_FROM_INT).to_le_bytes();
    assert!(
        runtime[metadata.code_offset..]
            .windows(conversion.len())
            .any(|window| window == conversion),
        "mixed int/decimal runtime arithmetic must promote through DECIMAL_FROM_INT"
    );

    let runtime_value = run_mixed_int_decimal(&runtime, "9007199254740993", "0.125")
        .expect("execute mixed runtime arithmetic");
    let folded = compile(
        "seiyaku MixedFolded { view fn run() -> decimal { return 9007199254740993 + 0.125; } }",
    );
    let mut folded_vm = IVM::new(u64::MAX);
    folded_vm
        .load_program(&folded)
        .expect("load folded decimal");
    folded_vm
        .set_program_counter(entrypoint_pc(&folded))
        .expect("select folded decimal entrypoint");
    folded_vm.run().expect("execute folded decimal");
    let folded_output = folded_vm
        .validate_tlv(folded_vm.register(10))
        .expect("validate folded decimal pointer");
    assert_eq!(folded_output.type_id, PointerType::Decimal);
    let folded_value = DecimalValueV1::decode_frame(folded_output.payload)
        .expect("decode folded decimal")
        .into_numeric();

    assert_eq!(runtime_value, folded_value);
    assert_eq!(runtime_value.to_string(), "9007199254740993.125");
}

#[test]
fn ordinary_addition_and_subtraction_trap_at_signed_512_bit_boundaries() {
    let add = compile(
        "seiyaku CheckedAdd { view fn run(int left, int right) -> int { return left + right; } }",
    );
    assert_eq!(run_binary(&add, MAX_INT, "0").unwrap(), bigint(MAX_INT));
    assert_eq!(run_binary(&add, MIN_INT, "0").unwrap(), bigint(MIN_INT));
    assert!(matches!(
        run_binary(&add, MAX_INT, "1"),
        Err(VMError::NumericFault(NumericFaultV1::MantissaOverflow))
    ));
    assert!(matches!(
        run_binary(&add, MIN_INT, "-1"),
        Err(VMError::NumericFault(NumericFaultV1::MantissaOverflow))
    ));

    let sub = compile(
        "seiyaku CheckedSub { view fn run(int left, int right) -> int { return left - right; } }",
    );
    assert_eq!(run_binary(&sub, MIN_INT, "0").unwrap(), bigint(MIN_INT));
    assert_eq!(run_binary(&sub, MAX_INT, "0").unwrap(), bigint(MAX_INT));
    assert!(matches!(
        run_binary(&sub, MIN_INT, "1"),
        Err(VMError::NumericFault(NumericFaultV1::MantissaOverflow))
    ));
    assert!(matches!(
        run_binary(&sub, MAX_INT, "-1"),
        Err(VMError::NumericFault(NumericFaultV1::MantissaOverflow))
    ));
}

#[test]
fn ordinary_multiplication_and_negation_trap_at_signed_512_bit_boundaries() {
    let mul = compile(
        "seiyaku CheckedMul { view fn run(int left, int right) -> int { return left * right; } }",
    );
    assert_eq!(run_binary(&mul, MAX_INT, "1").unwrap(), bigint(MAX_INT));
    assert_eq!(run_binary(&mul, MIN_INT, "1").unwrap(), bigint(MIN_INT));
    assert!(matches!(
        run_binary(&mul, MAX_INT, "2"),
        Err(VMError::NumericFault(NumericFaultV1::MantissaOverflow))
    ));
    assert!(matches!(
        run_binary(&mul, MIN_INT, "-1"),
        Err(VMError::NumericFault(NumericFaultV1::MantissaOverflow))
    ));

    let neg = compile("seiyaku CheckedNeg { view fn run(int value) -> int { return -value; } }");
    assert_eq!(
        run_unary(&neg, MAX_INT).unwrap(),
        bigint(&format!("-{MAX_INT}"))
    );
    assert_eq!(run_unary(&neg, "0").unwrap(), bigint("0"));
    assert!(matches!(
        run_unary(&neg, MIN_INT),
        Err(VMError::NumericFault(NumericFaultV1::MantissaOverflow))
    ));
}

#[test]
fn constant_folding_uses_checked_signed_512_bit_rules() {
    let safe = compile(
        "seiyaku CheckedConstant { view fn run() -> int { return (6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047 - 1) + 1; } }",
    );
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&safe).unwrap();
    vm.set_program_counter(entrypoint_pc(&safe)).unwrap();
    vm.run().unwrap();
    assert_eq!(common::decode_int_register(&vm, 10), bigint(MAX_INT));

    for source in [
        "seiyaku OverflowAdd { view fn run() -> int { return 6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047 + 1; } }",
        "seiyaku OverflowNeg { view fn run() -> int { return -(-6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042048); } }",
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
  view fn run() -> (int, int, int, int) {
    return (
        math::wrapping_add(left: 6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047, right: 1),
        math::wrapping_sub(left: -6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042048, right: 1),
        math::wrapping_mul(left: 6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047, right: 2),
        math::wrapping_neg(-6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042048)
    );
  }
}
"#,
    );
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&program).unwrap();
    vm.set_program_counter(entrypoint_pc(&program)).unwrap();
    vm.run().unwrap();
    assert_eq!(common::decode_int_register(&vm, 10), bigint(MIN_INT));
    assert_eq!(common::decode_int_register(&vm, 11), bigint(MAX_INT));
    assert_eq!(common::decode_int_register(&vm, 12), bigint("-2"));
    assert_eq!(common::decode_int_register(&vm, 13), bigint(MIN_INT));
}
