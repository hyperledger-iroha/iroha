//! Opaque software signing from an owner-only supervisor credential.
//!
//! This adapter never exposes the loaded private key. Its source must independently authenticate
//! finalized custody and durable reservation ownership; the adapter does not provide that source.
//! The operation coordinator retains the before/after-provider and completion-release fences.

use super::{
    MAX_RESERVATION_MS, SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1, SignerKeyOperationProviderV1,
    SignerKeyOperationRequestV1, SignerOperationCoordinatorV1, SignerOperationErrorV1,
    SignerOperationStateSourceV1, SignerReservedObservationPhaseV1, required_purposes,
};
use crate::runtime_credential::load_bounded_runtime_credential_v1;
use iroha_crypto::{ExposedPrivateKey, KeyPair, Signature};
use sorafs_manifest::signer::custody::{
    SignerCustodyBindingV1, SignerCustodyTrustV1, verify_signer_custody_use_v1,
};
use sorafs_manifest::signer::protocol::SignerKeyOperationPurposeV1;
use std::{path::Path, sync::Arc};
use zeroize::Zeroizing;

// A canonical private-key multihash is hex text; the largest admitted key payload is 8 KiB.
const MAX_CREDENTIAL_BYTES_V1: usize = 16 * 1024 + 256;

/// Software key operations bound to one independently enrolled custody generation.
///
/// Construction reads one private credential, checks its exact public key, and retains only the
/// secret-owning key pair. Rotation creates a new provider after new authoritative enrollment;
/// the old provider cannot follow a changed active head or reservation fence.
pub struct SoftwareCredentialSignerKeyOperationProviderV1 {
    binding: SignerCustodyBindingV1,
    record: Vec<u8>,
    trust: SignerCustodyTrustV1,
    source: Arc<dyn SignerOperationStateSourceV1>,
    keypair: KeyPair,
}

impl SoftwareCredentialSignerKeyOperationProviderV1 {
    /// Load a canonical bare private-key multihash followed by one newline from a supervisor file.
    ///
    /// The shared credential reader requires an owner-only regular file with trusted ancestors,
    /// rejects symlinks and changed descriptors, and zeroizes the input allocation. The caller
    /// supplies the independently configured custody binding, signed record, trust and finalized
    /// state source; this constructor cannot create or replace authoritative operation state.
    ///
    /// # Errors
    /// Returns a secret-free failure for invalid custody, unavailable state, malformed credentials
    /// or a credential whose algorithm or public key differs from the active enrolled generation.
    pub fn load_from_supervisor_credential(
        path: &Path,
        binding: SignerCustodyBindingV1,
        record: Vec<u8>,
        trust: SignerCustodyTrustV1,
        source: Arc<dyn SignerOperationStateSourceV1>,
    ) -> Result<Self, SignerOperationErrorV1> {
        binding
            .validate()
            .map_err(SignerOperationErrorV1::Custody)?;
        if !binding.runtime_handle.starts_with("software://")
            || !binding.key_handle.starts_with("software://")
        {
            return Err(SignerOperationErrorV1::InvalidOperation);
        }
        let context = source.observe(&binding)?;
        verify_signer_custody_use_v1(&record, &binding, &trust, &context)
            .map_err(SignerOperationErrorV1::Custody)?;

        let bytes = load_bounded_runtime_credential_v1(path, 2, MAX_CREDENTIAL_BYTES_V1)
            .map_err(|_| SignerOperationErrorV1::ProviderUnavailable)?;
        let literal = bytes
            .strip_suffix(b"\n")
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .ok_or(SignerOperationErrorV1::ProviderUnavailable)?;
        let exposed: ExposedPrivateKey = literal
            .parse()
            .map_err(|_| SignerOperationErrorV1::ProviderUnavailable)?;
        let canonical = Zeroizing::new(
            exposed
                .try_to_multihash_string()
                .map_err(|_| SignerOperationErrorV1::ProviderUnavailable)?,
        );
        if canonical.as_str() != literal || exposed.0.algorithm() != binding.algorithm.algorithm() {
            return Err(SignerOperationErrorV1::ProviderUnavailable);
        }
        let keypair = KeyPair::from_private_key(exposed.0)
            .map_err(|_| SignerOperationErrorV1::ProviderUnavailable)?;
        if keypair.public_key() != &binding.public_key {
            return Err(SignerOperationErrorV1::ProviderUnavailable);
        }
        Ok(Self {
            binding,
            record,
            trust,
            source,
            keypair,
        })
    }
}

