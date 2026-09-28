//! Inbound light clients (`specs/sccp.md` §4.13). Owners: ws33 (Parliament half:
//! [`is_usable`], [`initialize`], [`install_checkpoint`], [`freeze`]) and ws41 (advance half:
//! [`execute_advance`], [`execute_report_equivocation`], [`verify_source_proof`],
//! [`preverify_keeper_advance`], [`prune`]).
//!
//! World state stores authenticated source validator sets with validity ranges and finalized
//! checkpoints. Every advance is permissionless and proof-carrying; the Parliament only
//! initializes, re-initializes, freezes and installs trusted checkpoints. Chain verification
//! itself lives in `iroha_sccp::light_client`.

use super::{
    Error,
    admission::{SccpAdmissionKeysV1, SccpAdmissionRejectV1},
    store,
};
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::{
    account::AccountId,
    bridge::SccpNetworkV1,
    isi::sccp::{AdvanceSccpLightClientV1, ReportSccpLightClientEquivocationV1},
    sccp::{
        events::{
            SccpEvent, SccpLightClientAdvancedV1, SccpLightClientFrozenV1,
            SccpLightClientInitializedV1, SccpTrustedCheckpointInstalledV1,
        },
        governance::{
            SccpFreezeLightClientActionV1, SccpInitializeLightClientActionV1,
            SccpInstallTrustedCheckpointActionV1,
        },
        inbound::SccpSourceProofBytesV1,
        light_client::{
            SccpLcCheckpointOriginV1, SccpLcCheckpointV1, SccpLcConsensusSetV1,
            SccpLcFreezeReasonV1, SccpLcParliamentFreezeV1, SccpLightClientV1,
        },
    },
};
use iroha_sccp::light_client::{
    self as lc,
    proof::SccpVerifiedProofV1,
    state::{SccpLcPurgeV1, SccpLcStateView, state_hash},
};

/// [`SccpLcStateView`] over committed or executing world state.
pub struct WorldLightClientView<'world, W: WorldReadOnly + ?Sized>(pub &'world W);

impl<W: WorldReadOnly + ?Sized> SccpLcStateView for WorldLightClientView<'_, W> {
    fn light_client(&self, network: SccpNetworkV1) -> Option<SccpLightClientV1> {
        store::light_clients::get(self.0, &network).copied()
    }
    fn consensus_set(&self, network: SccpNetworkV1, set_id: u64) -> Option<SccpLcConsensusSetV1> {
        store::light_client_sets::get(self.0, &(network, set_id)).cloned()
    }
    fn checkpoint(&self, network: SccpNetworkV1, source_height: u64) -> Option<SccpLcCheckpointV1> {
        store::light_client_checkpoints::get(self.0, &(network, source_height)).copied()
    }
}

fn refuse(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(format!("SCCP light client: {reason}").into())
}

