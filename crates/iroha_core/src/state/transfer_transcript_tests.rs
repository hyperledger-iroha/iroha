// Transfer-transcript tests remain in the parent state test module.

use iroha_data_model::block::BlockHeader;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use nonzero_ext::nonzero;

use super::*;
use crate::kura::Kura;

fn sample_delta(amount: u32) -> TransferDeltaTranscript {
    TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        amount: Quantity::from(amount),
        from_balance_before: Quantity::from(100_u32),
        from_balance_after: Quantity::from(100_u32.saturating_sub(amount)),
        to_balance_before: Quantity::zero(),
        to_balance_after: Quantity::from(amount),
        from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
    }
}

#[test]
fn transfer_transcripts_flush_into_block_map_on_apply() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    let call_hash = iroha_crypto::Hash::prehashed([0_u8; iroha_crypto::Hash::LENGTH]);
    tx.tx_call_hash = Some(call_hash);
    let asset_definition: iroha_data_model::asset::AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
    let delta = TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition,
        amount: Quantity::zero(),
        from_balance_before: Quantity::zero(),
        from_balance_after: Quantity::zero(),
        to_balance_before: Quantity::zero(),
        to_balance_after: Quantity::zero(),
        from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
    };
    let expected_poseidon = crate::fastpq::poseidon_preimage_digest(&delta, &call_hash);
    tx.record_transfer_transcript(&ALICE_ID, delta)
        .expect("record transcript");
    assert_eq!(
        tx.pending_transfer_transcripts[0].poseidon_preimage_digest,
        Some(expected_poseidon),
        "single-delta transfer transcript digest should be computed while recording"
    );
    tx.apply();
    let transcripts = block.drain_transfer_transcripts();
    let entry = transcripts
        .get(&call_hash)
        .expect("transcripts recorded for call hash");
    assert_eq!(entry.len(), 1);
    let transcript = &entry[0];
    assert_eq!(
        transcript.authority_digest,
        crate::fastpq::authority_digest(&ALICE_ID)
    );
    assert_eq!(transcript.poseidon_preimage_digest, Some(expected_poseidon));
}

#[test]
fn transfer_transcripts_reject_missing_call_hash() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    let asset_definition: iroha_data_model::asset::AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
    let delta = TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition,
        amount: Quantity::zero(),
        from_balance_before: Quantity::zero(),
        from_balance_after: Quantity::zero(),
        to_balance_before: Quantity::zero(),
        to_balance_after: Quantity::zero(),
        from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
    };
    let err = tx
        .record_transfer_transcript(&ALICE_ID, delta)
        .expect_err("missing transaction call_hash must fail");
    assert!(
        err.to_string().contains("transaction call_hash"),
        "unexpected error: {err}"
    );
    tx.apply();
    let transcripts = block.drain_transfer_transcripts();
    assert!(transcripts.is_empty());
}

#[test]
fn transfer_transcript_identity_preflight_fails_closed_during_replay() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);

    {
        let mut transaction = block.transaction();
        let error = transaction
            .require_transfer_transcript_identity("test transfer")
            .expect_err("live transfer without call_hash must fail closed");
        assert!(error.to_string().contains("transaction call_hash"));

        let call_hash = iroha_crypto::Hash::prehashed([0xA5; iroha_crypto::Hash::LENGTH]);
        transaction.tx_call_hash = Some(call_hash);
        assert_eq!(
            transaction
                .require_transfer_transcript_identity("test transfer")
                .expect("live transfer with call_hash must pass"),
            call_hash
        );
    }

    block.replay_compatibility = true;
    let transaction = block.transaction();
    let error = transaction
        .require_transfer_transcript_identity("test replay transfer")
        .expect_err("replay transfer without call_hash must fail closed");
    assert!(error.to_string().contains("transaction call_hash"));
}

