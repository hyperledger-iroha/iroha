//! Public compiler-session compile-fail diagnostic goldens for Kotodama V1.

use kotodama_lang::{
    diagnostic::DiagnosticPhase,
    session::{CompileRequest, CompilerSession},
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
        phase: DiagnosticPhase::Semantic,
        code: "K2001",
        message: "declaration `repeated` duplicates an earlier declaration",
        line: 3,
    },
    CompileFailCase {
        name: "unknown-name",
        source: "seiyaku UnknownName {\nfn read() -> i64 { return missing_value; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "K2002",
        message: "unknown name `missing_value`",
        line: 2,
    },
    CompileFailCase {
        name: "unknown-type",
        source: "seiyaku UnknownType {\nfn read() { let value: Missing = 1; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "K2002",
        message: "unknown type `Missing`",
        line: 2,
    },
    CompileFailCase {
        name: "unknown-call",
        source: "seiyaku UnknownCall {\nfn run() { missing_call(); }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "K2002",
        message: "unknown function or builtin `missing_call`",
        line: 2,
    },
    CompileFailCase {
        name: "builtin-collision",
        source: "seiyaku BuiltinCollision {\nfn account_id(value: string) -> i64 { return 1; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_RESERVED_DECLARATION",
        message: "function `account_id` uses a compiler-reserved name",
        line: 2,
    },
    CompileFailCase {
        name: "implicit-conversion",
        source: "seiyaku ImplicitConversion {\nfn run() { let value: i64 = true; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "K2003",
        message: "binding initializer has type `bool`, expected `i64`; implicit conversions are not part of Kotodama V1",
        line: 2,
    },
    CompileFailCase {
        name: "implicit-pointer-conversion",
        source: "seiyaku ImplicitPointerConversion {\nfn run(value: bytes) { let id = AccountId::parse(value); }\n}",
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
        message: "expected Colon",
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
        source: "seiyaku DynamicRangeLoop {\nfn run(limit: i64) { for index in range(limit) {} }\n}",
        phase: DiagnosticPhase::Parse,
        code: "K1001",
        message: "E_UNBOUNDED_LOOP",
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
        message: "non-ASCII character",
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
        phase: DiagnosticPhase::Parse,
        code: "K1001",
        message: "expected identifier but found Number(1)",
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
        phase: DiagnosticPhase::Semantic,
        code: "E_INTERNAL_BUILTIN",
        message: "builtin `alloc` is compiler-internal",
        line: 2,
    },
    CompileFailCase {
        name: "raw-norito-constructor",
        source: "seiyaku RawNorito {\nfn run() { let payload = norito_bytes(b\"opaque\"); }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_INTERNAL_BUILTIN",
        message: "builtin `norito_bytes` is compiler-internal",
        line: 2,
    },
    CompileFailCase {
        name: "raw-blob-constructor",
        source: "seiyaku RawBlob {\nfn run() { let payload = blob(\"opaque\"); }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "E_INTERNAL_BUILTIN",
        message: "builtin `blob` is compiler-internal",
        line: 2,
    },
    CompileFailCase {
        name: "opaque-instruction-submission",
        source: "seiyaku OpaqueInstruction {\nkotoage fn run(payload: bytes) authorize(\"Run\") { execute_instruction(payload); }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "K2002",
        message: "unknown function or builtin `execute_instruction`",
        line: 2,
    },
    CompileFailCase {
        name: "cyclic-value-type",
        source: "seiyaku CyclicValueType {\nstruct Node { next: Node; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "K2006",
        message: "recursive value type cycle: Node",
        line: 2,
    },
    CompileFailCase {
        name: "argument-register-window",
        source: "seiyaku ArgumentRegisterWindow {\nview fn run(value: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)) -> i64 { return value.0; }\n}",
        phase: DiagnosticPhase::Semantic,
        code: "K2007",
        message: "requires 14 flattened argument words, exceeding the Kotodama V1 limit of 13",
        line: 2,
    },
];

#[test]
fn public_session_compile_fail_diagnostics_are_stable() {
    let session = CompilerSession::default();

    for case in CASES {
        let source_name = format!("{}.ko", case.name);
        let diagnostics = match session.build(CompileRequest {
            source: case.source,
            source_name: Some(&source_name),
        }) {
            Ok(_) => panic!("{} unexpectedly compiled", case.name),
            Err(diagnostics) => diagnostics,
        };
        let diagnostic = diagnostics
            .diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.phase == case.phase
                    && diagnostic.code == case.code
                    && diagnostic.message.contains(case.message)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} omitted {} {} containing {:?}: {:#?}",
                    case.name,
                    case.phase.as_str(),
                    case.code,
                    case.message,
                    diagnostics.diagnostics
                )
            });
        let span = diagnostic
            .primary_span
            .as_ref()
            .unwrap_or_else(|| panic!("{} diagnostic has no primary span", case.name));
        assert_eq!(
            span.source.as_deref(),
            Some(source_name.as_str()),
            "{} source name drifted",
            case.name
        );
        assert_eq!(
            span.start.line, case.line,
            "{} source line drifted: {diagnostic:#?}",
            case.name
        );
        assert!(
            span.start.column >= 1,
            "{} span must use one-based columns: {diagnostic:#?}",
            case.name
        );
    }
}

#[test]
fn multi_error_renderers_preserve_identical_semantic_records_and_exact_spans() {
    let source = r#"seiyaku Broken {
  fn first(value: Missing) -> i64 { let amount: Amount = 1; return missing_value; }
  fn second(flag: bool) { flag = false; missing_call(); }
}"#;
    let source_name = "multi-error-renderers.ko";
    let diagnostics = CompilerSession::default()
        .build(CompileRequest {
            source,
            source_name: Some(source_name),
        })
        .expect_err("independent semantic errors must fail compilation");

    for code in ["K2002", "K2003", "E_IMMUTABLE_ASSIGNMENT"] {
        assert!(
            diagnostics
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code),
            "missing {code}: {diagnostics:#?}"
        );
    }
    assert!(
        diagnostics.diagnostics.len() >= 5,
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
