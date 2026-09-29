//! Current archive ownership after State publication, using genuine four-validator certificates.
//!
//! Archive failure retains an already committed decision; retry may capture it, but cannot execute or notify it twice.
use super::*;
use crate::{
    query::{
        provider_ingest_finalized::{
            ProviderIngestFinalizedArchiveBoundsV1, ProviderIngestFinalizedArchiveV1,
        },
        reputation_finalized::{ReputationFinalizedArchive, ReputationFinalizedArchiveBounds},
    },
    state::World,
    sumeragi::{
        block_store::KuraBlockStore,
        crypto::BlsCrypto,
        driver::traits::BlockStore as _,
        test_chain::{CertifiedTestChain, Signers, TestChainConfig},
    },
};
use iroha_data_model::{
    isi::sorafs::SetSorafsReputationJournalAuthorityPolicy,
    sorafs::reputation::{
        REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1, REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1,
        ReputationJournalAuthorityPolicyV1,
    },
};
use iroha_executor_data_model::permission::sorafs::CanManageSorafsReputationJournalPolicy;

fn chain() -> CertifiedTestChain {
    use iroha_data_model::{
        account::Account,
        asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId},
        isi::{
            Grant, Register,
            sorafs::{SetSorafsOrderbookPolicy, SetSorafsReservePolicy},
        },
        sorafs::{
            orderbook::{ORDERBOOK_ADMISSION_POLICY_VERSION_V1, OrderbookAdmissionPolicyV1},
            reserve::{
                RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyV1, ReservePolicyV1,
            },
        },
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanSetSorafsPricing, CanSetSorafsReservePolicy,
    };
    let mut config = TestChainConfig::new(World::new(), 1_750_000_000_000);
    let authority = AccountId::new(config.genesis_key.public_key().clone());
    let custody = iroha_test_samples::ALICE_ID.clone();
    let asset_id = AssetDefinitionId::derive_from_components(
        iroha_genesis::GENESIS_DOMAIN_ID.clone(),
        "reserve".parse().unwrap(),
    );
    // Every feed capture queries has its actual governed policy, enacted by signed genesis.
    config.genesis_instructions.extend([
        Grant::account_permission(CanManageSorafsReputationJournalPolicy, authority.clone()).into(),
        SetSorafsReputationJournalAuthorityPolicy::new(ReputationJournalAuthorityPolicyV1 {
            version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            por_recorder_authority: authority.clone(),
            dispute_recorder_authority: authority.clone(),
            token_recorder_authority: authority.clone(),
            max_source_age_ms: REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1,
        })
        .into(),
        Grant::account_permission(CanSetSorafsPricing, authority.clone()).into(),
        SetSorafsOrderbookPolicy::new(OrderbookAdmissionPolicyV1 {
            version: ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            market_id: [0xA5; 32],
            matcher_authority: authority.clone(),
            settlement_authority: authority.clone(),
            paused: false,
            min_order_gib: 1,
            max_order_gib: 1024,
            price_tick_micro_xor: 10,
            max_maker_fee_bps: 100,
            max_taker_fee_bps: 200,
            max_order_lifetime_secs: 3600,
            max_receipt_age_secs: 300,
            max_clock_skew_secs: 5,
            max_receipt_bytes: 1024,
            max_receipts_per_channel: 2,
        })
        .into(),
        Register::account(Account::new(custody.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(
            asset_id.clone(),
            "Reserve".to_owned(),
            AssetBalancePolicy::Global,
            None,
        ))
        .into(),
        Grant::account_permission(CanSetSorafsReservePolicy, authority.clone()).into(),
        SetSorafsReservePolicy::new(ReserveAuthorityPolicyV1 {
            version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            economics: ReservePolicyV1::default(),
            asset_definition: asset_id,
            custody_account: custody,
            treasury_account: authority.clone(),
            operations_authority: authority.clone(),
            decision_authority: authority,
            grace_period_days: 7,
            default_after_days: 30,
            max_provider_debt: sorafs_manifest::deal::XorQuantity::try_from_micro(1_000_000_000)
                .unwrap(),
            max_pending_movements_per_provider: 4,
            max_open_appeals_per_provider: 2,
        })
        .into(),
    ]);
    CertifiedTestChain::start(config).unwrap()
}