#[test]
fn replay_transfer_transcripts_reject_missing_call_hash_without_fastpq_work() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    block.replay_compatibility = true;
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    crate::sumeragi::witness::start_block();
    let mut tx = block.transaction();

    tx.record_transfer_transcript(&ALICE_ID, sample_delta(1))
        .expect_err("replay mode must not bypass transcript identity");
    assert!(
        tx.pending_transfer_transcripts.is_empty(),
        "rejected replay transfer must not stage FASTPQ transcripts"
    );
    tx.apply();

    assert!(block.drain_transfer_transcripts().is_empty());
    let witness = crate::sumeragi::witness::drain_exec_witness();
    assert!(witness.fastpq_transcripts.is_empty());
    assert!(witness.fastpq_batches.is_empty());
}

#[test]
fn generated_rwa_id_rejects_missing_call_hash() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    let domain = DomainId::try_new("wonderland", "universal").unwrap();

    let err = tx
        .next_generated_rwa_id(&domain, "test")
        .expect_err("missing transaction call_hash must fail");
    assert!(
        err.to_string().contains("transaction call_hash"),
        "unexpected error: {err}"
    );
}

#[test]
fn transfer_transcripts_batch_records_multiple_deltas() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    tx.tx_call_hash = Some(iroha_crypto::Hash::prehashed(
        [1_u8; iroha_crypto::Hash::LENGTH],
    ));
    let asset_definition: iroha_data_model::asset::AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
    let delta_a = TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: asset_definition.clone(),
        amount: Quantity::from(10_u32),
        from_balance_before: Quantity::from(100_u32),
        from_balance_after: Quantity::from(90_u32),
        to_balance_before: Quantity::from(0_u32),
        to_balance_after: Quantity::from(10_u32),
        from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
    };
    let delta_b = TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition,
        amount: Quantity::from(5_u32),
        from_balance_before: Quantity::from(90_u32),
        from_balance_after: Quantity::from(85_u32),
        to_balance_before: Quantity::from(10_u32),
        to_balance_after: Quantity::from(15_u32),
        from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
    };
    tx.record_transfer_transcripts(&ALICE_ID, vec![delta_a.clone(), delta_b.clone()])
        .expect("record batch transcript");
    tx.apply();
    let transcripts = block.drain_transfer_transcripts();
    assert_eq!(transcripts.len(), 1);
    let entry = transcripts.values().next().expect("batch recorded");
    assert_eq!(entry.len(), 1);
    let transcript = &entry[0];
    assert_eq!(transcript.deltas, vec![delta_a, delta_b]);
    assert_eq!(
        transcript.authority_digest,
        crate::fastpq::authority_digest(&ALICE_ID)
    );
    assert!(transcript.poseidon_preimage_digest.is_none());
}

#[test]
fn transfer_transcripts_batch_flushes_each_recorded_transaction_hash() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    let call_hash_a = iroha_crypto::Hash::prehashed([2_u8; iroha_crypto::Hash::LENGTH]);
    let call_hash_b = iroha_crypto::Hash::prehashed([3_u8; iroha_crypto::Hash::LENGTH]);
    let asset_definition: iroha_data_model::asset::AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
    let delta_a = TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: asset_definition.clone(),
        amount: Quantity::from(10_u32),
        from_balance_before: Quantity::from(100_u32),
        from_balance_after: Quantity::from(90_u32),
        to_balance_before: Quantity::from(0_u32),
        to_balance_after: Quantity::from(10_u32),
        from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
    };
    let delta_b = TransferDeltaTranscript {
        from_account: (*BOB_ID).clone(),
        to_account: (*ALICE_ID).clone(),
        asset_definition,
        amount: Quantity::from(5_u32),
        from_balance_before: Quantity::from(10_u32),
        from_balance_after: Quantity::from(5_u32),
        to_balance_before: Quantity::from(90_u32),
        to_balance_after: Quantity::from(95_u32),
        from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
    };
    tx.tx_call_hash = Some(call_hash_a);
    tx.record_transfer_transcript(&ALICE_ID, delta_a.clone())
        .expect("record first transcript");
    tx.tx_call_hash = Some(call_hash_b);
    tx.record_transfer_transcript(&BOB_ID, delta_b.clone())
        .expect("record second transcript");
    tx.tx_call_hash = None;
    tx.apply();

    let transcripts = block.drain_transfer_transcripts();
    assert_eq!(transcripts.len(), 2);
    assert_eq!(
        transcripts
            .get(&call_hash_a)
            .expect("first hash transcript")[0]
            .deltas,
        vec![delta_a]
    );
    assert_eq!(
        transcripts
            .get(&call_hash_b)
            .expect("second hash transcript")[0]
            .deltas,
        vec![delta_b]
    );
}

