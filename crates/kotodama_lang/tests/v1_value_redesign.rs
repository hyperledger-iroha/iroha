//! Acceptance coverage for composable values, nominal errors, and explicit mutation.
use kotodama_lang::{
    compiler::Compiler,
    parser,
    semantic::{self, Type, TypedItem},
};

fn checked(body: &str) -> Result<semantic::TypedProgram, semantic::SemanticError> {
    let source = format!("seiyaku Values {{ {body} }}");
    semantic::analyze(&parser::parse(&source).expect("parse final V1 syntax"))
}
fn rejected(body: &str, code: &str) {
    let error = checked(body).expect_err("source must be rejected");
    assert_eq!(error.code(), code, "{error}");
}
#[test]
fn unit_is_a_value_in_aggregates_state_and_public_schemas() {
    let source = r#"seiyaku Units {
        struct Receipt { () marker; List<(), 2> markers; }
        state () marker;
        hajimari() { marker = (); }
        view fn main() -> Receipt { Receipt { marker: (), markers: [(), ()] } }
        fn omitted() { return (); }
        fn some_unit() -> Option<()> { Option::some(()) }
        fn result_unit() -> Result<(), ListError> { Result::ok(()) }
    }"#;
    let bytes = Compiler::new()
        .compile_source(source)
        .expect("compile composable Unit");
    let metadata = kotodama_lang::metadata::ProgramMetadata::parse(&bytes).unwrap();
    let interface = metadata.contract_interface.unwrap();
    assert!(matches!(
        interface.states[0].ty,
        kotodama_lang::metadata::EmbeddedStateType::Unit
    ));
    let main = interface
        .entrypoints
        .iter()
        .find(|entry| entry.name == "main")
        .unwrap();
    assert!(
        main.return_schema
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|node| matches!(node, ivm_abi::entrypoint::EntrypointValueTypeNodeV1::Unit))
    );
}
#[test]
fn error_codes_are_enum_local_and_nominal_values_support_exhaustive_match() {
    let typed = checked(
        r#"
        error enum Left { Failed = 1 }
        error enum Right { Failed = 1 }
        fn convert(Left value) -> Right {
            match value { Left::Failed => { Right::Failed } }
        }
        view fn main() -> bool { Left::Failed == Left::Failed }
    "#,
    )
    .unwrap();
    let left = typed
        .error_types
        .iter()
        .find(|item| item.identity.ends_with("::Left"))
        .unwrap();
    let right = typed
        .error_types
        .iter()
        .find(|item| item.identity.ends_with("::Right"))
        .unwrap();
    assert_ne!(left.identity, right.identity);
    assert_eq!(left.schema_hash(), right.schema_hash());
    rejected(
        "error enum Left { Failed = 1 } error enum Right { Failed = 1 } fn f() -> bool { Left::Failed == Right::Failed }",
        "K2003",
    );
    rejected(
        "error enum E { One = 1; Two = 2 } fn f(E value) -> int { match value { E::One => { 1 } } }",
        "E_MATCH_NON_EXHAUSTIVE",
    );
    rejected(
        "error enum E { One = 1; Two = 2 } fn f(E value) -> int { match value { E::One => { 1 }, E::One => { 2 } } }",
        "E_MATCH_DUPLICATE_PATTERN",
    );
}
#[test]
fn nominal_errors_work_with_japanese_branding_and_source_text() {
    let source = "誓約 Values { /* 不足は明示的な失敗です */ error enum Failure { Missing = 1 } view fn convert(Failure value) -> Failure { match value { Failure::Missing => { Failure::Missing } } } }";
    semantic::analyze(&parser::parse(source).unwrap()).unwrap();
}
#[test]
fn list_mutations_have_exact_results_and_mutable_receivers() {
    let typed = checked(
        r#"
        fn f() -> Result<(), ListError> {
            var List<int, 2> values = [1];
            values.set(index: 0, value: 2);
            values.push(3);
            values.try_set(index: 0, value: 4)?;
            values.try_push(5)
        }
    "#,
    )
    .unwrap();
    let TypedItem::Function(function) = &typed.items[0];
    assert!(
        matches!(function.ret_ty.as_ref(), Some(Type::Result(ok, err)) if **ok == Type::Unit && matches!(**err, Type::ErrorEnum(_)))
    );
    for method in [
        "set(index: 0, value: 2)",
        "push(2)",
        "try_set(index: 0, value: 2)",
        "try_push(2)",
    ] {
        rejected(
            &format!("fn f() {{ let List<int,2> values = [1]; let _ = values.{method}; }}"),
            "E_LIST_MUTABLE_RECEIVER",
        );
    }
}
#[test]
fn results_cannot_be_lost_implicitly_or_by_overwrite() {
    for source in [
        "fn f() { quantity::try_from_int(-1); }",
        "fn f() { let result = quantity::try_from_int(-1); }",
        "fn f() { var result = quantity::try_from_int(1); result = quantity::try_from_int(2); let _ = result; }",
        "fn f(bool flag) { let result = quantity::try_from_int(1); if flag { let _ = result; } }",
        "fn f() { for i in range(2) { let result = quantity::try_from_int(i); break; let _ = result; } }",
    ] {
        rejected(source, "E_RESULT_MUST_USE");
    }
    checked("fn f() { let _ = quantity::try_from_int(-1); }").unwrap();
    checked("fn f(bool flag) { let result = quantity::try_from_int(1); if flag { let _ = result; } else { let _ = result; } }").unwrap();
    checked("state Result<quantity, NumericError> stored; hajimari() { let result = quantity::try_from_int(1); stored = result; stored = quantity::try_from_int(2); }").unwrap();
}
#[test]
fn propagation_requires_identical_nominal_error_types() {
    rejected(
        "error enum E { Failed = 1 } fn f() -> Result<quantity, E> { let value = quantity::try_from_int(1)?; Result::ok(value) }",
        "E_PROPAGATE_ERROR_TYPE",
    );
}
#[test]
fn fused_rounding_folds_once_and_lowers_to_one_fused_instruction() {
    let typed = checked(r#"
        fn dynamic(decimal value, decimal multiplier, decimal divisor) -> decimal {
            value.mul_div_round(multiplier: multiplier, divisor: divisor, scale: 2, mode: Rounding::floor)
        }
        fn folded() -> decimal {
            7.13.mul_div_round(multiplier: 1.17, divisor: 3.0, scale: 2, mode: Rounding::floor)
        }
    "#).unwrap();
    let TypedItem::Function(folded) = &typed.items[1];
    assert!(
        matches!(&folded.body.tail.as_ref().unwrap().expr, semantic::ExprKind::DecimalLiteral { value, .. } if value.to_string() == "2.78")
    );
    let lowered = kotodama_lang::ir::lower(&typed).unwrap();
    let fused = lowered
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instrs)
        .filter(|instruction| {
            matches!(
                instruction,
                kotodama_lang::ir::Instr::NumericRound {
                    multiplier: Some(_),
                    op: kotodama_lang::ir::NumericRoundOp::DecimalMulDiv,
                    ..
                }
            )
        })
        .count();
    assert_eq!(fused, 1);
}
