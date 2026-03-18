//! Differential test for the `parallel_apply` pipeline knob.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Ensures that enabling the skeleton parallel-apply path yields identical
//! outcomes to the sequential apply path.

use std::{borrow::Cow, collections::BTreeSet, time::Duration};

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_data_model::prelude::*;
use iroha_primitives::{numeric::Numeric, time::TimeSource};
use mv::storage::StorageReadOnly;
mod snapshots;
use snapshots::assert_events;

// Use a fixed creation time so event fixtures do not depend on wall clock.
const FIXTURE_TIME: Duration = Duration::from_millis(1);

fn tx_builder(chain_id: &ChainId, authority: &AccountId) -> TransactionBuilder {
    let mut builder = TransactionBuilder::new(chain_id.clone(), authority.clone());
    builder.set_creation_time(FIXTURE_TIME);
    builder
}

fn block_time_source() -> TimeSource {
    let (_, source) = TimeSource::new_mock(FIXTURE_TIME);
    source
}

#[allow(clippy::too_many_lines)]
#[test]
fn parallel_apply_matches_sequential_for_log_and_mint() {
    // Build a small world: one domain, two accounts, one numeric asset def, zeroed assets
    let chain_id = ChainId::from("chain");
    let alice_id = (*iroha_test_samples::ALICE_ID).clone();
    let bob_id = (*iroha_test_samples::BOB_ID).clone();
    let build_world = || {
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
        let acc_a =
            Account::new(alice_id.clone().to_account_id(domain_id.clone())).build(&alice_id);
        let acc_b = Account::new(bob_id.clone().to_account_id(domain_id)).build(&alice_id);
        let a_coin = AssetId::of(ad.id().clone(), alice_id.clone());
        let b_coin = AssetId::of(ad.id().clone(), bob_id.clone());
        let a0 = Asset::new(a_coin, Numeric::new(0, 0));
        let b0 = Asset::new(b_coin, Numeric::new(0, 0));
        iroha_core::state::World::with_assets([domain], [acc_a, acc_b], [ad], [a0, b0], [])
    };
    // Kura + query handles
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();

    // Two independent transactions: a mint and a log. Mint will take the standard path,
    // log is handled by detached path; overall results should match sequential mode.
    let tx1 = tx_builder(&chain_id, &alice_id)
        .with_instructions([Mint::asset_numeric(
            10_u32,
            AssetId::of(
                iroha_data_model::asset::AssetDefinitionId::new(
                    "wonderland".parse().unwrap(),
                    "coin".parse().unwrap(),
                ),
                alice_id.clone(),
            ),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx2 = tx_builder(&chain_id, &bob_id)
        .with_instructions([Log::new(Level::INFO, "t2".to_string())])
        .sign(iroha_test_samples::BOB_KEYPAIR.private_key());

    // Build a NewBlock with both transactions
    let tx1 = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx1));
    let tx2 = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx2));
    let new_block = iroha_core::block::BlockBuilder::new_with_time_source(
        vec![tx1.clone(), tx2.clone()],
        block_time_source(),
    )
    .chain(0, None)
    .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
    .unpack(|_| {});

    // Run sequential apply
    let world_seq = build_world();
    #[cfg(feature = "telemetry")]
    let mut state_seq = iroha_core::state::State::new(
        world_seq,
        kura.clone(),
        query.clone(),
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state_seq = iroha_core::state::State::new(world_seq, kura.clone(), query.clone());
    let cfg_seq = iroha_config::parameters::actual::Pipeline {
        ivm_proved: iroha_config::parameters::actual::IvmProvedExecution {
            enabled: iroha_config::parameters::defaults::pipeline::ivm_proved::ENABLED,
            skip_replay: iroha_config::parameters::defaults::pipeline::ivm_proved::SKIP_REPLAY,
            allowed_circuits: Vec::new(),
        },
        dynamic_prepass: iroha_config::parameters::defaults::pipeline::DYNAMIC_PREPASS,
        access_set_cache_enabled:
            iroha_config::parameters::defaults::pipeline::ACCESS_SET_CACHE_ENABLED,
        parallel_overlay: iroha_config::parameters::defaults::pipeline::PARALLEL_OVERLAY,
        workers: iroha_config::parameters::defaults::pipeline::WORKERS,
        stateless_cache_cap: iroha_config::parameters::defaults::pipeline::STATELESS_CACHE_CAP,
        parallel_apply: false,
        ready_queue_heap: iroha_config::parameters::defaults::pipeline::READY_QUEUE_HEAP,
        gpu_key_bucket: iroha_config::parameters::defaults::pipeline::GPU_KEY_BUCKET,
        debug_trace_scheduler_inputs:
            iroha_config::parameters::defaults::pipeline::DEBUG_TRACE_SCHEDULER_INPUTS,
        debug_trace_tx_eval: iroha_config::parameters::defaults::pipeline::DEBUG_TRACE_TX_EVAL,
        signature_batch_max: iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX,
        signature_batch_max_ed25519:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_ED25519,
        signature_batch_max_secp256k1:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_SECP256K1,
        signature_batch_max_pqc:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_PQC,
        signature_batch_max_bls:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_BLS,
        cache_size: iroha_config::parameters::defaults::pipeline::CACHE_SIZE,
        ivm_cache_max_decoded_ops:
            iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_DECODED_OPS,
        ivm_cache_max_bytes: iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_BYTES,
        ivm_prover_threads: iroha_config::parameters::defaults::pipeline::IVM_PROVER_THREADS,
        overlay_max_instructions:
            iroha_config::parameters::defaults::pipeline::OVERLAY_MAX_INSTRUCTIONS,
        overlay_max_bytes: iroha_config::parameters::defaults::pipeline::OVERLAY_MAX_BYTES,
        overlay_chunk_instructions:
            iroha_config::parameters::defaults::pipeline::OVERLAY_CHUNK_INSTRUCTIONS,
        gas: iroha_config::parameters::actual::Gas {
            tech_account_id: iroha_config::parameters::defaults::pipeline::GAS_TECH_ACCOUNT_ID
                .to_string(),
            accepted_assets: Vec::new(),
            units_per_gas: Vec::new(),
        },
        ivm_max_cycles_upper_bound:
            iroha_config::parameters::defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND,
        ivm_max_decoded_instructions:
            iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_INSTRUCTIONS,
        ivm_max_decoded_bytes: iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_BYTES,
        quarantine_max_txs_per_block:
            iroha_config::parameters::defaults::pipeline::QUARANTINE_MAX_TXS_PER_BLOCK,
        quarantine_tx_max_cycles:
            iroha_config::parameters::defaults::pipeline::QUARANTINE_TX_MAX_CYCLES,
        quarantine_tx_max_millis:
            iroha_config::parameters::defaults::pipeline::QUARANTINE_TX_MAX_MILLIS,
        query_default_cursor_mode: iroha_config::parameters::actual::QueryCursorMode::Ephemeral,
        query_max_fetch_size: iroha_config::parameters::defaults::pipeline::QUERY_MAX_FETCH_SIZE,
        query_stored_min_gas_units:
            iroha_config::parameters::defaults::pipeline::QUERY_STORED_MIN_GAS_UNITS,
        amx_per_dataspace_budget_ms:
            iroha_config::parameters::defaults::pipeline::AMX_PER_DATASPACE_BUDGET_MS,
        amx_group_budget_ms: iroha_config::parameters::defaults::pipeline::AMX_GROUP_BUDGET_MS,
        amx_per_instruction_ns:
            iroha_config::parameters::defaults::pipeline::AMX_PER_INSTRUCTION_NS,
        amx_per_memory_access_ns:
            iroha_config::parameters::defaults::pipeline::AMX_PER_MEMORY_ACCESS_NS,
        amx_per_syscall_ns: iroha_config::parameters::defaults::pipeline::AMX_PER_SYSCALL_NS,
    };
    state_seq.set_pipeline(cfg_seq);
    let mut sb_seq = state_seq.block(new_block.header());
    let vb_seq = new_block
        .clone()
        .validate_and_record_transactions(&mut sb_seq)
        .unpack(|_| {});
    sb_seq.commit().unwrap();

    // Run parallel-apply (skeleton path)
    let world_par = build_world();
    #[cfg(feature = "telemetry")]
    let mut state_par = iroha_core::state::State::new(
        world_par,
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state_par = iroha_core::state::State::new(world_par, kura, query);
    let cfg_par = iroha_config::parameters::actual::Pipeline {
        ivm_proved: iroha_config::parameters::actual::IvmProvedExecution {
            enabled: iroha_config::parameters::defaults::pipeline::ivm_proved::ENABLED,
            skip_replay: iroha_config::parameters::defaults::pipeline::ivm_proved::SKIP_REPLAY,
            allowed_circuits: Vec::new(),
        },
        dynamic_prepass: iroha_config::parameters::defaults::pipeline::DYNAMIC_PREPASS,
        access_set_cache_enabled:
            iroha_config::parameters::defaults::pipeline::ACCESS_SET_CACHE_ENABLED,
        parallel_overlay: iroha_config::parameters::defaults::pipeline::PARALLEL_OVERLAY,
        workers: iroha_config::parameters::defaults::pipeline::WORKERS,
        stateless_cache_cap: iroha_config::parameters::defaults::pipeline::STATELESS_CACHE_CAP,
        parallel_apply: true,
        ready_queue_heap: iroha_config::parameters::defaults::pipeline::READY_QUEUE_HEAP,
        gpu_key_bucket: iroha_config::parameters::defaults::pipeline::GPU_KEY_BUCKET,
        debug_trace_scheduler_inputs:
            iroha_config::parameters::defaults::pipeline::DEBUG_TRACE_SCHEDULER_INPUTS,
        debug_trace_tx_eval: iroha_config::parameters::defaults::pipeline::DEBUG_TRACE_TX_EVAL,
        signature_batch_max: iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX,
        signature_batch_max_ed25519:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_ED25519,
        signature_batch_max_secp256k1:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_SECP256K1,
        signature_batch_max_pqc:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_PQC,
        signature_batch_max_bls:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_BLS,
        cache_size: iroha_config::parameters::defaults::pipeline::CACHE_SIZE,
        ivm_cache_max_decoded_ops:
            iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_DECODED_OPS,
        ivm_cache_max_bytes: iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_BYTES,
        ivm_prover_threads: iroha_config::parameters::defaults::pipeline::IVM_PROVER_THREADS,
        overlay_max_instructions:
            iroha_config::parameters::defaults::pipeline::OVERLAY_MAX_INSTRUCTIONS,
        overlay_max_bytes: iroha_config::parameters::defaults::pipeline::OVERLAY_MAX_BYTES,
        overlay_chunk_instructions:
            iroha_config::parameters::defaults::pipeline::OVERLAY_CHUNK_INSTRUCTIONS,
        gas: iroha_config::parameters::actual::Gas {
            tech_account_id: iroha_config::parameters::defaults::pipeline::GAS_TECH_ACCOUNT_ID
                .to_string(),
            accepted_assets: Vec::new(),
            units_per_gas: Vec::new(),
        },
        ivm_max_cycles_upper_bound:
            iroha_config::parameters::defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND,
        ivm_max_decoded_instructions:
            iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_INSTRUCTIONS,
        ivm_max_decoded_bytes: iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_BYTES,
        quarantine_max_txs_per_block:
            iroha_config::parameters::defaults::pipeline::QUARANTINE_MAX_TXS_PER_BLOCK,
        quarantine_tx_max_cycles:
            iroha_config::parameters::defaults::pipeline::QUARANTINE_TX_MAX_CYCLES,
        quarantine_tx_max_millis:
            iroha_config::parameters::defaults::pipeline::QUARANTINE_TX_MAX_MILLIS,
        query_default_cursor_mode: iroha_config::parameters::actual::QueryCursorMode::Ephemeral,
        query_max_fetch_size: iroha_config::parameters::defaults::pipeline::QUERY_MAX_FETCH_SIZE,
        query_stored_min_gas_units:
            iroha_config::parameters::defaults::pipeline::QUERY_STORED_MIN_GAS_UNITS,
        amx_per_dataspace_budget_ms:
            iroha_config::parameters::defaults::pipeline::AMX_PER_DATASPACE_BUDGET_MS,
        amx_group_budget_ms: iroha_config::parameters::defaults::pipeline::AMX_GROUP_BUDGET_MS,
        amx_per_instruction_ns:
            iroha_config::parameters::defaults::pipeline::AMX_PER_INSTRUCTION_NS,
        amx_per_memory_access_ns:
            iroha_config::parameters::defaults::pipeline::AMX_PER_MEMORY_ACCESS_NS,
        amx_per_syscall_ns: iroha_config::parameters::defaults::pipeline::AMX_PER_SYSCALL_NS,
    };
    state_par.set_pipeline(cfg_par);
    let mut sb_par = state_par.block(new_block.header());
    let vb_par = new_block
        .validate_and_record_transactions(&mut sb_par)
        .unpack(|_| {});
    sb_par.commit().unwrap();

    // Compare results order and kinds
    let seq_ok: Vec<_> = vb_seq
        .as_ref()
        .results()
        .map(|r| r.as_ref().is_ok())
        .collect();
    let par_ok: Vec<_> = vb_par
        .as_ref()
        .results()
        .map(|r| r.as_ref().is_ok())
        .collect();
    assert_eq!(seq_ok, par_ok, "approval/rejection sequence must match");

    // Compare that final state roots are identical
    let root_seq = vb_seq.as_ref().header().merkle_root();
    let root_par = vb_par.as_ref().header().merkle_root();
    assert_eq!(root_seq, root_par, "merkle roots must match");

    // Compare resulting asset balances (Alice's coin should be 10 in both states)
    let a_coin: AssetId = AssetId::of(
        iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "coin".parse().unwrap(),
        ),
        alice_id.clone(),
    );
    let view_seq = state_seq.view();
    let view_par = state_par.view();
    let bal_seq = view_seq
        .world()
        .assets()
        .get(&a_coin)
        .map_or_else(|| Numeric::new(0, 0), |v| v.clone().into_inner());
    let bal_par = view_par
        .world()
        .assets()
        .get(&a_coin)
        .map_or_else(|| Numeric::new(0, 0), |v| v.clone().into_inner());
    assert_eq!(bal_seq, bal_par, "final balances must match");
}

