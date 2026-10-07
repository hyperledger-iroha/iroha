//! Native admission of an existing account and its actual enrolled custody slot.
//!
//! The installation owner supplies an authenticated immutable verifier and the canonical
//! complete source-qualified wallet graph, including all sigma, receipt, Q, operation
//! and Omega owners whose originals were strictly imported.
//! Foreign inputs supply original credential, certificate and AccountId
//! frames, never slot/scheme/wallet/artifact identifiers or a replacement account vault. Native
//! enumerates real provider slots, reconciles their markers, checks the positive hardware key,
//! retained intent/request/current credential and selected capsule, then asks the existing
//! Ed25519 account key to sign one fresh, source-bound Native challenge. Failed or abandoned
//! admission never generates a key, selects a monetary head, signs a payment or erases custody.
//!
//! This is admission to the unfinished native operation owner. It does not implement
//! NativeProofs or upgrade the foreign wallet-open ABI to availability by itself.

use std::sync::Arc;

use iroha_crypto::{Algorithm, Signature};
use iroha_data_model::{account::AccountId, kagemusha::kagemusha_wallet_v1::*};
use rand::rand_core::TryRngCore as _;

use crate::{
    kagemusha_wallet_advance_v1::{
        KagemushaWalletFsV1, KagemushaWalletMarkerRecordV1, KagemushaWalletPlatformV1,
        KagemushaWalletProbeV1, KagemushaWalletProviderErrorV1, KagemushaWalletProviderV1,
        KagemushaWalletSlotIdV1, KagemushaWalletSlotStatusV1, KagemushaWalletUnavailableV1,
        kagemusha_wallet_provider_digest_v1,
    },
    kagemusha_wallet_artifacts_v1::{
        InstalledVerifierPackV1, producer_inventory::QualifiedWalletSourcesV1,
    },
    kagemusha_wallet_preparation_v1::PreparationV1,
};

/// Bounded original single-key account frame, before canonical decoding.
pub const ACCOUNT_ORIGINAL_MAX_BYTES_V1: usize = 4096;
/// Bounded authoritative asset-scope original, before canonical decoding.
pub const ASSET_SCOPE_ORIGINAL_MAX_BYTES_V1: usize = 1024;

