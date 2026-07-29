//! Public compiler-session compile-fail diagnostic goldens for Kotodama V1.

use kotodama_lang::{
    diagnostic::DiagnosticPhase,
    semantic::MAX_EXPANDED_TYPE_NODES,
    session::{CompileRequest, CompilerSession},
    source::MAX_NESTING_DEPTH,
};

#[derive(Clone, Copy)]
struct CompileFailCase {
    name: &'static str,
    source: &'static str,
    phase: DiagnosticPhase,
    code: &'static str,
    message: &'static str,
    line: usize,
}

const CASES: &[CompileFailCase] = &[
    CompileFailCase {
        name: "duplicate-declaration",
        source: "seiyaku Duplicate {\nfn repeated() {}\nfn repeated() {}\n}",
        phase: DiagnosticPhase::Resolve,
        code: "E_DUPLICATE_DECLARATION",
        message: "declaration name `repeated` is already used by a function",
        line: 3,
    },
    CompileFailCase {
        name: "unknown-name",
        source: "seiyaku UnknownName {\nfn read() -> int { return missing_value; }\n}",
        phase: DiagnosticPhase::Resolve,
        code: "K2002",
        message: "unknown value `missing_value`",
        line: 2,
    },
    CompileFailCase {
        name: "unknown-type",
        source: "seiyaku UnknownType {\nfn read() { let Missing value = 1; }\n}",
        phase: DiagnosticPhase::Resolve,
        code: "K2002",
        message: "unknown type `Missing`",
        line: 2,
    },
    CompileFailCase {
        name: "unknown-call",
        source: "seiyaku UnknownCall {\nfn run() { missing_call(); }\n}",
        phase: DiagnosticPhase::Resolve,
        code: "K2002",
        message: "unknown function or builtin `missing_call`",
        line: 2,
    },
    CompileFailCase {
        name: "builtin-collision",
        source: "seiyaku BuiltinCollision {\nfn account_id(string value) -> int { return 1; }\n}",
        phase: DiagnosticPhase::Resolve,
        code: "E_RESERVED_DECLARATION",
        message: "function `account_id` uses a compiler-reserved name",
        line: 2,
    },
    CompileFailCase {
        name: "implicit-conversion",
        source: "seiyaku ImplicitConversion {\nfn run() { let int value = true; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_TYPE_ANNOTATION_MISMATCH",
        message: "type annotation mismatch: expected int, got bool",
        line: 2,
    },
    CompileFailCase {
        name: "tail-type-mismatch",
        source: "seiyaku TailMismatch {\nfn value() -> bool { 1 }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_TAIL_TYPE_MISMATCH",
        message: "block tail type mismatch: type annotation mismatch: expected bool, got int",
        line: 2,
    },
    CompileFailCase {
        name: "implicit-int-decimal-arithmetic",
        source: "seiyaku ImplicitNumericArithmetic {\nfn add(int whole, decimal fraction) -> decimal { return whole + fraction; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_IMPLICIT_NUMERIC_CONVERSION",
        message: "`int` and `decimal` operands cannot be mixed implicitly; convert the `int` with `decimal::from_int(value)`",
        line: 2,
    },
    CompileFailCase {
        name: "implicit-int-decimal-comparison",
        source: "seiyaku ImplicitNumericComparison {\nfn less(int whole, decimal fraction) -> bool { return whole < fraction; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_IMPLICIT_NUMERIC_CONVERSION",
        message: "`int` and `decimal` operands cannot be mixed implicitly; convert the `int` with `decimal::from_int(value)`",
        line: 2,
    },
    CompileFailCase {
        name: "implicit-int-decimal-equality",
        source: "seiyaku ImplicitNumericEquality {\nfn equal(int whole, decimal fraction) -> bool { return whole == fraction; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_IMPLICIT_NUMERIC_CONVERSION",
        message: "`int` and `decimal` operands cannot be mixed implicitly; convert the `int` with `decimal::from_int(value)`",
        line: 2,
    },
    CompileFailCase {
        name: "implicit-int-decimal-compound-assignment",
        source: "seiyaku ImplicitNumericCompound {\nfn add(int delta) -> decimal { var decimal value = 1.5; value += delta; return value; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_IMPLICIT_NUMERIC_CONVERSION",
        message: "`int` and `decimal` operands cannot be mixed implicitly; convert the `int` with `decimal::from_int(value)`",
        line: 2,
    },
    CompileFailCase {
        name: "implicit-pointer-conversion",
        source: "seiyaku ImplicitPointerConversion {\nfn run(bytes value) { let id = AccountId::parse(value); }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "K2003",
        message: "AccountId::parse expects string",
        line: 2,
    },
    CompileFailCase {
        name: "untyped-parameter",
        source: "seiyaku UntypedParameter {\nfn run(value) {}\n}",
        phase: DiagnosticPhase::Parse,
        code: "K1001",
        message: "expected identifier but found RParen",
        line: 2,
    },
    CompileFailCase {
        name: "missing-kotoage-authorization",
        source: "seiyaku MissingAuthorization {\nkotoage fn run() {}\n}",
        phase: DiagnosticPhase::Parse,
        code: "K1001",
        message: "kotoage function `run` requires `authorize(\"Permission\")` before its body",
        line: 2,
    },
    CompileFailCase {
        name: "retired-parameter-order",
        source: "seiyaku RetiredParameterOrder {\nfn run(value: int) {}\n}",
        phase: DiagnosticPhase::Parse,
        code: "E_RETIRED_DECLARATION_ORDER",
        message: "parameters are type-first: write `int value`",
        line: 2,
    },
    CompileFailCase {
        name: "retired-state-order",
        source: "seiyaku RetiredStateOrder {\nstate value: int;\n}",
        phase: DiagnosticPhase::Parse,
        code: "E_RETIRED_DECLARATION_ORDER",
        message: "state declarations are type-first: write `state int value;`",
        line: 2,
    },
    CompileFailCase {
        name: "retired-const-order",
        source: "seiyaku RetiredConstOrder {\nconst limit: int = 1;\n}",
        phase: DiagnosticPhase::Parse,
        code: "E_RETIRED_DECLARATION_ORDER",
        message: "constants are type-first: write `const int limit = 1;`",
        line: 2,
    },
    CompileFailCase {
        name: "retired-struct-field-order",
        source: "seiyaku RetiredStructFieldOrder {\nstruct Pair { value: int; }\n}",
        phase: DiagnosticPhase::Parse,
        code: "E_RETIRED_DECLARATION_ORDER",
        message: "struct fields are type-first: write `int field;`",
        line: 2,
    },
    CompileFailCase {
        name: "retired-local-order",
        source: "seiyaku RetiredLocalOrder {\nfn run() { let value: int = 1; }\n}",
        phase: DiagnosticPhase::Parse,
        code: "E_RETIRED_DECLARATION_ORDER",
        message: "typed locals are type-first: write `let int value = ...;`",
        line: 2,
    },
    CompileFailCase {
        name: "immutable-assignment",
        source: "seiyaku ImmutableAssignment {\nfn run() { let value = 1; value = 2; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_IMMUTABLE_ASSIGNMENT",
        message: "cannot assign to immutable binding `value`",
        line: 2,
    },
    CompileFailCase {
        name: "unbounded-loop",
        source: "seiyaku UnboundedLoop {\nfn run() { while true {} }\n}",
        phase: DiagnosticPhase::Parse,
        code: "K1001",
        message: "`while` is not supported in Kotodama V1",
        line: 2,
    },
    CompileFailCase {
        name: "dynamic-range-loop",
        source: "seiyaku DynamicRangeLoop {\nfn run(int limit) { for index in range(limit) {} }\n}",
        phase: DiagnosticPhase::Parse,
        code: "E_UNBOUNDED_LOOP",
        message: "numeric range bounds must be non-negative integer literals",
        line: 2,
    },
    CompileFailCase {
        name: "recursion",
        source: "seiyaku Recursion {\nfn recurse() { recurse(); }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "K2006",
        message: "recursive function calls are not supported",
        line: 2,
    },
    CompileFailCase {
        name: "non-ascii-identifier",
        source: "seiyaku Café {\nfn run() {}\n}",
        phase: DiagnosticPhase::Lex,
        code: "K0100",
        message: "non-ASCII identifier outside the branded Japanese keyword set",
        line: 1,
    },
    CompileFailCase {
        name: "english-declaration-keyword",
        source: "contract Alias {\nfn run() {}\n}",
        phase: DiagnosticPhase::Parse,
        code: "K1001",
        message: "exactly one `seiyaku Name",
        line: 1,
    },
    CompileFailCase {
        name: "numeric-prefix-identifier",
        source: "seiyaku 1Invalid {\nfn run() {}\n}",
        phase: DiagnosticPhase::Lex,
        code: "E_RETIRED_NUMERIC_SUFFIX",
        message: "numeric literal suffixes are not part of Kotodama V1",
        line: 1,
    },
    CompileFailCase {
        name: "legacy-macro",
        source: "seiyaku LegacyMacro {\nfn run() { let id = account!(\"alice\"); }\n}",
        phase: DiagnosticPhase::Parse,
        code: "K1001",
        message: "macros are not part of Kotodama V1",
        line: 2,
    },
    CompileFailCase {
        name: "raw-allocation",
        source: "seiyaku RawAllocation {\nfn run() { let pointer = alloc(16); }\n}",
        phase: DiagnosticPhase::Resolve,
        code: "K2002",
        message: "unknown function or builtin `alloc`",
        line: 2,
    },
    CompileFailCase {
        name: "raw-norito-constructor",
        source: "seiyaku RawNorito {\nfn run() { let payload = norito_bytes(b\"opaque\"); }\n}",
        phase: DiagnosticPhase::Resolve,
        code: "K2002",
        message: "unknown function or builtin `norito_bytes`",
        line: 2,
    },
    CompileFailCase {
        name: "raw-blob-constructor",
        source: "seiyaku RawBlob {\nfn run() { let payload = blob(\"opaque\"); }\n}",
        phase: DiagnosticPhase::Resolve,
        code: "K2002",
        message: "unknown function or builtin `blob`",
        line: 2,
    },
    CompileFailCase {
        name: "opaque-instruction-submission",
        source: "seiyaku OpaqueInstruction {\nkotoage fn run(bytes payload) authorize(\"Run\") { execute_instruction(payload); }\n}",
        phase: DiagnosticPhase::Resolve,
        code: "K2002",
        message: "unknown function or builtin `execute_instruction`",
        line: 2,
    },
    CompileFailCase {
        name: "cyclic-value-type",
        source: "seiyaku CyclicValueType {\nstruct Node { Node next; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "K2006",
        message: "cyclic value struct definition: Node -> Node",
        line: 2,
    },
    CompileFailCase {
        name: "retired-positional-struct",
        source: "seiyaku PositionalStruct {\nstruct Pair { int left, int right }\nfn run() { let pair = Pair(1, 2); }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_POSITIONAL_STRUCT",
        message: "positional construction `Pair(...)` is retired",
        line: 3,
    },
    CompileFailCase {
        name: "mixed-call-style",
        source: "seiyaku MixedCall {\nfn target(int first, int second) {}\nfn run() { target(1, second: 2); }\n}",
        phase: DiagnosticPhase::Parse,
        code: "E_MIXED_CALL_ARGUMENTS",
        message: "calls must use either all positional or all named source arguments",
        line: 3,
    },
    CompileFailCase {
        name: "unsafe-list-read",
        source: "seiyaku UnsafeListRead {\nfn read(List<int, 2> values) -> Option<int> { return values[0]; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_LIST_UNSAFE_INDEX",
        message: "unchecked List indexing is not part of Kotodama V1",
        line: 2,
    },
    CompileFailCase {
        name: "unsafe-list-write",
        source: "seiyaku UnsafeListWrite {\nfn write() { var List<int, 2> values = [1]; values[0] = 2; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_LIST_UNSAFE_INDEX",
        message: "unchecked List writes are not part of Kotodama V1",
        line: 2,
    },
    CompileFailCase {
        name: "legacy-option-placeholder",
        source: "seiyaku LegacyOption {\nfn read() -> Option<int> { option::none(0) }\n}",
        phase: DiagnosticPhase::Parse,
        code: "E_LEGACY_SUM_CONSTRUCTOR",
        message: "`option::none` is retired",
        line: 2,
    },
    CompileFailCase {
        name: "named-only-repeated-types",
        source: "seiyaku NamedOnly {\nfn target(int left, int right) {} fn run() { target(1, 2); }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_NAMED_ARGUMENTS_REQUIRED",
        message: "requires named arguments because repeated parameter types",
        line: 2,
    },
    CompileFailCase {
        name: "contextless-option-none",
        source: "seiyaku ContextlessNone {\nfn run() { let value = Option::none; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_SUM_MISSING_CONTEXT",
        message: "`Option::none` requires an exact `Option<T>` context",
        line: 2,
    },
    CompileFailCase {
        name: "option-propagation-context",
        source: "seiyaku PropagationContext {\nfn read(Option<int> value) -> int { value? }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_PROPAGATE_CONTEXT",
        message: "postfix `?` on Option requires an Option-returning function",
        line: 2,
    },
    CompileFailCase {
        name: "non-exhaustive-option-match",
        source: "seiyaku NonExhaustive {\nfn read(Option<int> value) -> int { match value { Option::some(item) => { item }, } }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_MATCH_NON_EXHAUSTIVE",
        message: "match must cover both namespaced variants",
        line: 2,
    },
    CompileFailCase {
        name: "non-exhaustive-result-match",
        source: "seiyaku NonExhaustiveResult {\nfn read(Result<int, bool> value) -> int { match value { Result::ok(item) => { item }, } }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_MATCH_NON_EXHAUSTIVE",
        message: "match must cover both namespaced variants",
        line: 2,
    },
    CompileFailCase {
        name: "list-comprehension-capacity",
        source: "seiyaku ComprehensionCapacity {\nfn copy() { let List<int, 8> source = [1]; let List<int, 4> result = [item for item in source if false]; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_LIST_COMPREHENSION_CAPACITY",
        message: "filters do not reduce the proven maximum",
        line: 2,
    },
    CompileFailCase {
        name: "duplicate-native-json-key",
        source: "seiyaku DuplicateJsonKey {\nfn build() -> Json { json { owner: 1, \"owner\": 2 } }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_JSON_DUPLICATE_KEY",
        message: "native JSON object key `owner` is supplied more than once",
        line: 2,
    },
    CompileFailCase {
        name: "malformed-decimal",
        source: "seiyaku MalformedDecimal {\nfn value() -> decimal { 1. }\n}",
        phase: DiagnosticPhase::Lex,
        code: "E_DECIMAL_MALFORMED",
        message: "decimal literals require at least one digit after `.`",
        line: 2,
    },
    CompileFailCase {
        name: "quantity-constant-underflow",
        source: "seiyaku QuantityUnderflow {\nfn value() -> quantity { 1 - 2 }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_QUANTITY_UNDERFLOW",
        message: "Quantity subtraction would produce a negative result",
        line: 2,
    },
    CompileFailCase {
        name: "implicit-int-to-quantity",
        source: "seiyaku QuantityConversion {\nfn value() { let int count = 10; let quantity result = count; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_TYPE_ANNOTATION_MISMATCH",
        message: "type annotation mismatch: expected quantity, got int",
        line: 2,
    },
    CompileFailCase {
        name: "retired-amount-type",
        source: "seiyaku RejectedAmountType {\nfn value(Amount input) { }\n}",
        phase: DiagnosticPhase::Parse,
        code: "E_RETIRED_NUMERIC_TYPE",
        message: "numeric type `Amount` is not part of Kotodama V1; use `quantity`",
        line: 2,
    },
    CompileFailCase {
        name: "retired-i64-type",
        source: "seiyaku RetiredIntType {\nfn value(i64 input) { }\n}",
        phase: DiagnosticPhase::Parse,
        code: "E_RETIRED_NUMERIC_TYPE",
        message: "numeric type `i64` is not part of Kotodama V1; use `int`",
        line: 2,
    },
    CompileFailCase {
        name: "retired-amount-suffix",
        source: "seiyaku RejectedAmountSuffix {\nfn value() -> quantity { 1.25amt }\n}",
        phase: DiagnosticPhase::Lex,
        code: "E_RETIRED_NUMERIC_SUFFIX",
        message: "numeric literal suffixes are not part of Kotodama V1",
        line: 2,
    },
    CompileFailCase {
        name: "retired-quantity-suffix",
        source: "seiyaku RejectedQuantitySuffix {\nfn value() -> quantity { 1.25qty }\n}",
        phase: DiagnosticPhase::Lex,
        code: "E_RETIRED_NUMERIC_SUFFIX",
        message: "numeric literal suffixes are not part of Kotodama V1",
        line: 2,
    },
    CompileFailCase {
        name: "retired-u128-suffix",
        source: "seiyaku RetiredIntSuffix {\nfn value() -> int { 10u128 }\n}",
        phase: DiagnosticPhase::Lex,
        code: "E_RETIRED_NUMERIC_SUFFIX",
        message: "numeric literal suffixes are not part of Kotodama V1",
        line: 2,
    },
    CompileFailCase {
        name: "retired-scalar-json-setter",
        source: "seiyaku RetiredJsonSetter {\nfn value() -> Json { json::set_i64(json::object(), Name::parse(\"n\"), 1) }\n}",
        phase: DiagnosticPhase::Parse,
        code: "E_RETIRED_NUMERIC_HELPER",
        message: "scalar JSON setters are not part of Kotodama V1",
        line: 2,
    },
    CompileFailCase {
        name: "retired-quantity-json-getter",
        source: "seiyaku RetiredQuantityGetter {\nfn value(Json object, Name key) { object.get_amount(key); }\n}",
        phase: DiagnosticPhase::Parse,
        code: "E_LEGACY_JSON_GETTER",
        message: "legacy numeric JSON getters were retired; use `.get_quantity(key)`",
        line: 2,
    },
    CompileFailCase {
        name: "typed-query-key-mismatch",
        source: "seiyaku QueryKeyMismatch {\nview fn account(bytes raw) { let found = ledger::query::account(raw); }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_QUERY_KEY_TYPE",
        message: "byte-returning core-query compatibility is not part of Kotodama V1",
        line: 2,
    },
    CompileFailCase {
        name: "typed-query-result-mismatch",
        source: "seiyaku QueryResultMismatch {\nview fn account(AccountId id) { let bytes encoded = ledger::query::account(id); }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_QUERY_RESULT_TYPE",
        message: "byte-returning compatibility is not part of Kotodama V1",
        line: 2,
    },
    CompileFailCase {
        name: "argument-register-window",
        source: "seiyaku ArgumentRegisterWindow {\nview fn run((int, int, int, int, int, int, int, int, int, int, int, int, int, int) value) -> int { return value.0; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "K2007",
        message: "requiring 14 flattened argument words; V1 permits at most 13",
        line: 2,
    },
];

