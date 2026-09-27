//! Native ownership of nonce-bound, authority-retained bootstrap freshness.
//!
//! The transport carries bytes only. A native attempt owns its nonce and continuous clock,
//! authenticates the authority's signed time interval and retained checkpoint, and consumes
//! itself on completion. This establishes a short startup lease, not wallet anti-rollback.

use iroha_data_model::{
    NetworkId,
    kagemusha::{
        KagemushaMobileBootstrapCheckpointV1, KagemushaMobileBootstrapFreshnessPackageV1,
        KagemushaMobileBootstrapFreshnessPinsV1, KagemushaMobileBootstrapPackageV1,
        KagemushaMobileBootstrapReplayPinV1, KagemushaMobileBootstrapScopeV1,
        KagemushaReleaseAuthorityPolicyV1,
    },
};
use rand::{TryRngCore as _, rngs::OsRng};

use crate::{
    KagemushaTestnetNativeStartupContextV1, KagemushaTestnetNativeStartupFreshnessProviderV1,
    KagemushaTestnetNativeStartupFreshnessV1,
    kagemusha_core_coordinator_v1::native_deadline::{
        MAX_LIFETIME, NativeContinuousInstantV1, NativeDeadlineV1,
    },
};

struct DeploymentPins {
    policy: KagemushaReleaseAuthorityPolicyV1,
    network_id: NetworkId,
    scope: KagemushaMobileBootstrapScopeV1,
    release_id: [u8; 32],
    release_attestation_digest: [u8; 32],
    minimum_sequence: u64,
    previous: Option<KagemushaMobileBootstrapReplayPinV1>,
}

/// One native-generated request, consumed even when the downloaded reply is rejected.
///
/// Neither this owner nor its deadline can be decoded, cloned, or created from a transport
/// nonce. Applications send `request_nonce()` and the original bootstrap archive to their
/// independently configured authority. That service must authenticate the bootstrap and
/// durably retain its exact checkpoint before issuing a threshold-signed freshness reply.
#[must_use]
pub struct KagemushaNativeBootstrapFreshnessAttemptV1 {
    pins: DeploymentPins,
    package: KagemushaMobileBootstrapPackageV1,
    nonce: [u8; 32],
    started: NativeContinuousInstantV1,
    deadline: NativeDeadlineV1,
}

impl KagemushaNativeBootstrapFreshnessAttemptV1 {
    /// Start a bounded native attempt using independently provisioned deployment pins.
    ///
    /// `minimum_sequence` and `previous` are independent native floors, never read from
    /// downloaded app storage. The returned checkpoint remains untrusted until `complete`.
    /// No network request or durable wallet mutation occurs in this method.
    ///
    /// # Errors
    /// Rejects unavailable native entropy/time, malformed archives and invalid native floors.
    pub fn begin(
        context: &KagemushaTestnetNativeStartupContextV1,
        bootstrap_archive: &[u8],
        minimum_sequence: u64,
        previous: Option<KagemushaMobileBootstrapReplayPinV1>,
    ) -> Result<Self, String> {
        let started = NativeContinuousInstantV1::now()
            .map_err(|_| "bootstrap freshness continuous clock is unavailable".to_owned())?;
        let deadline = NativeDeadlineV1::from_reading(started, MAX_LIFETIME)
            .map_err(|_| "bootstrap freshness request deadline is invalid".to_owned())?;
        if minimum_sequence == 0
            || previous.is_some_and(|pin| pin.sequence == 0 || pin.checkpoint_digest == [0; 32])
        {
            return Err("bootstrap freshness native replay floor is invalid".to_owned());
        }
        context
            .authority_policy
            .canonical_digest()
            .map_err(|_| "bootstrap freshness native authority policy is invalid".to_owned())?;
        let package = KagemushaMobileBootstrapPackageV1::decode_canonical_exact(bootstrap_archive)?;
        let mut nonce = [0; 32];
        OsRng
            .try_fill_bytes(&mut nonce)
            .map_err(|_| "bootstrap freshness native entropy is unavailable".to_owned())?;
        if nonce == [0; 32] {
            return Err("bootstrap freshness native nonce is invalid".to_owned());
        }
        deadline
            .check()
            .map_err(|_| "bootstrap freshness request already expired".to_owned())?;
        Ok(Self {
            pins: DeploymentPins {
                policy: context.authority_policy.clone(),
                network_id: context.network_id,
                scope: context.scope,
                release_id: context.release_id,
                release_attestation_digest: context.release_attestation_digest,
                minimum_sequence,
                previous,
            },
            package,
            nonce,
            started,
            deadline,
        })
    }

    /// Return the native-generated nonce to transport to the authority.
    #[must_use]
    pub const fn request_nonce(&self) -> [u8; 32] {
        self.nonce
    }

    /// Inspect the exact candidate checkpoint; this accessor grants no trust.
    #[must_use]
    pub const fn checkpoint(&self) -> &KagemushaMobileBootstrapCheckpointV1 {
        &self.package.checkpoint
    }

