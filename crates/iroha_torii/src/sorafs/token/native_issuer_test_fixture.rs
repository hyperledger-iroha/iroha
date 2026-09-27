//! Access to the actual issuer for native daemon tests across the crate boundary.
//!
//! This provides no signer, state, finality, quota ledger or reputation replacement. The caller
//! constructs the production issuer and supplies its real dependencies. HTTP authentication and
//! deployment-owned gateway admission/reputation services remain separate qualification work.
use super::*;

/// Run the real issuer with an explicitly test-authenticated quota identity and ordinary defaults.
///
/// # Errors
/// Returns the production issuance error without intercepting or replacing any verification.
pub fn issue_native_token_v1(
    issuer: &StreamTokenIssuer,
    operator: &PublicKey,
    manifest_cid: Vec<u8>,
    provider_id: [u8; 32],
    profile_handle: String,
) -> Result<StreamTokenV1, StreamTokenIssuerError> {
    issuer
        .issue_token(
            StreamTokenQuotaSubject::from_authenticated_operator(operator),
            manifest_cid,
            provider_id,
            profile_handle,
            TokenOverrides::default(),
        )
        .map(|issued| issued.token)
}

/// Run the real current-custody check used when an issued token reaches serving admission.
///
/// # Errors
/// Returns the production admission error, including current revocation or missing finality.
pub fn before_native_token_admission_v1(
    issuer: &StreamTokenIssuer,
    token: &StreamTokenV1,
) -> Result<u64, StreamTokenIssuerError> {
    issuer.before_admission(&token.body)
}
