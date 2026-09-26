//! Canonical authority-signed mobile bootstrap used by operators and native wallets.
//!
//! This protocol authenticates a checkpoint against independently selected deployment,
//! replay and time pins. It grants neither monetary authority nor device qualification;
//! native consumers must independently establish those pins and installation lifetime.

use iroha_crypto::{Hash, PublicKey, SignatureOf};
use sha2::{Digest as _, Sha256};

use super::{KAGEMUSHA_ASSET_SCALE_MAX_V1, KagemushaReleaseAuthorityPolicyV1};
use crate::{NetworkId, block::consensus_v2::HeightContextId};

const APPROVAL_DOMAIN: &str = "iroha:kagemusha:v1:mobile-bootstrap-approval";
const CHECKPOINT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:mobile-bootstrap-checkpoint\0";

/// Maximum complete canonical bootstrap archive, checked before decoding any collection.
pub const KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1: usize = 1024 * 1024;

/// Exact asset and reserve scope selected independently by the native operator.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_mobile_bootstrap_v1::KagemushaMobileBootstrapScopeV1"
)]
pub struct KagemushaMobileBootstrapScopeV1 {
    /// Normalized asset identity from the trusted asset registration.
    pub asset_identity_digest: [u8; 32],
    /// Exact asset incarnation.
    pub asset_incarnation: [u8; 32],
    /// Decimal asset scale.
    pub asset_scale: u32,
    /// Reserve-liability pool for this installation.
    pub liability_pool_id: [u8; 32],
}

/// Complete immutable subject approved by the native policy's threshold signers.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_mobile_bootstrap_v1::KagemushaMobileBootstrapCheckpointV1"
)]
pub struct KagemushaMobileBootstrapCheckpointV1 {
    /// Wire version; exactly one.
    pub version: u16,
    /// Digest of the independently installed release-authority policy.
    pub authority_policy_digest: [u8; 32],
    /// Network whose signed finality chain begins at `first_context_id`.
    pub network_id: NetworkId,
    /// Asset and reserve scope.
    pub scope: KagemushaMobileBootstrapScopeV1,
    /// Exact release identifier whose artifacts the host must authenticate separately.
    pub release_id: [u8; 32],
    /// Exact release-attestation digest.
    pub release_attestation_digest: [u8; 32],
    /// Authority-authenticated first height context for later finality-chain verification.
    pub first_context_id: HeightContextId,
    /// Monotonically increasing sequence within this native policy/network/scope.
    pub sequence: u64,
    /// Inclusive issuance time in milliseconds since the Unix epoch.
    pub issued_at_ms: u64,
    /// Exclusive expiry time in milliseconds since the Unix epoch.
    pub expires_at_ms: u64,
}

impl KagemushaMobileBootstrapCheckpointV1 {
    /// Construct the canonical domain-separated payload for an authority's signature.
    #[must_use]
    pub fn approval_payload(&self) -> KagemushaMobileBootstrapApprovalPayloadV1 {
        KagemushaMobileBootstrapApprovalPayloadV1 {
            domain: APPROVAL_DOMAIN.to_owned(),
            checkpoint: *self,
        }
    }
}

/// Domain-separated value signed by a bootstrap authority.
#[derive(Clone, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_mobile_bootstrap_v1::KagemushaMobileBootstrapApprovalPayloadV1"
)]
pub struct KagemushaMobileBootstrapApprovalPayloadV1 {
    /// Required cross-protocol signature separator.
    pub domain: String,
    /// Exact approved checkpoint.
    pub checkpoint: KagemushaMobileBootstrapCheckpointV1,
}

/// One authority approval; the key must occur in the independently installed policy.
#[derive(Clone, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_mobile_bootstrap_v1::KagemushaMobileBootstrapApprovalV1"
)]
pub struct KagemushaMobileBootstrapApprovalV1 {
    /// Signer's key, checked against the native policy before signature verification.
    pub public_key: PublicKey,
    /// Signature over the complete domain-separated checkpoint.
    pub signature: SignatureOf<KagemushaMobileBootstrapApprovalPayloadV1>,
}

