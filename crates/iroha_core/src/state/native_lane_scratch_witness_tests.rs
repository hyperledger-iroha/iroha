// Standalone scratch refuses an unrelated live recorder before acquiring State.
// All transcripts below come from actual authenticated State transfers/hooks.

const NATIVE_SCRATCH_UNLOCK: &str = "native-scratch-witness-unlock";

#[inline(never)]
fn native_scratch_due_unlock_fixture() -> Box<NativeEconomicFixture> {
    native_economic_fixture_with_world_initializer(
        &[NativeEconomicCase::Transfer(25)],
        true,
        None,
        None,
        |world| {
            let owner = AccountId::new(
                KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519)
                    .unwrap()
                    .public_key()
                    .clone(),
            );
            let escrow = AccountId::new(
                KeyPair::try_from_seed(vec![0x72; 32], Algorithm::Ed25519)
                    .unwrap()
                    .public_key()
                    .clone(),
            );
            let definition = AssetDefinitionId::derive_from_components(
                DomainId::try_new("native-economics", "universal").unwrap(),
                "coin".parse().unwrap(),
            );
            // Reallocate the actual genesis supply, preserving its quantity and
            // the production genesis incarnation/configured-catalog owners.
            for (account, amount) in [(&owner, 90u32), (&escrow, 10u32)] {
                let (id, value) =
                    Asset::new(AssetId::new(definition.clone(), account.clone()), amount)
                        .into_key_value();
                world.assets.insert(id, value);
            }
            let mut locks = GovernanceLocksForReferendum::default();
            locks.locks.insert(
                owner.clone(),
                GovernanceLockRecord {
                    owner: owner.clone(),
                    amount: Quantity::from(10u32),
                    slashed: Quantity::zero(),
                    expiry_height: 6,
                    direction: 0,
                    duration_blocks: 0,
                    custody: GovernanceLockCustody {
                        escrowed: true,
                        asset_definition_id: definition,
                        bond_escrow_account: escrow,
                        slash_receiver_account: owner,
                    },
                },
            );
            let mut block = world.block();
            block.put_governance_locks(NATIVE_SCRATCH_UNLOCK.into(), locks);
            block.commit();
        },
    )
}

fn assert_native_scratch_unlock_applied(overlay: &StateBlock<'_>, fixture: &NativeEconomicFixture) {
    assert_eq!(overlay._curr_block.height().get(), 7);
    assert!(
        overlay
            .world
            .governance_locks
            .get(NATIVE_SCRATCH_UNLOCK)
            .is_none()
    );
    assert_eq!(*overlay.world.governance_last_unlock_sweep_height, 7);
    assert_eq!(
        overlay.world.assets.get(&fixture.source).unwrap().0,
        Quantity::from(75u32)
    );
    assert_eq!(
        overlay.world.assets.get(&fixture.destination).unwrap().0,
        Quantity::from(25u32)
    );
}

// Keep the actual constructor's heap owner throughout the test, just as native
// scratch does. Avoid several large StateBlock return temporaries on test stacks.
#[inline(never)]
fn native_scratch_owned_start(state: &State, header: BlockHeader) -> Box<StateBlock<'_>> {
    let (block, ()) = state
        .block_with_owned_start_stages(
            header,
            |_| Ok::<(), std::convert::Infallible>(()),
            |_, ()| Ok(()),
        )
        .unwrap();
    block
}

#[inline(never)]
fn native_scratch_recorded_start(
    state: &State,
    header: BlockHeader,
) -> (
    Box<StateBlock<'_>>,
    crate::sumeragi::witness::ExecWitnessGuard,
) {
    // The actual constructor retains the State writers before opening capture,
    // so the hook and following transfers share the proper lock order too.
    state
        .block_with_owned_start_stages(
            header,
            |_| crate::sumeragi::witness::begin_exec_witness_capture(),
            |_, guard| Ok(guard),
        )
        .unwrap()
}

