use super::*;
use crate::kura::Kura;
use iroha_data_model::{block::BlockHeader, nexus::LaneConfig as LaneConfigModel};
use nonzero_ext::nonzero;
use std::{
    sync::{Arc, Barrier, mpsc},
    thread,
    time::{Duration, Instant},
};
#[test]
fn state_commit_does_not_hold_tiered_backend_while_waiting_for_state_write_lock() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::default(), kura, query));
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let _write_guard = state.state_write_lock.lock();
    let barrier = Arc::new(Barrier::new(2));
    let commit_state = Arc::clone(&state);
    let commit_barrier = Arc::clone(&barrier);
    let handle = thread::spawn(move || {
        commit_barrier.wait();
        let block = commit_state.block(header);
        block
            .commit_empty_block_for_testing()
            .expect("commit should succeed");
    });
    barrier.wait();
    let start = Instant::now();
    let mut locked_while_waiting = false;
    while start.elapsed() < Duration::from_millis(200) {
        if handle.is_finished() {
            break;
        }
        if state.tiered_backend.try_lock().is_none() {
            locked_while_waiting = true;
            break;
        }
        thread::yield_now();
    }
    assert!(
        !locked_while_waiting,
        "tiered backend locked while commit waits for state_write_lock"
    );
    drop(_write_guard);
    handle.join().expect("commit thread");
}
#[test]
fn lane_lifecycle_and_commit_do_not_deadlock_on_lock_order() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::default(), kura, query));

    let plan = iroha_data_model::nexus::LaneLifecyclePlan {
        additions: vec![LaneConfigModel {
            id: LaneId::new(1),
            alias: "beta".to_string(),
            ..LaneConfigModel::default()
        }],
        retire: Vec::new(),
    };
    let (done_tx, done_rx) = mpsc::channel();
    let barrier = Arc::new(Barrier::new(3));
    let lane_state = Arc::clone(&state);
    let lane_done = done_tx.clone();
    let lane_barrier = Arc::clone(&barrier);
    let lane_handle = thread::spawn(move || {
        lane_barrier.wait();
        lane_state
            .apply_lane_lifecycle(&plan)
            .expect("lane lifecycle");
        let _ = lane_done.send(());
    });
    let commit_state = Arc::clone(&state);
    let commit_done = done_tx.clone();
    let commit_barrier = Arc::clone(&barrier);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let commit_handle = thread::spawn(move || {
        commit_barrier.wait();
        let block = commit_state.block(header);
        block.commit_empty_block_for_testing().expect("commit");
        let _ = commit_done.send(());
    });
    barrier.wait();
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("first serialized operation completion");
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("second serialized operation completion");
    lane_handle.join().expect("lane lifecycle thread");
    commit_handle.join().expect("commit thread");
    assert!(
        state
            .nexus_snapshot()
            .lane_catalog
            .by_alias("beta")
            .is_some(),
        "lane lifecycle should publish after serialization with commit"
    );
}
#[test]
fn lane_lifecycle_waits_for_prebuilt_runtime_without_holding_publication_fences() {
    // The channel handshakes establish ordering; the deadline only bounds a
    // deadlock under a heavily loaded test runner.
    let timeout = Duration::from_secs(30);
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::default(), kura, query));

    let plan = iroha_data_model::nexus::LaneLifecyclePlan {
        additions: vec![LaneConfigModel {
            id: LaneId::new(1),
            alias: "prebuilt-beta".to_string(),
            ..LaneConfigModel::default()
        }],
        retire: Vec::new(),
    };
    let (block_ready_tx, block_ready_rx) = mpsc::channel();
    let (commit_release_tx, commit_release_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let commit_state = Arc::clone(&state);
    let commit_done = done_tx.clone();
    let commit_handle = thread::spawn(move || {
        let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
        let block = commit_state.block(header);
        block_ready_tx
            .send(())
            .expect("notify prebuilt block is holding its overlay");
        commit_release_rx
            .recv()
            .expect("wait for the lifecycle runtime-writer probe");
        block
            .commit_empty_block_for_testing()
            .expect("commit prebuilt block");
        let _ = commit_done.send("commit");
    });
    block_ready_rx
        .recv_timeout(timeout)
        .expect("prebuilt block ready");
    let runtime_before = state.canonical_runtime.view().get().clone();
    let manifests_before = Arc::clone(&state.lane_manifests.read());
    let generation_before = state.state_view_generation();
    let (runtime_wait_tx, runtime_wait_rx) = mpsc::channel();
    let lifecycle_state = Arc::clone(&state);
    let lifecycle_done = done_tx.clone();
    let lifecycle_handle = thread::spawn(move || {
        canonical_runtime::observe_next_runtime_replacement_for_test(runtime_wait_tx);
        lifecycle_state
            .apply_lane_lifecycle(&plan)
            .expect("lane lifecycle");
        let _ = lifecycle_done.send("lifecycle");
    });
    runtime_wait_rx
        .recv_timeout(timeout)
        .expect("lifecycle reached its actual runtime-writer acquisition");
    // The original prebuilt block owns this writer, so the lifecycle cannot
    // pass this acquisition. It must leave every enclosing fence available to
    // that block's commit and must publish no partial runtime or manifest cut.
    assert!(state.state_commit_lock.try_lock().is_some());
    assert!(state.lane_lifecycle_lock.try_lock().is_some());
    assert!(state.state_write_lock.try_lock().is_some());
    assert_eq!(state.canonical_runtime.view().get(), &runtime_before);
    assert!(Arc::ptr_eq(&state.lane_manifests.read(), &manifests_before));
    assert_eq!(state.state_view_generation(), generation_before);
    assert!(
        state
            .nexus_snapshot()
            .lane_catalog
            .by_alias("prebuilt-beta")
            .is_none(),
        "catalog must remain at the captured predecessor until its writer releases"
    );
    commit_release_tx
        .send(())
        .expect("release prebuilt block commit");
    done_rx
        .recv_timeout(timeout)
        .expect("first operation completion");
    done_rx
        .recv_timeout(timeout)
        .expect("second operation completion");
    lifecycle_handle.join().expect("lane lifecycle thread");
    commit_handle.join().expect("commit thread");
    assert!(
        state
            .nexus_snapshot()
            .lane_catalog
            .by_alias("prebuilt-beta")
            .is_some(),
        "published lane should survive prebuilt block serialization"
    );
}
#[test]
fn transaction_and_state_views_keep_canonical_catalog_after_projection_cache_drift() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);

    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let block_catalog = block.nexus.lane_catalog.clone();
    let updated_catalog = iroha_data_model::nexus::LaneCatalog::new(
        nonzero!(2_u32),
        vec![
            LaneConfigModel::default(),
            LaneConfigModel {
                id: LaneId::new(1),
                alias: "post-block-beta".to_owned(),
                ..LaneConfigModel::default()
            },
        ],
    )
    .expect("updated lane catalog");
    {
        let mut nexus = state.nexus.write();
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&updated_catalog);
        nexus.lane_catalog = updated_catalog;
    }
    assert!(
        state
            .nexus
            .read()
            .lane_catalog
            .by_alias("post-block-beta")
            .is_some()
    );
    let canonical = state.nexus_snapshot();
    assert_eq!(canonical.lane_catalog, block_catalog);
    assert!(
        canonical.lane_catalog.by_alias("post-block-beta").is_none(),
        "derived cache drift must not replace the canonical State catalog"
    );
    let tx = block.transaction();
    assert_eq!(tx.nexus.lane_catalog, block_catalog);
    assert!(
        tx.nexus.lane_catalog.by_alias("post-block-beta").is_none(),
        "transactions opened from a prebuilt block must retain their canonical catalog"
    );
}
// Structural SCCP fixtures exercise metadata refusal without constructing finality.
// Verify the exact bridge error, the propagated typed failure, and both staged
// and committed State identity; merely receiving an unrelated error is insufficient.
#[inline(never)]
fn assert_sccp_apply_refusal(
    committed: &crate::block::CommittedBlock,
    expected: crate::bridge::SccpCommittedBlockValidationError,
) {
    assert_eq!(
        crate::bridge::validate_sccp_commitment_root_for_signed_block(committed.as_ref()),
        Err(expected.clone()),
    );
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    let committed_before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let mut state_block = state.block(committed.as_ref().header());
    let staged_before = crate::snapshot::canonical_staged_state_snapshot_hash(&state_block);
    let (events, result) = state_block.apply_without_execution_inner(
        committed,
        Vec::new(),
        ApplyTopologyAuthority::Fixture,
    );
    let error = result.expect_err("malformed SCCP record must refuse metadata preparation");
    assert!(
        matches!(
            error,
            MergeLedgerCommitError::ExecutionBatchInvalid(ref reason)
                if reason == &format!("carrier metadata preparation: SCCP commitment: {expected:?}")
        ),
        "unexpected preparation refusal: {error:?}"
    );
    assert!(
        events.is_empty(),
        "refusal must not prepare publication events"
    );
    assert_eq!(
        crate::snapshot::canonical_staged_state_snapshot_hash(&state_block),
        staged_before,
        "SCCP validation must precede every staged State mutation",
    );
    drop(state_block);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        committed_before,
        "SCCP refusal must not change committed State",
    );
}

