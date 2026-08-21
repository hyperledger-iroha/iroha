use super::*;
use ed25519_dalek::{Signer as _, SigningKey};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    events::data::sorafs::SorafsModerationLedgerEvent,
    metadata::Metadata,
    prelude::Json,
    sorafs::{
        moderation::{
            SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1, SoraFsModerationBallotContextV1,
        },
        moderation_ledger::{
            MODERATION_APPEAL_INTAKE_VERSION_V1, MODERATION_LEDGER_CASE_VERSION_V1,
            MODERATION_LEDGER_POLICY_VERSION_V1, ModerationAppealIntakeV1,
            ModerationAppealRecordV1, ModerationCaseRecordV1, ModerationCaseSpecV1,
            ModerationJurorEligibilityClassV1, ModerationJurorEligibilityRecordV1,
            ModerationLedgerPolicyRecord, ModerationLedgerPolicyV1, ModerationLedgerStatusV1,
            ModerationNoShowKindV1, ModerationNoShowRecordV1, ModerationOutcomeKindV1,
            ModerationOutcomeRecordV1, ModerationPanelSelectionV1, ModerationPoPRegistrySnapshotV1,
            ModerationVoteCountsV1, sorafs_moderation_panel_roster_hash_v1,
        },
    },
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU32,
    sync::{
        Arc, Condvar, Mutex, Weak,
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
        mpsc,
    },
    thread,
};
use tempfile::TempDir;
const TEST_ENVELOPE_CREATION_UNIX_MS: u64 = 1_700_000_000_000;
const TRANSACTION_SIGNER_HANDLE: &str = "moderation-hsm-primary";
const STRICT_INGRESS_HANDLE: &str = "moderation-ingress-primary";
const HANDOFF_PROVIDER_HANDLE: &str = "moderation-handoff-primary";
const PANEL_NOTIFICATION_PROVIDER_HANDLE: &str = "moderation-notification-primary";
const PANEL_NOTIFICATION_ARCHIVE_HANDLE: &str = "object-lock:prod-moderation-receipts";
const PANEL_NOTIFICATION_ARCHIVE_ID: [u8; 32] = [0xD4; 32];
const PANEL_NOTIFICATION_ARCHIVE_SIGNING_SEED: [u8; 32] = [0xE4; 32];
const PANEL_NOTIFICATION_ARCHIVE_ROTATED_SIGNING_SEED: [u8; 32] = [0xE5; 32];
const CHECKPOINT_STORE_HANDLE: &str = "sealed-cas:moderation-checkpoint-primary";
const CHECKPOINT_STORE_ATTESTATION_SIGNING_SEED: [u8; 32] = [0xE7; 32];
const TRANSACTION_SIGNER_QUALIFICATION: ModerationRuntimeProviderQualificationV1 =
    ModerationRuntimeProviderQualificationV1::new(1, [0xA1; 32]);
const STRICT_INGRESS_QUALIFICATION: ModerationRuntimeProviderQualificationV1 =
    ModerationRuntimeProviderQualificationV1::new(1, [0xA2; 32]);
const HANDOFF_PROVIDER_QUALIFICATION: ModerationRuntimeProviderQualificationV1 =
    ModerationRuntimeProviderQualificationV1::new(1, [0xA3; 32]);
const PANEL_NOTIFICATION_PROVIDER_QUALIFICATION: ModerationRuntimeProviderQualificationV1 =
    ModerationRuntimeProviderQualificationV1::new(1, [0xA4; 32]);
const PANEL_NOTIFICATION_ARCHIVE_QUALIFICATION: ModerationRuntimeProviderQualificationV1 =
    ModerationRuntimeProviderQualificationV1::new(1, [0xA5; 32]);
const PANEL_NOTIFICATION_ARCHIVE_ROTATED_QUALIFICATION: ModerationRuntimeProviderQualificationV1 =
    ModerationRuntimeProviderQualificationV1::new(2, [0xB5; 32]);
const CHECKPOINT_STORE_QUALIFICATION: ModerationRuntimeProviderQualificationV1 =
    ModerationRuntimeProviderQualificationV1::new(7, [0xA7; 32]);
