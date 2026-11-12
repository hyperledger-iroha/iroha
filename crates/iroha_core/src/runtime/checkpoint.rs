//! The committed checkpoint policy and the boundaries of the universal root's history
//! (`specs/live_runtime_upgrades.md` §7.3, §13; `iroha_data_model::runtime::checkpoint`).
//!
//! The adoption anchor and the startup floors are rows of `world.runtime_boundaries`, storage of
//! the `iroha.runtime.lifecycle.v1` addition, written only by the two instructions of that
//! addition. A root that has not activated it holds no boundary and binds neither instruction,
//! and activation starts the table empty, so every boundary a root reads was checked by the
//! rules below. The checkpoint policy is the custom parameter `runtime_checkpoint_policy_v1`,
//! which governance sets like the other runtime policies.
//!
//! - **Commitment.** [`ratify_anchor`] and [`raise_floor`] check what committed State decides:
//!   the committed block hash at the record's height, the transition rules and the statement of
//!   every validator of the current committee. Those statements are the authority, so any
//!   account relays a record.
//! - **Attestation.** What State cannot decide, each validator vouches for. A node derives the
//!   adoption record from its own store ([`adoption_record`]) and signs a record only after
//!   checking it against its own certified history ([`attest_anchor`], [`attest_floor`]): the
//!   block, its committed time, the certified World roots around the adoption height, the
//!   committee, and for a floor that it holds the checkpoint.
//! - **Boundary.** [`history_boundary`] is the height at or below which no release re-executes
//!   a block: the highest boundary in force.
//!
//! The genesis execution-policy digest stays re-validated at startup after an anchor is
//! ratified: it is the only guard on the validity inputs a node still takes from its
//! configuration. TODO(`status.md`, ZK delivery F.4): commit those inputs, then delete it.
//!
//! The snapshot maker produces a checkpoint at the heights the committed policy names and keeps
//! the ones it retains (`crate::snapshot::checkpoint_store`); Torii's runtime boundary routes have
//! a node sign its statements. TODO(§7.3): start a node from a transferred checkpoint without the
//! block bodies below it.

use iroha_crypto::{Hash, HashOf, KeyPair, PublicKey};
use iroha_data_model::{
    NetworkId,
    isi::error::InstructionExecutionError,
    parameter::{CustomParameter, Parameters, RuntimeCheckpointPolicyV1},
    runtime::{
        adoption::SignedAdoptionAttestationV1,
        checkpoint::{
            RuntimeAdoptionAnchorV1, RuntimeBoundaryV1, RuntimeStartupFloorV1,
            SignedStartupFloorAttestationV1, StartupFloorV1,
        },
        definition::RootSelectionV1,
        lifecycle::{AdoptionRecordV1, committee_digest},
    },
};
use mv::storage::StorageReadOnly as _;

use super::migration::lifecycle_installed;
use crate::state::{
    StateReadOnly, StateTransaction, WorldReadOnly, upgrade_checkpoint::UpgradeCheckpointV1,
};

/// The committed checkpoint policy of a root running `selection`, if it has activated the
/// lifecycle State and committed a well-formed one.
///
/// The frozen baseline stores whatever `SetParameter` writes under this identity, so a value a
/// root committed before it activated the lifecycle State is judged here, when it is read: a
/// malformed one is no policy.
#[must_use]
pub fn committed_policy(
    selection: &RootSelectionV1,
    parameters: &Parameters,
) -> Option<RuntimeCheckpointPolicyV1> {
    if !lifecycle_installed(selection) {
        return None;
    }
    parameters
        .custom()
        .get(RuntimeCheckpointPolicyV1::parameter_id())
        .and_then(|parameter| RuntimeCheckpointPolicyV1::from_custom_parameter(parameter).ok())
}

/// The adoption anchor the root of `world` ratified, if it ratified one.
#[must_use]
pub fn committed_anchor(world: &impl WorldReadOnly) -> Option<RuntimeAdoptionAnchorV1> {
    // The anchor is the lowest boundary: no floor lies at or below it, and it is ratified
    // before any floor.
    match world.runtime_boundaries().iter().next() {
        Some((_, RuntimeBoundaryV1::AdoptionAnchor(anchor))) => Some(anchor.clone()),
        _ => None,
    }
}

