//! Finite transaction execution on an authenticated disposable replay State.
//!
//! No SignedBlock, vote, QC, finality authorization or publication owner is constructed.
//! The actual signed External input uses the production Network attempt, fee settlement,
//! rollback, source quota and output-fit owner. All carrier callback capacity is reserved,
//! but subsequent Pipeline/Time phases and block sealing are deliberately not qualified.

use super::*;
use crate::{
    exec_witness,
    smartcontracts::ivm::cache::IvmCache,
    state::{
        CellField, State, StateReadOnly, StorageField, WorldBlock, WorldReadOnly,
        network_policy_routes::CapturedNetworkPolicyRoute,
        world_projection::{WorldProjection, WorldStateAccumulator, hash_value},
    },
    tx::{AcceptedTransaction, CheckedTransaction},
};
use iroha_data_model::{
    block::{BlockHeader, execution_output::ExecutionInputs},
    transaction::SignedTransaction,
};
use mv::{Key, Value};
use norito::codec::Encode;
use norito::derive::JsonSerialize;
use std::{num::NonZeroU64, time::Duration};

/// Evidence from one discarded signed repair transaction, not a block/finality certificate.
#[derive(Debug, JsonSerialize)]
pub struct SorafsRepairRehearsalReport {
    /// Fixed diagnostic scope; no consensus execution authorization is implied.
    pub scope: String,
    /// Exact authenticated native parent height.
    pub parent_height: u64,
    /// Exact native parent header hash.
    pub parent_core_hash: String,
    /// Exact native parent execution result.
    pub parent_result_hash: String,
    /// Signature-bound transaction identity.
    pub transaction_hash: String,
    /// Domain-separated all-four repair plan identity.
    pub repair_plan_id: String,
    /// Proposed execution time used by normal transaction and certificate admission.
    pub execution_time_ms: u64,
    /// The actual retained Network result, including its fee receipt.
    pub output: ExecutionOutputV1,
    /// Cold World commitment before the attempt.
    pub committed_world_before: String,
    /// Cold World commitment of the discarded overlay after the Network attempt.
    pub attempted_world: String,
    /// Exhaustive World field visitation, including every field with no changed row.
    pub visited_world_fields: Vec<String>,
    /// Exact original-parent to normal block-start changes, with no raw ledger values.
    pub block_start_changes: Vec<SorafsRepairWorldChange>,
    /// Exact normal block-start to completed Network attempt changes.
    pub network_changes: Vec<SorafsRepairWorldChange>,
    /// Cold World commitment after the entire overlay was dropped.
    pub committed_world_after: String,
    /// True only when the real Network output records successful execution.
    pub applied_in_discarded_overlay: bool,
    /// Always false: this diagnostic cannot publish its StateBlock.
    pub state_published: bool,
}

/// One canonical owner projection change; `None` keys denote cells, not absent table rows.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize)]
pub struct SorafsRepairWorldChange {
    /// Exhaustive World visitor's field identity.
    pub field: String,
    /// Native bare-Norito key hash for a table, absent for a cell.
    pub key_hash: Option<Hash>,
    /// Native owner-projected value hash before the phase; absent means no stored row.
    pub before_hash: Option<Hash>,
    /// Native owner-projected value hash after the phase; absent means no stored row.
    pub after_hash: Option<Hash>,
}

#[derive(Default)]
struct WorldChanges {
    fields: Vec<String>,
    changes: Vec<SorafsRepairWorldChange>,
}

