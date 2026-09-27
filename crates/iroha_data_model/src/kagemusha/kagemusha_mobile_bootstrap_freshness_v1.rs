//! Nonce-bound authority time and retained-checkpoint acknowledgements for native startup.
//!
//! The release authority signs a reply only after durably retaining its exact checkpoint
//! for the deployment's policy/network/scope. Its time interval must include the uncertainty
//! of an independently trusted current UTC source. An unsigned node time endpoint, an old
//! block timestamp, and handset wall time do not establish that interval.
//!
//! Native consumers independently install the authority policy and deployment selection,
//! generate a fresh unpredictable nonce, and retain an exclusive pending attempt with a
//! suspend-aware continuous deadline before exposing the request to transport. Authentication
//! here validates data, not that native ownership or deadline. Every startup requires a fresh
//! online attempt; this protocol establishes neither wallet-state antirollback nor device
//! qualification or monetary authority.

use iroha_crypto::{PublicKey, SignatureOf};

use super::{
    KagemushaMobileBootstrapCheckpointV1, KagemushaMobileBootstrapPinsV1,
    KagemushaMobileBootstrapReplayPinV1, KagemushaMobileBootstrapScopeV1,
    KagemushaReleaseAuthorityPolicyV1,
};
use crate::NetworkId;

const APPROVAL_DOMAIN: &str = "iroha:kagemusha:v1:mobile-bootstrap-freshness-approval";

/// Maximum complete canonical freshness response, checked before decoding collections.
pub const KAGEMUSHA_MOBILE_BOOTSTRAP_FRESHNESS_MAX_BYTES_V1: usize = 1024 * 1024;
/// Maximum native elapsed bound for a single pending freshness attempt, in milliseconds.
pub const KAGEMUSHA_MOBILE_BOOTSTRAP_FRESHNESS_MAX_ELAPSED_MS_V1: u64 = 120_000;

/// Authority statement binding a native challenge to a retained bootstrap and current time.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_mobile_bootstrap_freshness_v1::KagemushaMobileBootstrapFreshnessStatementV1"
)]
pub struct KagemushaMobileBootstrapFreshnessStatementV1 {
    /// Wire version; exactly one.
    pub version: u16,
    /// Nonzero unpredictable nonce generated and retained by the pending native attempt.
    pub request_nonce: [u8; 32],
    /// Digest of the independently installed release-authority policy.
    pub authority_policy_digest: [u8; 32],
    /// Independently selected deployment network.
    pub network_id: NetworkId,
    /// Independently selected asset and reserve scope.
    pub scope: KagemushaMobileBootstrapScopeV1,
    /// Domain-separated digest of the exact bootstrap checkpoint durably retained by issuer.
    pub checkpoint_digest: [u8; 32],
    /// Retained sequence for this policy/network/scope; equals the acknowledged checkpoint.
    ///
    /// Before signing, the issuer must atomically reject sequence regression or same-sequence
    /// equivocation and retain this exact sequence/digest in its authoritative durable state.
    pub retained_sequence: u64,
    /// Inclusive lower bound of trusted UTC at the issuer's observation, in Unix milliseconds.
    pub authority_time_lower_ms: u64,
    /// Inclusive upper bound of trusted UTC at the same observation, including uncertainty.
    pub authority_time_upper_ms: u64,
}

impl KagemushaMobileBootstrapFreshnessStatementV1 {
    /// Construct the canonical cross-protocol-separated authority signing payload.
    #[must_use]
    pub fn approval_payload(&self) -> KagemushaMobileBootstrapFreshnessApprovalPayloadV1 {
        KagemushaMobileBootstrapFreshnessApprovalPayloadV1 {
            domain: APPROVAL_DOMAIN.to_owned(),
            statement: *self,
        }
    }
}

/// Domain-separated freshness value signed by one release authority.
#[derive(Clone, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_mobile_bootstrap_freshness_v1::KagemushaMobileBootstrapFreshnessApprovalPayloadV1"
)]
pub struct KagemushaMobileBootstrapFreshnessApprovalPayloadV1 {
    /// Required signature domain, reconstructed by verification rather than caller selected.
    pub domain: String,
    /// Complete nonce, checkpoint-retention and authority-time statement.
    pub statement: KagemushaMobileBootstrapFreshnessStatementV1,
}

/// Partial approval under the independently installed release-authority policy.
#[derive(Clone, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_mobile_bootstrap_freshness_v1::KagemushaMobileBootstrapFreshnessApprovalV1"
)]
pub struct KagemushaMobileBootstrapFreshnessApprovalV1 {
    /// Signer, required to belong to the independently installed policy.
    pub public_key: PublicKey,
    /// Signature over the complete domain-separated statement.
    pub signature: SignatureOf<KagemushaMobileBootstrapFreshnessApprovalPayloadV1>,
}