macro_rules! orchestrator_fixture {
    ($orchestrator:ident; $( $binding:ident = $value:expr; )+ => $config:expr; $dependencies:expr; $open_error:literal) => {
        $(let $binding = $value;)+
        let $orchestrator = ModerationOrchestratorV1::open($config, $dependencies)
            .expect($open_error);
    };
}
macro_rules! open_test_orchestrator {
    ($orchestrator:ident = $config:expr; $dependencies:expr; $open_error:literal) => {
        let $orchestrator = open_test_orchestrator!($config; $dependencies; $open_error);
    };
    ($config:expr; $dependencies:expr; $open_error:literal) => {
        ModerationOrchestratorV1::open($config, $dependencies).expect($open_error)
    };
}
macro_rules! isolated_orchestrator {
    ($config:expr; $snapshot:expr; $submitter_value:expr; $open_error:literal) => {{
        let reader = Arc::new(MockSnapshotReader::new($snapshot));
        let submitter = Arc::new($submitter_value);
        ModerationOrchestratorV1::open($config, deps(reader, submitter)).expect($open_error)
    }};
}
macro_rules! test_runtime_deps {
    (
        $checkpoint_store:expr; $submitter:expr; $snapshot_reader:expr;
        $settlement_sink:expr; $publication_sink:expr;
        $panel_notification_sink:expr; $panel_notification_archive:expr
    ) => {
        ModerationOrchestratorDepsV1 {
            checkpoint_store: $checkpoint_store,
            submitter: $submitter,
            snapshot_reader: $snapshot_reader,
            settlement_sink: $settlement_sink,
            publication_sink: $publication_sink,
            panel_notification_sink: $panel_notification_sink,
            panel_notification_archive: $panel_notification_archive,
        }
    };
}
macro_rules! compact_panel_receipts {
    ($orchestrator:expr, $limit:expr; $compact_error:literal; $head_error:literal) => {
        $orchestrator
            .compact_panel_notification_receipts($limit)
            .expect($compact_error)
            .expect($head_error)
    };
}
macro_rules! publish_panel_archive {
    ($orchestrator:expr; $error:literal) => {
        $orchestrator
            .reconcile_panel_notification_archive_publication()
            .expect($error)
    };
}
macro_rules! audit_panel_archive {
    ($orchestrator:expr, $limit:expr; $error:literal) => {
        $orchestrator
            .audit_panel_notification_archive($limit)
            .expect($error)
    };
}
macro_rules! panel_archive_health {
    ($orchestrator:expr; $error:literal) => {
        $orchestrator.durable_health().expect($error)
    };
}
macro_rules! seed_default_operation {
    ($orchestrator:expr; $nonce:expr) => {
        seed_ready_operation_without_delivery(
            &$orchestrator,
            account(1),
            policy_action(policy(1)),
            $nonce,
        )
    };
}
macro_rules! find_outbox_entry {
    ($state:ident, $entry:ident = $orchestrator:expr, $operation_id:expr; $state_error:literal; $entry_error:literal) => {
        let $state = $orchestrator.state.lock().expect($state_error);
        let $entry = $state
            .outbox
            .iter()
            .find(|entry| entry.operation_id == $operation_id)
            .expect($entry_error);
    };
}
macro_rules! single_outbox_entry {
    ($state:ident, $entry:ident = $orchestrator:expr; $state_error:literal; $entry_error:literal) => {
        let $state = $orchestrator.state.lock().expect($state_error);
        let [$entry] = $state.outbox.as_slice() else {
            panic!($entry_error);
        };
    };
}
macro_rules! unpack_saturated_fixture {
    ($fixture:expr; $($fields:tt)*) => {
        let SaturatedPanelNotificationFixture {
            $($fields)*
            ..
        } = $fixture;
    };
}
macro_rules! assert_default_open_error {
    ($config:expr; $error:pat_param if $guard:expr) => {{
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
        assert!(matches!(
            ModerationOrchestratorV1::open($config, deps(reader, submitter)),
            $error if $guard
        ));
    }};
}
fn test_network_id() -> iroha_data_model::NetworkId {
    iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        iroha_crypto::Hash::prehashed([0xA5; iroha_crypto::Hash::LENGTH]),
    ))
}
#[derive(Debug)]
struct MockRuntimeProvider {
    handle: String,
    qualification: Mutex<
        Result<ModerationRuntimeProviderQualificationV1, ModerationRuntimeProviderReadinessErrorV1>,
    >,
}
impl MockRuntimeProvider {
    fn new(
        handle: impl Into<String>,
        qualification: ModerationRuntimeProviderQualificationV1,
    ) -> Self {
        Self {
            handle: handle.into(),
            qualification: Mutex::new(Ok(qualification)),
        }
    }
    fn set_qualification(&self, qualification: ModerationRuntimeProviderQualificationV1) {
        *self
            .qualification
            .lock()
            .expect("provider qualification lock") = Ok(qualification);
    }
    fn set_readiness(&self, readiness: ModerationRuntimeProviderReadinessErrorV1) {
        *self
            .qualification
            .lock()
            .expect("provider qualification lock") = Err(readiness);
    }
}
impl ModerationRuntimeProviderV1 for MockRuntimeProvider {
    fn handle(&self) -> &str {
        &self.handle
    }
    fn qualification(
        &self,
    ) -> Result<ModerationRuntimeProviderQualificationV1, ModerationRuntimeProviderReadinessErrorV1>
    {
        *self
            .qualification
            .lock()
            .expect("provider qualification lock")
    }
}
#[derive(Debug)]
struct MockSnapshotReader {
    snapshot: Mutex<ModerationFinalizedLedgerSnapshotV1>,
    checkpoint_store: Arc<MockCheckpointStore>,
}
impl MockSnapshotReader {
    fn new(snapshot: ModerationFinalizedLedgerSnapshotV1) -> Self {
        Self {
            snapshot: Mutex::new(snapshot),
            checkpoint_store: Arc::new(MockCheckpointStore::default()),
        }
    }
    fn replace(&self, snapshot: ModerationFinalizedLedgerSnapshotV1) {
        *self.snapshot.lock().expect("snapshot lock") = snapshot;
    }
}
impl ModerationFinalizedSnapshotReaderV1 for MockSnapshotReader {
    fn read_finalized_snapshot(
        &self,
        _max_cases: usize,
        _max_events: usize,
    ) -> Result<ModerationFinalizedLedgerSnapshotV1, ModerationSnapshotReadErrorV1> {
        Ok(self.snapshot.lock().expect("snapshot lock").clone())
    }
}
#[derive(Debug)]
struct MockSubmitterState {
    calls: usize,
    sign_calls: usize,
    actions: Vec<ModerationNativeActionV1>,
    signed: BTreeMap<([u8; 32], u32), ModerationSignedTransactionV1>,
    operations: BTreeMap<([u8; 32], [u8; 32]), ModerationSubmissionLookupV1>,
    fallback: ModerationSubmissionLookupV1,
    sign_failure: Option<ModerationSubmissionFailureV1>,
    failure: Option<ModerationSubmissionFailureV1>,
    ambiguous_is_applied: bool,
}
#[derive(Debug)]
struct MockSubmitter {
    state: Mutex<MockSubmitterState>,
    transaction_signer_provider: MockRuntimeProvider,
    strict_ingress_provider: MockRuntimeProvider,
}
impl MockSubmitter {
    fn new(fallback: ModerationSubmissionLookupV1) -> Self {
        Self {
            state: Mutex::new(MockSubmitterState {
                calls: 0,
                sign_calls: 0,
                actions: Vec::new(),
                signed: BTreeMap::new(),
                operations: BTreeMap::new(),
                fallback,
                sign_failure: None,
                failure: None,
                ambiguous_is_applied: false,
            }),
            transaction_signer_provider: MockRuntimeProvider::new(
                TRANSACTION_SIGNER_HANDLE,
                TRANSACTION_SIGNER_QUALIFICATION,
            ),
            strict_ingress_provider: MockRuntimeProvider::new(
                STRICT_INGRESS_HANDLE,
                STRICT_INGRESS_QUALIFICATION,
            ),
        }
    }
    fn ambiguous_applied(fallback: ModerationSubmissionLookupV1) -> Self {
        Self {
            state: Mutex::new(MockSubmitterState {
                calls: 0,
                sign_calls: 0,
                actions: Vec::new(),
                signed: BTreeMap::new(),
                operations: BTreeMap::new(),
                fallback,
                sign_failure: None,
                failure: Some(ModerationSubmissionFailureV1::Ambiguous),
                ambiguous_is_applied: true,
            }),
            transaction_signer_provider: MockRuntimeProvider::new(
                TRANSACTION_SIGNER_HANDLE,
                TRANSACTION_SIGNER_QUALIFICATION,
            ),
            strict_ingress_provider: MockRuntimeProvider::new(
                STRICT_INGRESS_HANDLE,
                STRICT_INGRESS_QUALIFICATION,
            ),
        }
    }
    fn calls(&self) -> usize {
        self.state.lock().expect("submitter lock").calls
    }
    fn actions(&self) -> Vec<ModerationNativeActionV1> {
        self.state.lock().expect("submitter lock").actions.clone()
    }
    fn sign_calls(&self) -> usize {
        self.state.lock().expect("submitter lock").sign_calls
    }
    fn set_failure(&self, failure: Option<ModerationSubmissionFailureV1>) {
        self.state.lock().expect("submitter lock").failure = failure;
    }
    fn set_sign_failure(&self, failure: Option<ModerationSubmissionFailureV1>) {
        self.state.lock().expect("submitter lock").sign_failure = failure;
    }
    fn set_lookup(
        &self,
        operation_id: [u8; 32],
        transaction_id: [u8; 32],
        lookup: ModerationSubmissionLookupV1,
    ) {
        self.state
            .lock()
            .expect("submitter lock")
            .operations
            .insert((operation_id, transaction_id), lookup);
    }
}
impl ModerationTransactionSubmitterV1 for MockSubmitter {
    fn transaction_signer_provider(&self) -> &dyn ModerationRuntimeProviderV1 {
        &self.transaction_signer_provider
    }
    fn strict_ingress_provider(&self) -> &dyn ModerationRuntimeProviderV1 {
        &self.strict_ingress_provider
    }
    fn network_id(&self) -> iroha_data_model::NetworkId {
        test_network_id()
    }
    fn sign(
        &self,
        request: &ModerationTransactionRequestV1,
    ) -> Result<ModerationSignedTransactionV1, ModerationSubmissionFailureV1> {
        let mut state = self.state.lock().expect("submitter lock");
        state.sign_calls = state.sign_calls.saturating_add(1);
        if let Some(failure) = state.sign_failure {
            return Err(failure);
        }
        let signed_key = (request.operation_id, request.envelope_generation);
        if let Some(signed) = state.signed.get(&signed_key) {
            return Ok(signed.clone());
        }
        let signer = key_for_authority(&request.authority);
        let mut builder = TransactionBuilder::new(
            request.network_id,
            request.authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_ttl(core::time::Duration::from_millis(
            MODERATION_TRANSACTION_TTL_MS_V1,
        ));
        let generation_offset = u64::from(request.envelope_generation.saturating_sub(1))
            .checked_mul(MODERATION_TRANSACTION_TTL_MS_V1.saturating_add(1))
            .ok_or(ModerationSubmissionFailureV1::PermanentRejection)?;
        let creation_time = TEST_ENVELOPE_CREATION_UNIX_MS
            .checked_add(generation_offset)
            .ok_or(ModerationSubmissionFailureV1::PermanentRejection)?;
        builder.set_creation_time(core::time::Duration::from_millis(creation_time));
        let transaction = builder
            .with_instructions([request.action.instruction()])
            .sign(signer.private_key());
        let signed = ModerationSignedTransactionV1::from_signed_transaction(request, &transaction)?;
        state.signed.insert(signed_key, signed.clone());
        Ok(signed)
    }
    fn submit_signed(
        &self,
        request: &ModerationTransactionRequestV1,
        signed: &ModerationSignedTransactionV1,
    ) -> Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1> {
        signed.decode_for_request(request)?;
        let mut state = self.state.lock().expect("submitter lock");
        let lookup_key = (request.operation_id, signed.transaction_id);
        if let Some(existing) = state.operations.get(&lookup_key).copied() {
            let existing_transaction_id = match existing {
                ModerationSubmissionLookupV1::Pending { transaction_id }
                | ModerationSubmissionLookupV1::Applied { transaction_id } => transaction_id,
                ModerationSubmissionLookupV1::Rejected {
                    transaction_id: Some(transaction_id),
                    ..
                } => transaction_id,
                ModerationSubmissionLookupV1::NotFound { .. }
                | ModerationSubmissionLookupV1::Rejected {
                    transaction_id: None,
                    ..
                }
                | ModerationSubmissionLookupV1::Unknown => {
                    return Err(ModerationSubmissionFailureV1::Ambiguous);
                }
            };
            return if existing_transaction_id == signed.transaction_id {
                Ok(ModerationTransactionReceiptV1 {
                    transaction_id: signed.transaction_id,
                    observed_finalized_height: request.baseline_finalized_height,
                })
            } else {
                Err(ModerationSubmissionFailureV1::Ambiguous)
            };
        }
        state.calls = state.calls.saturating_add(1);
        state.actions.push(request.action.clone());
        if state.ambiguous_is_applied {
            state.operations.insert(
                lookup_key,
                ModerationSubmissionLookupV1::Applied {
                    transaction_id: signed.transaction_id,
                },
            );
        }
        if let Some(failure) = state.failure {
            return Err(failure);
        }
        state.operations.insert(
            lookup_key,
            ModerationSubmissionLookupV1::Pending {
                transaction_id: signed.transaction_id,
            },
        );
        Ok(ModerationTransactionReceiptV1 {
            transaction_id: signed.transaction_id,
            observed_finalized_height: request.baseline_finalized_height,
        })
    }
    fn lookup(
        &self,
        operation_id: [u8; 32],
        transaction_id: Option<[u8; 32]>,
    ) -> ModerationSubmissionLookupV1 {
        let state = self.state.lock().expect("submitter lock");
        transaction_id
            .and_then(|transaction_id| {
                state
                    .operations
                    .get(&(operation_id, transaction_id))
                    .copied()
            })
            .unwrap_or(state.fallback)
    }
}
#[derive(Debug)]
struct MockHandoffSink {
    provider: MockRuntimeProvider,
    delivered: Mutex<Vec<[u8; 32]>>,
    published_archive_heads: Mutex<BTreeMap<[u8; 32], ModerationPanelNotificationArchiveHeadV1>>,
    calls: AtomicUsize,
}
impl Default for MockHandoffSink {
    fn default() -> Self {
        Self {
            provider: MockRuntimeProvider::new(
                HANDOFF_PROVIDER_HANDLE,
                HANDOFF_PROVIDER_QUALIFICATION,
            ),
            delivered: Mutex::new(Vec::new()),
            published_archive_heads: Mutex::new(BTreeMap::new()),
            calls: AtomicUsize::new(0),
        }
    }
}
impl MockHandoffSink {
    fn delivered(&self) -> Vec<[u8; 32]> {
        self.delivered.lock().expect("handoff sink lock").clone()
    }
    fn calls(&self) -> usize {
        self.calls.load(AtomicOrdering::Relaxed)
    }
    fn published_archive_head_count(&self) -> usize {
        self.published_archive_heads
            .lock()
            .expect("archive publication lock")
            .len()
    }
}
impl ModerationRuntimeProviderV1 for MockHandoffSink {
    fn handle(&self) -> &str {
        self.provider.handle()
    }
    fn qualification(
        &self,
    ) -> Result<ModerationRuntimeProviderQualificationV1, ModerationRuntimeProviderReadinessErrorV1>
    {
        self.provider.qualification()
    }
}
impl ModerationTerminalHandoffSinkV1 for MockHandoffSink {
    fn deliver(
        &self,
        handoff: &ModerationTerminalHandoffV1,
    ) -> Result<(), ModerationHandoffFailureV1> {
        self.calls.fetch_add(1, AtomicOrdering::Relaxed);
        let mut delivered = self.delivered.lock().expect("handoff sink lock");
        if !delivered.contains(&handoff.handoff_id) {
            delivered.push(handoff.handoff_id);
        }
        Ok(())
    }
    fn publish_panel_notification_archive_head(
        &self,
        head: &ModerationPanelNotificationArchiveHeadV1,
    ) -> Result<(), ModerationHandoffFailureV1> {
        verify_panel_notification_archive_head(head)
            .map_err(|_| ModerationHandoffFailureV1::Permanent)?;
        let mut published = self
            .published_archive_heads
            .lock()
            .expect("archive publication lock");
        if let Some(existing) = published.get(&head.operation_id) {
            return if existing == head {
                Ok(())
            } else {
                Err(ModerationHandoffFailureV1::Permanent)
            };
        }
        if let Some(predecessor) = published.values().max_by_key(|value| value.generation) {
            verify_panel_notification_archive_lineage_link(head, predecessor)
                .map_err(|_| ModerationHandoffFailureV1::Permanent)?;
        } else if head.generation != 1 {
            return Err(ModerationHandoffFailureV1::Permanent);
        }
        published.insert(head.operation_id, head.clone());
        Ok(())
    }
    fn read_panel_notification_archive_head(
        &self,
    ) -> Result<Option<ModerationPanelNotificationArchiveHeadV1>, ModerationHandoffFailureV1> {
        Ok(self
            .published_archive_heads
            .lock()
            .expect("archive publication lock")
            .values()
            .max_by_key(|head| head.generation)
            .cloned())
    }
}
#[derive(Debug, Default)]
struct ReentrantLockProbe {
    orchestrator: Mutex<Option<Weak<ModerationOrchestratorV1>>>,
    checks: AtomicUsize,
}
impl ReentrantLockProbe {
    fn attach(&self, orchestrator: &Arc<ModerationOrchestratorV1>) {
        *self.orchestrator.lock().expect("probe lock") = Some(Arc::downgrade(orchestrator));
    }
    fn check(&self) {
        let orchestrator = self
            .orchestrator
            .lock()
            .expect("probe lock")
            .as_ref()
            .and_then(Weak::upgrade);
        let Some(orchestrator) = orchestrator else {
            return;
        };
        assert!(
            orchestrator.state.try_lock().is_ok(),
            "external collaborator ran while the orchestrator mutex was held"
        );
        let _ = orchestrator.snapshot();
        self.checks.fetch_add(1, AtomicOrdering::Relaxed);
    }
    fn checks(&self) -> usize {
        self.checks.load(AtomicOrdering::Relaxed)
    }
}
#[derive(Debug)]
struct ProbedSnapshotReader {
    inner: Arc<MockSnapshotReader>,
    probe: Arc<ReentrantLockProbe>,
}
impl ModerationFinalizedSnapshotReaderV1 for ProbedSnapshotReader {
    fn read_finalized_snapshot(
        &self,
        max_cases: usize,
        max_events: usize,
    ) -> Result<ModerationFinalizedLedgerSnapshotV1, ModerationSnapshotReadErrorV1> {
        self.probe.check();
        self.inner.read_finalized_snapshot(max_cases, max_events)
    }
}
#[derive(Debug)]
struct ProbedSubmitter {
    inner: Arc<MockSubmitter>,
    probe: Arc<ReentrantLockProbe>,
}
impl ModerationTransactionSubmitterV1 for ProbedSubmitter {
    fn transaction_signer_provider(&self) -> &dyn ModerationRuntimeProviderV1 {
        self.inner.transaction_signer_provider()
    }
    fn strict_ingress_provider(&self) -> &dyn ModerationRuntimeProviderV1 {
        self.inner.strict_ingress_provider()
    }
    fn network_id(&self) -> iroha_data_model::NetworkId {
        self.inner.network_id()
    }
    fn sign(
        &self,
        request: &ModerationTransactionRequestV1,
    ) -> Result<ModerationSignedTransactionV1, ModerationSubmissionFailureV1> {
        self.probe.check();
        self.inner.sign(request)
    }
    fn submit_signed(
        &self,
        request: &ModerationTransactionRequestV1,
        signed: &ModerationSignedTransactionV1,
    ) -> Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1> {
        self.probe.check();
        self.inner.submit_signed(request, signed)
    }
    fn lookup(
        &self,
        operation_id: [u8; 32],
        transaction_id: Option<[u8; 32]>,
    ) -> ModerationSubmissionLookupV1 {
        self.probe.check();
        self.inner.lookup(operation_id, transaction_id)
    }
}
#[derive(Debug)]
struct ProbedHandoffSink {
    inner: Arc<MockHandoffSink>,
    probe: Arc<ReentrantLockProbe>,
}
impl ModerationRuntimeProviderV1 for ProbedHandoffSink {
    fn handle(&self) -> &str {
        self.inner.handle()
    }
    fn qualification(
        &self,
    ) -> Result<ModerationRuntimeProviderQualificationV1, ModerationRuntimeProviderReadinessErrorV1>
    {
        self.inner.qualification()
    }
}
impl ModerationTerminalHandoffSinkV1 for ProbedHandoffSink {
    fn deliver(
        &self,
        handoff: &ModerationTerminalHandoffV1,
    ) -> Result<(), ModerationHandoffFailureV1> {
        self.probe.check();
        self.inner.deliver(handoff)
    }
    fn publish_panel_notification_archive_head(
        &self,
        head: &ModerationPanelNotificationArchiveHeadV1,
    ) -> Result<(), ModerationHandoffFailureV1> {
        self.probe.check();
        self.inner.publish_panel_notification_archive_head(head)
    }
    fn read_panel_notification_archive_head(
        &self,
    ) -> Result<Option<ModerationPanelNotificationArchiveHeadV1>, ModerationHandoffFailureV1> {
        self.probe.check();
        self.inner.read_panel_notification_archive_head()
    }
}
#[derive(Debug)]
struct BlockingSignSubmitter {
    inner: Arc<MockSubmitter>,
    entered: Mutex<Option<mpsc::Sender<()>>>,
    released: Mutex<bool>,
    release: Condvar,
}
impl BlockingSignSubmitter {
    fn new(inner: Arc<MockSubmitter>, entered: mpsc::Sender<()>) -> Self {
        Self {
            inner,
            entered: Mutex::new(Some(entered)),
            released: Mutex::new(false),
            release: Condvar::new(),
        }
    }
    fn release(&self) {
        *self.released.lock().expect("release lock") = true;
        self.release.notify_all();
    }
}
impl ModerationTransactionSubmitterV1 for BlockingSignSubmitter {
    fn transaction_signer_provider(&self) -> &dyn ModerationRuntimeProviderV1 {
        self.inner.transaction_signer_provider()
    }
    fn strict_ingress_provider(&self) -> &dyn ModerationRuntimeProviderV1 {
        self.inner.strict_ingress_provider()
    }
    fn network_id(&self) -> iroha_data_model::NetworkId {
        self.inner.network_id()
    }
    fn sign(
        &self,
        request: &ModerationTransactionRequestV1,
    ) -> Result<ModerationSignedTransactionV1, ModerationSubmissionFailureV1> {
        let entered = self.entered.lock().expect("entered lock").take();
        if let Some(entered) = entered {
            entered.send(()).expect("signal blocking signer");
            let released = self.released.lock().expect("release lock");
            drop(
                self.release
                    .wait_while(released, |released| !*released)
                    .expect("wait for signer release"),
            );
        }
        self.inner.sign(request)
    }
    fn submit_signed(
        &self,
        request: &ModerationTransactionRequestV1,
        signed: &ModerationSignedTransactionV1,
    ) -> Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1> {
        self.inner.submit_signed(request, signed)
    }
    fn lookup(
        &self,
        operation_id: [u8; 32],
        transaction_id: Option<[u8; 32]>,
    ) -> ModerationSubmissionLookupV1 {
        self.inner.lookup(operation_id, transaction_id)
    }
}
#[derive(Debug)]
struct DriftingSubmitter {
    inner: Arc<MockSubmitter>,
    signer_after_sign: Option<ModerationRuntimeProviderQualificationV1>,
    ingress_after_submit: Option<ModerationRuntimeProviderQualificationV1>,
    ingress_after_lookup: Option<ModerationRuntimeProviderQualificationV1>,
}
impl ModerationTransactionSubmitterV1 for DriftingSubmitter {
    fn transaction_signer_provider(&self) -> &dyn ModerationRuntimeProviderV1 {
        self.inner.transaction_signer_provider()
    }
    fn strict_ingress_provider(&self) -> &dyn ModerationRuntimeProviderV1 {
        self.inner.strict_ingress_provider()
    }
    fn network_id(&self) -> iroha_data_model::NetworkId {
        self.inner.network_id()
    }
    fn sign(
        &self,
        request: &ModerationTransactionRequestV1,
    ) -> Result<ModerationSignedTransactionV1, ModerationSubmissionFailureV1> {
        let result = self.inner.sign(request);
        if let Some(qualification) = self.signer_after_sign {
            self.inner
                .transaction_signer_provider
                .set_qualification(qualification);
        }
        result
    }
    fn submit_signed(
        &self,
        request: &ModerationTransactionRequestV1,
        signed: &ModerationSignedTransactionV1,
    ) -> Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1> {
        let result = self.inner.submit_signed(request, signed);
        if let Some(qualification) = self.ingress_after_submit {
            self.inner
                .strict_ingress_provider
                .set_qualification(qualification);
        }
        result
    }
    fn lookup(
        &self,
        operation_id: [u8; 32],
        transaction_id: Option<[u8; 32]>,
    ) -> ModerationSubmissionLookupV1 {
        let result = self.inner.lookup(operation_id, transaction_id);
        if let Some(qualification) = self.ingress_after_lookup {
            self.inner
                .strict_ingress_provider
                .set_qualification(qualification);
        }
        result
    }
}
#[derive(Debug)]
struct DriftingHandoffSink {
    inner: Arc<MockHandoffSink>,
    qualification_after_delivery: ModerationRuntimeProviderQualificationV1,
}
impl ModerationRuntimeProviderV1 for DriftingHandoffSink {
    fn handle(&self) -> &str {
        self.inner.handle()
    }
    fn qualification(
        &self,
    ) -> Result<ModerationRuntimeProviderQualificationV1, ModerationRuntimeProviderReadinessErrorV1>
    {
        self.inner.qualification()
    }
}
impl ModerationTerminalHandoffSinkV1 for DriftingHandoffSink {
    fn deliver(
        &self,
        handoff: &ModerationTerminalHandoffV1,
    ) -> Result<(), ModerationHandoffFailureV1> {
        let result = self.inner.deliver(handoff);
        self.inner
            .provider
            .set_qualification(self.qualification_after_delivery);
        result
    }
    fn publish_panel_notification_archive_head(
        &self,
        head: &ModerationPanelNotificationArchiveHeadV1,
    ) -> Result<(), ModerationHandoffFailureV1> {
        let result = self.inner.publish_panel_notification_archive_head(head);
        self.inner
            .provider
            .set_qualification(self.qualification_after_delivery);
        result
    }
    fn read_panel_notification_archive_head(
        &self,
    ) -> Result<Option<ModerationPanelNotificationArchiveHeadV1>, ModerationHandoffFailureV1> {
        self.inner.read_panel_notification_archive_head()
    }
}
#[derive(Debug)]
struct MockPanelNotificationSink {
    provider: MockRuntimeProvider,
    calls: Mutex<usize>,
    receipts: Mutex<BTreeMap<[u8; 32], ModerationPanelNotificationDeliveryReceiptV1>>,
}
impl Default for MockPanelNotificationSink {
    fn default() -> Self {
        Self {
            provider: MockRuntimeProvider::new(
                PANEL_NOTIFICATION_PROVIDER_HANDLE,
                PANEL_NOTIFICATION_PROVIDER_QUALIFICATION,
            ),
            calls: Mutex::new(0),
            receipts: Mutex::new(BTreeMap::new()),
        }
    }
}
impl MockPanelNotificationSink {
    fn deliver(
        &self,
        claim: &ModerationPanelNotificationClaimV1,
        delivered_at_unix_ms: u64,
    ) -> ModerationPanelNotificationDeliveryReceiptV1 {
        let mut calls = self.calls.lock().expect("panel sink calls lock");
        *calls = calls.saturating_add(1);
        let mut receipts = self.receipts.lock().expect("panel sink receipt lock");
        *receipts
            .entry(claim.notification.notification_id)
            .or_insert_with(|| ModerationPanelNotificationDeliveryReceiptV1 {
                notification_id: claim.notification.notification_id,
                receipt_digest: domain_hash(
                    b"sorafs.moderation.test-panel-receipt.v1",
                    &[&claim.notification.notification_id],
                ),
                delivered_at_unix_ms,
            })
    }
    fn calls(&self) -> usize {
        *self.calls.lock().expect("panel sink calls lock")
    }
    fn unique_deliveries(&self) -> usize {
        self.receipts.lock().expect("panel sink receipt lock").len()
    }
}
impl ModerationRuntimeProviderV1 for MockPanelNotificationSink {
    fn handle(&self) -> &str {
        self.provider.handle()
    }
    fn qualification(
        &self,
    ) -> Result<ModerationRuntimeProviderQualificationV1, ModerationRuntimeProviderReadinessErrorV1>
    {
        self.provider.qualification()
    }
}
impl ModerationPanelNotificationSinkV1 for MockPanelNotificationSink {
    fn deliver(
        &self,
        claim: &ModerationPanelNotificationClaimV1,
    ) -> Result<ModerationPanelNotificationDeliveryReceiptV1, ModerationPanelNotificationFailureV1>
    {
        Ok(MockPanelNotificationSink::deliver(
            self,
            claim,
            claim
                .notification
                .source_occurred_at_unix_ms
                .saturating_add(1),
        ))
    }
}
type MockPanelNotificationArchiveArtifacts =
    BTreeMap<[u8; 32], ([u8; 32], ModerationPanelNotificationArchiveReadbackV1)>;
struct MockPanelNotificationArchive {
    provider: MockRuntimeProvider,
    archive_id: [u8; 32],
    signing_key: Mutex<SigningKey>,
    artifacts: Mutex<MockPanelNotificationArchiveArtifacts>,
    install_calls: AtomicUsize,
    read_calls: AtomicUsize,
    next_install_behavior: AtomicUsize,
    next_read_behavior: AtomicUsize,
}
impl fmt::Debug for MockPanelNotificationArchive {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MockPanelNotificationArchive")
            .field("provider", &self.provider)
            .field("archive_id", &self.archive_id)
            .field("signing_key", &"<test-signing-key>")
            .finish_non_exhaustive()
    }
}
impl Default for MockPanelNotificationArchive {
    fn default() -> Self {
        Self::with_handle(PANEL_NOTIFICATION_ARCHIVE_HANDLE)
    }
}
impl MockPanelNotificationArchive {
    fn with_handle(handle: impl Into<String>) -> Self {
        Self {
            provider: MockRuntimeProvider::new(handle, PANEL_NOTIFICATION_ARCHIVE_QUALIFICATION),
            archive_id: PANEL_NOTIFICATION_ARCHIVE_ID,
            signing_key: Mutex::new(SigningKey::from_bytes(
                &PANEL_NOTIFICATION_ARCHIVE_SIGNING_SEED,
            )),
            artifacts: Mutex::new(BTreeMap::new()),
            install_calls: AtomicUsize::new(0),
            read_calls: AtomicUsize::new(0),
            next_install_behavior: AtomicUsize::new(0),
            next_read_behavior: AtomicUsize::new(0),
        }
    }
    fn public_key(&self) -> [u8; 32] {
        self.signing_key
            .lock()
            .expect("notification archive signing key")
            .verifying_key()
            .to_bytes()
    }
    fn rotate_signing_key(&self, signing_seed: [u8; 32]) {
        *self
            .signing_key
            .lock()
            .expect("notification archive signing key") = SigningKey::from_bytes(&signing_seed);
    }
    fn fail_next_install(&self, behavior: usize) {
        self.next_install_behavior
            .store(behavior, AtomicOrdering::SeqCst);
    }
    fn fail_next_read(&self, behavior: usize) {
        self.next_read_behavior
            .store(behavior, AtomicOrdering::SeqCst);
    }
    fn install_calls(&self) -> usize {
        self.install_calls.load(AtomicOrdering::SeqCst)
    }
    fn read_calls(&self) -> usize {
        self.read_calls.load(AtomicOrdering::SeqCst)
    }
    fn artifact_count(&self) -> usize {
        self.artifacts
            .lock()
            .expect("notification archive artifacts")
            .len()
    }
    fn artifact(&self, operation_id: [u8; 32]) -> Vec<u8> {
        self.artifacts
            .lock()
            .expect("notification archive artifacts")
            .get(&operation_id)
            .expect("installed notification archive artifact")
            .1
            .canonical_artifact
            .clone()
    }
    fn replace_artifact(&self, operation_id: [u8; 32], bytes: Vec<u8>) {
        self.artifacts
            .lock()
            .expect("notification archive artifacts")
            .get_mut(&operation_id)
            .expect("installed notification archive artifact")
            .1
            .canonical_artifact = bytes;
    }
}
impl ModerationRuntimeProviderV1 for MockPanelNotificationArchive {
    fn handle(&self) -> &str {
        self.provider.handle()
    }
    fn qualification(
        &self,
    ) -> Result<ModerationRuntimeProviderQualificationV1, ModerationRuntimeProviderReadinessErrorV1>
    {
        self.provider.qualification()
    }
}
impl ModerationPanelNotificationArchiveV1 for MockPanelNotificationArchive {
    fn archive_id(&self) -> [u8; 32] {
        self.archive_id
    }
    fn signing_public_key(&self) -> [u8; 32] {
        self.public_key()
    }
    fn install(
        &self,
        operation_id: [u8; 32],
        receipt_message: [u8; 32],
        canonical_artifact: &[u8],
    ) -> Result<[u8; 64], ModerationPanelNotificationArchiveExternalErrorV1> {
        self.install_calls.fetch_add(1, AtomicOrdering::SeqCst);
        let behavior = self.next_install_behavior.swap(0, AtomicOrdering::SeqCst);
        if behavior == 1 {
            return Err(ModerationPanelNotificationArchiveExternalErrorV1::Unavailable);
        }
        let mut artifacts = self
            .artifacts
            .lock()
            .map_err(|_| ModerationPanelNotificationArchiveExternalErrorV1::Unavailable)?;
        let result = match artifacts.get(&operation_id) {
            Some((existing_message, existing))
                if *existing_message == receipt_message
                    && existing.canonical_artifact.as_slice() == canonical_artifact =>
            {
                Ok(existing.signature)
            }
            Some(_) => Err(ModerationPanelNotificationArchiveExternalErrorV1::Rejected),
            None => {
                let signature = self
                    .signing_key
                    .lock()
                    .map_err(|_| ModerationPanelNotificationArchiveExternalErrorV1::Unavailable)?
                    .sign(&receipt_message)
                    .to_bytes();
                artifacts.insert(
                    operation_id,
                    (
                        receipt_message,
                        ModerationPanelNotificationArchiveReadbackV1 {
                            canonical_artifact: canonical_artifact.to_vec(),
                            signature,
                        },
                    ),
                );
                Ok(signature)
            }
        };
        if behavior == 2 && result.is_ok() {
            Err(ModerationPanelNotificationArchiveExternalErrorV1::Ambiguous)
        } else {
            result
        }
    }
    fn read(
        &self,
        operation_id: [u8; 32],
    ) -> Result<
        Option<ModerationPanelNotificationArchiveReadbackV1>,
        ModerationPanelNotificationArchiveExternalErrorV1,
    > {
        self.read_calls.fetch_add(1, AtomicOrdering::SeqCst);
        let behavior = self.next_read_behavior.swap(0, AtomicOrdering::SeqCst);
        if behavior == 1 {
            return Ok(None);
        }
        if behavior == 5 {
            return Err(ModerationPanelNotificationArchiveExternalErrorV1::Unavailable);
        }
        let mut readback = self
            .artifacts
            .lock()
            .map_err(|_| ModerationPanelNotificationArchiveExternalErrorV1::Unavailable)?
            .get(&operation_id)
            .map(|(_, readback)| readback.clone());
        if let Some(readback) = readback.as_mut() {
            match behavior {
                2 => {
                    if let Some(byte) = readback.canonical_artifact.first_mut() {
                        *byte ^= 1;
                    }
                }
                3 => readback.signature[0] ^= 1,
                4 => readback.canonical_artifact.push(0),
                _ => {}
            }
        }
        Ok(readback)
    }
}
#[derive(Debug)]
struct ProbedPanelNotificationArchive {
    inner: Arc<MockPanelNotificationArchive>,
    probe: Arc<ReentrantLockProbe>,
}
impl ModerationRuntimeProviderV1 for ProbedPanelNotificationArchive {
    fn handle(&self) -> &str {
        self.inner.handle()
    }
    fn qualification(
        &self,
    ) -> Result<ModerationRuntimeProviderQualificationV1, ModerationRuntimeProviderReadinessErrorV1>
    {
        self.probe.check();
        self.inner.qualification()
    }
}
impl ModerationPanelNotificationArchiveV1 for ProbedPanelNotificationArchive {
    fn archive_id(&self) -> [u8; 32] {
        self.probe.check();
        self.inner.archive_id()
    }
    fn signing_public_key(&self) -> [u8; 32] {
        self.probe.check();
        self.inner.signing_public_key()
    }
    fn install(
        &self,
        operation_id: [u8; 32],
        receipt_message: [u8; 32],
        canonical_artifact: &[u8],
    ) -> Result<[u8; 64], ModerationPanelNotificationArchiveExternalErrorV1> {
        self.probe.check();
        self.inner
            .install(operation_id, receipt_message, canonical_artifact)
    }
    fn read(
        &self,
        operation_id: [u8; 32],
    ) -> Result<
        Option<ModerationPanelNotificationArchiveReadbackV1>,
        ModerationPanelNotificationArchiveExternalErrorV1,
    > {
        self.probe.check();
        self.inner.read(operation_id)
    }
}
fn account(seed: u8) -> AccountId {
    let keypair = KeyPair::try_from_seed(vec![seed.max(1); 32], Algorithm::Ed25519)
        .expect("deterministic account");
    AccountId::new(keypair.public_key().clone())
}
fn key_for_authority(authority: &AccountId) -> KeyPair {
    (1_u8..=u8::MAX)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("deterministic authority key")
        })
        .find(|key| key.public_key() == authority.expect_single_signatory())
        .expect("test authority must use the deterministic account fixture")
}
fn policy(revision: u64) -> ModerationLedgerPolicyV1 {
    ModerationLedgerPolicyV1 {
        version: MODERATION_LEDGER_POLICY_VERSION_V1,
        revision,
        predecessor_policy_digest: (revision > 1).then_some([0xA5; 32]),
        max_panel_size: 5,
        max_candidate_pool_size: 32,
        max_waitlist_size: 5,
        max_exclusions_per_case: 16,
        max_total_window_ms: 60_000,
        max_challenges_per_case: 4,
        missing_commit_penalty_points: 10,
        unrevealed_commit_penalty_points: 20,
    }
}
fn policy_action(policy: ModerationLedgerPolicyV1) -> ModerationNativeActionV1 {
    ModerationNativeActionV1::SetPolicy(SetSorafsModerationPolicy::new(policy))
}
fn empty_snapshot(height: u64, block_hash: [u8; 32]) -> ModerationFinalizedLedgerSnapshotV1 {
    ModerationFinalizedLedgerSnapshotV1 {
        version: MODERATION_FINALIZED_SNAPSHOT_VERSION_V1,
        finalized_height: height,
        finalized_block_hash: block_hash,
        finalized_at_unix_ms: height.max(1),
        policy: None,
        status: None,
        appeals: Vec::new(),
        cases: Vec::new(),
        events: Vec::new(),
    }
}
fn empty_snapshot_at(
    height: u64,
    block_hash: [u8; 32],
    finalized_at_unix_ms: u64,
) -> ModerationFinalizedLedgerSnapshotV1 {
    let mut snapshot = empty_snapshot(height, block_hash);
    snapshot.finalized_at_unix_ms = finalized_at_unix_ms;
    snapshot
}
fn snapshot_with_policy(
    height: u64,
    block_hash: [u8; 32],
    policy: ModerationLedgerPolicyV1,
    authority: AccountId,
) -> ModerationFinalizedLedgerSnapshotV1 {
    let policy_digest = policy.digest().expect("policy digest");
    ModerationFinalizedLedgerSnapshotV1 {
        version: MODERATION_FINALIZED_SNAPSHOT_VERSION_V1,
        finalized_height: height,
        finalized_block_hash: block_hash,
        finalized_at_unix_ms: height.max(1),
        policy: Some(ModerationLedgerPolicyRecord {
            policy,
            policy_digest,
            activated_at_unix_ms: 1,
            activated_by: authority.clone(),
        }),
        status: Some(ModerationLedgerStatusV1 {
            updated_at_unix_ms: 1,
            ..ModerationLedgerStatusV1::default()
        }),
        appeals: Vec::new(),
        cases: Vec::new(),
        events: vec![ModerationFinalizedEventV1 {
            sequence: 1,
            block_height: height,
            block_hash,
            event_index: 0,
            event: SorafsModerationLedgerEvent::new(
                SorafsModerationLedgerEventKind::PolicyActivated,
                None,
                None,
                authority,
                1,
            ),
        }],
    }
}
fn awaiting_acceptance_snapshot(
    height: u64,
    block_hash: [u8; 32],
    governance: AccountId,
) -> (ModerationFinalizedLedgerSnapshotV1, [u8; 32]) {
    let active_policy = policy(1);
    let policy_digest = active_policy.digest().expect("policy digest");
    let appellant = account(90);
    let pop_snapshot = ModerationPoPRegistrySnapshotV1 {
        issuer_policy_digest: [0x31; 32],
        commitment_root: [0x32; 32],
        commitment_tree_version: 1,
        revocation_root: [0x33; 32],
        revocation_list_version: 1,
        registry_audit_sequence: 1,
        registry_audit_head: [0x34; 32],
        captured_at_unix_ms: 2,
    };
    let pop_snapshot_digest = pop_snapshot.digest().expect("PoP snapshot digest");
    let intake = ModerationAppealIntakeV1 {
        version: MODERATION_APPEAL_INTAKE_VERSION_V1,
        case_id: "case-failover".to_owned(),
        round_id: "round-1".to_owned(),
        appellant: appellant.clone(),
        appealed_decision_digest: [0x41; 32],
        proof_token_digest: [0x42; 32],
        evidence_bundle_digest: [0x43; 32],
        appeal_deposit_lock_digest: [0x44; 32],
        appeal_finance_config_version: "finance-v1".to_owned(),
        policy_reference: "policy-v1".to_owned(),
        evidence_uri: Some("ipfs://case-failover".to_owned()),
        panel_size: 2,
        waitlist_size: 1,
        quorum: 1,
        exclusions: vec![appellant.clone()],
        registration_deadline_unix_ms: 20,
        acceptance_deadline_unix_ms: 30,
        commit_deadline_unix_ms: 40,
        challenge_deadline_unix_ms: 50,
        reveal_deadline_unix_ms: 60,
        policy_digest,
    };
    let intake_digest = intake.digest().expect("intake digest");
    let mut eligibility = (1_u8..=3)
        .map(|seed| ModerationJurorEligibilityRecordV1 {
            case_id: intake.case_id.clone(),
            round_id: intake.round_id.clone(),
            juror: account(seed),
            eligibility_class: ModerationJurorEligibilityClassV1::General,
            proof_digest: [seed.saturating_add(0x50); 32],
            nullifier: [seed.saturating_add(0x60); 32],
            pop_snapshot_digest,
            credential_expires_at_epoch: 1_000,
            registered_at_unix_ms: 10 + u64::from(seed),
        })
        .collect::<Vec<_>>();
    eligibility.sort_by_key(|record| record.juror.to_string());
    let eligible_jurors = eligibility
        .iter()
        .map(|record| record.juror.clone())
        .collect::<Vec<_>>();
    let randomness_anchor = [0x71; 32];
    let (jurors, waitlist, seed_digest, sortition_digest) = sorafs_moderation_select_panel_v1(
        intake_digest,
        pop_snapshot_digest,
        randomness_anchor,
        &eligibility,
        intake.panel_size,
        intake.waitlist_size,
        intake.quorum,
    )
    .expect("deterministic sortition");
    let selection = ModerationPanelSelectionV1 {
        randomness_anchor,
        seed_digest,
        jurors,
        waitlist,
        sortition_digest,
        selected_at_unix_ms: 21,
        selected_by: governance.clone(),
    };
    let appeal = ModerationAppealRecordV1 {
        intake,
        intake_digest,
        policy: active_policy,
        pop_snapshot,
        pop_snapshot_digest,
        status: ModerationAppealStatusV1::AwaitingAcceptance,
        submitted_by: appellant,
        submitted_at_unix_ms: 3,
        eligible_jurors,
        selection: Some(selection),
        accepted_jurors: Vec::new(),
        replacements: Vec::new(),
        activated_at_unix_ms: None,
        finalized_at_unix_ms: None,
    };
    (
        ModerationFinalizedLedgerSnapshotV1 {
            version: MODERATION_FINALIZED_SNAPSHOT_VERSION_V1,
            finalized_height: height,
            finalized_block_hash: block_hash,
            finalized_at_unix_ms: 31,
            policy: Some(ModerationLedgerPolicyRecord {
                policy: active_policy,
                policy_digest,
                activated_at_unix_ms: 1,
                activated_by: governance.clone(),
            }),
            status: Some(ModerationLedgerStatusV1 {
                appeal_intakes: 1,
                eligibility_proofs: 3,
                panel_selections: 1,
                updated_at_unix_ms: 21,
                ..ModerationLedgerStatusV1::default()
            }),
            appeals: vec![ModerationFinalizedAppealViewV1 {
                appeal,
                eligibility,
            }],
            cases: Vec::new(),
            events: vec![ModerationFinalizedEventV1 {
                sequence: 5,
                block_height: height,
                block_hash,
                event_index: 0,
                event: SorafsModerationLedgerEvent::new(
                    SorafsModerationLedgerEventKind::SortitionFinalized,
                    Some("case-failover".to_owned()),
                    Some("round-1".to_owned()),
                    governance,
                    21,
                ),
            }],
        },
        sortition_digest,
    )
}
fn activated_case_snapshot(
    height: u64,
    block_hash: [u8; 32],
    governance: AccountId,
) -> ModerationFinalizedLedgerSnapshotV1 {
    let (mut snapshot, _) = awaiting_acceptance_snapshot(height, block_hash, governance.clone());
    let appeal_view = snapshot.appeals.first_mut().expect("appeal projection");
    let appeal = &mut appeal_view.appeal;
    let selection = appeal.selection.clone().expect("panel selection");
    let mut accepted_jurors = selection.jurors.clone();
    accepted_jurors.sort_by_key(ToString::to_string);
    appeal.status = ModerationAppealStatusV1::BallotOpen;
    appeal.accepted_jurors = accepted_jurors;
    appeal.activated_at_unix_ms = Some(31);
    let intake = &appeal.intake;
    let jurors = selection.jurors;
    let case = ModerationCaseRecordV1 {
        spec: ModerationCaseSpecV1 {
            version: MODERATION_LEDGER_CASE_VERSION_V1,
            context: SoraFsModerationBallotContextV1 {
                version: SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
                case_id: intake.case_id.clone(),
                evidence_bundle_digest: intake.evidence_bundle_digest,
                appeal_finance_config_version: intake.appeal_finance_config_version.clone(),
                panel_roster_hash: sorafs_moderation_panel_roster_hash_v1(&jurors, intake.quorum),
                policy_reference: intake.policy_reference.clone(),
                evidence_uri: intake.evidence_uri.clone(),
            },
            round_id: intake.round_id.clone(),
            jurors,
            quorum: intake.quorum,
            commit_deadline_unix_ms: intake.commit_deadline_unix_ms,
            challenge_deadline_unix_ms: intake.challenge_deadline_unix_ms,
            reveal_deadline_unix_ms: intake.reveal_deadline_unix_ms,
            policy_digest: intake.policy_digest,
        },
        policy: appeal.policy,
        status: ModerationCaseStatusV1::Open,
        opened_at_unix_ms: 31,
        opened_by: governance.clone(),
        commitment_count: 0,
        reveal_count: 0,
        challenge_count: 0,
        challenge_ids: Vec::new(),
        pending_challenge_count: 0,
        accepted_challenge_count: 0,
        expired_challenge_count: 0,
    };
    snapshot.cases = vec![ModerationFinalizedCaseViewV1 {
        case,
        commits: Vec::new(),
        reveals: Vec::new(),
        challenges: Vec::new(),
        outcome: None,
        no_shows: Vec::new(),
    }];
    snapshot.status = Some(ModerationLedgerStatusV1 {
        appeal_intakes: 1,
        eligibility_proofs: 3,
        panel_selections: 1,
        assignment_acceptances: 2,
        open_cases: 1,
        updated_at_unix_ms: 31,
        ..ModerationLedgerStatusV1::default()
    });
    snapshot.events = vec![ModerationFinalizedEventV1 {
        sequence: 6,
        block_height: height,
        block_hash,
        event_index: 0,
        event: SorafsModerationLedgerEvent::new(
            SorafsModerationLedgerEventKind::CaseActivated,
            Some("case-failover".to_owned()),
            Some("round-1".to_owned()),
            governance,
            31,
        ),
    }];
    snapshot
}
fn finalized_case_snapshot(
    mut snapshot: ModerationFinalizedLedgerSnapshotV1,
    height: u64,
    block_hash: [u8; 32],
    governance: AccountId,
) -> ModerationFinalizedLedgerSnapshotV1 {
    const FINALIZED_AT_UNIX_MS: u64 = 61;
    snapshot.finalized_height = height;
    snapshot.finalized_block_hash = block_hash;
    snapshot.finalized_at_unix_ms = FINALIZED_AT_UNIX_MS;
    let appeal = &mut snapshot
        .appeals
        .first_mut()
        .expect("appeal projection")
        .appeal;
    appeal.status = ModerationAppealStatusV1::Finalized;
    appeal.finalized_at_unix_ms = Some(FINALIZED_AT_UNIX_MS);
    let case_view = snapshot.cases.first_mut().expect("case projection");
    case_view.case.status = ModerationCaseStatusV1::Finalized;
    let policy_digest = case_view.case.spec.policy_digest;
    case_view.no_shows = case_view
        .case
        .spec
        .jurors
        .iter()
        .cloned()
        .map(|juror| ModerationNoShowRecordV1 {
            case_id: "case-failover".to_owned(),
            round_id: "round-1".to_owned(),
            juror,
            kind: ModerationNoShowKindV1::MissingCommit,
            penalty_points: case_view.case.policy.missing_commit_penalty_points,
            policy_digest,
            recorded_at_unix_ms: FINALIZED_AT_UNIX_MS,
        })
        .collect();
    case_view
        .no_shows
        .sort_by_key(|record| record.juror.to_string());
    case_view.outcome = Some(ModerationOutcomeRecordV1 {
        case_id: "case-failover".to_owned(),
        round_id: "round-1".to_owned(),
        kind: ModerationOutcomeKindV1::QuorumNotMet,
        counts: ModerationVoteCountsV1::default(),
        votes_total: 0,
        quorum: case_view.case.spec.quorum,
        no_show_count: u32::try_from(case_view.no_shows.len()).expect("bounded no-show count"),
        finalized_at_unix_ms: FINALIZED_AT_UNIX_MS,
        finalized_by: governance.clone(),
    });
    snapshot.status = Some(ModerationLedgerStatusV1 {
        appeal_intakes: 1,
        eligibility_proofs: 3,
        panel_selections: 1,
        assignment_acceptances: 2,
        finalized_cases: 1,
        outcomes: 1,
        no_shows: u64::try_from(case_view.no_shows.len()).expect("bounded no-show count"),
        updated_at_unix_ms: FINALIZED_AT_UNIX_MS,
        ..ModerationLedgerStatusV1::default()
    });
    snapshot.events = vec![ModerationFinalizedEventV1 {
        sequence: 7,
        block_height: height,
        block_hash,
        event_index: 0,
        event: SorafsModerationLedgerEvent::new(
            SorafsModerationLedgerEventKind::CaseFinalized,
            Some("case-failover".to_owned()),
            Some("round-1".to_owned()),
            governance,
            FINALIZED_AT_UNIX_MS,
        ),
    }];
    snapshot
}
fn config(temp: &TempDir, name: &str) -> ModerationOrchestratorConfigV1 {
    let canonical_temp = temp.path().canonicalize().expect("canonical tempdir");
    ModerationOrchestratorConfigV1 {
        checkpoint_path: canonical_temp.join(name),
        checkpoint_store_handle: CHECKPOINT_STORE_HANDLE.to_owned(),
        expected_checkpoint_store_qualification: CHECKPOINT_STORE_QUALIFICATION,
        checkpoint_store_attestation_public_key: SigningKey::from_bytes(
            &CHECKPOINT_STORE_ATTESTATION_SIGNING_SEED,
        )
        .verifying_key()
        .to_bytes(),
        max_cases: 64,
        max_events: 256,
        max_outbox_entries: 16,
        max_idempotency_records: 64,
        max_handoffs: 64,
        max_submit_attempts: 3,
        checkpoint_max_bytes: 4 * 1024 * 1024,
        panel_notification_archive_max_bytes: 5 * 1024 * 1024,
        transaction_signer_handle: TRANSACTION_SIGNER_HANDLE.to_owned(),
        expected_transaction_signer_qualification: TRANSACTION_SIGNER_QUALIFICATION,
        strict_ingress_handle: STRICT_INGRESS_HANDLE.to_owned(),
        expected_strict_ingress_qualification: STRICT_INGRESS_QUALIFICATION,
        settlement_handoff_handle: HANDOFF_PROVIDER_HANDLE.to_owned(),
        expected_settlement_handoff_qualification: HANDOFF_PROVIDER_QUALIFICATION,
        publication_handoff_handle: HANDOFF_PROVIDER_HANDLE.to_owned(),
        expected_publication_handoff_qualification: HANDOFF_PROVIDER_QUALIFICATION,
        panel_notification_handle: PANEL_NOTIFICATION_PROVIDER_HANDLE.to_owned(),
        expected_panel_notification_qualification: PANEL_NOTIFICATION_PROVIDER_QUALIFICATION,
        panel_notification_archive_handle: PANEL_NOTIFICATION_ARCHIVE_HANDLE.to_owned(),
        expected_panel_notification_archive_qualification: PANEL_NOTIFICATION_ARCHIVE_QUALIFICATION,
        panel_notification_archive_id: PANEL_NOTIFICATION_ARCHIVE_ID,
        panel_notification_archive_bootstrap_public_key: SigningKey::from_bytes(
            &PANEL_NOTIFICATION_ARCHIVE_SIGNING_SEED,
        )
        .verifying_key()
        .to_bytes(),
        panel_notification_archive_public_key: SigningKey::from_bytes(
            &PANEL_NOTIFICATION_ARCHIVE_SIGNING_SEED,
        )
        .verifying_key()
        .to_bytes(),
        panel_notification_archive_predecessor_revocation_generation: None,
        panel_notification_archive_predecessor_authorization_signature: None,
        panel_notification_archive_new_key_possession_signature: None,
    }
}
fn provider_test_request() -> ModerationTransactionRequestV1 {
    ModerationTransactionRequestV1::new(
        test_network_id(),
        1,
        account(41),
        policy_action(policy(1)),
        [0x71; 32],
        7,
        [0x72; 32],
    )
    .expect("canonical provider test request")
}
#[test]
fn runtime_provider_handles_use_canonical_production_grammar() {
    for handle in [
        "hsm://sorafs/moderation/signer-primary",
        "https-pinned-source-pool:moderation-ingress-primary",
    ] {
        assert_eq!(
            validate_moderation_runtime_provider_handle(handle, true),
            Ok(())
        );
    }
    for handle in [
        "hsm://sorafs/moderation/operator@signer",
        "hsm://sorafs/moderation/signer?token",
        "hsm://sorafs/moderation/signer#fragment",
        "hsm://sorafs/moderation/%73igner",
        "hsm://sorafs/moderation/signer\\primary",
    ] {
        assert_eq!(
            validate_moderation_runtime_provider_handle(handle, true),
            Err(ModerationRuntimeProviderQualificationErrorV1::InvalidConfiguredHandle)
        );
        assert_eq!(
            validate_moderation_runtime_provider_handle(handle, false),
            Err(ModerationRuntimeProviderQualificationErrorV1::InvalidProviderHandle)
        );
    }
    assert_eq!(
        validate_moderation_runtime_provider_handle("hsm://sorafs/moderation/dummy", true,),
        Err(ModerationRuntimeProviderQualificationErrorV1::TestMarkedConfiguredHandle)
    );
    assert_eq!(
        validate_moderation_runtime_provider_handle("hsm://sorafs/moderation/dummy", false,),
        Err(ModerationRuntimeProviderQualificationErrorV1::TestMarkedProviderHandle)
    );
}
#[test]
fn external_providers_are_qualified_before_checkpoint_access() {
    let temp = TempDir::new().expect("tempdir");
    let mut config = config(&temp, "missing/checkpoint.bin");
    let missing_parent = config
        .checkpoint_path
        .parent()
        .expect("checkpoint parent")
        .to_path_buf();
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    submitter
        .transaction_signer_provider
        .set_readiness(ModerationRuntimeProviderReadinessErrorV1::Rejected);
    let error = ModerationOrchestratorV1::open(config.clone(), deps(reader, submitter))
        .expect_err("unqualified signer must fail before checkpoint access");
    assert!(matches!(
        error,
        ModerationOrchestratorError::InvalidConfiguration(message)
            if message.contains("runtime provider binding")
    ));
    assert!(!missing_parent.exists());
    config.transaction_signer_handle = "moderation-hsm-secondary".to_owned();
    assert_default_open_error!(config.clone(); Err(ModerationOrchestratorError::InvalidConfiguration(message)) if message.contains("runtime provider binding"));
    assert!(!missing_parent.exists());
    config.transaction_signer_handle = TRANSACTION_SIGNER_HANDLE.to_owned();
    for settlement in [true, false] {
        let mut boundary_config = config.clone();
        if settlement {
            boundary_config.settlement_handoff_handle =
                "moderation-settlement-secondary".to_owned();
        } else {
            boundary_config.publication_handoff_handle =
                "moderation-publication-secondary".to_owned();
        }
        assert_default_open_error!(boundary_config; Err(ModerationOrchestratorError::InvalidConfiguration(message)) if message.contains("runtime provider binding"));
        assert!(!missing_parent.exists());
    }
    config.panel_notification_handle = "moderation-notification-secondary".to_owned();
    assert_default_open_error!(config; Err(ModerationOrchestratorError::InvalidConfiguration(message)) if message.contains("runtime provider binding"));
    assert!(!missing_parent.exists());
}
#[test]
fn snapshot_bounds_cannot_exceed_native_query_ceilings() {
    let temp = TempDir::new().expect("tempdir");
    for configure in [
        |config: &mut ModerationOrchestratorConfigV1| {
            config.max_cases = MODERATION_QUERY_MAX_CASES_V1 as usize + 1;
        },
        |config: &mut ModerationOrchestratorConfigV1| {
            config.max_events = MODERATION_QUERY_MAX_EVENTS_V1 as usize + 1;
        },
    ] {
        let mut config = config(&temp, "missing/native-query-ceiling.bin");
        let checkpoint_parent = config
            .checkpoint_path
            .parent()
            .expect("checkpoint parent")
            .to_path_buf();
        configure(&mut config);
        assert_default_open_error!(config; Err(ModerationOrchestratorError::InvalidConfiguration(message)) if message.contains("native query ceiling"));
        assert!(!checkpoint_parent.exists());
    }
}
macro_rules! runtime_provider_drift_tests {
    ($( $name:ident($qualified:ident, $inner:ident): $checkpoint:literal, $signer_after_sign:expr, $ingress_after_submit:expr, $ingress_after_lookup:expr => $body:block )+) => {$(
        #[test]
        fn $name() {
            let temp = TempDir::new().expect("tempdir");
            let config = config(&temp, $checkpoint);
            let $inner = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
            let submitter: Arc<dyn ModerationTransactionSubmitterV1> = Arc::new(DriftingSubmitter {
                inner: Arc::clone(&$inner),
                signer_after_sign: $signer_after_sign,
                ingress_after_submit: $ingress_after_submit,
                ingress_after_lookup: $ingress_after_lookup,
            });
            let $qualified = QualifiedModerationTransactionSubmitterV1::try_new(&config, submitter)
                .expect("initially qualified submitter");
            $body
        }
    )+};
}
runtime_provider_drift_tests! {
    signer_policy_drift_discards_the_returned_envelope(qualified, inner): "signer-drift.bin", Some(ModerationRuntimeProviderQualificationV1::new(2, [0xB1; 32])), None, None => { assert_eq!(qualified.sign(&provider_test_request()), Err(ModerationSubmissionFailureV1::RuntimeUnavailable)); assert_eq!(inner.sign_calls(), 1); }
    ingress_policy_drift_after_admission_is_ambiguous(qualified, inner): "ingress-drift.bin", None, Some(ModerationRuntimeProviderQualificationV1::new(2, [0xB2; 32])), None => { let request = provider_test_request(); let signed = qualified.sign(&request).expect("qualified signer result"); assert_eq!(qualified.submit_signed(&request, &signed), Err(ModerationSubmissionFailureV1::Ambiguous)); assert_eq!(inner.calls(), 1); }
    ingress_policy_drift_discards_a_positive_lookup(qualified, inner): "lookup-drift.bin", None, None, Some(ModerationRuntimeProviderQualificationV1::new(2, [0xC2; 32])) => { let request = provider_test_request(); let signed = qualified.sign(&request).expect("qualified signer result"); qualified.submit_signed(&request, &signed).expect("qualified admission"); assert_eq!(qualified.lookup(request.operation_id, Some(signed.transaction_id)), ModerationSubmissionLookupV1::Unknown); }
}
#[test]
fn canonical_committed_event_sequence_must_be_contiguous() {
    let temp = TempDir::new().expect("tempdir");
    let config = config(&temp, "event-sequence.bin");
    let authority = account(7);
    let mut snapshot = snapshot_with_policy(5, [0x55; 32], policy(1), authority.clone());
    snapshot.events.clear();
    assert!(matches!(
        validate_finalized_snapshot(&snapshot, &config),
        Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(message))
            if message.contains("no committed event")
    ));
    snapshot.events.push(ModerationFinalizedEventV1 {
        sequence: 7,
        block_height: 4,
        block_hash: [0x44; 32],
        event_index: 0,
        event: SorafsModerationLedgerEvent::new(
            SorafsModerationLedgerEventKind::PolicyActivated,
            None,
            None,
            authority.clone(),
            1,
        ),
    });
    validate_finalized_snapshot(&snapshot, &config).expect("single retained event suffix");
    let mut skipped_block_index = snapshot.clone();
    skipped_block_index.events.push(ModerationFinalizedEventV1 {
        sequence: 8,
        block_height: 4,
        block_hash: [0x44; 32],
        event_index: 2,
        event: SorafsModerationLedgerEvent::new(
            SorafsModerationLedgerEventKind::PolicyActivated,
            None,
            None,
            authority.clone(),
            1,
        ),
    });
    assert!(matches!(
        validate_finalized_snapshot(&skipped_block_index, &config),
        Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(message))
            if message.contains("block index")
    ));
    snapshot.events.push(ModerationFinalizedEventV1 {
        sequence: 9,
        block_height: 4,
        block_hash: [0x44; 32],
        event_index: 1,
        event: SorafsModerationLedgerEvent::new(
            SorafsModerationLedgerEventKind::PolicyActivated,
            None,
            None,
            authority,
            1,
        ),
    });
    assert!(matches!(
        validate_finalized_snapshot(&snapshot, &config),
        Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(message))
            if message.contains("sequence")
    ));
}
fn deps(
    reader: Arc<MockSnapshotReader>,
    submitter: Arc<MockSubmitter>,
) -> ModerationOrchestratorDepsV1 {
    ModerationOrchestratorDepsV1 {
        checkpoint_store: reader.checkpoint_store.clone(),
        submitter,
        snapshot_reader: reader,
        settlement_sink: Arc::new(MockHandoffSink::default()),
        publication_sink: Arc::new(MockHandoffSink::default()),
        panel_notification_sink: Arc::new(MockPanelNotificationSink::default()),
        panel_notification_archive: Arc::new(MockPanelNotificationArchive::default()),
    }
}
fn seed_ready_operation_without_delivery(
    orchestrator: &ModerationOrchestratorV1,
    authority: AccountId,
    action: ModerationNativeActionV1,
    request_binding_digest: [u8; 32],
) -> [u8; 32] {
    orchestrator.reconcile().expect("initial reconciliation");
    let action_digest = action.action_digest().expect("action digest");
    let operation_id = action
        .operation_id(&orchestrator.network_id, &authority)
        .expect("operation id");
    let mut state = orchestrator.state.lock().expect("orchestrator state");
    state.operations.push(StoredOperationV1 {
        operation_id,
        authority: authority.clone(),
        action_digest,
        status: StoredOperationStatusV1::Pending,
        transaction_id: None,
    });
    state.outbox.push(StoredOutboxEntryV1 {
        operation_id,
        authority,
        action,
        action_digest,
        request_binding_digest,
        envelope_generation: 1,
        retired_envelopes: Vec::new(),
        baseline_finalized_height: 0,
        baseline_finalized_block_hash: [0; 32],
        transaction_id: None,
        signed_transaction_digest: None,
        signed_transaction_bytes: None,
        attempts: 0,
        state: StoredOutboxStateV1::Ready,
        work_generation: 0,
        work_claim: None,
        last_lookup_finalized_height: 0,
        last_lookup_finalized_block_hash: [0; 32],
    });
    orchestrator
        .persist_checkpoint_locked(&mut state)
        .expect("persist ready operation");
    operation_id
}
fn execute_one_prepared_sign(orchestrator: &ModerationOrchestratorV1, operation_id: [u8; 32]) {
    let prepared = {
        let mut state = orchestrator.state.lock().expect("orchestrator state");
        orchestrator
            .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
            .expect("prepare signer work")
            .expect("one signer claim")
    };
    assert!(matches!(
        &prepared,
        PreparedExternalWorkV1::Sign { identity, .. }
            if identity.identity == operation_id
    ));
    orchestrator
        .execute_external_work(prepared)
        .expect("execute signer work");
}
fn prepare_one_submit(
    orchestrator: &ModerationOrchestratorV1,
    operation_id: [u8; 32],
) -> PreparedExternalWorkV1 {
    let prepared = {
        let mut state = orchestrator.state.lock().expect("orchestrator state");
        orchestrator
            .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
            .expect("prepare ingress work")
            .expect("one ingress claim")
    };
    assert!(matches!(
        &prepared,
        PreparedExternalWorkV1::Submit { identity, .. }
            if identity.identity == operation_id
    ));
    prepared
}
fn retained_envelope(
    orchestrator: &ModerationOrchestratorV1,
) -> (
    [u8; 32],
    u32,
    ModerationSignedTransactionV1,
    SignedEnvelopeTimingV1,
    StoredOutboxStateV1,
) {
    let state = orchestrator.state.lock().expect("orchestrator state");
    let [entry] = state.outbox.as_slice() else {
        panic!("one retained moderation envelope");
    };
    let request = moderation_transaction_request(&orchestrator.network_id, entry)
        .expect("retained transaction request");
    let signed = moderation_signed_transaction(entry).expect("retained signed transaction");
    let transaction = signed
        .decode_for_request(&request)
        .expect("valid retained signed transaction");
    let timing = signed_envelope_timing(&transaction).expect("retained envelope timing");
    (
        entry.operation_id,
        entry.envelope_generation,
        signed,
        timing,
        entry.state,
    )
}
fn sign_dead_letter_resolution(resolution: &ModerationDeadLetterResolutionV1) -> [u8; 64] {
    SigningKey::from_bytes(&CHECKPOINT_STORE_ATTESTATION_SIGNING_SEED)
        .sign(
            &resolution
                .signing_message()
                .expect("canonical dead-letter resolution message"),
        )
        .to_bytes()
}
fn assert_finalized_authority_rejection_has_no_native_mutation(
    snapshot: ModerationFinalizedLedgerSnapshotV1,
    authenticated: AccountId,
    required: &AccountId,
    action: ModerationNativeActionV1,
) {
    let temp = tempfile::tempdir().expect("tempdir");
    let finalized_height = snapshot.finalized_height;
    orchestrator_fixture!(orchestrator; reader = Arc::new(MockSnapshotReader::new(snapshot)); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: finalized_height })); => config(&temp, "authority-negative.norito"); deps(reader, Arc::clone(&submitter)); "orchestrator");
    let action_label = action.label();
    let error = orchestrator
        .submit(authenticated.clone(), action, [0xE1; 32])
        .expect_err("non-ledger authority must fail closed");
    assert_eq!(
        error,
        ModerationOrchestratorError::AuthorityMismatch {
            action: action_label,
            authenticated: authenticated.to_string(),
            native: required.to_string(),
        }
    );
    assert_eq!(submitter.calls(), 0);
    let state = orchestrator.state.lock().expect("orchestrator state");
    assert!(state.operations.is_empty());
    assert!(state.outbox.is_empty());
    assert!(state.dead_letters.is_empty());
}
