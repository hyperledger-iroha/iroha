//! Genuine retained suffix originals through BuildPayload and StateExecutor.
//! No activation row, proof, permission, result or QC is injected. This component timing
//! fixture is not a running validator network or a physical-device capacity test.
use super::*;
use crate::sumeragi::test_chain::{CertifiedTestChain, Signers, kagemusha_setup};
use iroha_core_zk::kagemusha_wallet_artifacts_v1::{VERIFIER_PACK_MAX_BYTES_V1, VerifierPackV1};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    asset::{AssetBalanceScope, AssetId},
    isi::{
        InstructionBox, Transfer,
        kagemusha_wallet::{KagemushaWalletLedgerActionV1 as Action, KagemushaWalletLedgerV1},
    },
    kagemusha::{
        KAGEMUSHA_WALLET_ACTIVATION_MAX_BYTES_V1, KagemushaWalletActivationV1,
        KagemushaWalletAssetScopeV1,
        kagemusha_wallet_suffix_unload_v1::{
            KAGEMUSHA_WALLET_SUFFIX_ANCHOR_MAX_BYTES_V1,
            KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1,
            KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1, KagemushaWalletSuffixAnchorV1 as Anchor,
            KagemushaWalletSuffixChunkV1 as Chunk, KagemushaWalletSuffixHeaderV1 as Header,
            kagemusha_wallet_suffix_authority_digest_v1,
        },
    },
    transaction::SignedTransaction,
};
use iroha_primitives::{
    numeric::{Numeric, Quantity},
    time::TimeSource,
};
use mv::storage::StorageReadOnly as _;
use std::{path::Path, time::Instant};
#[path = "../kagemusha_wallet_v1/tests/suffix_performance_cpu.rs"]
mod performance;

fn read(path: &Path, maximum: usize) -> Vec<u8> {
    assert!(std::fs::metadata(path).unwrap().len() <= maximum as u64);
    std::fs::read(path).unwrap()
}
fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}
fn sign(chain: &CertifiedTestChain, seed: u8, instruction: InstructionBox) -> SignedTransaction {
    chain.sign(&key(seed), [instruction], (chain.height() + 1) * 1_000 - 1)
}
fn commit(chain: &mut CertifiedTestChain, seed: u8, instruction: InstructionBox) {
    let signed = sign(chain, seed, instruction);
    assert_eq!(
        chain.commit_at((chain.height() + 1) * 1_000, vec![signed]),
        vec![true]
    );
}
fn cpu_delta(before: Option<u128>) -> String {
    before.zip(performance::cpu_ns()).map_or_else(
        || "unavailable".to_owned(),
        |(before, after)| after.saturating_sub(before).to_string(),
    )
}

#[test]
#[ignore = "requires genuine stages and the same merchant's authentic Native activation original"]
fn genuine_suffix_multi_append_payload_and_execution_budget() {
    crate::sumeragi::threads::sumeragi_thread_builder("suffix-performance")
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}