/// The first `[zk.sccp]` limit that one verifier call's `work`, the transaction total `in_tx` or
/// the block total `in_block` (both including `work`) exceeds (§4.12.1 step 5).
pub(crate) fn exceeded_verifier_limit(
    limits: &iroha_config::parameters::actual::Sccp,
    work: &lc::SccpVerifierWorkV1,
    in_tx: &lc::SccpVerifierWorkV1,
    in_block: &lc::SccpVerifierWorkV1,
) -> Option<&'static str> {
    if work.proof_bytes > limits.max_proof_bytes_per_proof.get() {
        return Some("max_proof_bytes_per_proof");
    }
    let categories: [(u64, u64, u64, u64, &'static str, &'static str); 8] = [
        (
            in_tx.proofs.into(),
            in_block.proofs.into(),
            limits.max_proofs_per_transaction.get().into(),
            limits.max_proofs_per_block.get().into(),
            "max_proofs_per_transaction",
            "max_proofs_per_block",
        ),
        (
            in_tx.proof_bytes,
            in_block.proof_bytes,
            limits.max_proof_bytes_per_transaction.get(),
            limits.max_proof_bytes_per_block.get(),
            "max_proof_bytes_per_transaction",
            "max_proof_bytes_per_block",
        ),
        (
            in_tx.native_headers.into(),
            in_block.native_headers.into(),
            limits.max_native_headers_per_transaction.get().into(),
            limits.max_native_headers_per_block.get().into(),
            "max_native_headers_per_transaction",
            "max_native_headers_per_block",
        ),
        (
            in_tx.native_header_bytes,
            in_block.native_header_bytes,
            limits.max_native_header_bytes_per_transaction.get(),
            limits.max_native_header_bytes_per_block.get(),
            "max_native_header_bytes_per_transaction",
            "max_native_header_bytes_per_block",
        ),
        (
            in_tx.ethereum_light_client_updates.into(),
            in_block.ethereum_light_client_updates.into(),
            limits
                .max_ethereum_light_client_updates_per_transaction
                .get()
                .into(),
            limits
                .max_ethereum_light_client_updates_per_block
                .get()
                .into(),
            "max_ethereum_light_client_updates_per_transaction",
            "max_ethereum_light_client_updates_per_block",
        ),
        (
            in_tx.bls_vote_attestations.into(),
            in_block.bls_vote_attestations.into(),
            limits
                .max_bls_vote_attestations_per_transaction
                .get()
                .into(),
            limits.max_bls_vote_attestations_per_block.get().into(),
            "max_bls_vote_attestations_per_transaction",
            "max_bls_vote_attestations_per_block",
        ),
        (
            in_tx.secp256k1_recoveries.into(),
            in_block.secp256k1_recoveries.into(),
            limits.max_secp256k1_recoveries_per_transaction.get().into(),
            limits.max_secp256k1_recoveries_per_block.get().into(),
            "max_secp256k1_recoveries_per_transaction",
            "max_secp256k1_recoveries_per_block",
        ),
        (
            in_tx.ed25519_signature_checks.into(),
            in_block.ed25519_signature_checks.into(),
            limits
                .max_ed25519_signature_checks_per_transaction
                .get()
                .into(),
            limits.max_ed25519_signature_checks_per_block.get().into(),
            "max_ed25519_signature_checks_per_transaction",
            "max_ed25519_signature_checks_per_block",
        ),
    ];
    categories
        .into_iter()
        .find_map(|(tx, block, tx_limit, block_limit, tx_name, block_name)| {
            if tx > tx_limit {
                Some(tx_name)
            } else if block > block_limit {
                Some(block_name)
            } else {
                None
            }
        })
}

/// Reserve one verifier call's estimated work, or refuse an undecodable frame or a call over a
/// `[zk.sccp]` limit before any cryptography runs.
fn reserve_work(
    state_transaction: &mut StateTransaction<'_, '_>,
    work: Result<lc::SccpVerifierWorkV1, lc::SccpLcError>,
) -> Result<(), Error> {
    let work = work.map_err(|error| refuse(format_args!("frame: {error}")))?;
    state_transaction
        .reserve_sccp_verifier_work(&work)
        .map_err(|limit| refuse(format_args!("SCCP verifier work exceeds zk.sccp.{limit}")))
}

/// Return whether the light client of `network` is installed, not frozen and within its
/// weak-subjectivity bound at Taira time `now_ms`, so burns on its chain are provable
/// (§4.13.2). A network whose light client this release cannot verify is never usable.
#[must_use]
pub fn is_usable(
    world: &(impl WorldReadOnly + ?Sized),
    network: SccpNetworkV1,
    now_ms: u64,
) -> bool {
    store::light_clients::get(world, &network).is_some_and(|light_client| !light_client.is_frozen())
        && lc::is_aged(&WorldLightClientView(world), network, now_ms) == Ok(false)
}

/// Write `checkpoint` of `network` (replacing a same-height record) and keep the lowest
/// checkpoint of its stride bucket in the permanent stride index (§4.13.1).
///
/// # Errors
///
/// Fails when the stored index breaks its invariant.
pub fn record_checkpoint(
    state_transaction: &mut StateTransaction<'_, '_>,
    network: SccpNetworkV1,
    checkpoint: SccpLcCheckpointV1,
) -> Result<(), Error> {
    let height = checkpoint.data.source_height;
    store::light_client_checkpoints::insert(state_transaction, (network, height), checkpoint)?;
    let stride = store::light_clients::get(&*state_transaction.world, &network)
        .map_or(0, |light_client| light_client.params.checkpoint_stride);
    if let Some(bucket) = height.checked_div(stride) {
        let lowest =
            store::light_client_stride_index::get(&*state_transaction.world, &(network, bucket))
                .copied();
        if lowest.is_none_or(|lowest| height < lowest) {
            store::light_client_stride_index::insert(state_transaction, (network, bucket), height)?;
        }
    }
    Ok(())
}

