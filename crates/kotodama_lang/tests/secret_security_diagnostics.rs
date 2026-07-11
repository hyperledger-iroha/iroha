//! Public-session security diagnostics for Kotodama V1 `Secret<T>` flows.

use kotodama_lang::{
    compiler::{CompilerOptions, DEFAULT_MAX_CYCLES},
    diagnostic::{DiagnosticBundle, DiagnosticPhase, Severity},
    session::{CompileRequest, CompilerSession},
};
use norito::json::{self, Value};

const SOURCE_NAME: &str = "security/secret-flow.ko";

#[derive(Clone, Copy)]
struct RejectCase {
    name: &'static str,
    source: &'static str,
    code: &'static str,
    primary: &'static str,
}

const REJECT_CASES: &[RejectCase] = &[
    RejectCase {
        name: "direct-secret-control-flow",
        source: r#"seiyaku Privacy {
    kotoage fn direct() authorize("UsePrivacy") {
        if crypto::private_input(0) { return; }
    }
}"#,
        code: "E_SECRET_CONTROL_FLOW",
        primary: "if crypto::private_input(0) { return; }",
    },
    RejectCase {
        name: "helper-transitive-secret-control-flow",
        source: r#"seiyaku Privacy {
    fn relay(value: Secret<i64>) -> Secret<i64> { return value; }
    kotoage fn transitive() authorize("UsePrivacy") {
        let value: Secret<i64> = crypto::private_input(0);
        if relay(value) { return; }
    }
}"#,
        code: "E_SECRET_CONTROL_FLOW",
        primary: "if relay(value) { return; }",
    },
    RejectCase {
        name: "public-secret-return",
        source: r#"seiyaku Privacy {
    kotoage fn expose() -> Secret<i64> authorize("UsePrivacy") {
        return crypto::private_input(0);
    }
}"#,
        code: "E_SECRET_PUBLIC_RETURN",
        primary: "Secret<i64>",
    },
    RejectCase {
        name: "public-secret-parameter",
        source: r#"seiyaku Privacy {
    kotoage fn accept(value: Secret<i64>) authorize("UsePrivacy") {}
}"#,
        code: "E_SECRET_PUBLIC_PARAMETER",
        primary: "Secret<i64>",
    },
    RejectCase {
        name: "secret-log",
        source: r#"seiyaku Privacy {
    kotoage fn log_secret() authorize("UsePrivacy") {
        debug::info(crypto::private_input(0));
    }
}"#,
        code: "E_SECRET_LOG",
        primary: "debug::info(crypto::private_input(0))",
    },
    RejectCase {
        name: "secret-bearing-durable-state-type",
        source: r#"seiyaku Privacy {
    state leaked: Secret<i64>;
    hajimari() {}
}"#,
        code: "E_SECRET_STATE_TYPE",
        primary: "Secret<i64>",
    },
    RejectCase {
        name: "secret-durable-state-key",
        source: r#"seiyaku Privacy {
    state values: StateMap<i64, i64>;
    kotoage fn write() authorize("UsePrivacy") {
        values[crypto::private_input(0)] = 1;
    }
}"#,
        code: "E_SECRET_STATE_KEY",
        primary: "values[crypto::private_input(0)] = 1;",
    },
    RejectCase {
        name: "secret-durable-state-value",
        source: r#"seiyaku Privacy {
    state values: StateMap<i64, i64>;
    kotoage fn write() authorize("UsePrivacy") {
        values[0] = crypto::private_input(0);
    }
}"#,
        code: "E_SECRET_STATE_WRITE",
        primary: "values[0] = crypto::private_input(0);",
    },
    RejectCase {
        name: "secret-raw-state-host-sink",
        source: r#"seiyaku Privacy {
    kotoage fn write() authorize("UsePrivacy") {
        state::set(path: crypto::private_input(0), value: 1);
    }
}"#,
        code: "E_SECRET_STATE_SINK",
        primary: "state::set(path: crypto::private_input(0), value: 1)",
    },
    RejectCase {
        name: "secret-ledger-query-sink",
        source: r#"seiyaku Privacy {
    kotoage fn query() authorize("UsePrivacy") {
        let result = ledger::query::account(id: crypto::private_input(0));
    }
}"#,
        code: "E_SECRET_HOST_SINK",
        primary: "ledger::query::account(id: crypto::private_input(0))",
    },
    RejectCase {
        name: "secret-ledger-write-sink",
        source: r#"seiyaku Privacy {
    kotoage fn register() authorize("UsePrivacy") {
        ledger::domain::register(domain: crypto::private_input(0));
    }
}"#,
        code: "E_SECRET_HOST_SINK",
        primary: "ledger::domain::register(domain: crypto::private_input(0))",
    },
    RejectCase {
        name: "raw-secret-nullifier-disclosure",
        source: r#"seiyaku Privacy {
    kotoage fn disclose() authorize("UsePrivacy") {
        crypto::use_nullifier(crypto::private_input(0));
    }
}"#,
        code: "E_SECRET_NULLIFIER_DISCLOSURE",
        primary: "crypto::use_nullifier(crypto::private_input(0))",
    },
    RejectCase {
        name: "secret-private-input-index",
        source: r#"seiyaku Privacy {
    kotoage fn dynamic_index() authorize("UsePrivacy") {
        let index: Secret<i64> = crypto::private_input(0);
        let value = crypto::private_input(index);
    }
}"#,
        code: "E_SECRET_PRIVATE_INPUT_INDEX",
        primary: "crypto::private_input(index)",
    },
    RejectCase {
        name: "mixed-public-secret-commitment",
        source: r#"seiyaku Privacy {
    kotoage fn weak_commitment() authorize("UsePrivacy") {
        let value: Secret<i64> = crypto::private_input(0);
        let commitment = crypto::valcom(left: value, right: 7);
    }
}"#,
        code: "E_SECRET_MIXED_COMMITMENT",
        primary: "crypto::valcom(left: value, right: 7)",
    },
];