fn run() {
    let directory = std::path::PathBuf::from(
        std::env::var_os("KAGEMUSHA_GENUINE_SUFFIX_STAGES").expect("genuine stage directory"),
    );
    let activation_path = std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_GENUINE_SUFFIX_ACTIVATION")
        .expect("actual Native-authorized activation of the retained merchant is required; never seed Active"));
    let header_bytes = read(
        &directory.join("header.norito"),
        KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1,
    );
    let header: Header = norito::decode_canonical_with_limits(
        &header_bytes,
        norito::canonical_decode_limits(KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1),
    )
    .unwrap();
    header.validate().unwrap();
    let anchor_bytes = read(
        &directory.join("anchor.norito"),
        KAGEMUSHA_WALLET_SUFFIX_ANCHOR_MAX_BYTES_V1,
    );
    let anchor = Anchor::decode_canonical(&anchor_bytes, &header.scheme_id).unwrap();
    anchor.require_header(&header).unwrap();
    let activation_bytes = read(&activation_path, KAGEMUSHA_WALLET_ACTIVATION_MAX_BYTES_V1);
    let activation =
        KagemushaWalletActivationV1::decode_canonical(&activation_bytes, &header.scheme_id)
            .unwrap();
    assert_eq!(
        activation.credential, anchor.credential,
        "same authentic wallet, never a substitute activation"
    );
    let pack_bytes = read(
        &directory.join("verifier-pack.norito"),
        VERIFIER_PACK_MAX_BYTES_V1,
    );
    let pack = VerifierPackV1::decode_canonical(&pack_bytes).unwrap();
    let asset_bytes = read(&directory.join("asset.norito"), 4_096);
    let asset: KagemushaWalletAssetScopeV1 =
        norito::decode_canonical_with_limits(&asset_bytes, norito::canonical_decode_limits(4_096))
            .unwrap();
    let mut setup = kagemusha_setup::start();
    assert_eq!(
        &header.network_id,
        setup.chain.network_id().as_bytes(),
        "same signed genesis"
    );
    assert_eq!(
        setup.asset, asset,
        "same executed asset incarnation and scale"
    );
    assert_eq!(activation.asset, asset);
    assert_eq!(
        header.account, setup.recipients[0],
        "retained merchant account seed 42"
    );
    let instruction = |action| KagemushaWalletLedgerV1::new(header.scheme_id, action).into();
    commit(
        &mut setup.chain,
        95,
        instruction(Action::Register {
            scheme: pack.scheme.clone(),
            asset: asset_bytes,
            reserve: setup.reserve.clone(),
            balance_scope: AssetBalanceScope::Global,
        }),
    );
    commit(
        &mut setup.chain,
        95,
        instruction(Action::InstallVerifierPack {
            asset: asset.asset_digest(),
            manifest_digest: header.artifact_manifest,
            pack: pack_bytes,
        }),
    );
    commit(
        &mut setup.chain,
        42,
        instruction(Action::Activate(activation_bytes)),
    );
    let amount = header.payout_terms().unwrap().amount;
    assert!(
        amount <= 1_000,
        "fixture must fund using its genuine genesis supply"
    );
    commit(
        &mut setup.chain,
        41,
        Transfer::asset_quantity(
            AssetId::of(asset.asset.clone(), setup.account.clone()),
            Quantity::from_canonical_numeric(Numeric::new(amount, asset.scale)).unwrap(),
            setup.reserve.clone(),
        )
        .into(),
    );
    let authority = kagemusha_wallet_suffix_authority_digest_v1(&header_bytes).unwrap();
    commit(
        &mut setup.chain,
        42,
        instruction(Action::SuffixUnloadBegin {
            wallet: header.wallet_id,
            authority,
            account: header.account.clone(),
            ordinal: 0,
            header: header_bytes,
            anchor: anchor_bytes,
        }),
    );
    // Two actual 8-step Appends are enough to test realistic shared-block work without
    // rerunning the full 100-step dispatch gate. Quotas and genesis limits stay unchanged.
    assert!(header.chunk_count >= 2);
    let mut steps = 0;
    let appends = (0..2_u128)
        .map(|index| {
            let chunk = read(
                &directory.join(format!("chunk-{index:03}.norito")),
                KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1,
            );
            steps += Chunk::decode_canonical(&chunk, &header.scheme_id)
                .unwrap()
                .decode_steps(&header.scheme_id)
                .unwrap()
                .len();
            instruction(Action::SuffixUnloadAppend {
                wallet: header.wallet_id,
                authority,
                account: header.account.clone(),
                ordinal: index + 1,
                chunk,
            })
        })
        .collect::<Vec<InstructionBox>>();
    let balance = |chain: &CertifiedTestChain| {
        chain
            .state()
            .view()
            .world()
            .assets()
            .get(&AssetId::of(asset.asset.clone(), header.account.clone()))
            .map(|value| value.as_ref().clone())
    };
    let before = balance(&setup.chain);
    for label in ["multi_append_first", "multi_append_duplicate"] {
        let signed = appends
            .iter()
            .cloned()
            .map(|action| sign(&setup.chain, 42, action))
            .collect::<Vec<_>>();
        let proposal = build(&setup.chain, signed, label, steps);
        // Author the real charged availability source before timing StateExecutor, so
        // availability encoding, certification and publication have separate boundaries.
        let available = setup
            .chain
            .author_proposal(proposal, iroha_sumeragi::types::ControlWitness::default());
        let cpu = performance::cpu_ns();
        let started = Instant::now();
        let result = setup.chain.begin_available_proposal(available);
        eprintln!(
            "SUFFIX_MULTI_PERF label={label} phase=state_executor steps={steps} transactions=2 success={} wall_ns={} cpu_ns={}",
            result.is_ok(),
            started.elapsed().as_nanos(),
            cpu_delta(cpu)
        );
        let mut pending =
            result.expect("real StateExecutor executes the exact BuildPayload proposal");
        let committed = pending
            .publish(Signers::Quorum)
            .expect("actual result receives the real fixture committee QC and publishes");
        assert!(
            committed
                .block()
                .output_results()
                .all(|result| result.is_ok()),
            "both actual Appends execute successfully"
        );
        drop(pending);
        assert_eq!(
            balance(&setup.chain),
            before,
            "Append never pays the account"
        );
    }
}

