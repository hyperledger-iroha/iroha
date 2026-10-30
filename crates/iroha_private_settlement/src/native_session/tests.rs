//! Session ownership controls; native receipt fixtures use real QCs/signatures and synthetic
//! execution/proof bytes. These tests do not qualify proving, monetary execution or SDK bindings.
use super::*;
use crate::{
    FilePrivateSettlementArtifacts, PrivateSettlementBusinessTerms, PrivateSettlementContextSource,
    PrivateSettlementRestrictedOpening, PrivateSettlementWallet,
    test_support::client_with_base_url,
};
use iroha::workflow::{
    Digest32, FileWorkflowStorage, WorkflowFuture, WorkflowId, WorkflowStorage,
    WorkflowStorageErrorKind,
};
use iroha_core_privacy::privacy_engines::atomic_private_settlement::AtomicPrivateSettlementAuthorizedWalletLegV1;
use iroha_data_model::{
    nexus::{
        PrivateSettlementContextRequestV1, PrivateSettlementContextsProofV1,
        PrivateSettlementProofIntentV1,
    },
    sumeragi_finality::test_fixtures::NativeFinalityFixture,
};
use std::{cell::RefCell, time::Duration};
use tokio::sync::oneshot;

struct NoWallet;
impl PrivateSettlementWallet for NoWallet {
    fn open_policy(
        &self,
        _: &PrivateSettlementProofIntentV1,
        _: u8,
        _: &PrivateSettlementBusinessTerms,
    ) -> Result<PrivateSettlementRestrictedOpening, OneCallPrivateSettlementError> {
        panic!("session control must not open wallet")
    }
    fn reserve_or_reopen(
        &self,
        _: Digest32,
        _: &PrivateSettlementProofIntentV1,
        _: u8,
        _: &PrivateSettlementBusinessTerms,
    ) -> Result<AtomicPrivateSettlementAuthorizedWalletLegV1, OneCallPrivateSettlementError> {
        panic!("session control must not reserve notes")
    }
    fn reconcile_verified(
        &self,
        _: Digest32,
        _: &PrivateSettlementRequest,
        _: &VerifiedPrivateSettlementOutcome,
    ) -> Result<(), OneCallPrivateSettlementError> {
        panic!("session control cannot invent successful reconciliation")
    }
}
struct NoSource;
impl PrivateSettlementContextSource for NoSource {
    fn fetch(
        &self,
        _: u64,
        _: PrivateSettlementContextRequestV1,
    ) -> WorkflowFuture<'_, Result<PrivateSettlementContextsProofV1, OneCallPrivateSettlementError>>
    {
        Box::pin(async { panic!("fresh recovery must not call source") })
    }
}
fn operation(
    root: &std::path::Path,
    source: Arc<dyn PrivateSettlementContextSource>,
) -> OneCallPrivateSettlementOperation {
    let mut native = NativeFinalityFixture::start("private-native-session");
    let mut builder =
        client_with_base_url(&"http://session.invalid/".parse().unwrap()).to_builder();
    builder.network_id = native.network_id();
    builder.chain = native.chain_id().parse().unwrap();
    let client = builder.build().unwrap();
    let (proof, intent) = crate::cold_context_tests::context(&client);
    let verifier = native.verifier();
    let block = native.block_with_submitted_work(native.next_header());
    let finality =
        native.certify_with_world_root(block, proof.contexts[0].pool.world.root().unwrap());
    let client = client
        .to_builder()
        .http_transport(Arc::new(crate::cold_context_tests::FinalityTransport(
            finality,
        )))
        .build()
        .unwrap();
    OneCallPrivateSettlementOperation::with_source(
        client.account_client().unwrap(),
        crate::cold_context_tests::request(&client, &proof, &intent),
        Arc::new(FileWorkflowStorage::open_or_create(root.join("journals")).unwrap()),
        Arc::new(FilePrivateSettlementArtifacts::open(root.join("artifacts"), &[63; 32]).unwrap()),
        source,
        Arc::new(NoWallet),
        verifier,
    )
    .unwrap()
}
fn limits() -> NativePrivateSettlementLimits {
    NativePrivateSettlementLimits {
        maximum_sessions: NonZeroU16::new(1).unwrap(),
        maximum_original_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        maximum_artifact_bytes: NonZeroUsize::new(64 * 1024 * 1024).unwrap(),
        maximum_prepared_bytes: NonZeroUsize::new(64 * 1024 * 1024).unwrap(),
    }
}
async fn opened(
    host: &NativePrivateSettlementHost,
    operation: OneCallPrivateSettlementOperation,
) -> NativePrivateSettlementSessionId {
    let ticket = host.admit().unwrap().open(operation).unwrap();
    assert_eq!(
        host.wait(ticket).await.unwrap(),
        NativePrivateSettlementStatus::Ready
    );
    assert!(matches!(
        host.take(ticket).unwrap(),
        NativePrivateSettlementOutcome::Opened
    ));
    ticket.session()
}

