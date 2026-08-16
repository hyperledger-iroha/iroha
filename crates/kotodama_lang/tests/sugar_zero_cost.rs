//! IR and executable-equivalence tests for expression-oriented Kotodama V1 sugar.
use kotodama_lang::{
    compiler::Compiler, ir, metadata::ProgramMetadata, parser::parse, semantic::analyze,
};
fn executable_code(source: &str) -> Vec<u8> {
    let artifact = Compiler::new()
        .compile_source(source)
        .expect("compile Kotodama V1 source");
    let metadata = ProgramMetadata::parse(&artifact).expect("parse Kotodama V1 artifact");
    artifact[metadata.code_offset..].to_vec()
}
fn assert_executable_equivalent(sugar: &str, explicit: &str, description: &str) {
    assert_eq!(
        executable_code(sugar),
        executable_code(explicit),
        "{description} must be erased before executable code generation"
    );
}
fn assert_ir_equivalent(sugar: &str, explicit: &str, description: &str) {
    let lower = |source| {
        let parsed = parse(source).expect("parse Kotodama V1 source for IR comparison");
        let typed = analyze(&parsed).expect("analyze Kotodama V1 source for IR comparison");
        ir::lower(&typed).expect("lower Kotodama V1 source for IR comparison")
    };
    let sugar = lower(sugar);
    let explicit = lower(explicit);
    assert_eq!(
        sugar.functions.len(),
        explicit.functions.len(),
        "{description} must produce the same IR function set"
    );
    for (sugar, explicit) in sugar.functions.iter().zip(&explicit.functions) {
        assert_eq!(sugar.name, explicit.name, "{description}: function name");
        assert_eq!(
            sugar.params, explicit.params,
            "{description}: function parameters"
        );
        assert_eq!(sugar.entry, explicit.entry, "{description}: entry block");
        assert_eq!(
            sugar.blocks, explicit.blocks,
            "{description} must be erased during IR lowering"
        );
    }
}
#[test]
fn result_propagation_matches_the_exhaustive_early_return_form() {
    let propagated = include_str!("../fixtures/koto_v1/sugar_zero_cost/001.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let explicit = include_str!("../fixtures/koto_v1/sugar_zero_cost/002.ko").strip_suffix('\n').expect("fixture sentinel newline");
    assert_ir_equivalent(
        propagated,
        explicit,
        "postfix Result propagation with a changed success layout",
    );
    assert_executable_equivalent(
        propagated,
        explicit,
        "postfix Result propagation with a changed success layout",
    );
}
#[test]
fn option_propagation_matches_the_exhaustive_early_return_form() {
    let propagated = include_str!("../fixtures/koto_v1/sugar_zero_cost/003.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let explicit = include_str!("../fixtures/koto_v1/sugar_zero_cost/004.ko").strip_suffix('\n').expect("fixture sentinel newline");
    assert_ir_equivalent(
        propagated,
        explicit,
        "postfix Option propagation with a changed payload layout",
    );
    assert_executable_equivalent(
        propagated,
        explicit,
        "postfix Option propagation with a changed payload layout",
    );
}
#[test]
fn function_tail_matches_explicit_return() {
    let tail = include_str!("../fixtures/koto_v1/sugar_zero_cost/005.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let explicit = include_str!("../fixtures/koto_v1/sugar_zero_cost/006.ko").strip_suffix('\n').expect("fixture sentinel newline");
    assert_ir_equivalent(tail, explicit, "function tail expression");
    assert_executable_equivalent(tail, explicit, "function tail expression");
}
#[test]
fn if_block_expression_matches_the_existing_ternary() {
    let block = include_str!("../fixtures/koto_v1/sugar_zero_cost/007.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let ternary = include_str!("../fixtures/koto_v1/sugar_zero_cost/008.ko").strip_suffix('\n').expect("fixture sentinel newline");
    assert_ir_equivalent(block, ternary, "expression-valued if block");
    assert_executable_equivalent(block, ternary, "expression-valued if block");
}
#[test]
fn if_let_matches_the_exhaustive_match_form() {
    let if_let = include_str!("../fixtures/koto_v1/sugar_zero_cost/009.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let exhaustive = include_str!("../fixtures/koto_v1/sugar_zero_cost/010.ko").strip_suffix('\n').expect("fixture sentinel newline");
    assert_ir_equivalent(if_let, exhaustive, "if let expression");
    assert_executable_equivalent(if_let, exhaustive, "if let expression");
}
#[test]
fn result_if_let_matches_the_exhaustive_match_form() {
    let if_let = include_str!("../fixtures/koto_v1/sugar_zero_cost/011.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let exhaustive = include_str!("../fixtures/koto_v1/sugar_zero_cost/012.ko").strip_suffix('\n').expect("fixture sentinel newline");
    assert_ir_equivalent(if_let, exhaustive, "Result if let expression");
    assert_executable_equivalent(if_let, exhaustive, "Result if let expression");
}
#[test]
fn named_call_matches_explicit_source_order_and_positional_abi_order() {
    let named = include_str!("../fixtures/koto_v1/sugar_zero_cost/013.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let explicit = include_str!("../fixtures/koto_v1/sugar_zero_cost/014.ko").strip_suffix('\n').expect("fixture sentinel newline");
    assert_ir_equivalent(named, explicit, "out-of-order named call");
    assert_executable_equivalent(named, explicit, "out-of-order named call");
}
#[test]
fn named_struct_matches_explicit_source_order_and_declaration_layout() {
    let named = include_str!("../fixtures/koto_v1/sugar_zero_cost/015.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let explicit = include_str!("../fixtures/koto_v1/sugar_zero_cost/016.ko").strip_suffix('\n').expect("fixture sentinel newline");
    assert_ir_equivalent(
        named,
        explicit,
        "out-of-order named struct with observable field evaluation",
    );
    assert_executable_equivalent(
        named,
        explicit,
        "out-of-order named struct with observable field evaluation",
    );
}
#[test]
fn exhaustive_option_match_matches_eager_unwrap_or() {
    let matched = include_str!("../fixtures/koto_v1/sugar_zero_cost/017.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let explicit = include_str!("../fixtures/koto_v1/sugar_zero_cost/018.ko").strip_suffix('\n').expect("fixture sentinel newline");
    assert_ir_equivalent(matched, explicit, "exhaustive Option match");
    assert_executable_equivalent(matched, explicit, "exhaustive Option match");
}
