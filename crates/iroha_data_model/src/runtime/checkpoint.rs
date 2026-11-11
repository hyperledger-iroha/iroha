//! The boundaries of a root's history, as committed State
//! (`specs/live_runtime_upgrades.md` §7.3, §13).
//!
//! Two facts about a root's history are certified policy rather than executable code: the
//! adoption anchor of a network that adopted the foundation at a checkpoint
//! ([`AdoptionRecordV1`]), and the startup floor, the oldest checkpoint a release promises to
//! load ([`StartupFloorV1`]). Each is a boundary: no release re-executes a block at or below
//! the highest one in force.
//!
//! The kernel holds them in `world.runtime_boundaries`, keyed by height, as storage of the
//! `iroha.runtime.lifecycle.v1` addition, so no release catalog or selection has to hold a
//! network-specific identity and nothing a root committed before it activated that addition is
//! ever read as a boundary:
//!
//! - [`RuntimeAdoptionAnchorV1`] is the adoption record with one statement from every validator
//!   of the committee that ratifies it. A root ratifies it once and it never changes.
//! - [`RuntimeStartupFloorV1`] is a floor with one statement from every validator and the
//!   committed time from which it is in force, which the kernel sets to the committed
//!   checkpoint policy's notice after the block that raises it. The floor only moves forward,
//!   and only to a checkpoint that policy calls recoverable.
//!
//! The statements are the authority: any account relays a record that carries all of them, and
//! the instruction that commits it checks everything committed State can decide: the network,
//! the committed block hash at the named height, the transition rules and the all-`n` coverage
//! of the current committee. What State cannot decide, the validators vouch for: each node signs
//! a record only after checking it against its own store (`iroha_core::runtime::checkpoint`).
//! All `n` sign, never a quorum, for the reason a change set needs every validator's support
//! (§6).

use std::collections::BTreeSet;

use iroha_crypto::{Hash, PrivateKey, PublicKey, Signature};
use norito::codec::{Decode, Encode};

use super::{
    adoption::{AdoptionAttestationV1, SignedAdoptionAttestationV1},
    lifecycle::AdoptionRecordV1,
};
use crate::{NetworkId, parameter::RuntimeCheckpointPolicyV1};

/// Domain separator of [`StartupFloorV1::id`].
pub const STARTUP_FLOOR_DOMAIN: &[u8] = b"iroha:runtime:startup-floor:v1\0";
/// Signature purpose of a validator's startup-floor statement.
pub const STARTUP_FLOOR_ATTESTATION_DOMAIN: &[u8] = b"iroha:runtime:startup-floor-attestation:v1\0";

/// The oldest checkpoint the releases of a root promise to load (§7.3).
///
/// A node restores no State below it and re-executes no block at or below it. Once it is in
/// force, code that only blocks at or below it needed can be deleted (§7.4).
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::checkpoint::StartupFloorV1")]
pub struct StartupFloorV1 {
    /// Network the floor belongs to.
    pub network: NetworkId,
    /// Height of the floor checkpoint: the published State after this block.
    pub height: u64,
    /// Hash of the block at that height.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub block_hash: [u8; 32],
    /// Committed time of that block, in milliseconds.
    pub committed_ms: u64,
}

/// A record, a statement or a transition that breaks the rules of this module.
#[derive(Debug, thiserror::Error)]
pub enum CheckpointError {
    /// The record names another network.
    #[error("the record belongs to another network")]
    Network,
    /// The record names a height that is not committed yet, or height zero.
    #[error("the record names a block that is not committed")]
    NotCommitted,
    /// The record names another block hash than the chain committed at its height.
    #[error("the record names another block than the chain committed at its height")]
    BlockHash,
    /// The committee is outside Sumeragi's `3f + 1` bound or repeats a member.
    #[error("the committee must have 3f+1 distinct members for 1 <= f <= 10")]
    Committee,
    /// The statements are not exactly one valid statement of this record per committee member.
    #[error("the record needs exactly one valid statement from every committee member")]
    Coverage,
    /// An adoption anchor is already ratified.
    #[error("a ratified adoption anchor never changes")]
    AnchorRatified,
    /// A startup floor is already committed: the root's history starts above the anchor.
    #[error("an adoption anchor is ratified before any startup floor")]
    AnchorAfterFloor,
    /// No checkpoint policy is committed, so no floor is raised.
    #[error("the checkpoint policy is not committed; no startup floor is raised")]
    NoPolicy,
    /// An earlier floor has not come into force yet.
    #[error("an earlier startup floor is still waiting out its notice")]
    Pending,
    /// The floor does not lie above the latest floor and the adoption anchor.
    #[error("the startup floor only moves forward, above the adoption anchor")]
    Regressed,
    /// The floor checkpoint is younger than the recovery window when the floor comes into force.
    #[error("the floor checkpoint is younger than the committed recovery window")]
    NotRecoverable,
    /// The committed time at which the floor would come into force is not representable.
    #[error("the committed notice puts the floor beyond representable time")]
    Notice,
    /// The record cannot be encoded.
    #[error("record encoding failed: {0}")]
    Encoding(String),
}

