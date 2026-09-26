//! Authority-signed mobile trust bootstrap, independent of operation-status responses.
//!
//! A package supplies authenticated finality coordinates under an independently installed
//! native policy. It cannot select that policy, admit value, qualify hardware, or authenticate
//! proof artifacts. The native host must still authenticate the matching release and retain
//! sequence/time freshness independently of the downloaded package and rollbackable app data.

use std::time::Duration;

use iroha_crypto::{Hash, PublicKey, SignatureOf};
use iroha_data_model::{
    NetworkId,
    block::consensus_v2::HeightContextId,
    kagemusha::{KAGEMUSHA_ASSET_SCALE_MAX_V1, KagemushaReleaseAuthorityPolicyV1},
};
use sha2::{Digest as _, Sha256};

use crate::kagemusha_core_coordinator_v1::native_deadline::{
    MAX_LIFETIME, NativeContinuousInstantV1, NativeDeadlineV1,
};

const APPROVAL_DOMAIN: &str = "iroha:kagemusha:v1:mobile-bootstrap-approval";
const CHECKPOINT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:mobile-bootstrap-checkpoint\0";

/// Maximum complete canonical bootstrap archive, checked before decoding any collection.
pub const KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1: usize = 1024 * 1024;

/// Exact asset and reserve scope selected independently by the native operator.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema,
)]
#[norito_schema(name = "connect_norito_bridge::KagemushaMobileBootstrapScopeV1")]
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
#[norito_schema(name = "connect_norito_bridge::KagemushaMobileBootstrapCheckpointV1")]
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
#[norito_schema(name = "connect_norito_bridge::KagemushaMobileBootstrapApprovalPayloadV1")]
pub struct KagemushaMobileBootstrapApprovalPayloadV1 {
    /// Required cross-protocol signature separator.
    pub domain: String,
    /// Exact approved checkpoint.
    pub checkpoint: KagemushaMobileBootstrapCheckpointV1,
}

/// One authority approval; the key must occur in the independently installed policy.
#[derive(Clone, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaMobileBootstrapApprovalV1")]
pub struct KagemushaMobileBootstrapApprovalV1 {
    /// Signer's key, checked against the native policy before signature verification.
    pub public_key: PublicKey,
    /// Signature over the complete domain-separated checkpoint.
    pub signature: SignatureOf<KagemushaMobileBootstrapApprovalPayloadV1>,
}

/// Untrusted portable bootstrap package; decoding alone grants no authority.
#[derive(Clone, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaMobileBootstrapPackageV1")]
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

/// Authenticated bootstrap pins, constructible only by the bounded verifier.
///
/// This nonserializable token authenticates configuration only. Host installation must still
/// validate the corresponding signed release, proof artifacts, and durable wallet state.
/// Its installation lease is at most 120 seconds, includes device sleep, and never extends
/// the package's remaining lifetime. An expired token requires fresh package verification
/// with current native time and replay pins; this lease does not establish those pins.
pub struct KagemushaVerifiedMobileBootstrapV1 {
    checkpoint: KagemushaMobileBootstrapCheckpointV1,
    checkpoint_digest: [u8; 32],
    authority_policy: KagemushaReleaseAuthorityPolicyV1,
    installation_deadline: NativeDeadlineV1,
}

impl KagemushaVerifiedMobileBootstrapV1 {
    /// Check the short, suspend-inclusive lease before installation and owner publication.
    ///
    /// # Errors
    /// Rejects a token retained beyond its original lease, process-fork replay, unavailable
    /// continuous time, or a clock regression. Passing this check does not refresh native
    /// policy, trusted UTC, or the persistent sequence floor.
    pub fn require_unexpired(&self) -> Result<(), String> {
        self.installation_deadline.check().map(|_| ()).map_err(|_| {
            "KAGEMUSHA mobile bootstrap installation lease expired or invalid".to_owned()
        })
    }

    /// Inspect the immutable authenticated checkpoint for native host installation.
    #[must_use]
    pub const fn checkpoint(&self) -> &KagemushaMobileBootstrapCheckpointV1 {
        &self.checkpoint
    }

    /// Return the independently selected policy that authenticated this checkpoint.
    #[must_use]
    pub const fn trusted_authority_policy(&self) -> &KagemushaReleaseAuthorityPolicyV1 {
        &self.authority_policy
    }

    /// Return the authenticated deployment network.
    #[must_use]
    pub const fn network_id(&self) -> NetworkId {
        self.checkpoint.network_id
    }

    /// Return the authenticated first context for signed finality-chain verification.
    #[must_use]
    pub const fn first_context_id(&self) -> HeightContextId {
        self.checkpoint.first_context_id
    }