/// Delete what `purge` names for `network` before a (re-)initialization (§4.13.2).
fn purge(
    state_transaction: &mut StateTransaction<'_, '_>,
    network: SccpNetworkV1,
    purge: SccpLcPurgeV1,
) -> Result<(), Error> {
    if purge == SccpLcPurgeV1::KeepStored {
        return Ok(());
    }
    let world = &*state_transaction.world;
    let sets: Vec<u64> = store::light_client_sets::range(world, (network, 0)..=(network, u64::MAX))
        .map(|((_, set_id), _)| *set_id)
        .collect();
    let checkpoints: Vec<(u64, bool)> =
        store::light_client_checkpoints::range(world, (network, 0)..=(network, u64::MAX))
            .map(|((_, height), checkpoint)| {
                (
                    *height,
                    checkpoint.origin == SccpLcCheckpointOriginV1::Parliament,
                )
            })
            .collect();
    let buckets: Vec<u64> =
        store::light_client_stride_index::range(world, (network, 0)..=(network, u64::MAX))
            .map(|((_, bucket), _)| *bucket)
            .collect();
    for set_id in sets {
        store::light_client_sets::remove(state_transaction, (network, set_id));
    }
    for (height, _) in checkpoints.iter().filter(|(_, parliament)| !parliament) {
        store::light_client_checkpoints::remove(state_transaction, (network, *height));
    }
    // The stride index is rebuilt below from the surviving Parliament checkpoints.
    for bucket in buckets {
        store::light_client_stride_index::remove(state_transaction, (network, bucket));
    }
    Ok(())
}

/// Rebuild the stride index of `network` from its stored checkpoints.
fn reindex_strides(
    state_transaction: &mut StateTransaction<'_, '_>,
    network: SccpNetworkV1,
) -> Result<(), Error> {
    let stored: Vec<SccpLcCheckpointV1> = store::light_client_checkpoints::range(
        &*state_transaction.world,
        (network, 0)..=(network, u64::MAX),
    )
    .map(|(_, checkpoint)| *checkpoint)
    .collect();
    for checkpoint in stored {
        record_checkpoint(state_transaction, network, checkpoint)?;
    }
    Ok(())
}

/// Apply an enacted `InitializeLightClient` (§4.14.3).
///
/// The expectation, the freshness of the bootstrap at the enactment block time and conflicts
/// with surviving stored data are checked by `iroha_sccp::light_client`; this applies its
/// result in order (purge, supersessions, light client, sets, checkpoints) and emits
/// `SccpLightClientInitialized`.
///
/// # Errors
///
/// Fails when the expectation does not hold or the bootstrap is invalid or stale.
pub fn initialize(
    state_transaction: &mut StateTransaction<'_, '_>,
    action: &SccpInitializeLightClientActionV1,
    proposal_id: [u8; 32],
) -> Result<(), Error> {
    let network = action.network;
    let now_ms = state_transaction.block_unix_timestamp_ms();
    let initial = lc::initialize_light_client(
        &WorldLightClientView(&*state_transaction.world),
        network,
        action.expected,
        &action.params,
        &action.bootstrap,
        now_ms,
    )
    .map_err(|error| {
        refuse(format_args!(
            "initialization of {}: {error}",
            network.profile_key()
        ))
    })?;
    purge(state_transaction, network, initial.purge)?;
    for supersession in &initial.superseded_sets {
        let key = (network, supersession.set_id);
        if let Some(mut set) =
            store::light_client_sets::get(&*state_transaction.world, &key).cloned()
        {
            set.superseded_at_source_ms = Some(supersession.superseded_at_source_ms);
            store::light_client_sets::insert(state_transaction, key, set)?;
        }
    }
    let light_client = initial.light_client;
    store::light_clients::insert(state_transaction, network, light_client)?;
    for set in initial.sets {
        store::light_client_sets::insert(state_transaction, (network, set.set_id), set)?;
    }
    if initial.purge == SccpLcPurgeV1::DiscardUnvetted {
        reindex_strides(state_transaction, network)?;
    }
    for checkpoint in initial.checkpoints {
        record_checkpoint(state_transaction, network, checkpoint)?;
    }
    state_transaction
        .world
        .emit_events(Some(SccpEvent::LightClientInitialized(
            SccpLightClientInitializedV1 {
                network,
                latest_set_id: light_client.head.latest_set_id,
                latest_finalized: light_client.head.latest_finalized,
                state_hash: light_client.state_hash,
                proposal_id,
            },
        )));
    Ok(())
}

