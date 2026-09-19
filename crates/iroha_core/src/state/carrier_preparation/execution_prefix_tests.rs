//! Actual four-validator execution custody, transfer refusal and tail controls.

use super::*;
use crate::{block::valid::SumeragiV2ValidationContext, sumeragi::network_topology::Topology};
use iroha_data_model::block::{SignedBlock, consensus_v2::HeightContext};
use iroha_primitives::time::TimeSource;
use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID;
use mv::storage::StorageReadOnly;

fn validated<'state>(
    state: &'state State,
    proposal: SignedBlock,
    topology: &Topology,
    context: &HeightContext,
) -> (ValidBlock, Box<StateBlock<'state>>) {
    ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(
        proposal,
        topology,
        &SAMPLE_GENESIS_ACCOUNT_ID,
        &TimeSource::new_system(),
        state.sumeragi_block_cadence(),
        SumeragiV2ValidationContext::from_height_context(context),
        state,
        &mut None,
    )
    .unpack(|_| {})
    .unwrap_or_else(|(_, error)| panic!("real candidate execution: {error}"))
}

#[test]
fn prefix_capture_moves_original_sources_inventory_and_witness_without_publishing() {
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let (valid, staged) = validated(&state, proposal.clone(), &topology, &context);
    let expected = staged.execution_commitment_for_testing(&valid).unwrap();
    let inventory = Arc::clone(
        staged
            .fastpq_source_inventory
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap(),
    );
    let proving_context_present = staged.fastpq_witness_context.is_some();
    let casting_bindings = staged
        .parliament_timed_ovn_casting_bindings
        .as_ref()
        .map(|bindings| bindings.as_ptr());
    let witness = staged.exec_witness.as_ref().unwrap();
    assert!(!witness.writes.is_empty());
    let writes = witness.writes.as_ptr();
    let Some(output_capacity::ExecutionOutputPlanState::Sealed(sealed)) =
        staged.execution_output_plan.as_ref()
    else {
        panic!("actual execution must be sealed")
    };
    assert!(!sealed.sources().entries().is_empty());
    let sources = sealed.sources().entries().as_ptr();
    let (prepared, _, commitment) = PrefixPreparation::capture(staged, &valid, None)
        .unwrap_or_else(|error| panic!("capture exact prefix: {error}"));
    assert_eq!(commitment, expected);
    assert!(Arc::ptr_eq(prepared.prefix.inventory(), &inventory));
    assert_eq!(prepared.prefix.witness().writes.as_ptr(), writes);
    assert_eq!(prepared.prefix.sources().entries().as_ptr(), sources);
    assert_eq!(
        prepared.prefix.sources().entries().len(),
        valid.as_ref().execution_outputs().len()
    );
    assert!(prepared.prefix.retains_closed_state(&prepared.state));
    assert!(!prepared.prefix.sources().is_native());
    assert_eq!(
        prepared.prefix.sources().source_context().network_id,
        context.network_id
    );
    assert_eq!(
        prepared.prefix.sources().source_context().height,
        context.height
    );
    assert_eq!(
        prepared.prefix.fastpq_witness_context().is_some(),
        proving_context_present
    );
    assert_eq!(
        prepared
            .prefix
            .parliament_timed_ovn_casting_bindings()
            .map(|bindings| bindings.as_ptr()),
        casting_bindings
    );
    let PrefixPreparation {
        state: staged,
        prefix,
    } = prepared;
    assert!(matches!(
        (*staged).commit().unwrap_err(),
        storage_transactions::TransactionsBlockError::ExecutionOutputCapacity
    ));
    // Even retaining every valid prefix owner separately never opens raw commit.
    assert_eq!(prefix.witness().writes.as_ptr(), writes);
    drop(prefix);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.kura.blocks_count(), 0);
    drop(state.world.block());
    drop(state.transactions.block());
}

