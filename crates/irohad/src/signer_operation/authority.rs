//! Provider-free custody authority and committed-observation view, shared by signing and recovery.

use super::*;

/// Private observation capability. Receipt-only recovery sources implement these real operations
/// without supplying Reserve/Complete stubs or installing either protected signing driver.
pub(super) trait SignerOperationObservationSourceV1: Send + Sync {
    fn observe(
        &self,
        binding: &SignerCustodyBindingV1,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1>;
    fn observe_committed(
        &self,
        request: &SignerOperationCommitRequestV1<'_>,
        phase: SignerCommittedObservationPhaseV1,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1>;
}

/// Existing full sources keep their public generic contract; this is only a private borrowed view.
impl SignerOperationObservationSourceV1 for Arc<dyn SignerOperationStateSourceV1> {
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
    pub(super) source: &'a dyn SignerOperationObservationSourceV1,
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
