//! Governed native gateway policy and execution claims.
//!
//! These values describe the consensus owner's inputs. Decoding a policy or execution claim
//! does not authenticate it. The native executor must derive execution coordinates from its
//! directly signed instruction, check current ledger permissions and retain the original
//! admission history. Gateway operators attest Torii's signature and route validation; the
//! gateway quota owner does not independently acquire the private token presentation.

use std::collections::BTreeSet;

use iroha_crypto::{Hash, PublicKey};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{StreamTokenGatewayAdmissionErrorV1, StreamTokenGatewayAdmissionQualificationV1};
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize, NetworkId,
    account::{AccountController, AccountId},
    sorafs::reputation::derive_stream_token_gateway_id_v1,
};

/// Maximum number of independently authorized operators or observers in one gateway policy.
pub const STREAM_TOKEN_GATEWAY_MAX_AUTHORITIES_V1: usize = 16;
/// Maximum original observation age admitted by a native gateway policy.
pub const STREAM_TOKEN_GATEWAY_MAX_OBSERVATION_AGE_MS_V1: u64 = 300_000;
/// Maximum complete canonical native gateway policy frame.
pub const STREAM_TOKEN_GATEWAY_MAX_POLICY_BYTES_V1: usize = 16 * 1024;

mod check;
pub use check::{
    STREAM_TOKEN_GATEWAY_MAX_PENDING_READBACK_BYTES_V1, StreamTokenGatewayCheckSubjectV1,
    StreamTokenGatewayCheckV1, StreamTokenGatewayFinalityFloorV1,
    stream_token_gateway_pending_readback_digest_v1,
};

mod request;
pub use request::{
    STREAM_TOKEN_GATEWAY_MAX_EXPIRY_ITEMS_V1, STREAM_TOKEN_GATEWAY_MAX_REQUEST_BYTES_V1,
    StreamTokenGatewayActionV1, StreamTokenGatewayRequestV1,
};

const POLICY_DOMAIN: &[u8] = b"iroha.sorafs.stream-token.gateway-policy.v1\0";

/// Exact governed binding for a native gateway admission authority.
///
/// The stable gateway identity excludes keys and revisions, so rotating an operator cannot
/// reset quotas, leases, callback sequences or replay history. Old admission records retain
/// their complete original qualification. Reducing a live capacity never evicts obligations;
/// further allocation waits for sufficient capacity to drain.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::native::StreamTokenGatewayPolicyV1"
)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenGatewayPolicyV1 {
    /// Exact original global network identity.
    pub network_id: NetworkId,
    /// Canonical governed gateway label used to derive the stable gateway identifier.
    pub compliance_gateway_id: String,
    /// Current revision, exact policy digest and bounded allocation parameters.
    pub qualification: StreamTokenGatewayAdmissionQualificationV1,
    /// Accounts allowed to attest new requests and discharge retained gateway obligations.
    pub operators: BTreeSet<AccountId>,
    /// Independent accounts allowed to challenge native readback.
    pub observers: BTreeSet<AccountId>,
    /// Inclusive beginning of this policy's new-admission interval.
    pub valid_from_unix_ms: u64,
    /// Exclusive end of this policy's new-admission interval.
    pub valid_until_unix_ms: u64,
    /// Maximum delay from the original Torii observation to native execution.
    pub max_observation_age_ms: u64,
    /// Whether new admissions are allowed; disabling never discards pending callbacks or leases.
    pub admission_enabled: bool,
}