#[test]
fn public_session_compile_fail_diagnostics_are_stable() {
    let session = CompilerSession::default();
    let mut failures = Vec::new();

    for case in CASES {
        let source_name = format!("{}.ko", case.name);
        let diagnostics = match session.build(CompileRequest {
            source: case.source,
            source_name: Some(&source_name),
        }) {
            Ok(_) => {
                failures.push(format!("{} unexpectedly compiled", case.name));
                continue;
            }
            Err(diagnostics) => diagnostics,
        };
        let Some(diagnostic) = diagnostics.diagnostics.iter().find(|diagnostic| {
            diagnostic.phase == case.phase
                && diagnostic.code == case.code
                && diagnostic.message.contains(case.message)
        }) else {
            failures.push(format!(
                "{} omitted {} {} containing {:?}: {:#?}",
                case.name,
                case.phase.as_str(),
                case.code,
                case.message,
                diagnostics.diagnostics
            ));
            continue;
        };
        let Some(span) = diagnostic.primary_span.as_ref() else {
            failures.push(format!("{} diagnostic has no primary span", case.name));
            continue;
        };
        if span.source.as_deref() != Some(source_name.as_str()) {
            failures.push(format!(
                "{} source name drifted: {diagnostic:#?}",
                case.name
            ));
        }
        if span.start.line != case.line {
            failures.push(format!(
                "{} source line drifted: {diagnostic:#?}",
                case.name
            ));
        }
        if span.start.column < 1 {
            failures.push(format!(
                "{} span must use one-based columns: {diagnostic:#?}",
                case.name
            ));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn tail_type_mismatch_points_at_the_exact_tail_expression() {
    let source = "seiyaku TailMismatch {\nfn value() -> bool { 1 }\n}";
    let diagnostics = CompilerSession::default()
        .build(CompileRequest {
            source,
            source_name: Some("tail-type-mismatch-exact.ko"),
        })
        .expect_err("the int tail must not satisfy the declared bool result");
    let diagnostic = diagnostics
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.phase == DiagnosticPhase::Semantic
                && diagnostic.code == "E_TAIL_TYPE_MISMATCH"
        })
        .expect("exact tail mismatch diagnostic");

    assert_eq!(
        diagnostic.message,
        "block tail type mismatch: type annotation mismatch: expected bool, got int"
    );
    let span = diagnostic
        .primary_span
        .as_ref()
        .expect("tail mismatch must retain the tail expression span");
    assert_eq!(span.source.as_deref(), Some("tail-type-mismatch-exact.ko"));
    assert_eq!((span.start.line, span.start.column), (2, 22));
    assert_eq!((span.end.line, span.end.column), (2, 23));
    let range = span
        .byte_range
        .expect("tail mismatch must retain the exact byte range");
    let literal = source.rfind('1').expect("tail literal");
    assert_eq!(
        (range.start, range.end),
        (
            u32::try_from(literal).expect("source offset fits u32"),
            u32::try_from(literal + 1).expect("source offset fits u32"),
        )
    );
    assert_eq!(
        &source[range.start as usize..range.end as usize],
        "1",
        "the diagnostic must select only the incompatible tail expression"
    );
}

fn trigger_metadata_contract(value: &str) -> String {
    format!(
        r#"
        seiyaku TriggerMetadata {{
            kotoage fn run() authorize("RunTrigger") {{}}
            trigger wake -> run {{
                on time pre_commit;
                metadata {{ payload: {value}; }}
            }}
        }}
        "#,
    )
}

#[test]
fn public_session_enforces_json_parse_arguments_in_trigger_metadata() {
    let session = CompilerSession::default();
    for value in [r#"Json::parse("{}")"#, r#"Json::parse(value: "{}")"#] {
        let source = trigger_metadata_contract(value);
        session
            .build(CompileRequest {
                source: &source,
                source_name: Some("trigger-json-canonical.ko"),
            })
            .unwrap_or_else(|diagnostics| {
                panic!("canonical trigger metadata `{value}` failed: {diagnostics:#?}")
            });
    }

    for (value, phase, code, message) in [
        (
            r#"Json::parse(raw: "{}")"#,
            DiagnosticPhase::Semantic,
            "E_UNKNOWN_NAMED_ARGUMENT",
            "call `Json::parse` has no parameter named `raw`",
        ),
        (
            r#"Json::parse(value: "{}", value: "{}")"#,
            DiagnosticPhase::Parse,
            "E_DUPLICATE_NAMED_ARGUMENT",
            "named argument `value` is supplied more than once",
        ),
        (
            "Json::parse()",
            DiagnosticPhase::Semantic,
            "K2003",
            "Json::parse expects one argument",
        ),
        (
            r#"Json::parse("{}", "{}")"#,
            DiagnosticPhase::Semantic,
            "K2003",
            "Json::parse expects one argument",
        ),
        (
            r#"json("{}")"#,
            DiagnosticPhase::Resolve,
            "K2002",
            "unknown function or builtin `json`",
        ),
    ] {
        let source = trigger_metadata_contract(value);
        let diagnostics = match session.build(CompileRequest {
            source: &source,
            source_name: Some("trigger-json-invalid.ko"),
        }) {
            Ok(_) => panic!("invalid trigger metadata `{value}` compiled"),
            Err(diagnostics) => diagnostics,
        };
        let diagnostic = diagnostics
            .diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.phase == phase
                    && diagnostic.code == code
                    && diagnostic.message == message
            })
            .unwrap_or_else(|| {
                panic!(
                    "trigger metadata `{value}` omitted {phase:?} {code} {message:?}: {:#?}",
                    diagnostics.diagnostics
                )
            });
        assert!(
            diagnostic.primary_span.is_some(),
            "trigger metadata `{value}` must retain a source span"
        );
    }
}

