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
include!(concat!(env!("OUT_DIR"), "/kotodama_compile_fail_cases.rs"));
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
            const string dynamic = "{{}}";
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
            "Json::parse(value: dynamic)",
            DiagnosticPhase::Semantic,
            "E_JSON_LITERAL_REQUIRED",
            "Json::parse requires a direct string literal so native JSON is validated at compile time",
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
