//! Native identical-source capture and fresh compiler parity for actual VM controls.

use super::single_use_private::{assert_public_metadata, compile};
#[path = "../../../../fixtures/kotodama/private_single_use/cases.rs"]
mod cases;

fn validate_observable_expectation(case: &cases::Case) {
    match case.outcome {
        cases::Outcome::Success(words) => {
            assert!(!words.is_empty());
            for word in words {
                match word {
                    cases::Word::Int(value) => assert!((-100..=100).contains(value)),
                    cases::Word::Unit => assert_eq!(case.id, "unit"),
                }
            }
        }
        cases::Outcome::Fault(fault) => match fault {
            cases::Fault::DivisionByZero => assert_eq!(case.id, "division_trap"),
            cases::Fault::MantissaOverflow => assert_eq!(case.id, "overflow_trap"),
        },
    }
    assert_eq!(
        case.trace.is_some(),
        case.source.contains("state int trace;")
    );
}

fn fresh_pair(
    case: &cases::Case,
) -> (crate::session::CompileOutput, crate::session::CompileOutput) {
    validate_observable_expectation(case);
    let before = compile(case.source, true);
    let after = compile(case.source, false);
    assert_public_metadata(&before, &after);
    assert!(
        before
            .report
            .budget_report
            .iter()
            .any(|function| function.function_name == case.moved_helper)
    );
    assert!(
        !after
            .report
            .budget_report
            .iter()
            .any(|function| function.function_name == case.moved_helper)
    );
    assert_ne!(before.artifact, after.artifact);
    assert!(before.artifact.len() <= cases::MAX_ARTIFACT_BYTES);
    assert!(after.artifact.len() <= cases::MAX_ARTIFACT_BYTES);
    (before, after)
}

#[test]
#[ignore = "explicit native same-compiler before/after fixture capture; not a qualification gate"]
fn capture_private_single_use_native_pairs() {
    for case in cases::CASES {
        let (before, after) = fresh_pair(case);
        // A bounded native producer, never a hand-authored bytecode fixture.
        // The fixture owner saves only these exact emitted six-column rows.
        println!(
            "PRIVATE_SINGLE_USE_NATIVE\t{}\t{}\t{}\t{}\t{}\t{}",
            case.id,
            iroha_crypto::Hash::new(case.source.as_bytes()),
            iroha_crypto::Hash::new(&before.artifact),
            iroha_crypto::Hash::new(&after.artifact),
            hex::encode(&before.artifact),
            hex::encode(&after.artifact)
        );
    }
}

#[test]
fn native_pairs_reproduce_the_current_compiler_with_identical_source_and_public_roots() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pairs = cases::load_pairs(&root);
    for case in cases::CASES {
        let pair = &pairs[case.id];
        let (before, after) = fresh_pair(case);
        assert_eq!(
            pair.before, before.artifact,
            "current same-source retained-call artifact for {}",
            case.id
        );
        assert_eq!(
            pair.after, after.artifact,
            "current same-source sole-call artifact for {}",
            case.id
        );
    }
}
