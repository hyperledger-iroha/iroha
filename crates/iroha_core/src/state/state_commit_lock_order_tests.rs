use std::{
    sync::{Arc, Barrier, mpsc},
    thread,
    time::{Duration, Instant},
};

use iroha_data_model::{block::BlockHeader, nexus::LaneConfig as LaneConfigModel};
use nonzero_ext::nonzero;

use super::*;
use crate::kura::Kura;

#[test]
fn state_commit_does_not_hold_tiered_backend_while_waiting_for_state_write_lock() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::default(), kura, query));

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let _write_guard = state.state_write_lock.lock();
    let barrier = Arc::new(Barrier::new(2));
    let commit_state = Arc::clone(&state);
    let commit_barrier = Arc::clone(&barrier);
    let handle = thread::spawn(move || {
        commit_barrier.wait();
        let block = commit_state.block(header);
        block.commit().expect("commit should succeed");
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
    state.nexus.write().enabled = true;

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
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let commit_handle = thread::spawn(move || {
        commit_barrier.wait();
        let block = commit_state.block(header);
        block.commit().expect("commit");
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
fn lane_lifecycle_cleanup_does_not_hold_commit_serialization_from_prebuilt_block() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::default(), kura, query));
    state.nexus.write().enabled = true;

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
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let block = commit_state.block(header);
        block_ready_tx
            .send(())
            .expect("notify prebuilt block is holding its overlay");
        commit_release_rx
            .recv()
            .expect("wait for lifecycle catalog publication");
        block.commit().expect("commit prebuilt block");
        let _ = commit_done.send("commit");
    });

    block_ready_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("prebuilt block ready");

    let lifecycle_state = Arc::clone(&state);
    let lifecycle_done = done_tx.clone();
    let lifecycle_handle = thread::spawn(move || {
        lifecycle_state
            .apply_lane_lifecycle(&plan)
            .expect("lane lifecycle");
        let _ = lifecycle_done.send("lifecycle");
    });

    let publication_start = Instant::now();
    let mut catalog_published = false;
    while publication_start.elapsed() < Duration::from_secs(1) {
        if state
            .nexus_snapshot()
            .lane_catalog
            .by_alias("prebuilt-beta")
            .is_some()
        {
            catalog_published = true;
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert!(
        catalog_published,
        "lane lifecycle should publish catalog before waiting on world-backed cleanup"
    );
    commit_release_tx
        .send(())
        .expect("release prebuilt block commit");

    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("first operation completion");
    done_rx
        .recv_timeout(Duration::from_secs(2))
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
fn transaction_uses_prebuilt_block_nexus_snapshot_after_shared_catalog_update() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    state.nexus.write().enabled = true;

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
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
            .nexus_snapshot()
            .lane_catalog
            .by_alias("post-block-beta")
            .is_some(),
        "shared Nexus catalog update should be visible to new state snapshots"
    );

    let tx = block.transaction();

    assert_eq!(tx.nexus.lane_catalog, block_catalog);
    assert!(
        tx.nexus.lane_catalog.by_alias("post-block-beta").is_none(),
        "transactions opened from a prebuilt block must not observe later Nexus catalog updates"
    );
}

#[test]
#[should_panic(
    expected = "committed block failed SCCP commitment validation before apply_without_execution"
)]
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
    block
        .set_transaction_results(
            Vec::new(),
            &[entry_hash],
            vec![Ok(
                iroha_data_model::transaction::DataTriggerSequence::default(),
            )],
        )
        .expect("test block entrypoint hash should match payload");
    let messages = crate::bridge::collect_sccp_messages_from_signed_block(&block);
    let root = crate::bridge::sccp_commitment_root_from_messages(&messages)
        .expect("deduplicated SCCP root");
    block.set_sccp_commitment_root(Some(root));
    let committed = crate::block::ValidBlock::committed_from_replay_signed_block(block);

    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let mut state_block = state.block(committed.as_ref().header());

    let _ = state_block.apply_without_execution(&committed, Vec::new());
}

#[test]
#[should_panic(
    expected = "committed block failed SCCP commitment validation before apply_without_execution"
)]
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
    block
        .set_transaction_results(
            Vec::new(),
            &[entry_hash],
            vec![Ok(
                iroha_data_model::transaction::DataTriggerSequence::default(),
            )],
        )
        .expect("test block entrypoint hash should match payload");
    let committed = crate::block::ValidBlock::committed_from_replay_signed_block(block);

    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let mut state_block = state.block(committed.as_ref().header());

    let _ = state_block.apply_without_execution(&committed, Vec::new());
}

#[test]
#[should_panic(
    expected = "committed block failed SCCP commitment validation before apply_without_execution"
)]
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
    block
        .set_transaction_results(
            Vec::new(),
            &[entry_hash],
            vec![Ok(
                iroha_data_model::transaction::DataTriggerSequence::default(),
            )],
        )
        .expect("test block entrypoint hash should match payload");
    let committed = crate::block::ValidBlock::committed_from_replay_signed_block(block);

    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let mut state_block = state.block(committed.as_ref().header());

    let _ = state_block.apply_without_execution(&committed, Vec::new());
}

#[test]
#[should_panic(
    expected = "committed block failed SCCP commitment validation before apply_without_execution"
)]
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
    block
        .set_transaction_results(
            Vec::new(),
            &[entry_hash],
            vec![Ok(
                iroha_data_model::transaction::DataTriggerSequence::default(),
            )],
        )
        .expect("test block entrypoint hash should match payload");
    let committed = crate::block::ValidBlock::committed_from_replay_signed_block(block);

    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let mut state_block = state.block(committed.as_ref().header());

    let _ = state_block.apply_without_execution(&committed, Vec::new());
}

#[test]
#[should_panic(
    expected = "committed block failed SCCP commitment validation before apply_without_execution"
)]
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

    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let mut state_block = state.block(committed.as_ref().header());

    let _ = state_block.apply_without_execution(&committed, Vec::new());
}

#[test]
fn lane_lifecycle_waits_for_inflight_state_commit_lock() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::default(), kura, query));
    state.nexus.write().enabled = true;

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