fn zk_session() -> CompilerSession {
    CompilerSession::new(CompilerOptions {
        force_zk: true,
        max_cycles: DEFAULT_MAX_CYCLES,
        ..CompilerOptions::default()
    })
}

fn request(source: &str) -> CompileRequest<'_> {
    CompileRequest {
        source,
        source_name: Some(SOURCE_NAME),
    }
}

fn reject_with_check_and_build(case: RejectCase) -> DiagnosticBundle {
    let session = zk_session();
    let check = session
        .check(request(case.source))
        .expect_err("security-invalid source must fail `check`");
    let build = session
        .build(request(case.source))
        .expect_err("security-invalid source must fail `build`");
    assert_eq!(
        check, build,
        "{}: check/build diagnostics diverged",
        case.name
    );
    check
}

fn assert_rejection(case: RejectCase, bundle: &DiagnosticBundle) {
    assert_eq!(bundle.diagnostics.len(), 1, "{}: {bundle:?}", case.name);
    let diagnostic = &bundle.diagnostics[0];
    assert_eq!(diagnostic.code, case.code, "{}: {diagnostic:?}", case.name);
    assert_eq!(
        diagnostic.phase,
        DiagnosticPhase::Semantic,
        "{}: {diagnostic:?}",
        case.name
    );
    assert_eq!(diagnostic.severity, Severity::Error);
    assert!(
        !diagnostic.message.starts_with(case.code)
            && !diagnostic.message.starts_with(&format!("[{}]", case.code))
            && !diagnostic.message.contains(&format!("{}:", case.code)),
        "{}: diagnostic message embeds its stable code: {:?}",
        case.name,
        diagnostic.message
    );

    let span = diagnostic
        .primary_span
        .as_ref()
        .unwrap_or_else(|| panic!("{}: missing primary span: {diagnostic:?}", case.name));
    assert_eq!(span.source.as_deref(), Some(SOURCE_NAME), "{}", case.name);
    let range = span
        .byte_range
        .unwrap_or_else(|| panic!("{}: missing primary byte range", case.name));
    assert!(range.start < range.end, "{}: empty span", case.name);
    let start = usize::try_from(range.start).expect("source offset fits usize");
    let end = usize::try_from(range.end).expect("source offset fits usize");
    assert_eq!(
        &case.source[start..end],
        case.primary,
        "{}: primary span must identify the security-relevant construct",
        case.name
    );
    let expected_start = case
        .source
        .find(case.primary)
        .unwrap_or_else(|| panic!("{}: primary fixture text is absent", case.name));
    assert_eq!(
        (start, end),
        (expected_start, expected_start + case.primary.len()),
        "{}: primary span is not the exact intended occurrence",
        case.name
    );
}

#[test]
fn compiler_session_rejects_every_public_secret_flow_with_exact_diagnostics() {
    for case in REJECT_CASES.iter().copied() {
        let bundle = reject_with_check_and_build(case);
        assert_rejection(case, &bundle);
    }
}

#[test]
fn approved_all_secret_commitment_checks_and_builds() {
    let source = r#"seiyaku Privacy {
    hajimari() {}
    kaizen() {}
    kotoage fn commitment() -> i64 authorize("UsePrivacy") {
        let value: Secret<i64> = crypto::private_input(0);
        let blinding: Secret<i64> = crypto::private_input(1);
        return crypto::valcom(left: value, right: blinding);
    }
}"#;
    let session = zk_session();
    session
        .check(request(source))
        .expect("an all-secret approved commitment must pass `check`");
    let output = session
        .build(request(source))
        .expect("an all-secret approved commitment must produce a deployable artifact");
    assert!(!output.artifact.is_empty());
    assert_eq!(output.manifest.seiyaku_name.as_deref(), Some("Privacy"));
}

#[test]
fn human_json_and_sarif_preserve_the_same_secret_security_record() {
    let case = REJECT_CASES[0];
    let bundle = reject_with_check_and_build(case);
    assert_rejection(case, &bundle);
    let diagnostic = &bundle.diagnostics[0];
    let canonical = diagnostic.to_json_value();

    let rendered_json = bundle.render_json().expect("render JSON diagnostic");
    let json_value: Value = json::from_str(&rendered_json).expect("parse JSON diagnostic");
    assert_eq!(json_value[0], canonical);

    let rendered_sarif = bundle.render_sarif().expect("render SARIF diagnostic");
    let sarif_value: Value = json::from_str(&rendered_sarif).expect("parse SARIF diagnostic");
    assert_eq!(
        sarif_value["runs"][0]["results"][0]["properties"]["kotodama"],
        canonical
    );

    let human = bundle.render_human();
    assert!(human.contains(&format!("error[{}] semantic", diagnostic.code)));
    assert!(human.contains(&diagnostic.message));
    assert!(human.contains(SOURCE_NAME));
    let range = diagnostic
        .primary_span
        .as_ref()
        .and_then(|span| span.byte_range)
        .expect("representative diagnostic has bytes");
    assert!(human.contains(&format!("[bytes {}..{}]", range.start, range.end)));
}