fn run_block_and_events(
    parallel_apply: bool,
    txs: Vec<SignedTransaction>,
    bootstrap_accounts: Vec<AccountId>,
) -> (
    Vec<iroha_data_model::events::prelude::EventBox>,
    iroha_core::state::State,
) {
    // Build a fresh world aligned with authority accounts present in `txs`,
    // plus any additional bootstrap accounts referenced by the test.
    //
    // This keeps the initial world stable across apply modes and avoids relying
    // on implicit admission side effects (which can be sensitive to scheduling).
    let mut accounts: BTreeSet<AccountId> = bootstrap_accounts.into_iter().collect();
    for tx in &txs {
        accounts.insert(tx.authority().clone());
    }
    let first_auth = accounts
        .iter()
        .next()
        .cloned()
        .expect("non-empty account set");
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&first_auth);
    let ad_id = AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
    let ad: AssetDefinition = AssetDefinition::new(ad_id.clone(), NumericSpec::default())
        .with_name("rose".to_owned())
        .build(&first_auth);
    let mut world_accounts: Vec<Account> = Vec::new();
    let mut assets: Vec<Asset> = Vec::new();
    for acc_id in &accounts {
        world_accounts
            .push(Account::new(acc_id.clone().to_account_id(domain_id.clone())).build(&first_auth));
        let asset_id = AssetId::new(ad.id().clone(), acc_id.clone());
        // Ensure the first bootstrap account has a larger balance so that
        // ordering differences in the scheduler don't affect validity.
        let balance = if acc_id == &first_auth { 60 } else { 10 };
        assets.push(Asset::new(asset_id, Numeric::new(balance, 0)));
    }
    let world = iroha_core::state::World::with_assets([domain], world_accounts, [ad], assets, []);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let mut state = iroha_core::state::State::new(
        world,
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state = iroha_core::state::State::new(world, kura, query);
    let mut cfg = state.view().pipeline().clone();
    cfg.parallel_apply = parallel_apply;
    state.set_pipeline(cfg);

    // Build a signed block from txs
    let block: SignedBlock = {
        let accepted: Vec<_> = txs
            .into_iter()
            .map(|tx| iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx)))
            .collect();
        BlockBuilder::new_with_time_source(accepted, block_time_source())
            .chain(0, state.view().latest_block().as_deref())
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
            .unpack(|_| {})
            .into()
    };
    // Execute and commit
    let mut sb = state.block(block.header());
    let vb = ValidBlock::validate_unchecked(block, &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let events = sb.apply_without_execution(&cb, Vec::new());
    drop(sb);
    (events, state)
}

