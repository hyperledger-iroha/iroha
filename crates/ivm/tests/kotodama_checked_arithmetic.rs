//! Checked and explicitly wrapping Kotodama `int` arithmetic regressions.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_data_model::prelude::Name;
use iroha_primitives::{
    bigint::BigInt,
    json::Json,
    numeric::{Numeric, Quantity},
    numeric_abi::{DecimalValueV1, QuantityValueV1},
};
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

#[derive(Clone, Copy, Debug)]
enum NumericReturnKind {
    Int,
    Decimal,
    Quantity,
}

#[derive(Debug, PartialEq, Eq)]
enum NumericValue {
    Int(BigInt),
    Decimal(Numeric),
    Quantity(Quantity),
}

#[derive(Debug, PartialEq, Eq)]
enum NumericOutcome {
    Value(NumericValue),
    Fault(NumericFaultV1),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FoldedSyscallExpectation {
    Omitted,
    Retained,
}

fn contains_extended_syscall(program: &[u8], syscall: u32) -> bool {
    let metadata = ProgramMetadata::parse(program).expect("parse numeric differential artifact");
    let expected = encoding::wide::encode_syscallx(syscall);
    program[metadata.code_offset..]
        .chunks_exact(4)
        .any(|word| u32::from_le_bytes(word.try_into().expect("four-byte instruction")) == expected)
}

fn decode_numeric_return(vm: &IVM, kind: NumericReturnKind) -> NumericValue {
    match kind {
        NumericReturnKind::Int => NumericValue::Int(common::decode_int_register(vm, 10)),
        NumericReturnKind::Decimal => {
            let output = vm
                .validate_tlv(vm.register(10))
                .expect("validate returned decimal pointer");
            assert_eq!(output.type_id, PointerType::Decimal);
            NumericValue::Decimal(
                DecimalValueV1::decode_frame(output.payload)
                    .expect("decode returned canonical decimal")
                    .into_numeric(),
            )
        }
        NumericReturnKind::Quantity => {
            let output = vm
                .validate_tlv(vm.register(10))
                .expect("validate returned quantity pointer");
            assert_eq!(output.type_id, PointerType::Quantity);
            NumericValue::Quantity(
                QuantityValueV1::decode_frame(output.payload)
                    .expect("decode returned canonical quantity")
                    .into_quantity(),
            )
        }
    }
}

fn numeric_fault_from_vm_error(error: &VMError) -> Option<NumericFaultV1> {
    match error {
        VMError::NumericFault(fault) => Some(*fault),
        VMError::Metered { source, .. } => numeric_fault_from_vm_error(source),
        _ => None,
    }
}

fn execute_numeric_program(
    program: &[u8],
    payload: Option<&Json>,
    kind: NumericReturnKind,
) -> NumericOutcome {
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(program)
        .expect("load numeric differential artifact");
    vm.set_program_counter(entrypoint_pc(program))
        .expect("select numeric differential entrypoint");
    let host = match payload {
        Some(payload) => argument_host(program, payload).expect("encode numeric arguments"),
        None => DefaultHost::new(),
    };
    vm.set_host(host);
    match vm.run() {
        Ok(()) => NumericOutcome::Value(decode_numeric_return(&vm, kind)),
        Err(error) => {
            let fault = numeric_fault_from_vm_error(&error).or_else(|| {
                // Infallible source conversions turn the recoverable ABI status into an
                // ABORT. Preserve the underlying numeric class in this differential gate.
                (error == VMError::AssertionFailed)
                    .then(|| NumericFaultV1::from_tag(vm.register(11)))
                    .flatten()
            });
            NumericOutcome::Fault(
                fault.unwrap_or_else(|| panic!("unexpected numeric runtime failure: {error:?}")),
            )
        }
    }
}

fn compiler_numeric_fault(error: &str) -> NumericFaultV1 {
    for (code, fault) in [
        (
            "E_DECIMAL_MANTISSA_OVERFLOW",
            NumericFaultV1::MantissaOverflow,
        ),
        ("E_INT_OVERFLOW", NumericFaultV1::MantissaOverflow),
        ("E_DECIMAL_SCALE_OVERFLOW", NumericFaultV1::ScaleOverflow),
        ("E_DIVISION_BY_ZERO", NumericFaultV1::DivisionByZero),
        ("E_REPEATING_DECIMAL", NumericFaultV1::RepeatingDecimal),
        (
            "E_EXACT_DIVISION_SCALE_OVERFLOW",
            NumericFaultV1::ExactDivisionScaleOverflow,
        ),
        ("E_INVALID_SCALE", NumericFaultV1::InvalidScale),
        ("E_INEXACT_CONVERSION", NumericFaultV1::InexactConversion),
        ("E_NEGATIVE_QUANTITY", NumericFaultV1::NegativeQuantity),
        ("E_QUANTITY_UNDERFLOW", NumericFaultV1::QuantityUnderflow),
    ] {
        if error.contains(code) {
            return fault;
        }
    }
    panic!("unexpected folded numeric failure: {error}");
}

fn assert_numeric_fold_runtime_parity(
    case: &str,
    folded_source: &str,
    runtime_source: &str,
    runtime_payload: &str,
    kind: NumericReturnKind,
    syscall: u32,
    folded_syscall: FoldedSyscallExpectation,
) {
    let runtime_program = compile(runtime_source);
    assert!(
        contains_extended_syscall(&runtime_program, syscall),
        "{case}: parameterized runtime program must invoke syscall 0x{syscall:06x}"
    );
    let payload = Json::from_str_norito(runtime_payload).expect("valid numeric argument JSON");
    let runtime = execute_numeric_program(&runtime_program, Some(&payload), kind);

    let folded = match Compiler::new().compile_source(folded_source) {
        Ok(program) => {
            let contains = contains_extended_syscall(&program, syscall);
            match folded_syscall {
                FoldedSyscallExpectation::Omitted => assert!(
                    !contains,
                    "{case}: constant-folded program retained syscall 0x{syscall:06x}"
                ),
                FoldedSyscallExpectation::Retained => assert!(
                    contains,
                    "{case}: recoverable conversion unexpectedly lost its runtime syscall"
                ),
            }
            execute_numeric_program(&program, None, kind)
        }
        Err(error) => NumericOutcome::Fault(compiler_numeric_fault(&error)),
    };

    assert_eq!(
        folded, runtime,
        "{case}: constant folding and parameterized VM execution diverged"
    );
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
fn explicit_int_to_decimal_conversion_matches_contextual_literal_folding() {
    let runtime = compile(
        "seiyaku MixedRuntime { view fn run(int left, decimal right) -> decimal { return decimal::from_int(left) + right; } }",
    );
    let metadata = ProgramMetadata::parse(&runtime).expect("parse explicit-conversion artifact");
    let conversion =
        encoding::wide::encode_syscallx(syscalls::SYSCALL_DECIMAL_FROM_INT).to_le_bytes();
    assert!(
        runtime[metadata.code_offset..]
            .windows(conversion.len())
            .any(|window| window == conversion),
        "decimal::from_int must lower through DECIMAL_FROM_INT"
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
fn decimal_constant_folding_matches_parameterized_vm_arithmetic_and_faults() {
    for (case, operator, left, right, syscall) in [
        (
            "decimal-add",
            "+",
            "1.2",
            "2.3",
            syscalls::SYSCALL_DECIMAL_ADD,
        ),
        (
            "decimal-sub",
            "-",
            "5",
            "1.25",
            syscalls::SYSCALL_DECIMAL_SUB,
        ),
        (
            "decimal-mul",
            "*",
            "1.5",
            "2",
            syscalls::SYSCALL_DECIMAL_MUL,
        ),
        (
            "decimal-exact-div",
            "/",
            "1",
            "8",
            syscalls::SYSCALL_DECIMAL_DIV_EXACT,
        ),
        (
            "decimal-repeating-div",
            "/",
            "1",
            "3",
            syscalls::SYSCALL_DECIMAL_DIV_EXACT,
        ),
        (
            "decimal-exact-div-scale-overflow",
            "/",
            "1",
            "100000000000000000000000000000",
            syscalls::SYSCALL_DECIMAL_DIV_EXACT,
        ),
        (
            "decimal-div-zero",
            "/",
            "1",
            "0",
            syscalls::SYSCALL_DECIMAL_DIV_EXACT,
        ),
        (
            "decimal-result-scale-overflow",
            "*",
            "0.000000000000001",
            "0.000000000000001",
            syscalls::SYSCALL_DECIMAL_MUL,
        ),
        (
            "decimal-mantissa-overflow",
            "+",
            MAX_INT,
            "1",
            syscalls::SYSCALL_DECIMAL_ADD,
        ),
    ] {
        let folded_source = format!(
            "seiyaku FoldedDecimal {{\n\
                 const decimal LEFT = {left};\n\
                 const decimal RIGHT = {right};\n\
                 view fn run() -> decimal {{ return LEFT {operator} RIGHT; }}\n\
             }}"
        );
        let runtime_source = format!(
            "seiyaku RuntimeDecimal {{\n\
                 view fn run(decimal left, decimal right) -> decimal {{\n\
                     return left {operator} right;\n\
                 }}\n\
             }}"
        );
        let payload = format!(r#"{{"left":"{left}","right":"{right}"}}"#);
        assert_numeric_fold_runtime_parity(
            case,
            &folded_source,
            &runtime_source,
            &payload,
            NumericReturnKind::Decimal,
            syscall,
            FoldedSyscallExpectation::Omitted,
        );
    }
}

#[test]
fn every_decimal_rounding_mode_matches_between_folding_and_vm_execution() {
    for (mode, dividend) in [
        ("toward_zero", "1"),
        ("away_from_zero", "1"),
        ("floor", "-1"),
        ("ceil", "-1"),
        ("nearest_even", "1"),
        ("nearest_away", "1"),
        ("nearest_toward_zero", "1"),
    ] {
        let folded_source = format!(
            "seiyaku FoldedRoundedDecimal {{\n\
                 const decimal VALUE = {dividend};\n\
                 const decimal DIVISOR = 8.0;\n\
                 const int SCALE = 2;\n\
                 view fn run() -> decimal {{\n\
                     return VALUE.div_round(\n\
                         divisor: DIVISOR, scale: SCALE, mode: Rounding::{mode});\n\
                 }}\n\
             }}"
        );
        let runtime_source = format!(
            "seiyaku RuntimeRoundedDecimal {{\n\
                 view fn run(decimal value, decimal divisor, int scale) -> decimal {{\n\
                     return value.div_round(\n\
                         divisor: divisor, scale: scale, mode: Rounding::{mode});\n\
                 }}\n\
             }}"
        );
        let payload = format!(r#"{{"value":"{dividend}","divisor":"8","scale":"2"}}"#);
        assert_numeric_fold_runtime_parity(
            mode,
            &folded_source,
            &runtime_source,
            &payload,
            NumericReturnKind::Decimal,
            syscalls::SYSCALL_DECIMAL_DIV_ROUND,
            FoldedSyscallExpectation::Omitted,
        );
    }
}

#[test]
fn decimal_to_int_conversions_match_folding_for_success_and_failure() {
    for (case, value) in [
        ("decimal-to-int-exact", "42"),
        ("decimal-to-int-inexact", "1.25"),
    ] {
        // The outer checked addition makes the exact cast part of a foldable numeric
        // expression. It also guards against a future lowering that silently defers a
        // known-inexact constant to runtime.
        let folded_source = format!(
            "seiyaku FoldedExactConversion {{\n\
                 const decimal VALUE = {value};\n\
                 view fn run() -> int {{\n\
                     return decimal::to_int_exact(value: VALUE) + 0;\n\
                 }}\n\
             }}"
        );
        let runtime_source = "seiyaku RuntimeExactConversion {\n\
             view fn run(decimal value) -> int {\n\
                 return decimal::to_int_exact(value: value) + 0;\n\
             }\n\
         }";
        let payload = format!(r#"{{"value":"{value}"}}"#);
        assert_numeric_fold_runtime_parity(
            case,
            &folded_source,
            runtime_source,
            &payload,
            NumericReturnKind::Int,
            syscalls::SYSCALL_DECIMAL_TRY_TO_INT_EXACT,
            FoldedSyscallExpectation::Omitted,
        );
    }

    assert_numeric_fold_runtime_parity(
        "decimal-to-int-trunc",
        "seiyaku FoldedTruncConversion {\n\
             view fn run() -> int {\n\
                 return decimal::to_int_trunc(value: -1.9);\n\
             }\n\
         }",
        "seiyaku RuntimeTruncConversion {\n\
             view fn run(decimal value) -> int {\n\
                 return decimal::to_int_trunc(value: value);\n\
             }\n\
         }",
        r#"{"value":"-1.9"}"#,
        NumericReturnKind::Int,
        syscalls::SYSCALL_DECIMAL_TO_INT_TRUNC,
        FoldedSyscallExpectation::Omitted,
    );

    for mode in [
        "toward_zero",
        "away_from_zero",
        "floor",
        "ceil",
        "nearest_even",
        "nearest_away",
        "nearest_toward_zero",
    ] {
        let folded_source = format!(
            "seiyaku FoldedRoundedConversion {{\n\
                 view fn run() -> int {{\n\
                     return decimal::to_int_round(\n\
                         value: 2.5, mode: Rounding::{mode});\n\
                 }}\n\
             }}"
        );
        let runtime_source = format!(
            "seiyaku RuntimeRoundedConversion {{\n\
                 view fn run(decimal value) -> int {{\n\
                     return decimal::to_int_round(\n\
                         value: value, mode: Rounding::{mode});\n\
                 }}\n\
             }}"
        );
        assert_numeric_fold_runtime_parity(
            mode,
            &folded_source,
            &runtime_source,
            r#"{"value":"2.5"}"#,
            NumericReturnKind::Int,
            syscalls::SYSCALL_DECIMAL_TO_INT_ROUND,
            FoldedSyscallExpectation::Omitted,
        );
    }
}

#[test]
fn quantity_arithmetic_folding_matches_parameterized_vm_execution() {
    for (case, constants, params, expression, payload, kind, syscall) in [
        (
            "quantity-add",
            "const quantity LEFT = 1.25; const quantity RIGHT = 2.75;",
            "quantity left, quantity right",
            "left + right",
            r#"{"left":"1.25","right":"2.75"}"#,
            NumericReturnKind::Quantity,
            syscalls::SYSCALL_QUANTITY_ADD,
        ),
        (
            "quantity-sub",
            "const quantity LEFT = 5.0; const quantity RIGHT = 1.25;",
            "quantity left, quantity right",
            "left - right",
            r#"{"left":"5","right":"1.25"}"#,
            NumericReturnKind::Quantity,
            syscalls::SYSCALL_QUANTITY_SUB,
        ),
        (
            "quantity-underflow",
            "const quantity LEFT = 1.0; const quantity RIGHT = 2.0;",
            "quantity left, quantity right",
            "left - right",
            r#"{"left":"1","right":"2"}"#,
            NumericReturnKind::Quantity,
            syscalls::SYSCALL_QUANTITY_SUB,
        ),
        (
            "quantity-mul-decimal",
            "const quantity LEFT = 1.5; const decimal RIGHT = 2.0;",
            "quantity left, decimal right",
            "left * right",
            r#"{"left":"1.5","right":"2"}"#,
            NumericReturnKind::Quantity,
            syscalls::SYSCALL_QUANTITY_MUL_DECIMAL,
        ),
        (
            "quantity-negative-result",
            "const quantity LEFT = 1.0; const decimal RIGHT = -1.0;",
            "quantity left, decimal right",
            "left * right",
            r#"{"left":"1","right":"-1"}"#,
            NumericReturnKind::Quantity,
            syscalls::SYSCALL_QUANTITY_MUL_DECIMAL,
        ),
        (
            "quantity-div-decimal",
            "const quantity LEFT = 1.0; const decimal RIGHT = 8.0;",
            "quantity left, decimal right",
            "left / right",
            r#"{"left":"1","right":"8"}"#,
            NumericReturnKind::Quantity,
            syscalls::SYSCALL_QUANTITY_DIV_DECIMAL_EXACT,
        ),
        (
            "quantity-ratio",
            "const quantity LEFT = 1.0; const quantity RIGHT = 8.0;",
            "quantity left, quantity right",
            "left / right",
            r#"{"left":"1","right":"8"}"#,
            NumericReturnKind::Decimal,
            syscalls::SYSCALL_QUANTITY_RATIO_EXACT,
        ),
    ] {
        let return_type = match kind {
            NumericReturnKind::Int => "int",
            NumericReturnKind::Decimal => "decimal",
            NumericReturnKind::Quantity => "quantity",
        };
        let folded_expression = expression.replace("left", "LEFT").replace("right", "RIGHT");
        let folded_source = format!(
            "seiyaku FoldedQuantity {{\n\
                 {constants}\n\
                 view fn run() -> {return_type} {{ return {folded_expression}; }}\n\
             }}"
        );
        let runtime_source = format!(
            "seiyaku RuntimeQuantity {{\n\
                 view fn run({params}) -> {return_type} {{ return {expression}; }}\n\
             }}"
        );
        assert_numeric_fold_runtime_parity(
            case,
            &folded_source,
            &runtime_source,
            payload,
            kind,
            syscall,
            FoldedSyscallExpectation::Omitted,
        );
    }
}

#[test]
fn explicit_quantity_conversions_match_for_values_and_negative_failures() {
    for (case, source_type, value, syscall) in [
        (
            "quantity-from-int",
            "int",
            "7",
            syscalls::SYSCALL_QUANTITY_TRY_FROM_INT,
        ),
        (
            "quantity-from-decimal",
            "decimal",
            "7.25",
            syscalls::SYSCALL_QUANTITY_TRY_FROM_DECIMAL,
        ),
    ] {
        let conversion = if source_type == "int" {
            "quantity::try_from_int"
        } else {
            "quantity::try_from_decimal"
        };
        let folded_source = format!(
            "seiyaku FoldedQuantityConversion {{\n\
                 view fn run() -> quantity {{\n\
                     let outcome = {conversion}(value: {value});\n\
                     return match outcome {{\n\
                         Result::ok(converted) => converted,\n\
                         Result::err(_) => 0\n\
                     }};\n\
                 }}\n\
             }}"
        );
        let runtime_source = format!(
            "seiyaku RuntimeQuantityConversion {{\n\
                 view fn run({source_type} value) -> quantity {{\n\
                     let outcome = {conversion}(value: value);\n\
                     return match outcome {{\n\
                         Result::ok(converted) => converted,\n\
                         Result::err(_) => 0\n\
                     }};\n\
                 }}\n\
             }}"
        );
        let payload = format!(r#"{{"value":"{value}"}}"#);
        assert_numeric_fold_runtime_parity(
            case,
            &folded_source,
            &runtime_source,
            &payload,
            NumericReturnKind::Quantity,
            syscall,
            // Recoverable conversions deliberately remain runtime operations so
            // their Result shape and stable fault payload cannot be optimized away.
            FoldedSyscallExpectation::Retained,
        );
    }

    for (case, source_type, value, syscall) in [
        (
            "negative-int-to-quantity",
            "int",
            "-1",
            syscalls::SYSCALL_QUANTITY_TRY_FROM_INT,
        ),
        (
            "negative-decimal-to-quantity",
            "decimal",
            "-1.25",
            syscalls::SYSCALL_QUANTITY_TRY_FROM_DECIMAL,
        ),
    ] {
        let conversion = if source_type == "int" {
            "quantity::try_from_int"
        } else {
            "quantity::try_from_decimal"
        };
        let folded_source = format!(
            "seiyaku FoldedNegativeQuantityConversion {{\n\
                 view fn run() -> int {{\n\
                     let outcome = {conversion}(value: {value});\n\
                     return match outcome {{\n\
                         Result::ok(_) => 0,\n\
                         Result::err(code) => code\n\
                     }};\n\
                 }}\n\
             }}"
        );
        let runtime_source = format!(
            "seiyaku RuntimeNegativeQuantityConversion {{\n\
                 view fn run({source_type} value) -> int {{\n\
                     let outcome = {conversion}(value: value);\n\
                     return match outcome {{\n\
                         Result::ok(_) => 0,\n\
                         Result::err(code) => code\n\
                     }};\n\
                 }}\n\
             }}"
        );
        let payload = format!(r#"{{"value":"{value}"}}"#);
        assert_numeric_fold_runtime_parity(
            case,
            &folded_source,
            &runtime_source,
            &payload,
            NumericReturnKind::Int,
            syscall,
            FoldedSyscallExpectation::Retained,
        );
    }

    assert_numeric_fold_runtime_parity(
        "quantity-to-decimal",
        "seiyaku FoldedQuantityToDecimal {\n\
             const quantity VALUE = 1.25;\n\
             view fn run() -> decimal {\n\
                 return decimal::from_quantity(value: VALUE) + 0.0;\n\
             }\n\
         }",
        "seiyaku RuntimeQuantityToDecimal {\n\
             view fn run(quantity value) -> decimal {\n\
                 return decimal::from_quantity(value: value) + 0.0;\n\
             }\n\
         }",
        r#"{"value":"1.25"}"#,
        NumericReturnKind::Decimal,
        syscalls::SYSCALL_QUANTITY_TO_DECIMAL,
        FoldedSyscallExpectation::Omitted,
    );
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