#[tokio::test(flavor = "current_thread")]
async fn admission_and_global_ids_are_finite_before_open_and_not_reused_across_hosts() {
    let first = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
    let token = first.admit().unwrap();
    let first_id = token.id();
    assert!(matches!(
        first.admit(),
        Err(NativePrivateSettlementSessionError::Capacity)
    ));
    // No operation/decoder/provider was needed to establish admission refusal.
    drop(token);
    let second_id = first.admit().unwrap().id();
    assert!(second_id.get() > first_id.get());
    first.shutdown().await.unwrap();
    let second = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
    assert!(second.admit().unwrap().id().get() > second_id.get());
    second.shutdown().await.unwrap();
    let mut oversized = limits();
    oversized.maximum_original_bytes = NonZeroUsize::new(usize::MAX).unwrap();
    assert!(matches!(
        NativePrivateSettlementHost::new(Handle::current(), oversized),
        Err(NativePrivateSettlementSessionError::Capacity)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn queued_command_close_and_dropped_waiters_preserve_one_result_and_no_fresh_effects() {
    let root = tempfile::tempdir().unwrap();
    let host = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
    let id = opened(&host, operation(root.path(), Arc::new(NoSource))).await;
    let ticket = host
        .start(
            id,
            NativePrivateSettlementCommand::Prepared,
            PrivateSettlementRunOptions::default(),
        )
        .unwrap();
    assert!(matches!(
        host.start(
            id,
            NativePrivateSettlementCommand::Recover,
            PrivateSettlementRunOptions::default()
        ),
        Err(NativePrivateSettlementSessionError::Busy)
    ));
    drop(host.wait(ticket));
    assert_eq!(
        host.wait(ticket).await.unwrap(),
        NativePrivateSettlementStatus::Ready
    );
    assert!(matches!(
        host.take(ticket).unwrap(),
        NativePrivateSettlementOutcome::NotPrepared
    ));
    assert!(matches!(
        host.take(ticket),
        Err(NativePrivateSettlementSessionError::NotReady)
    ));
    let next = host
        .start(
            id,
            NativePrivateSettlementCommand::Recover,
            PrivateSettlementRunOptions::default(),
        )
        .unwrap();
    assert!(matches!(
        host.status(ticket),
        Err(NativePrivateSettlementSessionError::Handle)
    ));
    // Eager close before the supervisor polls this queued command cannot dispose its original.
    let close = host.close(id);
    assert_eq!(
        host.status(next).unwrap(),
        NativePrivateSettlementStatus::Closing
    );
    drop(close);
    let close_again = host.close(id);
    tokio::time::timeout(Duration::from_secs(5), close_again)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        std::fs::read_dir(root.path().join("journals"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("artifacts"))
            .unwrap()
            .count(),
        0
    );
    host.shutdown().await.unwrap();
}

struct HeldTls {
    entered: Option<oneshot::Sender<()>>,
    release: std::sync::mpsc::Receiver<()>,
}
impl Drop for HeldTls {
    fn drop(&mut self) {
        let _ = self.entered.take().unwrap().send(());
        let _ = self.release.recv();
    }
}
thread_local! { static SOURCE_TLS: RefCell<Option<HeldTls>> = const { RefCell::new(None) }; }
struct HeldSource {
    hold: Mutex<Option<(oneshot::Sender<()>, std::sync::mpsc::Receiver<()>)>>,
    storage: Arc<FileWorkflowStorage>,
    id: WorkflowId,
}
impl PrivateSettlementContextSource for HeldSource {
    fn fetch(
        &self,
        _: u64,
        _: PrivateSettlementContextRequestV1,
    ) -> WorkflowFuture<'_, Result<PrivateSettlementContextsProofV1, OneCallPrivateSettlementError>>
    {
        Box::pin(async move {
            let (observed, release) = self.hold.lock().unwrap().take().unwrap();
            let original_lease = self.storage.acquire_runner(&self.id).unwrap();
            let permit = Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap();
            let (entered, entered_rx) = oneshot::channel();
            let work = run_native_worker(permit, original_lease, move || {
                SOURCE_TLS.with(|tls| {
                    *tls.borrow_mut() = Some(HeldTls {
                        entered: Some(entered),
                        release,
                    })
                });
            });
            tokio::pin!(work);
            tokio::select! {
                _ = &mut work => panic!("original source native TLS returned before release"),
                _ = entered_rx => {}
            }
            let _ = observed.send(());
            panic!("restricted-provider-panic-canary");
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn command_panic_and_cancelled_close_hold_session_and_original_source_lease_through_tls() {
    let root = tempfile::tempdir().unwrap();
    let source_path = root.path().join("source-journal");
    let source_storage = Arc::new(FileWorkflowStorage::open_or_create(&source_path).unwrap());
    let source_id = WorkflowId::from_bytes([84; 32]);
    let (entered, entered_rx) = oneshot::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    let source = Arc::new(HeldSource {
        hold: Mutex::new(Some((entered, release_rx))),
        storage: source_storage,
        id: source_id,
    });
    let host = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
    let id = opened(&host, operation(root.path(), source)).await;
    let ticket = host
        .start(
            id,
            NativePrivateSettlementCommand::Prepare,
            PrivateSettlementRunOptions::default(),
        )
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), entered_rx)
        .await
        .unwrap()
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(20), host.wait(ticket))
            .await
            .is_err()
    );
    let reopened = FileWorkflowStorage::open_or_create(&source_path).unwrap();
    match reopened.acquire_runner(&source_id) {
        Err(error) => assert_eq!(error.kind, WorkflowStorageErrorKind::Busy),
        Ok(_) => panic!("original source lease released before native TLS"),
    }
    drop(host.close(id));
    assert!(matches!(
        host.admit(),
        Err(NativePrivateSettlementSessionError::Capacity)
    ));
    assert!(matches!(
        host.start(
            id,
            NativePrivateSettlementCommand::Recover,
            PrivateSettlementRunOptions::default()
        ),
        Err(NativePrivateSettlementSessionError::Closing)
    ));
    let closing = host.close(id);
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), closing)
        .await
        .unwrap()
        .unwrap();
    assert!(reopened.acquire_runner(&source_id).is_ok());
    assert!(host.admit().is_ok());
    drop(host.shutdown());
    tokio::time::timeout(Duration::from_secs(5), host.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[test]
fn stopped_runtime_discards_unpolled_supervisor_without_false_close_or_shutdown_hang() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let handle = runtime.handle().clone();
    drop(runtime);
    let host = NativePrivateSettlementHost::new(handle, limits()).unwrap();
    let observer = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    observer.block_on(async {
        assert!(matches!(
            host.admit(),
            Err(NativePrivateSettlementSessionError::Unavailable)
        ));
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), host.shutdown())
                .await
                .unwrap(),
            Err(NativePrivateSettlementSessionError::Unavailable)
        );
    });
}

