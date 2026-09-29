//! Current first-release identity checks and explicit native capture inventory.
//!
//! Ordinary checks and the ignored capture printer share the same typed inventory.

use super::Case;

const CASES: &[Case] = &[
    Case::bidirectional::<crate::fastpq::FastpqTransitionBatch>(
        "iroha_data_model::fastpq::FastpqTransitionBatch",
    ),
    Case::bidirectional::<crate::kagemusha::KagemushaGovernedVerifierReleaseV1>(
        "iroha:kagemusha:governed-verifier-release:v1",
    ),
    Case::bidirectional::<crate::kagemusha::KagemushaGovernedVerifierRegistryV1>(
        "iroha:kagemusha:governed-verifier-registry:v1",
    ),
    #[cfg(feature = "governance")]
    Case::bidirectional::<crate::governance::types::KagemushaVerifierPolicyInstallProposalV1>(
        "iroha_data_model::parliament_types::KagemushaVerifierPolicyInstallProposalV1",
    ),
    #[cfg(feature = "governance")]
    Case::bidirectional::<crate::governance::types::KagemushaVerifierReleaseInstallProposalV1>(
        "iroha_data_model::parliament_types::KagemushaVerifierReleaseInstallProposalV1",
    ),
    #[cfg(feature = "governance")]
    Case::bidirectional::<crate::governance::types::KagemushaVerifierReleaseActivateProposalV1>(
        "iroha_data_model::parliament_types::KagemushaVerifierReleaseActivateProposalV1",
    ),
    #[cfg(feature = "governance")]
    Case::bidirectional::<crate::isi::governance::ProposeKagemushaVerifierPolicyInstallV1>(
        "iroha_data_model::isi::governance::ProposeKagemushaVerifierPolicyInstallV1",
    ),
    #[cfg(feature = "governance")]
    Case::bidirectional::<crate::isi::governance::ProposeKagemushaVerifierReleaseInstallV1>(
        "iroha_data_model::isi::governance::ProposeKagemushaVerifierReleaseInstallV1",
    ),
    #[cfg(feature = "governance")]
    Case::bidirectional::<crate::isi::governance::ProposeKagemushaVerifierReleaseActivateV1>(
        "iroha_data_model::isi::governance::ProposeKagemushaVerifierReleaseActivateV1",
    ),
];

super::native_capture::owner_printer!(CASES);

#[test]
fn current_release_identities_match_native_capture() {
    for case in CASES {
        case.check();
    }
}