/// Apply an enacted `InstallTrustedCheckpoint` (§4.14.3).
///
/// # Errors
///
/// Fails when the light client is not installed, the checkpoint is malformed, or it conflicts
/// with a stored checkpoint at the same height.
pub fn install_checkpoint(
    state_transaction: &mut StateTransaction<'_, '_>,
    action: &SccpInstallTrustedCheckpointActionV1,
    proposal_id: [u8; 32],
) -> Result<(), Error> {
    let network = action.network;
    let checkpoint = lc::verify_trusted_checkpoint(
        &WorldLightClientView(&*state_transaction.world),
        network,
        &action.checkpoint,
        state_transaction.block_unix_timestamp_ms(),
    )
    .map_err(|error| {
        refuse(format_args!(
            "trusted checkpoint of {}: {error}",
            network.profile_key()
        ))
    })?;
    record_checkpoint(state_transaction, network, checkpoint)?;
    state_transaction
        .world
        .emit_events(Some(SccpEvent::TrustedCheckpointInstalled(
            SccpTrustedCheckpointInstalledV1 {
                network,
                source_height: checkpoint.data.source_height,
                block_hash: checkpoint.data.block_hash,
                proposal_id,
            },
        )));
    Ok(())
}

/// Apply an enacted `FreezeLightClient` (§4.14.3).
///
/// A light client that is already frozen keeps its original reason, so equivocation evidence
/// is never overwritten.
///
/// # Errors
///
/// Fails when the light client is not installed.
pub fn freeze(
    state_transaction: &mut StateTransaction<'_, '_>,
    action: &SccpFreezeLightClientActionV1,
    proposal_id: [u8; 32],
) -> Result<(), Error> {
    let network = action.network;
    let mut light_client = store::light_clients::get(&*state_transaction.world, &network)
        .copied()
        .ok_or_else(|| refuse(format_args!("{} is not installed", network.profile_key())))?;
    if light_client.is_frozen() {
        return Ok(());
    }
    let reason = SccpLcFreezeReasonV1::Parliament(SccpLcParliamentFreezeV1 { proposal_id });
    light_client.frozen = Some(reason);
    light_client.state_hash = state_hash(&light_client.params, &light_client.head, Some(&reason));
    store::light_clients::insert(state_transaction, network, light_client)?;
    state_transaction
        .world
        .emit_events(Some(SccpEvent::LightClientFrozen(
            SccpLightClientFrozenV1 { network, reason },
        )));
    Ok(())
}

/// Apply a verified light-client `delta` of `network` (§4.13.2): new sets, supersessions,
/// checkpoints (idempotent, stride-indexed) and the head. Return whether the head moved.
///
/// # Errors
///
/// Fails when a stored map breaks its invariant.
fn apply_delta(
    state_transaction: &mut StateTransaction<'_, '_>,
    network: SccpNetworkV1,
    current: &SccpLightClientV1,
    delta: &iroha_sccp::light_client::state::SccpLcDeltaV1,
) -> Result<bool, Error> {
    for set in &delta.new_sets {
        store::light_client_sets::insert(state_transaction, (network, set.set_id), set.clone())?;
    }
    for supersession in &delta.superseded_sets {
        let key = (network, supersession.set_id);
        if let Some(mut set) =
            store::light_client_sets::get(&*state_transaction.world, &key).cloned()
        {
            set.superseded_at_source_ms = Some(supersession.superseded_at_source_ms);
            store::light_client_sets::insert(state_transaction, key, set)?;
        }
    }
    for checkpoint in &delta.checkpoints {
        record_checkpoint(state_transaction, network, *checkpoint)?;
    }
    let next = delta.next_light_client(current);
    let moved = delta.moves_head();
    if moved {
        store::light_clients::insert(state_transaction, network, next)?;
        state_transaction
            .world
            .emit_events(Some(SccpEvent::LightClientAdvanced(
                SccpLightClientAdvancedV1 {
                    network,
                    latest_set_id: next.head.latest_set_id,
                    latest_finalized: next.head.latest_finalized,
                    state_hash: next.state_hash,
                },
            )));
    }
    Ok(moved)
}