#[tokio::test(flavor = "current_thread")]
async fn original_request_capacity_failure_is_typed_redacted_and_keeps_durable_originals() {
    let root = tempfile::tempdir().unwrap();
    let mut small = limits();
    small.maximum_original_bytes = NonZeroUsize::new(1).unwrap();
    let host = NativePrivateSettlementHost::new(Handle::current(), small).unwrap();
    let ticket = host
        .admit()
        .unwrap()
        .open(operation(root.path(), Arc::new(NoSource)))
        .unwrap();
    host.wait(ticket).await.unwrap();
    assert!(matches!(
        host.take(ticket).unwrap(),
        NativePrivateSettlementOutcome::Failed(NativePrivateSettlementCommandError::Operation(
            OneCallPrivateSettlementError::Capacity
        ))
    ));
    assert!(matches!(
        host.start(
            ticket.session(),
            NativePrivateSettlementCommand::Run,
            PrivateSettlementRunOptions::default()
        ),
        Err(NativePrivateSettlementSessionError::NotOpen)
    ));
    assert_eq!(
        std::fs::read_dir(root.path().join("artifacts"))
            .unwrap()
            .count(),
        0
    );
    host.close(ticket.session()).await.unwrap();
    host.shutdown().await.unwrap();
    let failure = NativePrivateSettlementOutcome::error(OneCallPrivateSettlementError::Provider(
        eyre::eyre!("restricted-error-canary"),
    ));
    assert!(!format!("{failure:?}").contains("canary"));
}