/// Untrusted portable response; decoding alone does not establish freshness.
#[derive(Clone, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_mobile_bootstrap_freshness_v1::KagemushaMobileBootstrapFreshnessPackageV1"
)]
pub struct KagemushaMobileBootstrapFreshnessPackageV1 {
    /// Complete signed reply.
    pub statement: KagemushaMobileBootstrapFreshnessStatementV1,
    /// Strictly key-ordered distinct approvals satisfying the independent policy threshold.
    pub approvals: Vec<KagemushaMobileBootstrapFreshnessApprovalV1>,
}

/// Independent deployment selection and native attempt inputs for freshness authentication.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaMobileBootstrapFreshnessPinsV1<'a> {
    /// Operator-provisioned policy; a response cannot choose its own verification roots.
    pub authority_policy: &'a KagemushaReleaseAuthorityPolicyV1,
    /// Independently selected network.
    pub network_id: NetworkId,
    /// Independently selected asset and reserve scope.
    pub scope: KagemushaMobileBootstrapScopeV1,
    /// Independently selected release identifier.
    pub release_id: [u8; 32],
    /// Independently selected release-attestation digest.
    pub release_attestation_digest: [u8; 32],
    /// Independently retained nonzero sequence floor.
    pub minimum_sequence: u64,
    /// Previously retained trusted checkpoint, when available.
    pub previous: Option<KagemushaMobileBootstrapReplayPinV1>,
    /// Exact checkpoint selected by the pending native attempt.
    pub checkpoint: &'a KagemushaMobileBootstrapCheckpointV1,
    /// Fresh unpredictable nonce owned by that native attempt, never taken from the reply.
    pub request_nonce: [u8; 32],
    /// Conservative elapsed upper bound since native nonce creation, rounded up to milliseconds.
    ///
    /// This is a native-only input, measured with the same process-owned, suspend-aware
    /// continuous clock and deadline as the pending attempt. Never accept it from a remote
    /// reply, handset wall clock, managed caller, or restored storage. The owner must advance
    /// the returned time interval for any additional native elapsed before consumption.
    pub native_elapsed_ms: u64,
}

/// Authenticated time interval and exact retained checkpoint for the supplied native instant.
///
/// This is protocol data, not a live native capability. The native owner must still enforce
/// exclusive nonce consumption, its original continuous deadline and later elapsed time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaMobileBootstrapFreshnessObservationV1 {
    /// Inclusive trusted UTC lower bound, used to enforce checkpoint issuance.
    pub trusted_time_lower_ms: u64,
    /// Inclusive trusted UTC upper bound, used to enforce exclusive checkpoint expiry.
    pub trusted_time_upper_ms: u64,
    /// Exact checkpoint the authority acknowledges it durably retained before signing.
    pub replay_pin: KagemushaMobileBootstrapReplayPinV1,
}

impl KagemushaMobileBootstrapFreshnessApprovalV1 {
    /// Verify one partial approval with the independent policy and freshness signature domain.
    ///
    /// This does not establish the threshold, native nonce ownership or time freshness.
    ///
    /// # Errors
    /// Rejects invalid or substituted policies, unknown signers and invalid signatures.
    pub fn verify(
        &self,
        statement: &KagemushaMobileBootstrapFreshnessStatementV1,
        policy: &KagemushaReleaseAuthorityPolicyV1,
    ) -> Result<(), String> {
        let policy_digest = policy
            .canonical_digest()
            .map_err(|_| "KAGEMUSHA bootstrap freshness has an invalid native policy".to_owned())?;
        if statement.authority_policy_digest != policy_digest {
            return Err("KAGEMUSHA bootstrap freshness policy digest differs".to_owned());
        }
        if policy
            .authorized_signers
            .binary_search(&self.public_key)
            .is_err()
        {
            return Err("KAGEMUSHA bootstrap freshness signer is not authorized".to_owned());
        }
        self.signature
            .verify(&self.public_key, &statement.approval_payload())
            .map_err(|_| "KAGEMUSHA bootstrap freshness signature failed".to_owned())
    }
}

