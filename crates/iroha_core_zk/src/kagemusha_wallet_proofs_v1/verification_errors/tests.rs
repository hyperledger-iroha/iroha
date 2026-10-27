//! Error provenance controls: no prover, verifier, artifacts or kernel execution.

use super::*;

fn budget() -> MsmError {
    MsmError::Budget(iroha_pasta::msm::BudgetExceeded {
        required: 4096,
        budget: 1024,
    })
}

#[test]
fn original_kernel_failures_survive_every_verifier_wrapper() {
    for kernel_error in [
        budget(),
        MsmError::LengthMismatch(iroha_pasta::LengthMismatch { left: 2, right: 3 }),
        MsmError::ScalarBound,
        MsmError::TooLarge { n: usize::MAX },
    ] {
        let ipa = IpaError::Msm(kernel_error);
        let nested = MultiopenError::Ipa(ipa);
        for original in [
            VerifyError::Ipa(ipa),
            VerifyError::Multiopen(nested),
            VerifyError::Protocol(ProtocolError::Plan(nested)),
            VerifyError::BatchItem {
                index: 4,
                error: Box::new(VerifyError::Ipa(ipa)),
            },
            VerifyError::BatchItem {
                index: 5,
                error: Box::new(VerifyError::BatchItem {
                    index: 6,
                    error: Box::new(VerifyError::Protocol(ProtocolError::Plan(nested))),
                }),
            },
        ] {
            let mapped = verification_error(original.clone());
            assert_eq!(
                mapped.is_verification_memory_budget(),
                kernel_error == budget()
            );
            assert_eq!(mapped, Error::Verification(original));
        }
        for original in [
            iroha_plonk_recursion::Error::Kernel(kernel_error),
            iroha_plonk_recursion::Error::Parameters(ipa),
        ] {
            let mapped = recursion_error(original);
            assert_eq!(
                mapped.is_verification_memory_budget(),
                kernel_error == budget()
            );
            assert_eq!(mapped, Error::Recursion(original));
        }
    }
}

#[test]
fn only_completed_incoming_equations_and_transcripts_are_rejections() {
    for original in [
        VerifyError::Transcript(TranscriptError::ProofTruncated),
        VerifyError::Transcript(TranscriptError::TrailingBytes { remaining: 1 }),
        VerifyError::Transcript(TranscriptError::NonCanonicalScalar),
        VerifyError::Transcript(TranscriptError::InvalidPoint),
        VerifyError::Ipa(IpaError::OpeningFailed),
        VerifyError::Ipa(IpaError::FoldedGeneratorMismatch),
        VerifyError::Multiopen(MultiopenError::ConflictingEvaluations { query: 2 }),
        VerifyError::BatchItem {
            index: 3,
            error: Box::new(VerifyError::BatchRejected),
        },
    ] {
        assert_eq!(verification_error(original), Error::Proof);
    }
    assert_eq!(
        recursion_error(iroha_plonk_recursion::Error::Undecidable),
        Error::Proof
    );
    assert_eq!(
        recursion_error(iroha_plonk_recursion::Error::FoldEquation),
        Error::Proof
    );

    // The same nested equation diagnostic in trusted parameter/table setup is not
    // evidence that the incoming receipt was ever fully verified.
    for original in [
        VerifyError::Protocol(ProtocolError::Plan(MultiopenError::Ipa(
            IpaError::OpeningFailed,
        ))),
        VerifyError::Transcript(TranscriptError::ProfileMismatch),
        VerifyError::KeyMismatch,
        VerifyError::ParamsMismatch,
        VerifyError::InstanceColumns {
            expected: 1,
            found: 2,
        },
        VerifyError::Ipa(IpaError::ParamsTooSmall {
            needed: 16,
            available: 1,
        }),
    ] {
        assert_eq!(
            verification_error(original.clone()),
            Error::Verification(original)
        );
    }
    let original = iroha_plonk_recursion::Error::Parameters(IpaError::OpeningFailed);
    assert_eq!(recursion_error(original), Error::Recursion(original));
}

#[test]
fn nested_cancellation_remains_operational_including_trusted_context() {
    for original in [
        VerifyError::Cancelled,
        VerifyError::VerifyingKey(iroha_plonk::keys::VkError::Cancelled),
        VerifyError::Ipa(IpaError::Msm(MsmError::Cancelled)),
        VerifyError::Protocol(ProtocolError::Plan(MultiopenError::Cancelled)),
        VerifyError::BatchItem {
            index: 9,
            error: Box::new(VerifyError::Protocol(ProtocolError::Plan(
                MultiopenError::Ipa(IpaError::Msm(MsmError::Cancelled)),
            ))),
        },
    ] {
        assert_eq!(verification_error(original), Error::Cancelled);
    }
    for original in [
        iroha_plonk_recursion::Error::Cancelled,
        iroha_plonk_recursion::Error::Kernel(MsmError::Cancelled),
        iroha_plonk_recursion::Error::Parameters(IpaError::Msm(MsmError::Cancelled)),
    ] {
        assert_eq!(recursion_error(original), Error::Cancelled);
    }
}
