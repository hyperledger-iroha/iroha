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
    permission::Permissions,
    sorafs::reputation::{
        REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1, REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1,
        ReputationJournalAuthorityPolicyV1,
    },
};
use iroha_executor_data_model::permission::sorafs::CanManageSorafsReputationJournalPolicy;

fn chain() -> CertifiedTestChain {
    let mut config = TestChainConfig::new(World::new(), 1_750_000_000_000);
    let authority = AccountId::new(config.genesis_key.public_key().clone());
    let mut permissions = Permissions::new();
    permissions.insert(CanManageSorafsReputationJournalPolicy.into());
    config
        .world
        .account_permissions
        .insert(authority.clone(), permissions);
    config.genesis_instructions.push(
        SetSorafsReputationJournalAuthorityPolicy::new(ReputationJournalAuthorityPolicyV1 {
            version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            por_recorder_authority: authority.clone(),
            dispute_recorder_authority: authority.clone(),
            token_recorder_authority: authority,
            max_source_age_ms: REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1,
        })
        .into(),
    );
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
    executor
        .attach_finalized_archives(archives.clone())
        .unwrap();
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
    let provider_reopened = ProviderIngestFinalizedArchiveV1::try_open(
        directory.path().join("provider"),
        provider_bounds(),
    )
    .unwrap();
    let reputation_reopened =
        ReputationFinalizedArchive::try_open(reputation.root(), reputation.bounds()).unwrap();
    let view = chain.state().view();
    provider_reopened
        .qualify_against_certified_tip(&view, chain.kura(), 0)
        .unwrap();
    reputation_reopened
        .qualify_against_certified_tip(&view, chain.kura(), 0)
        .unwrap();
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