#[inline(never)]
fn native_scratch_prove_due_hook_records(fixture: &NativeEconomicFixture) {
    use crate::sumeragi::witness;
    let header = empty_global_block_after(Some(&fixture.native.block)).header();
    let (overlay, _owner) = native_scratch_recorded_start(&fixture.native.state, header);
    assert_eq!(header.height().get(), 7);
    assert!(
        overlay
            .world
            .governance_locks
            .get(NATIVE_SCRATCH_UNLOCK)
            .is_none()
    );
    assert_eq!(
        overlay.world.assets.get(&fixture.source).unwrap().0,
        Quantity::from(100u32)
    );
    assert!(overlay.world.assets.get(&fixture.destination).is_none());
    assert!(!overlay.fastpq_transcripts.is_empty());
    let captured = witness::drain_exec_witness_checked(|_| Ok(())).unwrap();
    assert!(
        !captured.fastpq_transcripts.is_empty(),
        "the real due hook records before native economics starts"
    );
    drop(overlay);
}

#[inline(never)]
fn native_scratch_owner_transfer(overlay: &mut StateBlock<'_>, fixture: &NativeEconomicFixture) {
    let mut transaction = overlay.transaction();
    // Reuse a real typed protocol movement; it validates the exact staker and
    // performs debit/credit plus source capture. No transcript is synthesized.
    crate::smartcontracts::isi::asset::isi::execute_staking_bond_transfer(
        &mut transaction,
        fixture.source.account(),
        LaneId::SINGLE,
        fixture.destination.account(),
        fixture.source.account(),
        false,
        fixture.source.clone(),
        fixture.destination.clone(),
        Quantity::from(1u32),
    )
    .unwrap();
    transaction.apply();
}

#[inline(never)]
fn native_scratch_capture_around(
    owner: &NativeEconomicFixture,
    mut scratch: impl FnMut(),
) -> Vec<u8> {
    use crate::sumeragi::witness;
    let header = empty_global_block_after(Some(&owner.native.block)).header();
    let (mut overlay, _owner) = native_scratch_recorded_start(&owner.native.state, header);
    native_scratch_owner_transfer(&mut overlay, owner);
    let before = witness::snapshot_exec_witness();
    assert!(!before.fastpq_transcripts.is_empty());
    let before_bytes = norito::encode_canonical(&before).unwrap();
    scratch();
    assert_eq!(
        norito::encode_canonical(&witness::snapshot_exec_witness()).unwrap(),
        before_bytes
    );

    // The second real transfer is held in an existing generation-bound recorder
    // overlay. Reset/drain/rebind inside scratch would discard it on commit.
    let held = witness::begin_exec_witness_overlay();
    native_scratch_owner_transfer(&mut overlay, owner);
    scratch();
    assert_eq!(
        norito::encode_canonical(&witness::snapshot_exec_witness()).unwrap(),
        before_bytes
    );
    held.commit();
    let captured = witness::drain_exec_witness_checked(|_| Ok(())).unwrap();
    let bytes = norito::encode_canonical(&captured).unwrap();
    assert_ne!(
        bytes, before_bytes,
        "the original generation must accept its held real transfer"
    );
    assert_eq!(
        overlay.world.assets.get(&owner.source).unwrap().0,
        Quantity::from(98u32)
    );
    assert_eq!(
        overlay.world.assets.get(&owner.destination).unwrap().0,
        Quantity::from(2u32)
    );
    drop(overlay);
    bytes
}

