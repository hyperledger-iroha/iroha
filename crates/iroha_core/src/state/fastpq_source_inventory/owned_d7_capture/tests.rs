//! Draft preparation regressions; these are not capture, finality or policy qualification.

use super::super::tests::{
    apply_source, cache_canonical_test_transaction_set, delta, header, state,
};
use super::*;
use crate::sumeragi::witness;
use iroha_data_model::{
    NetworkId, Registrable,
    block::consensus::ExecWitness,
    execution_witness::FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1,
    permission::Permission,
    role::{Role, RoleId},
};
use iroha_primitives::{json::Json, numeric::Quantity};
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;

type TranscriptMap = BTreeMap<Hash, Vec<TransferTranscript>>;

fn limits() -> FastpqSourceStatementBuildLimits {
    // Fixture-only caps. No value here is a production or authenticated policy default.
    FastpqSourceStatementBuildLimits {
        max_executed_entries: 8,
        max_transcripts: 8,
        max_deltas: 16,
        max_input_transcript_bytes: 1_000_000,
        max_statement_bytes: 1_000_000,
        max_total_statement_bytes: 4_000_000,
    }
}

fn seal_source(block: &mut StateBlock<'_>, source: Hash) -> TranscriptMap {
    apply_source(block, source, false, None);
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    block.drain_transfer_transcripts_with_pending(None)
}

fn role(id: &str, permission: &str, epoch: u64) -> Role {
    let role_id: RoleId = id.parse().unwrap();
    Role::new(role_id, (*ALICE_ID).clone())
        .add_permission_with_epoch(Permission::new(permission.to_owned(), Json::new(())), epoch)
        .build(&ALICE_ID)
}

fn assert_unpublished(block: &StateBlock<'_>) {
    assert!(block.exec_witness.is_none());
    assert!(block.fastpq_witness_context.is_none());
    assert!(block.parliament_timed_ovn_casting_bindings.is_none());
    assert!(block.fastpq_source_inventory.as_ref().unwrap().is_ok());
}

fn empty_witness() -> ExecWitness {
    ExecWitness {
        reads: Vec::new(),
        writes: Vec::new(),
        fastpq_transcripts: Vec::new(),
        fastpq_batches: Vec::new(),
    }
}

#[test]
fn checked_raw_drain_prepares_final_context_without_publishing_or_inserting_d7() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let source = Hash::new(b"owned D7 raw recorder callback");
    let archive = seal_source(&mut block, source);
    assert!(block.fastpq_transcripts.is_empty());
    let inventory = block
        .verified_fastpq_source_inventory_for_capture()
        .unwrap();

    // The final carrier and world become available after the inventory was sealed.
    block._curr_block.creation_time_ms = 23;
    let completed_role = role("d7_completed_role", "d7_completed_permission", 4);
    block
        .world
        .roles
        .insert(completed_role.id.clone(), completed_role);
    let perm_root = crate::fastpq::permission_table_root(block.world.roles.iter());
    assert_ne!(perm_root, [0; 32]);
    let expected = inventory
        .derive_manifest(23_000_000, perm_root, &archive, limits())
        .unwrap();
    let mut prepared = None;
    let drained = witness::drain_exec_witness_checked(|raw| {
        assert_eq!(raw, &archive);
        prepared = Some(block.prepare_owned_fastpq_d7_capture(raw, limits())?);
        assert_eq!(raw, &archive);
        Ok(())
    })
    .unwrap();
    assert_eq!(drained.fastpq_transcripts.len(), 1);
    assert_eq!(drained.fastpq_transcripts[0].entry_hash, source);
    assert_eq!(drained.fastpq_transcripts[0].transcripts, archive[&source]);
    assert!(drained.fastpq_batches.is_empty());
    assert!(
        drained
            .writes
            .iter()
            .all(|write| write.key.first()
                != FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1.first())
    );
    assert_unpublished(&block);

    let prepared = prepared.unwrap();
    let leaf_allocation = prepared.leaves.as_ptr();
    let (manifest, leaves, context) = prepared.into_parts();
    assert_eq!((manifest, &leaves), (expected.0, &expected.1));
    assert_eq!(
        leaves.as_ptr(),
        leaf_allocation,
        "the leaf allocation is moved"
    );
    assert!(Arc::ptr_eq(&context.inventory, &inventory));
    assert!(std::ptr::eq(context.inventory(), inventory.as_ref()));
    assert_eq!(context.creation_time_ms(), 23);
    assert_eq!(context.slot(), 23_000_000);
    assert_eq!(context.permission_root(), perm_root);
    let retained_limits = context.limits();
    let expected_limits = limits();
    assert_eq!(
        retained_limits.max_executed_entries,
        expected_limits.max_executed_entries
    );
    assert_eq!(
        retained_limits.max_transcripts,
        expected_limits.max_transcripts
    );
    assert_eq!(retained_limits.max_deltas, expected_limits.max_deltas);
    assert_eq!(
        retained_limits.max_input_transcript_bytes,
        expected_limits.max_input_transcript_bytes
    );
    assert_eq!(
        retained_limits.max_statement_bytes,
        expected_limits.max_statement_bytes
    );
    assert_eq!(
        retained_limits.max_total_statement_bytes,
        expected_limits.max_total_statement_bytes
    );
    drop(leaves);
    assert_eq!(*context.manifest(), manifest);
    assert!(context.verify_current(&block).is_ok());
    assert_unpublished(&block);
}

