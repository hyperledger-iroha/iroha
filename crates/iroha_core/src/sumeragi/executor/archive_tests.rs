//! Current archive ownership after State publication, using genuine four-validator certificates.
//!
//! These replace the old pre-WSV V2 archive crash/reservation cases. Archive failure now retains
//! an already committed decision; retry may capture it, but cannot execute or notify it twice.
use super::*;
use crate::{
    query::{
        provider_ingest_finalized::{
            ProviderIngestFinalizedArchiveBoundsV1, ProviderIngestFinalizedArchiveV1,
        },
        reputation_finalized::{ReputationFinalizedArchive, ReputationFinalizedArchiveBounds},
    },
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig},
};
use iroha_data_model::{
    events::time::{TimeEvent, TimeInterval},
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
    let (events, receiver) = tokio::sync::broadcast::channel(16);
    (
        ExecutorContext {
            state: Arc::clone(chain.state()),
            queue: None,
            staging: Staging::new(),
            events,
            genesis_account: chain.genesis_account().clone(),
            consensus_mode: ConsensusMode::Permissioned,
            applied: (1, chain.committed(1).core_hash()),
            crypto: None,
            applied_watch: Arc::new(crate::sumeragi::lanes::global::AppliedWatch::new(1, None)),
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
        results: BTreeMap::new(),
        last_built: None,
        queue: context.queue.clone(),
        beacon: None,
        archives: Some(archives),
        pending_commit: None,
    }
}

fn pending(chain: &CertifiedTestChain) -> (Block, Qc, PendingCommit) {
    let committed = chain.committed(2);
    let block = committed.block();
    let header = committed.header().unwrap().clone();
    let qc: Qc = norito::decode_canonical(&block.commit_certificate().unwrap().commit_qc).unwrap();
    let payload = block.canonical_resultless_proposal().encode_wire().unwrap();
    let next = chain
        .state()
        .view()
        .world()
        .consensus_schedule()
        .get(4)
        .unwrap()
        .height_config()
        .unwrap();
    let pending = PendingCommit {
        header: header.clone(),
        qc: qc.clone(),
        state_hash: block.hash(),
        next,
        hashes: block
            .external_entrypoints_slice()
            .iter()
            .map(TransactionEntrypoint::hash)
            .collect(),
        events: vec![EventBox::Time(TimeEvent {
            interval: TimeInterval {
                since_ms: 1,
                length_ms: 1,
            },
        })],
    };
    (Block { header, payload }, qc, pending)
}

#[test]
fn partial_archive_failure_retains_exact_decision_and_retries_without_reexecution_or_notifications()
{
    let mut chain = chain();
    let (directory, archives) = archives();
    archives.capture(&chain.state().view()).unwrap();
    let provider = archives.provider_ingest.as_ref().unwrap();
    let reputation = archives.reputation.as_ref().unwrap();
    let provider_before = provider.health_generation().unwrap();
    let reputation_before = reputation.health_generation().unwrap();
    let (_clock, time) =
        iroha_primitives::time::TimeSource::new_mock(Duration::from_millis(1_750_000_000_099));
    let queue = Arc::new(Queue::test(
        iroha_config::parameters::actual::Queue::default(),
        &time,
    ));
    let clock_key =
        iroha_crypto::KeyPair::from_seed(vec![0xCC; 32], iroha_crypto::Algorithm::Ed25519);
    let transaction = chain.sign(
        &clock_key,
        [iroha_data_model::isi::Log::new(
            iroha_data_model::Level::INFO,
            "archive pending queue cleanup".into(),
        )
        .into()],
        1_750_000_000_099,
    );
    let entry_hash = transaction.hash_as_entrypoint();
    let accepted = crate::tx::AcceptedTransaction::accept_with_time_source(
        transaction.clone(),
        &chain.network_id(),
        Duration::from_secs(1),
        chain.state().view().world().parameters().transaction(),
        &iroha_config::parameters::actual::Crypto::default(),
        &time,
    )
    .unwrap();
    queue.push(accepted, chain.state().view()).unwrap();
    assert!(queue.contains_entrypoint_hash(entry_hash));
    assert_eq!(
        chain.commit_at(1_750_000_000_100, vec![transaction]),
        vec![true]
    );
    // The fixture performs real execution/prepare/store/State commit. It does not capture these
    // archives: the exact pending decision below is the executor's post-publication seam.
    assert_eq!(provider.health_generation().unwrap(), provider_before);
    assert_eq!(reputation.health_generation().unwrap(), reputation_before);
    let (mut context, mut events) = context(&chain);
    context.queue = Some(Arc::clone(&queue));
    let mut worker = worker(&context, archives.clone());
    let (block, qc, retained) = pending(&chain);
    let hash = retained.state_hash;
    let input_hashes = retained.hashes.clone();
    assert!(!input_hashes.is_empty());
    worker.pending_commit = Some(retained);
    let root = reputation.root().to_owned();
    let hidden = root.with_extension("temporarily-unavailable");
    std::fs::rename(&root, &hidden).unwrap();
    assert!(worker.commit(&block, &qc).is_err());
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
        worker.live.is_none(),
        "retry must not acquire another State overlay"
    );
    std::fs::rename(&hidden, &root).unwrap();
    assert_eq!(reputation.health_generation().unwrap(), reputation_before);
    worker.commit(&block, &qc).unwrap();
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
    assert!(events.try_recv().is_ok());
    assert!(events.try_recv().is_err());
    assert!(worker.commit(&block, &qc).is_err());
    assert!(events.try_recv().is_err(), "completion cannot emit twice");
    assert_eq!(provider.health_generation().unwrap(), provider_before + 1);
}

#[test]
fn pending_capture_rejects_substituted_header_qc_state_and_missing_certificate() {
    let mut chain = chain();
    let (_directory, archives) = archives();
    archives.capture(&chain.state().view()).unwrap();
    chain.commit_at(1_750_000_000_100, vec![]);
    let (context, mut events) = context(&chain);
    let mut worker = worker(&context, archives.clone());
    let (block, qc, retained) = pending(&chain);
    worker.pending_commit = Some(retained);
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
    let StateExecutor { requests, _thread } = executor;
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
    let mut chain = chain();
    let (_directory, archives) = archives();
    archives.capture(&chain.state().view()).unwrap();
    let generation = archives
        .provider_ingest
        .as_ref()
        .unwrap()
        .health_generation()
        .unwrap();
    chain.commit_with(Some(1_750_000_000_100), vec![], Signers::BelowQuorum);
    let (context, mut events) = context(&chain);
    let mut worker = worker(&context, archives.clone());
    let (block, qc, retained) = pending(&chain);
    worker.pending_commit = Some(retained);
    assert!(worker.commit(&block, &qc).is_err());
    assert!(worker.pending_commit.is_some());
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
