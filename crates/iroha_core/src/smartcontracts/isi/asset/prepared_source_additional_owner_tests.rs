// Source-occurrence tests live inside the private asset ISI owner boundary.

use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, StateTransaction, World},
};
use iroha_data_model::block::BlockHeader;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use nonzero_ext::nonzero;

fn wonderland_domain_id() -> DomainId {
    DomainId::try_new("wonderland", "universal").unwrap()
}

fn wonderland_asset_definition_id(name: &str) -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(wonderland_domain_id(), name.parse().unwrap())
}

fn build_numeric_asset_definition(
    id: &AssetDefinitionId,
    name: &str,
    owner: &AccountId,
) -> AssetDefinition {
    AssetDefinition::numeric(
        id.clone(),
        name.to_owned(),
        AssetBalancePolicy::Global,
        None,
    )
    .build(owner)
}

fn asset_route_test_state(world: World) -> State {
    State::new(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn build_asset_transfer_control_test_state(balance: u32) -> (State, AssetDefinitionId, AssetId) {
    let definition = wonderland_asset_definition_id("rose");
    let source = AssetId::new(definition.clone(), ALICE_ID.clone());
    let world = World::with_assets(
        [Domain::new(wonderland_domain_id()).build(&ALICE_ID)],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&ALICE_ID),
        ],
        [build_numeric_asset_definition(
            &definition,
            "rose",
            &ALICE_ID,
        )],
        [Asset::new(source.clone(), Quantity::from(balance))],
        [],
    );
    (asset_route_test_state(world), definition, source)
}

fn asset_balance_or_zero(tx: &StateTransaction<'_, '_>, id: &AssetId) -> Quantity {
    tx.world
        .assets
        .get(id)
        .map(|asset| asset.as_ref().clone())
        .unwrap_or_else(Quantity::zero)
}

fn occurrence_header() -> BlockHeader {
    BlockHeader::new(nonzero!(1_u64), None, None, None, 86_400_000, 0)
}

fn expected_occurrence(
    hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
) -> iroha_data_model::fastpq::TransferTranscript {
    iroha_data_model::fastpq::TransferTranscript {
        batch_hash: hash,
        authority_digest: crate::fastpq::authority_digest(&ALICE_ID),
        poseidon_preimage_digest: match deltas.as_slice() {
            [delta] => Some(crate::fastpq::poseidon_preimage_digest(delta, &hash)),
            _ => None,
        },
        deltas,
    }
}

#[test]
fn aggregate_batch_preserves_one_ordered_occurrence_for_repeated_and_self_legs() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, source) = build_asset_transfer_control_test_state(10);
    let destination = AssetId::new(definition, BOB_ID.clone());
    let mut block = state.block(occurrence_header());
    let mut tx = block.transaction();
    let hash = Hash::new(b"prepared aggregate occurrence");
    tx.tx_call_hash = Some(hash);
    let batch = PreparedNumericAssetMovementBatch::prepare_user(
        &mut tx,
        &ALICE_ID,
        &[
            (source.clone(), destination.clone(), Quantity::from(3_u32)),
            (source.clone(), destination.clone(), Quantity::from(2_u32)),
            (source.clone(), source.clone(), Quantity::one()),
        ],
    )
    .unwrap();
    let deltas = batch
        .plans
        .iter()
        .map(|plan| plan.prechecked_delta.clone())
        .collect::<Vec<_>>();
    assert_eq!(deltas[1].from_balance_before, Quantity::from(7_u32));
    assert_eq!(deltas[1].to_balance_before, Quantity::from(3_u32));
    assert_eq!(deltas[2].from_balance_before, Quantity::from(5_u32));
    assert_eq!(deltas[2].to_balance_after, Quantity::from(5_u32));
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
    let applied = batch.apply(&mut tx).unwrap();
    assert_eq!(applied.len(), 3);
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 1);
    assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::from(5_u32));
    assert_eq!(
        asset_balance_or_zero(&tx, &destination),
        Quantity::from(5_u32)
    );
    tx.apply();
    let transcripts = block.drain_transfer_transcripts();
    let expected = expected_occurrence(hash, deltas);
    assert_eq!(transcripts[&hash], vec![expected.clone()]);
    assert_eq!(expected.poseidon_preimage_digest, None);
    assert_eq!(
        norito::encode_canonical(&transcripts[&hash][0]).unwrap(),
        norito::encode_canonical(&expected).unwrap()
    );
    assert!(!block.captured_fastpq_transcript_sources().unwrap()[&hash].is_protocol_purpose());
}