impl StartupFloorV1 {
    /// Identity of the floor: `H(STARTUP_FLOOR_DOMAIN ‖ canonical bytes)`.
    ///
    /// # Errors
    /// The floor cannot be encoded.
    pub fn id(&self) -> Result<[u8; 32], CheckpointError> {
        let bytes = norito::encode_canonical(self)
            .map_err(|error| CheckpointError::Encoding(error.to_string()))?;
        Ok(Hash::new_from_chunks(&[STARTUP_FLOOR_DOMAIN, &bytes]).into())
    }
}

/// A validator's statement about one exact startup floor.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::checkpoint::StartupFloorAttestationV1")]
pub struct StartupFloorAttestationV1 {
    /// [`StartupFloorV1::id`] of the floor.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub floor: [u8; 32],
    /// The validator that signs the statement.
    pub validator: PublicKey,
}

/// One validator's startup-floor statement and its signature.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::checkpoint::SignedStartupFloorAttestationV1")]
pub struct SignedStartupFloorAttestationV1 {
    /// The statement.
    pub attestation: StartupFloorAttestationV1,
    /// Signature over the domain-separated canonical statement.
    pub signature: Signature,
}

impl StartupFloorAttestationV1 {
    fn signing_payload(&self) -> Result<Vec<u8>, CheckpointError> {
        let wire = norito::encode_canonical(self)
            .map_err(|error| CheckpointError::Encoding(error.to_string()))?;
        let mut payload = Vec::with_capacity(STARTUP_FLOOR_ATTESTATION_DOMAIN.len() + wire.len());
        payload.extend_from_slice(STARTUP_FLOOR_ATTESTATION_DOMAIN);
        payload.extend_from_slice(&wire);
        Ok(payload)
    }

    /// Sign the statement with the validator's key.
    ///
    /// # Errors
    /// The statement cannot be encoded or the key cannot sign.
    pub fn sign(
        self,
        key: &PrivateKey,
    ) -> Result<SignedStartupFloorAttestationV1, CheckpointError> {
        let signature = Signature::try_new(key, &self.signing_payload()?)
            .map_err(|error| CheckpointError::Encoding(error.to_string()))?;
        Ok(SignedStartupFloorAttestationV1 {
            attestation: self,
            signature,
        })
    }
}

/// Whether `statements` are exactly one valid statement per member of `committee`, where
/// `statement` yields a statement's validator and whether it is a valid statement of the record.
fn covered<S>(
    committee: &[PublicKey],
    statements: &[S],
    statement: impl Fn(&S) -> (&PublicKey, bool),
) -> Result<(), CheckpointError> {
    if !iroha_model_base::topology::is_valid_committee_size(committee.len()) {
        return Err(CheckpointError::Committee);
    }
    let members: BTreeSet<&PublicKey> = committee.iter().collect();
    if members.len() != committee.len() {
        return Err(CheckpointError::Committee);
    }
    if statements.len() != committee.len() {
        return Err(CheckpointError::Coverage);
    }
    let mut seen = BTreeSet::new();
    for signed in statements {
        let (validator, valid) = statement(signed);
        if !valid || !members.contains(validator) || !seen.insert(validator) {
            return Err(CheckpointError::Coverage);
        }
    }
    Ok(())
}

/// The committed block hash at `height`, which the record must name.
fn committed_block(
    height: u64,
    block_hash: [u8; 32],
    committed_height: u64,
    block_hash_at: impl Fn(u64) -> Option<[u8; 32]>,
) -> Result<(), CheckpointError> {
    if height == 0 || height > committed_height {
        return Err(CheckpointError::NotCommitted);
    }
    match block_hash_at(height) {
        Some(committed) if committed == block_hash => Ok(()),
        Some(_) => Err(CheckpointError::BlockHash),
        None => Err(CheckpointError::NotCommitted),
    }
}