/// Untrusted portable bootstrap package; decoding alone grants no authority.
#[derive(Clone, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_mobile_bootstrap_v1::KagemushaMobileBootstrapPackageV1"
)]
pub struct KagemushaMobileBootstrapPackageV1 {
    /// Exact signed subject, including its V1 wire version.
    pub checkpoint: KagemushaMobileBootstrapCheckpointV1,
    /// Strictly key-ordered distinct approvals satisfying the native threshold.
    pub approvals: Vec<KagemushaMobileBootstrapApprovalV1>,
}

/// Previously accepted checkpoint retained by the native freshness authority.
///
/// This value is an input pin, not an independently authenticated capability. It must come
/// from trusted native state, never the package, operation response, or untrusted app storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaMobileBootstrapReplayPinV1 {
    /// Highest previously accepted sequence for this policy/network/scope.
    pub sequence: u64,
    /// Digest of the exact checkpoint accepted at that sequence.
    pub checkpoint_digest: [u8; 32],
}

/// Independent native selection and freshness policy for a downloaded bootstrap package.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaMobileBootstrapPinsV1<'a> {
    /// Operator-provisioned verification policy, never selected by the package.
    pub authority_policy: &'a KagemushaReleaseAuthorityPolicyV1,
    /// Independently selected deployment network.
    pub network_id: NetworkId,
    /// Independently selected asset and reserve scope.
    pub scope: KagemushaMobileBootstrapScopeV1,
    /// Release identifier selected by native configuration or authenticated release loading.
    pub release_id: [u8; 32],
    /// Exact release attestation selected by native configuration or release loading.
    pub release_attestation_digest: [u8; 32],
    /// Lowest acceptable sequence, supplied by the native freshness authority; nonzero.
    pub minimum_sequence: u64,
    /// Previously accepted checkpoint in this policy/network/scope, when available.
    pub previous: Option<KagemushaMobileBootstrapReplayPinV1>,
    /// Trusted current time; handset wall-clock values alone do not establish freshness.
    pub trusted_now_ms: u64,
}

impl KagemushaMobileBootstrapCheckpointV1 {
    /// Validate this exact subject against independent deployment and freshness pins.
    ///
    /// Returns its domain-separated checkpoint digest for replay retention. This does not
    /// verify any signature; use the complete package's `authenticate` method for admission.
    ///
    /// # Errors
    /// Rejects invalid policy, substituted pins, invalid scope, expiry, or replay.
    pub fn validate_pins(
        &self,
        pins: &KagemushaMobileBootstrapPinsV1<'_>,
    ) -> Result<[u8; 32], String> {
        let policy_digest = pins
            .authority_policy
            .canonical_digest()
            .map_err(|_| "KAGEMUSHA mobile bootstrap has an invalid native policy".to_owned())?;
        let checkpoint = *self;
        let scope = checkpoint.scope;
        if checkpoint.version != 1
            || checkpoint.authority_policy_digest != policy_digest
            || checkpoint.network_id != pins.network_id
            || checkpoint.network_id.as_bytes() == &[0; 32]
            || checkpoint.network_id.as_bytes() == Hash::prehashed([0; 32]).as_ref()
            || scope != pins.scope
            || scope.asset_identity_digest == [0; 32]
            || scope.asset_incarnation == [0; 32]
            || scope.asset_scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
            || scope.liability_pool_id == [0; 32]
            || scope.asset_identity_digest == scope.liability_pool_id
            || checkpoint.release_id != pins.release_id
            || checkpoint.release_attestation_digest != pins.release_attestation_digest
            || checkpoint.release_id == [0; 32]
            || checkpoint.release_attestation_digest == [0; 32]
            || checkpoint.release_id == checkpoint.release_attestation_digest
            || &checkpoint.release_id == checkpoint.network_id.as_bytes()
            || &checkpoint.release_attestation_digest == checkpoint.network_id.as_bytes()
            || checkpoint.first_context_id.0.as_ref() == Hash::prehashed([0; 32]).as_ref()
        {
            return Err(
                "KAGEMUSHA mobile bootstrap differs from native deployment pins".to_owned(),
            );
        }
        if pins.minimum_sequence == 0
            || checkpoint.sequence < pins.minimum_sequence
            || checkpoint.issued_at_ms == 0
            || checkpoint.issued_at_ms >= checkpoint.expires_at_ms
            || pins.trusted_now_ms < checkpoint.issued_at_ms
            || pins.trusted_now_ms >= checkpoint.expires_at_ms
        {
            return Err("KAGEMUSHA mobile bootstrap freshness failed".to_owned());
        }
        let encoded = norito::encode_canonical(&checkpoint)
            .map_err(|_| "KAGEMUSHA mobile bootstrap checkpoint encoding failed".to_owned())?;
        let mut digest = Sha256::new();
        digest.update(CHECKPOINT_DOMAIN);
        digest.update((encoded.len() as u64).to_le_bytes());
        digest.update(encoded);
        let checkpoint_digest: [u8; 32] = digest.finalize().into();
        if pins.previous.is_some_and(|previous| {
            previous.sequence == 0
                || previous.checkpoint_digest == [0; 32]
                || checkpoint.sequence < previous.sequence
                || (checkpoint.sequence == previous.sequence
                    && checkpoint_digest != previous.checkpoint_digest)
        }) {
            return Err("KAGEMUSHA mobile bootstrap replay pin changed or regressed".to_owned());
        }
        Ok(checkpoint_digest)
    }
}