fn named_type_chain_source(contract: &str, struct_count: usize) -> String {
    assert!(struct_count != 0, "a named-type chain has a product root");
    let mut source = format!("seiyaku {contract} {{\n");
    for index in 0..struct_count {
        if index + 1 == struct_count {
            source.push_str(&format!("struct S{index:03} {{ int value; }}\n"));
        } else {
            source.push_str(&format!(
                "struct S{index:03} {{ S{:03} next; }}\n",
                index + 1
            ));
        }
    }
    source.push_str("view fn run() {}\n}\n");
    source
}

fn with_private_parameter(source: String, declaration: &str) -> String {
    source.replacen(
        "view fn run() {}\n}",
        &format!("{declaration}\nview fn run() {{}}\n}}"),
        1,
    )
}

fn branching_named_type_use_source(contract: &str, repeated_roots: usize) -> String {
    let mut source = format!("seiyaku {contract} {{\n");
    for index in 0..14 {
        source.push_str(&format!(
            "struct S{index:03} {{ S{:03} left; S{:03} right; }}\n",
            index + 1,
            index + 1
        ));
    }
    source.push_str("struct S014 { int value; }\n");
    let repeated = std::iter::repeat_n("S000", repeated_roots)
        .collect::<Vec<_>>()
        .join(", ");
    source.push_str(&format!(
        "fn keep(Option<({repeated})> value) {{}}\nview fn run() {{}}\n}}\n"
    ));
    source
}