#[test]
fn empty_and_nontransfer_inventories_keep_complete_entry_counts() {
    let _guard = witness::exec_witness_guard();
    let state = state();
    for nontransfer in [false, true] {
        witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        let times = nontransfer.then(|| Hash::new(b"D7 nontransfer time invocation"));
        let times = times.into_iter().collect::<Vec<_>>();
        block
            .finalize_fastpq_source_inventory(&[], &[], &times)
            .unwrap();
        let archive = block.drain_transfer_transcripts_with_pending(None);
        assert!(archive.is_empty());
        let exact = FastpqSourceStatementBuildLimits {
            max_executed_entries: u32::from(nontransfer),
            ..limits()
        };
        let (manifest, leaves, context) = block
            .prepare_owned_fastpq_d7_capture(&archive, exact)
            .unwrap()
            .into_parts();
        assert_eq!(manifest.executed_entry_count, u32::from(nontransfer));
        assert_eq!(manifest.statement_count, 0);
        assert!(leaves.is_empty());
        assert_eq!(
            context.inventory().entries().len(),
            usize::from(nontransfer)
        );
        assert!(context.verify_current(&block).is_ok());
        if nontransfer {
            let too_small = FastpqSourceStatementBuildLimits {
                max_executed_entries: 0,
                ..exact
            };
            assert!(
                block
                    .prepare_owned_fastpq_d7_capture(&archive, too_small)
                    .is_err()
            );
        }
        assert_unpublished(&block);
    }
}

#[test]
fn slot_boundaries_match_existing_template_and_retain_exact_milliseconds() {
    let _guard = witness::exec_witness_guard();
    let state = state();
    for timestamp in [
        0,
        1,
        u64::MAX / 1_000_000,
        u64::MAX / 1_000_000 + 1,
        u64::MAX,
    ] {
        witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        block
            .finalize_fastpq_source_inventory(&[], &[], &[])
            .unwrap();
        block._curr_block.creation_time_ms = timestamp;
        let (_, _, context) = block
            .prepare_owned_fastpq_d7_capture(&TranscriptMap::new(), limits())
            .unwrap()
            .into_parts();
        let template = crate::fastpq::public_inputs_template_from_block(
            &block._curr_block,
            &empty_witness(),
            [0; 32],
        );
        assert_eq!(context.creation_time_ms(), timestamp);
        assert_eq!(context.slot(), timestamp.saturating_mul(1_000_000));
        assert_eq!(context.slot(), template.slot);
        assert!(context.verify_current(&block).is_ok());
        if timestamp > u64::MAX / 1_000_000 {
            block._curr_block.creation_time_ms = if timestamp == u64::MAX {
                timestamp - 1
            } else {
                timestamp + 1
            };
            assert_eq!(
                block._curr_block.creation_time_ms.saturating_mul(1_000_000),
                context.slot()
            );
            assert_eq!(
                context.verify_current(&block).unwrap_err(),
                "FASTPQ prepared D7 carrier timestamp changed"
            );
        }
    }
}