impl KagemushaMobileBootstrapApprovalV1 {
    /// Verify one partial approval under an independently selected valid policy.
    ///
    /// This checks signer membership and the exact domain-separated checkpoint signature,
    /// but one partial approval does not establish the policy's complete threshold.
    ///
    /// # Errors
    /// Rejects invalid policies, unknown signers, and changed subjects or signatures.
    pub fn verify(
        &self,
        checkpoint: &KagemushaMobileBootstrapCheckpointV1,
        policy: &KagemushaReleaseAuthorityPolicyV1,
    ) -> Result<(), String> {
        let digest = policy
            .canonical_digest()
            .map_err(|_| "KAGEMUSHA mobile bootstrap has an invalid native policy".to_owned())?;
        if checkpoint.authority_policy_digest != digest {
            return Err("KAGEMUSHA mobile bootstrap policy digest differs".to_owned());
        }
        if policy
            .authorized_signers
            .binary_search(&self.public_key)
            .is_err()
        {
            return Err("KAGEMUSHA mobile bootstrap signer is not authorized".to_owned());
        }
        self.signature
            .verify(&self.public_key, &checkpoint.approval_payload())
            .map_err(|_| "KAGEMUSHA mobile bootstrap signature failed".to_owned())
    }
}

impl KagemushaMobileBootstrapPackageV1 {
    /// Decode one complete bounded canonical package without granting trust.
    ///
    /// # Errors
    /// Rejects oversized, empty, malformed, trailing, or noncanonical archives.
    pub fn decode_canonical_exact(archive: &[u8]) -> Result<Self, String> {
        if archive.is_empty() || archive.len() > KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1 {
            return Err("KAGEMUSHA mobile bootstrap exceeds its archive bound".to_owned());
        }
        norito::decode_canonical_with_limits(
            archive,
            norito::canonical_decode_limits(archive.len()),
        )
        .map_err(|_| "KAGEMUSHA mobile bootstrap is not canonical Norito".to_owned())
    }

    /// Authenticate all exact checkpoint pins and distinct ordered threshold approvals.
    ///
    /// Returns the checkpoint digest to retain with its sequence. The caller must establish
    /// independent freshness and durable replay retention; a decoded package supplies neither.
    ///
    /// # Errors
    /// Rejects substituted/expired/replayed checkpoints and insufficient or invalid approvals.
    pub fn authenticate(
        &self,
        pins: &KagemushaMobileBootstrapPinsV1<'_>,
    ) -> Result<[u8; 32], String> {
        let digest = self.checkpoint.validate_pins(pins)?;
        if self.approvals.len() < usize::from(pins.authority_policy.threshold)
            || self.approvals.len() > pins.authority_policy.authorized_signers.len()
            || !self
                .approvals
                .windows(2)
                .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(
                "KAGEMUSHA mobile bootstrap requires distinct threshold approvals".to_owned(),
            );
        }
        for approval in &self.approvals {
            approval.verify(&self.checkpoint, pins.authority_policy)?;
        }
        Ok(digest)
    }
}

#[cfg(test)]
#[path = "kagemusha_mobile_bootstrap_v1_tests.rs"]
mod tests;