fn branching_named_type_expression_source(contract: &str, repeated_roots: usize) -> String {
    let mut source = format!("seiyaku {contract} {{\n");
    for index in 0..14 {
        source.push_str(&format!(
            "struct S{index:03} {{ S{:03} left; S{:03} right; }}\n",
            index + 1,
            index + 1
        ));
    }
    source.push_str("struct S014 { int value; }\nstate StateMap<int, S000> records;\n");
    let repeated = std::iter::repeat_n("records.get(0)", repeated_roots)
        .collect::<Vec<_>>()
        .join(", ");
    source.push_str(&format!(
        "fn infer() {{ let values = ({repeated}); }}\nview fn run() {{}}\n}}\n"
    ));
    source
}

#[test]
fn acyclic_named_type_chain_preserves_the_exact_v1_resolution_boundary() {
    // Expanded depth counts every product wrapper and the terminal scalar. A
    // chain of 255 structs plus `int` is therefore exactly 256 levels.
    let boundary = with_private_parameter(
        named_type_chain_source("DepthBoundary", MAX_NESTING_DEPTH - 1),
        "fn keep(S000 value) {}",
    );
    CompilerSession::default()
        .build(CompileRequest {
            source: &boundary,
            source_name: Some("depth-boundary.ko"),
        })
        .expect("a named type exactly 256 expanded levels deep must compile");

    // Adding one product wrapper produces the required hostile 257-level
    // acyclic chain without relying on syntactic generic nesting.
    let source = named_type_chain_source("DeepAcyclic", MAX_NESTING_DEPTH);

    let diagnostics = CompilerSession::default()
        .build(CompileRequest {
            source: &source,
            source_name: Some("deep-acyclic.ko"),
        })
        .expect_err("a 257-level expanded acyclic named type must fail within the fixed budget");
    let diagnostic = diagnostics
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "K2008")
        .expect("named-type depth diagnostic");

    assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
    assert_eq!(
        diagnostic.message,
        format!(
            "expanded value type `S000` exceeds the V1 nesting limit of {MAX_NESTING_DEPTH} levels"
        )
    );
    let span = diagnostic.primary_span.as_ref().expect("exact type span");
    assert_eq!(span.source.as_deref(), Some("deep-acyclic.ko"));
    assert_eq!(span.start.line, 2);
}