impl WorldChanges {
    fn capture(world: &WorldBlock<'_>) -> Result<Self, String> {
        let mut inventory = Self::default();
        world.project_world(&mut inventory)?;
        inventory.fields.sort();
        if inventory.fields.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("repair inventory visited a World field twice".into());
        }
        inventory.changes.sort_by(|left, right| {
            (&left.field, left.key_hash).cmp(&(&right.field, right.key_hash))
        });
        Ok(inventory)
    }

    fn field(&mut self, name: &str) -> Result<(), String> {
        self.fields
            .try_reserve(1)
            .map_err(|error| error.to_string())?;
        self.fields.push(name.to_owned());
        Ok(())
    }

    fn change(
        &mut self,
        name: &str,
        key: Option<Hash>,
        before: Option<Hash>,
        after: Option<Hash>,
    ) -> Result<(), String> {
        if before != after {
            self.changes
                .try_reserve(1)
                .map_err(|error| error.to_string())?;
            self.changes.push(SorafsRepairWorldChange {
                field: name.to_owned(),
                key_hash: key,
                before_hash: before,
                after_hash: after,
            });
        }
        Ok(())
    }
}

impl WorldProjection for WorldChanges {
    type Error = String;

    fn append_storage_with<K: Key + Encode, V: Value, M: mv::storage::StorageMode<K, V>>(
        &mut self,
        name: &'static str,
        storage: &StorageField<'_, K, V, M>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), String> {
        self.field(name)?;
        for entry in storage.touched_entries() {
            self.change(
                name,
                Some(hash_value(entry.key)?),
                entry.before.map(&encode).transpose()?,
                entry.after.map(&encode).transpose()?,
            )?;
        }
        Ok(())
    }

    fn append_cell_with<V: mv::CellValue, C: Send + Sync + 'static>(
        &mut self,
        name: &'static str,
        cell: &CellField<'_, V, C>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), String> {
        self.field(name)?;
        if let Some(value) = cell.touched_value() {
            self.change(
                name,
                None,
                Some(encode(value.before)?),
                Some(encode(value.after)?),
            )?;
        }
        Ok(())
    }
}

/// Recover the exact second-phase delta, including changes reverted to the original preimage.
fn network_changes(
    start: &WorldChanges,
    final_state: &WorldChanges,
) -> Result<Vec<SorafsRepairWorldChange>, String> {
    use std::collections::BTreeMap;
    if start.fields != final_state.fields {
        return Err("repair phases visited different World fields".into());
    }
    let start_rows: BTreeMap<_, _> = start
        .changes
        .iter()
        .map(|row| ((&row.field, row.key_hash), row))
        .collect();
    let final_rows: BTreeMap<_, _> = final_state
        .changes
        .iter()
        .map(|row| ((&row.field, row.key_hash), row))
        .collect();
    let mut keys: Vec<_> = start_rows
        .keys()
        .chain(final_rows.keys())
        .copied()
        .collect();
    keys.sort();
    keys.dedup();
    let mut changes = Vec::new();
    changes
        .try_reserve_exact(keys.len())
        .map_err(|error| error.to_string())?;
    for key in keys {
        let start = start_rows.get(&key).copied();
        let end = final_rows.get(&key).copied();
        if let (Some(start), Some(end)) = (start, end)
            && start.before_hash != end.before_hash
        {
            return Err("repair phases lost the original World preimage".into());
        }
        let before =
            start.map_or_else(|| end.and_then(|row| row.before_hash), |row| row.after_hash);
        let after = end.map_or_else(
            || start.and_then(|row| row.before_hash),
            |row| row.after_hash,
        );
        if before != after {
            changes.push(SorafsRepairWorldChange {
                field: key.0.clone(),
                key_hash: key.1,
                before_hash: before,
                after_hash: after,
            });
        }
    }
    Ok(changes)
}

struct SingleInput<'a>(&'a TransactionEntrypoint);

impl ExecutionInputs for SingleInput<'_> {
    fn input_count(&self) -> usize {
        1
    }

    fn input_at(&self, index: usize) -> Option<&TransactionEntrypoint> {
        (index == 0).then_some(self.0)
    }
}