#[test]
fn actual_original_and_repeated_manifest_prepared_bounds_accept_exact_size_and_refuse_one_less() {
    let root = tempfile::tempdir().unwrap();
    let operation = operation(root.path(), Arc::new(NoSource));
    let mut local = limits();
    let bytes = bounds::original_bytes(&operation).unwrap();
    local.maximum_original_bytes = NonZeroUsize::new(bytes).unwrap();
    bounds::original(&operation, local).unwrap();
    local.maximum_original_bytes = NonZeroUsize::new(bytes - 1).unwrap();
    assert!(matches!(
        bounds::original(&operation, local),
        Err(OneCallPrivateSettlementError::Capacity)
    ));

    // This is a size fixture only; synthetic proof bytes are never submitted or verified as a proof.
    let fixture = crate::wallet_store::test_fixture::complete();
    let bytes = bounds::prepared_bytes(&operation, &fixture.prepared).unwrap();
    let manifest_frames = fixture
        .prepared
        .materials()
        .iter()
        .map(|material| norito::canonical_frame_len(&material.manifest).unwrap())
        .sum::<usize>();
    assert!(
        bytes >= manifest_frames,
        "every material retains its own full manifest"
    );
    local.maximum_prepared_bytes = NonZeroUsize::new(bytes).unwrap();
    bounds::prepared(&operation, &fixture.prepared, local).unwrap();
    local.maximum_prepared_bytes = NonZeroUsize::new(bytes - 1).unwrap();
    assert!(matches!(
        bounds::prepared(&operation, &fixture.prepared, local),
        Err(OneCallPrivateSettlementError::Capacity)
    ));
}

#[test]
fn typed_verified_success_and_pending_local_cleanup_never_become_generic_failure() {
    let fixture = crate::wallet_store::test_fixture::complete();
    let hash = fixture.receipt.transaction_hash();
    let success = NativePrivateSettlementOutcome::Verified {
        outcome: VerifiedPrivateSettlementOutcome::Settled(fixture.receipt.clone()),
        progress: None,
    };
    let NativePrivateSettlementOutcome::Verified { outcome, progress } = success else {
        panic!("success type lost")
    };
    assert_eq!(outcome.settled_receipt().unwrap().transaction_hash(), hash);
    assert!(progress.is_none());
    let cause = OneCallPrivateSettlementError::Provider(eyre::eyre!("restricted-cleanup-canary"));
    let outcome = NativePrivateSettlementOutcome::error(
        OneCallPrivateSettlementError::ReconciliationPending(Box::new(
            PrivateSettlementReconciliationPending::new(
                VerifiedPrivateSettlementOutcome::Settled(fixture.receipt),
                None,
                cause,
            ),
        )),
    );
    assert!(!format!("{outcome:?}").contains("canary"));
    let NativePrivateSettlementOutcome::ReconciliationPending(original) = outcome else {
        panic!("settled money was downgraded")
    };
    let (outcome, progress, cause) = original.into_parts();
    assert_eq!(outcome.settled_receipt().unwrap().transaction_hash(), hash);
    assert!(progress.is_none());
    assert!(matches!(cause, OneCallPrivateSettlementError::Provider(_)));
}