/// The latest startup floor the root of `world` committed, in force or waiting out its notice.
#[must_use]
pub fn latest_floor(world: &impl WorldReadOnly) -> Option<RuntimeStartupFloorV1> {
    world
        .runtime_boundaries()
        .iter()
        .filter_map(|(_, boundary)| match boundary {
            RuntimeBoundaryV1::StartupFloor(floor) => Some(floor),
            RuntimeBoundaryV1::AdoptionAnchor(_) => None,
        })
        .next_back()
        .cloned()
}

/// The history boundary the root of `world` set at committed time `now_ms` (§7.3, §13): the
/// height and block at or below which no release re-executes a block.
#[must_use]
pub fn history_boundary(world: &impl WorldReadOnly, now_ms: u64) -> Option<UpgradeCheckpointV1> {
    iroha_data_model::runtime::checkpoint::history_boundary(
        world
            .runtime_boundaries()
            .iter()
            .map(|(_, boundary)| boundary),
        now_ms,
    )
    .map(|(height, block_hash)| UpgradeCheckpointV1 {
        height,
        block_hash: HashOf::from_untyped_unchecked(Hash::prehashed(block_hash)),
    })
}

/// Check a custom parameter `SetParameter` is about to commit, if it is the checkpoint policy
/// of a root that has activated the lifecycle State (§7.3): the policy must be well formed.
/// Every other parameter, and every parameter of a root without the lifecycle State, passes
/// unchanged, so that root executes `SetParameter` exactly as the frozen baseline does.
///
/// # Errors
/// The rule the policy violates.
pub(crate) fn admit_parameter(
    state_transaction: &StateTransaction<'_, '_>,
    parameter: &CustomParameter,
) -> Result<(), String> {
    if parameter.id() != RuntimeCheckpointPolicyV1::parameter_id()
        || !lifecycle_installed(state_transaction.world.runtime_selection.get())
    {
        return Ok(());
    }
    RuntimeCheckpointPolicyV1::from_custom_parameter(parameter)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn refused(reason: impl core::fmt::Display) -> InstructionExecutionError {
    InstructionExecutionError::InvariantViolation(format!("runtime boundary: {reason}").into())
}

/// The committee whose every member signs a boundary, the committed tip and the committed block
/// hashes the executing block builds on.
fn committed_chain<'a>(
    state_transaction: &'a StateTransaction<'_, '_>,
) -> (Vec<PublicKey>, u64, impl Fn(u64) -> Option<[u8; 32]> + 'a) {
    let committee = state_transaction
        .commit_topology()
        .get()
        .iter()
        .map(|peer| peer.public_key().clone())
        .collect();
    // The blocks below the executing one are committed.
    let committed_height = state_transaction
        ._curr_block
        .height()
        .get()
        .saturating_sub(1);
    let block_hash_at = move |height: u64| {
        usize::try_from(height)
            .ok()
            .and_then(|height| height.checked_sub(1))
            .and_then(|index| state_transaction.block_hashes.get(index))
            .map(|hash| {
                let bytes: &[u8; 32] = hash.as_ref();
                *bytes
            })
    };
    (committee, committed_height, block_hash_at)
}

/// Ratify the adoption anchor of the root (§13).
///
/// The root holds no boundary yet. The record names this network and the block the chain
/// committed at its height, and carries the statement of every validator of the current
/// committee ([`RuntimeAdoptionAnchorV1::check_ratification`]).
///
/// # Errors
/// The first rule the ratification violates.
pub(crate) fn ratify_anchor(
    state_transaction: &mut StateTransaction<'_, '_>,
    anchor: RuntimeAdoptionAnchorV1,
) -> Result<(), InstructionExecutionError> {
    {
        let (committee, committed_height, block_hash_at) = committed_chain(state_transaction);
        let lowest = state_transaction
            .world
            .runtime_boundaries
            .iter()
            .next()
            .map(|(_, boundary)| boundary);
        anchor
            .check_ratification(
                lowest,
                &state_transaction.network_id,
                &committee,
                committed_height,
                block_hash_at,
            )
            .map_err(refused)?;
    }
    state_transaction.world.runtime_boundaries.insert(
        anchor.record.height,
        RuntimeBoundaryV1::AdoptionAnchor(anchor),
    );
    Ok(())
}