state_test! { sync native_scratch_due_hooks_and_markers_preserve_unrelated_same_thread_witness
    let fixture = native_scratch_due_unlock_fixture();
    let owner = native_economic_fixture(&[NativeEconomicCase::Transfer(1)], true);
    assert!(!Arc::ptr_eq(&fixture.native.state.kura, &owner.native.state.kura));
    let groups = native_economic_groups(&fixture);
    let header = empty_global_block_after(Some(&fixture.native.block)).header();
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.native.state).expect("stable valid fixture snapshot");
    native_scratch_prove_due_hook_records(&fixture);
    let expected = native_scratch_capture_around(&owner, || {});
    for with_markers in [false, true] {
        // Actual scratch economics still runs and rolls back outside a capture.
        if with_markers {
            let prepared = fixture.native.state.prepare_native_batch_on_carrier(header, groups.clone()).unwrap();
            assert!(prepared.executions()[0].result.is_ok());
            assert_native_scratch_unlock_applied(prepared.overlay(), &fixture);
            assert!(prepared.overlay().native_lane_stage.is_some());
            drop(prepared);
        } else {
            let (overlay, executions) = fixture.native.state.preexecute_lane_decision_groups(header, &groups).unwrap();
            assert!(executions[0].result.is_ok());
            assert_native_scratch_unlock_applied(&overlay, &fixture);
            drop(overlay);
        }
        let actual = native_scratch_capture_around(&owner, || {
            let error = if with_markers {
                fixture.native.state.prepare_native_batch_on_carrier(header, groups.clone()).err()
            } else {
                fixture.native.state.preexecute_lane_decision_groups(header, &groups).err()
            }.expect("nested scratch must refuse before State acquisition");
            assert!(matches!(error, MergeLedgerCommitError::ExecutionRecorderConflict(_)), "{error}");
        });
        assert_eq!(actual, expected, "same real owner bytes with wrapper markers={with_markers}");
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.native.state).expect("stable valid fixture snapshot"), before);
    }
}

state_test! { sync native_scratch_constructor_failure_preserves_unrelated_same_thread_witness
    let fixture = native_scratch_due_unlock_fixture();
    let owner = native_economic_fixture(&[NativeEconomicCase::Transfer(1)], true);
    let groups = native_economic_groups(&fixture);
    let mut foreign_parent = empty_global_block_after(Some(&fixture.native.block)).header();
    foreign_parent.set_prev_block_hash(Some(HashOf::from_untyped_unchecked(Hash::new(b"foreign scratch parent"))));
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.native.state).expect("stable valid fixture snapshot");
    let expected = native_scratch_capture_around(&owner, || {});
    for with_markers in [false, true] {
        let error = if with_markers {
            fixture.native.state.prepare_native_batch_on_carrier(foreign_parent, groups.clone()).err()
        } else {
            fixture.native.state.preexecute_lane_decision_groups(foreign_parent, &groups).err()
        }.expect("pre-State constructor must refuse another parent");
        assert!(matches!(error, MergeLedgerCommitError::ExecutionBatchInvalid(_)), "{error}");
        let actual = native_scratch_capture_around(&owner, || {
            let error = if with_markers {
                fixture.native.state.prepare_native_batch_on_carrier(foreign_parent, groups.clone()).err()
            } else {
                fixture.native.state.preexecute_lane_decision_groups(foreign_parent, &groups).err()
            }.expect("nested scratch must refuse before inspecting the parent");
            assert!(matches!(error, MergeLedgerCommitError::ExecutionRecorderConflict(_)), "{error}");
        });
        assert_eq!(actual, expected);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.native.state).expect("stable valid fixture snapshot"), before);
    }
}