#[test]
fn typed_verified_abort_and_pending_local_release_preserve_original_authority() {
    let fixture = crate::wallet_store::test_fixture::complete();
    let abort = crate::wallet_store::test_fixture::verified_abort(
        &fixture,
        "private-wallet-all-leg-cleanup",
    );
    let hash = abort.transaction_hash();
    let value = NativePrivateSettlementOutcome::Verified {
        outcome: VerifiedPrivateSettlementOutcome::Aborted(abort.clone()),
        progress: None,
    };
    let NativePrivateSettlementOutcome::Verified { outcome, progress } = value else {
        panic!("verified Abort type lost")
    };
    assert!(outcome.settled_receipt().is_none());
    assert_eq!(outcome.abort_receipt().unwrap().transaction_hash(), hash);
    assert!(progress.is_none());
    let value = NativePrivateSettlementOutcome::error(
        OneCallPrivateSettlementError::ReconciliationPending(Box::new(
            PrivateSettlementReconciliationPending::new(
                VerifiedPrivateSettlementOutcome::Aborted(abort),
                None,
                OneCallPrivateSettlementError::Provider(eyre::eyre!("restricted-abort-canary")),
            ),
        )),
    );
    assert!(!format!("{value:?}").contains("canary"));
    let NativePrivateSettlementOutcome::ReconciliationPending(original) = value else {
        panic!("authenticated Abort was downgraded")
    };
    let (outcome, progress, cause) = original.into_parts();
    assert!(outcome.settled_receipt().is_none());
    assert_eq!(outcome.abort_receipt().unwrap().transaction_hash(), hash);
    assert!(progress.is_none());
    assert!(matches!(cause, OneCallPrivateSettlementError::Provider(_)));
}