#[test]
fn use_site_wrapper_cannot_hide_an_over_depth_named_type() {
    let source = with_private_parameter(
        named_type_chain_source("WrappedDepth", MAX_NESTING_DEPTH - 1),
        "fn keep(Option<S000> value) {}",
    );
    let diagnostics = CompilerSession::default()
        .build(CompileRequest {
            source: &source,
            source_name: Some("wrapped-depth.ko"),
        })
        .expect_err("a wrapper around a 256-level named type reaches 257 expanded levels");
    let diagnostic = diagnostics
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "K2008")
        .expect("use-site named-type depth diagnostic");

    assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
    assert_eq!(
        diagnostic.message,
        format!(
            "expanded use-site value type exceeds the V1 nesting limit of {MAX_NESTING_DEPTH} levels"
        )
    );
    let span = diagnostic
        .primary_span
        .as_ref()
        .expect("exact use-site span");
    assert_eq!(span.source.as_deref(), Some("wrapped-depth.ko"));
    let range = span.byte_range.expect("exact use-site byte range");
    assert_eq!(
        &source[range.start as usize..range.end as usize],
        "Option<S000>"
    );
}

#[test]
fn repeated_shared_named_type_uses_obey_the_same_expanded_node_budget() {
    // Fourteen branching definitions produce a canonical S000 DAG with 49,151
    // conceptual expanded nodes. Five references plus the Option/tuple wrappers
    // remain below 250,000; a sixth reference exceeds it. Both sources stay
    // tiny, so the test specifically exercises semantic expansion accounting.
    let legitimate = branching_named_type_use_source("SharedUseBoundary", 5);
    CompilerSession::default()
        .build(CompileRequest {
            source: &legitimate,
            source_name: Some("shared-use-boundary.ko"),
        })
        .expect("repeated shared named-type references below the node budget must compile");

    let hostile = branching_named_type_use_source("SharedUseOverflow", 6);
    let diagnostics = CompilerSession::default()
        .build(CompileRequest {
            source: &hostile,
            source_name: Some("shared-use-overflow.ko"),
        })
        .expect_err("repeated shared named-type references must not multiply past the budget");
    let diagnostic = diagnostics
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "K2008")
        .expect("use-site named-type node diagnostic");
    assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
    assert_eq!(
        diagnostic.message,
        format!(
            "expanded use-site value type exceeds the V1 resource limit of {MAX_EXPANDED_TYPE_NODES} type nodes"
        )
    );
    let span = diagnostic
        .primary_span
        .as_ref()
        .expect("exact use-site span");
    assert_eq!(span.source.as_deref(), Some("shared-use-overflow.ko"));
    let range = span.byte_range.expect("exact use-site byte range");
    assert_eq!(
        &hostile[range.start as usize..range.end as usize],
        "Option<(S000, S000, S000, S000, S000, S000)>"
    );
}

