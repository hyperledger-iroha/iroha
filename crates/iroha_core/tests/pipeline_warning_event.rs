//! Canonical execution is independent of local advisory DAG sidecars.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Advisory metadata cannot add consensus-visible warning events.
use iroha_config::parameters::actual::LaneConfig;
use iroha_core::{
    governance::manifest::LaneManifestRegistry,
    kura::{Kura, PipelineDagSnapshot, PipelineRecoverySidecar, PipelineTxSnapshot},
    query::store::LiveQueryStore,
    state::State,
};
use iroha_data_model::{events::EventBox, prelude::*};
use iroha_model_base::chain::ChainId;
use iroha_model_base::domain::DomainId;
use std::sync::Arc;
// unused
#[test]
fn canonical_execution_ignores_mismatching_local_dag_sidecar() {
    // Build a persistent Kura in a temp directory so sidecars are writable.
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let (kura, _block_count) = Kura::new_fresh_single_lane(
        &iroha_config::parameters::actual::Kura {
            init_mode: iroha_config::kura::InitMode::Strict,
            store_dir: iroha_config::base::WithOrigin::inline(
                temp_dir.path().to_str().unwrap().into(),
            ),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,
            lane_history_retention:
                iroha_config::parameters::defaults::kura::LANE_HISTORY_RETENTION,
            block_hash_history_bytes:
                iroha_config::parameters::defaults::kura::BLOCK_HASH_HISTORY_BYTES,
            transaction_history_bytes:
                iroha_config::parameters::defaults::kura::TRANSACTION_HISTORY_BYTES,
            fastpq_artifacts: iroha_config::parameters::defaults::kura::FASTPQ_ARTIFACT_POLICY,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
        },
        &LaneConfig::default(),
    )
    .expect("kura init");
    let query = LiveQueryStore::start_test();
    // Minimal world: one domain, two accounts, one asset def
    let (alice_id, alice_keypair) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, bob_keypair) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let ad: AssetDefinition = AssetDefinition::new(
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        ),
        "coin".to_owned(),
        NumericSpec::default(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&alice_id);
    let acc_a = Account::new(alice_id.clone()).build(&alice_id);
    let acc_b = Account::new(bob_id.clone()).build(&alice_id);
    let world = iroha_core::state::World::with([domain], [acc_a, acc_b], [ad]);
    let chain_id = ChainId::from("chain");
    let state = State::new_with_chain_for_testing(world, kura.clone(), query, chain_id.clone());
    let network_id = *state.network_id_ref();
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));
    let genesis = state
        .seed_signed_genesis_for_testing(&iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR)
        .expect("publish fixture genesis");
    // Build a block with two txs (independent)
    let rose: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
    let a_coin = AssetId::of(rose.clone(), alice_id.clone());
    let tx1 = TransactionBuilder::new(
        network_id,
        alice_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Mint::asset_quantity(5_u32, a_coin.clone())])
    .sign(alice_keypair.private_key());
    let tx2 = TransactionBuilder::new(
        network_id,
        bob_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([SetKeyValue::account(
        bob_id.clone(),
        "k".parse().unwrap(),
        iroha_primitives::json::Json::new("v"),
    )])
    .sign(bob_keypair.private_key());
    let acc: Vec<_> = vec![tx1, tx2]
        .into_iter()
        .map(|t| iroha_core::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(t)))
        .collect();
    let new_block = iroha_core::block::BlockBuilder::new(acc)
        .chain(0, Some(&genesis))
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    // Inject a mismatching sidecar for this block height before validation
    let height = new_block.header().height().get();
    let block_hash = new_block.header().hash();
    let mut fingerprint = [0u8; 32];
    fingerprint[..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    let sidecar_txs: Vec<PipelineTxSnapshot> = new_block
        .transactions()
        .iter()
        .map(|tx| PipelineTxSnapshot::compact(tx.as_ref().hash_as_entrypoint(), 0, 0))
        .collect();
    let sidecar = PipelineRecoverySidecar::new(
        height,
        block_hash,
        PipelineDagSnapshot {
            fingerprint,
            key_count: 0,
        },
        sidecar_txs,
    );
    kura.write_pipeline_metadata(&sidecar);
    // Publish canonical outputs; local DAG metadata has no consensus authority.
    let mut sb = state.block(new_block.header());
    let vb =
        iroha_core::block::ValidBlock::validate_unchecked(new_block.into(), &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    assert!(
        cb.as_ref()
            .output_results()
            .all(|result| result.as_ref().is_ok())
    );
    let events = state
        .commit_executed_block_for_testing(sb, cb)
        .expect("publish canonical outputs independently of advisory metadata");
    assert!(
        !events.is_empty(),
        "publication must emit the transaction events"
    );
    let warned = events.iter().any(|e| match e {
        EventBox::Pipeline(iroha_data_model::events::pipeline::PipelineEventBox::Warning(w)) => {
            w.kind == "dag_fingerprint_mismatch"
        }
        EventBox::PipelineBatch(batch) => batch.iter().any(|event| {
            matches!(
                event,
                iroha_data_model::events::pipeline::PipelineEventBox::Warning(w)
                    if w.kind == "dag_fingerprint_mismatch"
            )
        }),
        _ => false,
    });
    assert!(
        !warned,
        "local DAG mismatch must not affect canonical events"
    );
}
#[test]
fn pipeline_warning_ignored_for_stale_sidecar() {
    // Build a persistent Kura in a temp directory so sidecars are writable.
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let (kura, _block_count) = Kura::new_fresh_single_lane(
        &iroha_config::parameters::actual::Kura {
            init_mode: iroha_config::kura::InitMode::Strict,
            store_dir: iroha_config::base::WithOrigin::inline(
                temp_dir.path().to_str().unwrap().into(),
            ),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,
            lane_history_retention:
                iroha_config::parameters::defaults::kura::LANE_HISTORY_RETENTION,
            block_hash_history_bytes:
                iroha_config::parameters::defaults::kura::BLOCK_HASH_HISTORY_BYTES,
            transaction_history_bytes:
                iroha_config::parameters::defaults::kura::TRANSACTION_HISTORY_BYTES,
            fastpq_artifacts: iroha_config::parameters::defaults::kura::FASTPQ_ARTIFACT_POLICY,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
        },
        &LaneConfig::default(),
    )
    .expect("kura init");
    let query = LiveQueryStore::start_test();
    // Minimal world: one domain, two accounts, one asset def
    let (alice_id, alice_keypair) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, bob_keypair) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let ad: AssetDefinition = AssetDefinition::new(
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        ),
        "coin".to_owned(),
        NumericSpec::default(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&alice_id);
    let acc_a = Account::new(alice_id.clone()).build(&alice_id);
    let acc_b = Account::new(bob_id.clone()).build(&alice_id);
    let world = iroha_core::state::World::with([domain], [acc_a, acc_b], [ad]);
    let chain_id = ChainId::from("chain");
    let state = State::new_with_chain_for_testing(world, kura.clone(), query, chain_id.clone());
    let network_id = *state.network_id_ref();
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));
    let genesis = state
        .seed_signed_genesis_for_testing(&iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR)
        .expect("publish fixture genesis");
    // Build a block with two txs (independent)
    let rose: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
    let a_coin = AssetId::of(rose.clone(), alice_id.clone());
    let tx1 = TransactionBuilder::new(
        network_id,
        alice_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Mint::asset_quantity(5_u32, a_coin.clone())])
    .sign(alice_keypair.private_key());
    let tx2 = TransactionBuilder::new(
        network_id,
        bob_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([SetKeyValue::account(
        bob_id.clone(),
        "k".parse().unwrap(),
        iroha_primitives::json::Json::new("v"),
    )])
    .sign(bob_keypair.private_key());
    let acc: Vec<_> = vec![tx1, tx2]
        .into_iter()
        .map(|t| iroha_core::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(t)))
        .collect();
    let new_block = iroha_core::block::BlockBuilder::new(acc)
        .chain(0, Some(&genesis))
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    // Inject a stale sidecar (no tx hashes for this block height) before validation
    let height = new_block.header().height().get();
    let block_hash = new_block.header().hash();
    let mut fingerprint = [0u8; 32];
    fingerprint[..4].copy_from_slice(&[0xBA, 0xAD, 0xF0, 0x0D]);
    let sidecar = PipelineRecoverySidecar::new(
        height,
        block_hash,
        PipelineDagSnapshot {
            fingerprint,
            key_count: 0,
        },
        Vec::new(),
    );
    kura.write_pipeline_metadata(&sidecar);
    // Validate and apply; expect no DAG mismatch warning when sidecar txs do not match the block
    let mut sb = state.block(new_block.header());
    let vb =
        iroha_core::block::ValidBlock::validate_unchecked(new_block.into(), &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    assert!(
        cb.as_ref()
            .output_results()
            .all(|result| result.as_ref().is_ok())
    );
    let events = state
        .commit_executed_block_for_testing(sb, cb)
        .expect("publish canonical outputs independently of advisory metadata");
    assert!(
        !events.is_empty(),
        "publication must emit the transaction events"
    );
    let warned = events.iter().any(|e| match e {
        EventBox::Pipeline(iroha_data_model::events::pipeline::PipelineEventBox::Warning(w)) => {
            w.kind == "dag_fingerprint_mismatch"
        }
        EventBox::PipelineBatch(batch) => batch.iter().any(|event| {
            matches!(
                event,
                iroha_data_model::events::pipeline::PipelineEventBox::Warning(w)
                    if w.kind == "dag_fingerprint_mismatch"
            )
        }),
        _ => false,
    });
    assert!(!warned, "expected no pipeline warning for stale sidecar");
}