state_test! { sync native_scratch_late_marker_failure_rolls_back_hook_and_preserves_owner_generation
    use iroha_model_base::state_path::StatePath;
    let fixture = native_scratch_due_unlock_fixture();
    let owner = native_economic_fixture(&[NativeEconomicCase::Transfer(1)], true);
    let groups = native_economic_groups(&fixture);
    let slot = &groups[0].body().payload().descriptor.slots[1];
    let marker: StatePath = format!("native_lane_applied_instance_{}", hex::encode(slot.instance_id.as_ref())).parse().unwrap();
    let mut storage = fixture.native.state.world.smart_contract_state.block();
    storage.insert(marker, norito::encode_canonical(&Hash::new(b"existing native application")).unwrap());
    storage.commit();
    let header = empty_global_block_after(Some(&fixture.native.block)).header();
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.native.state).expect("stable valid fixture snapshot");
    let error = fixture.native.state.prepare_native_batch_on_carrier(header, groups.clone()).err().expect("late native marker collision");
    assert!(matches!(error, MergeLedgerCommitError::ExecutionMarkerConflict(_)), "{error}");
    let expected = native_scratch_capture_around(&owner, || {});
    let actual = native_scratch_capture_around(&owner, || {
        let error = fixture.native.state.prepare_native_batch_on_carrier(header, groups.clone()).err().expect("nested scratch must refuse before inspecting markers");
        assert!(matches!(error, MergeLedgerCommitError::ExecutionRecorderConflict(_)), "{error}");
    });
    assert_eq!(actual, expected);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.native.state).expect("stable valid fixture snapshot"), before);
    let world = fixture.native.state.world.view();
    assert!(world.governance_locks.get(NATIVE_SCRATCH_UNLOCK).is_some());
    assert_eq!(world.assets.get(&fixture.source).unwrap().0, Quantity::from(90u32));
    assert_eq!(world.assets.get(&fixture.destination).unwrap().0, Quantity::from(10u32));
}

state_test! { sync native_scratch_entries_refuse_recorder_owner_before_waiting_for_state
    use super::NativeLaneBatchSourcePreparationV1;
    use std::sync::atomic::{AtomicBool, Ordering};
    use crate::sumeragi::witness;
    let (fixture, finalized_carrier, included) = retained_native_batch_fixture();
    let carrier = finalized_carrier.canonical_resultless_proposal();
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let batch = state.prepare_lane_decision_batch(&groups).unwrap();
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state.prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
        else { panic!("original source"); };
    let header = carrier.header();
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let guard = witness::begin_exec_witness_capture().unwrap();
    witness::record_read_asset(&fixture.source, Some(&Quantity::from(100u32)));
    let witness_before = norito::encode_canonical(&witness::snapshot_exec_witness()).unwrap();
    let released = AtomicBool::new(false);
    let (held_tx, held_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let released = &released;
        let holder = scope.spawn(move || {
            let overlay = native_scratch_owned_start(state, header);
            held_tx.send(()).unwrap();
            // A missing early check fails finitely. Never acquire the recorder
            // here: the timeout must be able to release the State writer.
            let _ = release_rx.recv_timeout(std::time::Duration::from_secs(5));
            released.store(true, Ordering::Release);
            drop(overlay);
        });
        held_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        // Suppression does not waive recorder ownership. Each wrapper must
        // refuse before its own observation, snapshot, context or State writer.
        let suppression = witness::suppress_recording_for_current_thread();
        let errors = [
            state.preexecute_lane_decision_groups(header, &groups).err(),
            state.prepare_native_batch_on_carrier(header, groups.clone()).err(),
            state.prepare_lane_decision_batch(&groups).err(),
            state.replay_lane_decision_batch(&header, &batch, groups.clone()).err(),
            source.stage_with_start_hooks().err(),
            state.replay_proposed_native_lane_batch(&carrier, &[]).err(),
            state.replay_finalized_native_lane_batch(&included, &[]).err(),
        ];
        let source_errors = [
            state.prepare_proposed_native_lane_batch_source(&carrier, &[]).err(),
            state.prepare_finalized_native_lane_batch_source(&included, &[]).err(),
        ];
        let refused_while_held = !released.load(Ordering::Acquire);
        drop(suppression);
        let _ = release_tx.send(());
        holder.join().unwrap();
        for (index, error) in errors.into_iter().enumerate() {
            assert!(matches!(error, Some(MergeLedgerCommitError::ExecutionRecorderConflict(_))), "entry {index}: {error:?}");
        }
        for error in source_errors {
            assert!(error.unwrap().contains("already belongs"));
        }
        assert!(refused_while_held, "all Native entries must refuse before waiting for State");
    });
    assert_eq!(norito::encode_canonical(&witness::snapshot_exec_witness()).unwrap(), witness_before);
    witness::drain_exec_witness_checked(|_| Ok(())).unwrap();
    drop(guard);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
}
