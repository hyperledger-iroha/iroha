//! Genuine same-compiler capture and fresh parity for exact frame-emission controls.
use super::tests::{assert_metadata, compile};
#[path = "../../../../../fixtures/kotodama/frame_emission/cases.rs"]
mod cases;
fn pair(case: &cases::Case) -> (crate::session::CompileOutput, crate::session::CompileOutput) {
    // Read the shared observable inventory here as well as in the VM consumer.
    // The native producer rejects an altered or unbounded fixture declaration
    // before emitting any row; these declarations are not execution evidence.
    match case.outcome {
        cases::Outcome::Success(words) => {
            assert!(!words.is_empty() && words.len() <= 102);
            for word in words {
                match word {
                    cases::Word::Int(value) => assert!((-100_000..=100_000).contains(value)),
                    cases::Word::Unit => assert_eq!(case.id, "unit_returns"),
                }
            }
        }
        cases::Outcome::Fault(cases::Fault::DivisionByZero) => assert_eq!(case.id, "division_trap"),
    }
    assert_eq!(
        case.trace.is_some(),
        case.source.contains("state int trace;")
    );
    let before = compile(case.source, true);
    let after = compile(case.source, false);
    assert_metadata(&before, &after);
    assert!(after.artifact.len() < before.artifact.len());
    for output in [&before, &after] {
        assert!(
            output
                .report
                .budget_report
                .iter()
                .any(|function| function.function_name == case.retained_helper),
            "the same genuinely called private declaration remains in both callable inventories"
        );
    }
    assert!(before.artifact.len() <= cases::MAX_ARTIFACT_BYTES);
    assert!(after.artifact.len() <= cases::MAX_ARTIFACT_BYTES);
    (before, after)
}
#[test]
#[ignore = "explicit genuine native compiler capture; this producer is not a qualification verdict"]
fn capture_actual_frame_emission_native_pairs() {
    for case in cases::CASES {
        let (before, after) = pair(case);
        println!(
            "FRAME_EMISSION_NATIVE\t{}\t{}\t{}\t{}\t{}\t{}",
            case.id,
            iroha_crypto::Hash::new(case.source.as_bytes()),
            iroha_crypto::Hash::new(&before.artifact),
            iroha_crypto::Hash::new(&after.artifact),
            hex::encode(before.artifact),
            hex::encode(after.artifact)
        );
    }
}
#[test]
fn frame_pairs_reproduce_current_full_compiler_artifacts_and_exact_public_abi() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pairs = cases::load_pairs(&root);
    for case in cases::CASES {
        let (before, after) = pair(case);
        let captured = &pairs[case.id];
        assert_eq!(
            captured.before, before.artifact,
            "exact native baseline for {}",
            case.id
        );
        assert_eq!(
            captured.after, after.artifact,
            "exact native optimized artifact for {}",
            case.id
        );
    }
}
