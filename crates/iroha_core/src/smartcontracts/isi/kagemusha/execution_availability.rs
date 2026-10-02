//! Deterministic governed eligibility separated from local verifier availability.
//!
//! The finalized World registry decides which releases can act. A missing,
//! stale or unknown process implementation cannot choose a transaction result.
//! This boundary neither authenticates release files nor verifies a proof.

use super::{AuthenticatedKagemushaV1RuntimeVerifier, KagemushaV1RuntimeVerifier};
use iroha_data_model::{
    NetworkId,
    kagemusha::{
        KAGEMUSHA_RELEASE_ACTIVE_V1, KAGEMUSHA_RELEASE_VERIFICATION_ONLY_V1,
        KagemushaGovernedVerifierRegistryV1,
    },
};

#[derive(Clone, Copy, Debug)]
pub(super) enum Operation {
    TopUp,
    Redemption,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    Rejected(&'static str),
    Unavailable,
}

/// Check the complete governed set before consulting any local implementation.
/// The returned success merely permits the original native recursive verifier
/// to run; it never substitutes for its cryptographic or release-scope checks.
pub(super) fn require(
    verifier: &dyn KagemushaV1RuntimeVerifier,
    network_id: NetworkId,
    registry: &KagemushaGovernedVerifierRegistryV1,
    release_id: [u8; 32],
    operation: Operation,
) -> Result<(), Failure> {
    registry.validate().map_err(Failure::Rejected)?;
    let governed = registry
        .releases
        .binary_search_by_key(&release_id, |row| row.release_id)
        .ok()
        .map(|i| &registry.releases[i])
        .ok_or(Failure::Rejected("KAGEMUSHA proof release is not governed"))?;
    let allowed = match operation {
        Operation::TopUp => {
            governed.status == KAGEMUSHA_RELEASE_ACTIVE_V1
                && registry.active_release_id == Some(release_id)
        }
        Operation::Redemption => matches!(
            governed.status,
            KAGEMUSHA_RELEASE_ACTIVE_V1 | KAGEMUSHA_RELEASE_VERIFICATION_ONLY_V1
        ),
    };
    if !allowed {
        return Err(Failure::Rejected(
            "KAGEMUSHA proof release lifecycle refuses this operation",
        ));
    }
    let verifier: &dyn std::any::Any = verifier;
    let runtime = verifier
        .downcast_ref::<AuthenticatedKagemushaV1RuntimeVerifier>()
        .ok_or(Failure::Unavailable)?;
    super::authority::runtime_matches_governed_registry(runtime, registry)
        .map_err(|_| Failure::Unavailable)?;
    // Network is independently supplied by the actual StateTransaction. Every
    // retained release belongs to the same governed network, including retired
    // verification-only releases; request bytes cannot pick another runtime.
    if runtime
        .releases
        .values()
        .any(|row| row.network_id != network_id)
    {
        return Err(Failure::Unavailable);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