/// Execute `AdvanceSccpLightClientV1` (§4.13.2).
///
/// The optional `expected_state_hash` is a compare-and-swap guard. Re-proving stored data is a
/// successful no-op, so concurrent keepers and wallets do not fail.
///
/// # Errors
///
/// Fails when the guard does not match, or the advance does not verify (a conflict with stored
/// data points to `ReportSccpLightClientEquivocationV1`).
pub fn execute_advance(
    instruction: AdvanceSccpLightClientV1,
    _authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let network = instruction.network;
    let current = store::light_clients::get(&*state_transaction.world, &network)
        .copied()
        .ok_or_else(|| refuse(format_args!("{} is not installed", network.profile_key())))?;
    if instruction
        .expected_state_hash
        .is_some_and(|expected| expected != current.state_hash)
    {
        return Err(refuse("the light client moved since the advance was built"));
    }
    reserve_work(
        state_transaction,
        lc::advance_work(network, &instruction.advance),
    )?;
    let delta = lc::apply_advance(
        &WorldLightClientView(&*state_transaction.world),
        network,
        &instruction.advance,
        state_transaction.block_unix_timestamp_ms(),
    )
    .map_err(|error| {
        refuse(format_args!(
            "advance of {}: {error}",
            network.profile_key()
        ))
    })?;
    apply_delta(state_transaction, network, &current, &delta).map(|_| ())
}

/// Execute `ReportSccpLightClientEquivocationV1`: two quorum-valid conflicting records freeze
/// the light client (§4.13.2).
///
/// # Errors
///
/// Fails when the evidence does not prove an equivocation.
pub fn execute_report_equivocation(
    instruction: ReportSccpLightClientEquivocationV1,
    _authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let network = instruction.network;
    let mut light_client = store::light_clients::get(&*state_transaction.world, &network)
        .copied()
        .ok_or_else(|| refuse(format_args!("{} is not installed", network.profile_key())))?;
    if light_client.is_frozen() {
        return Err(refuse("the light client is already frozen"));
    }
    reserve_work(
        state_transaction,
        lc::evidence_work(network, &instruction.a, &instruction.b),
    )?;
    let reason = lc::verify_equivocation(
        &WorldLightClientView(&*state_transaction.world),
        network,
        &instruction.a,
        &instruction.b,
        state_transaction.block_unix_timestamp_ms(),
    )
    .map_err(|error| refuse(format_args!("equivocation evidence: {error}")))?;
    light_client.frozen = Some(reason);
    light_client.state_hash = state_hash(&light_client.params, &light_client.head, Some(&reason));
    store::light_clients::insert(state_transaction, network, light_client)?;
    state_transaction
        .world
        .emit_events(Some(SccpEvent::LightClientFrozen(
            SccpLightClientFrozenV1 { network, reason },
        )));
    Ok(())
}

/// Verify an inbound or void `proof` against `network`'s light client at the executing block
/// time, record the checkpoints it proved, and return the normalized source event (§4.12.1
/// step 4).
///
/// The proof's verifier work is reserved under the `[zk.sccp]` limits first.
///
/// # Errors
///
/// Fails when the light client is unusable or the proof does not verify.
pub fn verify_source_proof(
    state_transaction: &mut StateTransaction<'_, '_>,
    network: SccpNetworkV1,
    proof: &SccpSourceProofBytesV1,
) -> Result<SccpVerifiedProofV1, Error> {
    reserve_work(state_transaction, lc::proof_work(network, proof))?;
    let verified = lc::verify_proof(
        &WorldLightClientView(&*state_transaction.world),
        network,
        proof,
        state_transaction.block_unix_timestamp_ms(),
    )
    .map_err(|error| {
        refuse(format_args!(
            "source proof of {}: {error}",
            network.profile_key()
        ))
    })?;
    for checkpoint in &verified.checkpoints {
        record_checkpoint(state_transaction, network, *checkpoint)?;
    }
    Ok(verified)
}