#[test]
fn native_batch_keeps_typed_purpose_and_finalizes_a_single_leg() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, source) = build_asset_transfer_control_test_state(10);
    let destination = AssetId::new(definition, BOB_ID.clone());
    let mut block = state.block(occurrence_header());
    let mut tx = block.transaction();
    assert!(tx.tx_call_hash.is_none());
    let authorization = NumericAssetMovementAuthorization::bilateral(
        &ALICE_ID,
        "prepared-test-native-batch",
        vec![0x53],
    );
    let bindings = vec![(source, destination, Quantity::from(3_u32))];
    let hash = authorization
        .resolve_transcript_identity(&tx, &bindings)
        .unwrap();
    let batch = PreparedNumericAssetMovementBatch::prepare_with_authorization(
        &mut tx,
        &bindings,
        authorization,
    )
    .unwrap();
    let expected = expected_occurrence(hash, vec![batch.plans[0].prechecked_delta.clone()]);
    batch.apply(&mut tx).unwrap();
    assert!(tx.tx_call_hash.is_none());
    tx.apply();
    assert_eq!(block.drain_transfer_transcripts()[&hash], vec![expected]);
    assert!(block.captured_fastpq_transcript_sources().unwrap()[&hash].is_protocol_purpose());
}

#[test]
fn stale_aggregate_batch_rejects_before_balance_and_occurrence_writes() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, source) = build_asset_transfer_control_test_state(10);
    let destination = AssetId::new(definition, BOB_ID.clone());
    let mut block = state.block(occurrence_header());
    let mut tx = block.transaction();
    tx.tx_call_hash = Some(Hash::new(b"stale aggregate occurrence"));
    let batch = PreparedNumericAssetMovementBatch::prepare_user(
        &mut tx,
        &ALICE_ID,
        &[(source.clone(), destination.clone(), Quantity::from(3_u32))],
    )
    .unwrap();
    **tx.world.assets.get_mut(&source).unwrap() = Quantity::from(9_u32);
    let events_before = tx.world.internal_event_buf.len();
    let error = match batch.apply(&mut tx) {
        Ok(_) => panic!("stale aggregate batch must fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("balance changed"));
    assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::from(9_u32));
    assert_eq!(asset_balance_or_zero(&tx, &destination), Quantity::zero());
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
    assert_eq!(tx.world.internal_event_buf.len(), events_before);
}

fn pair_state() -> (State, [AssetId; 4]) {
    let domain = wonderland_domain_id();
    let first = wonderland_asset_definition_id("rose");
    let second = wonderland_asset_definition_id("lily");
    let ids = [
        AssetId::new(first.clone(), ALICE_ID.clone()),
        AssetId::new(first.clone(), BOB_ID.clone()),
        AssetId::new(second.clone(), ALICE_ID.clone()),
        AssetId::new(second.clone(), BOB_ID.clone()),
    ];
    let world = World::with_assets(
        [Domain::new(domain).build(&ALICE_ID)],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&ALICE_ID),
        ],
        [
            build_numeric_asset_definition(&first, "rose", &ALICE_ID),
            build_numeric_asset_definition(&second, "lily", &ALICE_ID),
        ],
        [
            Asset::new(ids[0].clone(), Quantity::from(10_u32)),
            Asset::new(ids[2].clone(), Quantity::from(20_u32)),
        ],
        [],
    );
    (asset_route_test_state(world), ids)
}

