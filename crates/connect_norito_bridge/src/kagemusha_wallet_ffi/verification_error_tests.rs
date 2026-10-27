//! Final ABI projection keeps unfinished verification outside proof-rejection status.

use super::*;
use iroha_core_zk::kagemusha_wallet_proofs_v1::Error as ProofError;

#[test]
fn archive_control_failures_never_project_a_financial_or_receipt_verdict() {
    use state::ArchiveAdmissionErrorV1 as A;
    for (error, expected) in [
        (A::Closed, CLOSED),
        (A::Foreign, INVALID),
        (A::Stale, INVALID),
        (A::NotDue, INVALID),
        (A::Busy, UNCERTAIN),
        (A::ExchangeOpen, UNCERTAIN),
        (A::FoldBacklog, FOLD_REQUIRED),
        (A::Exhausted, RESOURCE),
        (A::InvalidResolution, INTERNAL),
    ] {
        let actual = Failure::from(state::Error::ArchiveAdmission(error));
        assert_eq!(actual, Failure::code(expected));
        assert_ne!(actual.status, PROOF_REJECTED);
        assert_ne!(actual.status, CONFLICT);
    }
}

#[test]
fn verifier_budget_context_and_receipt_verdict_have_distinct_existing_statuses() {
    let original = iroha_plonk::VerifyError::BatchItem {
        index: 3,
        error: Box::new(iroha_plonk::VerifyError::Ipa(
            iroha_plonk::pcs::ipa::IpaError::Msm(iroha_pasta::msm::MsmError::Budget(
                iroha_pasta::msm::BudgetExceeded {
                    required: 8192,
                    budget: 1024,
                },
            )),
        )),
    };
    assert_eq!(
        Failure::from(state::Error::NativeVerification(ProofError::Verification(
            original
        )))
        .status,
        RESOURCE
    );
    for error in [
        ProofError::Profile,
        ProofError::RuntimeBinding,
        ProofError::Inventory,
        ProofError::Verification(iroha_plonk::VerifyError::ParamsMismatch),
        ProofError::Verification(iroha_plonk::VerifyError::Ipa(
            iroha_plonk::pcs::ipa::IpaError::Msm(iroha_pasta::msm::MsmError::ScalarBound),
        )),
    ] {
        assert_eq!(
            Failure::from(state::Error::NativeVerification(error)).status,
            INTERNAL
        );
    }
    assert_eq!(Failure::from(state::Error::Cancelled).status, CANCELLED);
    assert_eq!(
        Failure::from(state::Error::ArtifactsUnavailable("fixture")).status,
        ARTIFACTS_UNAVAILABLE
    );
    assert_eq!(
        Failure::from(state::Error::ReceiptRejected("fixture")).status,
        PROOF_REJECTED
    );
    assert_eq!(
        Failure::from(state::Error::Proof("source failure")).status,
        PROOF_REJECTED
    );
}