fn authenticate<'a>(
    block: &StateBlock<'_>,
    input: &'a TransactionEntrypoint,
) -> Result<AcceptedTransaction<'a>, String> {
    let parameters = block.world.parameters.get();
    let accepted = AcceptedTransaction::accept_borrowed_entrypoint_at_time(
        input,
        &block.network_id,
        parameters.sumeragi().max_clock_drift(),
        parameters.transaction(),
        block
            .crypto_admission_policy()
            .map_err(|error| error.to_string())?,
        block._curr_block.creation_time(),
        false,
    )
    .map_err(crate::tx::execution_attempt_from_admission_failure)
    .map_err(|error| error.to_string())?;
    CheckedTransaction::new(accepted, block)
        .map(CheckedTransaction::into_accepted)
        .map_err(|_| "repair transaction is already committed".to_owned())
}

impl State {
    /// Rehearse a sole signed repair through the production External/Network output owner.
    ///
    /// The caller must first restore and replay authenticated history on this temporary Kura.
    /// This method additionally requires the exact durable native tip and verifies the cold
    /// committed World before and after dropping the complete overlay. It never writes history.
    ///
    /// # Errors
    /// Refuses non-temporary stores, missing/mismatched native history, non-repair inputs,
    /// stateless or replay admission failure, resource refusal, or violated output ownership.
    /// A completed transaction rejection remains in the returned actual Network output.
    pub fn rehearse_sorafs_repair(
        &self,
        signed: SignedTransaction,
        execution_time_ms: u64,
    ) -> Result<SorafsRepairRehearsalReport, String> {
        if !self.kura.is_temporary_staging() {
            return Err("repair rehearsal requires the owned temporary replay store".into());
        }
        let repair = crate::runtime::sorafs_repair::sole_repair(signed.instructions())
            .ok_or("repair rehearsal requires exactly one typed repair instruction")?;
        let plan_id = hex::encode(
            repair
                .certificate
                .plan
                .id()
                .map_err(|error| error.to_string())?,
        );
        let transaction_hash = signed.hash().to_string();
        let before = self
            .verify_world_state_accumulator()
            .map_err(|error| error.to_string())?;
        let view = self.view();
        let tip = view
            .native_execution_tip()
            .ok_or("authenticated native parent is absent")?;
        let durable = self
            .kura
            .exact_durable_blocks_count()
            .map_err(|error| error.to_string())?;
        if u64::try_from(durable).ok() != Some(tip.height())
            || u64::try_from(view.height()).ok() != Some(tip.height())
            || view.latest_block_hash() != Some(tip.iroha_hash())
        {
            return Err("rehearsal State does not reproduce the exact durable native tip".into());
        }
        let height = NonZeroU64::new(tip.height().checked_add(1).ok_or("height overflow")?)
            .ok_or("invalid successor height")?;
        if execution_time_ms < tip.creation_time_ms()
            || signed.creation_time() >= Duration::from_millis(execution_time_ms)
        {
            return Err(
                "repair execution time must follow the signed source and native parent".into(),
            );
        }
        drop(view);
        let input = TransactionEntrypoint::External(signed);
        let root = MerkleTree::root_from_typed_leaves(std::iter::once(input.hash()));
        let header = BlockHeader::new(height, Some(tip.iroha_hash()), root, execution_time_ms, 0);
        exec_witness::ensure_exec_witness_capture_available()?;
        let attempted = (|| -> Result<_, String> {
            let (mut block, (recording, routing, policy)) = self
                .block_with_owned_start_stages(
                    header,
                    |block| {
                        // Lock order and original recorder binding match the production carrier.
                        let recording = exec_witness::begin_exec_witness_capture()?;
                        block.bind_original_execution_recorder()?;
                        crate::smartcontracts::ivm::active_host_abi_hashes(&block.world)
                            .map_err(|error| error.to_string())?;
                        let accepted = authenticate(block, &input)?;
                        let (routing, policy) =
                            CapturedNetworkPolicyRoute::for_repair_rehearsal(block, &accepted)
                                .map_err(|error| error.to_string())?;
                        Ok::<_, String>((recording, routing, policy))
                    },
                    |_, captured| Ok(captured),
                )
                .map_err(|error| format!("repair overlay acquisition failed: {error:?}"))?;
            let block_start = WorldChanges::capture(&block.world)?;
            let accepted = authenticate(&block, &input)?;
            let TransactionEntrypoint::External(source) = &input else {
                return Err("repair source lost its external owner".into());
            };
            // Match ordinary preflight fraud and one-source quarantine selection.
            #[cfg(feature = "telemetry")]
            let telemetry = Some(block.telemetry);
            #[cfg(not(feature = "telemetry"))]
            let telemetry = None;
            let admitted = policy
                .physical()
                .map_or_else(
                    || Ok(()),
                    |physical| {
                        let physical = physical.decision();
                        crate::tx::enforce_fraud_policy(
                            &block.fraud_monitoring,
                            source.metadata(),
                            telemetry,
                            &crate::tx::LaneAssignment {
                                lane_id: physical.lane_id,
                                dataspace_id: physical.dataspace_id,
                                dataspace_catalog: &block.nexus.dataspace_catalog,
                            },
                        )
                    },
                )
                .map(|()| accepted);
            let quarantine_overflow = admitted.is_ok()
                && crate::tx::is_quarantine_transaction(source)
                && block.pipeline.quarantine_max_txs_per_block == 0;
            // Reserve the exact frozen envelope, including all possible later callback rows.
            block.reserve_execution_outputs(1, root.map(Hash::from))?;
            let plan = block
                .execution_output_plan
                .replace(ExecutionOutputPlanState::Running);
            let Some(ExecutionOutputPlanState::Reserved(mut plan)) = plan else {
                return Err("repair output reservation lost its owner".into());
            };
            let reservation = plan
                .budget
                .begin(ExecutionOutputV1::network_output_limit_rejection(0))?;
            let mut cache = IvmCache::with_prepared_contract_cache(
                block.pipeline.cache_size,
                block.pipeline_ivm_prepared_cache.clone(),
            );
            let output = network::execute_network_attempt_with_amx(
                &mut block,
                &SingleInput(&input),
                &input,
                0,
                0,
                height.get(),
                routing,
                &policy,
                admitted,
                None,
                quarantine_overflow,
                reservation,
                &mut cache,
                None,
            )
            .map_err(|error| error.to_string())?;
            let attempted_world = WorldStateAccumulator::capture(&block.world)?
                .root(block.world.runtime_selection())?
                .to_string();
            let final_world = WorldChanges::capture(&block.world)?;
            let network_delta = network_changes(&block_start, &final_world)?;
            // The unfinished carrier guard can never be promoted to a publication owner.
            block.execution_output_plan = Some(ExecutionOutputPlanState::Poisoned);
            drop(recording);
            drop(block);
            Ok((output, attempted_world, block_start, network_delta))
        })();
        let after = self
            .verify_world_state_accumulator()
            .map_err(|error| error.to_string())?;
        if after != before
            || self.view().native_execution_tip() != Some(tip)
            || self
                .kura
                .exact_durable_blocks_count()
                .map_err(|error| error.to_string())?
                != durable
        {
            return Err("discarded repair changed committed State or durable history".into());
        }
        // Retain the charged canonical output bytes through report construction,
        // after the discarded overlay and unchanged committed State are checked.
        let ((output, _retained_output_bytes), attempted_world, block_start, network_delta) =
            attempted?;
        Ok(SorafsRepairRehearsalReport {
            scope: "signed_repair_transaction_discarded_overlay".into(),
            parent_height: tip.height(),
            parent_core_hash: hex::encode(tip.core_hash().0),
            parent_result_hash: hex::encode(tip.result().0),
            transaction_hash,
            repair_plan_id: plan_id,
            execution_time_ms,
            applied_in_discarded_overlay: output.result().is_ok(),
            output,
            committed_world_before: before.to_string(),
            attempted_world,
            visited_world_fields: block_start.fields,
            block_start_changes: block_start.changes,
            network_changes: network_delta,
            committed_world_after: after.to_string(),
            state_published: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::World};
    use iroha_data_model::{isi::Log, prelude::TransactionBuilder, transaction::FeePaymentIntent};
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

    #[test]
    fn phase_inventory_keeps_network_reversion_and_rejects_changed_preimages() {
        let original = Hash::new(b"original");
        let started = Hash::new(b"started");
        let key = Some(Hash::new(b"key"));
        let start = WorldChanges {
            fields: vec!["smart_contract_state".into()],
            changes: vec![SorafsRepairWorldChange {
                field: "smart_contract_state".into(),
                key_hash: key,
                before_hash: Some(original),
                after_hash: Some(started),
            }],
        };
        let final_state = WorldChanges {
            fields: start.fields.clone(),
            changes: Vec::new(),
        };
        let delta = network_changes(&start, &final_state).unwrap();
        assert_eq!(delta.len(), 1);
        assert_eq!(delta[0].before_hash, Some(started));
        assert_eq!(delta[0].after_hash, Some(original));
        let mut foreign = WorldChanges {
            fields: start.fields.clone(),
            changes: start.changes.clone(),
        };
        foreign.changes[0].before_hash = None;
        assert!(
            network_changes(&start, &foreign)
                .unwrap_err()
                .contains("original World preimage")
        );
    }

    #[test]
    fn single_input_projection_never_aliases_a_second_source() {
        let signed = TransactionBuilder::new(
            iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::new(b"rehearsal-projection"),
            )),
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(iroha_data_model::Level::INFO, "projection".into())])
        .sign(ALICE_KEYPAIR.private_key());
        let input = TransactionEntrypoint::External(signed);
        let source = SingleInput(&input);
        assert_eq!(source.input_count(), 1);
        assert!(core::ptr::eq(source.input_at(0).unwrap(), &input));
        assert!(source.input_at(1).is_none());
    }

    #[test]
    fn rehearsal_refuses_non_repair_before_acquiring_an_overlay() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let signed = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            iroha_data_model::Level::INFO,
            "not a repair".into(),
        )])
        .sign(ALICE_KEYPAIR.private_key());
        assert!(
            state
                .rehearse_sorafs_repair(signed, 1)
                .unwrap_err()
                .contains("exactly one typed repair")
        );
        assert_eq!(state.committed_height(), 0);
    }

    #[test]
    fn signed_foreign_network_repair_uses_real_output_owner_and_discards_every_effect() {
        use crate::smartcontracts::isi::{
            sorafs_bootstrap::tests::Fixture, sorafs_provider_admission::test_fixture::key,
        };
        use iroha_data_model::isi::sorafs_repair::ApplySorafsRepairV1;

        let fixture = Fixture::new();
        let state = fixture.chain.state();
        let tip = state.view().native_execution_tip().unwrap();
        let certificate = fixture.certificate(&fixture.payload());
        certificate.verify().unwrap();
        let signed = fixture.chain.sign(
            &key(6),
            [ApplySorafsRepairV1 { certificate }.into()],
            tip.creation_time_ms() + 1,
        );
        let before = state.verify_world_state_accumulator().unwrap();
        let report = state
            .rehearse_sorafs_repair(signed, tip.creation_time_ms() + 2)
            .expect("real external execution completes with the production hard-pin rejection");
        assert!(!report.applied_in_discarded_overlay);
        assert!(!report.state_published);
        assert!(
            report
                .visited_world_fields
                .iter()
                .any(|field| field == "accounts")
        );
        assert!(
            report
                .visited_world_fields
                .iter()
                .any(|field| field == "assets")
        );
        assert!(
            report
                .network_changes
                .iter()
                .all(|row| row.field != "accounts" && row.field != "runtime_selection")
        );
        assert!(format!("{:?}", report.output.result()).contains("DefinitionInactive"));
        assert_eq!(report.committed_world_before, report.committed_world_after);
        assert_eq!(state.verify_world_state_accumulator().unwrap(), before);
        assert_eq!(state.view().native_execution_tip(), Some(tip));
    }
}