#[test]
fn prefix_capture_rejects_missing_witness_and_changed_owned_inventory_before_tail() {
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    for change in 0..3 {
        let (valid, mut staged) = validated(&state, proposal.clone(), &topology, &context);
        match change {
            0 => {
                staged.exec_witness = None;
            }
            1 => {
                staged.fastpq_source_inventory = None;
            }
            2 => {
                staged.fastpq_tx_set_hash =
                    Some(iroha_crypto::Hash::new(b"foreign transaction set").into());
            }
            _ => unreachable!(),
        }
        let error = PrefixPreparation::capture(staged, &valid, None)
            .err()
            .expect("missing original owner must refuse");
        assert!(
            error.contains("witness") || error.contains("inventory"),
            "{error}"
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
        assert_eq!(state.kura.blocks_count(), 0);
        drop(state.world.block());
        drop(state.transactions.block());
    }
}

#[test]
fn prefix_capture_rejects_changed_world_and_competing_membership_without_publication() {
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    for competing_membership in [false, true] {
        let (valid, mut staged) = validated(&state, proposal.clone(), &topology, &context);
        if competing_membership {
            staged
                .merge_carrier_entrypoints
                .insert(valid.as_ref().network_entrypoints().next().unwrap().hash());
        } else {
            // Actual World mutation after attachment must fail its sealed delta.
            staged.world.musubi_resolver_index_checkpoints.insert(
                MusubiResolverIndexRevisionV1::new(2).unwrap(),
                iroha_data_model::musubi::MusubiRegistrySnapshotV1 {
                    finalized_height: 1,
                    finalized_block_hash: *proposal.hash().as_ref(),
                    index_revision: 2,
                },
            );
        }
        let error = PrefixPreparation::capture(staged, &valid, None)
            .err()
            .expect("changed execution refused");
        assert!(
            if competing_membership {
                error.contains("source owner")
            } else {
                error.contains("World values changed")
            },
            "{error}"
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
        assert_eq!(state.kura.blocks_count(), 0);
    }
}

#[test]
fn completed_tail_retains_prefix_under_whole_candidate_admission_and_static_handoff() {
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let prepared = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("real candidate preparation: {error}"));
    let sources = prepared.source_prefix.sources().entries().as_ptr() as usize;
    let writes = prepared.source_prefix.witness().writes.as_ptr() as usize;
    let inventory = Arc::clone(prepared.source_prefix.inventory());
    assert!(
        prepared
            .source_prefix
            .retains_closed_state(prepared.state())
    );
    // The metadata tail changed World; its custody is retained rather than
    // incorrectly rechecking the old World delta against the completed tail.
    assert_eq!(
        prepared.state.world.musubi_resolver_index_checkpoints.len(),
        1
    );
    prepared
        .source_prefix
        .sealed
        .verify_wire_binding(prepared.block())
        .expect("post-tail source authentication preserves the original World seal meaning");
    let mut calls = 0;
    let journals = prepared
        .prepare_journals(None, None, |inputs| {
            calls += 1;
            assert_eq!(inputs.prefix.sources().entries().as_ptr() as usize, sources);
            assert_eq!(inputs.prefix.witness().writes.as_ptr() as usize, writes);
            assert!(Arc::ptr_eq(inputs.prefix.inventory(), &inventory));
            assert!(inputs.prefix.retains_closed_state(inputs.state));
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
    assert_eq!(calls, 1);
    drop(state.world.block());
    drop(state.transactions.block());
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
    drop(state);
    let journals = std::thread::spawn(move || journals).join().unwrap();
    assert_eq!(
        journals.source_prefix().sources().entries().as_ptr() as usize,
        sources
    );
    assert_eq!(
        journals.source_prefix().witness().writes.as_ptr() as usize,
        writes
    );
    assert!(Arc::ptr_eq(
        journals.source_prefix().inventory(),
        &inventory
    ));
    drop(journals);
}

#[test]
fn transferred_prefix_keeps_both_transaction_apply_paths_closed() {
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    for consensus_only in [false, true] {
        let (valid, staged) = validated(&state, proposal.clone(), &topology, &context);
        let (mut prepared, _, _) = PrefixPreparation::capture(staged, &valid, None)
            .unwrap_or_else(|error| panic!("capture exact prefix: {error}"));
        let writes = prepared.prefix.witness().writes.as_ptr();
        let key = MusubiResolverIndexRevisionV1::new(2).unwrap();
        assert!(
            prepared
                .state
                .world
                .musubi_resolver_index_checkpoints
                .get(&key)
                .is_none()
        );
        let mut transaction = prepared.state.transaction();
        transaction.world.musubi_resolver_index_checkpoints.insert(
            key,
            iroha_data_model::musubi::MusubiRegistrySnapshotV1 {
                finalized_height: 1,
                finalized_block_hash: *proposal.hash().as_ref(),
                index_revision: 2,
            },
        );
        if consensus_only {
            transaction.apply_consensus_effects();
        } else {
            transaction.apply();
        }
        assert!(
            prepared
                .state
                .world
                .musubi_resolver_index_checkpoints
                .get(&key)
                .is_none()
        );
        assert!(matches!(
            prepared.state.execution_output_plan,
            Some(output_capacity::ExecutionOutputPlanState::Poisoned)
        ));
        assert_eq!(prepared.prefix.witness().writes.as_ptr(), writes);
        assert!(matches!(
            (*prepared.state).commit().unwrap_err(),
            storage_transactions::TransactionsBlockError::ExecutionOutputCapacity
        ));
        drop(prepared.prefix);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
        assert_eq!(state.kura.blocks_count(), 0);
    }
}

#[test]
fn world_carrier_tail_rejects_a_scope_without_owned_execution() {
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let (valid, executed) = validated(&state, proposal, &topology, &context);
    drop(executed);
    let block = Box::new(state.block(valid.as_ref().header()));
    let before = crate::state::world_projection::WorldStateBaseline::capture_current(&block.world)
        .unwrap()
        .root();
    assert!(PrefixPreparation::capture(block, &valid, None).is_err());
    assert_eq!(
        crate::state::world_projection::WorldStateBaseline::capture_current(&state.world.block())
            .unwrap()
            .root(),
        before
    );
    assert_eq!(state.committed_height(), 0);
}