impl SignerOperationCoordinatorV1 {
    /// Construct a software signer only with an explicitly supplied authoritative state source.
    ///
    /// The source must authenticate finalized custody, the current audit head, exclusive
    /// reservations and durable completions. Until deployment supplies that source, construction
    /// fails before reading the supervisor credential. The same source is passed to the provider
    /// and coordinator so key use and completion observe the same authority.
    ///
    /// # Errors
    /// Returns `StateUnavailable` when no source is configured, or the provider/coordinator
    /// failure when the credential or authoritative state cannot be verified.
    pub fn from_software_supervisor_credential(
        path: &Path,
        binding: SignerCustodyBindingV1,
        record: Vec<u8>,
        trust: SignerCustodyTrustV1,
        source: Option<Arc<dyn SignerOperationStateSourceV1>>,
    ) -> Result<Self, SignerOperationErrorV1> {
        let source = source.ok_or(SignerOperationErrorV1::StateUnavailable)?;
        let provider = Arc::new(
            SoftwareCredentialSignerKeyOperationProviderV1::load_from_supervisor_credential(
                path,
                binding.clone(),
                record.clone(),
                trust.clone(),
                Arc::clone(&source),
            )?,
        );
        Self::new(binding, record, trust, provider, source)
    }
}

impl SignerKeyOperationProviderV1 for SoftwareCredentialSignerKeyOperationProviderV1 {
    fn sign(
        &self,
        request: &SignerKeyOperationRequestV1<'_>,
    ) -> Result<Signature, SignerOperationErrorV1> {
        let check = request.check();
        let original = check.request().custody();
        let reservation = check.reservation();
        let ordinal = usize::from(request.ordinal());
        if original.statement().binding != self.binding
            || original.record_digest() == [0; 32]
            || check.request().intent_digest()
                != check
                    .request()
                    .intent()
                    .digest()
                    .map_err(|_| SignerOperationErrorV1::InvalidOperation)?
            || reservation.reservation_id == [0; 32]
            || reservation.fence == 0
            || ordinal == 0
            || required_purposes(check.request().intent().action).get(ordinal - 1)
                != Some(&request.purpose())
            || request.message().is_empty()
            || request.message().len() > SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1
            || (request.purpose() != SignerKeyOperationPurposeV1::RolePayload
                && request.message().len() != 32)
        {
            return Err(SignerOperationErrorV1::InvalidOperation);
        }
        // Authenticate this exact fence again inside the provider before touching the key. The
        // coordinator separately observes after the provider and before durable completion.
        let context = self
            .source
            .observe_reserved(check, SignerReservedObservationPhaseV1::BeforeProvider)?;
        let current =
            verify_signer_custody_use_v1(&self.record, &self.binding, &self.trust, &context)
                .map_err(SignerOperationErrorV1::Custody)?;
        let now = current.verified_at_unix_ms();
        if !current.continues_active_state(original)
            || reservation.expires_at_unix_ms <= now
            || reservation.expires_at_unix_ms > current.statement().expires_at_unix_ms
            || reservation.expires_at_unix_ms.saturating_sub(now) > MAX_RESERVATION_MS
        {
            return Err(SignerOperationErrorV1::ReservationConflict);
        }
        let signature = Signature::try_new(self.keypair.private_key(), request.message())
            .map_err(|_| SignerOperationErrorV1::ProviderUnavailable)?;
        signature
            .verify(&self.binding.public_key, request.message())
            .map_err(|_| SignerOperationErrorV1::InvalidSignature)?;
        Ok(signature)
    }
}