#[test]
fn apply_without_execution_rejects_duplicate_sccp_records_before_state_mutation() {
    let keypair = crate::state::checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let payload = iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
        nonce: 44,
        route_revision: 1,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        asset_id: b"xor".to_vec(),
        amount: 1,
        sender_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        sender: b"sora:bridge".to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
        recipient: vec![0x22; 20],
        route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        route_id: b"nexus:eth:xor".to_vec(),
    });
    let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&payload)
        .expect("valid SCCP apply-without-execution fixture payload encodes");
    let record = crate::bridge::test_record_sccp_message(payload_bytes.clone());
    let tx = iroha_data_model::transaction::TransactionBuilder::new(
        *DEFAULT_TEST_NETWORK_ID,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(iroha_data_model::transaction::Executable::IvmProved(
        iroha_data_model::transaction::IvmProved {
            bytecode: iroha_data_model::transaction::IvmBytecode::from_compiled(vec![
                0x01, 0x02, 0x03,
            ]),
            overlay: vec![
                iroha_data_model::isi::InstructionBox::from(record.clone()),
                iroha_data_model::isi::InstructionBox::from(record),
            ]
            .into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        },
    ))
    .sign(keypair.private_key());
    let entry_hash = tx.hash_as_entrypoint();
    let accepted = crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(tx));
    let leader = crate::state::checked_keypair();
    let mut block: SignedBlock = crate::block::BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .sign(leader.private_key())
        .unpack(|_| {})
        .into();
    // Header setters invalidate attached execution results. Bind the SCCP root
    // first, then attach the result-bearing projection this control validates.
    let messages = crate::bridge::collect_sccp_messages_from_signed_block(&block);
    let root = crate::bridge::sccp_commitment_root_from_messages(&messages)
        .expect("deduplicated SCCP root");
    block.set_sccp_commitment_root(Some(root));
    {
        let outputs = crate::execution_output_test_support::structural_network_outputs(
            &block,
            &[entry_hash],
            vec![Ok(
                iroha_data_model::transaction::DataTriggerSequence::default(),
            )],
        );
        let fragments =
            u64::try_from(outputs.iter().filter(|row| row.result().is_ok()).count()).unwrap();
        block.set_execution_outputs(
            outputs,
            fragments,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            Vec::new(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
    }
    .expect("test block entrypoint hash should match payload");
    let committed = crate::block::ValidBlock::committed_from_replay_signed_block(block);
    assert_sccp_apply_refusal(
        &committed,
        crate::bridge::SccpCommittedBlockValidationError::DuplicateOutboundMessage(
            crate::bridge::test_sccp_outbound_message_key(&payload),
        ),
    );
}
#[test]
fn apply_without_execution_rejects_invalid_sccp_record_payload_before_state_mutation() {
    let keypair = crate::state::checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let record = crate::bridge::test_record_sccp_message(b"not a canonical SCCP payload".to_vec());
    let tx = iroha_data_model::transaction::TransactionBuilder::new(
        *DEFAULT_TEST_NETWORK_ID,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(iroha_data_model::transaction::Executable::IvmProved(
        iroha_data_model::transaction::IvmProved {
            bytecode: iroha_data_model::transaction::IvmBytecode::from_compiled(vec![
                0x01, 0x02, 0x03,
            ]),
            overlay: vec![iroha_data_model::isi::InstructionBox::from(record)].into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        },
    ))
    .sign(keypair.private_key());
    let entry_hash = tx.hash_as_entrypoint();
    let accepted = crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(tx));
    let leader = crate::state::checked_keypair();
    let mut block: SignedBlock = crate::block::BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .sign(leader.private_key())
        .unpack(|_| {})
        .into();
    {
        let outputs = crate::execution_output_test_support::structural_network_outputs(
            &block,
            &[entry_hash],
            vec![Ok(
                iroha_data_model::transaction::DataTriggerSequence::default(),
            )],
        );
        let fragments =
            u64::try_from(outputs.iter().filter(|row| row.result().is_ok()).count()).unwrap();
        block.set_execution_outputs(
            outputs,
            fragments,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            Vec::new(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
    }
    .expect("test block entrypoint hash should match payload");
    let committed = crate::block::ValidBlock::committed_from_replay_signed_block(block);
    assert_sccp_apply_refusal(
        &committed,
        crate::bridge::SccpCommittedBlockValidationError::InvalidRecordInstruction(
            crate::bridge::SccpRecordInstructionValidationError::InvalidPayload {
                tx_index: 0,
                instruction_index: 0,
            },
        ),
    );
}
#[test]
fn apply_without_execution_rejects_unbound_sccp_record_route_before_state_mutation() {
    let keypair = crate::state::checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let payload = iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
        nonce: 47,
        route_revision: 1,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        asset_id: b"xor".to_vec(),
        amount: 1,
        sender_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        sender: b"sora:bridge".to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
        recipient: vec![0x22; 20],
        route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        route_id: b"nexus:bsc:xor".to_vec(),
    });
    let mut record = crate::bridge::test_record_sccp_message(
        iroha_sccp::canonical_sccp_payload_bytes(&payload)
            .expect("valid SCCP apply-without-execution fixture payload encodes"),
    );
    // Exact lane identity binds the destination; route labels are opaque.
    record.context.lane.target = iroha_data_model::bridge::SccpNetworkV1::BscMainnet;
    let tx = iroha_data_model::transaction::TransactionBuilder::new(
        *DEFAULT_TEST_NETWORK_ID,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(iroha_data_model::transaction::Executable::IvmProved(
        iroha_data_model::transaction::IvmProved {
            bytecode: iroha_data_model::transaction::IvmBytecode::from_compiled(vec![
                0x01, 0x02, 0x03,
            ]),
            overlay: vec![iroha_data_model::isi::InstructionBox::from(record)].into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        },
    ))
    .sign(keypair.private_key());
    let entry_hash = tx.hash_as_entrypoint();
    let accepted = crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(tx));
    let leader = crate::state::checked_keypair();
    let mut block: SignedBlock = crate::block::BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .sign(leader.private_key())
        .unpack(|_| {})
        .into();
    {
        let outputs = crate::execution_output_test_support::structural_network_outputs(
            &block,
            &[entry_hash],
            vec![Ok(
                iroha_data_model::transaction::DataTriggerSequence::default(),
            )],
        );
        let fragments =
            u64::try_from(outputs.iter().filter(|row| row.result().is_ok()).count()).unwrap();
        block.set_execution_outputs(
            outputs,
            fragments,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            Vec::new(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
    }
    .expect("test block entrypoint hash should match payload");
    let committed = crate::block::ValidBlock::committed_from_replay_signed_block(block);
    assert_sccp_apply_refusal(
        &committed,
        crate::bridge::SccpCommittedBlockValidationError::InvalidRecordInstruction(
            crate::bridge::SccpRecordInstructionValidationError::TargetProfileMismatch {
                tx_index: 0,
                instruction_index: 0,
                target: iroha_data_model::bridge::SccpNetworkV1::BscMainnet,
                payload_target_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            },
        ),
    );
}
#[test]
fn apply_without_execution_rejects_scoped_sccp_asset_alias_before_state_mutation() {
    let keypair = crate::state::checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let payload = iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
        nonce: 49,
        route_revision: 1,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        asset_id: b"xor#universal".to_vec(),
        amount: 1,
        sender_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        sender: b"sora:bridge".to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
        recipient: vec![0x22; 20],
        route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        route_id: b"nexus:eth:xor".to_vec(),
    });
    let record = crate::bridge::test_record_sccp_message(
        iroha_sccp::canonical_sccp_payload_bytes(&payload)
            .expect("valid SCCP apply-without-execution fixture payload encodes"),
    );
    let tx = iroha_data_model::transaction::TransactionBuilder::new(
        *DEFAULT_TEST_NETWORK_ID,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(iroha_data_model::transaction::Executable::IvmProved(
        iroha_data_model::transaction::IvmProved {
            bytecode: iroha_data_model::transaction::IvmBytecode::from_compiled(vec![
                0x01, 0x02, 0x03,
            ]),
            overlay: vec![iroha_data_model::isi::InstructionBox::from(record)].into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        },
    ))
    .sign(keypair.private_key());
    let entry_hash = tx.hash_as_entrypoint();
    let accepted = crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(tx));
    let leader = crate::state::checked_keypair();
    let mut block: SignedBlock = crate::block::BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .sign(leader.private_key())
        .unpack(|_| {})
        .into();
    {
        let outputs = crate::execution_output_test_support::structural_network_outputs(
            &block,
            &[entry_hash],
            vec![Ok(
                iroha_data_model::transaction::DataTriggerSequence::default(),
            )],
        );
        let fragments =
            u64::try_from(outputs.iter().filter(|row| row.result().is_ok()).count()).unwrap();
        block.set_execution_outputs(
            outputs,
            fragments,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            Vec::new(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
    }
    .expect("test block entrypoint hash should match payload");
    let committed = crate::block::ValidBlock::committed_from_replay_signed_block(block);
    assert_sccp_apply_refusal(
        &committed,
        crate::bridge::SccpCommittedBlockValidationError::InvalidRecordInstruction(
            crate::bridge::SccpRecordInstructionValidationError::RouteBinding {
                tx_index: 0,
                instruction_index: 0,
                error: crate::bridge::SccpOutboundRouteValidationError::AssetScopeAlias {
                    asset_key: "xor".to_owned(),
                    scope: "universal".to_owned(),
                },
            },
        ),
    );
}
#[test]
fn apply_without_execution_rejects_resultless_sccp_root_before_state_mutation() {
    let keypair = crate::state::checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let payload = iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
        nonce: 45,
        route_revision: 1,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        asset_id: b"xor".to_vec(),
        amount: 1,
        sender_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        sender: b"sora:bridge".to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
        recipient: vec![0x22; 20],
        route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        route_id: b"nexus:eth:xor".to_vec(),
    });
    let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&payload)
        .expect("valid SCCP apply-without-execution fixture payload encodes");
    let record = crate::bridge::test_record_sccp_message(payload_bytes);
    let tx = iroha_data_model::transaction::TransactionBuilder::new(
        *DEFAULT_TEST_NETWORK_ID,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(iroha_data_model::transaction::Executable::IvmProved(
        iroha_data_model::transaction::IvmProved {
            bytecode: iroha_data_model::transaction::IvmBytecode::from_compiled(vec![
                0x01, 0x02, 0x03,
            ]),
            overlay: vec![iroha_data_model::isi::InstructionBox::from(record)].into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        },
    ))
    .sign(keypair.private_key());
    let accepted = crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(tx));
    let leader = crate::state::checked_keypair();
    let mut block: SignedBlock = crate::block::BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .sign(leader.private_key())
        .unpack(|_| {})
        .into();
    let messages = crate::bridge::collect_sccp_messages_from_signed_block(&block);
    let root =
        crate::bridge::sccp_commitment_root_from_messages(&messages).expect("resultless SCCP root");
    block.set_sccp_commitment_root(Some(root));
    let committed = crate::block::ValidBlock::committed_from_replay_signed_block(block);
    assert_sccp_apply_refusal(
        &committed,
        crate::bridge::SccpCommittedBlockValidationError::MissingTransactionResults {
            actual: root,
        },
    );
}
#[test]
fn lane_lifecycle_waits_for_inflight_state_commit_lock() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::default(), kura, query));

    let plan = iroha_data_model::nexus::LaneLifecyclePlan {
        additions: vec![LaneConfigModel {
            id: LaneId::new(1),
            alias: "serialized-beta".to_string(),
            ..LaneConfigModel::default()
        }],
        retire: Vec::new(),
    };
    let commit_guard = state.state_commit_lock.lock();
    let (attempt_tx, attempt_rx) = mpsc::channel();
    let lifecycle_state = Arc::clone(&state);
    let handle = thread::spawn(move || {
        attempt_tx
            .send(())
            .expect("notify lifecycle attempt started");
        lifecycle_state
            .apply_lane_lifecycle(&plan)
            .expect("lane lifecycle");
    });
    attempt_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("lifecycle thread started");
    thread::sleep(Duration::from_millis(50));
    assert!(
        state
            .nexus_snapshot()
            .lane_catalog
            .by_alias("serialized-beta")
            .is_none(),
        "manual lifecycle must not publish while a state commit is in progress"
    );
    drop(commit_guard);
    handle.join().expect("lane lifecycle thread");
    assert!(
        state
            .nexus_snapshot()
            .lane_catalog
            .by_alias("serialized-beta")
            .is_some(),
        "manual lifecycle should publish after the state commit lock is released"
    );
}
#[test]
fn heavy_world_commit_bench_helper_commits_accounts() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let elapsed = state
        .commit_heavy_world_accounts_for_bench(nonzero!(1_u64), 16)
        .expect("heavy world bench commit");
    assert!(elapsed > Duration::ZERO);
    assert_eq!(state.view().world.accounts().iter().count(), 16);
}