/// Pre-verify a keeper advance from a bridge key's account and return its admission keys (one
/// pending exempt advance per authority and network, §4.13.4).
///
/// The advance is exempt only from an active or pending bridge key's account and only when it
/// moves the head against committed state.
///
/// # Errors
///
/// Rejects an authority that is not a live bridge key, an advance that does not verify, and an
/// advance that would not move the head.
pub fn preverify_keeper_advance(
    world: &(impl WorldReadOnly + ?Sized),
    now_ms: u64,
    instruction: &AdvanceSccpLightClientV1,
    authority: &AccountId,
) -> Result<SccpAdmissionKeysV1, SccpAdmissionRejectV1> {
    let address = super::bridge_keys::bridge_key_address_of(authority)
        .ok_or_else(|| SccpAdmissionRejectV1::new("authority is not a bridge key's account"))?;
    let live = store::bridge_key_owners::get(world, &address)
        .and_then(|peer| store::bridge_keys::get(world, peer))
        .is_some_and(|state| {
            state
                .active
                .iter()
                .chain(state.pending.iter())
                .any(|key| key.address == address && !key.faulted)
        });
    if !live {
        return Err(SccpAdmissionRejectV1::new(
            "authority is not an active or pending bridge key",
        ));
    }
    let network = instruction.network;
    let current = store::light_clients::get(world, &network)
        .ok_or_else(|| SccpAdmissionRejectV1::new("the light client is not installed"))?;
    if instruction
        .expected_state_hash
        .is_some_and(|expected| expected != current.state_hash)
    {
        return Err(SccpAdmissionRejectV1::new("stale expected state hash"));
    }
    let delta = lc::apply_advance(
        &WorldLightClientView(world),
        network,
        &instruction.advance,
        now_ms,
    )
    .map_err(|error| SccpAdmissionRejectV1::new(format!("advance: {error}")))?;
    if !delta.moves_head() {
        return Err(SccpAdmissionRejectV1::new(
            "the advance does not move the head",
        ));
    }
    Ok(
        SccpAdmissionKeysV1::new(super::admission::SccpExemptClassV1::KeeperAdvance { network })
            .with_exclusive(&(authority.clone(), network)),
    )
}

