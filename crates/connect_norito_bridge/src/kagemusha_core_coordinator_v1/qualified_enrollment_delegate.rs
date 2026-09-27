//! Rust-only adapter from a retained enrollment ticket to the consuming issuer kernel.
//!
//! A platform integration supplies the independent policies, release, owner, key and raw
//! evidence through a trusted provider. The application frame is only a bounded transport for
//! the signed preparation, app certificate, qualification and issuer challenge. This module
//! does not install a provider or turn an unqualified platform into monetary authority.

use std::sync::Arc;

use iroha_data_model::kagemusha::{
    KagemushaAppAttestationAuthorityPolicyV1, KagemushaAuthenticatedReleaseV1,
    KagemushaDevicePublicKeyV1, KagemushaRetailEnrollmentIssuerPolicyV1,
    KagemushaRetailEnrollmentOwnerV1,
};

use super::{
    AcceptedIssuerChallengeV1, InitialEnrollmentErrorV1, IssuerChallengeProjectionV1,
    KagemushaCoreCoordinatorBackendErrorV1, KagemushaCoreCoordinatorMethodV1,
    KagemushaEnrollmentLiveSelectionV1, KagemushaQualifiedEnrollmentDelegateV1,
    PendingIssuerEnrollmentV1, PreparedIssuerProofV1, archive_boundary,
    kagemusha_core_coordinator_decode_request_v1,
};

/// Independently provisioned context for one original journal selection.
///
/// The provider must retain the exact platform evidence and trusted service time outside the
/// application frame. Supplying this type does not itself qualify the evidence; the consuming
/// kernel verifies its signed certificate, device credential, release and policy bindings.
pub struct KagemushaEnrollmentProvisionedContextV1 {
    /// Independently pinned issuer authority and runtime.
    pub policy: Arc<KagemushaRetailEnrollmentIssuerPolicyV1>,
    /// Independently pinned app attestation authority.
    pub app_policy: Arc<KagemushaAppAttestationAuthorityPolicyV1>,
    /// Threshold-authenticated release and enabled hardware profiles.
    pub release: Arc<KagemushaAuthenticatedReleaseV1>,
    /// Account and lane selected by the native enrollment owner.
    pub owner: KagemushaRetailEnrollmentOwnerV1,
    /// Native authorization key selected before the app's phase-2 response.
    pub native_authorization_public_key: KagemushaDevicePublicKeyV1,
    /// Platform key ID selected for the issuer-signed preparation.
    pub selected_attested_key_id: [u8; 32],
    /// Complete original platform evidence, retained outside caller-supplied phase bytes.
    pub raw_platform_evidence: Vec<u8>,
    /// Trusted service time used by the signed preparation and app certificate checks.
    pub trusted_now_ms: u64,
}

/// Rust-only source of independently retained evidence for the exact live journal ticket.
///
/// Implementations must resolve `handle` and the original native selection to one provisioned
/// attempt. No C/JNI field, process cache replay, or HTTP projection may create this context.
pub trait KagemushaEnrollmentContextProviderV1: Send + Sync + 'static {
    /// Return one independently governed context or fail closed.
    fn context_for_selection(
        &self,
        handle: u64,
        selection: &KagemushaEnrollmentLiveSelectionV1,
    ) -> Result<KagemushaEnrollmentProvisionedContextV1, KagemushaCoreCoordinatorBackendErrorV1>;
}

/// Concrete consuming enrollment-kernel delegate. Construction does not install a backend.
pub struct KagemushaKernelEnrollmentDelegateV1 {
    context_provider: Arc<dyn KagemushaEnrollmentContextProviderV1>,
}

impl KagemushaKernelEnrollmentDelegateV1 {
    /// Bind one trusted Rust context provider without making it globally callable.
    #[must_use]
    pub fn new(context_provider: Arc<dyn KagemushaEnrollmentContextProviderV1>) -> Self {
        Self { context_provider }
    }
}

impl KagemushaQualifiedEnrollmentDelegateV1 for KagemushaKernelEnrollmentDelegateV1 {
    fn accept_challenge(
        &self,
        handle: u64,
        live_selection: KagemushaEnrollmentLiveSelectionV1,
        request_frame: &[u8],
    ) -> Result<AcceptedIssuerChallengeV1, KagemushaCoreCoordinatorBackendErrorV1> {
        if handle == 0 {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        let selection = live_selection
            .require_live()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        archive_boundary::validate_request(
            KagemushaCoreCoordinatorMethodV1::InitialEnrollment,
            request_frame,
        )
        .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        let fields = kagemusha_core_coordinator_decode_request_v1(request_frame)
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if fields.len() != 11
            || fields[0].as_slice() != 2_u32.to_le_bytes()
            || fields[1].as_slice() != selection.ticket.to_le_bytes()
        {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        let context = self
            .context_provider
            .context_for_selection(handle, &live_selection)?;
        let pending = PendingIssuerEnrollmentV1::begin_selected(
            live_selection,
            context.policy,
            context.app_policy,
            context.release,
            context.owner,
            context.native_authorization_public_key,
            context.selected_attested_key_id,
            &fields[2],
            &context.raw_platform_evidence,
            &fields[3],
            &fields[4],
            context.trusted_now_ms,
        )
        .map_err(map_kernel_error)?;
        pending
            .accept_challenge_with_certificate(
                &fields[5],
                IssuerChallengeProjectionV1 {
                    challenge_id: fixed_32(&fields[6])?,
                    account_signing_message: fixed_32(&fields[7])?,
                    device_request_id: fixed_32(&fields[8])?,
                    canonical_device_command: &fields[9],
                    expires_at_ms: u64::from_le_bytes(
                        fields[10]
                            .as_slice()
                            .try_into()
                            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?,
                    ),
                },
                &fields[3],
                context.trusted_now_ms,
            )
            .map_err(map_kernel_error)
    }

    fn prepare_proof(
        &self,
        handle: u64,
        live_selection: KagemushaEnrollmentLiveSelectionV1,
        accepted: AcceptedIssuerChallengeV1,
        raw_account_signature: &[u8],
        complete_device_response: &[u8],
    ) -> Result<PreparedIssuerProofV1, KagemushaCoreCoordinatorBackendErrorV1> {
        if handle == 0 {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        live_selection
            .require_live()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        accepted
            .require_same_live_selection(&live_selection)
            .map_err(map_kernel_error)?;
        accepted
            .prepare_proof(raw_account_signature, complete_device_response)
            .map_err(map_kernel_error)
    }
}

fn fixed_32(bytes: &[u8]) -> Result<[u8; 32], KagemushaCoreCoordinatorBackendErrorV1> {
    bytes
        .try_into()
        .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)
}

fn map_kernel_error(error: InitialEnrollmentErrorV1) -> KagemushaCoreCoordinatorBackendErrorV1 {
    match error {
        InitialEnrollmentErrorV1::RandomUnavailable => {
            KagemushaCoreCoordinatorBackendErrorV1::Unavailable
        }
        InitialEnrollmentErrorV1::Encoding
        | InitialEnrollmentErrorV1::Binding
        | InitialEnrollmentErrorV1::Authority
        | InitialEnrollmentErrorV1::Expired => KagemushaCoreCoordinatorBackendErrorV1::Rejected,
    }
}

#[cfg(test)]
#[path = "qualified_enrollment_delegate/tests.rs"]
mod tests;