#[test]
fn empty_manifest_does_not_itself_commit_timestamp_or_permission_rows() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    let (before, _, context) = block
        .prepare_owned_fastpq_d7_capture(&TranscriptMap::new(), limits())
        .unwrap()
        .into_parts();
    block._curr_block.creation_time_ms += 1;
    let changed = role("d7_empty_context", "d7_empty_permission", 0);
    block.world.roles.insert(changed.id.clone(), changed);
    let (after, leaves, later) = block
        .prepare_owned_fastpq_d7_capture(&TranscriptMap::new(), limits())
        .unwrap()
        .into_parts();
    assert_eq!(
        before, after,
        "there is no statement digest to bind these inputs"
    );
    assert!(leaves.is_empty());
    assert_ne!(context.creation_time_ms(), later.creation_time_ms());
    assert_ne!(context.permission_root(), later.permission_root());
    assert!(context.verify_current(&block).is_err());
}

#[test]
fn retained_context_rejects_equal_inventory_reallocated_under_another_owner() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    let (_, _, context) = block
        .prepare_owned_fastpq_d7_capture(&TranscriptMap::new(), limits())
        .unwrap()
        .into_parts();
    let replacement = Arc::new(context.inventory().clone());
    assert_eq!(replacement.as_ref(), context.inventory());
    block.fastpq_source_inventory = Some(Ok(replacement));
    assert_eq!(
        context.verify_current(&block).unwrap_err(),
        "FASTPQ prepared D7 inventory owner changed"
    );
    block.fastpq_source_inventory = Some(Ok(Arc::clone(&context.inventory)));
    assert!(context.verify_current(&block).is_ok());
}

#[test]
fn context_and_preparation_reject_missing_failed_or_stale_owned_source() {
    let _guard = witness::exec_witness_guard();
    let state = state();
    for mutation in 0..8 {
        witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        block
            .finalize_fastpq_source_inventory(&[], &[], &[])
            .unwrap();
        let archive = TranscriptMap::new();
        let (_, _, context) = block
            .prepare_owned_fastpq_d7_capture(&archive, limits())
            .unwrap()
            .into_parts();
        match mutation {
            0 => block.fastpq_source_inventory = None,
            1 => {
                block.fastpq_source_inventory = Some(Err("retained D7 construction failure".into()))
            }
            2 => block.fastpq_source_captures = Default::default(),
            3 => block.fastpq_source_context = None,
            4 => block._curr_block.height = nonzero!(2_u64),
            5 => block.network_id = NetworkId::from_genesis_hash(block._curr_block.hash()),
            6 => {
                Arc::make_mut(block.fastpq_source_context.as_mut().unwrap())
                    .source
                    .height += 1
            }
            7 => block.fastpq_tx_set_hash = Some([99; 32]),
            _ => unreachable!(),
        }
        assert!(
            context.verify_current(&block).is_err(),
            "mutation {mutation}"
        );
        let error = block
            .prepare_owned_fastpq_d7_capture(&archive, limits())
            .unwrap_err();
        if mutation == 1 {
            assert_eq!(error, "retained D7 construction failure");
        }
    }
}

#[test]
fn permission_row_id_value_and_epoch_drift_invalidate_retained_context() {
    let _guard = witness::exec_witness_guard();
    let state = state();
    for mutation in 0..3 {
        witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        let original = role("d7_role", "d7_permission", 4);
        block
            .world
            .roles
            .insert(original.id.clone(), original.clone());
        block
            .finalize_fastpq_source_inventory(&[], &[], &[])
            .unwrap();
        let (_, _, context) = block
            .prepare_owned_fastpq_d7_capture(&TranscriptMap::new(), limits())
            .unwrap()
            .into_parts();
        let replacement = match mutation {
            0 => role("d7_other_role", "d7_permission", 4),
            1 => role("d7_role", "d7_other_permission", 4),
            2 => role("d7_role", "d7_permission", 5),
            _ => unreachable!(),
        };
        block.world.roles.remove(original.id.clone());
        let replacement_id = replacement.id.clone();
        block
            .world
            .roles
            .insert(replacement.id.clone(), replacement);
        assert_eq!(
            context.verify_current(&block).unwrap_err(),
            "FASTPQ prepared D7 permission context changed"
        );
        block.world.roles.remove(replacement_id);
        block.world.roles.insert(original.id.clone(), original);
        assert!(context.verify_current(&block).is_ok());
    }
}

