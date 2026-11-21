//! Spartan initial-credential profile of one installed Native owner.
//!
//! The installed owner that admitted the signed program carrier binds this profile to its
//! enrollment owner; no E6 or credential byte selects it. The profile shares the already
//! admitted parameters, so verifying a credential never rebuilds a generator set. It only
//! authenticates the exact 596-byte original under those parameters and the independently
//! selected Enrollment certificate. Every E5, evidence, enrollment, lease and signer binding
//! is the shared [`ResultV1::verify_initial_fields`] verifier's, exactly as for the Model
//! frame; there is no fallback from one profile to the other.

use super::carrier::{RequestV1, ResultV1};
use crate::kagemusha_spartan_native::native;
use iroha_data_model::kagemusha::{
    KagemushaWalletCredentialBodyV1, KagemushaWalletSchemeV1, KagemushaWalletSignerCertificateV1,
    KagemushaWalletSignerRoleV1,
};
use std::sync::Arc;

/// Exact Spartan credential original: the 532-byte body followed by raw low-S `r || s`.
pub const SPARTAN_CREDENTIAL_BYTES_V1: usize = 596;
/// Exact signer-certificate original consumed by the Spartan verifier:
/// `body.transcript() || signature`.
pub const SPARTAN_SIGNER_CERTIFICATE_BYTES_V1: usize = 172;

/// The raw certificate original (`body.transcript() || signature`) of an already selected
/// signer certificate. This re-encodes selected DATA; it authenticates nothing.
#[must_use]
pub fn spartan_signer_certificate_original_v1(
    certificate: &KagemushaWalletSignerCertificateV1,
) -> Vec<u8> {
    let mut original = certificate.body.transcript();
    original.extend_from_slice(certificate.signature.as_ref());
    original
}

/// Credential profile of an installed Spartan generation. Cloning shares the admitted
/// parameters. It holds no signing, account or monetary capability.
#[derive(Clone)]
pub struct SpartanCredentialProfileV1 {
    parameters: Arc<native::AuthenticatedParameters>,
}

impl SpartanCredentialProfileV1 {
    /// Bind the parameters an installed owner already authenticated from its carrier.
    #[must_use]
    pub fn new(parameters: Arc<native::AuthenticatedParameters>) -> Self {
        Self { parameters }
    }

    /// Exact signed manifest digest (the wallet generation) of the admitted parameters.
    #[must_use]
    pub fn manifest_digest(&self) -> &[u8; 32] {
        self.parameters.manifest_digest()
    }

    /// Authenticate one exact 596-byte original under the admitted parameters and the
    /// selected Enrollment certificate: the scheme-rooted certificate, the issuer's low-S
    /// signature over the Spartan body message, and the certified payment key table. The
    /// returned fields still require the shared enrollment binding comparison.
    /// # Errors
    /// Rejects another length, scheme, signer or certificate, a signature or key table that
    /// does not verify, and fields that are not valid initial enrollment fields.
    pub fn authenticate(
        &self,
        credential: &[u8],
        scheme: &KagemushaWalletSchemeV1,
        selected_issuer: &KagemushaWalletSignerCertificateV1,
    ) -> Result<KagemushaWalletCredentialBodyV1, &'static str> {
        if credential.len() != SPARTAN_CREDENTIAL_BYTES_V1 {
            return Err("Spartan issuer credential length");
        }
        selected_issuer
            .verify_role(scheme, KagemushaWalletSignerRoleV1::Enrollment)
            .map_err(|_| "selected enrollment issuer")?;
        let certificate = spartan_signer_certificate_original_v1(selected_issuer);
        if certificate.len() != SPARTAN_SIGNER_CERTIFICATE_BYTES_V1 {
            return Err("selected enrollment issuer original");
        }
        let authenticated = native::NativeEngine::default()
            .authenticate_credential_originals(credential, &certificate, &self.parameters)
            .map_err(|_| "Spartan issuer credential")?;
        let fields = authenticated
            .enrollment_fields()
            .map_err(|_| "Spartan issuer credential fields")?;
        if fields.scheme_id != scheme.scheme_id()
            || fields.issuer_certificate != selected_issuer.certificate_digest()
        {
            return Err("Spartan issuer credential selection");
        }
        Ok(fields)
    }

    /// Authenticate a complete E6 delivery against the exact account-authorized E5 and the
    /// selected Enrollment certificate. The credential is authenticated as a Spartan
    /// original only; the Model decoder is never consulted for it.
    /// # Errors
    /// Rejects what [`Self::authenticate`] and [`ResultV1::verify_initial_fields`] reject.
    pub fn verify_result(
        &self,
        result: &ResultV1,
        scheme: &KagemushaWalletSchemeV1,
        selected_issuer: &KagemushaWalletSignerCertificateV1,
        request: &RequestV1,
    ) -> Result<KagemushaWalletCredentialBodyV1, &'static str> {
        let fields = self.authenticate(&result.credential, scheme, selected_issuer)?;
        result.verify_initial_fields(scheme, selected_issuer, request, &fields)?;
        Ok(fields)
    }
}