impl StreamTokenGatewayPolicyV1 {
    /// Calculate the canonical policy commitment, excluding only its own digest field.
    ///
    /// Callers set `qualification.policy_digest` to this result before validating a new policy.
    /// This operation does not authenticate or authorize the policy.
    ///
    /// # Errors
    ///
    /// Rejects oversized or unencodable policy material before allocating the canonical frame.
    pub fn calculate_policy_digest(&self) -> Result<[u8; 32], StreamTokenGatewayAdmissionErrorV1> {
        if norito::canonical_frame_len(self)
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::InvalidRequest)?
            > STREAM_TOKEN_GATEWAY_MAX_POLICY_BYTES_V1
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest);
        }
        let mut material = self.clone();
        material.qualification.policy_digest = [0; 32];
        let frame = norito::encode_canonical(&material)
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::InvalidRequest)?;
        let mut preimage = Vec::with_capacity(POLICY_DOMAIN.len() + frame.len());
        preimage.extend_from_slice(POLICY_DOMAIN);
        preimage.extend_from_slice(&frame);
        Ok(*Hash::new(preimage).as_ref())
    }

    /// Validate the exact network-derived identity, authority separation and policy commitment.
    ///
    /// # Errors
    ///
    /// Rejects malformed identities, inert or overlapping authority sets, unbounded timestamps,
    /// invalid capacity parameters and a substituted policy digest.
    pub fn validate(&self) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        self.qualification.validate()?;
        let gateway_id =
            derive_stream_token_gateway_id_v1(&self.network_id, &self.compliance_gateway_id)
                .map_err(|_| StreamTokenGatewayAdmissionErrorV1::BindingMismatch)?;
        if gateway_id != self.qualification.gateway_id
            || self.operators.is_empty()
            || self.operators.len() > STREAM_TOKEN_GATEWAY_MAX_AUTHORITIES_V1
            || self.observers.is_empty()
            || self.observers.len() > STREAM_TOKEN_GATEWAY_MAX_AUTHORITIES_V1
            || !self.operators.is_disjoint(&self.observers)
            || self.valid_from_unix_ms == 0
            || self.valid_until_unix_ms <= self.valid_from_unix_ms
            || self.valid_until_unix_ms == u64::MAX
            || self.max_observation_age_ms == 0
            || self.max_observation_age_ms > STREAM_TOKEN_GATEWAY_MAX_OBSERVATION_AGE_MS_V1
            || self.calculate_policy_digest()? != self.qualification.policy_digest
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::BindingMismatch);
        }
        // Different account controllers can still share keys (for example, a single-key
        // operator and a threshold-one observer). Check the underlying bounded key sets too.
        if !controller_keys(&self.operators).is_disjoint(&controller_keys(&self.observers)) {
            return Err(StreamTokenGatewayAdmissionErrorV1::BindingMismatch);
        }
        Ok(())
    }

    /// Validate the adjacent revision of the same gateway without resetting its identity.
    ///
    /// The native owner must additionally compare and atomically preserve committed state;
    /// this structural check alone does not execute a governance transition.
    ///
    /// # Errors
    ///
    /// Rejects a malformed predecessor, revision gap, replay or replacement network/gateway.
    pub fn validate_replacement(
        &self,
        previous: &Self,
    ) -> Result<(), StreamTokenGatewayAdmissionErrorV1> {
        previous.validate()?;
        self.validate()?;
        if self.network_id != previous.network_id
            || self.compliance_gateway_id != previous.compliance_gateway_id
            || self.qualification.gateway_id != previous.qualification.gateway_id
            || previous.qualification.revision.checked_add(1) != Some(self.qualification.revision)
        {
            return Err(StreamTokenGatewayAdmissionErrorV1::Conflict);
        }
        Ok(())
    }

    /// Whether this policy permits a new admission at a deterministic execution time.
    ///
    /// This checks policy eligibility only, not account permissions, request freshness, quotas
    /// or committed finality. Recovery of existing obligations does not use this predicate.
    #[must_use]
    pub fn allows_admission_at(&self, execution_unix_ms: u64) -> bool {
        self.admission_enabled
            && self.valid_from_unix_ms <= execution_unix_ms
            && execution_unix_ms < self.valid_until_unix_ms
    }
}

fn controller_keys(accounts: &BTreeSet<AccountId>) -> BTreeSet<&PublicKey> {
    let mut keys = BTreeSet::new();
    for account in accounts {
        match account.controller() {
            AccountController::Single(key) => {
                keys.insert(key);
            }
            AccountController::Multisig(policy) => {
                keys.extend(
                    policy
                        .members()
                        .iter()
                        .map(crate::account::controller::MultisigMember::public_key),
                );
            }
        }
    }
    keys
}

/// Actual directly signed native gateway execution coordinates.
///
/// Core derives these fields from the executing transaction and block. A decoded detached
/// claim cannot establish successful execution or current finalized State.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::native::StreamTokenGatewayExecutionV1"
)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenGatewayExecutionV1 {
    /// Actual one-based block height.
    pub height: u64,
    /// Hash of the exact signed native transaction entry.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub transaction_hash: [u8; 32],
    /// Zero-based transaction entry index in the block execution proof.
    pub entry_index: u32,
    /// Zero-based directly signed native instruction index.
    pub instruction_index: u32,
    /// Actual deterministic executing block timestamp in Unix milliseconds.
    pub recorded_at_unix_ms: u64,
    /// Account that signed the exact native instruction.
    pub authority: AccountId,
}

#[cfg(test)]
mod tests;