#[test]
fn inferred_aggregate_types_cannot_bypass_the_shared_node_budget() {
    let legitimate = branching_named_type_expression_source("InferredSharedBoundary", 5);
    CompilerSession::default()
        .check(CompileRequest {
            source: &legitimate,
            source_name: Some("inferred-shared-boundary.ko"),
        })
        .expect("an inferred shared aggregate below the semantic node budget must check");

    let hostile = branching_named_type_expression_source("InferredSharedOverflow", 6);
    let diagnostics = CompilerSession::default()
        .check(CompileRequest {
            source: &hostile,
            source_name: Some("inferred-shared-overflow.ko"),
        })
        .expect_err("inferred aggregates must use the same expanded-shape budget");
    let diagnostic = diagnostics
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "K2008")
        .expect("inferred use-site named-type node diagnostic");
    assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
    assert_eq!(
        diagnostic.message,
        format!(
            "expanded use-site value type exceeds the V1 resource limit of {MAX_EXPANDED_TYPE_NODES} type nodes"
        )
    );
    let span = diagnostic
        .primary_span
        .as_ref()
        .expect("exact inferred expression span");
    assert_eq!(span.source.as_deref(), Some("inferred-shared-overflow.ko"));
    let range = span.byte_range.expect("exact inferred byte range");
    assert_eq!(
        &hostile[range.start as usize..range.end as usize],
        "(records.get(0), records.get(0), records.get(0), records.get(0), records.get(0), records.get(0))"
    );
}

