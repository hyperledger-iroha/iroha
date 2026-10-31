//! Original post-validation artifact work retains real round custody through native cleanup.
//! Artifact proof bytes here are synthetic; these controls do not qualify native proving.

use super::*;
use iroha::workflow::{FileWorkflowStorage, WorkflowStorageErrorKind};
use std::{cell::RefCell, sync::atomic::AtomicBool, time::Duration};

fn require_busy(storage: &FileWorkflowStorage, id: &WorkflowId) {
    match storage.acquire_runner(id) {
        Err(error) => assert_eq!(error.kind, WorkflowStorageErrorKind::Busy),
        Ok(_) => panic!("original preparation round lease was released before native completion"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn preparation_authority_refuses_every_monetary_operation() {
    let binding = WorkflowBindingV1 {
        network: "original-preparation-network".into(),
        authority: "original-preparation-authority".into(),
    };
    let owner = NoPreparationEffects(binding.clone());
    let identity = TransactionIdentityV1 {
        signed_transaction_hash: [17; 32],
        entrypoint_hash: [17; 32],
    };
    assert_eq!(owner.binding(), binding);
    assert_eq!(
        owner.sign(&[]).await.unwrap_err().code,
        WorkflowErrorCode::UnsupportedOperation
    );
    assert_eq!(
        owner.identify(&[]).unwrap_err().code,
        WorkflowErrorCode::UnsupportedOperation
    );
    assert!(matches!(
        owner.dispatch(&[], &identity).await,
        DispatchOutcome::Refused { .. }
    ));
    assert!(matches!(
        owner.committed_outcome(&identity).await,
        CommittedOutcome::ReadFailed { .. }
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn original_artifact_completion_runs_off_executor_with_same_round_lease() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("journals");
    let storage = FileWorkflowStorage::open_or_create(&path).unwrap();
    let reopened = FileWorkflowStorage::open_or_create(&path).unwrap();
    let id = WorkflowId::from_bytes([71; 32]);
    let artifact = crate::artifacts::tests::fixture();
    let expected = artifact.digest().unwrap();
    let originals = OriginalArtifacts {
        artifacts: vec![artifact],
        max_carrier_bytes: PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 as u64,
        lease: storage.acquire_runner(&id).unwrap(),
    };
    let worker = Arc::new(tokio::sync::Semaphore::new(1));
    let executor_thread = std::thread::current().id();
    let actual = originals
        .complete(worker.clone(), move |artifacts| {
            assert_ne!(std::thread::current().id(), executor_thread);
            assert!(tokio::runtime::Handle::try_current().is_err());
            require_busy(&reopened, &id);
            assert_eq!(artifacts.len(), 1);
            artifacts[0].digest()
        })
        .await
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(worker.available_permits(), 1);
    assert!(storage.acquire_runner(&id).is_ok());
}

struct HeldCompletionTls {
    entered: Option<tokio::sync::oneshot::Sender<()>>,
    release: std::sync::mpsc::Receiver<()>,
}
impl Drop for HeldCompletionTls {
    fn drop(&mut self) {
        if let Some(entered) = self.entered.take() {
            let _ = entered.send(());
        }
        // Dropping the controller on an assertion failure also releases the original thread.
        let _ = self.release.recv();
    }
}
std::thread_local! {
    static COMPLETION_TLS: RefCell<Option<HeldCompletionTls>> = const { RefCell::new(None) };
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_original_completion_holds_round_lease_until_native_tls_exits() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("journals");
    let storage = FileWorkflowStorage::open_or_create(&path).unwrap();
    let id = WorkflowId::from_bytes([72; 32]);
    let originals = OriginalArtifacts {
        artifacts: vec![crate::artifacts::tests::fixture()],
        max_carrier_bytes: PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 as u64,
        lease: storage.acquire_runner(&id).unwrap(),
    };
    let worker = Arc::new(tokio::sync::Semaphore::new(1));
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    let task = tokio::spawn(originals.complete(worker.clone(), move |artifacts| {
        let _ = artifacts[0].digest()?;
        COMPLETION_TLS.with(|slot| {
            *slot.borrow_mut() = Some(HeldCompletionTls {
                entered: Some(entered),
                release: release_rx,
            });
        });
        Ok(())
    }));
    tokio::time::timeout(Duration::from_secs(5), entered_rx)
        .await
        .unwrap()
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(storage);
    let reopened = FileWorkflowStorage::open_or_create(&path).unwrap();
    require_busy(&reopened, &id);
    assert_eq!(worker.available_permits(), 0);
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while worker.available_permits() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(reopened.acquire_runner(&id).is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn original_completion_preserves_typed_cause_and_refuses_without_capacity() {
    let root = tempfile::tempdir().unwrap();
    let storage = FileWorkflowStorage::open_or_create(root.path().join("journals")).unwrap();
    let id = WorkflowId::from_bytes([73; 32]);
    let reached = Arc::new(AtomicBool::new(false));
    let marker = reached.clone();
    let original = OriginalArtifacts {
        artifacts: vec![crate::artifacts::tests::fixture()],
        max_carrier_bytes: PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 as u64,
        lease: storage.acquire_runner(&id).unwrap(),
    };
    assert!(matches!(
        original
            .complete(Arc::new(tokio::sync::Semaphore::new(0)), move |_| {
                marker.store(true, Ordering::SeqCst);
                Ok(())
            })
            .await,
        Err(OneCallPrivateSettlementError::Capacity)
    ));
    assert!(!reached.load(Ordering::SeqCst));
    let original = OriginalArtifacts {
        artifacts: vec![crate::artifacts::tests::fixture()],
        max_carrier_bytes: PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 as u64,
        lease: storage.acquire_runner(&id).unwrap(),
    };
    let error = original
        .complete(Arc::new(tokio::sync::Semaphore::new(1)), |_| {
            Err::<(), _>(OneCallPrivateSettlementError::Provider(eyre::Report::new(
                std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "original completion refusal",
                ),
            )))
        })
        .await
        .unwrap_err();
    assert!(
        matches!(error, OneCallPrivateSettlementError::Provider(cause)
        if cause.downcast_ref::<std::io::Error>().is_some_and(|error|
            error.kind() == std::io::ErrorKind::PermissionDenied))
    );
    assert!(storage.acquire_runner(&id).is_ok());
}
