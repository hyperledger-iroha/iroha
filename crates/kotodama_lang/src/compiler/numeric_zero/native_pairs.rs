//! Genuine complete same-source native producer and mandatory exact paired capture consumer.
use super::tests::{assert_metadata, compile};
#[path = "cases.rs"]
mod cases;
fn pair(case: &cases::Case) -> (crate::session::CompileOutput, crate::session::CompileOutput) {
    assert_eq!(
        case.trace.is_some(),
        case.source.contains("state int trace;")
    );
    match case.outcome {
        cases::Outcome::Success(words) => {
            assert!(!words.is_empty());
            for word in words {
                match word {
                    cases::Word::Int(value) => assert!((-1_000..=1_000).contains(value)),
                    cases::Word::Decimal(value) | cases::Word::Quantity(value) => {
                        assert!(value.len() <= 16)
                    }
                    cases::Word::Unit => assert_eq!(case.id, "unit_loop"),
                }
            }
        }
        cases::Outcome::Abort { code, name } => {
            assert!(matches!((code, name), (3, "Low") | (9, "High")))
        }
        cases::Outcome::DivisionByZero => assert_eq!(case.id, "rounded_trap"),
        cases::Outcome::InvalidScale => assert_eq!(case.id, "invalid_scale"),
        cases::Outcome::QuantityUnderflow => assert_eq!(case.id, "map_underflow"),
        cases::Outcome::Permission => assert_eq!(case.id, "permission"),
    }
    let before = compile(case.source, true);
    let after = compile(case.source, false);
    assert_metadata(&before, &after);
    assert!(after.artifact.len() <= before.artifact.len());
    assert!(before.artifact.len() <= cases::MAX_ARTIFACT_BYTES);
    assert!(after.artifact.len() <= cases::MAX_ARTIFACT_BYTES);
    (before, after)
}
#[test]
#[ignore = "explicit genuine native compiler capture; no qualification verdict"]
fn capture_actual_numeric_zero_native_pairs() {
    let mut changed = false;
    for case in cases::CASES {
        let (before, after) = pair(case);
        changed |= before.artifact != after.artifact;
        println!(
            "NUMERIC_ZERO_NATIVE\t{}\t{}\t{}\t{}\t{}\t{}",
            case.id,
            iroha_crypto::Hash::new(case.source.as_bytes()),
            iroha_crypto::Hash::new(&before.artifact),
            iroha_crypto::Hash::new(&after.artifact),
            hex::encode(before.artifact),
            hex::encode(after.artifact)
        );
    }
    assert!(
        changed,
        "at least one complete actual native pair must contain a measured reduction"
    );
}
#[test]
fn current_numeric_zero_pairs_reproduce_full_native_bytes_and_all_original_metadata() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let captured = cases::load_pairs(&root);
    for case in cases::CASES {
        let (before, after) = pair(case);
        assert_eq!(
            captured[case.id].before, before.artifact,
            "full original artifact for {}",
            case.id
        );
        assert_eq!(
            captured[case.id].after, after.artifact,
            "full sole production artifact for {}",
            case.id
        );
    }
}