/// Raise the startup floor of the root to `floor` (§7.3, "Advancing the floor").
///
/// The committed checkpoint policy decides when the floor comes into force and how old its
/// checkpoint must then be ([`RuntimeStartupFloorV1::raise`]). The floors the new one makes
/// unreachable, those below the floor in force, leave State, so the table holds the anchor, the
/// floor in force and the floor waiting out its notice.
///
/// # Errors
/// The first rule the raise violates.
pub(crate) fn raise_floor(
    state_transaction: &mut StateTransaction<'_, '_>,
    floor: StartupFloorV1,
    attestations: Vec<SignedStartupFloorAttestationV1>,
) -> Result<(), InstructionExecutionError> {
    let latest = latest_floor(&state_transaction.world);
    let raised = {
        let (committee, committed_height, block_hash_at) = committed_chain(state_transaction);
        let policy = committed_policy(
            state_transaction.world.runtime_selection.get(),
            WorldReadOnly::parameters(&state_transaction.world),
        );
        let anchor = committed_anchor(&state_transaction.world);
        RuntimeStartupFloorV1::raise(
            floor,
            attestations,
            latest.as_ref(),
            policy.as_ref(),
            anchor.as_ref().map(|anchor| &anchor.record),
            &state_transaction.network_id,
            &committee,
            committed_height,
            super::change_set::block_ms(state_transaction),
            block_hash_at,
        )
        .map_err(refused)?
    };
    // The latest floor is in force, or `raise` refused: every floor below it is superseded.
    if let Some(in_force) = latest {
        let superseded: Vec<u64> = state_transaction
            .world
            .runtime_boundaries
            .iter()
            .filter(|(height, boundary)| {
                **height < in_force.floor.height
                    && matches!(boundary, RuntimeBoundaryV1::StartupFloor(_))
            })
            .map(|(height, _)| *height)
            .collect();
        for height in superseded {
            state_transaction.world.runtime_boundaries.remove(height);
        }
    }
    state_transaction
        .world
        .runtime_boundaries
        .insert(raised.floor.height, RuntimeBoundaryV1::StartupFloor(raised));
    Ok(())
}

/// A node's own certified history, which it checks a record against before signing it.
pub trait History {
    /// Hash of the block the node stores at `height`.
    fn block_hash(&self, height: u64) -> Option<[u8; 32]>;
    /// Committed time of that block, in milliseconds.
    fn committed_ms(&self, height: u64) -> Option<u64>;
    /// The certified World root after the execution of the block at `height`, under the root
    /// rule of the release that executed it.
    fn world_root(&self, height: u64) -> Option<[u8; 32]>;
    /// The certified World root the block at `height` executed on.
    fn parent_world_root(&self, height: u64) -> Option<[u8; 32]>;
    /// The committee that certified the block at `height`.
    fn committee(&self, height: u64) -> Option<Vec<PublicKey>>;
}