#[test]
fn unrelated_header_fields_and_later_replay_bookkeeping_do_not_rewrite_d7_inputs() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    let (_, _, context) = block
        .prepare_owned_fastpq_d7_capture(&TranscriptMap::new(), limits())
        .unwrap()
        .into_parts();
    block._curr_block.view_change_index += 1;
    block._curr_block.sccp_commitment_root = Some([42; 32]);
    assert!(context.verify_current(&block).is_ok());
    block.authenticated_replay_commit = true;
    assert!(context.verify_current(&block).is_ok());
    assert_eq!(
        block
            .prepare_owned_fastpq_d7_capture(&TranscriptMap::new(), limits())
            .unwrap_err(),
        "authenticated replay cannot prepare new ordinary FASTPQ D7 facts"
    );
    assert_unpublished(&block);
}

#[test]
fn late_applied_occurrences_reject_even_when_their_archive_is_drained() {
    let _guard = witness::exec_witness_guard();
    let state = state();
    for same_key in [false, true] {
        witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        let source = Hash::new(b"D7 original occurrence");
        let archive = seal_source(&mut block, source);
        let (_, _, context) = block
            .prepare_owned_fastpq_d7_capture(&archive, limits())
            .unwrap()
            .into_parts();
        let late = if same_key {
            source
        } else {
            Hash::new(b"D7 late source")
        };
        apply_source(&mut block, late, false, None);
        block.drain_transfer_transcripts_with_pending(None);
        assert!(context.verify_current(&block).is_err());
        assert!(
            block
                .prepare_owned_fastpq_d7_capture(&archive, limits())
                .is_err()
        );
    }
}

#[test]
fn rolled_back_occurrence_and_empty_apply_preserve_prepared_context() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let source = Hash::new(b"D7 retained source through rollback");
    let archive = seal_source(&mut block, source);
    let (_, _, context) = block
        .prepare_owned_fastpq_d7_capture(&archive, limits())
        .unwrap()
        .into_parts();
    {
        let overlay = witness::begin_exec_witness_overlay();
        let mut transaction = block.transaction();
        transaction.tx_call_hash = Some(source);
        transaction.record_test_transfer_transcripts(&ALICE_ID, source, vec![delta()]);
        drop(transaction);
        drop(overlay);
    }
    block.transaction().apply();
    assert!(context.verify_current(&block).is_ok());
    assert!(
        block
            .prepare_owned_fastpq_d7_capture(&archive, limits())
            .is_ok()
    );
}

