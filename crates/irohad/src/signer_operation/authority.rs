//! Provider-free custody authority and committed-observation view, shared by signing and recovery.

use super::*;

/// Read-only finalized custody and completed-operation authority.
///
/// A receipt checker can retain this capability without retaining a signer or any method that
/// reserves, completes or renews an operation. Implementations must authenticate each observation
/// against authoritative finalized state; a decoded receipt or caller-supplied response is not a
/// source of finality.
pub trait SignerOperationFinalizedReadSourceV1: Send + Sync {
    /// Authenticate the current custody state for this exact binding.
    ///
    /// # Errors
    /// Fails when current finalized custody, revocation, policy, time or anchor is unavailable.
    fn observe(
        &self,
        binding: &SignerCustodyBindingV1,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1>;
    /// Authenticate the exact durable completed row and fresh current custody at this phase.
    ///
    /// # Errors
    /// Fails when the row is absent, not finalized, changed, expired before completion, or its
    /// original custody does not match the request.
    fn observe_committed(
        &self,
        request: &SignerOperationCommitRequestV1<'_>,
        phase: SignerCommittedObservationPhaseV1,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1>;
}

/// Existing full sources keep their public generic contract; this is only a private borrowed view.
impl SignerOperationFinalizedReadSourceV1 for Arc<dyn SignerOperationStateSourceV1> {
    fn observe(
        &self,
        binding: &SignerCustodyBindingV1,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1> {
        SignerOperationStateSourceV1::observe(self.as_ref(), binding)
    }
    fn observe_committed(
        &self,
        request: &SignerOperationCommitRequestV1<'_>,
        phase: SignerCommittedObservationPhaseV1,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1> {
        SignerOperationStateSourceV1::observe_committed(self.as_ref(), request, phase)
    }
}

/// Immutable view. Its owner retains configuration and the concrete observation capability;
/// this value contains no provider, signing method, decoder or independent freshness marker.
pub(super) struct SignerOperationAuthorityV1<'a> {
    pub(super) binding: &'a SignerCustodyBindingV1,
    pub(super) record: &'a [u8],
    pub(super) trust: &'a SignerCustodyTrustV1,
    pub(super) source: &'a dyn SignerOperationFinalizedReadSourceV1,
}
impl SignerOperationAuthorityV1<'_> {
    pub(super) fn verify(
        &self,
        context: &SignerCustodyUseContextV1,
    ) -> Result<VerifiedSignerCustodyV1, SignerOperationErrorV1> {
        verify_signer_custody_use_v1(self.record, self.binding, self.trust, context)
            .map_err(SignerOperationErrorV1::Custody)
    }
}