fn prepared_pair(
    tx: &mut StateTransaction<'_, '_>,
    ids: &[AssetId; 4],
) -> PreparedNumericTransferPair {
    prepare_authorized_numeric_asset_pair(
        tx,
        &ALICE_ID,
        ids[0].clone(),
        ids[1].clone(),
        Quantity::from(3_u32),
        ids[2].clone(),
        ids[3].clone(),
        Quantity::from(7_u32),
    )
    .unwrap()
}

#[test]
fn native_fx_apply_boundary_keeps_pair_order_and_one_multi_delta_occurrence() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, ids) = pair_state();
    let mut block = state.block(occurrence_header());
    let mut tx = block.transaction();
    let hash = Hash::new(b"prepared FX pair application");
    tx.tx_call_hash = Some(hash);
    let pair = prepared_pair(&mut tx, &ids);
    let expected = expected_occurrence(
        hash,
        vec![
            pair.source.prechecked_delta.clone(),
            pair.destination.prechecked_delta.clone(),
        ],
    );
    let (first, second) = pair
        .apply_with_transcript(&mut tx, &ALICE_ID, hash)
        .unwrap();
    assert_eq!(first.delta, expected.deltas[0]);
    assert_eq!(second.delta, expected.deltas[1]);
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 1);
    for (id, amount) in ids.iter().zip([7_u32, 3, 13, 7]) {
        assert_eq!(asset_balance_or_zero(&tx, id), Quantity::from(amount));
    }
    tx.apply();
    assert_eq!(block.drain_transfer_transcripts()[&hash], vec![expected]);
}

#[test]
fn second_pair_apply_error_stages_nothing_and_parent_rollback_remains_required() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, ids) = pair_state();
    let mut block = state.block(occurrence_header());
    {
        let mut tx = block.transaction();
        let hash = Hash::new(b"stale second FX pair leg");
        tx.tx_call_hash = Some(hash);
        let pair = prepared_pair(&mut tx, &ids);
        **tx.world.assets.get_mut(&ids[2]).unwrap() = Quantity::from(19_u32);
        let error = match pair.apply_with_transcript(&mut tx, &ALICE_ID, hash) {
            Ok(_) => panic!("stale second leg must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("balance changed"));
        assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
        // The occurrence helper does not invent a nested world rollback: the first
        // existing leg already applied, and the enclosing transaction must be discarded.
        assert_eq!(asset_balance_or_zero(&tx, &ids[0]), Quantity::from(7_u32));
    }
    assert!(block.drain_transfer_transcripts().is_empty());
    assert!(
        block
            .captured_fastpq_transcript_sources()
            .unwrap()
            .is_empty()
    );
    let tx = block.transaction();
    assert_eq!(asset_balance_or_zero(&tx, &ids[0]), Quantity::from(10_u32));
    assert_eq!(asset_balance_or_zero(&tx, &ids[2]), Quantity::from(20_u32));
}

fn prepared_sccp_release(
    tx: &mut StateTransaction<'_, '_>,
    source: &AssetId,
    destination: &AssetId,
    amount: u32,
) -> PreparedSccpInboundNumericAssetRelease {
    use iroha_data_model::bridge::{
        SccpLaneIdV1, SccpNetworkV1, SccpRouteKeyV1, SccpRouteLiabilityV1,
    };
    let route_key = SccpRouteKeyV1::new(
        SccpLaneIdV1 {
            source: SccpNetworkV1::EthereumMainnet,
            target: SccpNetworkV1::SoraTaira,
        },
        "prepared-occurrence-route".to_owned(),
        "prepared-occurrence-asset".to_owned(),
        1,
    )
    .unwrap();
    let liability_before = SccpRouteLiabilityV1::new(10).unwrap();
    let liability_after = liability_before.checked_debit(u128::from(amount)).unwrap();
    tx.world
        .sccp_route_liabilities
        .insert(route_key.clone(), liability_before);
    let amount = Quantity::from(amount);
    let delta = tx
        .world
        .precheck_numeric_asset_transfer_delta_exact(source, destination, &amount)
        .unwrap();
    PreparedSccpInboundNumericAssetRelease {
        route_key,
        source_id: source.clone(),
        destination_id: destination.clone(),
        amount,
        liability_before,
        liability_after,
        expected_escrow_balance_after: delta.from_balance_after.clone(),
        control_update: None,
        delta,
    }
}

#[test]
fn sccp_apply_preserves_exact_singleton_and_liability_update_or_removal() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    for amount in [3_u32, 10] {
        let (state, definition, source) = build_asset_transfer_control_test_state(10);
        let destination = AssetId::new(definition, BOB_ID.clone());
        let mut block = state.block(occurrence_header());
        let mut tx = block.transaction();
        let hash = Hash::new(amount.to_le_bytes());
        tx.tx_call_hash = Some(hash);
        let prepared = prepared_sccp_release(&mut tx, &source, &destination, amount);
        let expected = expected_occurrence(hash, vec![prepared.delta.clone()]);
        let route = prepared.route_key.clone();
        let liability_after = prepared.liability_after;
        apply_prepared_sccp_inbound_numeric_asset_release(&mut tx, &ALICE_ID, prepared).unwrap();
        assert_eq!(
            tx.world.sccp_route_liabilities.get(&route),
            liability_after.as_ref()
        );
        assert_eq!(
            asset_balance_or_zero(&tx, &source),
            Quantity::from(10 - amount)
        );
        assert_eq!(
            asset_balance_or_zero(&tx, &destination),
            Quantity::from(amount)
        );
        assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 1);
        tx.apply();
        assert_eq!(block.drain_transfer_transcripts()[&hash], vec![expected]);
    }
}

