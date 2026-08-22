#[test]
fn retired_numeric_literal_suffixes_are_rejected() {
    for suffix in ["i64", "u128", "amt", "qty", "float", "money"] {
        let source = format!("fn main() {{ let value = 1{suffix}; }}");
        let error = parse_module(&source).expect_err("numeric suffix must fail closed");
        assert!(
            error.contains("E_RETIRED_NUMERIC_SUFFIX"),
            "unexpected diagnostic for `{suffix}`: {error}"
        );
    }
}
#[test]
fn adaptive_width_int_literal_is_allowed_without_a_suffix() {
    let src = "fn main() { let int x = 340282366920938463463374607431768211455; }";
    let program = parse_module(src).expect("parse adaptive-width int literal");
    let function = program
        .items
        .into_iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("function present");
    let Statement::Let { value, .. } = function.body.statements[0].kind() else {
        panic!("expected let statement");
    };
    assert!(matches!(
        value.kind(),
        Expr::IntLiteral(value) if value.to_string() == "340282366920938463463374607431768211455"
    ));
}
#[test]
fn decimal_literal_ast_retains_exact_source_spelling() {
    let program =
        parse_module("fn main() { let decimal value = 1.250_0; }").expect("parse decimal literal");
    let function = program
        .items
        .into_iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("function present");
    let Statement::Let { value, .. } = function.body.statements[0].kind() else {
        panic!("expected let statement");
    };
    assert!(matches!(value.kind(), Expr::DecimalLiteral(value) if value == "1.250_0"));
}
#[test]
fn decimal_literals_follow_existing_expression_precedence() {
    let program = parse_module("fn main() { let value = true ? 1.0 : 2.0 + 3.0 * 4.0; }")
        .expect("parse decimal expression");
    let function = program
        .items
        .into_iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("function present");
    let Statement::Let { value, .. } = function.body.statements[0].kind() else {
        panic!("expected let statement");
    };
    let Expr::Conditional { else_expr, .. } = value.kind() else {
        panic!("expected conditional expression");
    };
    let Expr::Binary {
        op: BinaryOp::Add,
        right,
        ..
    } = else_expr.kind()
    else {
        panic!("expected addition in false branch");
    };
    assert!(matches!(
        right.kind(),
        Expr::Binary {
            op: BinaryOp::Mul,
            ..
        }
    ));
}
#[test]
fn signed_literals_retain_postfix_calls_after_atomic_range_parsing() {
    for literal in ["-1", "-1.0"] {
        let receiver = if literal == "-1" {
            format!("({literal})")
        } else {
            literal.to_owned()
        };
        parse_module(&format!(
            "fn main() {{ let value = {receiver}.operation(argument: 2); }}"
        ))
        .unwrap_or_else(|error| panic!("signed postfix `{literal}` failed: {error}"));
    }
}