    /// Consume this request and authenticate a complete nonce-bound authority reply.
    ///
    /// The original bootstrap approvals are also authenticated. Transport, decoding and
    /// signature verification all consume the original suspend-inclusive request lifetime.
    /// The returned provider can provision only startup freshness; proof artifacts and
    /// durable wallet recovery are still checked by the native startup installer.
    ///
    /// # Errors
    /// Rejects stale/foreign replies, invalid signatures, substituted deployment/checkpoint
    /// pins, missing authoritative retention, clock faults, or any expired time interval.
    pub fn complete(
        self,
        freshness_archive: &[u8],
    ) -> Result<KagemushaOnlineBootstrapFreshnessV1, String> {
        let native_elapsed_ms = elapsed_millis(&self.deadline, self.started)?;
        let reply =
            KagemushaMobileBootstrapFreshnessPackageV1::decode_canonical_exact(freshness_archive)?;
        let observation = reply.authenticate(&KagemushaMobileBootstrapFreshnessPinsV1 {
            authority_policy: &self.pins.policy,
            network_id: self.pins.network_id,
            scope: self.pins.scope,
            release_id: self.pins.release_id,
            release_attestation_digest: self.pins.release_attestation_digest,
            minimum_sequence: self.pins.minimum_sequence,
            previous: self.pins.previous,
            checkpoint: &self.package.checkpoint,
            request_nonce: self.nonce,
            native_elapsed_ms,
        })?;
        let pin = observation.replay_pin;
        self.package.authenticate(
            &iroha_data_model::kagemusha::KagemushaMobileBootstrapPinsV1 {
                authority_policy: &self.pins.policy,
                network_id: self.pins.network_id,
                scope: self.pins.scope,
                release_id: self.pins.release_id,
                release_attestation_digest: self.pins.release_attestation_digest,
                minimum_sequence: self.pins.minimum_sequence,
                previous: Some(pin),
                trusted_now_ms: observation.trusted_time_upper_ms,
            },
        )?;
        let provider = KagemushaOnlineBootstrapFreshnessV1 {
            started: self.started,
            deadline: self.deadline,
            authority_time_upper_ms: reply.statement.authority_time_upper_ms,
            expires_at_ms: self.package.checkpoint.expires_at_ms,
            pin,
        };
        // Account for all work after the first reading, including signature verification.
        provider.read_freshness()?;
        Ok(provider)
    }
}

/// Opaque short-lived freshness backed by an authority's exact retained checkpoint.
///
/// Only consuming a successful native request constructs this value. Each read conservatively
/// advances the signed UTC upper bound by the full elapsed time since request creation. This
/// may expire early; it cannot extend a lease across device sleep or a slow transport reply.
/// It is not a hardware counter and does not protect an offline wallet journal from rollback.
pub struct KagemushaOnlineBootstrapFreshnessV1 {
    started: NativeContinuousInstantV1,
    deadline: NativeDeadlineV1,
    authority_time_upper_ms: u64,
    expires_at_ms: u64,
    pin: KagemushaMobileBootstrapReplayPinV1,
}

impl KagemushaTestnetNativeStartupFreshnessProviderV1 for KagemushaOnlineBootstrapFreshnessV1 {
    fn read_freshness(&self) -> Result<KagemushaTestnetNativeStartupFreshnessV1, String> {
        let elapsed = elapsed_millis(&self.deadline, self.started)?;
        let trusted_now_ms = self
            .authority_time_upper_ms
            .checked_add(elapsed)
            .filter(|upper| *upper < self.expires_at_ms)
            .ok_or_else(|| "bootstrap freshness signed time interval expired".to_owned())?;
        Ok(KagemushaTestnetNativeStartupFreshnessV1 {
            trusted_now_ms,
            minimum_sequence: self.pin.sequence,
            previous: Some(self.pin),
        })
    }

    fn retain_verified_bootstrap(
        &self,
        pin: KagemushaMobileBootstrapReplayPinV1,
    ) -> Result<(), String> {
        self.read_freshness()?;
        if pin != self.pin {
            return Err("bootstrap freshness authority retained a different checkpoint".to_owned());
        }
        // The signed reply attests that the authority already durably retained this exact pin.
        // Local app storage is not substituted for that authoritative retention.
        Ok(())
    }
}

fn elapsed_millis(
    deadline: &NativeDeadlineV1,
    started: NativeContinuousInstantV1,
) -> Result<u64, String> {
    let now = deadline
        .check()
        .map_err(|_| "bootstrap freshness native request expired or invalid".to_owned())?;
    let elapsed = now
        .checked_duration_since(started)
        .ok_or_else(|| "bootstrap freshness continuous time regressed".to_owned())?;
    u64::try_from(elapsed.as_nanos().div_ceil(1_000_000))
        .map_err(|_| "bootstrap freshness elapsed time overflowed".to_owned())
}

#[cfg(test)]
#[path = "kagemusha_mobile_bootstrap_online_v1_tests.rs"]
mod tests;
