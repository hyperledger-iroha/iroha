//! Finite native independent-root unit transport. Source removal and its receipt are one
//! transaction; import reuses native foreign finality and never trusts endpoint responses.
//! TODO: qualify original-pool custody for the bounded cloned proof/state graph, priced
//! native relay work, committee handoffs, and full multi-root recovery before release.
use crate::{
    smartcontracts::{Execute, isi::Error},
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId, block::consensus::SumeragiRootScope, game_partition::*,
    isi::game_partition::*, permission::Permission, sumeragi_amx::AmxError,
};
use mv::storage::StorageReadOnly;

fn invalid(s: impl Into<String>) -> Error {
    Error::InvariantViolation(s.into().into())
}
fn governance(world: &impl WorldReadOnly, authority: &AccountId) -> bool {
    let matches =
        |p: &Permission| p.name() == "CanSetParameters" && p.payload().get().as_str() == "null";
    world
        .account_permissions_iter(authority)
        .is_ok_and(|ps| ps.into_iter().any(matches))
        || world.account_roles_iter(authority).any(|r| {
            world
                .roles()
                .get(r)
                .is_some_and(|r| r.permissions().any(matches))
        })
}
fn require_governance(
    state: &StateTransaction<'_, '_>,
    authority: &AccountId,
) -> Result<(), Error> {
    if state.genesis_execution_scope.is_some() && state._curr_block.is_genesis()
        || governance(&state.world, authority)
    {
        Ok(())
    } else {
        Err(invalid(
            "game partition registration and seeding require governance",
        ))
    }
}
fn local_instance(state: &mut StateTransaction<'_, '_>) -> Result<[u8; 32], Error> {
    let scope = crate::executor::root_scope::execution_root_scope(state)
        .map_err(|e| invalid(e.to_string()))?;
    if scope != SumeragiRootScope::Global {
        return Err(invalid(
            "game partition prototype requires an independent Global root",
        ));
    }
    scope
        .instance_id(
            &crate::sumeragi::crypto::BlsCrypto::new(),
            *state.network_id(),
            state.chain_id().as_str(),
        )
        .map(|h| h.0)
        .map_err(|e| invalid(e.to_string()))
}
fn read(state: &StateTransaction<'_, '_>, world: &Hash) -> Result<GamePartitionStateV1, Error> {
    state
        .world
        .game_partitions
        .get(world)
        .cloned()
        .ok_or_else(|| invalid("unknown game partition"))
}
fn write(
    state: &mut StateTransaction<'_, '_>,
    partition: GamePartitionStateV1,
) -> Result<(), Error> {
    partition.validate().map_err(invalid)?;
    state
        .world
        .game_partitions
        .insert(partition.world_id, partition);
    Ok(())
}
fn owned_index(
    partition: &GamePartitionStateV1,
    id: Hash,
    authority: &AccountId,
) -> Result<usize, Error> {
    let index = partition
        .units
        .binary_search_by_key(&id, |u| u.unit_id)
        .map_err(|_| invalid("game unit is absent or exported"))?;
    if &partition.units[index].owner != authority {
        return Err(invalid("game unit owner authorization required"));
    }
    Ok(index)
}
fn set_watermark(
    partition: &mut GamePartitionStateV1,
    id: Hash,
    sequence: u64,
) -> Result<(), Error> {
    match partition
        .known_units
        .binary_search_by_key(&id, |w| w.unit_id)
    {
        Ok(i) => partition.known_units[i].sequence = sequence,
        Err(i) => {
            if partition.known_units.len() >= GAME_PARTITION_MAX_RECEIPTS {
                return Err(invalid("game unit identity retention is full"));
            }
            partition.known_units.insert(
                i,
                GameUnitWatermarkV1 {
                    unit_id: id,
                    sequence,
                },
            );
        }
    }
    Ok(())
}
impl Execute for InstallGamePartitionV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_governance(state, authority)?;
        if self.kernel_hash != ivm::game_unit_step::kernel_hash() {
            return Err(invalid("unapproved game partition kernel"));
        }
        if state.world.game_partitions.get(&self.world_id).is_some()
            || state.world.game_partitions.iter().count() >= GAME_PARTITION_MAX_WORLDS
        {
            return Err(invalid("game partition already installed or capacity full"));
        }
        let local_instance = local_instance(state)?;
        write(
            state,
            GamePartitionStateV1 {
                world_id: self.world_id,
                local_instance,
                kernel_hash: self.kernel_hash,
                controller: authority.clone(),
                peers: vec![],
                units: vec![],
                exports: vec![],
                imported: vec![],
                known_units: vec![],
            },
        )
    }
}
impl Execute for RegisterGamePartitionPeerV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_governance(state, authority)?;
        let mut p = read(state, &self.world_id)?;
        if p.peers.len() >= GAME_PARTITION_MAX_PEERS
            || self.endpoints.is_empty()
            || self.endpoints.len() > 4
            || self
                .endpoints
                .iter()
                .any(|s| s.len() > 512 || !(s.starts_with("http://") || s.starts_with("https://")))
            || self.genesis.len() > 4 * 1024 * 1024
            || self.successor.len() > 4 * 1024 * 1024
        {
            return Err(invalid("game peer registration exceeds finite bound"));
        }
        let budget = state.pipeline_ivm_prepared_cache.execution_budget().clone();
        let tracker = crate::sumeragi::amx::authenticate_game_peer(
            &self.chain_id,
            self.network_id,
            &self.genesis,
            &self.successor,
            &budget,
        )
        .map_err(|e| state.attempt_error_to_instruction_error(e))?;
        if tracker.instance == p.local_instance
            || p.peers
                .iter()
                .any(|peer| peer.tracker.instance == tracker.instance)
        {
            return Err(invalid("game peer cannot replace an existing root"));
        }
        p.peers.push(GamePartitionPeerV1 {
            tracker,
            genesis_hash: Hash::new(&self.genesis),
            successor_hash: Hash::new(&self.successor),
            endpoints: self.endpoints,
        });
        write(state, p)
    }
}
impl Execute for SeedGameUnitV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_governance(state, authority)?;
        let mut p = read(state, &self.world_id)?;
        if p.units.len() >= GAME_PARTITION_MAX_UNITS
            || self.work_remaining > GAME_PARTITION_MAX_WORK
            || self.work_remaining == 0
            || self.hp == 0
            || state.world.accounts.get(&self.owner).is_none()
        {
            return Err(invalid("invalid game unit seed or capacity"));
        }
        let unit_id = game_unit_id(self.world_id, p.local_instance, self.nonce);
        if p.known_units.iter().any(|w| w.unit_id == unit_id) {
            return Err(invalid("game unit identity was already seeded or imported"));
        }
        let unit = GameUnitV1 {
            unit_id,
            origin: p.local_instance,
            owner: self.owner,
            hp: self.hp,
            x: self.x,
            sequence: 0,
            work_remaining: self.work_remaining,
        };
        set_watermark(&mut p, unit_id, 0)?;
        let at = p
            .units
            .binary_search_by_key(&unit_id, |u| u.unit_id)
            .unwrap_err();
        p.units.insert(at, unit);
        write(state, p)
    }
}
impl Execute for AdvanceGameUnitV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut p = read(state, &self.world_id)?;
        let i = owned_index(&p, self.unit_id, authority)?;
        if p.kernel_hash != ivm::game_unit_step::kernel_hash() || p.units[i].work_remaining == 0 {
            return Err(invalid(
                "game kernel mismatch or advance allowance exhausted",
            ));
        }
        let unit = &mut p.units[i];
        let next = ivm::game_unit_step::step_unit(ivm::game_unit_step::UnitStepInput {
            hp: unit.hp,
            x: unit.x,
        })
        .map_err(|e| invalid(format!("game IVM step failed: {e}")))?;
        unit.hp = next.hp;
        unit.x = next.x;
        unit.work_remaining -= 1;
        write(state, p)
    }
}
fn export_candidate(
    mut p: GamePartitionStateV1,
    id: Hash,
    destination: [u8; 32],
    authority: &AccountId,
    height: u64,
) -> Result<(GamePartitionStateV1, GameUnitExportV1), Error> {
    if !p
        .peers
        .iter()
        .any(|peer| peer.tracker.instance == destination)
        || p.exports.len() >= GAME_PARTITION_MAX_RECEIPTS
        || height == 0
    {
        return Err(invalid("game export destination or retention bound"));
    }
    let index = owned_index(&p, id, authority)?;
    let mut unit = p.units.remove(index);
    unit.sequence = unit
        .sequence
        .checked_add(1)
        .ok_or_else(|| invalid("game transfer sequence overflow"))?;
    set_watermark(&mut p, id, unit.sequence)?;
    let record = GameUnitExportV1 {
        world_id: p.world_id,
        source: p.local_instance,
        destination,
        kernel_hash: p.kernel_hash,
        unit,
        source_height: height,
    };
    p.exports.push(record.clone());
    p.validate().map_err(invalid)?;
    Ok((p, record))
}
impl Execute for ExportGameUnitV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let (p, record) = export_candidate(
            read(state, &self.world_id)?,
            self.unit_id,
            self.destination,
            authority,
            state._curr_block.height().get(),
        )?;
        // Both the authoritative tombstone and this witness belong to the transaction overlay.
        crate::exec_witness::record_write_game_unit_export(&record)
            .map_err(|e| invalid(e.to_string()))?;
        // export_candidate already validated this exact immutable graph. Do not put a
        // fallible/resource-sensitive revalidation after the receipt write: a nested
        // contract may catch an inner instruction rejection.
        state.world.game_partitions.insert(p.world_id, p);
        Ok(())
    }
}
fn proof_encoding_error(error: norito::core::DecodeAttemptError) -> AmxError {
    if error.kind() == norito::core::DecodeAttemptErrorKind::Invalid {
        AmxError::Encoding(error.to_string())
    } else {
        AmxError::Resource(error)
    }
}
#[expect(
    clippy::suspicious_operation_groupings,
    reason = "an import's `destination` is the root admitting it, which must be this partition's `local_instance`"
)]
fn check_import(p: &GamePartitionStateV1, proof: &GameUnitImportProofV1) -> Result<bool, AmxError> {
    let record = &proof.record;
    if record.world_id != p.world_id
        || record.destination != p.local_instance
        || record.source == p.local_instance
        || record.kernel_hash != p.kernel_hash
        || record.unit.sequence == 0
        || record.unit.work_remaining > GAME_PARTITION_MAX_WORK
    {
        return Err(AmxError::State(
            "game import world/root/kernel/budget mismatch",
        ));
    }
    let peer = p
        .peers
        .iter()
        .find(|peer| peer.tracker.instance == record.source)
        .ok_or(AmxError::State("unregistered game source"))?;
    // Bound all variable fields before crypto or encoding. Native QC validator enforces exact quorum.
    if proof.block.consensus_header.len() > 8192
        || proof.block.commit_qc.len() > 128 * 1024
        || proof.block.result_preimage.len() > GAME_PARTITION_MAX_PROOF_BYTES - 140 * 1024
        || proof.path.siblings.len() > 256
    {
        return Err(AmxError::State("game import proof exceeds finite bound"));
    }
    if norito::core::classify_decode_attempt(|| norito::canonical_frame_len(proof))
        .map_err(proof_encoding_error)?
        > GAME_PARTITION_MAX_PROOF_BYTES
    {
        return Err(AmxError::State("encoded game import graph exceeds bound"));
    }
    let verified = peer.tracker.verify_block(&proof.block)?;
    if verified.height != record.source_height {
        return Err(AmxError::State(
            "game export height differs from certified source",
        ));
    }
    let value = norito::core::classify_decode_attempt(|| norito::encode_canonical(record))
        .map_err(proof_encoding_error)?;
    if proof.path.root(&record.witness_key(), &value)?
        != verified.commitment().execution.ordinary_writes_root
    {
        return Err(AmxError::State(
            "game export is absent from certified ordinary writes",
        ));
    }
    if p.imported.contains(&record.receipt_id()) {
        return Ok(false);
    }
    if p.imported.len() >= GAME_PARTITION_MAX_RECEIPTS
        || p.units.len() >= GAME_PARTITION_MAX_UNITS
        || p.units.iter().any(|u| u.unit_id == record.unit.unit_id)
        || p.known_units
            .iter()
            .any(|w| w.unit_id == record.unit.unit_id && w.sequence >= record.unit.sequence)
        || p.known_units.len() >= GAME_PARTITION_MAX_RECEIPTS
            && !p
                .known_units
                .iter()
                .any(|w| w.unit_id == record.unit.unit_id)
    {
        return Err(AmxError::State(
            "game import replay, active duplicate or capacity",
        ));
    }
    Ok(true)
}
fn insert_import(
    mut p: GamePartitionStateV1,
    record: &GameUnitExportV1,
) -> Result<GamePartitionStateV1, Error> {
    let at = p
        .units
        .binary_search_by_key(&record.unit.unit_id, |u| u.unit_id)
        .map_or_else(|i| Ok(i), |_| Err(invalid("duplicate active game unit")))?;
    set_watermark(&mut p, record.unit.unit_id, record.unit.sequence)?;
    p.units.insert(at, record.unit.clone());
    p.imported.push(record.receipt_id());
    p.validate().map_err(invalid)?;
    Ok(p)
}
impl Execute for ImportGameUnitV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let p = read(state, &self.proof.record.world_id)?;
        let is_new = check_import(&p, &self.proof).map_err(|e| {
            if let Some(resource) = crate::sumeragi::amx::classified_decode_refusal(&e) {
                state.arm_local_storage_refusal(
                    crate::state::StateStorageAdmissionError::AmxDecode(resource),
                );
            }
            invalid(e.to_string())
        })?;
        if !is_new {
            return Ok(());
        }
        write(state, insert_import(p, &self.proof.record)?)
    }
}
/// Read one bounded partition snapshot from committed authoritative State.
///
/// # Errors
/// A corrupted partition fails its custody invariants.
pub fn game_partition_snapshot(
    view: &impl StateReadOnly,
    world_id: Hash,
) -> Result<Option<GamePartitionStateV1>, String> {
    let p = view.world().game_partitions().get(&world_id).cloned();
    if let Some(p) = &p {
        p.validate()?;
    }
    Ok(p)
}
/// Preverify a public import before a validator spends ordinary fees on its carrier.
/// Execution repeats every check; `false` means the exact receipt is already retained.
///
/// # Errors
/// Any invalid source, destination, finality proof, replay or finite capacity.
pub fn validate_game_partition_import(
    view: &impl StateReadOnly,
    proof: &GameUnitImportProofV1,
) -> Result<bool, String> {
    let p = view
        .world()
        .game_partitions()
        .get(&proof.record.world_id)
        .ok_or("unknown game partition")?;
    check_import(p, proof).map_err(|e| e.to_string())
}
/// Read a certified export from its original archived execution after any restart.
/// The identifier is a receipt ID, not a unit ID: a returning unit has distinct receipts.
///
/// # Errors
/// Missing or invalid original certified execution, archive corruption or inconsistent custody.
pub fn game_partition_export_proof(
    view: &impl StateReadOnly,
    world_id: Hash,
    receipt_id: Hash,
) -> Result<Option<GameUnitImportProofV1>, String> {
    let Some(p) = view.world().game_partitions().get(&world_id) else {
        return Ok(None);
    };
    let Some(record) = p.exports.iter().find(|r| r.receipt_id() == receipt_id) else {
        return Ok(None);
    };
    crate::query::native_receipts::game_partition_export_proof(view, record)
}

#[cfg(test)]
mod tests;