/// Prune expired light-client sets and checkpoints, deleting at most `budget` records, and
/// return the number deleted (§4.13.1).
///
/// Sets go `set_retention_ms` after supersession; checkpoints go `checkpoint_prune_after_ms`
/// after recording unless Parliament-installed or the lowest of their stride bucket.
pub fn prune(state_transaction: &mut StateTransaction<'_, '_>, budget: usize) -> usize {
    use iroha_sccp::light_client::state::{checkpoint_prune_due, set_prune_due};
    let now_ms = state_transaction.block_unix_timestamp_ms();
    let world = &*state_transaction.world;
    let mut sets = Vec::new();
    let mut checkpoints = Vec::new();
    for (network, light_client) in store::light_clients::iter(world) {
        let params = light_client.params;
        let network = *network;
        for ((_, set_id), set) in
            store::light_client_sets::range(world, (network, 0)..=(network, u64::MAX))
        {
            if sets.len() + checkpoints.len() >= budget {
                break;
            }
            if set_prune_due(set, &params, now_ms) {
                sets.push((network, *set_id));
            }
        }
        for ((_, height), checkpoint) in
            store::light_client_checkpoints::range(world, (network, 0)..=(network, u64::MAX))
        {
            if sets.len() + checkpoints.len() >= budget {
                break;
            }
            let lowest = height
                .checked_div(params.checkpoint_stride)
                .and_then(|bucket| {
                    store::light_client_stride_index::get(world, &(network, bucket)).copied()
                });
            if checkpoint_prune_due(network, checkpoint, &params, lowest, now_ms) {
                checkpoints.push((network, *height));
            }
        }
    }
    let deleted = sets.len() + checkpoints.len();
    for key in sets {
        store::light_client_sets::remove(state_transaction, key);
    }
    for key in checkpoints {
        store::light_client_checkpoints::remove(state_transaction, key);
    }
    deleted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{
        SampleInstructions, authority, blank_state, header,
    };

    #[test]
    fn advances_and_reports_need_an_installed_light_client() {
        let state = blank_state();
        let view = state.world_view();
        assert!(!is_usable(&view, SccpNetworkV1::EthereumMainnet, 0));
        let reject =
            preverify_keeper_advance(&view, 0, &SampleInstructions::advance(), &authority(1))
                .expect_err("not a bridge key");
        assert!(reject.reason.contains("bridge key"), "{reject}");
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        assert_eq!(prune(&mut stx, 1_024), 0);
        let proof = SccpSourceProofBytesV1::new(vec![1, 2, 3]).expect("bounded proof");
        verify_source_proof(&mut stx, SccpNetworkV1::EthereumMainnet, &proof)
            .expect_err("not installed");
        let error = execute_advance(SampleInstructions::advance(), &authority(1), &mut stx)
            .expect_err("not installed");
        assert!(error.to_string().contains("not installed"), "{error}");
        execute_report_equivocation(SampleInstructions::equivocation(), &authority(1), &mut stx)
            .expect_err("not installed");
    }

    #[test]
    fn verifier_work_is_reserved_per_transaction_and_block() {
        use lc::SccpVerifierWorkV1 as Work;
        let state = blank_state();
        let mut block = state.block(header(2));
        // A full TRON advance: 16 segments of 128 signed headers.
        let full_advance = Work {
            proof_bytes: 1_000,
            native_headers: 2_048,
            secp256k1_recoveries: 2_048,
            ..Work::default()
        };
        let one_recovery = Work {
            secp256k1_recoveries: 1,
            ..Work::default()
        };
        {
            let mut stx = block.transaction();
            stx.reserve_sccp_verifier_work(&full_advance)
                .expect("a full advance fits one transaction");
            assert_eq!(
                stx.reserve_sccp_verifier_work(&one_recovery),
                Err("max_secp256k1_recoveries_per_transaction")
            );
            stx.apply();
        }
        {
            // A dropped transaction releases its reservation.
            let mut stx = block.transaction();
            stx.reserve_sccp_verifier_work(&full_advance).expect("fits");
        }
        for _ in 0..3 {
            let mut stx = block.transaction();
            stx.reserve_sccp_verifier_work(&full_advance)
                .expect("four full advances fit one block");
            stx.apply();
        }
        let mut stx = block.transaction();
        assert_eq!(
            stx.reserve_sccp_verifier_work(&one_recovery),
            Err("max_secp256k1_recoveries_per_block")
        );
        let oversized = Work {
            proof_bytes: stx.zk.sccp.max_proof_bytes_per_proof.get() + 1,
            ..Work::default()
        };
        assert_eq!(
            stx.reserve_sccp_verifier_work(&oversized),
            Err("max_proof_bytes_per_proof")
        );
        let error = execute_advance(SampleInstructions::advance(), &authority(1), &mut stx)
            .expect_err("not installed");
        assert!(error.to_string().contains("not installed"), "{error}");
    }

    #[test]
    fn verifier_limits_name_the_first_exceeded_category() {
        use lc::SccpVerifierWorkV1 as Work;
        let limits = iroha_config::parameters::actual::Sccp::default();
        let within = Work {
            bls_vote_attestations: 16,
            ..Work::default()
        };
        assert_eq!(
            exceeded_verifier_limit(&limits, &within, &within, &within),
            None
        );
        let tx = Work {
            bls_vote_attestations: limits.max_bls_vote_attestations_per_transaction.get() + 1,
            ..Work::default()
        };
        assert_eq!(
            exceeded_verifier_limit(&limits, &within, &tx, &tx),
            Some("max_bls_vote_attestations_per_transaction")
        );
        let block = Work {
            ethereum_light_client_updates: limits.max_ethereum_light_client_updates_per_block.get()
                + 1,
            ..Work::default()
        };
        assert_eq!(
            exceeded_verifier_limit(&limits, &within, &within, &block),
            Some("max_ethereum_light_client_updates_per_block")
        );
    }

    #[test]
    fn a_stale_state_hash_guard_refuses_the_advance() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let mut advance = SampleInstructions::advance();
        store::light_clients::insert(&mut stx, advance.network, installed(100)).expect("lc");
        advance.expected_state_hash = Some([9; 32]);
        let error = execute_advance(advance, &authority(1), &mut stx).expect_err("stale");
        assert!(error.to_string().contains("moved"), "{error}");
    }

    #[test]
    fn expired_checkpoints_are_pruned_except_permanent_ones() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let network = SccpNetworkV1::EthereumMainnet;
        let mut light_client = installed(100);
        light_client.params.checkpoint_prune_after_ms = 1;
        store::light_clients::insert(&mut stx, network, light_client).expect("lc");
        for (height, origin) in [
            (120, SccpLcCheckpointOriginV1::Advance),
            (150, SccpLcCheckpointOriginV1::Advance),
            (160, SccpLcCheckpointOriginV1::Parliament),
        ] {
            record_checkpoint(&mut stx, network, checkpoint(height, origin)).expect("cp");
        }
        // Block 2 runs at 8 000 ms; checkpoints were recorded at 0.
        assert_eq!(
            prune(&mut stx, 1_024),
            1,
            "only the non-lowest advance checkpoint"
        );
        assert!(store::light_client_checkpoints::contains(
            &*stx.world,
            &(network, 120)
        ));
        assert!(!store::light_client_checkpoints::contains(
            &*stx.world,
            &(network, 150)
        ));
        assert!(store::light_client_checkpoints::contains(
            &*stx.world,
            &(network, 160)
        ));
    }

    fn installed(stride: u64) -> SccpLightClientV1 {
        let mut params =
            iroha_data_model::sccp::light_client::SccpLightClientParamsV1::defaults_for(
                SccpNetworkV1::EthereumMainnet,
            )
            .expect("ethereum defaults");
        params.checkpoint_stride = stride;
        let head = iroha_data_model::sccp::light_client::SccpLcHeadV1 {
            latest_set_id: 1,
            latest_finalized: iroha_data_model::sccp::light_client::SccpLcPointV1 {
                source_height: 100,
                block_hash: [1; 32],
                source_time_ms: 1_000,
            },
            last_progress_taira_ms: 0,
        };
        SccpLightClientV1 {
            params,
            head,
            frozen: None,
            state_hash: state_hash(&params, &head, None),
        }
    }

    fn checkpoint(height: u64, origin: SccpLcCheckpointOriginV1) -> SccpLcCheckpointV1 {
        SccpLcCheckpointV1 {
            data: iroha_data_model::sccp::light_client::SccpLcCheckpointDataV1 {
                source_height: height,
                block_hash: [2; 32],
                state_root: None,
                receipts_or_tx_root: [3; 32],
                source_time_ms: height,
            },
            recorded_at_taira_ms: 0,
            origin,
        }
    }

    #[test]
    fn checkpoints_keep_the_lowest_height_per_stride_bucket() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let network = SccpNetworkV1::EthereumMainnet;
        store::light_clients::insert(&mut stx, network, installed(100)).expect("installed");
        for height in [150, 120, 180, 250] {
            record_checkpoint(
                &mut stx,
                network,
                checkpoint(height, SccpLcCheckpointOriginV1::Advance),
            )
            .expect("checkpoint");
        }
        assert_eq!(
            store::light_client_stride_index::get(&*stx.world, &(network, 1)),
            Some(&120)
        );
        assert_eq!(
            store::light_client_stride_index::get(&*stx.world, &(network, 2)),
            Some(&250)
        );
        assert_eq!(store::light_client_checkpoints::len(&*stx.world), 4);
    }

    #[test]
    fn trusted_checkpoints_and_freezes_need_an_installed_light_client() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let network = SccpNetworkV1::EthereumMainnet;
        let action = SccpInstallTrustedCheckpointActionV1 {
            network,
            checkpoint: checkpoint(5, SccpLcCheckpointOriginV1::Parliament).data,
        };
        install_checkpoint(&mut stx, &action, [1; 32]).expect_err("not installed");
        freeze(
            &mut stx,
            &SccpFreezeLightClientActionV1 { network },
            [1; 32],
        )
        .expect_err("not installed");

        store::light_clients::insert(&mut stx, network, installed(100)).expect("installed");
        record_checkpoint(
            &mut stx,
            network,
            checkpoint(5, SccpLcCheckpointOriginV1::Advance),
        )
        .expect("checkpoint");
        install_checkpoint(&mut stx, &action, [1; 32]).expect("same block upgrades");
        assert_eq!(
            store::light_client_checkpoints::get(&*stx.world, &(network, 5)).map(|c| c.origin),
            Some(SccpLcCheckpointOriginV1::Parliament)
        );
        let mut conflicting = action.clone();
        conflicting.checkpoint.block_hash = [9; 32];
        install_checkpoint(&mut stx, &conflicting, [2; 32]).expect_err("conflict");

        freeze(
            &mut stx,
            &SccpFreezeLightClientActionV1 { network },
            [7; 32],
        )
        .expect("freeze");
        let frozen = *store::light_clients::get(&*stx.world, &network).expect("installed");
        assert!(frozen.is_frozen());
        assert_ne!(frozen.state_hash, installed(100).state_hash);
        freeze(
            &mut stx,
            &SccpFreezeLightClientActionV1 { network },
            [8; 32],
        )
        .expect("idempotent");
        assert_eq!(
            *store::light_clients::get(&*stx.world, &network).expect("installed"),
            frozen
        );
        assert!(!is_usable(&*stx.world, network, 0));
    }
}
