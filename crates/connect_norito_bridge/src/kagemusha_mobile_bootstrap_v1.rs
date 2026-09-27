//! Authority-signed mobile trust bootstrap, independent of operation-status responses.
//!
//! A package supplies authenticated finality coordinates under an independently installed
//! native policy. It cannot select that policy, admit value, qualify hardware, or authenticate
//! proof artifacts. The native host must still authenticate the matching release and retain
//! sequence/time freshness independently of the downloaded package and rollbackable app data.

use std::time::Duration;

use iroha_data_model::{
    NetworkId,
    block::consensus_v2::HeightContextId,
    kagemusha::{
        KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1, KagemushaMobileBootstrapCheckpointV1,
        KagemushaMobileBootstrapPackageV1, KagemushaMobileBootstrapPinsV1,
        KagemushaMobileBootstrapReplayPinV1, KagemushaMobileBootstrapScopeV1,
        KagemushaReleaseAuthorityPolicyV1,
    },
};

use crate::kagemusha_core_coordinator_v1::native_deadline::{
    MAX_LIFETIME, NativeContinuousInstantV1, NativeDeadlineV1,
};

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

/// Verify one bounded canonical package against synchronously read independent native pins.
///
/// An exact still-valid retry at the previous sequence is accepted. An older sequence or
/// changed checkpoint at that sequence is rejected. The caller must persist the returned
/// replay pin before using the checkpoint; this pure verifier cannot secure rollbackable
/// storage or make a handset clock trustworthy.
/// The callback must read current native freshness when invoked, rather than return an older
/// snapshot. Its execution and any suspension during it consume the installation lease.
///
/// # Errors
/// Rejects malformed/noncanonical archives, invalid scope or freshness, policy substitution,
/// insufficient/duplicate/unknown approvals, invalid signatures, and unset finality contexts.
pub fn verify_kagemusha_mobile_bootstrap_v1<'pins>(
    archive: &[u8],
    read_pins: impl FnOnce() -> Result<KagemushaMobileBootstrapPinsV1<'pins>, String>,
) -> Result<KagemushaVerifiedMobileBootstrapV1, String> {
    if archive.is_empty() || archive.len() > KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1 {
        return Err("KAGEMUSHA mobile bootstrap exceeds its archive bound".to_owned());
    }
    // Anchor before reading trusted UTC: suspension after that read must not renew its age.
    let verification_started = NativeContinuousInstantV1::now()
        .map_err(|_| "KAGEMUSHA mobile bootstrap continuous clock is unavailable".to_owned())?;
    verify_from_reading(archive, verification_started, read_pins)
}

fn verify_from_reading<'pins>(
    archive: &[u8],
    verification_started: NativeContinuousInstantV1,
    read_pins: impl FnOnce() -> Result<KagemushaMobileBootstrapPinsV1<'pins>, String>,
) -> Result<KagemushaVerifiedMobileBootstrapV1, String> {
    let pins = read_pins()?;
    let package = KagemushaMobileBootstrapPackageV1::decode_canonical_exact(archive)?;
    let checkpoint_digest = package.authenticate(&pins)?;
    let checkpoint = package.checkpoint;
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