/// Native admission failure. Storage/key errors remain distinct from invalid ownership.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum Error {
    /// Provider reconciliation, unknown storage or key availability, or custody loss.
    #[error(transparent)]
    Provider(#[from] KagemushaWalletProviderErrorV1),
    /// Noncanonical or unauthenticated original, wrong installed source or account binding.
    #[error("wallet original intake rejected: {0}")]
    Authority(&'static str),
    /// No exact durable enrolled slot exists; enrollment/recovery is a separate workflow.
    #[error("no enrolled custody slot for the issued wallet")]
    NotEnrolled,
    /// More than one durable slot purports to own this exact wallet/key incarnation.
    #[error("ambiguous enrolled custody slots")]
    Ambiguous,
    /// The exact selected source changed while account authorization was pending.
    #[error("wallet source changed during original intake")]
    SourceChanged,
}

fn require_installation(
    installed: ([u8; 32], [u8; 32]),
    qualified: ([u8; 32], [u8; 32]),
    provider_scheme: [u8; 32],
) -> Result<(), Error> {
    if qualified != installed || provider_scheme != installed.0 {
        return Err(Error::Authority("installed producer/provider scope"));
    }
    Ok(())
}

fn account(original: &[u8], expected_digest: &[u8; 32]) -> Result<AccountId, Error> {
    if original.is_empty() || original.len() > ACCOUNT_ORIGINAL_MAX_BYTES_V1 {
        return Err(Error::Authority("account frame bound"));
    }
    let account: AccountId = norito::decode_canonical_with_limits(
        original,
        norito::canonical_decode_limits(original.len()),
    )
    .map_err(|_| Error::Authority("canonical account frame"))?;
    account
        .try_signatory()
        .filter(|key| key.algorithm() == Algorithm::Ed25519)
        .ok_or(Error::Authority("existing single Ed25519 account"))?;
    if kagemusha_wallet_account_digest_v1(&account)
        .map_err(|_| Error::Authority("account digest"))?
        != *expected_digest
    {
        return Err(Error::Authority("enrolled account"));
    }
    Ok(account)
}

fn asset_scope(original: &[u8], expected: &[u8; 32]) -> Result<KagemushaWalletAssetScopeV1, Error> {
    if original.is_empty() || original.len() > ASSET_SCOPE_ORIGINAL_MAX_BYTES_V1 {
        return Err(Error::Authority("asset scope original bound"));
    }
    let scope: KagemushaWalletAssetScopeV1 = norito::decode_canonical_with_limits(
        original,
        norito::canonical_decode_limits(original.len()),
    )
    .map_err(|_| Error::Authority("canonical asset scope original"))?;
    scope
        .validate()
        .map_err(|_| Error::Authority("asset scope"))?;
    if scope.asset_digest() != *expected {
        return Err(Error::Authority("issuer-authenticated asset scope/scale"));
    }
    Ok(scope)
}

fn bind_challenge(
    challenge: &KagemushaWalletEnrollmentChallengeV1,
    marker: &KagemushaWalletMarkerV1,
    credential: &KagemushaWalletCredentialV1,
) -> Result<(), Error> {
    challenge
        .validate()
        .map_err(|_| Error::Authority("retained enrollment challenge"))?;
    marker
        .validate()
        .map_err(|_| Error::Authority("actual custody marker"))?;
    let body = &credential.body;
    if challenge.scheme_id != body.scheme_id
        || challenge.asset_digest != body.asset_digest
        || challenge.account_digest != body.account_digest
        || challenge.app_policy != body.app_policy
        || challenge.enrollment_id(&body.payment_key) != body.enrollment_id
        || challenge.wallet_id(&body.payment_key) != body.wallet_id
        || marker.scheme_id != body.scheme_id
        || marker.asset_digest != body.asset_digest
        || marker.wallet_id != body.wallet_id
        || marker.payment_key != body.payment_key
    {
        return Err(Error::Authority("retained owner/key/intent"));
    }
    Ok(())
}

fn authorize_account(account: &AccountId, challenge: &[u8], original: &[u8]) -> Result<(), Error> {
    let key = account
        .try_signatory()
        .filter(|key| key.algorithm() == Algorithm::Ed25519)
        .ok_or(Error::Authority("existing single Ed25519 account"))?;
    if original.len() != 64 || challenge.len() != 32 {
        return Err(Error::Authority("original account authorization bound"));
    }
    Signature::from_bytes(original)
        .verify(key, challenge)
        .map_err(|_| Error::Authority("existing account authorization"))
}

fn open_message(
    nonce: &[u8; 32],
    manifest: &[u8; 32],
    slot: &KagemushaWalletSlotIdV1,
    marker_file_digest: &[u8; 32],
    credential_original: &[u8],
    certificate_original: &[u8],
    account_original: &[u8],
    asset_scope_original: &[u8],
) -> Vec<u8> {
    // Provider-local SHA framing, not a monetary G1 signature domain. Length-delimited
    // originals prevent cross-role/cross-incarnation reuse of the existing account loan.
    let mut transcript = b"iroha:kagemusha:wallet:original-open:v1\0".to_vec();
    transcript.extend_from_slice(nonce);
    transcript.extend_from_slice(manifest);
    transcript.extend_from_slice(&slot.0);
    transcript.extend_from_slice(marker_file_digest);
    for original in [
        credential_original,
        certificate_original,
        account_original,
        asset_scope_original,
    ] {
        transcript.extend_from_slice(&(original.len() as u64).to_le_bytes());
        transcript.extend_from_slice(original);
    }
    kagemusha_wallet_provider_digest_v1("original-open-account", &transcript).to_vec()
}

/// Move-only account authorization challenge retaining exclusive real provider custody.
/// It cannot be reconstructed from a foreign handle/identifier or serialized checkpoint.
pub struct PendingWalletOpenV1<F: KagemushaWalletFsV1, P> {
    provider: KagemushaWalletProviderV1<F, P>,
    installed: Arc<InstalledVerifierPackV1>,
    sources: Arc<QualifiedWalletSourcesV1>,
    slot: KagemushaWalletSlotIdV1,
    marker_file_digest: [u8; 32],
    credential: KagemushaWalletCredentialV1,
    credential_original: Vec<u8>,
    certificate_original: Vec<u8>,
    account: AccountId,
    account_original: Vec<u8>,
    asset_scope: KagemushaWalletAssetScopeV1,
    asset_scope_original: Vec<u8>,
    challenge: Vec<u8>,
}

/// Fully admitted original owner, awaiting the genuine native operation/folding owner.
/// Private fields confer no arbitrary signature, fabricated proof or replacement-key path.
pub struct AdmittedWalletV1<F: KagemushaWalletFsV1, P> {
    provider: KagemushaWalletProviderV1<F, P>,
    installed: Arc<InstalledVerifierPackV1>,
    sources: Arc<QualifiedWalletSourcesV1>,
    slot: KagemushaWalletSlotIdV1,
    credential: KagemushaWalletCredentialV1,
    credential_original: Vec<u8>,
    certificate_original: Vec<u8>,
    account: AccountId,
    account_original: Vec<u8>,
    asset_scope: KagemushaWalletAssetScopeV1,
    asset_scope_original: Vec<u8>,
}

fn source<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1>(
    provider: &mut KagemushaWalletProviderV1<F, P>,
    slot: &KagemushaWalletSlotIdV1,
    credential: &KagemushaWalletCredentialV1,
    credential_original: &[u8],
) -> Result<KagemushaWalletMarkerRecordV1, Error> {
    let status = provider.status(slot)?;
    let marker = match status {
        KagemushaWalletSlotStatusV1::Enrollment(record)
        | KagemushaWalletSlotStatusV1::Pending(record)
        | KagemushaWalletSlotStatusV1::Released(record) => record,
        KagemushaWalletSlotStatusV1::Terminal(_) => {
            return Err(KagemushaWalletProviderErrorV1::Terminal.into());
        }
        _ => return Err(Error::NotEnrolled),
    };
    let intent = provider
        .read_intent(slot)?
        .ok_or(Error::Authority("missing actual enrollment intent"))?;
    bind_challenge(&intent.challenge, marker.marker(), credential)?;
    let enrollment = provider
        .enrollment_record(slot)?
        .ok_or(Error::Authority("missing retained enrollment request"))?;
    let original_marker =
        KagemushaWalletMarkerV1::enrollment(&intent.challenge, credential.body.payment_key)
            .map_err(|_| Error::Authority("enrollment marker original"))?;
    if enrollment.request.is_empty()
        || enrollment.enrollment_marker_digest
            != original_marker
                .marker_digest()
                .map_err(|_| Error::Authority("enrollment marker digest"))?
        || provider
            .credential(slot, credential.body.renewal_sequence)?
            .as_deref()
            != Some(credential_original)
    {
        return Err(Error::Authority("retained enrollment/credential original"));
    }
    // Reconciliation caches verified marker bytes. This fresh positive key read is needed
    // at both account-admission boundaries; null/error never becomes absence or freshness.
    match provider.probe_payment_key(slot)? {
        KagemushaWalletProbeV1::Present(key) if key == credential.body.payment_key => {}
        KagemushaWalletProbeV1::Present(_) | KagemushaWalletProbeV1::Absent => {
            return Err(KagemushaWalletProviderErrorV1::KeyLost.into());
        }
        KagemushaWalletProbeV1::Unavailable(reason) => {
            return Err(KagemushaWalletProviderErrorV1::Unavailable(reason).into());
        }
    }
    if let Some(capsule) = provider.current_capsule(slot)? {
        capsule
            .successor_state
            .validate_for_credential(credential)
            .map_err(|_| Error::Authority("source credential/state"))?;
        capsule
            .statement
            .validate_for_credential(credential)
            .map_err(|_| Error::Authority("source credential/statement"))?;
    } else if credential.body.renewal_sequence != 0 {
        return Err(Error::Authority("renewal without selected source"));
    }
    let after = provider.status(slot)?;
    if after.marker().map(|r| r.marker_file_digest()) != Some(marker.marker_file_digest()) {
        return Err(Error::SourceChanged);
    }
    Ok(marker)
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1> PendingWalletOpenV1<F, P> {
    /// Admit authenticated originals against a real durable slot and positive hardware key.
    /// The immutable installation capabilities come from Native's authenticated loader.
    /// No slot/wallet/scheme/manifest identity is accepted from the foreign caller.
    /// # Errors
    /// Invalid originals, another installed owner, no/ambiguous source, unknown storage/key
    /// state or lost custody. This consumes the exclusive provider even when admission fails.
    pub fn begin(
        mut provider: KagemushaWalletProviderV1<F, P>,
        installed: Arc<InstalledVerifierPackV1>,
        sources: Arc<QualifiedWalletSourcesV1>,
        credential_original: &[u8],
        certificate_original: &[u8],
        account_original: &[u8],
        asset_scope_original: &[u8],
    ) -> Result<Self, Error> {
        require_installation(
            (
                installed.verifier().scheme().scheme_id(),
                installed.verifier().manifest_digest(),
            ),
            sources.installation(),
            *provider.scheme_id(),
        )?;
        let preparation = PreparationV1::new(&installed)
            .map_err(|_| Error::Authority("installed preparation source"))?;
        let authenticated = preparation
            .authenticate_credential(credential_original, certificate_original)
            .map_err(|_| Error::Authority("issuer-authenticated originals"))?;
        let credential = *authenticated.credential();
        let account = account(account_original, &credential.body.account_digest)?;
        let asset_scope = asset_scope(asset_scope_original, &credential.body.asset_digest)?;
        let mut selected = None;
        for slot in provider.slots()? {
            let status = provider.status(&slot)?;
            if status
                .marker()
                .is_some_and(|record| record.marker().wallet_id == credential.body.wallet_id)
            {
                let marker = source(&mut provider, &slot, &credential, credential_original)?;
                if selected.is_some() {
                    return Err(Error::Ambiguous);
                }
                selected = Some((slot, *marker.marker_file_digest()));
            }
        }
        let (slot, marker_file_digest) = selected.ok_or(Error::NotEnrolled)?;
        let mut nonce = [0; 32];
        rand::rngs::OsRng.try_fill_bytes(&mut nonce).map_err(|_| {
            KagemushaWalletProviderErrorV1::Unavailable(KagemushaWalletUnavailableV1::Platform(0))
        })?;
        if nonce == [0; 32] {
            return Err(Error::Authority("zero Native nonce"));
        }
        let challenge = open_message(
            &nonce,
            &installed.verifier().manifest_digest(),
            &slot,
            &marker_file_digest,
            credential_original,
            certificate_original,
            account_original,
            asset_scope_original,
        );
        Ok(Self {
            provider,
            installed,
            sources,
            slot,
            marker_file_digest,
            credential,
            credential_original: credential_original.to_vec(),
            certificate_original: certificate_original.to_vec(),
            account,
            account_original: account_original.to_vec(),
            asset_scope,
            asset_scope_original: asset_scope_original.to_vec(),
            challenge,
        })
    }

    /// The exact fresh Native message for the selected existing Ed25519 account loan.
    #[must_use]
    pub fn challenge(&self) -> &[u8] {
        &self.challenge
    }

    /// Consume one authorization and recheck the exact source and actual payment key.
    /// Failed signatures consume this pending owner; a new begin samples a fresh challenge.
    /// # Errors
    /// Wrong account signature, stale/changed source, unknown key/storage or lost custody.
    pub fn finish(mut self, account_signature: &[u8]) -> Result<AdmittedWalletV1<F, P>, Error> {
        authorize_account(&self.account, &self.challenge, account_signature)?;
        let marker = source(
            &mut self.provider,
            &self.slot,
            &self.credential,
            &self.credential_original,
        )?;
        if marker.marker_file_digest() != &self.marker_file_digest {
            return Err(Error::SourceChanged);
        }
        Ok(AdmittedWalletV1 {
            provider: self.provider,
            installed: self.installed,
            sources: self.sources,
            slot: self.slot,
            credential: self.credential,
            credential_original: self.credential_original,
            certificate_original: self.certificate_original,
            account: self.account,
            account_original: self.account_original,
            asset_scope: self.asset_scope,
            asset_scope_original: self.asset_scope_original,
        })
    }
}

impl<F: KagemushaWalletFsV1, P> AdmittedWalletV1<F, P> {
    /// Exact account identity admitted from its original and existing key authorization.
    #[must_use]
    pub const fn account(&self) -> &AccountId {
        &self.account
    }

    /// Exact issuer-authenticated credential of the actual selected source.
    #[must_use]
    pub const fn credential(&self) -> &KagemushaWalletCredentialV1 {
        &self.credential
    }

    /// Original owner frames, retained for the authentic native operation owner.
    #[must_use]
    pub fn originals(&self) -> (&[u8], &[u8], &[u8], &[u8]) {
        (
            &self.credential_original,
            &self.certificate_original,
            &self.account_original,
            &self.asset_scope_original,
        )
    }

    /// Authoritative monetary asset and scale bound by the authenticated credential.
    /// Native immutable review metadata uses this original; the App never infers scope.
    #[must_use]
    pub const fn asset_scope(&self) -> &KagemushaWalletAssetScopeV1 {
        &self.asset_scope
    }

    /// Move the real admission into Native's operation owner, keeping exclusive custody
    /// and every exact owner original. This is crate-private and grants no foreign open.
    pub(crate) fn into_parts(
        self,
    ) -> (
        KagemushaWalletProviderV1<F, P>,
        Arc<InstalledVerifierPackV1>,
        Arc<QualifiedWalletSourcesV1>,
        KagemushaWalletSlotIdV1,
        KagemushaWalletCredentialV1,
        AccountId,
        KagemushaWalletAssetScopeV1,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
    ) {
        (
            self.provider,
            self.installed,
            self.sources,
            self.slot,
            self.credential,
            self.account,
            self.asset_scope,
            self.credential_original,
            self.certificate_original,
            self.account_original,
            self.asset_scope_original,
        )
    }
}

#[cfg(test)]
#[path = "kagemusha_wallet_intake_v1/tests.rs"]
mod tests;