impl<V: StateReadOnly + ?Sized> History
    for crate::sumeragi::certified_chain::CertifiedChain<'_, V>
{
    fn block_hash(&self, height: u64) -> Option<[u8; 32]> {
        let certified = self.certified(height).ok()?;
        let hash = certified.committed().block_hash();
        let bytes: &[u8; 32] = hash.as_ref();
        Some(*bytes)
    }

    fn committed_ms(&self, height: u64) -> Option<u64> {
        let certified = self.certified(height).ok()?;
        u64::try_from(
            certified
                .committed()
                .block()
                .header()
                .creation_time()
                .as_millis(),
        )
        .ok()
    }

    fn world_root(&self, height: u64) -> Option<[u8; 32]> {
        let certified = self.certified(height).ok()?;
        Some(
            certified
                .committed()
                .commitment()
                .execution
                .world_state_root
                .0,
        )
    }

    fn parent_world_root(&self, height: u64) -> Option<[u8; 32]> {
        let certified = self.certified(height).ok()?;
        Some(
            certified
                .committed()
                .commitment()
                .execution
                .parent_world_state_root
                .0,
        )
    }

    fn committee(&self, height: u64) -> Option<Vec<PublicKey>> {
        let certified = self.certified(height).ok()?;
        Some(
            certified
                .committed()
                .commitment()
                .schedule
                .current
                .committee
                .iter()
                .map(|member| member.validator.public_key().clone())
                .collect(),
        )
    }
}

/// Why this node does not sign a checkpoint record.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RecordAttestationError {
    /// This node's key is not a member of the current committee.
    #[error("this node's key is not a member of the current committee")]
    NotInCommittee,
    /// The record names another network.
    #[error("the record belongs to another network")]
    Network,
    /// The record disagrees with this node's certified history.
    #[error("the record disagrees with this node's history: {0}")]
    History(&'static str),
    /// The anchor is not the checkpoint this node was configured to adopt at.
    #[error("the anchor is not the upgrade checkpoint this node adopted the foundation at")]
    NotAdopted,
    /// The record or the committee cannot be encoded, or the key cannot sign.
    #[error("{0}")]
    Signature(String),
}

/// The adoption record this node's own store states (§13): the upgrade checkpoint the node was
/// `configured` to adopt the foundation at, with the certified World roots around it and the
/// committee of its height from `history`, the baseline this release founds a root on, and no
/// authorizing governance record.
///
/// Every validator of a network derives the same record from its own store, so none signs a
/// field another party supplied, and the statements a ratification collects agree only if the
/// stores do.
///
/// # Errors
/// The node was not configured to adopt at a checkpoint, or its history does not certify the
/// block at that height, the roots around it or its committee.
pub fn adoption_record(
    history: &impl History,
    network: &NetworkId,
    configured: Option<&UpgradeCheckpointV1>,
) -> Result<AdoptionRecordV1, RecordAttestationError> {
    let configured = configured.ok_or(RecordAttestationError::NotAdopted)?;
    let differs = |reason| RecordAttestationError::History(reason);
    let block_hash: &[u8; 32] = configured.block_hash.as_ref();
    if history.block_hash(configured.height) != Some(*block_hash) {
        return Err(differs("another block at the adoption height"));
    }
    let predecessor_world_root = history
        .world_root(configured.height)
        .ok_or_else(|| differs("no certified World root at the adoption height"))?;
    let imported_state_root = configured
        .height
        .checked_add(1)
        .and_then(|successor| history.parent_world_root(successor))
        .ok_or_else(|| differs("no certified first foundation block"))?;
    let adopting = history
        .committee(configured.height)
        .ok_or_else(|| differs("no certified committee at the adoption height"))?;
    Ok(AdoptionRecordV1 {
        network: *network,
        height: configured.height,
        block_hash: *block_hash,
        predecessor_world_root,
        imported_state_root,
        committee: committee_digest(&adopting)
            .map_err(|error| RecordAttestationError::Signature(error.to_string()))?,
        selection: super::release_catalog()
            .map_err(|error| RecordAttestationError::Signature(error.to_string()))?
            .baseline_id,
        authorization: [0; 32],
    })
}