#[test]
fn sccp_missing_identity_rejects_before_release_balance_and_liability_writes() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, source) = build_asset_transfer_control_test_state(10);
    let destination = AssetId::new(definition, BOB_ID.clone());
    let mut block = state.block(occurrence_header());
    let mut tx = block.transaction();
    let prepared = prepared_sccp_release(&mut tx, &source, &destination, 3);
    let route = prepared.route_key.clone();
    let liability_before = prepared.liability_before;
    let events_before = tx.world.internal_event_buf.len();
    let error = apply_prepared_sccp_inbound_numeric_asset_release(&mut tx, &ALICE_ID, prepared)
        .unwrap_err();
    assert!(error.to_string().contains("call_hash"));
    assert_eq!(
        tx.world.sccp_route_liabilities.get(&route),
        Some(&liability_before)
    );
    assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::from(10_u32));
    assert_eq!(asset_balance_or_zero(&tx, &destination), Quantity::zero());
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
    assert_eq!(tx.world.internal_event_buf.len(), events_before);
}

#[test]
fn sccp_callback_failure_stages_no_occurrence_and_drops_with_the_transaction() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, source) = build_asset_transfer_control_test_state(10);
    let destination = AssetId::new(definition, BOB_ID.clone());
    let mut block = state.block(occurrence_header());
    {
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(Hash::new(b"SCCP release callback error"));
        let mut prepared = prepared_sccp_release(&mut tx, &source, &destination, 3);
        prepared.expected_escrow_balance_after = Quantity::from(8_u32);
        let error = apply_prepared_sccp_inbound_numeric_asset_release(&mut tx, &ALICE_ID, prepared)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("not fully backed after inbound release")
        );
        assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
        assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::from(7_u32));
    }
    assert!(block.drain_transfer_transcripts().is_empty());
    assert!(
        block
            .captured_fastpq_transcript_sources()
            .unwrap()
            .is_empty()
    );
    let tx = block.transaction();
    assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::from(10_u32));
}

