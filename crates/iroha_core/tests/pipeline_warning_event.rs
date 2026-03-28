//! Pipeline warning delivery test: inject a mismatching DAG fingerprint sidecar
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! and assert that a Pipeline Warning event is emitted during block processing.

use iroha_config::parameters::actual::LaneConfig;
use iroha_core::{
    kura::{Kura, PipelineDagSnapshot, PipelineRecoverySidecar, PipelineTxSnapshot},
    query::store::LiveQueryStore,
    state::State,
};
use iroha_data_model::{events::EventBox, prelude::*};
// unused

#[test]
fn pipeline_warning_emitted_on_dag_mismatch() {
    // Build a persistent Kura in a temp directory so sidecars are writable.
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let (kura, _block_count) = Kura::new(
        &iroha_config::parameters::actual::Kura {
            init_mode: iroha_config::kura::InitMode::Strict,
            store_dir: iroha_config::base::WithOrigin::inline(
                temp_dir.path().to_str().unwrap().into(),
            ),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,
            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
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
    let (alice_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let ad: AssetDefinition = AssetDefinition::new(
        iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "coin".parse().unwrap(),
        ),
        NumericSpec::default(),
    )
    .build(&alice_id);
    let acc_a = Account::new_in_domain(alice_id.clone(), domain_id.clone()).build(&alice_id);
    let acc_b = Account::new_in_domain(bob_id.clone(), domain_id.clone()).build(&alice_id);
    let world = iroha_core::state::World::with([domain], [acc_a, acc_b], [ad]);
    #[cfg(feature = "telemetry")]
    let state = State::new(
        world,
        kura.clone(),
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(world, kura.clone(), query);

    // Build a block with two txs (independent)
    let chain_id = ChainId::from("chain");
    let rose: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "coin".parse().unwrap(),
    );
    let a_coin = AssetId::of(rose.clone(), alice_id.clone());
    let tx1 = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Mint::asset_numeric(5_u32, a_coin.clone())])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx2 = TransactionBuilder::new(chain_id.clone(), bob_id.clone())
        .with_instructions([SetKeyValue::account(
            bob_id.clone(),
            "k".parse().unwrap(),
            iroha_primitives::json::Json::new("v"),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let acc: Vec<_> = vec![tx1, tx2]
        .into_iter()
        .map(|t| iroha_core::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(t)))
        .collect();
    let new_block = iroha_core::block::BlockBuilder::new(acc)
        .chain(0, None)
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
        .map(|tx| PipelineTxSnapshot {
            hash: tx.as_ref().hash_as_entrypoint(),
            reads: Vec::new(),
            writes: Vec::new(),
        })
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

    // Validate and apply; expect a Pipeline Warning event among returned events
    let mut sb = state.block(new_block.header());
    let vb =
        iroha_core::block::ValidBlock::validate_unchecked(new_block.into(), &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let events = sb.apply_without_execution(&cb, Vec::new());

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
    assert!(warned, "expected a pipeline warning event for DAG mismatch");
}

#[test]
fn pipeline_warning_ignored_for_stale_sidecar() {
    // Build a persistent Kura in a temp directory so sidecars are writable.
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let (kura, _block_count) = Kura::new(
        &iroha_config::parameters::actual::Kura {
            init_mode: iroha_config::kura::InitMode::Strict,
            store_dir: iroha_config::base::WithOrigin::inline(
                temp_dir.path().to_str().unwrap().into(),
            ),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,
            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
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
    let (alice_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let ad: AssetDefinition = AssetDefinition::new(
        iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "coin".parse().unwrap(),
        ),
        NumericSpec::default(),
    )
    .build(&alice_id);
    let acc_a = Account::new_in_domain(alice_id.clone(), domain_id.clone()).build(&alice_id);
    let acc_b = Account::new_in_domain(bob_id.clone(), domain_id.clone()).build(&alice_id);
    let world = iroha_core::state::World::with([domain], [acc_a, acc_b], [ad]);
    #[cfg(feature = "telemetry")]
    let state = State::new(
        world,
        kura.clone(),
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(world, kura.clone(), query);

    // Build a block with two txs (independent)
    let chain_id = ChainId::from("chain");
    let rose: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "coin".parse().unwrap(),
    );
    let a_coin = AssetId::of(rose.clone(), alice_id.clone());
    let tx1 = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Mint::asset_numeric(5_u32, a_coin.clone())])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx2 = TransactionBuilder::new(chain_id.clone(), bob_id.clone())
        .with_instructions([SetKeyValue::account(
            bob_id.clone(),
            "k".parse().unwrap(),
            iroha_primitives::json::Json::new("v"),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let acc: Vec<_> = vec![tx1, tx2]
        .into_iter()
        .map(|t| iroha_core::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(t)))
        .collect();
    let new_block = iroha_core::block::BlockBuilder::new(acc)
        .chain(0, None)
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
    let events = sb.apply_without_execution(&cb, Vec::new());

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
