//! Deferred receipt metadata capacity is independent of proving and never mutates custody.

use super::*;
use crate::kagemusha_wallet_state_v1::capacity::deferred_archive_capacity;

#[test]
fn deferred_capacity_covers_redundant_bounded_paths_and_retained_intent() {
    let receipt = KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1;
    let plain = deferred_archive_capacity(receipt, false).unwrap();
    let anchored = deferred_archive_capacity(receipt, true).unwrap();
    let envelope = archive::METADATA_BOUND as u64;
    let paths = 6 * 258 * (index::INDEX_VALUE_LIMIT as u64 + 2 * envelope);
    assert!(plain.storage_bytes > 2 * (receipt as u64 + envelope + paths));
    assert!(plain.memory_bytes > 6 * (receipt as u64 + envelope + paths));
    assert!(anchored.storage_bytes >= plain.storage_bytes + 2 * REQUEST_MAX_BYTES as u64);
    assert!(anchored.memory_bytes >= plain.memory_bytes + 6 * REQUEST_MAX_BYTES as u64);
    assert!(matches!(
        deferred_archive_capacity(receipt + 1, false),
        Err(Error::Invalid("deferred receipt size"))
    ));
    assert!(matches!(
        deferred_archive_capacity(usize::MAX, true),
        Err(Error::Invalid("deferred receipt size"))
    ));
}

#[test]
fn deferred_capacity_refusals_preserve_custody_and_propagate_probe_errors() {
    let mut wallet = wallet();
    let receipt = KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1;
    let needed = deferred_archive_capacity(receipt, true).unwrap();
    // An unrelated proving budget cannot turn receipt retention into proof admission.
    wallet.proofs.capacity =
        crate::kagemusha_wallet_capacity_v1::KagemushaWalletRuntimeCapacityV1 {
            storage_bytes: u64::MAX,
            memory_bytes: u64::MAX,
        };
    let before = wallet.archive.records.lock().unwrap().clone();
    let checkpoint = wallet.custody.checkpoint.clone();
    let signatures = wallet.custody.signatures;
    let observation = NativeWorkTurnV1::start(NativeWorkKindV1::Execute);
    wallet.custody.resources.storage_bytes = needed.storage_bytes - 1;
    wallet.custody.resources.memory_bytes = Some(needed.memory_bytes);
    assert!(matches!(
        wallet.require_deferred_archive_capacity(receipt, true),
        Err(Error::NotEnoughStorage)
    ));
    wallet.custody.resources.storage_bytes = needed.storage_bytes;
    wallet.custody.resources.memory_bytes = Some(needed.memory_bytes - 1);
    assert!(matches!(
        wallet.require_deferred_archive_capacity(receipt, true),
        Err(Error::NotEnoughMemory)
    ));
    wallet.custody.resources.memory_bytes = Some(needed.memory_bytes);
    wallet
        .require_deferred_archive_capacity(receipt, true)
        .unwrap();
    wallet.custody.resources.memory_bytes = None;
    wallet
        .require_deferred_archive_capacity(receipt, true)
        .unwrap();
    wallet.custody.fail_resources = true;
    assert!(matches!(
        wallet.require_deferred_archive_capacity(receipt, true),
        Err(Error::Provider(ProviderError::Unavailable(_)))
    ));
    let queries = wallet.custody.resource_queries;
    assert!(matches!(
        wallet.require_deferred_archive_capacity(usize::MAX, true),
        Err(Error::Invalid("deferred receipt size"))
    ));
    assert_eq!(
        wallet.custody.resource_queries, queries,
        "bound before probe"
    );
    let observed = observation.finish(true).admissions;
    assert_eq!(queries, 5);
    assert_eq!(
        observed.count, 5,
        "telemetry does not perform another probe"
    );
    let values: Vec<_> = observed.records.iter().flatten().collect();
    assert_eq!(values.len(), 5);
    for value in &values {
        assert_eq!(value.phase, NativeAdmissionPhaseV1::DeferredArchive);
        assert_eq!(value.required, needed);
    }
    assert_eq!(
        values.iter().map(|value| value.result).collect::<Vec<_>>(),
        [
            NativeAdmissionResultV1::StorageShortfall,
            NativeAdmissionResultV1::MemoryShortfall,
            NativeAdmissionResultV1::Passed,
            NativeAdmissionResultV1::Passed,
            NativeAdmissionResultV1::ProbeFailed,
        ]
    );
    assert_eq!(*wallet.archive.records.lock().unwrap(), before);
    assert_eq!(wallet.custody.checkpoint, checkpoint);
    assert_eq!(wallet.custody.signatures, signatures);
}