fn provider_bounds() -> ProviderIngestFinalizedArchiveBoundsV1 {
    ProviderIngestFinalizedArchiveBoundsV1::try_new(4 << 20, 32, 64 << 20, 32, 32, 128, 32).unwrap()
}

fn archives() -> (tempfile::TempDir, FinalizedArchives) {
    let directory = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let provider = ProviderIngestFinalizedArchiveV1::try_open(
        directory.path().join("provider"),
        provider_bounds(),
    )
    .unwrap();
    let reputation = ReputationFinalizedArchive::try_open(
        directory.path().join("reputation"),
        ReputationFinalizedArchiveBounds::try_new(4 << 20, 32, 64 << 20).unwrap(),
    )
    .unwrap();
    (
        directory,
        FinalizedArchives {
            provider_ingest: Some(Arc::new(provider)),
            reputation: Some(Arc::new(reputation)),
        },
    )
}

fn archive_files(root: &std::path::Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn walk(
        root: &std::path::Path,
        path: &std::path::Path,
        files: &mut BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    std::fs::read(&path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

fn context(
    chain: &CertifiedTestChain,
) -> (ExecutorContext, tokio::sync::broadcast::Receiver<EventBox>) {
    let (events, receiver) = tokio::sync::broadcast::channel(1024);
    let crypto = Arc::new(BlsCrypto::new());
    crypto
        .admit_committee(
            chain
                .validators()
                .iter()
                .map(|(peer, pop)| (peer.public_key(), pop.as_slice())),
        )
        .unwrap();
    (
        ExecutorContext {
            state: Arc::clone(chain.state()),
            native_context_archive: Arc::new(
                NativeContextArchive::open(
                    chain.kura(),
                    chain.state().ivm_execution_budget(),
                    chain.kura().native_context_archive_max_bytes(),
                )
                .unwrap(),
            ),
            queue: None,
            staging: Staging::new(),
            events,
            genesis_account: chain.genesis_account().clone(),
            consensus_mode: ConsensusMode::Permissioned,
            applied: (1, chain.committed(1).core_hash()),
            crypto: Some(crypto),
            applied_watch: Arc::new(crate::sumeragi::lanes::global::AppliedWatch::new(1, None)),
            lane_blocks: std::sync::Arc::new(crate::sumeragi::lanes::merge::NoLanes),
        },
        receiver,
    )
}

fn worker(context: &ExecutorContext, archives: FinalizedArchives) -> Worker<'_> {
    Worker {
        context,
        state: &context.state,
        applied: context.applied,
        live: None,
        finishing: None,
        recovery: None,
        results: BTreeMap::new(),
        last_built: None,
        queue: context.queue.clone(),
        beacon: None,
        archives: Some(archives),
        pending_commit: None,
        attestation: None,
        quarantine_context: None,
    }
}

fn prepare_and_append(worker: &mut Worker<'_>, block: &Block, qc: &Qc) -> usize {
    assert_eq!(worker.prepare(block, qc).unwrap(), Some(qc.result));
    let original = worker
        .live
        .as_ref()
        .unwrap()
        .native_contexts
        .as_ref()
        .unwrap()
        .canonical_bytes()
        .as_ptr() as usize;
    KuraBlockStore::new(
        worker.state.kura_handle(),
        worker.context.crypto.as_ref().unwrap().clone(),
        1,
        worker.context.staging.clone(),
        worker.state.ivm_execution_budget(),
    )
    .append(block, qc)
    .unwrap();
    original
}

fn hold_archive_completion(worker: &mut Worker<'_>, block: &Block, qc: &Qc) {
    let original = prepare_and_append(worker, block, qc);
    let root = worker
        .archives
        .as_ref()
        .unwrap()
        .reputation
        .as_ref()
        .unwrap()
        .root()
        .to_owned();
    let hidden = root.with_extension("temporarily-unavailable");
    std::fs::rename(&root, &hidden).unwrap();
    let error = worker.commit(block, qc).unwrap_err();
    std::fs::rename(&hidden, &root).unwrap();
    assert!(
        error
            .to_string()
            .contains("reputation archive capture failed"),
        "{error}"
    );
    let pending = worker.pending_commit.as_ref().unwrap();
    assert_eq!(
        pending.native_contexts.canonical_bytes().as_ptr() as usize,
        original
    );
    assert!(worker.live.as_ref().unwrap().overlay.is_none());
}

fn run(test: impl FnOnce() + Send + 'static) {
    crate::sumeragi::threads::sumeragi_thread_builder("sumeragi-archive-test")
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn partial_archive_failure_retains_exact_decision_and_retries_without_reexecution_or_notifications()
{
    run(partial_archive_failure_retains_exact_decision_and_retries_without_reexecution_or_notifications_case);
}

fn partial_archive_failure_retains_exact_decision_and_retries_without_reexecution_or_notifications_case()
 {
    let chain = chain();
    let (directory, archives) = archives();
    archives.capture(&chain.state().view()).unwrap();
    let provider = archives.provider_ingest.as_ref().unwrap();
    let reputation = archives.reputation.as_ref().unwrap();
    let provider_before = provider.health_generation().unwrap();
    let reputation_before = reputation.health_generation().unwrap();
    let (context, mut events) = context(&chain);
    let mut worker = worker(&context, archives.clone());
    let (block, qc) = super::publication_tests::executed(&chain, &mut worker);
    let proposal = payload::decode(&block.payload).unwrap();
    let TransactionEntrypoint::External(transaction) = &proposal.external_entrypoints_slice()[0]
    else {
        panic!("real fixture carries its signed external transaction")
    };
    let transaction = transaction.clone();
    let entry_hash = transaction.hash_as_entrypoint();
    let (_clock, time) =
        iroha_primitives::time::TimeSource::new_mock(proposal.header().creation_time());
    let queue = Arc::new(Queue::test(
        iroha_config::parameters::actual::Queue::default(),
        &time,
    ));
    let accepted = crate::tx::AcceptedTransaction::accept_with_time_source(
        transaction,
        &chain.network_id(),
        Duration::from_secs(1),
        chain.state().view().world().parameters().transaction(),
        &iroha_config::parameters::actual::Crypto::default(),
        &time,
    )
    .unwrap();
    queue.push(accepted, chain.state().view()).unwrap();
    worker.queue = Some(Arc::clone(&queue));
    assert!(queue.contains_entrypoint_hash(entry_hash));
    let original_contexts = prepare_and_append(&mut worker, &block, &qc);
    let staged = context.staging.get(&qc.block_hash).unwrap();
    assert!(
        staged
            .executed
            .network_output_at(0)
            .is_some_and(|(_, output)| output.result.is_ok()),
        "the original queued transaction executes successfully"
    );
    assert_eq!(
        worker.state.view().height(),
        1,
        "append does not publish State"
    );
    let hash = staged.executed.hash();
    let input_hashes: Vec<_> = staged
        .executed
        .external_entrypoints_slice()
        .iter()
        .map(TransactionEntrypoint::hash)
        .collect();
    assert!(!input_hashes.is_empty());
    assert_eq!(provider.health_generation().unwrap(), provider_before);
    assert_eq!(reputation.health_generation().unwrap(), reputation_before);
    let root = reputation.root().to_owned();
    let hidden = root.with_extension("temporarily-unavailable");
    std::fs::rename(&root, &hidden).unwrap();
    let error = worker.commit(&block, &qc).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("reputation archive capture failed"),
        "{error}"
    );
    let retained_events = worker.pending_commit.as_ref().unwrap().events.len();
    assert!(retained_events > 0);
    assert_eq!(
        worker
            .pending_commit
            .as_ref()
            .unwrap()
            .native_contexts
            .canonical_bytes()
            .as_ptr() as usize,
        original_contexts
    );
    assert_eq!(provider.health_generation().unwrap(), provider_before + 1);
    let original_provider_files = archive_files(&directory.path().join("provider"));
    assert!(!original_provider_files.is_empty());
    assert!(
        reputation.health_generation().is_err(),
        "the actual archive directory is unavailable"
    );
    assert_eq!(worker.applied.0, 1);
    assert_eq!(worker.state.view().height(), 2);
    assert_eq!(worker.state.view().latest_block_hash(), Some(hash));
    assert_eq!(worker.pending_commit.as_ref().unwrap().hashes, input_hashes);
    assert!(
        queue.contains_entrypoint_hash(entry_hash),
        "archive failure cannot clean committed queue work"
    );
    assert!(events.try_recv().is_err());
    assert!(worker.execute(&block, qc.block_hash).is_none());
    assert_eq!(worker.build(3, 0, 1 << 20), (vec![], false));
    assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(qc.result));
    assert!(
        worker.live.as_ref().unwrap().overlay.is_none(),
        "retry must not acquire another State overlay"
    );
    std::fs::rename(&hidden, &root).unwrap();
    assert_eq!(reputation.health_generation().unwrap(), reputation_before);
    let completed = worker.commit(&block, &qc).unwrap();
    assert_eq!(worker.applied, (2, qc.block_hash));
    assert!(worker.pending_commit.is_none());
    assert!(
        !queue.contains_entrypoint_hash(entry_hash),
        "successful capture retires the original queue entry"
    );
    assert_eq!(
        archive_files(&directory.path().join("provider")),
        original_provider_files
    );
    assert_eq!(provider.health_generation().unwrap(), provider_before + 1);
    assert_eq!(
        reputation.health_generation().unwrap(),
        reputation_before + 1
    );
    assert_eq!(worker.state.view().height(), 2);
    assert_eq!(worker.state.view().latest_block_hash(), Some(hash));
    let mut emitted = 0;
    while events.try_recv().is_ok() {
        emitted += 1;
    }
    assert_eq!(
        emitted, retained_events,
        "publish every original real event once"
    );
    assert!(events.try_recv().is_err());
    assert_eq!(worker.commit(&block, &qc).unwrap(), completed);
    assert!(events.try_recv().is_err(), "completion cannot emit twice");
    assert_eq!(provider.health_generation().unwrap(), provider_before + 1);
}

#[test]
fn pending_capture_rejects_substituted_header_qc_state_and_missing_certificate() {
    run(pending_capture_rejects_substituted_header_qc_state_and_missing_certificate_case);
}

fn pending_capture_rejects_substituted_header_qc_state_and_missing_certificate_case() {
    let chain = chain();
    let (_directory, archives) = archives();
    archives.capture(&chain.state().view()).unwrap();
    let (context, mut events) = context(&chain);
    let mut worker = worker(&context, archives.clone());
    let (block, qc) = super::publication_tests::executed(&chain, &mut worker);
    hold_archive_completion(&mut worker, &block, &qc);
    let mut wrong_block = block.clone();
    wrong_block.header.height += 1;
    assert!(worker.prepare(&wrong_block, &qc).is_err());
    assert!(worker.commit(&wrong_block, &qc).is_err());
    let mut wrong_qc = qc.clone();
    wrong_qc.view += 1;
    assert!(worker.prepare(&block, &wrong_qc).is_err());
    assert!(worker.commit(&block, &wrong_qc).is_err());
    worker.pending_commit.as_mut().unwrap().state_hash =
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"foreign state"));
    assert!(worker.commit(&block, &qc).is_err());
    worker.pending_commit.as_mut().unwrap().state_hash = chain.committed(2).block_hash();
    assert!(events.try_recv().is_err());
    let retained = worker.pending_commit.take().unwrap();
    drop(worker);
    // A negative storage fixture retains the same exact journal and result-bearing frames,
    // removing only the tip certificate. It cannot manufacture positive finality authority.
    let missing_kura = crate::kura::Kura::blank_kura_for_testing();
    let mut missing_state = State::new_with_chain_and_network_id_for_testing(
        World::new(),
        Arc::clone(&missing_kura),
        crate::query::store::LiveQueryStore::start_test(),
        "sumeragi-certified-test-chain".into(),
        chain.network_id(),
    );
    for height in 1..=2 {
        let frame = chain.committed(height).block().as_ref().clone();
        let frame = if height == 2 {
            frame.with_commit_certificate(None)
        } else {
            frame
        };
        missing_state.push_block_hash_for_testing(frame.hash());
        missing_kura.store_block(frame).unwrap();
    }
    let missing_context = ExecutorContext {
        state: Arc::new(missing_state),
        ..context.clone()
    };
    let mut worker = self::worker(&missing_context, archives);
    worker.pending_commit = Some(retained);
    assert!(worker.commit(&block, &qc).is_err());
    assert!(worker.pending_commit.is_some());
    assert_eq!(worker.applied.0, 1);
    assert!(events.try_recv().is_err());
}

