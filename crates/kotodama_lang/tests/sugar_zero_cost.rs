//! Executable-equivalence tests for expression-oriented Kotodama V1 sugar.

use kotodama_lang::{compiler::Compiler, metadata::ProgramMetadata};

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

    assert_executable_equivalent(if_let, exhaustive, "if let expression");
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

    assert_executable_equivalent(matched, explicit, "exhaustive Option match");
}
