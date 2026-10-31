//! Borrowed signed-attachment X.509 admission through the sole native verifier.

use super::*;
use iroha_data_model::{
    isi::privacy::SubmitPrivacyZkX509CertificateV1, privacy::IrohaZkX509StarkP256StatementV1,
};

/// Verify one materialized X.509 action without copying its original native proof.
///
/// The caller authenticates the signed action and actual invocation. This function
/// grants no transaction authority: it checks the canonical request, governed
/// profile, actual execution context, current State, replay status and real
/// native proof before sealing the effect consumed by the Core handler.
///
/// # Errors
/// Returns the original typed request, context, State or native verification failure.
pub fn verify_privacy_zk_x509_attached_v1(
    request: &SubmitPrivacyZkX509CertificateV1,
    materialized: &IrohaZkX509StarkP256StatementV1,
    proof: &[u8],
    context: &PrivacyVerificationContextV1<'_>,
) -> Result<VerifiedPrivacyEffectsV1, PrivacyVerificationErrorV1> {
    validate_compiled_privacy_profile_v1(context.profile).map_err(|source| {
        PrivacyVerificationErrorV1::CompiledProfile(Box::new(PrivacyCompiledProfileFailureV1 {
            source,
        }))
    })?;
    let (statement_digest, encoded_action_bytes) = request
        .validate_materialized_against_profile_v1(
            materialized,
            proof,
            context.profile,
            context.protocol_limits,
            context.consensus_limits,
        )
        .map_err(|source| PrivacyVerificationErrorV1::AttachedZkX509(Box::new(source)))?;
    validate_privacy_execution_context_v1(&materialized.context, context)?;
    let role = PrivacyActionRoleV1::ZkX509CertificatePresentation;
    require_supported_privacy_action_role_v1(role)
        .map_err(|source| PrivacyVerificationErrorV1::UnsupportedOperation(Box::new(source)))?;
    let effect = verify_zk_x509_certificate_v1(materialized, proof, context)?;
    VerifiedPrivacyEffectsV1::seal_v1(
        role,
        statement_digest,
        context.expected_action_index,
        encoded_action_bytes,
        effect,
    )
}

#[cfg(test)]
mod tests;