#[test]
fn archive_attachment_captures_exact_tip_once_before_executor_work_and_survives_reopen() {
    run(archive_attachment_captures_exact_tip_once_before_executor_work_and_survives_reopen_case);
}

fn archive_attachment_captures_exact_tip_once_before_executor_work_and_survives_reopen_case() {
    let chain = chain();
    let (directory, archives) = archives();
    let provider = archives.provider_ingest.as_ref().unwrap();
    let reputation = archives.reputation.as_ref().unwrap();
    let (context, _events) = context(&chain);
    let executor = StateExecutor::spawn(context).unwrap();
    let reputation_root = reputation.root().to_owned();
    let reputation_bounds = reputation.bounds();
    let hidden_root = reputation_root.with_extension("temporarily-unavailable");
    std::fs::rename(&reputation_root, &hidden_root).unwrap();
    assert!(
        executor
            .attach_finalized_archives(archives.clone())
            .is_err()
    );
    let provider_generation = provider.health_generation().unwrap();
    assert_eq!(provider_generation, 1);
    assert_eq!(chain.state().view().height(), 1);
    std::fs::rename(&hidden_root, &reputation_root).unwrap();
    executor
        .attach_finalized_archives(archives.clone())
        .unwrap();
    assert_eq!(provider.health_generation().unwrap(), provider_generation);
    let generation = (
        provider.health_generation().unwrap(),
        reputation.health_generation().unwrap(),
    );
    assert!(
        executor
            .attach_finalized_archives(archives.clone())
            .is_err()
    );
    assert_eq!(
        generation,
        (
            provider.health_generation().unwrap(),
            reputation.health_generation().unwrap()
        )
    );
    // A restart releases the actual filesystem writers; merely dropping a join handle
    // would detach the worker and race its asynchronous release of the archive Arcs.
    let StateExecutor {
        requests, _thread, ..
    } = executor;
    drop(requests);
    _thread.join().unwrap();
    drop(archives);
    let provider_reopened = ProviderIngestFinalizedArchiveV1::try_open(
        directory.path().join("provider"),
        provider_bounds(),
    )
    .unwrap();
    let reputation_reopened =
        ReputationFinalizedArchive::try_open(&reputation_root, reputation_bounds).unwrap();
    let view = chain.state().view();
    let provider_qualification = provider_reopened
        .qualify_against_certified_tip(&view, chain.kura(), 0)
        .unwrap();
    let reputation_qualification = reputation_reopened
        .qualify_against_certified_tip(&view, chain.kura(), 0)
        .unwrap();
    assert_eq!(provider_qualification.kura_tip_height(), 1);
    assert_eq!(provider_qualification.lag_blocks(), 0);
    assert_eq!(provider_qualification.generation(), generation.0);
    assert_eq!(reputation_qualification.kura_tip_height(), 1);
    assert_eq!(reputation_qualification.lag_blocks(), 0);
    assert_eq!(reputation_qualification.generation(), generation.1);
    assert_eq!(provider_reopened.health_generation().unwrap(), generation.0);
    assert_eq!(
        reputation_reopened.health_generation().unwrap(),
        generation.1
    );
}