    /// Return the exact authenticated asset and reserve scope.
    #[must_use]
    pub const fn scope(&self) -> KagemushaMobileBootstrapScopeV1 {
        self.checkpoint.scope
    }

    /// Return the exact release identity the installer must authenticate.
    #[must_use]
    pub const fn release_id(&self) -> [u8; 32] {
        self.checkpoint.release_id
    }

    /// Return the exact authenticated release-attestation digest required at installation.
    #[must_use]
    pub const fn release_attestation_digest(&self) -> [u8; 32] {
        self.checkpoint.release_attestation_digest
    }

    /// Return the exact sequence and digest the native freshness authority must retain.
    #[must_use]
    pub const fn replay_pin(&self) -> KagemushaMobileBootstrapReplayPinV1 {
        KagemushaMobileBootstrapReplayPinV1 {
            sequence: self.checkpoint.sequence,
            checkpoint_digest: self.checkpoint_digest,
        }
    }
}

/// Verify one bounded canonical package against independent native pins.
///
/// An exact still-valid retry at the previous sequence is accepted. An older sequence or
/// changed checkpoint at that sequence is rejected. The caller must persist the returned
/// replay pin before using the checkpoint; this pure verifier cannot secure rollbackable
/// storage or make a handset clock trustworthy.
///
/// # Errors
/// Rejects malformed/noncanonical archives, invalid scope or freshness, policy substitution,
/// insufficient/duplicate/unknown approvals, invalid signatures, and unset finality contexts.
pub fn verify_kagemusha_mobile_bootstrap_v1(
    archive: &[u8],
    pins: KagemushaMobileBootstrapPinsV1<'_>,
) -> Result<KagemushaVerifiedMobileBootstrapV1, String> {
    if archive.is_empty() || archive.len() > KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1 {
        return Err("KAGEMUSHA mobile bootstrap exceeds its archive bound".to_owned());
    }
    // Read before decoding and signature work, so that verification consumes the lease too.
    let verification_started = NativeContinuousInstantV1::now()
        .map_err(|_| "KAGEMUSHA mobile bootstrap continuous clock is unavailable".to_owned())?;
    let package: KagemushaMobileBootstrapPackageV1 = norito::decode_canonical_with_limits(
        archive,
        norito::canonical_decode_limits(archive.len()),
    )
    .map_err(|_| "KAGEMUSHA mobile bootstrap is not canonical Norito".to_owned())?;
    let policy_digest = pins
        .authority_policy
        .canonical_digest()
        .map_err(|_| "KAGEMUSHA mobile bootstrap has an invalid native policy".to_owned())?;
    let checkpoint = package.checkpoint;
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
        return Err("KAGEMUSHA mobile bootstrap differs from native deployment pins".to_owned());
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
    if package.approvals.len() < usize::from(pins.authority_policy.threshold)
        || package.approvals.len() > pins.authority_policy.authorized_signers.len()
        || !package
            .approvals
            .windows(2)
            .all(|pair| pair[0].public_key < pair[1].public_key)
    {
        return Err("KAGEMUSHA mobile bootstrap requires distinct threshold approvals".to_owned());
    }
    let payload = checkpoint.approval_payload();
    for approval in &package.approvals {
        if pins
            .authority_policy
            .authorized_signers
            .binary_search(&approval.public_key)
            .is_err()
        {
            return Err("KAGEMUSHA mobile bootstrap signer is not authorized".to_owned());
        }
        approval
            .signature
            .verify(&approval.public_key, &payload)
            .map_err(|_| "KAGEMUSHA mobile bootstrap signature failed".to_owned())?;
    }
    let installation_deadline = NativeDeadlineV1::from_reading(
        verification_started,
        Duration::from_millis(checkpoint.expires_at_ms - pins.trusted_now_ms).min(MAX_LIFETIME),
    )
    .map_err(|_| "KAGEMUSHA mobile bootstrap installation lease is invalid".to_owned())?;
    let verified = KagemushaVerifiedMobileBootstrapV1 {
        checkpoint,
        checkpoint_digest,
        authority_policy: pins.authority_policy.clone(),
        installation_deadline,
    };
    verified.require_unexpired()?;
    Ok(verified)
}

#[cfg(test)]
#[path = "kagemusha_mobile_bootstrap_v1_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) fn verified_test_bootstrap_v1() -> KagemushaVerifiedMobileBootstrapV1 {
    tests::verified_fixture()
}

#[cfg(test)]
pub(crate) fn expired_test_bootstrap_v1() -> KagemushaVerifiedMobileBootstrapV1 {
    let mut verified = tests::verified_fixture();
    verified.installation_deadline = NativeDeadlineV1::expired_for_test();
    verified
}