#[test]
fn detached_asset_transfer_matches_sequential_transcript_and_events() {
    use crate::smartcontracts::Execute as _;
    use iroha_data_model::isi::Transfer;

    fn build_transfer_world(receiver_asset_balance: Option<u32>) -> (World, AssetId, AssetId) {
        let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob_account = Account::new(BOB_ID.clone()).build(&BOB_ID);
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "rose".parse().expect("asset name"),
        );
        let asset_definition = {
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "rose".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
        }
        .build(&ALICE_ID);
        let alice_asset_id = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
        let bob_asset_id = AssetId::new(asset_definition_id, BOB_ID.clone());
        let alice_asset = Asset::new(alice_asset_id.clone(), Quantity::from(10_u32));
        let mut assets = vec![alice_asset];
        if let Some(balance) = receiver_asset_balance {
            assets.push(Asset::new(bob_asset_id.clone(), Quantity::from(balance)));
        }
        let world = World::with_assets(
            [domain],
            [alice_account, bob_account],
            [asset_definition],
            assets,
            [],
        );
        (world, alice_asset_id, bob_asset_id)
    }

    fn balance(state: &State, asset_id: &AssetId) -> Quantity {
        state
            .view()
            .world()
            .assets()
            .get(asset_id)
            .map(|value| value.as_ref().clone())
            .unwrap_or_else(Quantity::zero)
    }

    let call_hash = iroha_crypto::Hash::prehashed([7_u8; iroha_crypto::Hash::LENGTH]);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);

    let (world_seq, alice_asset_id, bob_asset_id) = build_transfer_world(None);
    let kura_seq = Kura::blank_kura_for_testing();
    let query_seq = crate::query::store::LiveQueryStore::start_test();
    let state_seq = State::new(world_seq, Arc::clone(&kura_seq), query_seq);
    let mut block_seq = state_seq.block(header);
    {
        let mut tx = block_seq.transaction();
        tx.tx_call_hash = Some(call_hash);
        Transfer::asset_quantity(alice_asset_id.clone(), 3_u32, BOB_ID.clone())
            .execute(&ALICE_ID, &mut tx)
            .expect("sequential transfer");
        tx.apply();
    }
    let events_seq = block_seq.world.take_external_events();
    let transcripts_seq = block_seq.drain_transfer_transcripts();
    block_seq.commit().expect("commit sequential block");

    let (world_det, _, _) = build_transfer_world(None);
    let kura_det = Kura::blank_kura_for_testing();
    let query_det = crate::query::store::LiveQueryStore::start_test();
    let state_det = State::new(world_det, Arc::clone(&kura_det), query_det);
    let mut block_det = state_det.block(header);
    let instruction: InstructionBox =
        Transfer::asset_quantity(alice_asset_id.clone(), 3_u32, BOB_ID.clone()).into();
    let mut delta = DetachedStateTransactionDelta::default();
    crate::executor::execute_instruction_detached(&ALICE_ID, &instruction, &mut delta)
        .expect("detached transfer should be recorded");
    let delta_for_existing_transaction = delta.clone();
    delta
        .merge_into_with_context(
            &mut block_det,
            &ALICE_ID,
            DetachedMergeContext {
                tx_call_hash: Some(call_hash),
                current_tx_hash: None,
                current_lane_id: None,
                current_dataspace_id: None,
            },
        )
        .expect("detached transfer merge");
    let events_det = block_det.world.take_external_events();
    let transcripts_det = block_det.drain_transfer_transcripts();
    block_det.commit().expect("commit detached block");

    let (world_existing_tx, _, _) = build_transfer_world(None);
    let kura_existing_tx = Kura::blank_kura_for_testing();
    let query_existing_tx = crate::query::store::LiveQueryStore::start_test();
    let state_existing_tx = State::new(
        world_existing_tx,
        Arc::clone(&kura_existing_tx),
        query_existing_tx,
    );
    let mut block_existing_tx = state_existing_tx.block(header);
    {
        let mut tx = block_existing_tx.transaction();
        tx.tx_call_hash = Some(call_hash);
        delta_for_existing_transaction
            .merge_single_transfer_into_transaction(&mut tx, &ALICE_ID)
            .expect("detached delta should be a single transfer")
            .expect("detached transfer merge into existing transaction");
        tx.apply();
    }
    let events_existing_tx = block_existing_tx.world.take_external_events();
    let transcripts_existing_tx = block_existing_tx.drain_transfer_transcripts();
    block_existing_tx
        .commit()
        .expect("commit existing-transaction detached block");

    let second_call_hash = iroha_crypto::Hash::prehashed([8_u8; iroha_crypto::Hash::LENGTH]);
    let (world_batch, _, _) = build_transfer_world(None);
    let kura_batch = Kura::blank_kura_for_testing();
    let query_batch = crate::query::store::LiveQueryStore::start_test();
    let state_batch = State::new(world_batch, Arc::clone(&kura_batch), query_batch);
    let mut block_batch = state_batch.block(header);
    let mut first_delta = DetachedStateTransactionDelta::default();
    let first_instruction: InstructionBox =
        Transfer::asset_quantity(alice_asset_id.clone(), 3_u32, BOB_ID.clone()).into();
    crate::executor::execute_instruction_detached(&ALICE_ID, &first_instruction, &mut first_delta)
        .expect("first detached transfer should be recorded");
    let mut second_delta = DetachedStateTransactionDelta::default();
    let second_instruction: InstructionBox =
        Transfer::asset_quantity(alice_asset_id.clone(), 2_u32, BOB_ID.clone()).into();
    crate::executor::execute_instruction_detached(
        &ALICE_ID,
        &second_instruction,
        &mut second_delta,
    )
    .expect("second detached transfer should be recorded");
    {
        let mut tx = block_batch.transaction();
        tx.tx_call_hash = Some(call_hash);
        first_delta
            .merge_numeric_transfer_batch_into_transaction(&mut tx, &ALICE_ID)
            .expect("first delta should be a single transfer")
            .expect("first batch transfer merge");
        tx.tx_call_hash = Some(second_call_hash);
        second_delta
            .merge_numeric_transfer_batch_into_transaction(&mut tx, &ALICE_ID)
            .expect("second delta should be a single transfer")
            .expect("second batch transfer merge");
        tx.tx_call_hash = None;
        tx.apply();
    }
    block_batch.add_committed_fragments(1);
    assert_eq!(block_batch.committed_fragment_count(), 2);
    let events_batch = block_batch.world.take_external_events();
    let transcripts_batch = block_batch.drain_transfer_transcripts();
    block_batch.commit().expect("commit batch detached block");

    assert_eq!(events_seq, events_det);
    assert_eq!(events_seq, events_existing_tx);
    assert_eq!(transcripts_seq, transcripts_det);
    assert_eq!(transcripts_seq, transcripts_existing_tx);
    assert_eq!(
        transcripts_seq.get(&call_hash),
        transcripts_batch.get(&call_hash)
    );
    assert!(
        transcripts_batch.contains_key(&second_call_hash),
        "second batch transfer should keep its own transaction call hash"
    );
    assert!(
        events_batch.len() > events_seq.len(),
        "two-transfer batch should preserve both transfers' event stream"
    );

    let (world_batch_guard, _, _) = build_transfer_world(Some(0));
    let kura_batch_guard = Kura::blank_kura_for_testing();
    let query_batch_guard = crate::query::store::LiveQueryStore::start_test();
    let state_batch_guard = State::new(
        world_batch_guard,
        Arc::clone(&kura_batch_guard),
        query_batch_guard,
    );
    let mut block_batch_guard = state_batch_guard.block(header);
    let mut batch_guard_tx = block_batch_guard.transaction();
    assert!(
        first_delta.supports_numeric_transfer_batch_merge(&batch_guard_tx),
        "preseeded receiver assets without matching data triggers should batch"
    );

    let executable = iroha_data_model::transaction::Executable::Instructions(
        iroha_primitives::const_vec::ConstVec::from(Vec::<InstructionBox>::new()),
    );
    let nonmatching_action = crate::smartcontracts::triggers::specialized::SpecializedAction::new(
        executable.clone(),
        Repeats::Indefinitely,
        ALICE_ID.clone(),
        data_pre::DataEventFilter::Configuration(data_pre::ConfigurationEventFilter::new()),
    )
    .expect("test data-trigger action satisfies its authority invariant");
    batch_guard_tx
        .world
        .triggers
        .add_data_trigger(
            crate::smartcontracts::triggers::specialized::SpecializedTrigger::new(
                "nonmatching_transfer_batch_guard"
                    .parse()
                    .expect("trigger id"),
                nonmatching_action,
            ),
        )
        .expect("add nonmatching data trigger");
    assert!(
        first_delta.supports_numeric_transfer_batch_merge(&batch_guard_tx),
        "unrelated data triggers should not disable transfer batching"
    );

    let matching_action = crate::smartcontracts::triggers::specialized::SpecializedAction::new(
        executable,
        Repeats::Indefinitely,
        ALICE_ID.clone(),
        data_pre::DataEventFilter::Asset(data_pre::AssetEventFilter::new()),
    )
    .expect("test data-trigger action satisfies its authority invariant");
    batch_guard_tx
        .world
        .triggers
        .add_data_trigger(
            crate::smartcontracts::triggers::specialized::SpecializedTrigger::new(
                "matching_transfer_batch_guard".parse().expect("trigger id"),
                matching_action,
            ),
        )
        .expect("add matching data trigger");
    assert!(
        !first_delta.supports_numeric_transfer_batch_merge(&batch_guard_tx),
        "matching asset data triggers should keep per-transaction semantics"
    );
    drop(batch_guard_tx);

    assert_eq!(balance(&state_det, &alice_asset_id), Quantity::from(7_u32));
    assert_eq!(balance(&state_det, &bob_asset_id), Quantity::from(3_u32));
    assert_eq!(
        balance(&state_existing_tx, &alice_asset_id),
        Quantity::from(7_u32)
    );
    assert_eq!(
        balance(&state_existing_tx, &bob_asset_id),
        Quantity::from(3_u32)
    );
    assert_eq!(
        balance(&state_batch, &alice_asset_id),
        Quantity::from(5_u32)
    );
    assert_eq!(balance(&state_batch, &bob_asset_id), Quantity::from(5_u32));
    assert_eq!(
        balance(&state_seq, &alice_asset_id),
        balance(&state_det, &alice_asset_id)
    );
    assert_eq!(
        balance(&state_seq, &bob_asset_id),
        balance(&state_det, &bob_asset_id)
    );
    assert_eq!(
        balance(&state_seq, &alice_asset_id),
        balance(&state_existing_tx, &alice_asset_id)
    );
    assert_eq!(
        balance(&state_seq, &bob_asset_id),
        balance(&state_existing_tx, &bob_asset_id)
    );
    let transcript = transcripts_det
        .get(&call_hash)
        .and_then(|entry| entry.first())
        .expect("detached transfer transcript recorded");
    let transfer_delta = transcript
        .deltas
        .first()
        .expect("single transfer delta recorded");
    assert_eq!(transfer_delta.from_balance_before, Quantity::from(10_u32));
    assert_eq!(transfer_delta.from_balance_after, Quantity::from(7_u32));
    assert_eq!(transfer_delta.to_balance_before, Quantity::zero());
    assert_eq!(transfer_delta.to_balance_after, Quantity::from(3_u32));
    assert!(transcript.poseidon_preimage_digest.is_some());
}