#[test]
fn modest_shared_named_type_dag_compiles_below_the_node_budget() {
    let mut source = String::from("seiyaku ModestDag {\n");
    for index in 0..8 {
        source.push_str(&format!(
            "struct S{index:03} {{ S{:03} left; S{:03} right; }}\n",
            index + 1,
            index + 1
        ));
    }
    source.push_str("struct S008 { int value; }\nview fn run() {}\n}\n");

    CompilerSession::default()
        .build(CompileRequest {
            source: &source,
            source_name: Some("modest-dag.ko"),
        })
        .expect("a shared DAG whose expanded form is below the node budget must compile");
}

#[test]
fn branching_named_type_dag_is_measured_without_exponential_expansion() {
    let mut source = String::from("seiyaku BranchingDag {\n");
    for index in 0..17 {
        source.push_str(&format!(
            "struct S{index:03} {{ S{:03} left; S{:03} right; }}\n",
            index + 1,
            index + 1
        ));
    }
    source.push_str("struct S017 { int value; }\n}\n");

    let diagnostics = CompilerSession::default()
        .build(CompileRequest {
            source: &source,
            source_name: Some("branching-dag.ko"),
        })
        .expect_err("an exponentially expanding named-type DAG must fail before materialization");
    let diagnostic = diagnostics
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "K2008")
        .expect("named-type resource diagnostic");

    assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
    assert_eq!(
        diagnostic.message,
        format!(
            "expanded value type `S000` exceeds the V1 resource limit of {MAX_EXPANDED_TYPE_NODES} type nodes"
        )
    );
    let span = diagnostic.primary_span.as_ref().expect("exact type span");
    assert_eq!(span.source.as_deref(), Some("branching-dag.ko"));
    assert_eq!(span.start.line, 2);
}

