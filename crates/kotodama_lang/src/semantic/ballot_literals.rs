//! Shared validation for source literals in the inline ballot builder.
//!
//! Semantic analysis validates direct literals and declared constants and rejects direct external
//! runtime parameters. Code generation also applies these checks after local bindings and private
//! calls have been specialized.

use super::{SemanticError, sem_err};

/// Validate the governance selector carried by an inline ballot.
pub(crate) fn election_id(raw: &str) -> Result<(), SemanticError> {
    if iroha_data_model::governance::is_valid_governance_selector_v1(raw) {
        Ok(())
    } else {
        Err(sem_err(
            "E_BALLOT_ELECTION_ID",
            "build_submit_ballot_inline election_id must be 1-128 RFC 3986 unreserved ASCII characters and must not start with a dot".into(),
        ))
    }
}

/// Require the exact nullifier width encoded by `SubmitBallot`.
pub(crate) fn nullifier(raw: &[u8]) -> Result<[u8; 32], SemanticError> {
    raw.try_into().map_err(|_| {
        sem_err(
            "E_BALLOT_NULLIFIER",
            "build_submit_ballot_inline nullifier must be 32 bytes".into(),
        )
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        diagnostic::DiagnosticPhase,
        session::{CompileRequest, CompilerSession},
    };

    fn source(selector: &str, nullifier: &str, constants: bool) -> String {
        let (declarations, election, nullifier) = if constants {
            (
                format!(
                    "const string ELECTION = \"{selector}\"; const bytes NULLIFIER = b\"{nullifier}\";"
                ),
                "ELECTION".to_owned(),
                "NULLIFIER".to_owned(),
            )
        } else {
            (
                String::new(),
                format!("\"{selector}\""),
                format!("b\"{nullifier}\""),
            )
        };
        format!(
            r#"seiyaku Ballot {{
    permission Submit;
    {declarations}
    kotoage fn run() authorize(Submit) {{
        let ballot = ledger::governance::build_submit_ballot(
            election_id: {election},
            ciphertext: b"ciphertext",
            nullifier: {nullifier},
            backend: "halo2/ipa",
            proof: b"proof",
            verification_key: b"key",
        );
        ledger::governance::submit_ballot(value: ballot);
    }}
}}"#
        )
    }

    #[test]
    fn inline_ballot_runtime_parameters_fail_check_and_build_at_the_argument() {
        let session = CompilerSession::default();
        let nullifier = format!("b\"{}\"", "\\x00".repeat(32));
        let fields = [
            ("election_id", "string", "\"election\""),
            ("ciphertext", "bytes", "b\"ciphertext\""),
            ("nullifier", "bytes", nullifier.as_str()),
            ("backend", "string", "\"halo2/ipa\""),
            ("proof", "bytes", "b\"proof\""),
            ("verification_key", "bytes", "b\"key\""),
        ];
        for (runtime_index, (label, ty, _)) in fields.iter().enumerate() {
            for reordered in [false, true] {
                let mut arguments = fields
                    .iter()
                    .enumerate()
                    .map(|(index, (label, _, literal))| {
                        format!(
                            "{label}: {}",
                            if index == runtime_index {
                                "runtime_value"
                            } else {
                                literal
                            }
                        )
                    })
                    .collect::<Vec<_>>();
                if reordered {
                    arguments.reverse();
                }
                let source = format!(
                    "seiyaku RuntimeBallot {{ permission Submit; \
                    kotoage fn run({ty} runtime_value) authorize(Submit) {{ \
                        let ballot = ledger::governance::build_submit_ballot({}); \
                        ledger::governance::submit_ballot(value: ballot); \
                    }} }}",
                    arguments.join(", ")
                );
                let request = CompileRequest {
                    source: &source,
                    source_name: Some("runtime_ballot.ko"),
                };
                let built = session
                    .build(request)
                    .expect_err("runtime ballot cannot build");
                let checked = session
                    .check(request)
                    .expect_err("check must reject runtime ballot input");
                assert_eq!(checked, built, "{label}, reordered={reordered}");
                assert_eq!(checked.diagnostics.len(), 1, "{checked:?}");
                let diagnostic = &checked.diagnostics[0];
                assert_eq!(diagnostic.code, "E_BALLOT_LITERAL_REQUIRED");
                assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
                assert!(diagnostic.message.contains(label));
                assert!(diagnostic.help.is_some());
                let span = diagnostic
                    .primary_span
                    .as_ref()
                    .expect("argument source span");
                assert_eq!(span.source.as_deref(), Some("runtime_ballot.ko"));
                let range = span.byte_range.expect("exact argument byte range");
                assert_eq!(
                    &source[range.start as usize..range.end as usize],
                    "runtime_value"
                );
            }
        }
    }

    #[test]
    fn inline_ballot_all_literal_fields_survive_local_and_private_specialization() {
        let session = CompilerSession::default();
        let nullifier = "\\x00".repeat(32);
        let nullifier_literal = format!("b\"{nullifier}\"");
        let fields = [
            ("election_id", "string", "\"election\""),
            ("ciphertext", "bytes", "b\"ciphertext\""),
            ("nullifier", "bytes", nullifier_literal.as_str()),
            ("backend", "string", "\"halo2/ipa\""),
            ("proof", "bytes", "b\"proof\""),
            ("verification_key", "bytes", "b\"key\""),
        ];
        let mut bound = source("election", &nullifier, false);
        for (label, _, literal) in fields {
            bound = bound.replace(&format!("{label}: {literal}"), &format!("{label}: {label}"));
        }
        let locals = fields
            .iter()
            .map(|(label, _, literal)| format!("let {label} = {literal};"))
            .collect::<Vec<_>>()
            .join(" ");
        let local = bound.replace("let ballot =", &format!("{locals} let ballot ="));
        let parameters = fields
            .iter()
            .map(|(label, ty, _)| format!("{ty} {label}"))
            .collect::<Vec<_>>()
            .join(", ");
        let arguments = fields
            .iter()
            .map(|(label, _, literal)| format!("{label}: {literal}"))
            .collect::<Vec<_>>()
            .join(", ");
        let helper = bound.replace(
            "kotoage fn run() authorize(Submit)",
            &format!("fn stage({parameters})"),
        );
        let private = format!(
            "{} kotoage fn run() authorize(Submit) {{ stage({arguments}); }} }}",
            helper
                .strip_suffix('}')
                .expect("fixture contract terminator")
        );
        for source in [&local, &private] {
            let request = CompileRequest {
                source,
                source_name: Some("specialized_ballot.ko"),
            };
            session.check(request).expect("literal bindings must check");
            session.build(request).expect("literal bindings must build");
        }
    }

    #[test]
    fn inline_ballot_runtime_checks_preserve_type_and_literal_error_priority() {
        let session = CompilerSession::default();
        let nullifier = "\\x00".repeat(32);
        let wrong_type = source("election", &nullifier, false)
            .replace("kotoage fn run()", "kotoage fn run(int runtime_value)")
            .replace("proof: b\"proof\"", "proof: runtime_value");
        let invalid_selector = source("bad/selector", &nullifier, false)
            .replace("kotoage fn run()", "kotoage fn run(bytes runtime_value)")
            .replace("ciphertext: b\"ciphertext\"", "ciphertext: runtime_value");
        for (source, code) in [
            (wrong_type, "K2003"),
            (invalid_selector, "E_BALLOT_ELECTION_ID"),
        ] {
            let request = CompileRequest {
                source: &source,
                source_name: Some("invalid_ballot.ko"),
            };
            let checked = session
                .check(request)
                .expect_err("invalid ballot must fail");
            let built = session
                .build(request)
                .expect_err("invalid ballot must fail");
            assert_eq!(checked, built);
            assert_eq!(checked.diagnostics.len(), 1, "{checked:?}");
            assert_eq!(checked.diagnostics[0].code, code);
        }
    }

    #[test]
    fn inline_ballot_check_and_build_reject_invalid_literals_identically() {
        let session = CompilerSession::default();
        let overlong = "a".repeat(129);
        let valid_nullifier = "\\x00".repeat(32);
        let short_nullifier = "a".repeat(31);
        let long_nullifier = "a".repeat(33);
        let cases = [
            ("", valid_nullifier.as_str(), "E_BALLOT_ELECTION_ID"),
            (".hidden", valid_nullifier.as_str(), "E_BALLOT_ELECTION_ID"),
            ("a/b", valid_nullifier.as_str(), "E_BALLOT_ELECTION_ID"),
            ("a%2Fb", valid_nullifier.as_str(), "E_BALLOT_ELECTION_ID"),
            ("a b", valid_nullifier.as_str(), "E_BALLOT_ELECTION_ID"),
            ("投票", valid_nullifier.as_str(), "E_BALLOT_ELECTION_ID"),
            (
                overlong.as_str(),
                valid_nullifier.as_str(),
                "E_BALLOT_ELECTION_ID",
            ),
            ("election", "", "E_BALLOT_NULLIFIER"),
            ("election", short_nullifier.as_str(), "E_BALLOT_NULLIFIER"),
            ("election", long_nullifier.as_str(), "E_BALLOT_NULLIFIER"),
        ];
        for constants in [false, true] {
            for (selector, nullifier, code) in cases {
                let source = source(selector, nullifier, constants);
                let request = CompileRequest {
                    source: &source,
                    source_name: Some("ballot.ko"),
                };
                let checked = session
                    .check(request)
                    .expect_err("check must reject the malformed ballot literal");
                let built = session
                    .build(request)
                    .expect_err("build must reject the malformed ballot literal");
                assert_eq!(checked, built, "constants={constants}, code={code}");
                assert_eq!(checked.diagnostics.len(), 1, "{checked:?}");
                let diagnostic = &checked.diagnostics[0];
                assert_eq!(diagnostic.code, code);
                assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
                assert!(
                    diagnostic.help.is_some(),
                    "literal errors must be explainable"
                );
                let span = diagnostic
                    .primary_span
                    .as_ref()
                    .expect("argument source span");
                assert_eq!(span.source.as_deref(), Some("ballot.ko"));
                let range = span.byte_range.expect("exact argument byte range");
                let argument = &source[range.start as usize..range.end as usize];
                let expected = match (constants, code) {
                    (true, "E_BALLOT_ELECTION_ID") => "ELECTION".to_owned(),
                    (true, _) => "NULLIFIER".to_owned(),
                    (false, "E_BALLOT_ELECTION_ID") => format!("\"{selector}\""),
                    (false, _) => format!("b\"{nullifier}\""),
                };
                assert_eq!(argument, expected);
            }
        }
    }

    #[test]
    fn inline_ballot_literals_and_constant_bindings_preserve_interface() {
        let session = CompilerSession::default();
        let selector = "a".repeat(128);
        let nullifier = "\\x00".repeat(32);
        for constants in [false, true] {
            let source = source(&selector, &nullifier, constants);
            let request = CompileRequest {
                source: &source,
                source_name: Some("ballot.ko"),
            };
            session
                .check(request)
                .expect("canonical ballot literals pass check");
            let direct = session
                .build(request)
                .expect("canonical ballot literals build");
            // Local binding propagation remains an optimization concern; the frontend must
            // accept bindings that code generation can prove are literal.
            let bound_source = source.replace(
                "let ballot =",
                &format!("let selector = \"{selector}\"; let nullifier = b\"{nullifier}\"; let ballot ="),
            ).replace(
                &format!("election_id: {},", if constants { "ELECTION".to_owned() } else { format!("\"{selector}\"") }),
                "election_id: selector,",
            ).replace(
                &format!("nullifier: {},", if constants { "NULLIFIER".to_owned() } else { format!("b\"{nullifier}\"") }),
                "nullifier: nullifier,",
            );
            let request = CompileRequest {
                source: &bound_source,
                source_name: Some("ballot.ko"),
            };
            session
                .check(request)
                .expect("constant local bindings pass check");
            let bound = session
                .build(request)
                .expect("constant local bindings build");
            assert_eq!(
                direct.contract_interface, bound.contract_interface,
                "literal propagation must preserve the ballot access contract"
            );
        }
    }
}