/// The committed trust anchor of a network that adopted the foundation at a checkpoint (§13):
/// the adoption record and the statement of every validator of the committee that committed it.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::checkpoint::RuntimeAdoptionAnchorV1")]
pub struct RuntimeAdoptionAnchorV1 {
    /// The adoption record.
    pub record: AdoptionRecordV1,
    /// One statement of the record from every member of the ratifying committee.
    pub attestations: Vec<SignedAdoptionAttestationV1>,
}

impl RuntimeAdoptionAnchorV1 {
    /// The statement `validator` signs to ratify `record`.
    ///
    /// # Errors
    /// The record has no identity.
    pub fn statement(
        record: &AdoptionRecordV1,
        validator: PublicKey,
    ) -> Result<AdoptionAttestationV1, CheckpointError> {
        Ok(AdoptionAttestationV1 {
            adoption: record
                .id()
                .map_err(|error| CheckpointError::Encoding(error.to_string()))?,
            validator,
        })
    }

    /// Check that the root of `network`, whose committed tip is `committed_height`, may ratify
    /// this anchor (§13): it holds no boundary yet (`lowest` is the lowest one it holds), the
    /// record names this network and the block the chain committed at its height, and every
    /// member of the current `committee` signed it.
    ///
    /// The record names the committee of the adoption height, which may differ from the
    /// committee that ratifies it: the current validators vouch for it with the rest of the
    /// record.
    ///
    /// # Errors
    /// The first rule the ratification violates.
    pub fn check_ratification(
        &self,
        lowest: Option<&RuntimeBoundaryV1>,
        network: &NetworkId,
        committee: &[PublicKey],
        committed_height: u64,
        block_hash_at: impl Fn(u64) -> Option<[u8; 32]>,
    ) -> Result<(), CheckpointError> {
        match lowest {
            Some(RuntimeBoundaryV1::AdoptionAnchor(_)) => {
                return Err(CheckpointError::AnchorRatified);
            }
            Some(RuntimeBoundaryV1::StartupFloor(_)) => {
                return Err(CheckpointError::AnchorAfterFloor);
            }
            None => {}
        }
        if self.record.network != *network {
            return Err(CheckpointError::Network);
        }
        committed_block(
            self.record.height,
            self.record.block_hash,
            committed_height,
            block_hash_at,
        )?;
        covered(committee, &self.attestations, |signed| {
            (
                &signed.attestation.validator,
                signed.verify_for(&self.record).is_ok(),
            )
        })
    }
}

/// A committed startup floor of a root (§7.3): the floor, the committed time from which it is
/// in force, and the statement of every validator of the committee that committed it.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::checkpoint::RuntimeStartupFloorV1")]
pub struct RuntimeStartupFloorV1 {
    /// The floor.
    pub floor: StartupFloorV1,
    /// Committed time from which the floor is in force: the checkpoint policy's notice after
    /// the block that raised it.
    pub effective_ms: u64,
    /// One statement of the floor from every member of the committee that committed it.
    pub attestations: Vec<SignedStartupFloorAttestationV1>,
}

impl SignedStartupFloorAttestationV1 {
    /// Verify the statement for `floor` against its own declared key.
    ///
    /// # Errors
    /// The statement names another floor or its signature does not verify.
    pub fn verify_for(&self, floor: &StartupFloorV1) -> Result<(), CheckpointError> {
        if self.attestation.floor != floor.id()? {
            return Err(CheckpointError::Coverage);
        }
        self.signature
            .verify(
                &self.attestation.validator,
                &self.attestation.signing_payload()?,
            )
            .map_err(|_| CheckpointError::Coverage)
    }
}

impl RuntimeStartupFloorV1 {
    /// The statement `validator` signs for `floor`.
    ///
    /// # Errors
    /// The floor has no identity.
    pub fn statement(
        floor: &StartupFloorV1,
        validator: PublicKey,
    ) -> Result<StartupFloorAttestationV1, CheckpointError> {
        Ok(StartupFloorAttestationV1 {
            floor: floor.id()?,
            validator,
        })
    }