#[test]
fn over_budget_named_types_point_at_parameter_and_return_references() {
    for (source_name, declaration) in [
        ("oversized-param.ko", "view fn inspect(S000 value) {}"),
        ("oversized-return.ko", "view fn inspect() -> S000 {}"),
    ] {
        let mut source = String::from("seiyaku LocatedBudget {\n");
        for index in 0..17 {
            source.push_str(&format!(
                "struct S{index:03} {{ S{:03} left; S{:03} right; }}\n",
                index + 1,
                index + 1
            ));
        }
        source.push_str("struct S017 { int value; }\n");
        source.push_str(declaration);
        source.push_str("\n}\n");

        let diagnostics = CompilerSession::default()
            .build(CompileRequest {
                source: &source,
                source_name: Some(source_name),
            })
            .expect_err("the conceptual expanded shape exceeds the fixed V1 node budget");
        let diagnostic = diagnostics
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "K2008")
            .expect("located named-type resource diagnostic");
        let span = diagnostic.primary_span.as_ref().expect("exact use span");
        assert_eq!(span.source.as_deref(), Some(source_name));
        assert_eq!(span.start.line, 20);
        let range = span.byte_range.expect("exact type byte range");
        let start = usize::try_from(range.start).expect("source offset fits usize");
        let end = usize::try_from(range.end).expect("source offset fits usize");
        assert_eq!(&source[start..end], "S000");
    }
}

#[test]
fn multi_error_renderers_preserve_identical_semantic_records_and_exact_spans() {
    let source = r#"seiyaku Broken {
  fn first() { let quantity total = true; }
  fn second() { let value = 1; value = 2; }
}"#;
    let source_name = "multi-error-renderers.ko";
    let diagnostics = CompilerSession::default()
        .build(CompileRequest {
            source,
            source_name: Some(source_name),
        })
        .expect_err("independent semantic errors must fail compilation");

    for code in ["E_TYPE_ANNOTATION_MISMATCH", "E_IMMUTABLE_ASSIGNMENT"] {
        assert!(
            diagnostics
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code),
            "missing {code}: {diagnostics:#?}"
        );
    }
    assert!(
        diagnostics.diagnostics.len() >= 2,
        "all independent errors must be retained: {diagnostics:#?}"
    );

    let human = diagnostics.render_human();
    for diagnostic in &diagnostics.diagnostics {
        assert!(
            human.contains(&format!(
                "{}[{}] {}: {}",
                diagnostic.severity.as_str(),
                diagnostic.code,
                diagnostic.phase.as_str(),
                diagnostic.message
            )),
            "human output omitted the canonical header for {}: {human}",
            diagnostic.code,
        );
        let span = diagnostic
            .primary_span
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no primary span", diagnostic.code));
        assert_eq!(span.source.as_deref(), Some(source_name));
        let range = span
            .byte_range
            .unwrap_or_else(|| panic!("{} has no exact byte range", diagnostic.code));
        let start = usize::try_from(range.start).expect("byte offset fits usize");
        let end = usize::try_from(range.end).expect("byte offset fits usize");
        assert!(
            start < end && end <= source.len(),
            "{} has invalid byte range {start}..{end}",
            diagnostic.code
        );
        assert!(
            human.contains(&format!(
                "{source_name}:{}:{}-{}:{}",
                span.start.line, span.start.column, span.end.line, span.end.column
            )),
            "human output omitted the exact span for {}: {human}",
            diagnostic.code
        );
        for label in &diagnostic.labels {
            assert!(
                human.contains(&label.message),
                "human output omitted a label for {}: {human}",
                diagnostic.code
            );
        }
        for note in &diagnostic.notes {
            assert!(
                human.contains(note),
                "human output omitted a note for {}: {human}",
                diagnostic.code
            );
        }
        if let Some(help) = &diagnostic.help {
            assert!(
                human.contains(help),
                "human output omitted help for {}: {human}",
                diagnostic.code
            );
        }
        if let Some(fix) = &diagnostic.fix {
            assert!(
                human.contains(&fix.replacement),
                "human output omitted the fix for {}: {human}",
                diagnostic.code
            );
        }
    }

    let canonical: norito::json::Value = norito::json::from_str(
        &diagnostics
            .render_json()
            .expect("render canonical JSON diagnostics"),
    )
    .expect("decode canonical JSON diagnostics");
    let sarif: norito::json::Value = norito::json::from_str(
        &diagnostics
            .render_sarif()
            .expect("render canonical SARIF diagnostics"),
    )
    .expect("decode canonical SARIF diagnostics");
    let canonical_records = canonical.as_array().expect("canonical diagnostic array");
    let sarif_results = sarif
        .pointer("/runs/0/results")
        .and_then(norito::json::Value::as_array)
        .expect("SARIF result array");
    assert_eq!(canonical_records.len(), sarif_results.len());
    for (canonical_record, sarif_result) in canonical_records.iter().zip(sarif_results) {
        assert_eq!(
            sarif_result
                .pointer("/properties/kotodama")
                .expect("SARIF embeds the canonical Kotodama record"),
            canonical_record,
            "JSON and SARIF semantic fields diverged"
        );
    }
}
