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
    let propagated = r#"
        seiyaku ResultPropagation {
            view fn main(Result<int, bool> value) -> Result<(int, int), bool> {
                let payload = value?;
                Result::ok((payload, payload))
            }
        }
    "#;
    let explicit = r#"
        seiyaku ResultPropagation {
            view fn main(Result<int, bool> value) -> Result<(int, int), bool> {
                let payload = match value {
                    Result::ok(payload) => payload,
                    Result::err(failure) => { return Result::err(failure); },
                };
                Result::ok((payload, payload))
            }
        }
    "#;

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
    let propagated = r#"
        seiyaku OptionPropagation {
            view fn main(Option<int> value) -> Option<(int, int)> {
                let payload = value?;
                Option::some((payload, payload))
            }
        }
    "#;
    let explicit = r#"
        seiyaku OptionPropagation {
            view fn main(Option<int> value) -> Option<(int, int)> {
                let payload = match value {
                    Option::some(payload) => payload,
                    Option::none => { return Option::none; },
                };
                Option::some((payload, payload))
            }
        }
    "#;

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
    let tail = r#"
        seiyaku TailExpression {
            view fn main(int value) -> int { value + 1 }
        }
    "#;
    let explicit = r#"
        seiyaku TailExpression {
            view fn main(int value) -> int { return value + 1; }
        }
    "#;

    assert_ir_equivalent(tail, explicit, "function tail expression");
    assert_executable_equivalent(tail, explicit, "function tail expression");
}

#[test]
fn if_block_expression_matches_the_existing_ternary() {
    let block = r#"
        seiyaku IfExpression {
            view fn main(bool condition, int yes, int no) -> int {
                if condition { yes } else { no }
            }
        }
    "#;
    let ternary = r#"
        seiyaku IfExpression {
            view fn main(bool condition, int yes, int no) -> int {
                condition ? yes : no
            }
        }
    "#;

    assert_ir_equivalent(block, ternary, "expression-valued if block");
    assert_executable_equivalent(block, ternary, "expression-valued if block");
}

#[test]
fn if_let_matches_the_exhaustive_match_form() {
    let if_let = r#"
        seiyaku IfLetExpression {
            view fn main(Option<int> value, int fallback) -> int {
                if let Option::some(payload) = value { payload } else { fallback }
            }
        }
    "#;
    let exhaustive = r#"
        seiyaku IfLetExpression {
            view fn main(Option<int> value, int fallback) -> int {
                match value {
                    Option::some(payload) => payload,
                    Option::none => fallback,
                }
            }
        }
    "#;

    assert_ir_equivalent(if_let, exhaustive, "if let expression");
    assert_executable_equivalent(if_let, exhaustive, "if let expression");
}

#[test]
fn result_if_let_matches_the_exhaustive_match_form() {
    let if_let = r#"
        seiyaku ResultIfLetExpression {
            view fn main(Result<int, bool> value, int fallback) -> int {
                if let Result::ok(payload) = value { payload } else { fallback }
            }
        }
    "#;
    let exhaustive = r#"
        seiyaku ResultIfLetExpression {
            view fn main(Result<int, bool> value, int fallback) -> int {
                match value {
                    Result::ok(payload) => payload,
                    Result::err(_) => fallback,
                }
            }
        }
    "#;

    assert_ir_equivalent(if_let, exhaustive, "Result if let expression");
    assert_executable_equivalent(if_let, exhaustive, "Result if let expression");
}

#[test]
fn named_call_matches_explicit_source_order_and_positional_abi_order() {
    let named = r#"
        seiyaku NamedCall {
            fn first() -> int { 1 }
            fn second() -> bool { true }
            fn combine(int left, bool right) -> int {
                if right { left } else { 0 }
            }
            view fn main() -> int {
                combine(right: second(), left: first())
            }
        }
    "#;
    let explicit = r#"
        seiyaku NamedCall {
            fn first() -> int { 1 }
            fn second() -> bool { true }
            fn combine(int left, bool right) -> int {
                if right { left } else { 0 }
            }
            view fn main() -> int {
                let bool right_value = second();
                let int left_value = first();
                combine(left_value, right_value)
            }
        }
    "#;

    assert_ir_equivalent(named, explicit, "out-of-order named call");
    assert_executable_equivalent(named, explicit, "out-of-order named call");
}

#[test]
fn exhaustive_option_match_matches_eager_unwrap_or() {
    let matched = r#"
        seiyaku MatchExpression {
            view fn main(Option<int> value, int fallback) -> int {
                match value {
                    Option::some(payload) => payload,
                    Option::none => fallback,
                }
            }
        }
    "#;
    let explicit = r#"
        seiyaku MatchExpression {
            view fn main(Option<int> value, int fallback) -> int {
                value.unwrap_or(fallback)
            }
        }
    "#;

    assert_ir_equivalent(matched, explicit, "exhaustive Option match");
    assert_executable_equivalent(matched, explicit, "exhaustive Option match");
}
