    #[test]
    fn sum_and_expression_control_flow_fail_closed() {
        for (source, code) in [
            (
                "fn f() { let _value = Option::none; }",
                "E_SUM_MISSING_CONTEXT",
            ),
            (
                "fn f(int value) -> Option<int> { value?; Option::none }",
                "E_PROPAGATE_TYPE",
            ),
            (
                "fn f(Result<int, string> value) -> Result<int, bytes> { Result::ok(value?) }",
                "E_PROPAGATE_ERROR_TYPE",
            ),
            (
                "fn f(Option<int> value) -> int { match value { Option::some(item) => item, } }",
                "E_MATCH_NON_EXHAUSTIVE",
            ),
            (
                "fn f(Option<int> value) -> int { match value { Option::some(item) => item, Option::some(other) => other, Option::none => 0, } }",
                "E_MATCH_DUPLICATE_PATTERN",
            ),
            (
                "fn f(Option<int> value) -> int { match value { Result::ok(item) => item, Result::err(_) => 0, } }",
                "E_PATTERN_FAMILY",
            ),
            (
                "fn f(bool flag) -> int { if flag { 1 } else { false } }",
                "E_BRANCH_TYPE_MISMATCH",
            ),
        ] {
            let program = parse(source).expect("compile-fail sum fixture must parse");
            let error = analyze(&program).expect_err("invalid sum/control-flow source must fail");
            assert_eq!(error.code, code, "{source}: {}", error.message);
        }
    }
