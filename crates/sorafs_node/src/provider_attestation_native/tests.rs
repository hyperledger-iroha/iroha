//! Real portable custody controls; signed specimens test retention, not native finality.
use super::*;
use crate::provider_attestation_journal::{
    musubi_provider_attestation_journal_test_checkpoint_bytes_v1 as checkpoint,
    tests::native_inventory_test_attestation,
};

fn fixture() -> (
    tempfile::TempDir,
    NetworkId,
    ProviderId,
    MusubiProviderAttestationJournalPolicyV1,
) {
    let root = tempfile::tempdir().unwrap();
    let value = native_inventory_test_attestation(9, 3);
    let network = value.payload.binding.network_id;
    let provider = value.payload.binding.provider_id;
    let policy = MusubiProviderAttestationJournalPolicyV1::default();
    (root, network, provider, policy)
}
fn initialize() -> (tempfile::TempDir, NativeMusubiProviderAttestationCustodyV1) {
    let (root, network, provider, policy) = fixture();
    NativeMusubiProviderAttestationCustodyV1::initialize(root.path(), network, provider, policy)
        .unwrap();
    let custody =
        NativeMusubiProviderAttestationCustodyV1::open(root.path(), network, provider, policy)
            .unwrap();
    (root, custody)
}
#[test]
fn explicit_initialization_and_owned_reopen_preserve_all_originals() {
    let (root, network, provider, policy) = fixture();
    assert!(
        NativeMusubiProviderAttestationCustodyV1::open(root.path(), network, provider, policy)
            .is_err()
    );
    assert!(!root.path().join(DIRECTORY).exists());
    NativeMusubiProviderAttestationCustodyV1::initialize(root.path(), network, provider, policy)
        .unwrap();
    let custody =
        NativeMusubiProviderAttestationCustodyV1::open(root.path(), network, provider, policy)
            .unwrap();
    let original = std::fs::read(root.path().join(DIRECTORY).join(JOURNAL)).unwrap();
    assert!(
        NativeMusubiProviderAttestationCustodyV1::initialize(
            root.path(),
            network,
            provider,
            policy
        )
        .is_err()
    );
    assert!(
        NativeMusubiProviderAttestationCustodyV1::open(root.path(), network, provider, policy)
            .is_err()
    );
    let retained = custody.runtime();
    drop(custody);
    assert!(
        NativeMusubiProviderAttestationCustodyV1::open(root.path(), network, provider, policy)
            .is_err()
    );
    drop(retained);
    let _reopened =
        NativeMusubiProviderAttestationCustodyV1::open(root.path(), network, provider, policy)
            .unwrap();
    assert_eq!(
        std::fs::read(root.path().join(DIRECTORY).join(JOURNAL)).unwrap(),
        original
    );
}
#[tokio::test]
async fn exact_checkpoint_replay_recovers_lost_response_without_replacing_successor() {
    let (_root, custody) = initialize();
    let store = &custody.inventory.shared;
    let first = checkpoint(1, 0);
    let revision = musubi_provider_attestation_journal_checkpoint_revision_v1(&first);
    assert_eq!(
        store.compare_and_swap(None, first.clone()).await.unwrap(),
        MusubiProviderAttestationJournalCasOutcomeV1::Stored { revision }
    );
    // A caller that lost the successful write response may reconcile this exact original.
    assert_eq!(
        store.compare_and_swap(None, first).await.unwrap(),
        MusubiProviderAttestationJournalCasOutcomeV1::Stored { revision }
    );
    assert_eq!(
        store
            .compare_and_swap(None, checkpoint(2, 0))
            .await
            .unwrap(),
        MusubiProviderAttestationJournalCasOutcomeV1::Conflict
    );
    assert!(
        store
            .compare_and_swap(Some(revision), checkpoint(3, 0))
            .await
            .is_err()
    );
    let second = checkpoint(2, 0);
    let second_revision = musubi_provider_attestation_journal_checkpoint_revision_v1(&second);
    assert_eq!(
        store
            .compare_and_swap(Some(revision), second.clone())
            .await
            .unwrap(),
        MusubiProviderAttestationJournalCasOutcomeV1::Stored {
            revision: second_revision
        }
    );
    assert_eq!(
        store.load().await.unwrap().checkpoint_bytes(),
        Some(second.as_slice())
    );
}
#[tokio::test]
async fn exact_inventory_retention_reopens_and_conflicting_signed_evidence_is_refused() {
    let (root, custody) = initialize();
    let store = &custody.inventory.shared;
    let selected = store.binding;
    let policy = store.policy;
    let item =
        MusubiProviderAttestationInventoryItemV1::new(native_inventory_test_attestation(9, 3))
            .unwrap();
    let conflict =
        MusubiProviderAttestationInventoryItemV1::new(native_inventory_test_attestation(9, 4))
            .unwrap();
    assert_eq!(custody.inventory.put(item.clone()).await.unwrap(), 1);
    assert_eq!(custody.inventory.put(item.clone()).await.unwrap(), 1);
    assert_eq!(
        custody.inventory.put(conflict).await.unwrap_err(),
        MusubiProviderAttestationInventoryErrorV1::Conflict
    );
    let different_provider =
        MusubiProviderAttestationInventoryItemV1::new(native_inventory_test_attestation(10, 3))
            .unwrap();
    assert_eq!(
        custody.inventory.put(different_provider).await.unwrap_err(),
        MusubiProviderAttestationInventoryErrorV1::InvalidItem
    );
    drop(custody);
    let reopened = NativeMusubiProviderAttestationCustodyV1::open(
        root.path(),
        selected.network_id,
        selected.provider_id,
        policy,
    )
    .unwrap();
    let read = reopened
        .inventory
        .get(item.scope(), item.key())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.item(), &item);
    assert_eq!(read.inventory_revision(), 1);
    let inventory = reopened
        .inventory
        .inventory(item.scope())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(inventory.items(), &[item]);
}
#[test]
fn missing_history_and_foreign_scope_do_not_initialize_replacements() {
    let (root, custody) = initialize();
    let selected = custody.inventory.shared.binding;
    let policy = custody.inventory.shared.policy;
    drop(custody);
    let original = std::fs::read(root.path().join(DIRECTORY).join(JOURNAL)).unwrap();
    assert!(
        NativeMusubiProviderAttestationCustodyV1::open(
            root.path(),
            selected.network_id,
            ProviderId::new([10; 32]),
            policy
        )
        .is_err()
    );
    let mut changed_policy = policy;
    changed_policy.max_attempts -= 1;
    assert!(
        NativeMusubiProviderAttestationCustodyV1::open(
            root.path(),
            selected.network_id,
            selected.provider_id,
            changed_policy
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read(root.path().join(DIRECTORY).join(JOURNAL)).unwrap(),
        original
    );
    std::fs::remove_file(root.path().join(DIRECTORY).join(CLOCK)).unwrap();
    assert!(
        NativeMusubiProviderAttestationCustodyV1::open(
            root.path(),
            selected.network_id,
            selected.provider_id,
            policy
        )
        .is_err()
    );
    assert!(!root.path().join(DIRECTORY).join(CLOCK).exists());
}
#[tokio::test]
async fn clock_rollback_is_refused_without_granting_native_currentness() {
    let (_root, custody) = initialize();
    let store = &custody.inventory.shared;
    let original = store.clock().unwrap();
    store
        .encode(
            CLOCK,
            &ClockFile {
                binding: original.binding,
                floor_unix_ms: u64::MAX - 1,
            },
            PublishMode::Replace,
        )
        .unwrap();
    assert_eq!(
        store.now_unix_ms().await.unwrap_err(),
        MusubiProviderAttestationJournalErrorV1::ClockRollback
    );
    assert_eq!(store.clock().unwrap().floor_unix_ms, u64::MAX - 1);
}
#[test]
fn decoder_budget_corruption_and_hardlinks_refuse_before_original_use() {
    let (root, custody) = initialize();
    let store = &custody.inventory.shared;
    let result = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(4096, 4096, 4096, 0, 16),
        || store.journal(),
    );
    assert!(result.is_err());
    let bytes = std::fs::read(root.path().join(DIRECTORY).join(JOURNAL)).unwrap();
    store
        .directory
        .write_atomic(JOURNAL, b"not a canonical journal", PublishMode::Replace)
        .unwrap();
    assert!(store.journal().is_err());
    store
        .directory
        .write_atomic(JOURNAL, &bytes, PublishMode::Replace)
        .unwrap();
    std::fs::hard_link(
        root.path().join(DIRECTORY).join(JOURNAL),
        root.path().join("hardlink.nrt"),
    )
    .unwrap();
    assert!(store.journal().is_err());
    std::fs::remove_file(root.path().join("hardlink.nrt")).unwrap();
    assert!(store.journal().is_ok());
}