fn build(
    chain: &CertifiedTestChain,
    signed: Vec<SignedTransaction>,
    label: &'static str,
    steps: usize,
) -> SignedBlock {
    let height = chain.height() + 1;
    let (_, time) = TimeSource::new_mock(Duration::from_millis(height * 1_000));
    let view = chain.state().view();
    let schedule = view.world().consensus_schedule().ready(height).unwrap();
    let local = iroha_sumeragi::api::LocalParams::default();
    let pacemaker = iroha_sumeragi::pacemaker::Pacemaker::new(&local, local.t_max);
    let e_max = schedule.params.exec_budget_ms;
    let budget = pacemaker.exec_budget(e_max);
    let max_bytes = view.world().parameters().sumeragi().max_block_bytes.get();
    eprintln!(
        "SUFFIX_PERF_CONTEXT label={label} steps={steps} transactions={} configured_e_max_ms={e_max} local_t_base_ms={} build_budget_ms={budget} max_block_bytes={max_bytes} max_calls_per_tx={} max_calls_per_block={} max_proof_bytes_block={} local_policy=fixture_default validity_timeout=false",
        signed.len(),
        local.t_base,
        view.zk().max_verify_calls_per_tx,
        view.zk().max_verify_calls_per_block,
        view.zk().max_proof_bytes_block
    );
    drop(view);
    let mut built = None;
    publication_tests::with_worker_chain_and_clock(
        chain,
        ConsensusMode::Permissioned,
        Arc::new(crate::sumeragi::lanes::merge::NoLanes),
        time.clone(),
        |chain, worker, _, _| {
            let queue = Arc::new(Queue::test(
                iroha_config::parameters::actual::Queue::default(),
                &time,
            ));
            let mut expected = Vec::new();
            for signed in signed {
                let accepted = crate::tx::AcceptedTransaction::accept_with_time_source(
                    signed,
                    &chain.network_id(),
                    Duration::from_secs(1),
                    chain.state().view().world().parameters().transaction(),
                    chain
                        .state()
                        .transaction_admission_parameters()
                        .require_crypto_policy()
                        .unwrap(),
                    &time,
                )
                .expect("real transaction admission under the signed genesis crypto policy");
                expected.push(accepted.hash_as_entrypoint());
                queue.push(accepted, chain.state().view()).unwrap();
            }
            worker.queue = Some(queue);
            let cpu = performance::cpu_ns();
            let started = Instant::now();
            let result = worker.build(height, 0, max_bytes, u32::try_from(budget).unwrap());
            eprintln!(
                "SUFFIX_MULTI_PERF label={label} phase=build_payload steps={steps} transactions=2 success={} wall_ns={} cpu_ns={} build_budget_ms={budget}",
                matches!(result, Ok(Some(_))),
                started.elapsed().as_nanos(),
                cpu_delta(cpu)
            );
            let bytes = result
                .unwrap()
                .expect("actual BuildPayload selects queued suffix stages");
            let proposal = payload::decode(bytes.payload.as_slice()).unwrap();
            let actual = proposal
                .external_entrypoints_slice()
                .iter()
                .map(|entry| entry.hash())
                .collect::<Vec<_>>();
            assert_eq!(
                actual.len(),
                expected.len(),
                "the actual payload contains both Appends"
            );
            assert!(
                expected.iter().all(|hash| actual.contains(hash)),
                "exact signed originals, never replacement transactions"
            );
            built = Some(proposal);
        },
    );
    built.unwrap()
}