/// Sign this node's statement of the adoption `record` (§13), after checking every field of it
/// against the node's own store.
///
/// The record names this network and the upgrade checkpoint the node was `configured` to adopt
/// the foundation at, with no authorizing governance record, because node configuration
/// authorized that adoption. `history` certifies the block at that height, the World root the
/// predecessor certified after it, the root the first foundation block executed on, and the
/// committee of the adoption height. Its selection is the baseline this release founds a root
/// on. The node signs only while its key is a member of the current `committee`.
///
/// # Errors
/// The first check that fails; the node then signs nothing.
pub fn attest_anchor(
    history: &impl History,
    network: &NetworkId,
    configured: Option<&UpgradeCheckpointV1>,
    committee: &[PublicKey],
    record: &AdoptionRecordV1,
    key_pair: &KeyPair,
) -> Result<SignedAdoptionAttestationV1, RecordAttestationError> {
    let validator = key_pair.public_key().clone();
    if !committee.contains(&validator) {
        return Err(RecordAttestationError::NotInCommittee);
    }
    if record.network != *network {
        return Err(RecordAttestationError::Network);
    }
    let adopted = configured.is_some_and(|checkpoint| {
        let block_hash: &[u8; 32] = checkpoint.block_hash.as_ref();
        checkpoint.height == record.height && *block_hash == record.block_hash
    });
    if !adopted || record.authorization != [0; 32] {
        return Err(RecordAttestationError::NotAdopted);
    }
    let differs = |reason| RecordAttestationError::History(reason);
    if history.block_hash(record.height) != Some(record.block_hash) {
        return Err(differs("another block at the adoption height"));
    }
    if history.world_root(record.height) != Some(record.predecessor_world_root) {
        return Err(differs(
            "another certified World root at the adoption height",
        ));
    }
    let successor = record
        .height
        .checked_add(1)
        .ok_or_else(|| differs("no block follows the adoption height"))?;
    if history.parent_world_root(successor) != Some(record.imported_state_root) {
        return Err(differs(
            "another certified parent World root of the first foundation block",
        ));
    }
    let adopting = history
        .committee(record.height)
        .ok_or_else(|| differs("no certified committee at the adoption height"))?;
    if committee_digest(&adopting).ok() != Some(record.committee) {
        return Err(differs("another committee at the adoption height"));
    }
    let founded = super::release_catalog()
        .map_err(|error| RecordAttestationError::Signature(error.to_string()))?
        .baseline_id;
    if record.selection != founded {
        return Err(differs("another founded selection"));
    }
    RuntimeAdoptionAnchorV1::statement(record, validator)
        .map_err(|error| RecordAttestationError::Signature(error.to_string()))?
        .sign(key_pair.private_key())
        .map_err(|error| RecordAttestationError::Signature(error.to_string()))
}

/// Sign this node's statement of the startup `floor` (§7.3), after checking it against the
/// node's own store.
///
/// The floor names this network, the block the node stores at its height and that block's
/// committed time. `holds_checkpoint` says the node holds a restorable checkpoint at exactly
/// that height, which it restored from and caught up from before the caller asked: a validator
/// vouches for a floor only if it can still start from it. The node signs only while its key is
/// a member of the current `committee`.
///
/// # Errors
/// The first check that fails; the node then signs nothing.
pub fn attest_floor(
    history: &impl History,
    network: &NetworkId,
    committee: &[PublicKey],
    holds_checkpoint: bool,
    floor: &StartupFloorV1,
    key_pair: &KeyPair,
) -> Result<SignedStartupFloorAttestationV1, RecordAttestationError> {
    let validator = key_pair.public_key().clone();
    if !committee.contains(&validator) {
        return Err(RecordAttestationError::NotInCommittee);
    }
    if floor.network != *network {
        return Err(RecordAttestationError::Network);
    }
    let differs = |reason| RecordAttestationError::History(reason);
    if history.block_hash(floor.height) != Some(floor.block_hash) {
        return Err(differs("another block at the floor height"));
    }
    if history.committed_ms(floor.height) != Some(floor.committed_ms) {
        return Err(differs("another committed time of the floor block"));
    }
    if !holds_checkpoint {
        return Err(differs("this node holds no checkpoint at the floor height"));
    }
    RuntimeStartupFloorV1::statement(floor, validator)
        .map_err(|error| RecordAttestationError::Signature(error.to_string()))?
        .sign(key_pair.private_key())
        .map_err(|error| RecordAttestationError::Signature(error.to_string()))
}

#[cfg(test)]
mod tests;