#[test]
fn initial_publication_is_complete_or_absent_under_refusal() {
    let (root, network, provider, policy) = fixture();
    let refused = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(4096, 4096, 4096, 0, 16),
        || {
            NativeMusubiProviderAttestationCustodyV1::initialize(
                root.path(),
                network,
                provider,
                policy,
            )
        },
    );
    assert!(refused.is_err());
    assert!(!root.path().join(DIRECTORY).exists());
    NativeMusubiProviderAttestationCustodyV1::initialize(root.path(), network, provider, policy)
        .unwrap();
    let directory = PrivateDirectory::open(root.path().join(DIRECTORY)).unwrap();
    let names = directory.entries(4).unwrap();
    assert_eq!(names.len(), 4);
    for name in [LOCK, JOURNAL, CLOCK, INVENTORY] {
        assert!(names.iter().any(|entry| entry == name));
    }
    assert_eq!(
        directory.open_read(LOCK).unwrap().metadata().unwrap().len(),
        0
    );
    NativeMusubiProviderAttestationCustodyV1::open(root.path(), network, provider, policy).unwrap();
}

#[tokio::test]
async fn canceled_file_job_retains_exclusive_slot_until_actual_completion() {
    let (_root, custody) = initialize();
    let store = custody.inventory.shared.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
    let worker_store = store.clone();
    let caller = tokio::spawn(async move {
        worker_store
            .run_blocking((), move |store| {
                started_tx.send(()).unwrap();
                release_rx
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .map_err(|_| ())?;
                let clock = store.clock().map_err(|_| ())?;
                finished_tx.send(clock.floor_unix_ms).unwrap();
                Ok::<_, ()>(())
            })
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), started_rx)
        .await
        .unwrap()
        .unwrap();
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    assert_eq!(store.run_blocking((), |_| Ok::<_, ()>(())).await, Err(()));
    release_tx.send(()).unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), finished_rx)
            .await
            .unwrap()
            .unwrap()
            > 0
    );
    // The finish signal precedes release of the real job's permit.
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while store.jobs.available_permits() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(store.run_blocking((), |_| Ok::<_, ()>(())).await, Ok(()));
}