    /// The floor the root of `network` commits when it raises its floor to `floor` in the block
    /// committed at `now_ms`, with its tip at `committed_height` (§7.3, "Advancing the floor").
    ///
    /// The checkpoint `policy` is committed. The `latest` floor the root holds is in force.
    /// `floor` names this network and the block the chain committed at its height, and lies
    /// above the latest floor and the adoption `anchor`. It comes into force the policy's notice
    /// after `now_ms`, and its checkpoint is then at least the recovery window old. Every member
    /// of the current `committee` signed it.
    ///
    /// # Errors
    /// The first rule the raise violates.
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is one independent committed input of the raise"
    )]
    pub fn raise(
        floor: StartupFloorV1,
        attestations: Vec<SignedStartupFloorAttestationV1>,
        latest: Option<&Self>,
        policy: Option<&RuntimeCheckpointPolicyV1>,
        anchor: Option<&AdoptionRecordV1>,
        network: &NetworkId,
        committee: &[PublicKey],
        committed_height: u64,
        now_ms: u64,
        block_hash_at: impl Fn(u64) -> Option<[u8; 32]>,
    ) -> Result<Self, CheckpointError> {
        let policy = policy.ok_or(CheckpointError::NoPolicy)?;
        if latest.is_some_and(|latest| now_ms < latest.effective_ms) {
            return Err(CheckpointError::Pending);
        }
        if floor.network != *network {
            return Err(CheckpointError::Network);
        }
        committed_block(
            floor.height,
            floor.block_hash,
            committed_height,
            block_hash_at,
        )?;
        if latest.is_some_and(|latest| floor.height <= latest.floor.height)
            || anchor.is_some_and(|anchor| floor.height <= anchor.height)
        {
            return Err(CheckpointError::Regressed);
        }
        let effective_ms = policy
            .floor_effective_ms(now_ms)
            .ok_or(CheckpointError::Notice)?;
        if !policy.recoverable(floor.committed_ms, effective_ms) {
            return Err(CheckpointError::NotRecoverable);
        }
        covered(committee, &attestations, |signed| {
            (
                &signed.attestation.validator,
                signed.verify_for(&floor).is_ok(),
            )
        })?;
        Ok(Self {
            floor,
            effective_ms,
            attestations,
        })
    }
}

/// One boundary of a root's history: a row of `world.runtime_boundaries`, keyed by its height.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::checkpoint::RuntimeBoundaryV1")]
#[norito(tag = "kind", content = "content")]
pub enum RuntimeBoundaryV1 {
    /// The adoption anchor: the lowest boundary of a network that adopted the foundation at a
    /// checkpoint.
    AdoptionAnchor(RuntimeAdoptionAnchorV1),
    /// A startup floor.
    StartupFloor(RuntimeStartupFloorV1),
}

impl RuntimeBoundaryV1 {
    /// Height of the boundary block: the key of the row.
    #[must_use]
    pub const fn height(&self) -> u64 {
        match self {
            Self::AdoptionAnchor(anchor) => anchor.record.height,
            Self::StartupFloor(floor) => floor.floor.height,
        }
    }

    /// Hash of the boundary block.
    #[must_use]
    pub const fn block_hash(&self) -> [u8; 32] {
        match self {
            Self::AdoptionAnchor(anchor) => anchor.record.block_hash,
            Self::StartupFloor(floor) => floor.floor.block_hash,
        }
    }

    /// Whether the boundary is in force at committed time `now_ms`: an anchor from its
    /// ratification, a floor once its notice has passed.
    #[must_use]
    pub const fn in_force(&self, now_ms: u64) -> bool {
        match self {
            Self::AdoptionAnchor(_) => true,
            Self::StartupFloor(floor) => now_ms >= floor.effective_ms,
        }
    }
}

/// The history boundary `boundaries` set at committed time `now_ms`: the height and block hash
/// at or below which no release re-executes a block. It is the highest boundary in force.
#[must_use]
pub fn history_boundary<'a>(
    boundaries: impl IntoIterator<Item = &'a RuntimeBoundaryV1>,
    now_ms: u64,
) -> Option<(u64, [u8; 32])> {
    boundaries
        .into_iter()
        .filter(|boundary| boundary.in_force(now_ms))
        .map(|boundary| (boundary.height(), boundary.block_hash()))
        .max_by_key(|(height, _)| *height)
}

#[cfg(test)]
mod tests;