#[derive(Debug)]
struct HeldError {
    entered: Mutex<Option<oneshot::Sender<()>>>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
}
impl std::fmt::Display for HeldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("original controlled custody failure")
    }
}
impl std::error::Error for HeldError {}
impl Drop for HeldError {
    fn drop(&mut self) {
        let _ = self.entered.lock().unwrap().take().unwrap().send(());
        let _ = self.release.lock().unwrap().recv();
    }
}
struct ErrorSource(Mutex<Option<HeldError>>);
impl PrivateSettlementContextSource for ErrorSource {
    fn fetch(
        &self,
        _: u64,
        _: PrivateSettlementContextRequestV1,
    ) -> WorkflowFuture<'_, Result<PrivateSettlementContextsProofV1, OneCallPrivateSettlementError>>
    {
        Box::pin(async move {
            Err(OneCallPrivateSettlementError::Provider(eyre::Report::new(
                self.0.lock().unwrap().take().unwrap(),
            )))
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn ready_original_error_disposal_is_off_executor_and_close_cannot_refund_early() {
    let root = tempfile::tempdir().unwrap();
    let (entered, entered_rx) = oneshot::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    let source = Arc::new(ErrorSource(Mutex::new(Some(HeldError {
        entered: Mutex::new(Some(entered)),
        release: Mutex::new(release_rx),
    }))));
    let host = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
    let id = opened(&host, operation(root.path(), source)).await;
    let ticket = host
        .start(
            id,
            NativePrivateSettlementCommand::Prepare,
            PrivateSettlementRunOptions::default(),
        )
        .unwrap();
    assert_eq!(
        host.wait(ticket).await.unwrap(),
        NativePrivateSettlementStatus::Ready
    );
    drop(host.close(id));
    // If unclaimed result disposal ran on this current-thread executor, this wake/release
    // could not execute and the test would deadlock. It belongs to the canonical join owner.
    tokio::time::timeout(Duration::from_secs(5), entered_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        host.status(ticket).unwrap(),
        NativePrivateSettlementStatus::Closing
    );
    assert!(matches!(
        host.admit(),
        Err(NativePrivateSettlementSessionError::Capacity)
    ));
    let closed = host.close(id);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), host.shutdown())
            .await
            .is_err()
    );
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), closed)
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), host.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[test]
fn retained_values_and_concurrent_export_buffers_share_checked_aggregate() {
    assert!(limits().validate().is_ok());
    // One session and one export retain the same maximum result allowance twice.
    // This reaches the arithmetic ceiling exactly, without allocating either value.
    let result_allowance = (usize::MAX - 3) / 2;
    let mut exact = NativePrivateSettlementLimits {
        maximum_sessions: NonZeroU16::new(1).unwrap(),
        maximum_original_bytes: NonZeroUsize::new(1).unwrap(),
        maximum_artifact_bytes: NonZeroUsize::new(2).unwrap(),
        maximum_prepared_bytes: NonZeroUsize::new(result_allowance).unwrap(),
    };
    assert!(result_allowance >= NATIVE_PRIVATE_SETTLEMENT_TERMINAL_ALLOWANCE);
    assert_eq!(3 + 2 * result_allowance, usize::MAX);
    assert!(exact.validate().is_ok());
    exact.maximum_prepared_bytes = NonZeroUsize::new(result_allowance + 1).unwrap();
    // The old session-only sum still fits; the additional export makes it overflow.
    assert!(3_usize.checked_add(result_allowance + 1).is_some());
    assert!(matches!(
        exact.validate(),
        Err(NativePrivateSettlementSessionError::Capacity)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn taken_result_export_bound_and_eager_shutdown_share_original_host_admission() {
    let mut limits = limits();
    limits.maximum_sessions = NonZeroU16::new(2).unwrap();
    let host = NativePrivateSettlementHost::new(Handle::current(), limits).unwrap();
    let first = host.admit_export().unwrap();
    let second = host.admit_export().unwrap();
    assert!(matches!(
        host.admit_export(),
        Err(NativePrivateSettlementSessionError::Capacity)
    ));
    drop(first);
    let replacement = host.admit_export().unwrap();
    // Closing is eager even if this returned waiter is never polled.
    drop(host.shutdown());
    assert!(matches!(
        host.admit_export(),
        Err(NativePrivateSettlementSessionError::Closing)
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), host.shutdown())
            .await
            .is_err()
    );
    drop(second);
    drop(replacement);
    tokio::time::timeout(Duration::from_secs(5), host.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        host.admit_export(),
        Err(NativePrivateSettlementSessionError::Closing)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_taken_result_export_keeps_host_shutdown_pending_through_native_tls_exit() {
    let host = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
    let lease = host.admit_export().unwrap();
    let permit = Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap();
    let (entered, entered_rx) = oneshot::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    let waiter = tokio::spawn(run_native_worker(permit, lease, move || {
        SOURCE_TLS.with(|tls| {
            *tls.borrow_mut() = Some(HeldTls {
                entered: Some(entered),
                release: release_rx,
            })
        });
        vec![7_u8; 16]
    }));
    tokio::time::timeout(Duration::from_secs(5), entered_rx)
        .await
        .unwrap()
        .unwrap();
    waiter.abort();
    match waiter.await {
        Err(error) => assert!(error.is_cancelled()),
        Ok(_) => panic!("canceled export waiter returned before native TLS exit"),
    }
    drop(host.shutdown());
    assert!(
        tokio::time::timeout(Duration::from_millis(20), host.shutdown())
            .await
            .is_err()
    );
    assert_eq!(
        host.shared.registry().exports,
        1,
        "the actual native TLS still owns its original export lease"
    );
    drop(release);
    tokio::time::timeout(Duration::from_secs(5), host.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(host.shared.registry().exports, 0);
}