#[test]
fn changed_raw_public_content_keys_and_occurrence_count_fail_without_latching() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let source = Hash::new(b"D7 immutable public archive");
    let original = seal_source(&mut block, source);
    for mutation in 0..5 {
        let mut changed = original.clone();
        match mutation {
            0 => {
                changed.remove(&source);
            }
            1 => {
                changed.insert(Hash::new(b"D7 invented bundle"), changed[&source].clone());
            }
            2 => {
                changed.get_mut(&source).unwrap()[0].authority_digest =
                    Hash::new(b"D7 changed authority")
            }
            3 => changed.get_mut(&source).unwrap()[0].poseidon_preimage_digest = None,
            4 => {
                let duplicate = changed[&source][0].clone();
                changed.get_mut(&source).unwrap().push(duplicate);
            }
            _ => unreachable!(),
        }
        let retained = changed.clone();
        assert!(
            block
                .prepare_owned_fastpq_d7_capture(&changed, limits())
                .is_err(),
            "mutation {mutation}"
        );
        assert_eq!(changed, retained);
        assert_unpublished(&block);
    }
    assert!(
        block
            .prepare_owned_fastpq_d7_capture(&original, limits())
            .is_ok()
    );
    witness::drain_exec_witness_checked(|raw| {
        assert_eq!(
            raw, &original,
            "helper calls leave the active recorder unchanged"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn every_explicit_construction_cap_is_enforced_without_partial_publication() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let source = Hash::new(b"D7 independent construction caps");
    let archive = seal_source(&mut block, source);
    for cap in 0..6 {
        let mut reduced = limits();
        match cap {
            0 => reduced.max_executed_entries = 0,
            1 => reduced.max_transcripts = 0,
            2 => reduced.max_deltas = 0,
            3 => reduced.max_input_transcript_bytes = 0,
            4 => reduced.max_statement_bytes = 0,
            5 => reduced.max_total_statement_bytes = 0,
            _ => unreachable!(),
        }
        assert!(
            block
                .prepare_owned_fastpq_d7_capture(&archive, reduced)
                .is_err(),
            "cap {cap}"
        );
        assert_unpublished(&block);
    }
    assert!(
        block
            .prepare_owned_fastpq_d7_capture(&archive, limits())
            .is_ok()
    );
}

#[test]
fn changed_private_paths_are_bounded_but_do_not_change_prepared_public_leaves() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let source = Hash::new(b"D7 private paths stay outside the public seal");
    let original = seal_source(&mut block, source);
    let (expected_manifest, expected_leaves, _) = block
        .prepare_owned_fastpq_d7_capture(&original, limits())
        .unwrap()
        .into_parts();
    let mut changed = original.clone();
    let path = &mut changed.get_mut(&source).unwrap()[0].deltas[0].from_smt_witness;
    path.root_before = [3; 32];
    path.root_after = [5; 32];
    path.path_bits = vec![7; 33];
    path.siblings = vec![[9; 32]; 65];
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let measured = norito::core::encoded_frame_len(&changed[&source][0]).unwrap();
    let exact = FastpqSourceStatementBuildLimits {
        max_input_transcript_bytes: measured,
        ..limits()
    };
    let (manifest, leaves, context) = block
        .prepare_owned_fastpq_d7_capture(&changed, exact)
        .unwrap()
        .into_parts();
    assert_eq!((manifest, leaves), (expected_manifest, expected_leaves));
    assert_eq!(context.limits().max_input_transcript_bytes, measured);
    let too_small = FastpqSourceStatementBuildLimits {
        max_input_transcript_bytes: measured - 1,
        ..exact
    };
    assert!(
        block
            .prepare_owned_fastpq_d7_capture(&changed, too_small)
            .is_err()
    );
    assert!(context.verify_current(&block).is_ok());
    assert_unpublished(&block);
}

#[test]
fn full_domain_quantity_preparation_uses_the_strict_source_producer() {
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let source = Hash::new(b"D7 quantity above legacy u64");
    let mut transfer = delta();
    transfer.amount = "18446744073709551616".parse().unwrap();
    transfer.from_balance_before = "36893488147419103232".parse().unwrap();
    transfer.from_balance_after = transfer.amount.clone();
    transfer.to_balance_before = Quantity::zero();
    transfer.to_balance_after = transfer.amount.clone();
    let mut transaction = block.transaction();
    transaction.tx_call_hash = Some(source);
    transaction.record_test_transfer_transcripts(&ALICE_ID, source, vec![transfer]);
    transaction.apply();
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    let archive = block.drain_transfer_transcripts_with_pending(None);
    let (manifest, leaves, context) = block
        .prepare_owned_fastpq_d7_capture(&archive, limits())
        .unwrap()
        .into_parts();
    assert_eq!(manifest.executed_entry_count, 1);
    assert_eq!(manifest.statement_count, 1);
    assert_eq!(leaves.len(), 1);
    assert_eq!(leaves[0].entry_hash, source);
    assert!(context.verify_current(&block).is_ok());
    assert_unpublished(&block);
}