impl KagemushaMobileBootstrapFreshnessPackageV1 {
    /// Decode one bounded complete canonical response without trusting its contents.
    ///
    /// # Errors
    /// Rejects empty, oversized, noncanonical, malformed, truncated and trailing archives.
    pub fn decode_canonical_exact(archive: &[u8]) -> Result<Self, String> {
        if archive.is_empty() || archive.len() > KAGEMUSHA_MOBILE_BOOTSTRAP_FRESHNESS_MAX_BYTES_V1 {
            return Err("KAGEMUSHA bootstrap freshness exceeds its archive bound".to_owned());
        }
        norito::decode_canonical_with_limits(
            archive,
            norito::canonical_decode_limits(archive.len()),
        )
        .map_err(|_| "KAGEMUSHA bootstrap freshness is not canonical Norito".to_owned())
    }

    /// Authenticate a nonce-bound threshold reply and conservatively advance its UTC interval.
    ///
    /// The issuer observed UTC after receiving this unpredictable nonce and before the native
    /// instant measured by `native_elapsed_ms`. Therefore the current interval is conservatively
    /// `[authority_time_lower_ms, authority_time_upper_ms + native_elapsed_ms]`. Both bounds
    /// must fit the checkpoint's inclusive issuance and exclusive expiry. The full elapsed
    /// allowance includes network and issuer delays; none may be subtracted using remote data.
    ///
    /// # Errors
    /// Rejects substituted deployments/checkpoints/nonces, sequence rollback or equivocation,
    /// invalid threshold signatures, invalid intervals, overflow, expiry or excessive elapsed.
    pub fn authenticate(
        &self,
        pins: &KagemushaMobileBootstrapFreshnessPinsV1<'_>,
    ) -> Result<KagemushaMobileBootstrapFreshnessObservationV1, String> {
        let statement = self.statement;
        if statement.version != 1
            || pins.request_nonce == [0; 32]
            || statement.request_nonce != pins.request_nonce
            || statement.network_id != pins.network_id
            || statement.scope != pins.scope
            || statement.retained_sequence != pins.checkpoint.sequence
            || statement.authority_time_lower_ms == 0
            || statement.authority_time_lower_ms > statement.authority_time_upper_ms
            || pins.native_elapsed_ms > KAGEMUSHA_MOBILE_BOOTSTRAP_FRESHNESS_MAX_ELAPSED_MS_V1
        {
            return Err(
                "KAGEMUSHA bootstrap freshness differs from native attempt pins".to_owned(),
            );
        }
        if self.approvals.len() < usize::from(pins.authority_policy.threshold)
            || self.approvals.len() > pins.authority_policy.authorized_signers.len()
            || !self
                .approvals
                .windows(2)
                .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(
                "KAGEMUSHA bootstrap freshness requires distinct threshold approvals".to_owned(),
            );
        }
        // This validates the policy independently even when a malformed zero threshold would
        // otherwise leave the approval loop empty.
        let policy_digest = pins
            .authority_policy
            .canonical_digest()
            .map_err(|_| "KAGEMUSHA bootstrap freshness has an invalid native policy".to_owned())?;
        if statement.authority_policy_digest != policy_digest {
            return Err("KAGEMUSHA bootstrap freshness policy digest differs".to_owned());
        }
        for approval in &self.approvals {
            approval.verify(&statement, pins.authority_policy)?;
        }
        let upper_ms = statement
            .authority_time_upper_ms
            .checked_add(pins.native_elapsed_ms)
            .ok_or_else(|| "KAGEMUSHA bootstrap freshness time overflow".to_owned())?;
        let mut checkpoint_pins = KagemushaMobileBootstrapPinsV1 {
            authority_policy: pins.authority_policy,
            network_id: pins.network_id,
            scope: pins.scope,
            release_id: pins.release_id,
            release_attestation_digest: pins.release_attestation_digest,
            minimum_sequence: pins.minimum_sequence,
            previous: pins.previous,
            trusted_now_ms: statement.authority_time_lower_ms,
        };
        let checkpoint_digest = pins.checkpoint.validate_pins(&checkpoint_pins)?;
        checkpoint_pins.trusted_now_ms = upper_ms;
        pins.checkpoint.validate_pins(&checkpoint_pins)?;
        if statement.checkpoint_digest != checkpoint_digest {
            return Err("KAGEMUSHA bootstrap freshness checkpoint digest differs".to_owned());
        }
        Ok(KagemushaMobileBootstrapFreshnessObservationV1 {
            trusted_time_lower_ms: statement.authority_time_lower_ms,
            trusted_time_upper_ms: upper_ms,
            replay_pin: KagemushaMobileBootstrapReplayPinV1 {
                sequence: statement.retained_sequence,
                checkpoint_digest,
            },
        })
    }
}

#[cfg(test)]
#[path = "kagemusha_mobile_bootstrap_freshness_v1_tests.rs"]
mod tests;