#[test]
fn optimized_detached_merges_keep_each_prepared_call_after_final_hash_clear() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, source) = build_asset_transfer_control_test_state(10);
    let destination = AssetId::new(definition, BOB_ID.clone());
    let mut block = state.block(occurrence_header());
    let mut tx = block.transaction();
    let hashes = [
        Hash::new(b"optimized prepared first"),
        Hash::new(b"optimized prepared second"),
    ];
    let mut expected = Vec::new();
    for (hash, amount) in hashes.into_iter().zip([3_u32, 2]) {
        tx.tx_call_hash = Some(hash);
        let amount = Quantity::from(amount);
        let delta = tx
            .world
            .precheck_numeric_asset_transfer_delta_exact(&source, &destination, &amount)
            .unwrap();
        expected.push(expected_occurrence(hash, vec![delta]));
        assert!(
            execute_batch_merge_eligible_user_numeric_asset_transfer(
                &mut tx,
                &ALICE_ID,
                source.clone(),
                BOB_ID.clone(),
                amount
            )
            .unwrap()
        );
    }
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 2);
    assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::from(5_u32));
    tx.tx_call_hash = None;
    tx.apply();
    let transcripts = block.drain_transfer_transcripts();
    let sources = block.captured_fastpq_transcript_sources().unwrap();
    for occurrence in expected {
        let hash = occurrence.batch_hash;
        assert_eq!(transcripts[&hash], vec![occurrence]);
        assert!(!sources[&hash].is_protocol_purpose());
    }
}

#[test]
fn optimized_detached_fallbacks_preserve_balances_events_and_occurrences() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, source) = build_asset_transfer_control_test_state(10);
    let destination = AssetId::new(definition.clone(), BOB_ID.clone());
    let mut block = state.block(occurrence_header());
    let mut tx = block.transaction();
    let events_before = tx.world.internal_event_buf.len();
    assert!(
        !execute_batch_merge_eligible_user_numeric_asset_transfer(
            &mut tx,
            &ALICE_ID,
            source.clone(),
            iroha_test_samples::CARPENTER_ID.clone(),
            Quantity::one()
        )
        .unwrap()
    );
    assert_eq!(tx.world.internal_event_buf.len(), events_before);
    SetAssetTransferControl::new(
        ALICE_ID.clone(),
        definition,
        vec![AssetTransferLimit {
            window: AssetTransferControlWindow::Day,
            cap_amount: Some(Quantity::from(10_u32)),
        }],
    )
    .execute(&ALICE_ID, &mut tx)
    .unwrap();
    let events_before = tx.world.internal_event_buf.len();
    assert!(
        !execute_batch_merge_eligible_user_numeric_asset_transfer(
            &mut tx,
            &ALICE_ID,
            source.clone(),
            BOB_ID.clone(),
            Quantity::one()
        )
        .unwrap()
    );
    assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::from(10_u32));
    assert_eq!(asset_balance_or_zero(&tx, &destination), Quantity::zero());
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
    assert_eq!(tx.world.internal_event_buf.len(), events_before);
}

#[test]
fn optimized_detached_missing_identity_rejects_before_balance_and_event_writes() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, source) = build_asset_transfer_control_test_state(10);
    let destination = AssetId::new(definition, BOB_ID.clone());
    let mut block = state.block(occurrence_header());
    let mut tx = block.transaction();
    let events_before = tx.world.internal_event_buf.len();
    let error = execute_batch_merge_eligible_user_numeric_asset_transfer(
        &mut tx,
        &ALICE_ID,
        source.clone(),
        BOB_ID.clone(),
        Quantity::one(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("call_hash"));
    assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::from(10_u32));
    assert_eq!(asset_balance_or_zero(&tx, &destination), Quantity::zero());
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
    assert_eq!(tx.world.internal_event_buf.len(), events_before);
}