#[test]
fn below_quorum_current_frame_cannot_finish_pending_archive_capture() {
    run(below_quorum_current_frame_cannot_finish_pending_archive_capture_case);
}

fn below_quorum_current_frame_cannot_finish_pending_archive_capture_case() {
    let chain = chain();
    let (_directory, archives) = archives();
    archives.capture(&chain.state().view()).unwrap();
    let (context, mut events) = context(&chain);
    let mut worker = worker(&context, archives.clone());
    let (block, qc) = super::publication_tests::executed(&chain, &mut worker);
    let below = chain.commit_qc(
        2,
        qc.block_hash,
        qc.result,
        block.header.attest,
        Signers::BelowQuorum,
    );
    let committee = worker
        .scheduled(2)
        .unwrap()
        .height_config()
        .unwrap()
        .committee;
    assert!(
        iroha_sumeragi::crypto::Verifier::new(
            &**worker.context.crypto.as_ref().unwrap(),
            &chain.instance(),
            &block.header.epoch,
            &committee
        )
        .verify_qc(&iroha_sumeragi::crypto::NoAttestation, &below)
        .is_err(),
        "negative durable frame has an actual insufficient signed quorum"
    );
    hold_archive_completion(&mut worker, &block, &qc);
    let generation = archives
        .provider_ingest
        .as_ref()
        .unwrap()
        .health_generation()
        .unwrap();
    let retained = worker.pending_commit.take().unwrap();
    drop(worker);
    // Preserve the actual already-published pending owner. Only the negative
    // durable frame's quorum bytes change; they never authorize a new capture.
    let invalid_kura = crate::kura::Kura::blank_kura_for_testing();
    let mut invalid_state = State::new_with_chain_and_network_id_for_testing(
        World::new(),
        Arc::clone(&invalid_kura),
        crate::query::store::LiveQueryStore::start_test(),
        "sumeragi-certified-test-chain".into(),
        chain.network_id(),
    );
    for height in 1..=2 {
        let mut frame = chain.committed(height).block().as_ref().clone();
        if height == 2 {
            let certificate = frame.commit_certificate().unwrap();
            let invalid_certificate =
                iroha_data_model::block::CommitCertificate::from_untrusted_parts(
                    certificate.consensus_header().to_vec(),
                    norito::to_bytes(&below).unwrap(),
                    certificate.result_preimage().to_vec(),
                );
            frame = frame.with_commit_certificate(Some(invalid_certificate));
        }
        invalid_state.push_block_hash_for_testing(frame.hash());
        invalid_kura.store_block(frame).unwrap();
    }
    let invalid_context = ExecutorContext {
        state: Arc::new(invalid_state),
        ..context.clone()
    };
    let mut worker = self::worker(&invalid_context, archives.clone());
    worker.pending_commit = Some(retained);
    assert!(worker.commit(&block, &qc).is_err());
    assert!(worker.pending_commit.is_some());
    assert_eq!(worker.applied.0, 1);
    assert_eq!(
        archives
            .provider_ingest
            .as_ref()
            .unwrap()
            .health_generation()
            .unwrap(),
        generation
    );
    assert!(events.try_recv().is_err());
}