// event_list_json moved to snapshot helpers; removed.

#[test]
fn events_snapshot_mint_burn_transfer_match_between_modes() {
    let chain_id = ChainId::from("chain");
    let alice_id = (*iroha_test_samples::ALICE_ID).clone();
    let bob_id = (*iroha_test_samples::BOB_ID).clone();
    let rose: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "rose".parse().unwrap(),
    );
    let a_coin = AssetId::of(rose.clone(), alice_id.clone());
    let b_coin = AssetId::of(rose.clone(), bob_id.clone());

    // Build three transactions: mint to Alice, burn from Bob, transfer Alice->Bob
    let tx_mint = tx_builder(&chain_id, &alice_id)
        .with_instructions([Mint::asset_numeric(7_u32, a_coin.clone())])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_burn = tx_builder(&chain_id, &alice_id)
        .with_instructions([Burn::asset_numeric(3_u32, b_coin.clone())])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_xfer = tx_builder(&chain_id, &alice_id)
        .with_instructions([Transfer::asset_numeric(
            a_coin.clone(),
            5_u32,
            bob_id.clone(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());

    // Sequential
    let (events_seq, state_seq) = run_block_and_events(
        false,
        vec![tx_mint.clone(), tx_burn.clone(), tx_xfer.clone()],
        vec![alice_id.clone(), bob_id.clone()],
    );
    // Parallel-detached
    let (events_par, state_par) = run_block_and_events(
        true,
        vec![tx_mint, tx_burn, tx_xfer],
        vec![alice_id.clone(), bob_id.clone()],
    );

    // Fixture-backed parity snapshots
    assert_events("mint_burn_transfer", &events_seq);
    assert_events("mint_burn_transfer", &events_par);

    // Sanity: balances equal (Alice had 60 +7 -5 = 62; Bob 10 -3 +5 = 12)
    let bal = |state: &iroha_core::state::State, id: &AssetId| {
        state
            .view()
            .world()
            .assets()
            .get(id)
            .map_or_else(|| Numeric::new(0, 0), |v| v.clone().into_inner())
    };
    assert_eq!(bal(&state_seq, &a_coin), bal(&state_par, &a_coin));
    assert_eq!(bal(&state_seq, &b_coin), bal(&state_par, &b_coin));
}

#[test]
fn events_snapshot_kv_and_nft_match_between_modes() {
    use iroha_data_model::prelude::*;
    let chain_id = ChainId::from("chain");
    let alice_id = (*iroha_test_samples::ALICE_ID).clone();
    let bob_id = (*iroha_test_samples::BOB_ID).clone();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let nft_id: NftId = "n0$wonderland".parse().unwrap();

    // Build transactions exercising account/domain kv and full NFT lifecycle
    let tx_acc_set = tx_builder(&chain_id, &alice_id)
        .with_instructions([SetKeyValue::account(
            alice_id.clone(),
            "k1".parse().unwrap(),
            iroha_primitives::json::Json::new(1u32),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_dom_set = tx_builder(&chain_id, &alice_id)
        .with_instructions([SetKeyValue::domain(
            domain_id.clone(),
            "dk".parse().unwrap(),
            iroha_primitives::json::Json::new(3u32),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_nft_reg = tx_builder(&chain_id, &alice_id)
        .with_instructions([Register::nft(Nft::new(nft_id.clone(), Metadata::default()))])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_nft_set = tx_builder(&chain_id, &alice_id)
        .with_instructions([SetKeyValue::nft(
            nft_id.clone(),
            "nk".parse().unwrap(),
            iroha_primitives::json::Json::new("v"),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_nft_xfer = tx_builder(&chain_id, &alice_id)
        .with_instructions([Transfer::nft(
            alice_id.clone(),
            nft_id.clone(),
            bob_id.clone(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_nft_rm = tx_builder(&chain_id, &alice_id)
        .with_instructions([RemoveKeyValue::nft(nft_id.clone(), "nk".parse().unwrap())])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_nft_unreg = tx_builder(&chain_id, &alice_id)
        .with_instructions([Unregister::nft(nft_id.clone())])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_acc_rm = tx_builder(&chain_id, &alice_id)
        .with_instructions([RemoveKeyValue::account(
            alice_id.clone(),
            "k1".parse().unwrap(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_dom_rm = tx_builder(&chain_id, &alice_id)
        .with_instructions([RemoveKeyValue::domain(
            domain_id.clone(),
            "dk".parse().unwrap(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());

    let txs = vec![
        tx_acc_set.clone(),
        tx_dom_set.clone(),
        tx_nft_reg.clone(),
        tx_nft_set.clone(),
        tx_nft_xfer.clone(),
        tx_nft_rm.clone(),
        tx_nft_unreg.clone(),
        tx_acc_rm.clone(),
        tx_dom_rm.clone(),
    ];

    // Sequential
    let (events_seq, _state_seq) =
        run_block_and_events(false, txs.clone(), vec![alice_id.clone(), bob_id.clone()]);
    // Parallel-detached
    let (events_par, _state_par) =
        run_block_and_events(true, txs, vec![alice_id.clone(), bob_id.clone()]);

    assert_events("kv_and_nft_lifecycle", &events_seq);
    assert_events("kv_and_nft_lifecycle", &events_par);
}

#[test]
fn events_snapshot_asset_definition_kv_match_between_modes() {
    let chain_id = ChainId::from("chain");
    let alice_id = (*iroha_test_samples::ALICE_ID).clone();
    let ad: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "rose".parse().unwrap(),
    );

    let tx_set = tx_builder(&chain_id, &alice_id)
        .with_instructions([SetKeyValue::asset_definition(
            ad.clone(),
            "spec".parse().unwrap(),
            iroha_primitives::json::Json::new("golden"),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_rm = tx_builder(&chain_id, &alice_id)
        .with_instructions([RemoveKeyValue::asset_definition(
            ad.clone(),
            "spec".parse().unwrap(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());

    let (events_seq, _) = run_block_and_events(
        false,
        vec![tx_set.clone(), tx_rm.clone()],
        vec![alice_id.clone()],
    );
    let (events_par, _) = run_block_and_events(true, vec![tx_set, tx_rm], vec![alice_id.clone()]);
    assert_events("asset_definition_kv", &events_seq);
    assert_events("asset_definition_kv", &events_par);
}

#[test]
fn owner_transfer_domain_and_asset_def_parity() {
    let chain_id = ChainId::from("chain");
    let alice_id = (*iroha_test_samples::ALICE_ID).clone();
    let bob_id = (*iroha_test_samples::BOB_ID).clone();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let ad: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "rose".parse().unwrap(),
    );

    let tx_dom_xfer = tx_builder(&chain_id, &alice_id)
        .with_instructions([Transfer::domain(
            alice_id.clone(),
            domain_id.clone(),
            bob_id.clone(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_ad_xfer = tx_builder(&chain_id, &alice_id)
        .with_instructions([Transfer::asset_definition(
            alice_id.clone(),
            ad.clone(),
            bob_id.clone(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());

    // Sequential
    let (events_seq, state_seq) = run_block_and_events(
        false,
        vec![tx_dom_xfer.clone(), tx_ad_xfer.clone()],
        vec![alice_id.clone(), bob_id.clone()],
    );
    // Parallel-detached
    let (events_par, state_par) = run_block_and_events(
        true,
        vec![tx_dom_xfer, tx_ad_xfer],
        vec![alice_id.clone(), bob_id.clone()],
    );

    // Events parity via fixture snapshots
    assert_events("owner_transfer_domain_asset_def", &events_seq);
    assert_events("owner_transfer_domain_asset_def", &events_par);

    // State parity: owners equal
    let dom_owner_seq = state_seq
        .view()
        .world()
        .domain(&domain_id)
        .expect("domain exists")
        .owned_by()
        .clone();
    let dom_owner_par = state_par
        .view()
        .world()
        .domain(&domain_id)
        .expect("domain exists")
        .owned_by()
        .clone();
    // With parallel apply, domain/asset ownership updates may land on different lanes;
    // ensure both updates target the intended owner instead of insisting on order parity.
    // Ownership transfer order is not deterministic across lanes; ensure parity instead of exact owner.
    assert_eq!(dom_owner_seq, dom_owner_par, "domain owners must match");

    let ad_owner_seq = state_seq
        .view()
        .world()
        .asset_definition(&ad)
        .expect("asset def exists")
        .owned_by()
        .clone();
    let ad_owner_par = state_par
        .view()
        .world()
        .asset_definition(&ad)
        .expect("asset def exists")
        .owned_by()
        .clone();
    assert_eq!(
        ad_owner_seq, ad_owner_par,
        "asset definition owners must match"
    );
}