#[test]
fn detached_multi_transfer_honors_exact_delegation_like_sequential_execution() {
    use crate::smartcontracts::Execute as _;
    use iroha_data_model::isi::Transfer;

    fn build_transfer_world(grant_definition: bool) -> (World, AssetId, AssetId) {
        let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob_account = Account::new(BOB_ID.clone()).build(&BOB_ID);
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "rose".parse().expect("asset name"),
        );
        let asset_definition = {
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "rose".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
        }
        .build(&ALICE_ID);
        let source_id = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
        let destination_id = AssetId::new(asset_definition_id.clone(), BOB_ID.clone());
        let mut world = World::with_assets(
            [domain],
            [alice_account, bob_account],
            [asset_definition],
            [Asset::new(source_id.clone(), Quantity::from(10_u32))],
            [],
        );
        let permission = if grant_definition {
            iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition {
                asset_definition: asset_definition_id,
            }
            .into()
        } else {
            iroha_executor_data_model::permission::asset::CanTransferAsset {
                asset: source_id.clone(),
            }
            .into()
        };
        let mut permissions = Permissions::new();
        assert!(permissions.insert(permission));
        world
            .account_permissions_mut_for_testing()
            .insert(BOB_ID.clone(), permissions);
        (world, source_id, destination_id)
    }

    fn balance(state: &State, asset_id: &AssetId) -> Quantity {
        state
            .view()
            .world()
            .assets()
            .get(asset_id)
            .map(|value| value.as_ref().clone())
            .unwrap_or_else(Quantity::zero)
    }

    let call_hash = iroha_crypto::Hash::prehashed([0xA4; iroha_crypto::Hash::LENGTH]);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);

    for grant_definition in [false, true] {
        let (sequential_world, source_id, destination_id) = build_transfer_world(grant_definition);
        let sequential_state = State::new(
            sequential_world,
            Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let mut sequential_block = sequential_state.block(header);
        {
            let mut transaction = sequential_block.transaction();
            transaction.tx_call_hash = Some(call_hash);
            for amount in [2_u32, 3_u32] {
                Transfer::asset_quantity(source_id.clone(), amount, BOB_ID.clone())
                    .execute(&BOB_ID, &mut transaction)
                    .expect("exact delegated permission must authorize sequential transfer");
            }
            transaction.apply();
        }
        let sequential_events = sequential_block.world.take_external_events();
        let sequential_transcripts = sequential_block.drain_transfer_transcripts();
        sequential_block
            .commit()
            .expect("commit sequential delegated transfers");

        let (detached_world, _, _) = build_transfer_world(grant_definition);
        let detached_state = State::new(
            detached_world,
            Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let mut detached_block = detached_state.block(header);
        let mut delta = DetachedStateTransactionDelta::default();
        delta.transfer_asset(source_id.clone(), BOB_ID.clone(), Quantity::from(2_u32));
        delta.transfer_asset(source_id.clone(), BOB_ID.clone(), Quantity::from(3_u32));
        assert!(
            delta.single_transfer_delta().is_none(),
            "the regression must exercise the general multi-operation merge path"
        );
        delta
            .merge_into_with_context(
                &mut detached_block,
                &BOB_ID,
                DetachedMergeContext {
                    tx_call_hash: Some(call_hash),
                    ..DetachedMergeContext::default()
                },
            )
            .expect("exact delegated permission must authorize detached multi-transfer merge");
        let detached_events = detached_block.world.take_external_events();
        let detached_transcripts = detached_block.drain_transfer_transcripts();
        detached_block
            .commit()
            .expect("commit detached delegated transfers");

        assert_eq!(
            balance(&detached_state, &source_id),
            balance(&sequential_state, &source_id)
        );
        assert_eq!(
            balance(&detached_state, &destination_id),
            balance(&sequential_state, &destination_id)
        );
        assert_eq!(detached_events, sequential_events);
        assert_eq!(detached_transcripts, sequential_transcripts);
    }
}
