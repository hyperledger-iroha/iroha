//! DA ingest and persistence tests.
use super::*;
use crate::da::taikai;
use crate::da::taikai::taikai_ingest;
use crate::da::taikai::taikai_ingest::{
    AnchorSendError, AnchorSender, collect_pending_uploads, process_batch,
};
use crate::da::taikai::{
    TAIKAI_ANCHOR_INVALID_SUFFIX, TAIKAI_ANCHOR_READY_PREFIX, TAIKAI_ANCHOR_READY_SUFFIX,
    TAIKAI_ANCHOR_REQUEST_PREFIX, TAIKAI_ANCHOR_REQUEST_SUFFIX, TAIKAI_ANCHOR_SENTINEL_PREFIX,
    TAIKAI_ANCHOR_SENTINEL_SUFFIX, TAIKAI_SPOOL_SUBDIR, TAIKAI_TRM_LINEAGE_PREFIX,
    TAIKAI_TRM_LINEAGE_SUFFIX, TAIKAI_TRM_LOCK_PREFIX, TAIKAI_TRM_LOCK_SUFFIX,
    TAIKAI_TRM_PENDING_PREFIX, TAIKAI_TRM_PENDING_SUFFIX,
};
use crate::da::{
    DaReceiptLog, DaSpoolAction, DaSpoolActionOutput, DaSpoolBatch, DaSpooler, ReplayCursorStore,
};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use core::convert::TryInto;
use flate2::{Compression as FlateCompression, write::GzEncoder};
use http_body_util::BodyExt as _;
use iroha_config::parameters::actual::{
    DaTaikaiAnchor, LaneConfig as ConfigLaneConfig, Nexus as ConfigNexus, TelemetryProfile,
};
use iroha_core::{da::LaneEpoch, state::StateReadOnly, telemetry::Telemetry};
use iroha_crypto::{Algorithm, Hash, KeyPair, PrivateKey, Signature, SignatureOf};
use iroha_data_model::{
    Encode,
    account::AccountId,
    block::BlockHeader,
    da::{
        commitment::DaCommitmentBundle,
        ingest::{DaIngestAdmissionLaneV1, DaIngestAdmissionPolicyV1, DaStripeLayout},
        types::{BlobDigest, DaRentQuote, StorageTicketId},
    },
    name::Name,
    nexus::{
        DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog,
        LaneConfig as ModelLaneConfig, LaneId,
    },
    parameter::{Parameter, custom::CustomParameter},
    sorafs::{
        capacity::ProviderId,
        pin_registry::{ManifestAliasBinding, ManifestDigest},
    },
    taikai::{
        GuardDirectoryId, SegmentTimestamp, TAIKAI_ANCHOR_RECEIPT_SCHEMA_V1,
        TAIKAI_ANCHOR_RECEIPT_VERSION_V1, TaikaiAliasBinding, TaikaiAnchorReceiptBodyV1,
        TaikaiAnchorReceiptV1, TaikaiAvailabilityClass, TaikaiCarPointer, TaikaiCidIndexKey,
        TaikaiEnvelopeIndexes, TaikaiEventId, TaikaiGuardPolicy, TaikaiRenditionId,
        TaikaiRenditionRouteV1, TaikaiRoutingManifestV1, TaikaiSegmentEnvelopeV1,
        TaikaiSegmentSigningBodyV1, TaikaiSegmentSigningManifestV1, TaikaiSegmentWindow,
        TaikaiStreamId, TaikaiTimeIndexKey,
    },
};
use iroha_primitives::{json::Json, numeric::XorQuantity};
use iroha_telemetry::metrics::Metrics;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use norito::{
    NoritoDeserialize, from_bytes,
    json::{self, Value},
    to_bytes,
};
use reqwest::Url;
use sorafs_car::{CarBuildPlan, PersistedChunkRecord};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1, CouncilSignature, ProfileId,
    ProviderAdmissionCouncilPolicy, canonical_manifest_root_cid,
    pdp::{PdpCommitmentV1, PdpMerkleTreeV1},
    pin_registry::{
        AliasBindingV1, AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest,
    },
};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, ErrorKind, Read, Write},
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc, Barrier, LazyLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use tempfile::tempdir;
use tokio::{fs as async_fs, sync::Mutex as AsyncMutex};
fn checked_signature(private_key: &PrivateKey, payload: &[u8]) -> Signature {
    Signature::try_new(private_key, payload).expect("test fixture signing should succeed")
}
fn checked_taikai_segment_signature(
    private_key: &PrivateKey,
    body: &TaikaiSegmentSigningBodyV1,
) -> SignatureOf<TaikaiSegmentSigningBodyV1> {
    SignatureOf::try_new(private_key, body).expect("test Taikai segment signing should succeed")
}
fn checked_fixture_keypair(seed: Vec<u8>, algorithm: Algorithm) -> KeyPair {
    KeyPair::try_from_seed(seed, algorithm).expect("test fixture key derivation should succeed")
}
fn checked_fixture_ed25519_keypair(seed: u8) -> KeyPair {
    checked_fixture_keypair(vec![seed; 32], Algorithm::Ed25519)
}
fn checked_random_keypair() -> KeyPair {
    KeyPair::try_random().expect("test fixture random key generation should succeed")
}
fn checked_random_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
    KeyPair::try_random_with_algorithm(algorithm)
        .expect("test fixture algorithm-specific random key generation should succeed")
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn da_ingest_compute_jobs_respect_configured_parallelism() {
    const LIMIT: usize = 2;
    let limiter = Arc::new(tokio::sync::Semaphore::new(LIMIT));
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (started_tx, started_rx) = mpsc::channel();
    let mut release_senders = Vec::with_capacity(LIMIT);
    let mut tasks = Vec::with_capacity(LIMIT);
    for id in 0..LIMIT {
        let (release_tx, release_rx) = mpsc::channel();
        release_senders.push(Some(release_tx));
        let limiter = Arc::clone(&limiter);
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        let started_tx = started_tx.clone();
        tasks.push(tokio::spawn(run_da_ingest_compute_job(
            limiter,
            move || {
                let active_now = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(active_now, Ordering::SeqCst);
                started_tx.send(id).expect("report started compute job");
                release_rx.recv().expect("release compute job");
                active.fetch_sub(1, Ordering::SeqCst);
                Ok::<_, (StatusCode, String)>(id)
            },
        )));
    }
    drop(started_tx);
    let first = started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("first compute job should start");
    let second = started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("second compute job should start");
    assert_ne!(first, second);
    assert_eq!(peak.load(Ordering::SeqCst), LIMIT);
    let rejected_job_ran = Arc::new(AtomicBool::new(false));
    let rejected_job_ran_in_worker = Arc::clone(&rejected_job_ran);
    let err = tokio::time::timeout(
        Duration::from_secs(2),
        run_da_ingest_compute_job(Arc::clone(&limiter), move || {
            rejected_job_ran_in_worker.store(true, Ordering::SeqCst);
            Ok::<_, (StatusCode, String)>(())
        }),
    )
    .await
    .expect("saturated compute admission must return promptly")
    .expect_err("saturated compute admission must fail fast");
    assert_eq!(err.0, StatusCode::SERVICE_UNAVAILABLE);
    assert!(err.1.contains("capacity is saturated"));
    assert!(
        !rejected_job_ran.load(Ordering::SeqCst),
        "a rejected compute job must never reach a physical worker"
    );
    for sender in release_senders.into_iter().flatten() {
        let _ = sender.send(());
    }
    for task in tasks {
        task.await
            .expect("compute task should join")
            .expect("compute job should succeed");
    }
    assert_eq!(active.load(Ordering::SeqCst), 0);
    assert_eq!(peak.load(Ordering::SeqCst), LIMIT);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_da_ingest_keeps_compute_permit_until_physical_worker_exits() {
    let limiter = Arc::new(tokio::sync::Semaphore::new(1));
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let task = tokio::spawn(run_da_ingest_compute_job(Arc::clone(&limiter), move || {
        started_tx.send(()).expect("report started compute job");
        release_rx.recv().expect("release physical compute job");
        Ok::<_, (StatusCode, String)>(())
    }));
    started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("physical compute job should start");
    task.abort();
    let join_error = task
        .await
        .expect_err("aborted request task should not complete normally");
    assert!(join_error.is_cancelled());
    assert!(
        limiter.clone().try_acquire_owned().is_err(),
        "request cancellation must not release capacity while physical work continues"
    );
    release_tx.send(()).expect("release physical compute job");
    let permit = tokio::time::timeout(Duration::from_secs(2), limiter.clone().acquire_owned())
        .await
        .expect("physical worker should release capacity")
        .expect("compute limiter should remain open");
    drop(permit);
}
#[test]
fn checked_fixture_ed25519_keypair_uses_fallible_seed_derivation() {
    assert_eq!(
        checked_fixture_ed25519_keypair(0x50).algorithm(),
        Algorithm::Ed25519
    );
    assert!(
        KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
        "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
    );
}
#[test]
fn replay_cursor_temp_path_keeps_suffixes() {
    let base = Path::new("/var/lib/iroha/replay_cursors.norito.json");
    let tmp = persistence::replay_cursor_temp_path(base);
    assert_eq!(
        tmp,
        Path::new("/var/lib/iroha/replay_cursors.norito.json.tmp")
    );
}
// Error-envelope negotiation coverage is kept in an included child so this
// DA test module remains within the repository source budget.
include!("tests/error_response_tests.rs");
#[test]
fn parse_storage_ticket_hex_validates_variants() {
    let valid = format!("0x{}", "aa".repeat(32));
    let parsed = parse_storage_ticket_hex(&valid).expect("valid ticket");
    assert_eq!(parsed.len(), 32);
    assert!(parse_storage_ticket_hex("").is_err());
    assert!(parse_storage_ticket_hex("zz").is_err());
    assert!(parse_storage_ticket_hex("ab").is_err(), "too short");
}
fn spool_artifact_path(
    spool_dir: &Path,
    prefix: &str,
    ticket: &StorageTicketId,
    sequence: u64,
    fingerprint: [u8; 32],
) -> PathBuf {
    spool_artifact_path_for_key(
        spool_dir,
        prefix,
        LaneId::new(1),
        1,
        sequence,
        ticket,
        fingerprint,
    )
}
fn spool_artifact_path_for_key(
    spool_dir: &Path,
    prefix: &str,
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    ticket: &StorageTicketId,
    fingerprint: [u8; 32],
) -> PathBuf {
    match prefix {
        "manifest-" | "pdp-commitment-" => {
            let file_name = if prefix == "manifest-" {
                "manifest.norito"
            } else {
                "pdp-commitment.norito"
            };
            let artifact_dir = persistence::ticket_artifact_dir(spool_dir, ticket);
            fs::create_dir_all(&artifact_dir).expect("create ticket artifact fixture directory");
            artifact_dir.join(file_name)
        }
        "da-commitment-" | "da-commitment-schedule-" | "da-pin-intent-" | "da-pin-scope-" => {
            let lane = lane_id.as_u32();
            let ticket_hex = hex::encode(ticket.as_bytes());
            let fingerprint_hex = hex::encode(fingerprint);
            spool_dir.join(format!(
                "{prefix}{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito"
            ))
        }
        other => panic!("unknown spool artifact prefix `{other}`"),
    }
}
fn write_sample_manifest_artifact(
    dir: &Path,
) -> (
    ManifestFixtureContext,
    persistence::LoadedManifestArtifact,
    BlobDigest,
) {
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let ticket = context.artifacts.storage_ticket;
    let path = spool_artifact_path_for_key(
        dir,
        "manifest-",
        context.request.lane_id,
        context.request.epoch,
        context.request.sequence,
        &ticket,
        *context.artifacts.fingerprint.as_bytes(),
    );
    fs::write(&path, &context.artifacts.encoded).expect("manifest artifact");
    let artifact =
        persistence::load_manifest_artifact_from_spool(dir, &ticket).expect("manifest artifact");
    let manifest_hash = BlobDigest::from_hash(blake3_hash(&artifact.bytes));
    (context, artifact, manifest_hash)
}
#[tokio::test]
async fn da_spooler_executes_batch_before_ack() {
    let marker = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let spooler = DaSpooler::spawn(
        NonZeroUsize::new(4).expect("non-zero queue"),
        NonZeroUsize::new(2).expect("non-zero batch"),
        crate::routing::MaybeTelemetry::disabled(),
    );
    let mut batch = DaSpoolBatch::new();
    let marker_for_action = Arc::clone(&marker);
    batch.push(DaSpoolAction::new("test_artifact", move || {
        marker_for_action.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(DaSpoolActionOutput::None)
    }));
    let report = spooler.submit(batch).await;
    assert_eq!(marker.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(report.actions().len(), 1);
    assert_eq!(report.actions()[0].kind(), "test_artifact");
    assert!(report.actions()[0].error().is_none());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn da_spooler_cancelled_pending_send_restores_queue_depth() {
    let spooler = DaSpooler::spawn(
        NonZeroUsize::new(1).expect("non-zero queue"),
        NonZeroUsize::new(1).expect("non-zero batch"),
        crate::routing::MaybeTelemetry::disabled(),
    );
    let release = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let release_for_action = Arc::clone(&release);
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let mut first_batch = DaSpoolBatch::new();
    first_batch.push(DaSpoolAction::new("blocked", move || {
        let _ = started_tx.send(());
        let (lock, wake) = &*release_for_action;
        let mut released = lock.lock().expect("release lock");
        while !*released {
            released = wake.wait(released).expect("release wait");
        }
        Ok(DaSpoolActionOutput::None)
    }));
    let first_spooler = Arc::clone(&spooler);
    let first = tokio::spawn(async move { first_spooler.submit(first_batch).await });
    started_rx.await.expect("first worker action started");

    let mut second_batch = DaSpoolBatch::new();
    second_batch.push(DaSpoolAction::new("queued", || {
        Ok(DaSpoolActionOutput::None)
    }));
    let second_spooler = Arc::clone(&spooler);
    let second = tokio::spawn(async move { second_spooler.submit(second_batch).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while spooler.queued_depth() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("second batch must fill the bounded queue");

    let mut third_batch = DaSpoolBatch::new();
    third_batch.push(DaSpoolAction::new("cancelled", || {
        Ok(DaSpoolActionOutput::None)
    }));
    let third_spooler = Arc::clone(&spooler);
    let third = tokio::spawn(async move { third_spooler.submit(third_batch).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while spooler.queued_depth() != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("third submit must wait behind the full queue");
    third.abort();
    assert!(
        third
            .await
            .expect_err("third submit must cancel")
            .is_cancelled()
    );
    assert_eq!(
        spooler.queued_depth(),
        1,
        "cancelling a pending send must release its reserved depth"
    );

    let (lock, wake) = &*release;
    *lock.lock().expect("release lock") = true;
    wake.notify_all();
    tokio::time::timeout(Duration::from_secs(2), first)
        .await
        .expect("first submit must finish")
        .expect("first submit task");
    tokio::time::timeout(Duration::from_secs(2), second)
        .await
        .expect("second submit must finish")
        .expect("second submit task");
    assert_eq!(spooler.queued_depth(), 0);
}
#[test]
fn da_spool_batch_reports_action_panic_as_error() {
    let marker = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut batch = DaSpoolBatch::new();
    batch.push(DaSpoolAction::new(
        "manifest",
        || -> Result<DaSpoolActionOutput, String> {
            panic!("panic during DA spool action");
        },
    ));
    let marker_for_action = Arc::clone(&marker);
    batch.push(DaSpoolAction::new("receipt_log", move || {
        marker_for_action.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(DaSpoolActionOutput::None)
    }));
    let report = batch.execute_sync();
    assert_eq!(marker.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(report.actions().len(), 2);
    assert_eq!(report.actions()[0].kind(), "manifest");
    let error = report.actions()[0]
        .error()
        .expect("panic must be reported as an action error");
    assert!(
        error.contains("panicked") && error.contains("panic during DA spool action"),
        "unexpected panic report: {error}"
    );
    assert_eq!(report.actions()[1].kind(), "receipt_log");
    assert!(report.actions()[1].error().is_none());
    let response = da_spool_rejection_response(&report, ResponseFormat::Json)
        .expect("panic report must fail closed");
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
#[test]
fn da_spool_batch_skips_commit_after_artifact_error() {
    let independent_marker = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let commit_marker = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut batch = DaSpoolBatch::new();
    batch.push(DaSpoolAction::new("manifest", || {
        Err("disk full".to_owned())
    }));
    let independent_marker_for_action = Arc::clone(&independent_marker);
    batch.push(DaSpoolAction::new("taikai_envelope", move || {
        independent_marker_for_action.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(DaSpoolActionOutput::None)
    }));
    let commit_marker_for_action = Arc::clone(&commit_marker);
    batch.push_commit(DaSpoolAction::new("receipt_log", move || {
        commit_marker_for_action.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(DaSpoolActionOutput::None)
    }));

    let report = batch.execute_sync();

    assert_eq!(
        independent_marker.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "independent artifact actions should still complete"
    );
    assert_eq!(
        commit_marker.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "a failed artifact must prevent durable receipt publication"
    );
    assert_eq!(report.actions().len(), 2);
    assert_eq!(report.actions()[0].kind(), "manifest");
    assert_eq!(report.actions()[1].kind(), "taikai_envelope");
}
#[test]
fn da_spool_batch_runs_commit_after_artifacts_succeed() {
    let order = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut batch = DaSpoolBatch::new();
    let order_for_artifact = Arc::clone(&order);
    batch.push(DaSpoolAction::new("taikai_envelope", move || {
        order_for_artifact
            .lock()
            .expect("order lock")
            .push("artifact");
        Ok(DaSpoolActionOutput::None)
    }));
    let order_for_commit = Arc::clone(&order);
    batch.push_commit(DaSpoolAction::new("receipt_log", move || {
        order_for_commit.lock().expect("order lock").push("commit");
        Ok(DaSpoolActionOutput::None)
    }));

    let report = batch.execute_sync();

    assert_eq!(
        *order.lock().expect("order lock"),
        vec!["artifact", "commit"]
    );
    assert_eq!(report.actions().len(), 2);
    assert!(
        report
            .actions()
            .iter()
            .all(|action| action.error().is_none())
    );
}
#[tokio::test]
async fn da_spooler_reports_action_panic_before_ack() {
    let spooler = DaSpooler::spawn(
        NonZeroUsize::new(4).expect("non-zero queue"),
        NonZeroUsize::new(2).expect("non-zero batch"),
        crate::routing::MaybeTelemetry::disabled(),
    );
    let mut batch = DaSpoolBatch::new();
    batch.push(DaSpoolAction::new(
        "pdp_commitment",
        || -> Result<DaSpoolActionOutput, String> {
            std::panic::panic_any(1234_u64);
        },
    ));
    let report = spooler.submit(batch).await;
    assert_eq!(report.actions().len(), 1);
    assert_eq!(report.actions()[0].kind(), "pdp_commitment");
    let error = report.actions()[0]
        .error()
        .expect("panic must be reported before acknowledgement");
    assert!(
        error.contains("panicked") && error.contains("non-string panic payload"),
        "unexpected panic report: {error}"
    );
    let response = da_spool_rejection_response(&report, ResponseFormat::Json)
        .expect("panic report must fail closed");
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
#[test]
fn load_manifest_from_spool_locates_ticket() {
    let dir = tempdir().expect("dir");
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let ticket = context.artifacts.storage_ticket;
    let path = spool_artifact_path_for_key(
        dir.path(),
        "manifest-",
        context.request.lane_id,
        context.request.epoch,
        context.request.sequence,
        &ticket,
        *context.artifacts.fingerprint.as_bytes(),
    );
    fs::write(&path, &context.artifacts.encoded).expect("manifest file");
    let bytes = persistence::load_manifest_from_spool(dir.path(), &ticket).expect("manifest bytes");
    assert_eq!(bytes, context.artifacts.encoded);
    let missing = StorageTicketId::new([0x55; 32]);
    let err =
        persistence::load_manifest_from_spool(dir.path(), &missing).expect_err("missing ticket");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}
#[test]
fn load_pdp_commitment_from_spool_locates_ticket() {
    let dir = tempdir().expect("dir");
    let ticket = StorageTicketId::new([0x99; 32]);
    let path = spool_artifact_path(dir.path(), "pdp-commitment-", &ticket, 2, [0x55; 32]);
    let commitment = sample_pdp_commitment_for_tests();
    let bytes = encode_pdp_commitment_bytes(&commitment).expect("encode commitment");
    fs::write(&path, &bytes).expect("commitment file");
    let loaded =
        persistence::load_pdp_commitment_from_spool(dir.path(), &ticket).expect("commitment");
    assert_eq!(loaded, bytes);
    let missing = StorageTicketId::new([0x55; 32]);
    let err = persistence::load_pdp_commitment_from_spool(dir.path(), &missing)
        .expect_err("missing commitment");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}
#[test]
fn load_manifest_from_spool_ignores_unrelated_flat_artifacts() {
    let dir = tempdir().expect("dir");
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let ticket = context.artifacts.storage_ticket;
    let path = spool_artifact_path_for_key(
        dir.path(),
        "manifest-",
        context.request.lane_id,
        context.request.epoch,
        context.request.sequence,
        &ticket,
        *context.artifacts.fingerprint.as_bytes(),
    );
    fs::write(&path, &context.artifacts.encoded).expect("manifest file");
    fs::create_dir(dir.path().join("manifest-malformed.norito"))
        .expect("create unrelated malformed flat artifact");
    let loaded = persistence::load_manifest_from_spool(dir.path(), &ticket)
        .expect("ticket-indexed manifest");
    assert_eq!(loaded, context.artifacts.encoded);
}
#[test]
fn load_manifest_from_spool_rejects_manifest_shaped_directory() {
    let dir = tempdir().expect("dir");
    let ticket = StorageTicketId::new([0x77; 32]);
    let path = spool_artifact_path(dir.path(), "manifest-", &ticket, 2, [0x44; 32]);
    fs::create_dir(path).expect("create manifest-shaped directory");
    let err = persistence::load_manifest_from_spool(dir.path(), &ticket)
        .expect_err("manifest-shaped directory must fail closed");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("is not a regular file"),
        "unexpected error: {err}"
    );
}
#[test]
fn load_manifest_from_spool_rejects_body_ticket_mismatch() {
    let dir = tempdir().expect("dir");
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let ticket = context.artifacts.storage_ticket;
    let path = spool_artifact_path_for_key(
        dir.path(),
        "manifest-",
        context.request.lane_id,
        context.request.epoch,
        context.request.sequence,
        &ticket,
        *context.artifacts.fingerprint.as_bytes(),
    );
    let mut manifest = context.artifacts.manifest.clone();
    manifest.storage_ticket = StorageTicketId::new([0x99; 32]);
    let bytes = to_bytes(&manifest).expect("encode mismatched manifest");
    fs::write(&path, bytes).expect("manifest file");
    let err = persistence::load_manifest_from_spool(dir.path(), &ticket)
        .expect_err("body ticket mismatch must fail");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}
#[test]
fn load_manifest_from_spool_rejects_fingerprint_mismatch() {
    let dir = tempdir().expect("dir");
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let ticket = context.artifacts.storage_ticket;
    let path = spool_artifact_path_for_key(
        dir.path(),
        "manifest-",
        context.request.lane_id,
        context.request.epoch,
        context.request.sequence,
        &ticket,
        *context.artifacts.fingerprint.as_bytes(),
    );
    let mut manifest = context.artifacts.manifest.clone();
    manifest.blob_hash = BlobDigest::new([0xA5; 32]);
    fs::write(
        &path,
        to_bytes(&manifest).expect("encode tampered manifest"),
    )
    .expect("manifest file");
    let err = persistence::load_manifest_from_spool(dir.path(), &ticket)
        .expect_err("canonical manifest fingerprint mismatch must fail");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}
#[cfg(unix)]
#[test]
fn load_manifest_from_spool_rejects_ticket_shard_symlink() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().expect("dir");
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let ticket = context.artifacts.storage_ticket;
    let ticket_hex = hex::encode(ticket.as_bytes());
    let artifacts_dir = dir.path().join("artifacts");
    fs::create_dir(&artifacts_dir).expect("create artifact index");
    let external_shard = dir.path().join("external-shard");
    let external_ticket = external_shard.join(&ticket_hex);
    fs::create_dir_all(&external_ticket).expect("create external ticket directory");
    fs::write(
        external_ticket.join("manifest.norito"),
        &context.artifacts.encoded,
    )
    .expect("write external manifest");
    symlink(&external_shard, artifacts_dir.join(&ticket_hex[..2]))
        .expect("create ticket shard symlink");
    let err = persistence::load_manifest_from_spool(dir.path(), &ticket)
        .expect_err("ticket shard symlink must fail closed");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("DA spool path"),
        "unexpected ticket shard error: {err}"
    );
}
#[test]
fn load_pdp_commitment_from_spool_ignores_unrelated_flat_artifacts() {
    let dir = tempdir().expect("dir");
    let ticket = StorageTicketId::new([0x99; 32]);
    let path = spool_artifact_path(dir.path(), "pdp-commitment-", &ticket, 2, [0x55; 32]);
    let commitment = sample_pdp_commitment_for_tests();
    let bytes = encode_pdp_commitment_bytes(&commitment).expect("encode commitment");
    fs::write(path, &bytes).expect("commitment");
    fs::create_dir(dir.path().join("pdp-commitment-malformed.norito"))
        .expect("create unrelated malformed flat artifact");
    let loaded =
        persistence::load_pdp_commitment_from_spool(dir.path(), &ticket).expect("commitment");
    assert_eq!(loaded, bytes);
}
#[test]
fn load_pdp_commitment_from_spool_rejects_commitment_shaped_directory() {
    let dir = tempdir().expect("dir");
    let ticket = StorageTicketId::new([0x99; 32]);
    let path = spool_artifact_path(dir.path(), "pdp-commitment-", &ticket, 2, [0x55; 32]);
    fs::create_dir(path).expect("create PDP-shaped directory");
    let err = persistence::load_pdp_commitment_from_spool(dir.path(), &ticket)
        .expect_err("PDP-shaped directory must fail closed");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("is not a regular file"),
        "unexpected error: {err}"
    );
}
#[cfg(unix)]
#[test]
fn load_pdp_commitment_from_spool_rejects_spool_dir_symlink() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().expect("dir");
    let target = dir.path().join("pdp-spool-target");
    fs::create_dir(&target).expect("create target directory");
    let spool = dir.path().join("pdp-spool-link");
    symlink(&target, &spool).expect("create PDP spool symlink");
    let ticket = StorageTicketId::new([0x99; 32]);
    let err = persistence::load_pdp_commitment_from_spool(&spool, &ticket)
        .expect_err("symlinked PDP spool root must reject");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("DA spool path"),
        "unexpected PDP load error: {err}"
    );
    assert!(
        fs::symlink_metadata(&spool)
            .expect("inspect spool symlink")
            .file_type()
            .is_symlink(),
        "failed load should leave spool symlink visible"
    );
    assert!(
        target.exists(),
        "spool symlink target should not be removed"
    );
}
#[test]
fn load_pdp_commitment_from_spool_rejects_invalid_body() {
    let dir = tempdir().expect("dir");
    let ticket = StorageTicketId::new([0x99; 32]);
    let path = spool_artifact_path(dir.path(), "pdp-commitment-", &ticket, 2, [0x55; 32]);
    let mut commitment = sample_pdp_commitment_for_tests();
    commitment.manifest_digest = [0; 32];
    fs::write(
        &path,
        encode_pdp_commitment_bytes(&commitment).expect("encode commitment"),
    )
    .expect("commitment file");
    let err = persistence::load_pdp_commitment_from_spool(dir.path(), &ticket)
        .expect_err("invalid PDP body must fail");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}
#[test]
fn pdp_commitment_header_value_matches_base64_payload() {
    let commitment = sample_pdp_commitment_for_tests();
    let bytes = encode_pdp_commitment_bytes(&commitment).expect("encode commitment");
    let header_value = pdp_commitment_header_value(&bytes).expect("header value");
    let expected = BASE64.encode(bytes);
    assert_eq!(header_value.to_str().expect("utf8 header"), expected);
}
#[test]
fn manifest_response_pdp_header_is_optional_when_missing() {
    let dir = tempdir().expect("dir");
    let (_context, manifest_artifact, manifest_hash) = write_sample_manifest_artifact(dir.path());
    let response =
        utils::respond_value_with_format(Value::Object(Default::default()), ResponseFormat::Json);
    let response = attach_pdp_commitment_header_from_spool(
        dir.path(),
        &manifest_artifact,
        &manifest_hash,
        response,
        ResponseFormat::Json,
    )
    .expect("missing PDP commitment should remain optional");
    assert!(
        !response
            .headers()
            .contains_key(HeaderName::from_static(HEADER_SORA_PDP_COMMITMENT)),
        "missing PDP commitment must not attach a header"
    );
}
#[test]
fn manifest_response_attaches_pdp_commitment_header() {
    let dir = tempdir().expect("dir");
    let (context, manifest_artifact, manifest_hash) = write_sample_manifest_artifact(dir.path());
    let ticket = context.artifacts.storage_ticket;
    let mut commitment = sample_pdp_commitment_for_tests();
    commitment.manifest_digest = *manifest_hash.as_bytes();
    let bytes = encode_pdp_commitment_bytes(&commitment).expect("encode commitment");
    let path = spool_artifact_path_for_key(
        dir.path(),
        "pdp-commitment-",
        context.request.lane_id,
        context.request.epoch,
        context.request.sequence,
        &ticket,
        *context.artifacts.fingerprint.as_bytes(),
    );
    fs::write(&path, &bytes).expect("commitment file");
    let response =
        utils::respond_value_with_format(Value::Object(Default::default()), ResponseFormat::Json);
    let response = attach_pdp_commitment_header_from_spool(
        dir.path(),
        &manifest_artifact,
        &manifest_hash,
        response,
        ResponseFormat::Json,
    )
    .expect("valid PDP commitment should attach");
    let header = response
        .headers()
        .get(HeaderName::from_static(HEADER_SORA_PDP_COMMITMENT))
        .expect("PDP commitment header");
    assert_eq!(header.to_str().expect("header utf8"), BASE64.encode(bytes));
}
#[test]
fn manifest_response_rejects_corrupt_pdp_commitment_sidecar() {
    let dir = tempdir().expect("dir");
    let (context, manifest_artifact, manifest_hash) = write_sample_manifest_artifact(dir.path());
    let ticket = context.artifacts.storage_ticket;
    let path = spool_artifact_path_for_key(
        dir.path(),
        "pdp-commitment-",
        context.request.lane_id,
        context.request.epoch,
        context.request.sequence,
        &ticket,
        *context.artifacts.fingerprint.as_bytes(),
    );
    fs::write(&path, b"not a PDP commitment").expect("commitment file");
    let response =
        utils::respond_value_with_format(Value::Object(Default::default()), ResponseFormat::Json);
    let err = attach_pdp_commitment_header_from_spool(
        dir.path(),
        &manifest_artifact,
        &manifest_hash,
        response,
        ResponseFormat::Json,
    )
    .expect_err("corrupt PDP commitment should fail manifest response");
    let response = axum::response::IntoResponse::into_response(err);
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
#[test]
fn manifest_response_rejects_pdp_commitment_manifest_digest_mismatch() {
    let dir = tempdir().expect("dir");
    let (context, manifest_artifact, manifest_hash) = write_sample_manifest_artifact(dir.path());
    let ticket = context.artifacts.storage_ticket;
    let mut commitment = sample_pdp_commitment_for_tests();
    commitment.manifest_digest = *manifest_hash.as_bytes();
    commitment.manifest_digest[0] ^= 0xFF;
    let bytes = encode_pdp_commitment_bytes(&commitment).expect("encode commitment");
    let path = spool_artifact_path_for_key(
        dir.path(),
        "pdp-commitment-",
        context.request.lane_id,
        context.request.epoch,
        context.request.sequence,
        &ticket,
        *context.artifacts.fingerprint.as_bytes(),
    );
    fs::write(&path, &bytes).expect("digest-mismatched PDP commitment");
    let response =
        utils::respond_value_with_format(Value::Object(Default::default()), ResponseFormat::Json);
    let err = attach_pdp_commitment_header_from_spool(
        dir.path(),
        &manifest_artifact,
        &manifest_hash,
        response,
        ResponseFormat::Json,
    )
    .expect_err("wrong-digest PDP commitment should fail manifest response");
    let response = axum::response::IntoResponse::into_response(err);
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
fn taikai_metadata() -> ExtraMetadata {
    ExtraMetadata {
        items: vec![
            MetadataEntry::new(
                taikai::META_TAIKAI_EVENT_ID,
                b"global-keynote".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_STREAM_ID,
                b"stage-a".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_RENDITION_ID,
                b"1080p".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_TRACK_KIND,
                b"video".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_TRACK_CODEC,
                b"av1-main".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_TRACK_BITRATE,
                b"8000".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_TRACK_RESOLUTION,
                b"1920x1080".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_SEGMENT_SEQUENCE,
                b"42".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_SEGMENT_START,
                b"3600000".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_SEGMENT_DURATION,
                b"2000000".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_WALLCLOCK_MS,
                b"1702560000000".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_INGEST_LATENCY_MS,
                b"120".to_vec(),
                MetadataVisibility::Public,
            ),
            MetadataEntry::new(
                taikai::META_TAIKAI_INGEST_NODE_ID,
                b"ingest-node-1".to_vec(),
                MetadataVisibility::Public,
            ),
        ],
    }
}
#[test]
fn taikai_availability_defaults_without_trm() {
    let metadata = taikai_metadata();
    let availability = taikai::taikai_availability_from_metadata(&metadata, None).expect("derive");
    assert!(availability.is_none());
}
#[test]
fn taikai_availability_uses_trm_payload() {
    let metadata = taikai_metadata();
    let mut manifest = sample_trm_manifest();
    manifest.renditions[0].availability_class = TaikaiAvailabilityClass::Warm;
    let bytes = to_bytes(&manifest).expect("encode trm");
    let availability = taikai::taikai_availability_from_metadata(&metadata, Some(&bytes))
        .expect("derive")
        .expect("class");
    assert_eq!(availability, TaikaiAvailabilityClass::Warm);
}

#[test]
fn taikai_availability_rejects_duplicate_consumed_metadata() {
    let mut metadata = taikai_metadata();
    metadata.items.push(MetadataEntry::new(
        taikai::META_TAIKAI_EVENT_ID,
        b"shadow-event".to_vec(),
        MetadataVisibility::Public,
    ));
    let bytes = to_bytes(&sample_trm_manifest()).expect("encode trm");
    let err = taikai::taikai_availability_from_metadata(&metadata, Some(&bytes))
        .expect_err("duplicate Taikai metadata must reject before routing selection");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("metadata entry must appear at most once"),
        "unexpected duplicate-metadata error: {}",
        err.1
    );
}

#[test]
fn taikai_availability_rejects_rendition_window_that_misses_segment() {
    let metadata = taikai_metadata();
    let mut manifest = sample_trm_manifest();
    manifest.renditions[0].ssm_range = TaikaiSegmentWindow::new(50, 64);
    let bytes = to_bytes(&manifest).expect("encode trm");
    let err = taikai::taikai_availability_from_metadata(&metadata, Some(&bytes))
        .expect_err("out-of-window rendition must not select a retention policy");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("rendition `1080p` signing window"),
        "unexpected error: {}",
        err.1
    );
}
#[test]
fn taikai_ingest_tags_include_availability_and_cache_hint() {
    let mut metadata = taikai_metadata();
    metadata.items.extend([
        MetadataEntry::new(
            taikai::META_TAIKAI_AVAILABILITY_CLASS,
            b"hot".to_vec(),
            MetadataVisibility::Public,
        ),
        MetadataEntry::new(
            taikai::META_TAIKAI_AVAILABILITY_CLASS,
            b"warm".to_vec(),
            MetadataVisibility::Public,
        ),
    ]);
    let retention = RetentionPolicy {
        hot_retention_secs: 3_600,
        cold_retention_secs: 12 * 60 * 60,
        required_replicas: 4,
        storage_class: StorageClass::Warm,
        governance_tag: GovernanceTag::new("da.taikai.test"),
    };
    let payload_digest = BlobDigest::from_hash(blake3_hash(b"taikai payload bytes"));
    taikai::apply_taikai_ingest_tags(
        &mut metadata,
        Some(TaikaiAvailabilityClass::Cold),
        &retention,
        payload_digest,
        1024,
    )
    .expect("tagging succeeds");
    fn value_for(metadata: &ExtraMetadata, key: &str) -> String {
        let entry = metadata
            .items
            .iter()
            .find(|entry| entry.key == key)
            .unwrap_or_else(|| panic!("missing metadata entry `{key}`"));
        String::from_utf8(entry.value.clone()).expect("utf8 value")
    }
    assert_eq!(
        value_for(&metadata, taikai::META_TAIKAI_AVAILABILITY_CLASS),
        "cold"
    );
    assert_eq!(
        metadata
            .items
            .iter()
            .filter(|entry| entry.key == taikai::META_TAIKAI_AVAILABILITY_CLASS)
            .count(),
        1,
        "server-derived tags must replace every submitted copy"
    );
    assert_eq!(value_for(&metadata, taikai::META_DA_PROOF_TIER), "warm");
    assert_eq!(
        value_for(&metadata, taikai::META_TAIKAI_REPLICATION_REPLICAS),
        "4"
    );
    assert_eq!(
        value_for(&metadata, taikai::META_TAIKAI_REPLICATION_STORAGE),
        "warm"
    );
    assert_eq!(
        value_for(&metadata, taikai::META_TAIKAI_REPLICATION_HOT_SECS),
        "3600"
    );
    assert_eq!(
        value_for(&metadata, taikai::META_TAIKAI_REPLICATION_COLD_SECS),
        "43200"
    );
    assert_eq!(
        value_for(&metadata, taikai::META_DA_PDP_SAMPLE_WINDOW),
        "32"
    );
    assert_eq!(
        value_for(&metadata, taikai::META_DA_POTR_SAMPLE_WINDOW),
        "32"
    );
    let cache_hint_entry = metadata
        .items
        .iter()
        .find(|entry| entry.key == taikai::META_TAIKAI_CACHE_HINT)
        .expect("cache hint entry");
    let cache_hint: Value = json::from_slice(&cache_hint_entry.value).expect("cache hint json");
    let hint = cache_hint.as_object().expect("cache hint object");
    assert_eq!(
        hint.get("event").and_then(Value::as_str).expect("event id"),
        "global-keynote"
    );
    assert_eq!(
        hint.get("stream")
            .and_then(Value::as_str)
            .expect("stream id"),
        "stage-a"
    );
    assert_eq!(
        hint.get("rendition")
            .and_then(Value::as_str)
            .expect("rendition id"),
        "1080p"
    );
    assert_eq!(
        hint.get("sequence")
            .and_then(Value::as_u64)
            .expect("sequence"),
        42
    );
    assert_eq!(
        hint.get("payload_len")
            .and_then(Value::as_u64)
            .expect("payload_len"),
        1024
    );
    assert_eq!(
        hint.get("payload_blake3_hex")
            .and_then(Value::as_str)
            .expect("digest"),
        hex::encode(payload_digest.as_ref())
    );
}
fn taikai_manifest_fixture() -> (DaIngestRequest, ManifestArtifacts) {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let payload_digest = BlobDigest::from_hash(blake3_hash(canonical.as_slice()));
    let mut metadata = request.metadata.clone();
    taikai::apply_taikai_ingest_tags(
        &mut metadata,
        Some(TaikaiAvailabilityClass::Hot),
        &request.retention_policy,
        payload_digest,
        request.total_size,
    )
    .expect("tagging succeeds");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        0,
        &rent_policy,
    )
    .expect("manifest");
    (request, manifest)
}
#[test]
fn verify_manifest_rejects_cache_hint_mismatch() {
    let (request, manifest) = taikai_manifest_fixture();
    let mut tampered = manifest.manifest.clone();
    // Replace the cache hint digest with a mismatched value.
    let hint_entry = tampered
        .metadata
        .items
        .iter_mut()
        .find(|entry| entry.key == taikai::META_TAIKAI_CACHE_HINT)
        .expect("cache hint entry");
    let mut hint: Value = json::from_slice(&hint_entry.value).expect("decode cache hint");
    if let Value::Object(map) = &mut hint {
        map.insert(
            "payload_blake3_hex".into(),
            Value::from(hex::encode([0xCD; 32])),
        );
    } else {
        panic!("cache hint must be a JSON object");
    }
    hint_entry.value = json::to_vec(&hint).expect("encode cache hint");
    let err = verify_manifest_against_request(
        &request,
        &tampered,
        &request.retention_policy,
        &tampered.metadata,
        &tampered.chunks,
        manifest.blob_hash,
        manifest.chunk_root,
        &manifest.manifest.rent_quote,
    )
    .expect_err("cache hint digest mismatch must be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
#[test]
fn verify_manifest_rejects_missing_proof_tier() {
    let (request, manifest) = taikai_manifest_fixture();
    let mut tampered = manifest.manifest.clone();
    tampered
        .metadata
        .items
        .retain(|entry| entry.key != taikai::META_DA_PROOF_TIER);
    let err = verify_manifest_against_request(
        &request,
        &tampered,
        &request.retention_policy,
        &tampered.metadata,
        &tampered.chunks,
        manifest.blob_hash,
        manifest.chunk_root,
        &manifest.manifest.rent_quote,
    )
    .expect_err("missing proof tier must be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
fn sample_pdp_commitment_for_tests() -> PdpCommitmentV1 {
    let tree = PdpMerkleTreeV1::from_bytes(&[0x44; 8_193]).expect("fixture PDP tree");
    PdpCommitmentV1::from_tree(
        &tree,
        [0x11; 32],
        ChunkingProfileV1 {
            profile_id: ProfileId(0xAB),
            namespace: "inline".to_owned(),
            name: "inline".to_owned(),
            semver: "1.0.0".to_owned(),
            min_size: 64 * 1024,
            target_size: 64 * 1024,
            max_size: 64 * 1024,
            break_mask: 1,
            multihash_code: BLAKE3_256_MULTIHASH_CODE,
            aliases: vec!["inline.inline@1.0.0".to_owned()],
        },
        32,
        1_707_300_000,
    )
    .expect("fixture PDP commitment")
}
fn encode_alias_proof_bytes(
    alias_namespace: &str,
    alias_name: &str,
    manifest_cid: &[u8],
    bound_epoch: u64,
    expiry_epoch: u64,
    generated_at_unix: u64,
    expires_at_hint: u64,
    council_seeds: &[[u8; 32]],
) -> Vec<u8> {
    let binding = AliasBindingV1 {
        alias: format!("{alias_namespace}/{alias_name}"),
        manifest_cid: manifest_cid.to_vec(),
        bound_at: bound_epoch,
        expiry_epoch,
    };
    let mut bundle = AliasProofBundleV1 {
        binding,
        registry_root: [0u8; 32],
        registry_height: 1,
        generated_at_unix,
        expires_at_unix: expires_at_hint.max(generated_at_unix + 1),
        merkle_path: Vec::new(),
        council_signatures: Vec::new(),
    };
    bundle.registry_root =
        alias_merkle_root(&bundle.binding, &bundle.merkle_path).expect("compute alias proof root");
    let digest = alias_proof_signature_digest(&bundle);
    bundle.council_signatures = council_seeds
        .iter()
        .map(|seed| {
            let keypair = alias_council_keypair(seed);
            let signature = checked_signature(keypair.private_key(), digest.as_ref());
            let (_, signer_bytes) = keypair
                .public_key()
                .try_to_bytes()
                .expect("fixture public key must be valid");
            CouncilSignature {
                signer: signer_bytes.try_into().expect("ed25519 pk length"),
                signature: signature.payload().to_vec(),
            }
        })
        .collect();
    bundle
        .council_signatures
        .sort_by_key(|signature| signature.signer);
    to_bytes(&bundle).expect("encode alias proof")
}
fn alias_council_keypair(seed: &[u8; 32]) -> KeyPair {
    let private = PrivateKey::from_bytes(Algorithm::Ed25519, seed).expect("seeded council key");
    KeyPair::from_private_key(private).expect("derive council keypair")
}
fn alias_council_policy(
    council_seeds: &[[u8; 32]],
    threshold: usize,
) -> ProviderAdmissionCouncilPolicy {
    let trusted_signers = council_seeds.iter().map(|seed| {
        let keypair = alias_council_keypair(seed);
        let (_, signer_bytes) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must be valid");
        signer_bytes.try_into().expect("ed25519 pk length")
    });
    ProviderAdmissionCouncilPolicy::new(trusted_signers, threshold)
        .expect("valid fixture council policy")
}
fn build_ssm_bytes(
    manifest_hash: BlobDigest,
    car_digest: BlobDigest,
    envelope_hash: BlobDigest,
    segment_sequence: u64,
    generated_at_unix: u64,
    expires_at_hint: u64,
) -> Vec<u8> {
    build_ssm_bytes_with_alias_council(
        manifest_hash,
        manifest_hash,
        car_digest,
        envelope_hash,
        segment_sequence,
        generated_at_unix,
        expires_at_hint,
        Algorithm::Ed25519,
        &[[0x33; 32]],
    )
}
fn build_ssm_bytes_with_publisher_algorithm(
    manifest_hash: BlobDigest,
    car_digest: BlobDigest,
    envelope_hash: BlobDigest,
    segment_sequence: u64,
    generated_at_unix: u64,
    expires_at_hint: u64,
    publisher_algorithm: Algorithm,
) -> Vec<u8> {
    build_ssm_bytes_with_alias_council(
        manifest_hash,
        manifest_hash,
        car_digest,
        envelope_hash,
        segment_sequence,
        generated_at_unix,
        expires_at_hint,
        publisher_algorithm,
        &[[0x33; 32]],
    )
}
#[allow(clippy::too_many_arguments)]
fn build_ssm_bytes_with_alias_council(
    manifest_hash: BlobDigest,
    alias_manifest_hash: BlobDigest,
    car_digest: BlobDigest,
    envelope_hash: BlobDigest,
    segment_sequence: u64,
    generated_at_unix: u64,
    expires_at_hint: u64,
    publisher_algorithm: Algorithm,
    council_seeds: &[[u8; 32]],
) -> Vec<u8> {
    build_ssm_bytes_with_alias_council_and_body_mutation(
        manifest_hash,
        alias_manifest_hash,
        car_digest,
        envelope_hash,
        segment_sequence,
        generated_at_unix,
        expires_at_hint,
        publisher_algorithm,
        council_seeds,
        |_| {},
    )
}

#[allow(clippy::too_many_arguments)]
fn build_ssm_bytes_with_alias_council_and_body_mutation<F>(
    manifest_hash: BlobDigest,
    alias_manifest_hash: BlobDigest,
    car_digest: BlobDigest,
    envelope_hash: BlobDigest,
    segment_sequence: u64,
    generated_at_unix: u64,
    expires_at_hint: u64,
    publisher_algorithm: Algorithm,
    council_seeds: &[[u8; 32]],
    mutate_body: F,
) -> Vec<u8>
where
    F: FnOnce(&mut TaikaiSegmentSigningBodyV1),
{
    let manifest_cid = canonical_manifest_root_cid(*alias_manifest_hash.as_bytes());
    let alias_proof = encode_alias_proof_bytes(
        "sora",
        "docs",
        &manifest_cid,
        1,
        32,
        generated_at_unix,
        expires_at_hint,
        council_seeds,
    );
    let alias_binding = ManifestAliasBinding {
        name: "docs".into(),
        namespace: "sora".into(),
        proof: alias_proof,
    };
    let publisher = checked_random_keypair_with_algorithm(publisher_algorithm);
    let publisher_account = AccountId::new(publisher.public_key().clone());
    let mut body = TaikaiSegmentSigningBodyV1::new(
        TaikaiSegmentSigningBodyV1::VERSION,
        envelope_hash,
        manifest_hash,
        car_digest,
        segment_sequence,
        publisher_account,
        publisher.public_key().clone(),
        generated_at_unix * 1_000,
        alias_binding,
        ExtraMetadata::default(),
    );
    mutate_body(&mut body);
    let signature = checked_taikai_segment_signature(publisher.private_key(), &body);
    let manifest = TaikaiSegmentSigningManifestV1::new(body, signature);
    to_bytes(&manifest).expect("encode signing manifest")
}
fn sample_trm_manifest() -> TaikaiRoutingManifestV1 {
    let event_id = TaikaiEventId::new(Name::from_str("global-keynote").unwrap());
    let stream_id = TaikaiStreamId::new(Name::from_str("stage-a").unwrap());
    let rendition_id = TaikaiRenditionId::new(Name::from_str("1080p").unwrap());
    let route = TaikaiRenditionRouteV1 {
        rendition_id: rendition_id.clone(),
        latest_manifest_hash: BlobDigest::from_hash(blake3_hash(b"manifest")),
        latest_car: TaikaiCarPointer::new(
            "zbafyqra",
            BlobDigest::from_hash(blake3_hash(b"car")),
            131_072,
        ),
        availability_class: TaikaiAvailabilityClass::Hot,
        replication_targets: vec![ProviderId::new([0x22; 32])],
        soranet_circuit: GuardDirectoryId::new("soranet/demo"),
        ssm_range: TaikaiSegmentWindow::new(40, 64),
    };
    TaikaiRoutingManifestV1 {
        version: TaikaiRoutingManifestV1::VERSION,
        event_id,
        stream_id,
        segment_window: TaikaiSegmentWindow::new(0, 64),
        renditions: vec![route],
        alias_binding: TaikaiAliasBinding {
            name: "docs".to_owned(),
            namespace: "sora".to_owned(),
            proof: vec![0xAB, 0xCD],
        },
        guard_policy: TaikaiGuardPolicy::new(
            GuardDirectoryId::new("soranet/demo"),
            1,
            3,
            vec!["lane-a".to_owned()],
        ),
        metadata: ExtraMetadata::default(),
    }
}
fn sample_trm_bytes() -> Vec<u8> {
    to_bytes(&sample_trm_manifest()).expect("encode trm")
}
fn sample_trm_manifest_for_envelope(
    envelope: &taikai_ingest::EnvelopeArtifacts,
) -> TaikaiRoutingManifestV1 {
    let mut manifest = sample_trm_manifest();
    manifest.renditions[0].latest_manifest_hash = envelope.ingest.manifest_hash;
    manifest.renditions[0].latest_car = envelope.ingest.car.clone();
    manifest
}
fn taikai_envelope_fixture() -> taikai_ingest::EnvelopeArtifacts {
    let (_, envelope) = taikai_ssm_validation_fixture();
    envelope
}
fn sample_request() -> DaIngestRequest {
    // Golden fixture tests must not depend on OS randomness.
    let keypair = checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519);
    let payload = b"example".to_vec();
    DaIngestRequestIntentV1 {
        network_id: crate::signed_query_test_network_id(),
        owner: ALICE_ID.clone(),
        client_blob_id: BlobDigest::from_hash(blake3::hash(b"blob-id")),
        lane_id: LaneId::new(1),
        epoch: 5,
        sequence: 7,
        blob_class: BlobClass::TaikaiSegment,
        codec: BlobCodec::new("cmaf"),
        erasure_profile: ErasureProfile {
            data_shards: 8,
            parity_shards: 4,
            row_parity_stripes: 0,
            chunk_alignment: 2,
            fec_scheme: FecScheme::Rs12_10,
        },
        retention_policy: RetentionPolicy {
            hot_retention_secs: 3600,
            cold_retention_secs: 10 * 3600,
            required_replicas: 3,
            storage_class: StorageClass::Hot,
            governance_tag: GovernanceTag::new("baseline"),
        },
        chunk_size: 1 << 10,
        total_size: payload.len() as u64,
        payload_hash: BlobDigest::from_hash(blake3::hash(&payload)),
        compression: Compression::Identity,
        norito_manifest: None,
        payload,
        metadata: ExtraMetadata {
            items: vec![MetadataEntry::new(
                "content-type",
                b"video/cmaf".to_vec(),
                MetadataVisibility::Public,
            )],
        },
    }
    .try_sign(&keypair)
    .expect("sign canonical DA request fixture")
}

fn resign_sample_request(request: &mut DaIngestRequest) {
    let keypair = checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519);
    request.signatures.clear();
    request.pin_scope_signatures.clear();
    let signature = Signature::try_new(keypair.private_key(), &request.signing_digest())
        .expect("re-sign canonical DA request fixture");
    request.signatures.push(DaIngestSignatureV1 {
        signer: keypair.public_key().clone(),
        signature,
    });
}

fn signed_pin_intent(
    request: &DaIngestRequest,
    storage_ticket: StorageTicketId,
    manifest_hash: ManifestDigest,
    alias: Option<String>,
) -> DaPinIntent {
    let keypair = checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519);
    let authorization = request.authorization();
    let scope = DaPinScopeV1::new(&authorization, storage_ticket, manifest_hash, alias);
    let scope_authorization = DaPinScopeAuthorizationV1::try_sign(scope, &keypair)
        .expect("sign canonical DA pin-scope fixture");
    DaPinIntent::new(authorization, scope_authorization)
}

fn signed_pin_intent_for_manifest(
    request: &DaIngestRequest,
    manifest: &ManifestArtifacts,
) -> DaPinIntent {
    let keypair = checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519);
    let scope = build_da_pin_scope(request, manifest.storage_ticket, manifest.manifest_hash)
        .expect("build canonical DA pin scope");
    let scope_authorization =
        DaPinScopeAuthorizationV1::try_sign(scope, &keypair).expect("sign canonical DA pin scope");
    build_da_pin_intent(request, scope_authorization)
}

fn active_da_admission_incarnation(app: &crate::SharedAppState, lane_id: LaneId) -> Hash {
    let view = app.state.view();
    let proposal_height = u64::try_from(view.height())
        .expect("test state height fits u64")
        .checked_add(1)
        .expect("test proposal height advances");
    view.lane_incarnation_at_height(lane_id, proposal_height)
        .expect("test lane has an active incarnation")
}

fn seed_da_admission_parameter(app: &crate::SharedAppState, parameter: CustomParameter) {
    let next_height = u64::try_from(app.state.view().height())
        .expect("test state height fits u64")
        .checked_add(1)
        .expect("test block height advances");
    let header = BlockHeader::new(
        NonZeroU64::new(next_height).expect("test block height is non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = app.state.block(header);
    let mut transaction = block.transaction();
    transaction
        .world_mut_for_testing()
        .parameters_mut_for_testing()
        .get_mut()
        .set_parameter(Parameter::Custom(parameter));
    transaction.apply();
    block
        .commit()
        .expect("commit governed DA admission test parameter");
}

fn da_admission_policy(
    lane_id: LaneId,
    lane_incarnation: Hash,
    producers: Vec<AccountId>,
    current_epoch: u64,
    grace_epoch: Option<u64>,
) -> DaIngestAdmissionPolicyV1 {
    let policy = DaIngestAdmissionPolicyV1 {
        version: DaIngestAdmissionPolicyV1::VERSION,
        revision: 1,
        expected_previous_policy_hash: None,
        lanes: vec![DaIngestAdmissionLaneV1 {
            lane_id,
            lane_incarnation,
            producers,
            current_epoch,
            grace_epoch,
        }],
    };
    policy
        .validate()
        .expect("DA admission test policy must be canonical");
    policy
}

#[tokio::test]
async fn da_ingest_admission_fails_closed_without_governed_policy() {
    let app = crate::mk_app_state_for_tests();
    let error = admission_snapshot_for_request(&app, &ALICE_ID, LaneId::SINGLE, 5)
        .expect_err("DA ingest without a committed policy must fail closed");

    assert_eq!(error.0, StatusCode::SERVICE_UNAVAILABLE);
    assert!(error.1.contains("governance installs an admission policy"));
}

#[tokio::test]
async fn da_ingest_admission_fails_closed_for_malformed_governed_policy() {
    let app = crate::mk_app_state_for_tests();
    seed_da_admission_parameter(
        &app,
        CustomParameter::new(
            DaIngestAdmissionPolicyV1::parameter_id(),
            Json::new("not-a-da-admission-policy"),
        ),
    );

    let error = admission_snapshot_for_request(&app, &ALICE_ID, LaneId::SINGLE, 5)
        .expect_err("malformed committed DA admission policy must fail closed");
    assert_eq!(error.0, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        error
            .1
            .contains("committed DA ingest admission policy is invalid")
    );
}

#[tokio::test]
async fn da_ingest_admission_rejects_wrong_producer_and_epoch() {
    let app = crate::mk_app_state_for_tests();
    let lane_id = LaneId::SINGLE;
    let incarnation = active_da_admission_incarnation(&app, lane_id);
    let policy = da_admission_policy(lane_id, incarnation, vec![ALICE_ID.clone()], 5, Some(4));
    seed_da_admission_parameter(&app, policy.into_custom_parameter());

    for (owner, epoch, label) in [
        (&*BOB_ID, 5, "unlisted producer"),
        (&*ALICE_ID, 3, "retired epoch"),
        (&*ALICE_ID, 6, "future epoch"),
    ] {
        let error = admission_snapshot_for_request(&app, owner, lane_id, epoch).expect_err(label);
        assert_eq!(error.0, StatusCode::FORBIDDEN, "{label}");
    }
}

#[tokio::test]
async fn da_ingest_admission_rejects_wrong_lane_incarnation() {
    let app = crate::mk_app_state_for_tests();
    let lane_id = LaneId::SINGLE;
    let active_incarnation = active_da_admission_incarnation(&app, lane_id);
    let mut wrong_incarnation = Hash::prehashed([0xE1; Hash::LENGTH]);
    if wrong_incarnation == active_incarnation {
        wrong_incarnation = Hash::prehashed([0xE2; Hash::LENGTH]);
    }
    let policy = da_admission_policy(
        lane_id,
        wrong_incarnation,
        vec![ALICE_ID.clone()],
        5,
        Some(4),
    );
    seed_da_admission_parameter(&app, policy.into_custom_parameter());

    let error = admission_snapshot_for_request(&app, &ALICE_ID, lane_id, 5)
        .expect_err("policy for a retired lane incarnation must be rejected");
    assert_eq!(error.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn da_ingest_admission_accepts_current_and_grace_epochs_for_exact_scope() {
    let app = crate::mk_app_state_for_tests();
    let lane_id = LaneId::SINGLE;
    let incarnation = active_da_admission_incarnation(&app, lane_id);
    let policy = da_admission_policy(lane_id, incarnation, vec![ALICE_ID.clone()], 5, Some(4));
    seed_da_admission_parameter(&app, policy.into_custom_parameter());

    for epoch in [4, 5] {
        admission_snapshot_for_request(&app, &ALICE_ID, lane_id, epoch)
            .unwrap_or_else(|error| panic!("exact admitted epoch {epoch} rejected: {error:?}"));
    }
}

include!("tests/principal_binding_tests.rs");
#[test]
fn compute_da_manifest_artifacts_builds_canonical_pipeline_outputs() {
    let mut request = sample_request();
    request.blob_class = BlobClass::NexusLaneSidecar;
    let keypair = checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519);
    let digest = request.signing_digest();
    request.signatures[0].signature = checked_signature(keypair.private_key(), &digest);
    let replication_policy = DaReplicationPolicy::default();
    let rent_policy = DaRentPolicyV1::default();
    let nexus = nexus_with_scheme(request.lane_id, DaProofScheme::MerkleSha256);
    let computed = compute_da_manifest_artifacts(
        &request,
        &nexus,
        1,
        None,
        None,
        &replication_policy,
        &rent_policy,
        None,
    )
    .expect("canonical DA compute pipeline");
    assert_eq!(computed.proof_scheme, DaProofScheme::MerkleSha256);
    assert_eq!(computed.canonical_payload, request.payload);
    assert_eq!(
        computed.manifest.manifest.retention_policy,
        computed.enforced_retention
    );
    assert_eq!(
        computed.manifest.manifest.total_size,
        computed.canonical_payload.len() as u64
    );
    assert_eq!(
        computed.chunk_store.payload_len(),
        computed.canonical_payload.len() as u64
    );
    assert!(computed.taikai_ssm_payload.is_none());
    assert!(computed.taikai_trm_payload.is_none());
    assert!(computed.queued_at_secs > 0);
}
#[test]
fn compute_da_manifest_artifacts_authenticates_before_lane_lookup() {
    let nexus = nexus_with_scheme(LaneId::new(1), DaProofScheme::MerkleSha256);
    let replication_policy = DaReplicationPolicy::default();
    let rent_policy = DaRentPolicyV1::default();
    let mut invalid_signature_valid_lane = sample_request();
    invalid_signature_valid_lane.sequence += 1;
    let mut invalid_signature_unknown_lane = invalid_signature_valid_lane.clone();
    invalid_signature_unknown_lane.lane_id = LaneId::new(99);
    let compute_error = |request: &DaIngestRequest| {
        compute_da_manifest_artifacts(
            request,
            &nexus,
            1,
            None,
            None,
            &replication_policy,
            &rent_policy,
            None,
        )
        .err()
        .expect("request must be rejected")
    };
    let valid_lane_error = compute_error(&invalid_signature_valid_lane);
    let unknown_lane_error = compute_error(&invalid_signature_unknown_lane);
    assert_eq!(
        unknown_lane_error, valid_lane_error,
        "an invalid signature must not reveal whether its lane is active"
    );
    assert_eq!(unknown_lane_error.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        unknown_lane_error.1,
        "DA ingest request signature is invalid"
    );
    let keypair = checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519);
    invalid_signature_unknown_lane.signatures[0].signature = checked_signature(
        keypair.private_key(),
        &invalid_signature_unknown_lane.signing_digest(),
    );
    let authenticated_error = compute_error(&invalid_signature_unknown_lane);
    assert_eq!(authenticated_error.0, StatusCode::BAD_REQUEST);
    assert!(authenticated_error.1.contains("active lane catalog"));
}
fn lane_catalog_with_lanes(lanes: Vec<ModelLaneConfig>) -> LaneCatalog {
    let max_lane = lanes
        .iter()
        .map(|lane| lane.id.as_u32())
        .max()
        .unwrap_or_default();
    LaneCatalog::new(
        NonZeroU32::new(max_lane.saturating_add(1)).expect("lane count"),
        lanes,
    )
    .expect("lane catalog")
}
fn nexus_with_catalog(lane_catalog: LaneCatalog) -> ConfigNexus {
    let dataspace_catalog = DataSpaceCatalog::new(
        lane_catalog
            .lanes()
            .iter()
            .map(|lane| lane.dataspace_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|id| DataSpaceMetadata {
                id,
                alias: format!("ds-{}", id.as_u64()),
                description: None,
                fault_tolerance: 1,
            })
            .collect(),
    )
    .expect("dataspace catalog");
    ConfigNexus {
        lane_config: ConfigLaneConfig::from_catalog(&lane_catalog),
        lane_catalog,
        dataspace_catalog,
        ..Default::default()
    }
}
fn nexus_with_scheme(lane_id: LaneId, scheme: DaProofScheme) -> ConfigNexus {
    let lane = ModelLaneConfig {
        id: lane_id,
        dataspace_id: DataSpaceId::new(u64::from(lane_id.as_u32())),
        alias: format!("lane-{}", lane_id.as_u32()),
        proof_scheme: scheme,
        ..ModelLaneConfig::default()
    };
    nexus_with_catalog(lane_catalog_with_lanes(vec![lane]))
}
#[test]
fn validate_request_accepts_well_formed_payload() {
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    assert!(validate_request(&request, canonical.as_slice()).is_ok());
}
#[test]
fn validate_request_rejects_non_power_two_chunks() {
    let mut request = sample_request();
    request.chunk_size = 1_500;
    let canonical = normalize_payload(&request).expect("normalize payload");
    let err = match validate_request(&request, canonical.as_slice()) {
        Ok(_) => panic!("expected validation to reject non power-of-two chunk size"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
#[test]
fn validate_request_rejects_unbounded_erasure_work_before_allocation() {
    let canonical = sample_request().payload;
    let mut request = sample_request();
    request.erasure_profile.data_shards = MAX_DATA_SHARDS + 1;
    assert_eq!(
        validate_request(&request, &canonical)
            .expect_err("excess data shards must reject")
            .0,
        StatusCode::BAD_REQUEST
    );
    let mut request = sample_request();
    request.erasure_profile.parity_shards = MAX_PARITY_SHARDS + 1;
    assert_eq!(
        validate_request(&request, &canonical)
            .expect_err("excess parity shards must reject")
            .0,
        StatusCode::BAD_REQUEST
    );
    let mut request = sample_request();
    request.erasure_profile.row_parity_stripes = MAX_ROW_PARITY_STRIPES + 1;
    assert_eq!(
        validate_request(&request, &canonical)
            .expect_err("excess row parity must reject")
            .0,
        StatusCode::BAD_REQUEST
    );
    let mut request = sample_request();
    request.total_size = MAX_CANONICAL_PAYLOAD_BYTES;
    request.chunk_size = MAX_CHUNK_SIZE_BYTES;
    request.erasure_profile.data_shards = 1;
    request.erasure_profile.parity_shards = MAX_PARITY_SHARDS;
    request.erasure_profile.row_parity_stripes = 0;
    let err = validate_request_shape(&request)
        .expect_err("multiplicative parity output must be rejected before allocation");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("generated-parity budget"));
    let mut request = sample_request();
    request.total_size = MAX_CANONICAL_PAYLOAD_BYTES;
    request.chunk_size = MAX_CHUNK_SIZE_BYTES;
    request.erasure_profile.data_shards = 1;
    request.erasure_profile.parity_shards = 3;
    request.erasure_profile.row_parity_stripes = 1;
    let err = validate_request_shape(&request)
        .expect_err("retained row-parity matrix must fit the workspace budget");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("RS16 workspace budget"));
}
#[test]
fn validate_request_rejects_excess_source_chunks_and_row_work() {
    let mut request = sample_request();
    request.total_size =
        u64::try_from(MAX_DATA_CHUNKS + 1).unwrap() * u64::from(request.chunk_size);
    let err = validate_request_shape(&request).expect_err("excess source chunks must be rejected");
    assert_eq!(err.0, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(err.1.contains("source-chunk limit"));
    let mut request = sample_request();
    request.erasure_profile.data_shards = 1;
    request.erasure_profile.row_parity_stripes = 1;
    request.total_size =
        u64::try_from(MAX_ROW_PARITY_SOURCE_STRIPES + 1).unwrap() * u64::from(request.chunk_size);
    let err = validate_request_shape(&request)
        .expect_err("cubic row-parity source count must be bounded");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("source-stripe computation limit"));
}
#[test]
fn validate_request_rejects_terminal_sequence() {
    let mut request = sample_request();
    request.sequence = u64::MAX;
    let err = validate_request_shape(&request)
        .expect_err("a terminal sequence must not poison the replay window");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("monotonic successor"));
}
#[test]
fn normalize_payload_rejects_claimed_decompression_bomb_before_decoding() {
    let mut request = sample_request();
    request.compression = Compression::Gzip;
    request.payload = vec![0x00];
    request.total_size = MAX_CANONICAL_PAYLOAD_BYTES + 1;
    let err = normalize_payload(&request)
        .expect_err("oversized decompressed length must reject before decoding");
    assert_eq!(err.0, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(err.1.contains("64 MiB"));
}
fn fingerprint_for_request(request: &DaIngestRequest) -> ReplayFingerprint {
    let canonical = normalize_payload(request).expect("normalize payload");
    let chunk_store = build_chunk_store(request, canonical.as_slice());
    let rent_policy = DaRentPolicyV1::default();
    resolve_manifest(
        request,
        &chunk_store,
        canonical.as_slice(),
        &request.metadata,
        &request.retention_policy,
        0,
        &rent_policy,
    )
    .expect("manifest")
    .fingerprint
}
#[test]
fn fingerprint_changes_with_client_blob_id() {
    let request = sample_request();
    let mut other = request.clone();
    other.client_blob_id = BlobDigest::from_hash(blake3::hash(b"different"));
    assert_ne!(
        fingerprint_for_request(&request),
        fingerprint_for_request(&other)
    );
}
#[test]
fn fingerprint_ignores_manifest_storage_ticket_and_timestamp() {
    let mut request = sample_request();
    request.blob_class = BlobClass::NexusLaneSidecar;
    let canonical = normalize_payload(&request)
        .expect("normalize payload")
        .into_vec();
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let rent_policy = DaRentPolicyV1::default();
    let baseline_manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &request.metadata,
        &request.retention_policy,
        7,
        &rent_policy,
    )
    .expect("manifest");
    request.norito_manifest =
        Some(to_bytes(&baseline_manifest.manifest).expect("encode baseline manifest"));
    let baseline = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &request.metadata,
        &request.retention_policy,
        7,
        &rent_policy,
    )
    .expect("manifest with provided bytes");
    let mut tampered = baseline.manifest.clone();
    tampered.storage_ticket = StorageTicketId::new([0xAB; 32]);
    tampered.issued_at_unix = 123_456;
    request.norito_manifest = Some(to_bytes(&tampered).expect("encode manifest"));
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &request.metadata,
        &request.retention_policy,
        7,
        &rent_policy,
    )
    .expect("manifest with provided bytes");
    assert_eq!(baseline.fingerprint, manifest.fingerprint);
    assert_eq!(manifest.manifest.issued_at_unix, 7);
}

#[test]
fn supplied_taikai_manifest_is_stable_across_server_queue_times() {
    let (mut request, mut supplied) = taikai_manifest_fixture();
    let canonical = normalize_payload(&request)
        .expect("normalize payload")
        .into_vec();
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let rent_policy = DaRentPolicyV1::default();
    let manifest_metadata = supplied.manifest.metadata.clone();
    supplied.manifest.issued_at_unix = 1_701_000_123;
    let supplied_bytes = to_bytes(&supplied.manifest).expect("encode caller-supplied manifest");
    let supplied_hash = BlobDigest::from_hash(blake3_hash(&supplied_bytes));
    request.norito_manifest = Some(supplied_bytes.clone());

    let first = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &manifest_metadata,
        &request.retention_policy,
        1_701_000_200,
        &rent_policy,
    )
    .expect("resolve supplied manifest at first queue time");
    let second = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &manifest_metadata,
        &request.retention_policy,
        1_701_000_900,
        &rent_policy,
    )
    .expect("resolve supplied manifest at later queue time");

    assert_eq!(first.manifest.issued_at_unix, 1_701_000_123);
    assert_eq!(first.manifest, supplied.manifest);
    assert_eq!(first.encoded, supplied_bytes);
    assert_eq!(first.manifest_hash, supplied_hash);
    assert_eq!(first.manifest, second.manifest);
    assert_eq!(first.encoded, second.encoded);
    assert_eq!(first.manifest_hash, second.manifest_hash);
    assert_eq!(first.fingerprint, second.fingerprint);
    assert_eq!(first.storage_ticket, second.storage_ticket);
}

#[test]
fn supplied_taikai_manifest_rejects_zero_issued_at() {
    let (mut request, mut supplied) = taikai_manifest_fixture();
    let canonical = normalize_payload(&request)
        .expect("normalize payload")
        .into_vec();
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let rent_policy = DaRentPolicyV1::default();
    let manifest_metadata = supplied.manifest.metadata.clone();
    supplied.manifest.issued_at_unix = 0;
    request.norito_manifest =
        Some(to_bytes(&supplied.manifest).expect("encode zero-time manifest"));

    let err = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &manifest_metadata,
        &request.retention_policy,
        1_701_000_200,
        &rent_policy,
    )
    .expect_err("zero caller-supplied Taikai issued_at_unix must reject");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("issued_at_unix must be greater than zero"),
        "unexpected zero issued_at_unix error: {}",
        err.1
    );
}

#[test]
fn taikai_ssm_requires_caller_supplied_manifest_in_compute_path() {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    request.metadata.items.push(MetadataEntry::new(
        taikai::META_TAIKAI_SSM,
        b"signed-manifest-placeholder".to_vec(),
        MetadataVisibility::Public,
    ));
    let keypair = checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519);
    let digest = request.signing_digest();
    request.signatures[0].signature = checked_signature(keypair.private_key(), &digest);
    let nexus = nexus_with_scheme(request.lane_id, DaProofScheme::MerkleSha256);

    let err = compute_da_manifest_artifacts(
        &request,
        &nexus,
        1,
        None,
        None,
        &DaReplicationPolicy::default(),
        &DaRentPolicyV1::default(),
        None,
    )
    .err()
    .expect("Taikai SSM without caller-supplied manifest must reject");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1
            .contains("require a caller-supplied `norito_manifest`"),
        "unexpected missing manifest error: {}",
        err.1
    );
}

#[test]
fn lane_proof_scheme_rejects_stale_geometry_only_lane() {
    let stale_lane = LaneId::new(3);
    let authoritative_catalog = lane_catalog_with_lanes(vec![ModelLaneConfig::default()]);
    let stale_geometry_catalog = lane_catalog_with_lanes(vec![
        ModelLaneConfig::default(),
        ModelLaneConfig {
            id: stale_lane,
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "stale-ingest".to_owned(),
            proof_scheme: DaProofScheme::MerkleSha256,
            ..ModelLaneConfig::default()
        },
    ]);
    let mut nexus = nexus_with_catalog(authoritative_catalog);
    nexus.lane_config = ConfigLaneConfig::from_catalog(&stale_geometry_catalog);
    assert!(
        nexus.lane_config.entry(stale_lane).is_some(),
        "test must seed derived geometry for the removed lane"
    );
    let err = lane_proof_scheme(&nexus, stale_lane, 1)
        .expect_err("stale geometry-only lane must not resolve a proof scheme");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("active lane catalog"));
}
#[test]
fn lane_proof_scheme_rejects_future_created_autoscale_lane_before_committed_height() {
    let lane_id = LaneId::new(1);
    let mut elastic_lane = ModelLaneConfig {
        id: lane_id,
        dataspace_id: DataSpaceId::UNIVERSAL,
        alias: "elastic-lane-1".to_owned(),
        proof_scheme: DaProofScheme::MerkleSha256,
        ..ModelLaneConfig::default()
    };
    elastic_lane.metadata.insert(
        iroha_data_model::nexus::AUTOSCALE_META_MANAGED.to_owned(),
        "true".to_owned(),
    );
    elastic_lane.metadata.insert(
        iroha_data_model::nexus::AUTOSCALE_META_CREATED_HEIGHT.to_owned(),
        "7".to_owned(),
    );
    let mut nexus = nexus_with_catalog(lane_catalog_with_lanes(vec![
        ModelLaneConfig::default(),
        elastic_lane,
    ]));
    nexus.autoscale.enabled = true;
    nexus.autoscale.min_lane_id = NonZeroU32::new(1).expect("non-zero min lanes");
    nexus.autoscale.max_lane_id_exclusive = NonZeroU32::new(3).expect("non-zero max lanes");
    let err = lane_proof_scheme(&nexus, lane_id, 6)
        .expect_err("future-created autoscale lane must not resolve before creation height");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("active lane catalog"));
    let scheme = lane_proof_scheme(&nexus, lane_id, 7)
        .expect("autoscale lane should resolve at creation height");
    assert_eq!(scheme, DaProofScheme::MerkleSha256);
}
#[test]
fn taikai_envelope_generation_requires_metadata() {
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata = request.metadata.clone();
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        0,
        &rent_policy,
    )
    .expect("manifest");
    let err = match taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        None,
    ) {
        Ok(_) => panic!("missing metadata must error"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

fn taikai_envelope_error_with_metadata_value(key: &str, value: &[u8]) -> (StatusCode, String) {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    request
        .metadata
        .items
        .iter_mut()
        .find(|entry| entry.key == key)
        .expect("Taikai fixture metadata entry")
        .value = value.to_vec();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata = request.metadata.clone();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1,
        &DaRentPolicyV1::default(),
    )
    .expect("manifest");
    match taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        None,
    ) {
        Ok(_) => panic!("zero-valued `{key}` metadata must fail"),
        Err(err) => err,
    }
}

#[test]
fn taikai_envelope_generation_rejects_zero_bitrate() {
    let err = taikai_envelope_error_with_metadata_value(taikai::META_TAIKAI_TRACK_BITRATE, b"0");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("must be greater than zero"));
}

#[test]
fn taikai_envelope_generation_rejects_zero_duration() {
    let err = taikai_envelope_error_with_metadata_value(taikai::META_TAIKAI_SEGMENT_DURATION, b"0");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("must be greater than zero"));
}

#[test]
fn taikai_envelope_generation_computes_pointers() {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata = request.metadata.clone();
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1,
        &rent_policy,
    )
    .expect("manifest");
    let artifacts = taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        None,
    )
    .expect("taikai envelope");
    let envelope: TaikaiSegmentEnvelopeV1 =
        norito::decode_from_bytes(&artifacts.envelope_bytes).expect("decode framed envelope");
    assert_eq!(
        artifacts.envelope_bytes,
        to_bytes(&envelope).expect("re-encode framed envelope")
    );
    assert_eq!(
        envelope.event_id.as_name(),
        &Name::from_str("global-keynote").unwrap()
    );
    assert_eq!(envelope.segment_sequence, 42);
    assert_eq!(
        envelope.ingest.chunk_count,
        chunk_store.chunks().len() as u32
    );
    assert!(envelope.ingest.car.cid_multibase.starts_with('b'));
    let indexes: TaikaiEnvelopeIndexes =
        norito::json::from_slice(&artifacts.indexes_json).expect("decode indexes");
    assert_eq!(indexes.time_key.event_id, envelope.event_id);
    assert_eq!(
        indexes.cid_key.cid_multibase,
        envelope.ingest.car.cid_multibase
    );
}
#[test]
fn taikai_envelope_calls_chunking_observer() {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata = request.metadata.clone();
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1,
        &rent_policy,
    )
    .expect("manifest");
    let called = Cell::new(0u32);
    let observer = |_: Duration| {
        called.set(called.get() + 1);
    };
    taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        Some(&observer),
    )
    .expect("envelope");
    assert_eq!(called.get(), 1);
}
#[test]
fn taikai_artifacts_persist_idempotent() {
    let dir = tempdir().expect("tempdir");
    let lane_id = LaneId::new(3);
    let epoch = 7;
    let sequence = 11;
    let storage_ticket = StorageTicketId::new([0x11; 32]);
    let fingerprint = ReplayFingerprint::from_hash(blake3::hash(b"fingerprint"));
    let envelope_path = taikai_ingest::persist_envelope(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"envelope",
    )
    .expect("persist envelope")
    .expect("path");
    assert!(envelope_path.exists());
    let index_path = taikai_ingest::persist_indexes(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"indexes",
    )
    .expect("persist indexes")
    .expect("path");
    assert!(index_path.exists());
    let trm_path = taikai_ingest::persist_trm(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"trm",
    )
    .expect("persist trm")
    .expect("path");
    assert!(trm_path.exists());
    let ssm_path = taikai_ingest::persist_ssm(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"ssm",
    )
    .expect("persist ssm")
    .expect("path");
    assert!(ssm_path.exists());
    let envelope_second = taikai_ingest::persist_envelope(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"envelope",
    )
    .expect("persist envelope second")
    .expect("path");
    assert_eq!(envelope_path, envelope_second);
    let index_second = taikai_ingest::persist_indexes(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"indexes",
    )
    .expect("persist indexes second")
    .expect("path");
    assert_eq!(index_path, index_second);
    let ssm_second = taikai_ingest::persist_ssm(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"ssm",
    )
    .expect("persist ssm second")
    .expect("path");
    assert_eq!(ssm_path, ssm_second);
    let trm_second = taikai_ingest::persist_trm(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"trm",
    )
    .expect("persist trm second")
    .expect("path");
    assert_eq!(trm_path, trm_second);
    let ready_path = taikai_ingest::persist_anchor_ready(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
    )
    .expect("persist readiness marker")
    .expect("readiness path");
    assert_eq!(
        fs::read(&ready_path).expect("read readiness"),
        b"ready-v1\n"
    );
    let err = taikai_ingest::persist_envelope(
        dir.path(),
        lane_id,
        epoch,
        sequence,
        &storage_ticket,
        &fingerprint,
        b"other",
    )
    .expect_err("mismatched envelope bytes must fail");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}
#[test]
fn taikai_anchor_readiness_does_not_recreate_retired_sources() {
    let dir = tempdir().expect("tempdir");
    let path = taikai_ingest::persist_anchor_ready(
        dir.path(),
        LaneId::new(3),
        7,
        11,
        &StorageTicketId::new([0x11; 32]),
        &ReplayFingerprint::from_hash(blake3::hash(b"fingerprint")),
    )
    .expect("missing source is an idempotent no-op");
    assert!(path.is_none());
}
#[cfg(unix)]
#[test]
fn taikai_artifact_persistence_rejects_spool_dir_symlink() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().expect("tempdir");
    let target = dir.path().join("taikai-write-target");
    fs::create_dir(&target).expect("create Taikai target directory");
    let spool_link = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    symlink(&target, &spool_link).expect("create Taikai spool symlink");
    let lane_id = LaneId::new(3);
    let storage_ticket = StorageTicketId::new([0x11; 32]);
    let fingerprint = ReplayFingerprint::from_hash(blake3::hash(b"fingerprint"));
    let err = taikai_ingest::persist_envelope(
        dir.path(),
        lane_id,
        7,
        11,
        &storage_ticket,
        &fingerprint,
        b"envelope",
    )
    .expect_err("symlinked Taikai spool root must reject artifact persistence");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("Taikai spool directory"),
        "unexpected Taikai spool error: {err}"
    );
    assert!(
        fs::symlink_metadata(&spool_link)
            .expect("inspect Taikai spool symlink")
            .file_type()
            .is_symlink(),
        "failed persistence should leave Taikai spool symlink visible"
    );
    assert_eq!(
        fs::read_dir(&target)
            .expect("read Taikai target directory")
            .count(),
        0,
        "symlink target must not receive Taikai artifacts"
    );
}
#[test]
fn taikai_artifact_persistence_converges_under_same_process_writers() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().to_path_buf();
    let lane_id = LaneId::new(3);
    let epoch = 7;
    let sequence = 11;
    let storage_ticket = StorageTicketId::new([0x11; 32]);
    let fingerprint = ReplayFingerprint::from_hash(blake3::hash(b"fingerprint"));
    let barrier = Arc::new(Barrier::new(4));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let spool_dir = spool_dir.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                taikai_ingest::persist_envelope(
                    &spool_dir,
                    lane_id,
                    epoch,
                    sequence,
                    &storage_ticket,
                    &fingerprint,
                    b"envelope",
                )
                .expect("concurrent Taikai artifact persist")
                .expect("artifact path")
            })
        })
        .collect();
    let paths: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("writer thread"))
        .collect();
    let first = paths.first().expect("at least one writer");
    assert!(paths.iter().all(|path| path == first));
    assert_eq!(fs::read(first).expect("read Taikai envelope"), b"envelope");
    assert!(
        temp_artifact_names(&dir.path().join(TAIKAI_SPOOL_SUBDIR)).is_empty(),
        "concurrent Taikai install should not leave temp artifacts"
    );
}
#[derive(Default)]
struct MockAnchorSender {
    calls: AsyncMutex<Vec<(Url, String, Option<String>)>>,
}
#[async_trait]
impl AnchorSender for MockAnchorSender {
    async fn send(
        &self,
        endpoint: &Url,
        base_id: &str,
        body: &str,
        api_token: Option<&str>,
    ) -> Result<Vec<u8>, AnchorSendError> {
        self.calls.lock().await.push((
            endpoint.clone(),
            body.to_owned(),
            api_token.map(str::to_owned),
        ));
        Ok(signed_anchor_receipt(base_id, body))
    }
}
struct BlockingSentinelAnchorSender {
    calls: AsyncMutex<Vec<(Url, String, Option<String>)>>,
    sentinel_path: PathBuf,
}
#[async_trait]
impl AnchorSender for BlockingSentinelAnchorSender {
    async fn send(
        &self,
        endpoint: &Url,
        base_id: &str,
        body: &str,
        api_token: Option<&str>,
    ) -> Result<Vec<u8>, AnchorSendError> {
        {
            self.calls.lock().await.push((
                endpoint.clone(),
                body.to_owned(),
                api_token.map(str::to_owned),
            ));
        }
        async_fs::create_dir(&self.sentinel_path)
            .await
            .expect("block sentinel path");
        Ok(signed_anchor_receipt(base_id, body))
    }
}
#[derive(Default)]
struct FailingAnchorSender {
    calls: AsyncMutex<Vec<(Url, String, Option<String>)>>,
}
#[async_trait]
impl AnchorSender for FailingAnchorSender {
    async fn send(
        &self,
        endpoint: &Url,
        _base_id: &str,
        body: &str,
        api_token: Option<&str>,
    ) -> Result<Vec<u8>, AnchorSendError> {
        self.calls.lock().await.push((
            endpoint.clone(),
            body.to_owned(),
            api_token.map(str::to_owned),
        ));
        Err(Box::new(std::io::Error::new(
            ErrorKind::ConnectionRefused,
            "anchor service unavailable",
        )))
    }
}
#[derive(Default)]
struct FirstFailingAnchorSender {
    calls: AsyncMutex<Vec<(Url, String, Option<String>)>>,
}
#[async_trait]
impl AnchorSender for FirstFailingAnchorSender {
    async fn send(
        &self,
        endpoint: &Url,
        base_id: &str,
        body: &str,
        api_token: Option<&str>,
    ) -> Result<Vec<u8>, AnchorSendError> {
        let call_count = {
            let mut calls = self.calls.lock().await;
            calls.push((
                endpoint.clone(),
                body.to_owned(),
                api_token.map(str::to_owned),
            ));
            calls.len()
        };
        if call_count == 1 {
            return Err(Box::new(std::io::Error::new(
                ErrorKind::ConnectionRefused,
                "anchor service unavailable for first upload",
            )));
        }
        Ok(signed_anchor_receipt(base_id, body))
    }
}
struct FirstBlockingSentinelAnchorSender {
    calls: AsyncMutex<Vec<(Url, String, Option<String>)>>,
    sentinel_paths_by_body: BTreeMap<String, PathBuf>,
}
#[async_trait]
impl AnchorSender for FirstBlockingSentinelAnchorSender {
    async fn send(
        &self,
        endpoint: &Url,
        base_id: &str,
        body: &str,
        api_token: Option<&str>,
    ) -> Result<Vec<u8>, AnchorSendError> {
        let call_count = {
            let mut calls = self.calls.lock().await;
            calls.push((
                endpoint.clone(),
                body.to_owned(),
                api_token.map(str::to_owned),
            ));
            calls.len()
        };
        if call_count == 1 {
            let sentinel_path = self
                .sentinel_paths_by_body
                .get(body)
                .expect("first upload body should have a sentinel path");
            async_fs::create_dir(sentinel_path)
                .await
                .expect("block first sentinel path");
        }
        Ok(signed_anchor_receipt(base_id, body))
    }
}

struct StaticAnchorResponseSender {
    response: Vec<u8>,
}

#[async_trait]
impl AnchorSender for StaticAnchorResponseSender {
    async fn send(
        &self,
        _endpoint: &Url,
        _base_id: &str,
        _body: &str,
        _api_token: Option<&str>,
    ) -> Result<Vec<u8>, AnchorSendError> {
        Ok(self.response.clone())
    }
}

async fn write_minimal_taikai_anchor_artifacts(spool_dir: &Path, base_id: &str) {
    async_fs::create_dir_all(spool_dir)
        .await
        .expect("create spool");
    async_fs::write(
        spool_dir.join(format!("taikai-envelope-{base_id}.norito")),
        b"envelope-bytes",
    )
    .await
    .expect("write envelope");
    async_fs::write(
        spool_dir.join(format!("taikai-indexes-{base_id}.json")),
        b"{}",
    )
    .await
    .expect("write indexes");
    async_fs::write(
        spool_dir.join(format!("taikai-ssm-{base_id}.norito")),
        b"ssm-bytes",
    )
    .await
    .expect("write ssm");
    async_fs::write(
        spool_dir.join(format!(
            "{TAIKAI_ANCHOR_READY_PREFIX}{base_id}{TAIKAI_ANCHOR_READY_SUFFIX}"
        )),
        b"ready-v1\n",
    )
    .await
    .expect("write readiness marker");
}
const ANCHOR_BASE_ID: &str = "00000001-0000000000000002-0000000000000003-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
struct AnchorFixture {
    _dir: tempfile::TempDir,
    spool_dir: PathBuf,
    base_id: &'static str,
}
async fn minimal_anchor_fixture(base_id: &'static str) -> AnchorFixture {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    write_minimal_taikai_anchor_artifacts(&spool_dir, base_id).await;
    AnchorFixture {
        _dir: dir,
        spool_dir,
        base_id,
    }
}
fn taikai_anchor_config(api_token: Option<&str>) -> DaTaikaiAnchor {
    DaTaikaiAnchor {
        endpoint: Url::parse("http://localhost/anchor").unwrap(),
        api_token: api_token.map(str::to_owned),
        receipt_public_key: anchor_signer().public_key().clone(),
        poll_interval: Duration::from_secs(5),
        request_timeout: Duration::from_secs(5),
    }
}
fn anchor_signer() -> KeyPair {
    checked_fixture_ed25519_keypair(0xA7)
}
fn signed_anchor_receipt(base_id: &str, request_body: &str) -> Vec<u8> {
    signed_anchor_receipt_with_signer(base_id, request_body, &anchor_signer())
}

fn signed_anchor_receipt_with_signer(
    base_id: &str,
    request_body: &str,
    signer: &KeyPair,
) -> Vec<u8> {
    let body = TaikaiAnchorReceiptBodyV1 {
        schema: TAIKAI_ANCHOR_RECEIPT_SCHEMA_V1.to_owned(),
        version: TAIKAI_ANCHOR_RECEIPT_VERSION_V1,
        base_id: base_id.to_owned(),
        request_digest: *blake3_hash(request_body.as_bytes()).as_bytes(),
        acknowledged_unix_secs: 1_750_000_000,
    };
    let receipt =
        TaikaiAnchorReceiptV1::try_sign(body, signer).expect("sign Taikai anchor receipt");
    json::to_vec(&receipt).expect("encode Taikai anchor receipt")
}
#[cfg(unix)]
async fn replace_path_with_symlink(path: &Path, target_contents: &[u8]) -> PathBuf {
    use std::os::unix::fs::symlink;
    if let Err(err) = async_fs::remove_file(path).await {
        assert_eq!(
            err.kind(),
            ErrorKind::NotFound,
            "failed to remove existing path before symlink replacement: {err}"
        );
    }
    let target = path.with_extension("symlink-target");
    async_fs::write(&target, target_contents)
        .await
        .expect("write symlink target");
    symlink(&target, path).expect("create symlink");
    target
}
#[cfg(unix)]
fn assert_path_remains_symlink(path: &Path, target: &Path) {
    assert!(
        fs::symlink_metadata(path)
            .expect("inspect symlink")
            .file_type()
            .is_symlink(),
        "failed validation should leave symlink visible for operator repair"
    );
    assert!(target.exists(), "symlink target should not be removed");
}
#[tokio::test]
async fn taikai_collect_pending_uploads_sorts_by_base_id() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    let base_a = "00000001-0000000000000002-0000000000000003-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let base_b = "00000001-0000000000000002-0000000000000004-cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc-dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    write_minimal_taikai_anchor_artifacts(&spool_dir, base_b).await;
    write_minimal_taikai_anchor_artifacts(&spool_dir, base_a).await;
    let pending = collect_pending_uploads(&spool_dir)
        .await
        .expect("collect pending uploads");
    let observed: Vec<_> = pending.iter().map(|upload| upload.base_id()).collect();
    assert_eq!(observed, vec![base_a, base_b]);
}
#[tokio::test]
async fn taikai_anchor_collection_waits_for_durable_readiness_marker() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    write_minimal_taikai_anchor_artifacts(&spool_dir, ANCHOR_BASE_ID).await;
    let ready_path = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_READY_PREFIX}{ANCHOR_BASE_ID}{TAIKAI_ANCHOR_READY_SUFFIX}"
    ));
    async_fs::remove_file(&ready_path)
        .await
        .expect("remove readiness marker");
    assert!(
        collect_pending_uploads(&spool_dir)
            .await
            .expect("incomplete upload collection")
            .is_empty(),
        "an envelope must remain invisible until its durable receipt publishes readiness"
    );
    async_fs::write(&ready_path, b"ready-v1\n")
        .await
        .expect("restore readiness marker");
    assert_eq!(
        collect_pending_uploads(&spool_dir)
            .await
            .expect("ready upload collection")
            .len(),
        1
    );
}
#[tokio::test]
async fn taikai_anchor_processing_continues_after_candidate_load_failure() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    let corrupt_base = "00000001-0000000000000002-0000000000000003-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let valid_base = "00000001-0000000000000002-0000000000000004-cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc-dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    write_minimal_taikai_anchor_artifacts(&spool_dir, corrupt_base).await;
    write_minimal_taikai_anchor_artifacts(&spool_dir, valid_base).await;
    async_fs::write(
        spool_dir.join(format!("taikai-indexes-{corrupt_base}.json")),
        b"{not-json",
    )
    .await
    .expect("corrupt first indexes");
    let sender = MockAnchorSender::default();
    let err = process_batch(&spool_dir, &taikai_anchor_config(None), &sender)
        .await
        .expect_err("corrupt candidate must be reported");
    assert!(err.contains(corrupt_base), "unexpected error: {err}");
    assert_eq!(
        sender.calls.lock().await.len(),
        1,
        "the valid later candidate must still be delivered"
    );
    assert!(
        spool_dir
            .join(format!(
                "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{valid_base}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
            ))
            .exists(),
        "later successful candidate must be acknowledged"
    );
    assert!(
        spool_dir
            .join(format!(
                "{TAIKAI_ANCHOR_READY_PREFIX}{corrupt_base}{TAIKAI_ANCHOR_READY_SUFFIX}"
            ))
            .is_file(),
        "a failed candidate must remain durable for retry or operator repair"
    );
}
#[tokio::test]
async fn taikai_anchor_processing_generates_payload_and_sentinel() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    async_fs::create_dir_all(&spool_dir)
        .await
        .expect("create spool");
    let base_id = "00000001-0000000000000002-0000000000000003-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let envelope_path = spool_dir.join(format!("taikai-envelope-{base_id}.norito"));
    async_fs::write(&envelope_path, b"envelope-bytes")
        .await
        .expect("write envelope");
    let indexes = TaikaiEnvelopeIndexes {
        time_key: TaikaiTimeIndexKey {
            event_id: TaikaiEventId::new(Name::from_str("global-keynote").unwrap()),
            stream_id: TaikaiStreamId::new(Name::from_str("stage-a").unwrap()),
            rendition_id: TaikaiRenditionId::new(Name::from_str("1080p").unwrap()),
            segment_start_pts: SegmentTimestamp::new(3_600_000),
        },
        cid_key: TaikaiCidIndexKey {
            event_id: TaikaiEventId::new(Name::from_str("global-keynote").unwrap()),
            stream_id: TaikaiStreamId::new(Name::from_str("stage-a").unwrap()),
            rendition_id: TaikaiRenditionId::new(Name::from_str("1080p").unwrap()),
            cid_multibase: "zbafyqra".to_string(),
        },
    };
    let indexes_json = norito::json::to_json_pretty(&indexes).expect("indexes json");
    let indexes_path = spool_dir.join(format!("taikai-indexes-{base_id}.json"));
    async_fs::write(&indexes_path, indexes_json.as_bytes())
        .await
        .expect("write indexes");
    let ssm_path = spool_dir.join(format!("taikai-ssm-{base_id}.norito"));
    async_fs::write(&ssm_path, b"ssm-bytes")
        .await
        .expect("write ssm");
    async_fs::write(
        spool_dir.join(format!(
            "{TAIKAI_ANCHOR_READY_PREFIX}{base_id}{TAIKAI_ANCHOR_READY_SUFFIX}"
        )),
        b"ready-v1\n",
    )
    .await
    .expect("write readiness marker");
    let trm_bytes = sample_trm_bytes();
    let trm_path = spool_dir.join(format!("taikai-trm-{base_id}.norito"));
    async_fs::write(&trm_path, &trm_bytes)
        .await
        .expect("write trm");
    let mut lineage_hint = Map::new();
    lineage_hint.insert("version".into(), Value::from(1));
    lineage_hint.insert("alias_namespace".into(), Value::from("sora"));
    lineage_hint.insert("alias_name".into(), Value::from("docs"));
    lineage_hint.insert(
        "previous_manifest_digest_hex".into(),
        Value::from("cafebabe"),
    );
    lineage_hint.insert("previous_window_start_sequence".into(), Value::from(1));
    lineage_hint.insert("previous_window_end_sequence".into(), Value::from(120));
    lineage_hint.insert("previous_updated_unix".into(), Value::from(1_234_567));
    let lineage_value = Value::Object(lineage_hint.clone());
    let lineage_path = spool_dir.join(format!("taikai-lineage-{base_id}.json"));
    async_fs::write(
        &lineage_path,
        json::to_string(&lineage_value)
            .expect("lineage json")
            .as_bytes(),
    )
    .await
    .expect("write lineage hint");
    let anchor_cfg = taikai_anchor_config(Some("secret-token"));
    let pending = collect_pending_uploads(&spool_dir)
        .await
        .expect("collect pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].base_id(), base_id);
    let payload: Value = norito::json::from_str(pending[0].body()).expect("payload json");
    assert_eq!(
        payload.get("envelope_base64").and_then(Value::as_str),
        Some(BASE64.encode(b"envelope-bytes")).as_deref()
    );
    assert_eq!(
        payload.get("ssm_base64").and_then(Value::as_str),
        Some(BASE64.encode(b"ssm-bytes")).as_deref()
    );
    assert_eq!(
        payload.get("trm_base64").and_then(Value::as_str),
        Some(BASE64.encode(&trm_bytes)).as_deref()
    );
    assert_eq!(payload.get("lineage_hint"), Some(&lineage_value));
    let request_capture = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_REQUEST_PREFIX}{base_id}{TAIKAI_ANCHOR_REQUEST_SUFFIX}"
    ));
    let capture_contents = async_fs::read_to_string(&request_capture)
        .await
        .expect("request capture after collection");
    assert_eq!(capture_contents, pending[0].body());
    assert!(
        temp_artifact_names(&spool_dir).is_empty(),
        "request capture persistence should not leave temporary artifacts"
    );
    let pending_again = collect_pending_uploads(&spool_dir)
        .await
        .expect("collect pending idempotently");
    assert_eq!(pending_again.len(), 1);
    assert_eq!(pending_again[0].body(), pending[0].body());
    assert!(
        temp_artifact_names(&spool_dir).is_empty(),
        "idempotent request capture persistence should not leave temporary artifacts"
    );
    let sender = MockAnchorSender::default();
    process_batch(&spool_dir, &anchor_cfg, &sender)
        .await
        .expect("process batch");
    let calls = sender.calls.lock().await.clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, anchor_cfg.endpoint);
    assert_eq!(calls[0].2.as_deref(), anchor_cfg.api_token.as_deref());
    assert_eq!(calls[0].1, pending[0].body());
    let sentinel = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
    ));
    assert!(async_fs::metadata(&sentinel).await.is_ok());
    let capture_contents = async_fs::read_to_string(&request_capture)
        .await
        .expect("request capture");
    assert_eq!(capture_contents, pending[0].body());
    let pending_after = collect_pending_uploads(&spool_dir)
        .await
        .expect("collect after upload");
    assert!(pending_after.is_empty());
}

#[tokio::test]
async fn taikai_anchor_processing_rejects_status_only_success() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let sender = StaticAnchorResponseSender {
        response: Vec::new(),
    };

    let err = process_batch(&spool_dir, &taikai_anchor_config(None), &sender)
        .await
        .expect_err("an empty 2xx response must not acknowledge an upload");

    assert!(
        err.contains("invalid Taikai receipt"),
        "unexpected process error: {err}"
    );
    assert!(
        spool_dir
            .join(format!("taikai-envelope-{base_id}.norito"))
            .is_file(),
        "status-only success must leave source artefacts retryable"
    );
    assert!(
        !spool_dir
            .join(format!(
                "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
            ))
            .exists(),
        "status-only success must not create an acknowledgement"
    );
    assert_eq!(
        collect_pending_uploads(&spool_dir)
            .await
            .expect("collect after status-only success")
            .len(),
        1
    );
}

#[tokio::test]
async fn taikai_anchor_processing_rejects_receipt_for_different_request() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let sender = StaticAnchorResponseSender {
        response: signed_anchor_receipt(base_id, "different exact request bytes"),
    };

    let err = process_batch(&spool_dir, &taikai_anchor_config(None), &sender)
        .await
        .expect_err("a receipt for different bytes must not acknowledge an upload");

    assert!(
        err.contains("request digest does not match"),
        "unexpected process error: {err}"
    );
    assert!(
        spool_dir
            .join(format!("taikai-envelope-{base_id}.norito"))
            .is_file(),
        "request-binding failure must leave source artefacts retryable"
    );
}

#[tokio::test]
async fn taikai_anchor_processing_rejects_receipt_for_different_base_id() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let pending = collect_pending_uploads(&spool_dir)
        .await
        .expect("prepare request capture");
    let different_base_id = "00000001-0000000000000002-0000000000000004-cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc-dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    let sender = StaticAnchorResponseSender {
        response: signed_anchor_receipt(different_base_id, pending[0].body()),
    };

    let err = process_batch(&spool_dir, &taikai_anchor_config(None), &sender)
        .await
        .expect_err("a receipt for another artefact must not acknowledge an upload");

    assert!(
        err.contains("receipt base_id") && err.contains("does not match"),
        "unexpected process error: {err}"
    );
    assert!(
        spool_dir
            .join(format!("taikai-envelope-{base_id}.norito"))
            .is_file(),
        "artefact-binding failure must leave source artefacts retryable"
    );
}

#[tokio::test]
async fn taikai_anchor_processing_rejects_receipt_from_unpinned_signer() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let pending = collect_pending_uploads(&spool_dir)
        .await
        .expect("prepare request capture");
    let wrong_signer = checked_fixture_ed25519_keypair(0xB8);
    let sender = StaticAnchorResponseSender {
        response: signed_anchor_receipt_with_signer(base_id, pending[0].body(), &wrong_signer),
    };

    let err = process_batch(&spool_dir, &taikai_anchor_config(None), &sender)
        .await
        .expect_err("an untrusted signer must not acknowledge an upload");

    assert!(
        err.contains("signature validation failed"),
        "unexpected process error: {err}"
    );
    assert!(
        spool_dir
            .join(format!("taikai-envelope-{base_id}.norito"))
            .is_file(),
        "signer validation failure must leave source artefacts retryable"
    );
}

#[tokio::test]
async fn taikai_anchor_restart_quarantines_legacy_timestamp_sentinel() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    collect_pending_uploads(&spool_dir)
        .await
        .expect("prepare request capture");
    let sentinel = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
    ));
    async_fs::write(&sentinel, b"1750000000\n")
        .await
        .expect("write legacy timestamp sentinel");

    let pending = collect_pending_uploads(&spool_dir)
        .await
        .expect("a legacy marker should be quarantined without blocking retry");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].base_id(), base_id);
    assert!(
        spool_dir
            .join(format!("taikai-envelope-{base_id}.norito"))
            .is_file(),
        "unverified restart state must not retire source artefacts"
    );
    assert!(
        !sentinel.exists(),
        "legacy marker must leave the live namespace"
    );
    let quarantine_prefix = format!(
        "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}{TAIKAI_ANCHOR_INVALID_SUFFIX}-"
    );
    let quarantined = fs::read_dir(&spool_dir)
        .expect("scan quarantine evidence")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(&quarantine_prefix))
        .count();
    assert_eq!(quarantined, 1, "legacy marker must be retained as evidence");
}

#[tokio::test]
async fn taikai_anchor_prune_quarantines_orphan_legacy_sentinel() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    async_fs::create_dir(&spool_dir)
        .await
        .expect("create Taikai spool");
    let sentinel = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{ANCHOR_BASE_ID}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
    ));
    async_fs::write(&sentinel, b"1750000000\n")
        .await
        .expect("write orphan legacy sentinel");
    let sender = MockAnchorSender::default();

    process_batch(&spool_dir, &taikai_anchor_config(None), &sender)
        .await
        .expect("orphan legacy marker should be quarantined during pruning");

    assert!(
        sender.calls.lock().await.is_empty(),
        "an orphan acknowledgement must not trigger an upload"
    );
    assert!(
        !sentinel.exists(),
        "orphan legacy marker must leave the live acknowledgement namespace"
    );
    let quarantine_prefix = format!(
        "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{ANCHOR_BASE_ID}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}{TAIKAI_ANCHOR_INVALID_SUFFIX}-"
    );
    let quarantined = fs::read_dir(&spool_dir)
        .expect("scan orphan quarantine evidence")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(&quarantine_prefix))
        .count();
    assert_eq!(
        quarantined, 1,
        "orphan legacy marker must be retained as evidence"
    );
}

#[tokio::test]
async fn taikai_anchor_restart_accepts_exact_signed_receipt() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let pending = collect_pending_uploads(&spool_dir)
        .await
        .expect("prepare request capture");
    let sentinel = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
    ));
    async_fs::write(&sentinel, signed_anchor_receipt(base_id, pending[0].body()))
        .await
        .expect("persist signed receipt before simulated restart");

    assert!(
        collect_pending_uploads(&spool_dir)
            .await
            .expect("recover exact signed receipt")
            .is_empty()
    );
    for source in [
        format!("taikai-envelope-{base_id}.norito"),
        format!("taikai-indexes-{base_id}.json"),
        format!("taikai-ssm-{base_id}.norito"),
        format!("{TAIKAI_ANCHOR_READY_PREFIX}{base_id}{TAIKAI_ANCHOR_READY_SUFFIX}"),
    ] {
        assert!(
            !spool_dir.join(source).exists(),
            "verified restart recovery must retire source artefacts"
        );
    }
    assert!(
        sentinel.is_file(),
        "verified receipt remains as audit evidence"
    );
    assert!(
        spool_dir
            .join(format!(
                "{TAIKAI_ANCHOR_REQUEST_PREFIX}{base_id}{TAIKAI_ANCHOR_REQUEST_SUFFIX}"
            ))
            .is_file(),
        "exact request capture remains available for future verification"
    );
}

#[tokio::test]
async fn taikai_anchor_processing_reports_anchor_delivery_failure() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let sentinel = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
    ));
    let anchor_cfg = taikai_anchor_config(None);
    let sender = FailingAnchorSender::default();
    let err = process_batch(&spool_dir, &anchor_cfg, &sender)
        .await
        .expect_err("anchor delivery failure should fail batch processing");
    assert!(
        err.contains("failed to deliver Taikai envelope"),
        "unexpected process error: {err}"
    );
    assert!(
        err.contains(base_id),
        "delivery error should identify affected artifact: {err}"
    );
    assert!(
        err.contains("anchor service unavailable"),
        "delivery error should retain sender error context: {err}"
    );
    assert_eq!(sender.calls.lock().await.len(), 1);
    assert!(
        async_fs::metadata(&sentinel).await.is_err(),
        "failed delivery must not mark the upload as anchored"
    );
    let pending_after = collect_pending_uploads(&spool_dir)
        .await
        .expect("collect after failed delivery");
    assert_eq!(pending_after.len(), 1);
    assert_eq!(pending_after[0].base_id(), base_id);
}
#[tokio::test]
async fn taikai_anchor_processing_continues_after_anchor_delivery_failure() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    let base_ids = [
        "00000001-0000000000000002-0000000000000003-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "00000001-0000000000000002-0000000000000004-cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc-dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
    ];
    for (base_id, label) in base_ids.iter().zip(["first", "second"]) {
        write_minimal_taikai_anchor_artifacts(&spool_dir, base_id).await;
        async_fs::write(
            spool_dir.join(format!("taikai-indexes-{base_id}.json")),
            format!(r#"{{"case":"{label}"}}"#),
        )
        .await
        .expect("write distinct indexes");
    }
    let pending_before = collect_pending_uploads(&spool_dir)
        .await
        .expect("collect pending before upload");
    assert_eq!(pending_before.len(), 2);
    assert_ne!(
        pending_before[0].body(),
        pending_before[1].body(),
        "test fixture bodies must identify which upload failed"
    );
    let anchor_cfg = taikai_anchor_config(None);
    let sender = FirstFailingAnchorSender::default();
    let err = process_batch(&spool_dir, &anchor_cfg, &sender)
        .await
        .expect_err("first delivery failure should still be reported");
    assert!(
        err.contains("failed to deliver Taikai envelope"),
        "unexpected process error: {err}"
    );
    assert!(
        err.contains("anchor service unavailable for first upload"),
        "delivery error should retain sender error context: {err}"
    );
    let calls = sender.calls.lock().await.clone();
    assert_eq!(
        calls.len(),
        2,
        "batch processing must attempt later uploads after a delivery failure"
    );
    let failed_base_id = pending_before
        .iter()
        .find(|pending| pending.body() == calls[0].1.as_str())
        .map(|pending| pending.base_id().to_string())
        .expect("failed upload body should come from pending set");
    let succeeded_base_id = pending_before
        .iter()
        .find(|pending| pending.body() == calls[1].1.as_str())
        .map(|pending| pending.base_id().to_string())
        .expect("successful upload body should come from pending set");
    assert_ne!(failed_base_id, succeeded_base_id);
    assert!(
        err.contains(&failed_base_id),
        "delivery error should identify failed artifact: {err}"
    );
    let sentinel_path = |base_id: &str| {
        spool_dir.join(format!(
            "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
        ))
    };
    assert!(
        async_fs::metadata(sentinel_path(&failed_base_id))
            .await
            .is_err(),
        "failed delivery must not mark the upload as anchored"
    );
    assert!(
        async_fs::metadata(sentinel_path(&succeeded_base_id))
            .await
            .is_ok(),
        "later successful delivery should be marked as anchored"
    );
    let pending_after = collect_pending_uploads(&spool_dir)
        .await
        .expect("collect after partial delivery failure");
    assert_eq!(pending_after.len(), 1);
    assert_eq!(pending_after[0].base_id(), failed_base_id);
}
#[tokio::test]
async fn taikai_anchor_processing_reports_all_anchor_delivery_failures() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    let base_ids = [
        "00000001-0000000000000002-0000000000000003-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "00000001-0000000000000002-0000000000000004-cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc-dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
    ];
    for base_id in base_ids {
        write_minimal_taikai_anchor_artifacts(&spool_dir, base_id).await;
    }
    let anchor_cfg = taikai_anchor_config(None);
    let sender = FailingAnchorSender::default();
    let err = process_batch(&spool_dir, &anchor_cfg, &sender)
        .await
        .expect_err("delivery failures should fail batch processing");
    assert!(
        err.contains("failed to process 2 Taikai anchor uploads"),
        "unexpected process error: {err}"
    );
    for base_id in base_ids {
        assert!(
            err.contains(base_id),
            "aggregate error should identify every failed artifact: {err}"
        );
    }
    assert_eq!(
        sender.calls.lock().await.len(),
        2,
        "batch processing must attempt every pending upload"
    );
    let sentinel_path = |base_id: &str| {
        spool_dir.join(format!(
            "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
        ))
    };
    for base_id in base_ids {
        assert!(
            async_fs::metadata(sentinel_path(base_id)).await.is_err(),
            "failed delivery must not mark upload as anchored"
        );
    }
    let pending_after = collect_pending_uploads(&spool_dir)
        .await
        .expect("collect after failed deliveries");
    let pending_base_ids: BTreeSet<_> = pending_after
        .iter()
        .map(|pending| pending.base_id().to_string())
        .collect();
    assert_eq!(
        pending_base_ids,
        base_ids.into_iter().map(str::to_string).collect()
    );
}
#[tokio::test]
async fn taikai_anchor_processing_continues_after_sentinel_persistence_failure() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    let base_ids = [
        "00000001-0000000000000002-0000000000000003-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "00000001-0000000000000002-0000000000000004-cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc-dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
    ];
    for (base_id, label) in base_ids.iter().zip(["first", "second"]) {
        write_minimal_taikai_anchor_artifacts(&spool_dir, base_id).await;
        async_fs::write(
            spool_dir.join(format!("taikai-indexes-{base_id}.json")),
            format!(r#"{{"case":"{label}"}}"#),
        )
        .await
        .expect("write distinct indexes");
    }
    let pending_before = collect_pending_uploads(&spool_dir)
        .await
        .expect("collect pending before upload");
    assert_eq!(pending_before.len(), 2);
    let sentinel_paths_by_body = pending_before
        .iter()
        .map(|pending| {
            (
                pending.body().to_string(),
                spool_dir.join(format!(
                    "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}",
                    pending.base_id()
                )),
            )
        })
        .collect();
    let anchor_cfg = taikai_anchor_config(None);
    let sender = FirstBlockingSentinelAnchorSender {
        calls: AsyncMutex::new(Vec::new()),
        sentinel_paths_by_body,
    };
    let err = process_batch(&spool_dir, &anchor_cfg, &sender)
        .await
        .expect_err("blocked first sentinel should still be reported");
    assert!(
        err.contains("failed to persist Taikai anchor sentinel"),
        "unexpected process error: {err}"
    );
    let calls = sender.calls.lock().await.clone();
    assert_eq!(
        calls.len(),
        2,
        "batch processing must attempt later uploads after a sentinel failure"
    );
    let failed_base_id = pending_before
        .iter()
        .find(|pending| pending.body() == calls[0].1.as_str())
        .map(|pending| pending.base_id().to_string())
        .expect("failed upload body should come from pending set");
    let succeeded_base_id = pending_before
        .iter()
        .find(|pending| pending.body() == calls[1].1.as_str())
        .map(|pending| pending.base_id().to_string())
        .expect("successful upload body should come from pending set");
    assert_ne!(failed_base_id, succeeded_base_id);
    assert!(
        err.contains(&failed_base_id),
        "sentinel error should identify failed artifact path: {err}"
    );
    let sentinel_path = |base_id: &str| {
        spool_dir.join(format!(
            "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
        ))
    };
    assert!(
        async_fs::metadata(sentinel_path(&failed_base_id))
            .await
            .expect("failed sentinel path metadata")
            .is_dir(),
        "failed sentinel path should remain blocked for operator inspection"
    );
    assert!(
        async_fs::metadata(sentinel_path(&succeeded_base_id))
            .await
            .is_ok(),
        "later successful delivery should still be marked as anchored"
    );
    assert!(
        temp_artifact_names(&spool_dir).is_empty(),
        "failed sentinel persistence should clean up temporary artifacts"
    );
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("blocked sentinel must reject later anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("is not a regular file"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(&sentinel_path(&failed_base_id).display().to_string()),
        "error should identify non-file sentinel path: {err}"
    );
}
#[tokio::test]
async fn taikai_anchor_processing_rejects_unpersistable_sentinel_after_upload() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let sentinel = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
    ));
    let anchor_cfg = taikai_anchor_config(None);
    let sender = BlockingSentinelAnchorSender {
        calls: AsyncMutex::new(Vec::new()),
        sentinel_path: sentinel.clone(),
    };
    let err = process_batch(&spool_dir, &anchor_cfg, &sender)
        .await
        .expect_err("blocked sentinel should fail batch processing");
    assert!(
        err.contains("failed to persist Taikai anchor sentinel"),
        "unexpected process error: {err}"
    );
    assert!(
        err.contains(&sentinel.display().to_string()),
        "error should identify blocked sentinel path: {err}"
    );
    assert_eq!(sender.calls.lock().await.len(), 1);
    assert!(
        async_fs::metadata(&sentinel)
            .await
            .expect("sentinel path metadata")
            .is_dir(),
        "test sender should leave a directory at the sentinel path"
    );
    assert!(
        temp_artifact_names(&spool_dir).is_empty(),
        "failed sentinel persistence should clean up temporary artifacts"
    );
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("non-file sentinel must reject later anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("is not a regular file"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(&sentinel.display().to_string()),
        "error should identify non-file sentinel path: {err}"
    );
}
#[tokio::test]
async fn taikai_anchor_collection_rejects_malformed_base_id() {
    let base_id = "not-a-production-base-id";
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(base_id).await;
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("malformed base id must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("malformed spool artifact id"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(base_id),
        "error should identify malformed base id: {err}"
    );
}
#[tokio::test]
async fn taikai_anchor_collection_rejects_non_file_sentinel() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let sentinel = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
    ));
    async_fs::create_dir(&sentinel)
        .await
        .expect("create sentinel directory");
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("non-file sentinel must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("is not a regular file"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(&sentinel.display().to_string()),
        "error should identify non-file sentinel path: {err}"
    );
}
#[cfg(unix)]
#[tokio::test]
async fn taikai_anchor_collection_rejects_symlinked_sentinel() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let sentinel = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_SENTINEL_PREFIX}{base_id}{TAIKAI_ANCHOR_SENTINEL_SUFFIX}"
    ));
    let target = replace_path_with_symlink(&sentinel, b"uploaded").await;
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("symlinked sentinel must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("Taikai anchor sentinel") && err.contains("is not a regular file"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(&sentinel.display().to_string()),
        "error should identify symlinked sentinel path: {err}"
    );
    assert_path_remains_symlink(&sentinel, &target);
}
#[cfg(unix)]
#[tokio::test]
async fn taikai_anchor_collection_rejects_symlinked_spool_root() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().expect("tempdir");
    let target = dir.path().join("taikai-spool-target");
    async_fs::create_dir(&target)
        .await
        .expect("create target directory");
    let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    symlink(&target, &spool_dir).expect("create Taikai spool symlink");
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("symlinked Taikai spool root must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("Taikai spool directory") && err.contains("not a directory"),
        "unexpected anchor collection error: {err}"
    );
    assert_path_remains_symlink(&spool_dir, &target);
}
#[cfg(unix)]
#[tokio::test]
async fn taikai_anchor_collection_rejects_symlinked_envelope() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let envelope = spool_dir.join(format!("taikai-envelope-{base_id}.norito"));
    let target = replace_path_with_symlink(&envelope, b"envelope-bytes").await;
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("symlinked envelope must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("Taikai envelope") && err.contains("is not a regular file"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(&envelope.display().to_string()),
        "error should identify symlinked envelope path: {err}"
    );
    assert_path_remains_symlink(&envelope, &target);
}
#[cfg(unix)]
#[tokio::test]
async fn taikai_anchor_collection_rejects_symlinked_required_companion() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let indexes = spool_dir.join(format!("taikai-indexes-{base_id}.json"));
    let target = replace_path_with_symlink(&indexes, b"{}").await;
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("symlinked indexes companion must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("Taikai indexes JSON") && err.contains("is not a regular file"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(&indexes.display().to_string()),
        "error should identify symlinked indexes path: {err}"
    );
    assert_path_remains_symlink(&indexes, &target);
}
#[tokio::test]
async fn taikai_anchor_collection_rejects_missing_required_artifacts() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().join(TAIKAI_SPOOL_SUBDIR);
    async_fs::create_dir_all(&spool_dir)
        .await
        .expect("create spool");
    let base_id = "00000001-0000000000000002-0000000000000003-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    async_fs::write(
        spool_dir.join(format!("taikai-envelope-{base_id}.norito")),
        b"envelope-bytes",
    )
    .await
    .expect("write envelope");
    async_fs::write(
        spool_dir.join(format!(
            "{TAIKAI_ANCHOR_READY_PREFIX}{base_id}{TAIKAI_ANCHOR_READY_SUFFIX}"
        )),
        b"ready-v1\n",
    )
    .await
    .expect("write readiness marker");
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("missing required companion artifact must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("failed to read Taikai indexes JSON"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(base_id),
        "error should identify affected base id: {err}"
    );
}
#[tokio::test]
async fn taikai_anchor_collection_rejects_corrupt_indexes_json() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    async_fs::write(
        spool_dir.join(format!("taikai-indexes-{base_id}.json")),
        b"{not-json",
    )
    .await
    .expect("write corrupt indexes");
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("corrupt indexes JSON must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("failed to parse Taikai indexes JSON"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(base_id),
        "error should identify affected base id: {err}"
    );
}
#[tokio::test]
async fn taikai_anchor_collection_rejects_corrupt_lineage_hint() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    async_fs::write(
        spool_dir.join(format!("taikai-lineage-{base_id}.json")),
        b"{not-json",
    )
    .await
    .expect("write corrupt lineage");
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("corrupt lineage hint must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("failed to parse Taikai lineage hint JSON"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(base_id),
        "error should identify affected base id: {err}"
    );
}
#[cfg(unix)]
#[tokio::test]
async fn taikai_anchor_collection_rejects_symlinked_optional_trm() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let trm = spool_dir.join(format!("taikai-trm-{base_id}.norito"));
    let target = replace_path_with_symlink(&trm, b"trm-bytes").await;
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("symlinked optional TRM must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("Taikai routing manifest") && err.contains("is not a regular file"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(&trm.display().to_string()),
        "error should identify symlinked TRM path: {err}"
    );
    assert_path_remains_symlink(&trm, &target);
}
#[cfg(unix)]
#[tokio::test]
async fn taikai_anchor_collection_rejects_symlinked_optional_lineage_hint() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let lineage = spool_dir.join(format!("taikai-lineage-{base_id}.json"));
    let target = replace_path_with_symlink(&lineage, b"{}").await;
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("symlinked optional lineage hint must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("Taikai lineage hint JSON") && err.contains("is not a regular file"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(&lineage.display().to_string()),
        "error should identify symlinked lineage hint path: {err}"
    );
    assert_path_remains_symlink(&lineage, &target);
}
#[tokio::test]
async fn taikai_anchor_collection_rejects_blocked_request_capture() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let request_capture = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_REQUEST_PREFIX}{base_id}{TAIKAI_ANCHOR_REQUEST_SUFFIX}"
    ));
    async_fs::create_dir(&request_capture)
        .await
        .expect("block request capture path");
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("blocked request capture must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("failed to persist Taikai anchor request payload"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(&request_capture.display().to_string()),
        "error should identify blocked request capture path: {err}"
    );
}
#[tokio::test]
async fn taikai_anchor_collection_rejects_mismatched_request_capture() {
    let AnchorFixture {
        _dir,
        spool_dir,
        base_id,
    } = minimal_anchor_fixture(ANCHOR_BASE_ID).await;
    let request_capture = spool_dir.join(format!(
        "{TAIKAI_ANCHOR_REQUEST_PREFIX}{base_id}{TAIKAI_ANCHOR_REQUEST_SUFFIX}"
    ));
    async_fs::write(&request_capture, b"stale-different-body")
        .await
        .expect("write stale request capture");
    let err = match collect_pending_uploads(&spool_dir).await {
        Ok(_) => panic!("mismatched request capture must reject anchor collection"),
        Err(err) => err,
    };
    assert!(
        err.contains("different contents"),
        "unexpected anchor collection error: {err}"
    );
    assert!(
        err.contains(&request_capture.display().to_string()),
        "error should identify mismatched request capture path: {err}"
    );
}
#[test]
fn taikai_trm_lineage_guard_requires_zero_origin() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let mut manifest = sample_trm_manifest();
    let alias = manifest.alias_binding.clone();
    let digest = trm_digest_hex(0xA9);
    let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
        .expect("guard")
        .expect("enabled");
    for start_sequence in [1, u64::MAX - 1] {
        manifest.segment_window = TaikaiSegmentWindow::new(start_sequence, start_sequence);
        manifest.renditions[0].ssm_range = manifest.segment_window;
        manifest
            .validate()
            .expect("nonzero window remains structurally valid before lineage admission");
        let err = guard
            .validate(&manifest, &digest)
            .expect_err("fresh alias lineage must reject a nonzero window origin");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(
            err.1.contains("must start at sequence 0"),
            "unexpected origin error: {err:?}"
        );
        let err = guard
            .commit(manifest.segment_window, &digest)
            .expect_err("the authoritative commit must independently reject a nonzero origin");
        assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            err.1.contains("must start at sequence 0"),
            "unexpected commit-origin error: {err:?}"
        );
    }

    manifest.segment_window = TaikaiSegmentWindow::new(0, 15);
    manifest.renditions[0].ssm_range = manifest.segment_window;
    guard
        .validate(&manifest, &digest)
        .expect("zero-origin alias lineage must remain valid");
    guard
        .commit(manifest.segment_window, &digest)
        .expect("zero-origin authoritative lineage must remain valid");
}
#[test]
fn taikai_trm_lineage_guard_rejects_overlapping_windows() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let mut manifest = sample_trm_manifest();
    manifest.segment_window = TaikaiSegmentWindow::new(0, 15);
    let alias = manifest.alias_binding.clone();
    let first_digest = trm_digest_hex(0xAA);
    {
        let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        guard.validate(&manifest, &first_digest).expect("valid");
        guard
            .commit(manifest.segment_window, &first_digest)
            .expect("commit");
    }
    let mut overlap = manifest.clone();
    overlap.segment_window = TaikaiSegmentWindow::new(10, 20);
    let overlap_digest = trm_digest_hex(0xBB);
    let guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
        .expect("guard")
        .expect("enabled");
    guard
        .validate(&overlap, &overlap_digest)
        .expect_err("must reject overlapping manifest windows");
}
#[test]
fn taikai_trm_lineage_guard_requires_exact_contiguous_successor() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let mut manifest = sample_trm_manifest();
    manifest.segment_window = TaikaiSegmentWindow::new(0, 8);
    manifest.renditions[0].ssm_range = manifest.segment_window;
    manifest.validate().expect("valid root manifest");
    let alias = manifest.alias_binding.clone();
    let first_digest = trm_digest_hex(0xAC);
    {
        let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        guard
            .validate(&manifest, &first_digest)
            .expect("valid root");
        guard
            .commit(manifest.segment_window, &first_digest)
            .expect("commit root");
    }

    let mut successor = manifest.clone();
    successor.segment_window = TaikaiSegmentWindow::new(10, 16);
    successor.renditions[0].ssm_range = successor.segment_window;
    successor
        .validate()
        .expect("structurally valid gap manifest");
    let successor_digest = trm_digest_hex(0xAD);
    let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
        .expect("successor guard")
        .expect("enabled");
    let err = guard
        .validate(&successor, &successor_digest)
        .expect_err("a skipped routing window must be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("expected start 9"),
        "unexpected gap error: {err:?}"
    );
    let err = guard
        .commit(successor.segment_window, &successor_digest)
        .expect_err("the authoritative commit must independently reject a skipped window");
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        err.1.contains("expected contiguous successor 9"),
        "unexpected commit-gap error: {err:?}"
    );

    successor.segment_window = TaikaiSegmentWindow::new(9, 16);
    successor.renditions[0].ssm_range = successor.segment_window;
    successor
        .validate()
        .expect("structurally valid contiguous manifest");
    guard
        .validate(&successor, &successor_digest)
        .expect("the exact contiguous routing window must remain valid");
    guard
        .commit(successor.segment_window, &successor_digest)
        .expect("the exact contiguous authoritative window must remain valid");
}
struct StagedTaikaiLineageFixture {
    _dir: tempfile::TempDir,
    spool_dir: PathBuf,
    receipt_log: DaReceiptLog,
    manifest: TaikaiRoutingManifestV1,
    trm_bytes: Vec<u8>,
    trm_path: PathBuf,
    digest: String,
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    receipt: DaIngestReceipt,
    fingerprint: ReplayFingerprint,
}
impl StagedTaikaiLineageFixture {
    fn append_receipt(&self) {
        assert!(matches!(
            self.receipt_log
                .append(
                    LaneEpoch::new(self.lane_id, self.epoch),
                    self.sequence,
                    self.receipt.clone(),
                    self.fingerprint,
                )
                .expect("append exact durable receipt"),
            ReceiptInsertOutcome::Stored { .. }
        ));
    }
}
fn staged_taikai_lineage_fixture(seed: u8) -> StagedTaikaiLineageFixture {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path().join("spool");
    let cursor_store =
        Arc::new(ReplayCursorStore::empty(dir.path().join("cursors")).expect("cursor store"));
    let signer = checked_fixture_ed25519_keypair(seed);
    let receipt_log = open_receipt_log(&dir.path().join("receipts"), &cursor_store, &signer)
        .expect("receipt log");
    let mut manifest = sample_trm_manifest();
    manifest.segment_window = TaikaiSegmentWindow::new(0, 15);
    let alias = manifest.alias_binding.clone();
    let trm_bytes = to_bytes(&manifest).expect("encode routing manifest");
    let digest = hex::encode(blake3_hash(&trm_bytes).as_bytes());
    let lane_id = LaneId::new(7);
    let epoch = 42;
    let sequence = 0;
    let receipt = test_receipt(&signer, lane_id, epoch, sequence, seed);
    let fingerprint = receipt_fingerprint(&receipt);
    let trm_path = {
        let mut guard = taikai_ingest::TrmLineageGuard::new(&spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        guard.validate(&manifest, &digest).expect("fresh lineage");
        guard
            .stage_ingest(
                manifest.segment_window.clone(),
                &digest,
                lane_id,
                epoch,
                sequence,
                &receipt.storage_ticket,
                &fingerprint,
            )
            .expect("stage pending lineage");
        taikai_ingest::persist_trm(
            &spool_dir,
            lane_id,
            epoch,
            sequence,
            &receipt.storage_ticket,
            &fingerprint,
            &trm_bytes,
        )
        .expect("persist routing manifest")
        .expect("enabled routing manifest path")
    };
    StagedTaikaiLineageFixture {
        _dir: dir,
        spool_dir,
        receipt_log,
        manifest,
        trm_bytes,
        trm_path,
        digest,
        lane_id,
        epoch,
        sequence,
        receipt,
        fingerprint,
    }
}
#[test]
fn taikai_pending_lineage_without_durable_receipt_is_discarded() {
    let fixture = staged_taikai_lineage_fixture(0x66);
    assert!(
        taikai_lineage_artifact_path(
            &fixture.spool_dir,
            TAIKAI_TRM_PENDING_PREFIX,
            TAIKAI_TRM_PENDING_SUFFIX,
        )
        .is_some(),
        "staging should create a pending lineage record"
    );

    taikai_ingest::recover_pending_lineages(&fixture.spool_dir, &fixture.receipt_log)
        .expect("receipt-less pending lineage should be discarded");

    assert!(
        taikai_lineage_artifact_path(
            &fixture.spool_dir,
            TAIKAI_TRM_PENDING_PREFIX,
            TAIKAI_TRM_PENDING_SUFFIX,
        )
        .is_none(),
        "receipt-less recovery must remove the pending lineage record"
    );
    assert!(
        taikai_lineage_artifact_path(
            &fixture.spool_dir,
            TAIKAI_TRM_LINEAGE_PREFIX,
            TAIKAI_TRM_LINEAGE_SUFFIX,
        )
        .is_none(),
        "receipt-less recovery must not advance authoritative lineage"
    );
    let guard =
        taikai_ingest::TrmLineageGuard::new(&fixture.spool_dir, &fixture.manifest.alias_binding)
            .expect("recovered guard")
            .expect("enabled");
    assert_eq!(
        guard
            .validate_ingest_retry(
                &fixture.manifest,
                &fixture.digest,
                fixture.lane_id,
                fixture.epoch,
                fixture.sequence,
                &fixture.receipt.storage_ticket,
                &fixture.fingerprint,
                &fixture.trm_bytes,
            )
            .expect("discarded pending lineage must remain fresh"),
        taikai_ingest::TrmLineageValidation::Fresh
    );
}
#[test]
fn taikai_trm_lineage_guard_allows_exact_staged_ingest_retry() {
    let fixture = staged_taikai_lineage_fixture(0x67);
    fixture.append_receipt();

    taikai_ingest::recover_pending_lineages(&fixture.spool_dir, &fixture.receipt_log)
        .expect("promote receipt-backed pending lineage");

    assert!(
        taikai_lineage_artifact_path(
            &fixture.spool_dir,
            TAIKAI_TRM_PENDING_PREFIX,
            TAIKAI_TRM_PENDING_SUFFIX,
        )
        .is_none(),
        "successful recovery must remove the pending lineage record"
    );
    assert!(
        taikai_lineage_artifact_path(
            &fixture.spool_dir,
            TAIKAI_TRM_LINEAGE_PREFIX,
            TAIKAI_TRM_LINEAGE_SUFFIX,
        )
        .is_some(),
        "exact receipt and TRM must promote authoritative lineage"
    );
    let guard =
        taikai_ingest::TrmLineageGuard::new(&fixture.spool_dir, &fixture.manifest.alias_binding)
            .expect("retry guard")
            .expect("enabled");
    let validation = guard
        .validate_ingest_retry(
            &fixture.manifest,
            &fixture.digest,
            fixture.lane_id,
            fixture.epoch,
            fixture.sequence,
            &fixture.receipt.storage_ticket,
            &fixture.fingerprint,
            &fixture.trm_bytes,
        )
        .expect("exact staged retry must be admitted");
    assert_eq!(
        validation,
        taikai_ingest::TrmLineageValidation::ExactArtifactRetry
    );
    assert!(
        taikai_ingest::TrmLineageValidation::Fresh.records_alias_rotation(),
        "fresh lineage must emit one alias-rotation event"
    );
    assert!(
        !validation.records_alias_rotation(),
        "an exact retry must not emit a duplicate alias-rotation event"
    );
    let err = guard
        .validate_ingest_retry(
            &fixture.manifest,
            &fixture.digest,
            fixture.lane_id,
            fixture.epoch,
            fixture.sequence + 1,
            &fixture.receipt.storage_ticket,
            &fixture.fingerprint,
            &fixture.trm_bytes,
        )
        .expect_err("a receipt-backed lineage must admit only the exact retry coordinates");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("already accepted"),
        "unexpected inexact retry error: {:?}",
        err
    );
}
#[test]
fn taikai_pending_lineage_with_tampered_trm_is_not_promoted() {
    let fixture = staged_taikai_lineage_fixture(0x69);
    fixture.append_receipt();
    fs::write(&fixture.trm_path, b"tampered routing manifest")
        .expect("tamper staged routing manifest");

    let err = taikai_ingest::recover_pending_lineages(&fixture.spool_dir, &fixture.receipt_log)
        .expect_err("tampered staged TRM must not promote receipt-backed lineage");
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        err.1.contains("digest mismatch"),
        "unexpected tampered-TRM recovery error: {:?}",
        err
    );
    assert!(
        taikai_lineage_artifact_path(
            &fixture.spool_dir,
            TAIKAI_TRM_LINEAGE_PREFIX,
            TAIKAI_TRM_LINEAGE_SUFFIX,
        )
        .is_none(),
        "tampered staged TRM must not create authoritative lineage"
    );
    assert!(
        taikai_lineage_artifact_path(
            &fixture.spool_dir,
            TAIKAI_TRM_PENDING_PREFIX,
            TAIKAI_TRM_PENDING_SUFFIX,
        )
        .is_some(),
        "failed recovery should retain pending state for operator inspection"
    );
}
#[test]
fn taikai_pending_lineage_recovery_rejects_nonzero_origin() {
    let fixture = staged_taikai_lineage_fixture(0x6A);
    let pending_path = taikai_lineage_artifact_path(
        &fixture.spool_dir,
        TAIKAI_TRM_PENDING_PREFIX,
        TAIKAI_TRM_PENDING_SUFFIX,
    )
    .expect("pending lineage path");
    let mut pending: Value =
        json::from_slice(&fs::read(&pending_path).expect("read pending lineage record"))
            .expect("decode pending lineage record");
    pending
        .as_object_mut()
        .expect("pending lineage object")
        .insert("window_start_sequence".into(), Value::from(1_u64));
    fs::write(
        &pending_path,
        json::to_string(&pending)
            .expect("encode malformed pending lineage")
            .as_bytes(),
    )
    .expect("write malformed pending lineage");

    let err = taikai_ingest::recover_pending_lineages(&fixture.spool_dir, &fixture.receipt_log)
        .expect_err("a nonzero pending root must not become authoritative");
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        err.1.contains("must start at sequence 0"),
        "unexpected pending-root error: {err:?}"
    );
    assert!(
        taikai_lineage_artifact_path(
            &fixture.spool_dir,
            TAIKAI_TRM_LINEAGE_PREFIX,
            TAIKAI_TRM_LINEAGE_SUFFIX,
        )
        .is_none(),
        "invalid pending root must not create authoritative lineage"
    );
}
#[test]
fn taikai_pending_lineage_recovery_rejects_window_gap() {
    let fixture = staged_taikai_lineage_fixture(0x6B);
    fixture.append_receipt();
    taikai_ingest::recover_pending_lineages(&fixture.spool_dir, &fixture.receipt_log)
        .expect("promote the root lineage");

    let next_digest = trm_digest_hex(0xBC);
    {
        let mut guard = taikai_ingest::TrmLineageGuard::new(
            &fixture.spool_dir,
            &fixture.manifest.alias_binding,
        )
        .expect("successor guard")
        .expect("enabled");
        guard
            .stage_ingest(
                TaikaiSegmentWindow::new(16, 23),
                &next_digest,
                fixture.lane_id,
                fixture.epoch,
                1,
                &fixture.receipt.storage_ticket,
                &fixture.fingerprint,
            )
            .expect("stage exact successor before tampering it into a gap");
    }
    let pending_path = taikai_lineage_artifact_path(
        &fixture.spool_dir,
        TAIKAI_TRM_PENDING_PREFIX,
        TAIKAI_TRM_PENDING_SUFFIX,
    )
    .expect("pending lineage path");
    let mut pending: Value =
        json::from_slice(&fs::read(&pending_path).expect("read pending lineage record"))
            .expect("decode pending lineage record");
    pending
        .as_object_mut()
        .expect("pending lineage object")
        .insert("window_start_sequence".into(), Value::from(17_u64));
    fs::write(
        &pending_path,
        json::to_string(&pending)
            .expect("encode gapped pending lineage")
            .as_bytes(),
    )
    .expect("write gapped pending lineage");

    let err = taikai_ingest::recover_pending_lineages(&fixture.spool_dir, &fixture.receipt_log)
        .expect_err("a gapped pending successor must not become authoritative");
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        err.1.contains("expected contiguous successor 16"),
        "unexpected pending-gap error: {err:?}"
    );
    let lineage_path = taikai_lineage_artifact_path(
        &fixture.spool_dir,
        TAIKAI_TRM_LINEAGE_PREFIX,
        TAIKAI_TRM_LINEAGE_SUFFIX,
    )
    .expect("root lineage must remain authoritative");
    let lineage: Value =
        json::from_slice(&fs::read(lineage_path).expect("read authoritative lineage"))
            .expect("decode authoritative lineage");
    assert_eq!(
        lineage.get("window_end_sequence").and_then(Value::as_u64),
        Some(15),
        "failed gap recovery must not advance the authoritative head"
    );
}
#[test]
fn taikai_pending_lineage_recovery_rejects_terminal_and_oversized_windows() {
    for (label, end_sequence) in [
        ("terminal", u64::MAX),
        (
            "oversized",
            iroha_data_model::taikai::TAIKAI_SEGMENT_WINDOW_MAX_SEQUENCES_V1,
        ),
    ] {
        let fixture = staged_taikai_lineage_fixture(0x68);
        fixture.append_receipt();
        let pending_path = taikai_lineage_artifact_path(
            &fixture.spool_dir,
            TAIKAI_TRM_PENDING_PREFIX,
            TAIKAI_TRM_PENDING_SUFFIX,
        )
        .expect("pending lineage path");
        let mut pending: Value =
            json::from_slice(&fs::read(&pending_path).expect("read pending lineage record"))
                .expect("decode pending lineage record");
        pending
            .as_object_mut()
            .expect("pending lineage object")
            .insert("window_end_sequence".into(), Value::from(end_sequence));
        fs::write(
            &pending_path,
            json::to_string(&pending)
                .expect("encode malformed pending lineage")
                .as_bytes(),
        )
        .expect("write malformed pending lineage");

        let err = taikai_ingest::recover_pending_lineages(&fixture.spool_dir, &fixture.receipt_log)
            .expect_err("invalid pending window must not be promoted");
        assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR, "case: {label}");
        assert!(
            err.1.contains("invalid segment window"),
            "unexpected {label} pending-lineage error: {:?}",
            err
        );
        assert!(
            taikai_lineage_artifact_path(
                &fixture.spool_dir,
                TAIKAI_TRM_LINEAGE_PREFIX,
                TAIKAI_TRM_LINEAGE_SUFFIX,
            )
            .is_none(),
            "{label} pending state must not advance authoritative lineage"
        );
    }
}
#[test]
fn taikai_trm_lineage_guard_rejects_retry_from_legacy_lineage_without_provenance() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let mut manifest = sample_trm_manifest();
    manifest.segment_window = TaikaiSegmentWindow::new(0, 15);
    let alias = manifest.alias_binding.clone();
    let trm_bytes = to_bytes(&manifest).expect("encode routing manifest");
    let digest = hex::encode(blake3_hash(&trm_bytes).as_bytes());
    let lane_id = LaneId::new(7);
    let epoch = 42;
    let sequence = 9;
    let storage_ticket = StorageTicketId::new([0xA5; 32]);
    let fingerprint = ReplayFingerprint::from_hash(blake3_hash(b"legacy-lineage-retry"));
    {
        let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        taikai_ingest::persist_envelope(
            spool_dir,
            lane_id,
            epoch,
            sequence,
            &storage_ticket,
            &fingerprint,
            b"envelope",
        )
        .expect("persist envelope")
        .expect("enabled envelope path");
        taikai_ingest::persist_trm(
            spool_dir,
            lane_id,
            epoch,
            sequence,
            &storage_ticket,
            &fingerprint,
            &trm_bytes,
        )
        .expect("persist routing manifest")
        .expect("enabled routing manifest path");
        guard
            .commit(manifest.segment_window.clone(), &digest)
            .expect("seed legacy lineage without artifact provenance");
    }
    let guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
        .expect("retry guard")
        .expect("enabled");
    let err = guard
        .validate_ingest_retry(
            &manifest,
            &digest,
            lane_id,
            epoch,
            sequence,
            &storage_ticket,
            &fingerprint,
            &trm_bytes,
        )
        .expect_err("legacy lineage without provenance must not authenticate a retry");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("already accepted"),
        "unexpected legacy-lineage retry error: {:?}",
        err
    );
}
#[test]
fn taikai_trm_lineage_guard_rejects_staged_retry_at_different_coordinates() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let mut manifest = sample_trm_manifest();
    manifest.segment_window = TaikaiSegmentWindow::new(0, 15);
    let alias = manifest.alias_binding.clone();
    let trm_bytes = to_bytes(&manifest).expect("encode routing manifest");
    let digest = hex::encode(blake3_hash(&trm_bytes).as_bytes());
    let lane_id = LaneId::new(7);
    let epoch = 42;
    let sequence = 9;
    let storage_ticket = StorageTicketId::new([0xA5; 32]);
    let fingerprint = ReplayFingerprint::from_hash(blake3_hash(b"bound-lineage-retry"));
    {
        let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        taikai_ingest::persist_envelope(
            spool_dir,
            lane_id,
            epoch,
            sequence,
            &storage_ticket,
            &fingerprint,
            b"envelope",
        )
        .expect("persist envelope")
        .expect("enabled envelope path");
        taikai_ingest::persist_trm(
            spool_dir,
            lane_id,
            epoch,
            sequence,
            &storage_ticket,
            &fingerprint,
            &trm_bytes,
        )
        .expect("persist routing manifest")
        .expect("enabled routing manifest path");
        taikai_ingest::persist_envelope(
            spool_dir,
            lane_id,
            epoch,
            sequence + 1,
            &storage_ticket,
            &fingerprint,
            b"other-envelope",
        )
        .expect("persist other partial envelope")
        .expect("enabled other envelope path");
        taikai_ingest::persist_trm(
            spool_dir,
            lane_id,
            epoch,
            sequence + 1,
            &storage_ticket,
            &fingerprint,
            &trm_bytes,
        )
        .expect("persist other partial routing manifest")
        .expect("enabled other routing manifest path");
        guard
            .commit_ingest(
                manifest.segment_window.clone(),
                &digest,
                lane_id,
                epoch,
                sequence,
                &storage_ticket,
                &fingerprint,
            )
            .expect("commit lineage");
    }
    let other_ticket = StorageTicketId::new([0x5A; 32]);
    let other_fingerprint = ReplayFingerprint::from_hash(blake3_hash(b"other-lineage-retry"));
    let candidates = [
        (
            "lane",
            LaneId::new(8),
            epoch,
            sequence,
            storage_ticket.clone(),
            fingerprint,
        ),
        (
            "epoch",
            lane_id,
            epoch + 1,
            sequence,
            storage_ticket.clone(),
            fingerprint,
        ),
        (
            "sequence",
            lane_id,
            epoch,
            sequence + 1,
            storage_ticket.clone(),
            fingerprint,
        ),
        (
            "storage ticket",
            lane_id,
            epoch,
            sequence,
            other_ticket,
            fingerprint,
        ),
        (
            "fingerprint",
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            other_fingerprint,
        ),
    ];
    for (
        label,
        candidate_lane,
        candidate_epoch,
        candidate_sequence,
        candidate_ticket,
        candidate_fingerprint,
    ) in candidates
    {
        let guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("retry guard")
            .expect("enabled");
        let err = guard
            .validate_ingest_retry(
                &manifest,
                &digest,
                candidate_lane,
                candidate_epoch,
                candidate_sequence,
                &candidate_ticket,
                &candidate_fingerprint,
                &trm_bytes,
            )
            .expect_err("different replay coordinates must reject");
        assert_eq!(err.0, StatusCode::BAD_REQUEST, "coordinate: {label}");
        assert!(
            err.1.contains("already accepted"),
            "unexpected {label} retry error: {:?}",
            err
        );
    }
}
fn trm_digest_hex(byte: u8) -> String {
    hex::encode([byte; 32])
}
fn taikai_lineage_artifact_path(spool_dir: &Path, prefix: &str, suffix: &str) -> Option<PathBuf> {
    fs::read_dir(spool_dir.join(TAIKAI_SPOOL_SUBDIR))
        .expect("read taikai spool")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix) && name.ends_with(suffix))
        })
}
fn taikai_lineage_state_path(spool_dir: &Path) -> PathBuf {
    taikai_lineage_artifact_path(
        spool_dir,
        TAIKAI_TRM_LINEAGE_PREFIX,
        TAIKAI_TRM_LINEAGE_SUFFIX,
    )
    .expect("lineage state path")
}
fn taikai_lock_path(spool_dir: &Path) -> PathBuf {
    fs::read_dir(spool_dir.join(TAIKAI_SPOOL_SUBDIR))
        .expect("read taikai spool")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with(TAIKAI_TRM_LOCK_PREFIX)
                        && name.ends_with(TAIKAI_TRM_LOCK_SUFFIX)
                })
        })
        .expect("Taikai TRM lock path")
}
fn mutate_taikai_lineage_state(spool_dir: &Path, mutate: impl FnOnce(&mut Value)) {
    let path = taikai_lineage_state_path(spool_dir);
    let contents = fs::read_to_string(&path).expect("read lineage state");
    let mut value: Value = json::from_str(&contents).expect("decode lineage state");
    mutate(&mut value);
    fs::write(
        &path,
        json::to_string(&value).expect("encode mutated lineage state"),
    )
    .expect("write mutated lineage state");
}
fn assert_mutated_lineage_state_rejected(mutate: impl FnOnce(&mut Value), expected_message: &str) {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let mut manifest = sample_trm_manifest();
    manifest.segment_window = TaikaiSegmentWindow::new(0, 8);
    let alias = manifest.alias_binding.clone();
    let digest = trm_digest_hex(0xAB);
    let storage_ticket = StorageTicketId::new([0xA5; 32]);
    let fingerprint = ReplayFingerprint::from_hash(blake3_hash(b"mutated-lineage-state"));
    {
        let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        guard.validate(&manifest, &digest).expect("valid");
        guard
            .commit_ingest(
                manifest.segment_window,
                &digest,
                LaneId::new(7),
                42,
                9,
                &storage_ticket,
                &fingerprint,
            )
            .expect("commit");
    }
    mutate_taikai_lineage_state(spool_dir, mutate);
    let err = match taikai_ingest::TrmLineageGuard::new(spool_dir, &alias) {
        Ok(_) => panic!("mutated lineage state should be rejected"),
        Err(err) => err,
    };
    assert!(
        err.1.contains(expected_message),
        "unexpected error: {:?}",
        err
    );
}
#[test]
fn taikai_trm_lineage_guard_rejects_unsupported_state_version() {
    assert_mutated_lineage_state_rejected(
        |value| {
            value
                .as_object_mut()
                .expect("lineage object")
                .insert("version".into(), Value::from(2));
        },
        "unsupported Taikai routing manifest lineage record version",
    );
}
#[test]
fn taikai_trm_lineage_guard_rejects_alias_mismatch() {
    assert_mutated_lineage_state_rejected(
        |value| {
            value
                .as_object_mut()
                .expect("lineage object")
                .insert("alias_name".into(), Value::from("other-alias"));
        },
        "belongs to alias",
    );
}
#[test]
fn taikai_trm_lineage_guard_rejects_invalid_manifest_digest() {
    assert_mutated_lineage_state_rejected(
        |value| {
            value
                .as_object_mut()
                .expect("lineage object")
                .insert("manifest_digest_hex".into(), Value::from("deadbeef"));
        },
        "manifest_digest_hex must be 32-byte lowercase hex",
    );
}
#[test]
fn taikai_trm_lineage_guard_rejects_noncanonical_uppercase_manifest_digest() {
    assert_mutated_lineage_state_rejected(
        |value| {
            value
                .as_object_mut()
                .expect("lineage object")
                .insert("manifest_digest_hex".into(), Value::from("AB".repeat(32)));
        },
        "manifest_digest_hex must be 32-byte lowercase hex",
    );
}
#[test]
fn taikai_trm_lineage_guard_rejects_malformed_artifact_base_id() {
    assert_mutated_lineage_state_rejected(
        |value| {
            value
                .as_object_mut()
                .expect("lineage object")
                .insert("artifact_base_id".into(), Value::from("not-canonical"));
        },
        "artifact_base_id must be canonical lowercase hex",
    );
}
#[test]
fn taikai_trm_lineage_guard_rejects_inverted_window() {
    assert_mutated_lineage_state_rejected(
        |value| {
            let map = value.as_object_mut().expect("lineage object");
            map.insert("window_start_sequence".into(), Value::from(20));
            map.insert("window_end_sequence".into(), Value::from(10));
        },
        "window_start_sequence exceeds window_end_sequence",
    );
}
#[cfg(unix)]
#[test]
fn taikai_trm_lineage_guard_rejects_state_symlink() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let mut manifest = sample_trm_manifest();
    manifest.segment_window = TaikaiSegmentWindow::new(0, 8);
    let alias = manifest.alias_binding.clone();
    let digest = trm_digest_hex(0xAB);
    {
        let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        guard.validate(&manifest, &digest).expect("valid");
        guard
            .commit(manifest.segment_window, &digest)
            .expect("commit");
    }
    let state_path = taikai_lineage_state_path(spool_dir);
    let state_target = spool_dir
        .join(TAIKAI_SPOOL_SUBDIR)
        .join("lineage-state-target.json");
    fs::write(
        &state_target,
        fs::read(&state_path).expect("read lineage state"),
    )
    .expect("write lineage symlink target");
    fs::remove_file(&state_path).expect("remove lineage state");
    symlink(&state_target, &state_path).expect("create lineage state symlink");
    let err = match taikai_ingest::TrmLineageGuard::new(spool_dir, &alias) {
        Ok(_) => panic!("symlinked lineage state must reject guard acquisition"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        err.1
            .contains("Taikai routing manifest lineage record is not a regular file"),
        "unexpected lineage symlink error: {:?}",
        err
    );
    assert!(
        fs::symlink_metadata(&state_path)
            .expect("inspect lineage symlink")
            .file_type()
            .is_symlink(),
        "failed validation should leave lineage symlink visible"
    );
    assert!(
        state_target.exists(),
        "lineage symlink target should not be removed"
    );
}
#[cfg(unix)]
#[test]
fn taikai_trm_lineage_guard_rejects_spool_dir_symlink() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let target = spool_dir.join("taikai-lineage-target");
    fs::create_dir(&target).expect("create Taikai lineage target");
    let base_dir = spool_dir.join(TAIKAI_SPOOL_SUBDIR);
    symlink(&target, &base_dir).expect("create Taikai lineage spool symlink");
    let alias = sample_trm_manifest().alias_binding;
    let err = match taikai_ingest::TrmLineageGuard::new(spool_dir, &alias) {
        Ok(_) => panic!("symlinked Taikai lineage root must reject guard acquisition"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        err.1.contains("Taikai spool directory"),
        "unexpected lineage root symlink error: {:?}",
        err
    );
    assert!(
        fs::symlink_metadata(&base_dir)
            .expect("inspect Taikai lineage symlink")
            .file_type()
            .is_symlink(),
        "failed validation should leave lineage root symlink visible"
    );
    assert_eq!(
        fs::read_dir(&target)
            .expect("read lineage target directory")
            .count(),
        0,
        "symlink target must not receive lineage locks or records"
    );
}
#[test]
fn taikai_trm_lineage_guard_rejects_busy_live_lock() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let alias = sample_trm_manifest().alias_binding;
    let guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
        .expect("guard")
        .expect("enabled");
    let err = match taikai_ingest::TrmLineageGuard::new(spool_dir, &alias) {
        Ok(_) => panic!("busy live lock must reject lineage guard acquisition"),
        Err(err) => err,
    };
    drop(guard);
    assert_eq!(err.0, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        err.1.contains("routing manifest lock busy for alias slug"),
        "unexpected busy lock error: {:?}",
        err
    );
}
#[cfg(unix)]
#[test]
fn taikai_trm_lineage_guard_rejects_lock_symlink() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let alias = sample_trm_manifest().alias_binding;
    let guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
        .expect("guard")
        .expect("enabled");
    let lock_path = taikai_lock_path(spool_dir);
    drop(guard);
    fs::remove_file(&lock_path).expect("remove persistent lock before symlink test");
    let lock_target = spool_dir
        .join(TAIKAI_SPOOL_SUBDIR)
        .join("lineage-lock-target.lock");
    fs::write(&lock_target, b"0\n").expect("write lock symlink target");
    symlink(&lock_target, &lock_path).expect("create lock symlink");
    let err = match taikai_ingest::TrmLineageGuard::new(spool_dir, &alias) {
        Ok(_) => panic!("symlinked lock must reject lineage guard acquisition"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        err.1
            .contains("Taikai routing manifest lock is not a regular file"),
        "unexpected lock symlink error: {:?}",
        err
    );
    assert!(
        fs::symlink_metadata(&lock_path)
            .expect("inspect lock symlink")
            .file_type()
            .is_symlink(),
        "failed validation should leave lock symlink visible"
    );
    assert!(
        lock_target.exists(),
        "lock symlink target should not be removed"
    );
}
#[cfg(unix)]
#[test]
fn taikai_trm_lineage_guard_never_steals_an_aged_live_lock() {
    use std::time::SystemTime;
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let alias = sample_trm_manifest().alias_binding;
    let guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
        .expect("guard")
        .expect("enabled");
    let lock_path = taikai_lock_path(spool_dir);
    let stale_at = SystemTime::now() - Duration::from_secs(24 * 60 * 60);
    let stale_times = std::fs::FileTimes::new()
        .set_accessed(stale_at)
        .set_modified(stale_at);
    fs::File::options()
        .read(true)
        .open(&lock_path)
        .expect("open stale lock")
        .set_times(stale_times)
        .expect("age live lock");
    let err = match taikai_ingest::TrmLineageGuard::new(spool_dir, &alias) {
        Ok(_) => panic!("an aged live lock must not be stolen"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        err.1.contains("routing manifest lock busy for alias slug"),
        "unexpected busy lock error: {:?}",
        err
    );
    drop(guard);
    let recovered = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
        .expect("released persistent lock must be reusable")
        .expect("enabled");
    drop(recovered);
}
#[test]
fn taikai_trm_lineage_hint_contains_previous_digest() {
    let dir = tempdir().expect("tempdir");
    let spool_dir = dir.path();
    let mut manifest = sample_trm_manifest();
    manifest.segment_window = TaikaiSegmentWindow::new(0, 8);
    let alias = manifest.alias_binding.clone();
    let lane_id = LaneId::new(7);
    let epoch = 42;
    let storage_ticket = StorageTicketId::new([0xAA; 32]);
    let fingerprint = ReplayFingerprint::from_hash(blake3_hash(b"lineage-hint"));
    let first_digest = trm_digest_hex(0xA1);
    {
        let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        guard.validate(&manifest, &first_digest).expect("valid");
        guard
            .persist_lineage_hint(lane_id, epoch, 1, &storage_ticket, &fingerprint)
            .expect("persist hint");
        let base_id = format_base_id(lane_id, epoch, 1, &storage_ticket, &fingerprint);
        let hint_path = spool_dir
            .join(TAIKAI_SPOOL_SUBDIR)
            .join(format!("taikai-lineage-{base_id}.json"));
        let contents = fs::read_to_string(&hint_path).expect("lineage hint contents");
        let value: Value = json::from_str(&contents).expect("lineage value");
        assert!(
            value
                .get("previous_manifest_digest_hex")
                .is_some_and(Value::is_null)
        );
        guard
            .commit(manifest.segment_window, &first_digest)
            .expect("commit");
    }
    let mut next_manifest = manifest.clone();
    next_manifest.segment_window = TaikaiSegmentWindow::new(9, 16);
    let next_digest = trm_digest_hex(0xB2);
    {
        let mut guard = taikai_ingest::TrmLineageGuard::new(spool_dir, &alias)
            .expect("guard")
            .expect("enabled");
        guard.validate(&next_manifest, &next_digest).expect("valid");
        guard
            .persist_lineage_hint(lane_id, epoch, 2, &storage_ticket, &fingerprint)
            .expect("persist hint");
        let base_id = format_base_id(lane_id, epoch, 2, &storage_ticket, &fingerprint);
        let hint_path = spool_dir
            .join(TAIKAI_SPOOL_SUBDIR)
            .join(format!("taikai-lineage-{base_id}.json"));
        let contents = fs::read_to_string(&hint_path).expect("lineage hint contents");
        let value: Value = json::from_str(&contents).expect("lineage value");
        assert_eq!(
            value
                .get("previous_manifest_digest_hex")
                .and_then(Value::as_str),
            Some(first_digest.as_str())
        );
        guard
            .commit(next_manifest.segment_window, &next_digest)
            .expect("commit");
    }
}
#[test]
fn take_ssm_entry_returns_payload_and_strips_metadata() {
    let mut metadata = taikai_metadata();
    metadata.items.push(MetadataEntry::new(
        taikai::META_TAIKAI_SSM,
        vec![1, 2, 3],
        MetadataVisibility::Public,
    ));
    let payload = taikai_ingest::take_ssm_entry(&mut metadata)
        .expect("extract ssm")
        .expect("payload present");
    assert_eq!(payload, vec![1, 2, 3]);
    assert!(
        metadata
            .items
            .iter()
            .all(|entry| entry.key != taikai::META_TAIKAI_SSM)
    );
}
#[test]
fn take_ssm_entry_rejects_duplicate_payloads_without_mutating_metadata() {
    let mut metadata = taikai_metadata();
    metadata.items.extend([
        MetadataEntry::new(
            taikai::META_TAIKAI_SSM,
            vec![1, 2, 3],
            MetadataVisibility::Public,
        ),
        MetadataEntry::new(
            taikai::META_TAIKAI_SSM,
            vec![4, 5, 6],
            MetadataVisibility::Public,
        ),
    ]);
    let err = taikai_ingest::take_ssm_entry(&mut metadata)
        .expect_err("duplicate signing manifests must reject");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("metadata entry must appear at most once"));
    assert_eq!(
        metadata
            .items
            .iter()
            .filter(|entry| entry.key == taikai::META_TAIKAI_SSM)
            .count(),
        2,
        "rejected extraction must leave the signed request unchanged"
    );
}
#[test]
fn take_trm_entry_returns_payload_and_strips_metadata() {
    let mut metadata = taikai_metadata();
    metadata.items.push(MetadataEntry::new(
        taikai::META_TAIKAI_TRM,
        vec![9, 8, 7],
        MetadataVisibility::Public,
    ));
    let payload = taikai_ingest::take_trm_entry(&mut metadata)
        .expect("extract trm")
        .expect("payload present");
    assert_eq!(payload, vec![9, 8, 7]);
    assert!(
        metadata
            .items
            .iter()
            .all(|entry| entry.key != taikai::META_TAIKAI_TRM)
    );
}
#[test]
fn take_trm_entry_rejects_duplicate_payloads_without_mutating_metadata() {
    let mut metadata = taikai_metadata();
    metadata.items.extend([
        MetadataEntry::new(
            taikai::META_TAIKAI_TRM,
            vec![9, 8, 7],
            MetadataVisibility::Public,
        ),
        MetadataEntry::new(
            taikai::META_TAIKAI_TRM,
            vec![6, 5, 4],
            MetadataVisibility::Public,
        ),
    ]);
    let err = taikai_ingest::take_trm_entry(&mut metadata)
        .expect_err("duplicate routing manifests must reject");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("metadata entry must appear at most once"));
    assert_eq!(
        metadata
            .items
            .iter()
            .filter(|entry| entry.key == taikai::META_TAIKAI_TRM)
            .count(),
        2,
        "rejected extraction must leave the signed request unchanged"
    );
}
fn taikai_ssm_validation_fixture() -> (ManifestArtifacts, taikai_ingest::EnvelopeArtifacts) {
    let mut request = sample_request();
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1,
        &rent_policy,
    )
    .expect("manifest");
    let envelope = taikai_ingest::build_envelope(
        &request,
        &manifest,
        &chunk_store,
        canonical.as_slice(),
        None,
    )
    .expect("envelope");
    (manifest, envelope)
}
fn taikai_alias_cache_policy() -> crate::sorafs::AliasCachePolicy {
    crate::sorafs::AliasCachePolicy::new(
        Duration::from_secs(600),
        Duration::from_secs(60),
        Duration::from_secs(1_200),
        Duration::from_secs(60),
        Duration::from_secs(120),
        Duration::from_secs(10_000),
        Duration::from_secs(60),
        Duration::from_secs(60),
    )
}

#[test]
fn validate_taikai_ssm_rejects_malformed_norito() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let (_, telemetry) = telemetry_handle_for_tests();
    let err = taikai::validate_taikai_ssm(
        b"not-a-norito-signing-manifest",
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &taikai_alias_cache_policy(),
        Some(&alias_council_policy(&[[0x33; 32]], 1)),
        &telemetry,
    )
    .expect_err("malformed Norito SSM must fail admission");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("failed to decode signing manifest"),
        "unexpected malformed SSM error: {}",
        err.1
    );
}

#[test]
fn validate_taikai_ssm_accepts_matching_payload() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let ssm_bytes = build_ssm_bytes(
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
    );
    let alias_policy = taikai_alias_cache_policy();
    let (_, telemetry) = telemetry_handle_for_tests();
    let outcome = taikai::validate_taikai_ssm(
        &ssm_bytes,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &alias_policy,
        Some(&alias_council_policy(&[[0x33; 32]], 1)),
        &telemetry,
    )
    .expect("ssm valid");
    assert_eq!(outcome.alias_label, "sora/docs");
}

#[test]
fn validate_taikai_publisher_owner_binds_outer_principal() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let ssm_bytes = build_ssm_bytes(
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
    );
    let signing_manifest: TaikaiSegmentSigningManifestV1 =
        norito::decode_from_bytes(&ssm_bytes).expect("decode signing manifest");
    let telemetry = crate::routing::MaybeTelemetry::disabled();
    let outcome = taikai::validate_taikai_ssm(
        &ssm_bytes,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &taikai_alias_cache_policy(),
        Some(&alias_council_policy(&[[0x33; 32]], 1)),
        &telemetry,
    )
    .expect("ssm valid");
    assert_eq!(
        outcome.publisher_account,
        signing_manifest.body.publisher_account
    );
    taikai::validate_taikai_publisher_owner(&outcome, &signing_manifest.body.publisher_account)
        .expect("the authenticated publisher may submit its own segment");
    let relayer = if signing_manifest.body.publisher_account != *ALICE_ID {
        ALICE_ID.clone()
    } else {
        BOB_ID.clone()
    };
    let err = taikai::validate_taikai_publisher_owner(&outcome, &relayer)
        .expect_err("an unrelated DA owner must not submit another publisher's SSM");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("does not match the SSM publisher"));
}

#[test]
fn validate_taikai_ssm_rejects_unsupported_body_version() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let ssm_bytes = build_ssm_bytes_with_alias_council_and_body_mutation(
        manifest.manifest_hash,
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
        Algorithm::Ed25519,
        &[[0x33; 32]],
        |body| body.version = TaikaiSegmentSigningBodyV1::VERSION + 1,
    );
    let (_, telemetry) = telemetry_handle_for_tests();
    let err = taikai::validate_taikai_ssm(
        &ssm_bytes,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &taikai_alias_cache_policy(),
        Some(&alias_council_policy(&[[0x33; 32]], 1)),
        &telemetry,
    )
    .expect_err("unknown SSM body version must fail admission");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("unsupported signing manifest version"));
}

#[test]
fn validate_taikai_ssm_rejects_zero_signed_timestamp() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let ssm_bytes = build_ssm_bytes_with_alias_council_and_body_mutation(
        manifest.manifest_hash,
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
        Algorithm::Ed25519,
        &[[0x33; 32]],
        |body| body.signed_unix_ms = 0,
    );
    let (_, telemetry) = telemetry_handle_for_tests();
    let err = taikai::validate_taikai_ssm(
        &ssm_bytes,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &taikai_alias_cache_policy(),
        Some(&alias_council_policy(&[[0x33; 32]], 1)),
        &telemetry,
    )
    .expect_err("zero SSM production timestamp must fail admission");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1
            .contains("signed_unix_ms must be a non-zero production timestamp"),
        "unexpected zero timestamp error: {}",
        err.1
    );
}

#[test]
fn validate_taikai_ssm_rejects_publisher_account_key_mismatch() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let ssm_bytes = build_ssm_bytes_with_alias_council_and_body_mutation(
        manifest.manifest_hash,
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
        Algorithm::Ed25519,
        &[[0x33; 32]],
        |body| {
            body.publisher_account = if AccountId::new(body.publisher_key.clone()) != *ALICE_ID {
                ALICE_ID.clone()
            } else {
                BOB_ID.clone()
            };
        },
    );
    let (_, telemetry) = telemetry_handle_for_tests();
    let err = taikai::validate_taikai_ssm(
        &ssm_bytes,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &taikai_alias_cache_policy(),
        Some(&alias_council_policy(&[[0x33; 32]], 1)),
        &telemetry,
    )
    .expect_err("a valid signature must not authenticate another publisher account");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("publisher account does not match"));
}
#[test]
fn validate_taikai_ssm_rejects_self_asserted_alias_council() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let attacker_ssm = build_ssm_bytes(
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
    );
    let trusted_policy = alias_council_policy(&[[0x44; 32]], 1);
    let (_, telemetry) = telemetry_handle_for_tests();
    let err = taikai::validate_taikai_ssm(
        &attacker_ssm,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &taikai_alias_cache_policy(),
        Some(&trusted_policy),
        &telemetry,
    )
    .expect_err("self-asserted alias council must fail Taikai admission");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("not trusted"), "unexpected error: {}", err.1);
}
#[test]
fn validate_taikai_ssm_accepts_trusted_alias_council_threshold() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let council_seeds = [[0x33; 32], [0x44; 32], [0x55; 32]];
    let ssm = build_ssm_bytes_with_alias_council(
        manifest.manifest_hash,
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
        Algorithm::Ed25519,
        &council_seeds[..2],
    );
    let trusted_policy = alias_council_policy(&council_seeds, 2);
    let (_, telemetry) = telemetry_handle_for_tests();
    let outcome = taikai::validate_taikai_ssm(
        &ssm,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &taikai_alias_cache_policy(),
        Some(&trusted_policy),
        &telemetry,
    )
    .expect("trusted 2-of-3 alias council must authorize Taikai admission");
    assert_eq!(outcome.alias_label, "sora/docs");
}
#[test]
fn validate_taikai_ssm_rejects_alias_manifest_binding_mismatch() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let council_seeds = [[0x33; 32]];
    let ssm = build_ssm_bytes_with_alias_council(
        manifest.manifest_hash,
        BlobDigest::from_hash(blake3_hash(b"different DA manifest")),
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
        Algorithm::Ed25519,
        &council_seeds,
    );
    let trusted_policy = alias_council_policy(&council_seeds, 1);
    let (_, telemetry) = telemetry_handle_for_tests();
    let err = taikai::validate_taikai_ssm(
        &ssm,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &taikai_alias_cache_policy(),
        Some(&trusted_policy),
        &telemetry,
    )
    .expect_err("alias proof for another manifest must fail Taikai admission");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1
            .contains("does not commit to the canonical DA manifest"),
        "unexpected error: {}",
        err.1
    );
}
#[test]
fn validate_taikai_ssm_fails_closed_without_alias_council_policy() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let ssm = build_ssm_bytes(
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
    );
    let (_, telemetry) = telemetry_handle_for_tests();
    let err = taikai::validate_taikai_ssm(
        &ssm,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &taikai_alias_cache_policy(),
        None,
        &telemetry,
    )
    .expect_err("Taikai admission without a trust policy must fail closed");
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        err.1
            .contains("requires a configured SoraFS council trust policy"),
        "unexpected error: {}",
        err.1
    );
}
#[test]
fn validate_taikai_ssm_rejects_manifest_mismatch() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let bad_ssm = build_ssm_bytes(
        BlobDigest::from_hash(blake3_hash(b"other-manifest")),
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
    );
    let alias_policy = taikai_alias_cache_policy();
    let (_, telemetry) = telemetry_handle_for_tests();
    let err = taikai::validate_taikai_ssm(
        &bad_ssm,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &alias_policy,
        Some(&alias_council_policy(&[[0x33; 32]], 1)),
        &telemetry,
    )
    .expect_err("manifest mismatch must fail");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
#[test]
fn validate_taikai_ssm_rejects_tampered_signature() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let mut ssm_bytes = build_ssm_bytes(
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
    );
    // Flip a byte in the signature payload to break verification.
    if let Some(last) = ssm_bytes.last_mut() {
        *last ^= 0xFF;
    }
    let alias_policy = taikai_alias_cache_policy();
    let (_, telemetry) = telemetry_handle_for_tests();
    let err = taikai::validate_taikai_ssm(
        &ssm_bytes,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &alias_policy,
        Some(&alias_council_policy(&[[0x33; 32]], 1)),
        &telemetry,
    )
    .expect_err("tampered signature must fail");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
#[test]
fn validate_taikai_ssm_rejects_malformed_ed25519_signature_r() {
    const NONCANONICAL_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let ssm_bytes = build_ssm_bytes(
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
    );
    let signing_manifest: TaikaiSegmentSigningManifestV1 =
        norito::decode_from_bytes(&ssm_bytes).expect("decode signing manifest");
    let alias_policy = taikai_alias_cache_policy();
    let (_, telemetry) = telemetry_handle_for_tests();
    taikai::validate_taikai_ssm(
        &ssm_bytes,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &alias_policy,
        Some(&alias_council_policy(&[[0x33; 32]], 1)),
        &telemetry,
    )
    .expect("valid SSM should verify before mutation");
    let mut small_order_r = [0_u8; 32];
    small_order_r[0] = 1;
    for (label, replacement_r) in [
        ("small-order", small_order_r),
        ("noncanonical", NONCANONICAL_R),
    ] {
        let mut malformed = signing_manifest.clone();
        let mut signature_payload = malformed.signature.payload().to_vec();
        signature_payload[..replacement_r.len()].copy_from_slice(&replacement_r);
        malformed.signature =
            SignatureOf::from_signature(Signature::from_bytes(&signature_payload));
        let malformed_ssm = to_bytes(&malformed).expect("encode malformed signing manifest");
        let err = taikai::validate_taikai_ssm(
            &malformed_ssm,
            &manifest.manifest_hash,
            &taikai.car_digest,
            &taikai.envelope_bytes,
            taikai.telemetry.segment_sequence,
            &alias_policy,
            Some(&alias_council_policy(&[[0x33; 32]], 1)),
            &telemetry,
        )
        .expect_err("malformed Taikai SSM signature R must fail");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        let message = &err.1;
        assert!(
            message.contains("publisher signature material malformed"),
            "{label} malformed SSM signature R should fail admission: {message}"
        );
    }
}
#[test]
fn validate_taikai_ssm_rejects_malformed_mldsa_signature_lengths() {
    let (manifest, taikai) = taikai_ssm_validation_fixture();
    let now_secs = crate::sorafs::unix_now_secs();
    let ssm_bytes = build_ssm_bytes_with_publisher_algorithm(
        manifest.manifest_hash,
        taikai.car_digest,
        BlobDigest::from_hash(blake3_hash(&taikai.envelope_bytes)),
        taikai.telemetry.segment_sequence,
        now_secs,
        now_secs + 600,
        Algorithm::MlDsa,
    );
    let signing_manifest: TaikaiSegmentSigningManifestV1 =
        norito::decode_from_bytes(&ssm_bytes).expect("decode ML-DSA signing manifest");
    let alias_policy = taikai_alias_cache_policy();
    let (_, telemetry) = telemetry_handle_for_tests();
    taikai::validate_taikai_ssm(
        &ssm_bytes,
        &manifest.manifest_hash,
        &taikai.car_digest,
        &taikai.envelope_bytes,
        taikai.telemetry.segment_sequence,
        &alias_policy,
        Some(&alias_council_policy(&[[0x33; 32]], 1)),
        &telemetry,
    )
    .expect("valid ML-DSA SSM should verify before mutation");
    let mut extended = signing_manifest.signature.payload().to_vec();
    extended.push(0);
    for (label, signature_payload) in [
        (
            "truncated",
            signing_manifest.signature.payload()[..signing_manifest.signature.payload().len() - 1]
                .to_vec(),
        ),
        ("extended", extended),
    ] {
        let mut malformed = signing_manifest.clone();
        malformed.signature =
            SignatureOf::from_signature(Signature::from_bytes(&signature_payload));
        let malformed_ssm = to_bytes(&malformed).expect("encode malformed ML-DSA signing manifest");
        let err = taikai::validate_taikai_ssm(
            &malformed_ssm,
            &manifest.manifest_hash,
            &taikai.car_digest,
            &taikai.envelope_bytes,
            taikai.telemetry.segment_sequence,
            &alias_policy,
            Some(&alias_council_policy(&[[0x33; 32]], 1)),
            &telemetry,
        )
        .expect_err("malformed Taikai SSM ML-DSA signature length must fail");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        let message = &err.1;
        assert!(
            message.contains("publisher signature material malformed"),
            "{label} malformed SSM ML-DSA signature length should fail admission: {message}"
        );
    }
}
#[test]
fn validate_taikai_trm_accepts_matching_manifest() {
    let (_, taikai) = taikai_ssm_validation_fixture();
    let manifest = sample_trm_manifest_for_envelope(&taikai);
    let trm_bytes = to_bytes(&manifest).expect("encode trm");
    let routing_manifest =
        taikai::validate_taikai_trm(&trm_bytes, &taikai, &manifest.alias_binding)
            .expect("trm valid");
    assert_eq!(
        routing_manifest.alias_binding.name.as_str(),
        "docs",
        "alias binding should match the stream metadata"
    );
    assert_eq!(
        routing_manifest.segment_window.start_sequence, 0,
        "validated manifest should expose the expected window"
    );
}
#[test]
fn validate_taikai_trm_rejects_mismatched_event() {
    let (_, taikai) = taikai_ssm_validation_fixture();
    let mut trm = sample_trm_manifest_for_envelope(&taikai);
    trm.event_id = TaikaiEventId::new(Name::from_str("other-event").unwrap());
    let trm_bytes = to_bytes(&trm).expect("encode trm");
    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai, &trm.alias_binding)
        .expect_err("validation must fail");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
#[test]
fn validate_taikai_trm_rejects_invalid_version() {
    let taikai = taikai_envelope_fixture();
    let mut trm = sample_trm_manifest_for_envelope(&taikai);
    trm.version = TaikaiRoutingManifestV1::VERSION + 1;
    let trm_bytes = to_bytes(&trm).expect("encode trm");
    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai, &trm.alias_binding)
        .expect_err("validation must fail");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("unsupported manifest version"),
        "unexpected error message: {}",
        err.1
    );
}
#[test]
fn validate_taikai_trm_rejects_invalid_window() {
    let taikai = taikai_envelope_fixture();
    let mut trm = sample_trm_manifest_for_envelope(&taikai);
    trm.segment_window = TaikaiSegmentWindow::new(50, 40);
    let trm_bytes = to_bytes(&trm).expect("encode trm");
    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai, &trm.alias_binding)
        .expect_err("validation must fail");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("invalid routing manifest"),
        "unexpected error message: {}",
        err.1
    );
}

#[test]
fn validate_taikai_trm_rejects_terminal_window_before_lineage_mutation() {
    let taikai = taikai_envelope_fixture();
    let mut trm = sample_trm_manifest_for_envelope(&taikai);
    trm.segment_window = TaikaiSegmentWindow::new(40, u64::MAX);
    let trm_bytes = to_bytes(&trm).expect("encode terminal-window trm");

    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai, &trm.alias_binding)
        .expect_err("terminal TRM window must fail before a lineage guard is created");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1
            .contains("segment window end must be less than u64::MAX"),
        "unexpected terminal-window error: {}",
        err.1
    );
}

#[test]
fn validate_taikai_trm_rejects_oversized_window_before_lineage_mutation() {
    let taikai = taikai_envelope_fixture();
    let mut trm = sample_trm_manifest_for_envelope(&taikai);
    trm.segment_window = TaikaiSegmentWindow::new(40, 160);
    let trm_bytes = to_bytes(&trm).expect("encode oversized-window trm");

    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai, &trm.alias_binding)
        .expect_err("121-segment TRM window must fail before a lineage guard is created");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1
            .contains("segment window covers 121 sequences; maximum is 120"),
        "unexpected oversized-window error: {}",
        err.1
    );
}

#[test]
fn validate_taikai_trm_rejects_rendition_window_that_misses_segment() {
    let taikai = taikai_envelope_fixture();
    let mut trm = sample_trm_manifest_for_envelope(&taikai);
    trm.renditions[0].ssm_range = TaikaiSegmentWindow::new(50, 64);
    let trm_bytes = to_bytes(&trm).expect("encode trm");
    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai, &trm.alias_binding)
        .expect_err("rendition signing window must cover the admitted segment");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("rendition `1080p` signing window"),
        "unexpected error message: {}",
        err.1
    );
}
#[test]
fn validate_taikai_trm_rejects_head_manifest_mismatch() {
    let taikai = taikai_envelope_fixture();
    let mut trm = sample_trm_manifest_for_envelope(&taikai);
    trm.renditions[0].latest_manifest_hash = BlobDigest::from_hash(blake3_hash(b"other-manifest"));
    let trm_bytes = to_bytes(&trm).expect("encode trm");
    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai, &trm.alias_binding)
        .expect_err("TRM head manifest must bind the admitted segment");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("latest_manifest_hash"));
}
#[test]
fn validate_taikai_trm_rejects_head_car_mismatch() {
    let taikai = taikai_envelope_fixture();
    let mut trm = sample_trm_manifest_for_envelope(&taikai);
    trm.renditions[0].latest_car.car_digest = BlobDigest::from_hash(blake3_hash(b"other-car"));
    let trm_bytes = to_bytes(&trm).expect("encode trm");
    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai, &trm.alias_binding)
        .expect_err("TRM head CAR must bind the admitted segment");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("latest_car"));
}
#[test]
fn validate_taikai_trm_rejects_ssm_alias_mismatch() {
    let taikai = taikai_envelope_fixture();
    let trm = sample_trm_manifest_for_envelope(&taikai);
    let trm_bytes = to_bytes(&trm).expect("encode trm");
    let mut ssm_alias = trm.alias_binding.clone();
    ssm_alias.name = "other-alias".to_owned();
    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai, &ssm_alias)
        .expect_err("TRM alias must bind the authenticated SSM alias");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("authenticated SSM alias binding"));
}
#[test]
fn validate_taikai_trm_rejects_ssm_alias_proof_mismatch() {
    let taikai = taikai_envelope_fixture();
    let trm = sample_trm_manifest_for_envelope(&taikai);
    let trm_bytes = to_bytes(&trm).expect("encode trm");
    let mut ssm_alias = trm.alias_binding.clone();
    ssm_alias.proof.push(0xff);
    let err = taikai::validate_taikai_trm(&trm_bytes, &taikai, &ssm_alias)
        .expect_err("TRM alias proof must be the proof authenticated by the SSM");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("authenticated SSM alias binding"));
}
#[test]
fn normalize_payload_handles_gzip() {
    let mut request = sample_request();
    let canonical = request.payload.clone();
    let mut encoder = GzEncoder::new(Vec::new(), FlateCompression::default());
    encoder.write_all(&canonical).expect("write gzip payload");
    let compressed = encoder.finish().expect("finish gzip payload");
    request.payload = compressed;
    request.compression = Compression::Gzip;
    request.total_size = canonical.len() as u64;
    let normalized = normalize_payload(&request).expect("normalize gzip payload");
    assert_eq!(normalized.as_slice(), canonical.as_slice());
}
#[test]
fn normalize_payload_rejects_size_mismatch() {
    let mut request = sample_request();
    let canonical = request.payload.clone();
    let mut encoder = GzEncoder::new(Vec::new(), FlateCompression::default());
    encoder.write_all(&canonical).expect("write gzip payload");
    let compressed = encoder.finish().expect("finish gzip payload");
    request.payload = compressed;
    request.compression = Compression::Gzip;
    request.total_size = (canonical.len() as u64) + 1;
    let err = match normalize_payload(&request) {
        Ok(_) => panic!("expected normalization to reject mismatched size"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
struct CountingReader {
    remaining: usize,
    emitted: usize,
}
impl CountingReader {
    fn new(remaining: usize) -> Self {
        Self {
            remaining,
            emitted: 0,
        }
    }
}
impl Read for CountingReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Ok(0);
        }
        let n = self.remaining.min(buf.len());
        buf[..n].fill(0xA5);
        self.remaining -= n;
        self.emitted += n;
        Ok(n)
    }
}
#[test]
fn decompress_reader_stops_after_advertised_len_plus_one() {
    let mut reader = CountingReader::new(64);
    let err = decompress_reader(&mut reader, 8, "test")
        .expect_err("overlong decompressed stream should reject");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        reader.emitted, 9,
        "decompressor should read only one byte beyond advertised length"
    );
    assert!(
        err.1
            .contains("test payload decompressed to 9 bytes but total_size advertises 8 bytes"),
        "unexpected error message: {}",
        err.1
    );
}
#[test]
fn decompress_reader_rejects_unbounded_expected_len_without_reading() {
    let mut reader = CountingReader::new(1);
    let err = decompress_reader(&mut reader, usize::MAX, "test")
        .expect_err("unbounded expected length should reject");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert_eq!(reader.emitted, 0);
    assert!(
        err.1.contains("supported decompression boundary"),
        "unexpected error message: {}",
        err.1
    );
}
#[test]
fn build_receipt_includes_pdp_commitment() {
    let request = sample_request();
    let signer = checked_random_keypair();
    let pdp_commitment = sample_pdp_commitment_for_tests();
    let encoded = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let rent_quote = DaRentQuote {
        base_rent: XorQuantity::try_from_micro(111)
            .expect("legacy micro-XOR value is representable"),
        protocol_reserve: XorQuantity::try_from_micro(222)
            .expect("legacy micro-XOR value is representable"),
        provider_reward: XorQuantity::try_from_micro(333)
            .expect("legacy micro-XOR value is representable"),
        pdp_bonus: XorQuantity::try_from_micro(444)
            .expect("legacy micro-XOR value is representable"),
        potr_bonus: XorQuantity::try_from_micro(555)
            .expect("legacy micro-XOR value is representable"),
        egress_credit_per_gib: XorQuantity::try_from_micro(666)
            .expect("legacy micro-XOR value is representable"),
    };
    let receipt = build_receipt(
        &signer,
        &request,
        123,
        BlobDigest::from_hash(blake3_hash(b"blob-hash")),
        BlobDigest::from_hash(blake3_hash(b"chunk-root")),
        BlobDigest::from_hash(blake3_hash(b"manifest-hash")),
        StorageTicketId::new([0x44; 32]),
        encoded.clone(),
        rent_quote.clone(),
        DaStripeLayout::default(),
    )
    .expect("build receipt");
    assert_eq!(receipt.pdp_commitment, Some(encoded));
    assert_eq!(receipt.rent_quote, rent_quote);
}
#[test]
fn build_receipt_signs_with_operator_key() {
    let request = sample_request();
    let signer = checked_random_keypair();
    let receipt = build_receipt(
        &signer,
        &request,
        999,
        BlobDigest::from_hash(blake3_hash(b"blob-hash")),
        BlobDigest::from_hash(blake3_hash(b"chunk-root")),
        BlobDigest::from_hash(blake3_hash(b"manifest-hash")),
        StorageTicketId::new([0xAA; 32]),
        Vec::new(),
        DaRentQuote::default(),
        DaStripeLayout::default(),
    )
    .expect("build receipt");
    let unsigned_bytes =
        persistence::unsigned_receipt_bytes(&receipt, request.sequence).expect("unsigned receipt");
    receipt
        .operator_signature
        .verify(signer.public_key(), &unsigned_bytes)
        .expect("signature verifies");
    let wrong_sequence_bytes = persistence::unsigned_receipt_bytes(&receipt, request.sequence + 1)
        .expect("wrong-sequence unsigned receipt");
    assert!(
        receipt
            .operator_signature
            .verify(signer.public_key(), &wrong_sequence_bytes)
            .is_err(),
        "operator signature must bind the request sequence"
    );
}
#[test]
fn build_receipt_computes_chunk_root_from_payload() {
    let request = sample_request();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_000,
        &rent_policy,
    )
    .expect("resolve manifest");
    let pdp_commitment = compute_pdp_commitment(
        &manifest.manifest_hash,
        &manifest.manifest,
        &chunk_store,
        canonical.as_slice(),
        1_701_000_000,
    )
    .expect("pdp commitment");
    let pdp_tree =
        PdpMerkleTreeV1::from_bytes(canonical.as_slice()).expect("canonical PDP fixture tree");
    assert_eq!(pdp_commitment.payload_len, pdp_tree.payload_len());
    assert_eq!(pdp_commitment.hot_leaf_count, pdp_tree.hot_leaf_count());
    assert_eq!(pdp_commitment.segment_count, pdp_tree.segment_count());
    assert_eq!(pdp_commitment.commitment_root_hot, pdp_tree.hot_root());
    assert_eq!(
        pdp_commitment.commitment_root_segment,
        pdp_tree.segment_root()
    );
    let encoded_commitment =
        encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
    let signer = checked_random_keypair();
    let receipt = build_receipt(
        &signer,
        &request,
        1_701_000_000,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        encoded_commitment,
        manifest.manifest.rent_quote,
        stripe_layout,
    )
    .expect("build receipt");
    assert_eq!(receipt.chunk_root, manifest.chunk_root);
    assert_eq!(
        manifest.chunk_root,
        BlobDigest::new(*chunk_store.por_tree().root())
    );
}
#[test]
fn build_receipt_prefers_chunk_root_from_manifest() {
    let mut request = sample_request();
    // Seed Taikai metadata so manifest validation passes gateway checks.
    request.metadata = taikai_metadata();
    let canonical = normalize_payload(&request).expect("normalize payload");
    let canonical_bytes = canonical.as_slice().to_vec();
    drop(canonical);
    let payload_hash = BlobDigest::from_hash(blake3_hash(&canonical_bytes));
    let chunk_store = build_chunk_store(&request, canonical_bytes.as_slice());
    taikai::apply_taikai_ingest_tags(
        &mut request.metadata,
        None,
        &request.retention_policy,
        payload_hash.clone(),
        request.total_size,
    )
    .expect("apply taikai tags to metadata");
    let manifest_chunk_root = BlobDigest::new(*chunk_store.por_tree().root());
    let chunk_commitments =
        build_chunk_commitments(&request, &chunk_store, canonical_bytes.as_slice())
            .expect("expected chunk commitments");
    let ipa_commitment =
        ipa_commitment_from_chunks(&chunk_commitments).expect("ipa commitment from chunks");
    let (total_stripes_full, shards_per_stripe) =
        manifest_stripe_layout_fields(chunk_store.chunks().len(), &request.erasure_profile)
            .expect("manifest stripe layout");
    let rent_policy = DaRentPolicyV1::default();
    let (rent_gib, rent_months) =
        rent_usage_from_request(request.total_size, &request.retention_policy)
            .expect("rent usage should fit test inputs");
    let rent_quote = rent_policy
        .quote(rent_gib, rent_months)
        .expect("compute rent quote for manifest");
    let manifest = DaManifestV1 {
        version: DaManifestV1::VERSION,
        client_blob_id: request.client_blob_id.clone(),
        lane_id: request.lane_id,
        epoch: request.epoch,
        blob_class: request.blob_class,
        codec: request.codec.clone(),
        blob_hash: payload_hash,
        chunk_root: manifest_chunk_root.clone(),
        storage_ticket: StorageTicketId::new([0x55; 32]),
        total_size: request.total_size,
        chunk_size: request.chunk_size,
        total_stripes: total_stripes_full,
        shards_per_stripe,
        erasure_profile: request.erasure_profile,
        retention_policy: request.retention_policy.clone(),
        rent_quote,
        chunks: chunk_commitments,
        ipa_commitment,
        metadata: request.metadata.clone(),
        issued_at_unix: 42,
    };
    request.norito_manifest = Some(to_bytes(&manifest).expect("encode manifest"));
    let canonical = normalize_payload(&request).expect("normalize payload with manifest");
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_123,
        &rent_policy,
    )
    .expect("resolve provided manifest");
    let pdp_commitment = compute_pdp_commitment(
        &manifest.manifest_hash,
        &manifest.manifest,
        &chunk_store,
        canonical.as_slice(),
        1_701_000_123,
    )
    .expect("pdp commitment");
    let encoded_commitment =
        encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
    let signer = checked_random_keypair();
    let receipt = build_receipt(
        &signer,
        &request,
        1_701_000_123,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        encoded_commitment,
        manifest.manifest.rent_quote,
        stripe_layout,
    )
    .expect("build receipt");
    assert_eq!(receipt.chunk_root, manifest_chunk_root);
}
#[test]
fn build_chunk_commitments_rejects_oversized_chunk_length() {
    let mut request = sample_request();
    request.chunk_size = MIN_CHUNK_SIZE_BYTES;
    let oversized_chunk_size = MIN_CHUNK_SIZE_BYTES * 2;
    request.payload = vec![
        0xA5;
        usize::try_from(oversized_chunk_size)
            .expect("test chunk size fits the host address space")
    ];
    request.total_size = request.payload.len() as u64;
    let canonical = normalize_payload(&request).expect("normalize payload");
    let oversized_profile = chunk_profile_for_request(oversized_chunk_size);
    let mut chunk_store = ChunkStore::with_profile(oversized_profile);
    chunk_store
        .ingest_bytes(canonical.as_slice())
        .expect("ingest canonical payload");
    let err = build_chunk_commitments(&request, &chunk_store, canonical.as_slice())
        .expect_err("oversized chunk length should be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("exceeds configured chunk_size"),
        "unexpected error message: {}",
        err.1
    );
}
#[test]
fn manifest_stripe_layout_fields_rejects_zero_data_shards() {
    let mut profile = sample_request().erasure_profile;
    profile.data_shards = 0;
    let err = manifest_stripe_layout_fields(1, &profile)
        .expect_err("zero data shards should be rejected before stripe math");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("at least one data shard"),
        "unexpected error message: {}",
        err.1
    );
}
#[test]
fn manifest_stripe_layout_fields_rejects_total_stripe_overflow() {
    let mut profile = sample_request().erasure_profile;
    profile.data_shards = 1;
    profile.parity_shards = 0;
    profile.row_parity_stripes = 1;
    let err = manifest_stripe_layout_fields(u32::MAX as usize, &profile)
        .expect_err("row parity must not overflow total stripe count");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1
            .contains("total stripes exceeds supported manifest stripe space"),
        "unexpected error message: {}",
        err.1
    );
}
#[test]
fn ipa_params_len_for_commitment_count_rounds_up_without_overflow() {
    assert_eq!(ipa_params_len_for_commitment_count(0).unwrap(), 1);
    assert_eq!(ipa_params_len_for_commitment_count(1).unwrap(), 1);
    assert_eq!(ipa_params_len_for_commitment_count(8).unwrap(), 8);
    assert_eq!(ipa_params_len_for_commitment_count(9).unwrap(), 16);
}
#[test]
fn ipa_params_len_for_commitment_count_rejects_power_of_two_overflow() {
    let overflow_count = (usize::MAX / 2).saturating_add(2);
    let err = ipa_params_len_for_commitment_count(overflow_count)
        .expect_err("overflowing IPA parameter length must reject");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("IPA commitment parameter size"),
        "unexpected error message: {}",
        err.1
    );
}
#[test]
fn build_chunk_commitments_rejects_row_parity_base_offset_overflow() {
    let mut request = sample_request();
    request.chunk_size = MIN_CHUNK_SIZE_BYTES;
    request.payload = vec![0xA5, 0x5A];
    request.total_size = u64::MAX - 1;
    request.erasure_profile = ErasureProfile {
        data_shards: 1,
        parity_shards: 1,
        row_parity_stripes: 1,
        chunk_alignment: 2,
        fec_scheme: FecScheme::Rs12_10,
    };
    let chunk_store = build_chunk_store(&request, request.payload.as_slice());
    let err = build_chunk_commitments(&request, &chunk_store, request.payload.as_slice())
        .expect_err("row parity base offset overflow should be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1
            .contains("stripe parity base offset exceeded supported size"),
        "unexpected error message: {}",
        err.1
    );
}
#[test]
fn build_chunk_commitments_rejects_row_parity_chunk_offset_overflow() {
    let mut request = sample_request();
    request.chunk_size = MIN_CHUNK_SIZE_BYTES;
    request.payload = vec![0xA5, 0x5A];
    request.total_size = u64::MAX - 1;
    request.erasure_profile = ErasureProfile {
        data_shards: 2,
        parity_shards: 0,
        row_parity_stripes: 1,
        chunk_alignment: 2,
        fec_scheme: FecScheme::Rs12_10,
    };
    let chunk_store = build_chunk_store(&request, request.payload.as_slice());
    let err = build_chunk_commitments(&request, &chunk_store, request.payload.as_slice())
        .expect_err("row parity chunk offset overflow should be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1
            .contains("stripe parity chunk offset exceeded supported size"),
        "unexpected error message: {}",
        err.1
    );
}
struct ManifestResolutionFixture {
    request: DaIngestRequest,
    canonical: Vec<u8>,
    chunk_store: ChunkStore,
    metadata: ExtraMetadata,
    rent_policy: DaRentPolicyV1,
}
impl ManifestResolutionFixture {
    fn new(request: DaIngestRequest) -> Self {
        let canonical = normalize_payload(&request)
            .expect("normalize payload")
            .into_vec();
        let chunk_store = build_chunk_store(&request, canonical.as_slice());
        let metadata = encrypt_governance_metadata(&request.metadata, None, None)
            .expect("metadata encryption");
        Self {
            request,
            canonical,
            chunk_store,
            metadata,
            rent_policy: DaRentPolicyV1::default(),
        }
    }
    fn resolve(&self, queued_at_unix: u64) -> Result<ManifestArtifacts, (StatusCode, String)> {
        self.resolve_with_retention(&self.request.retention_policy, queued_at_unix)
    }
    fn resolve_with_retention(
        &self,
        retention_policy: &RetentionPolicy,
        queued_at_unix: u64,
    ) -> Result<ManifestArtifacts, (StatusCode, String)> {
        resolve_manifest(
            &self.request,
            &self.chunk_store,
            self.canonical.as_slice(),
            &self.metadata,
            retention_policy,
            queued_at_unix,
            &self.rent_policy,
        )
    }
}
fn resolved_manifest_fixture(
    request: DaIngestRequest,
    queued_at_unix: u64,
    expectation: &str,
) -> (ManifestResolutionFixture, ManifestArtifacts) {
    let fixture = ManifestResolutionFixture::new(request);
    let artifacts = fixture.resolve(queued_at_unix).expect(expectation);
    (fixture, artifacts)
}
fn commitment_record_fixture(
    fixture: &ManifestResolutionFixture,
    manifest: &ManifestArtifacts,
    queued_at_unix: u64,
) -> (Vec<u8>, DaCommitmentRecord) {
    let mut pdp_commitment = sample_pdp_commitment_for_tests();
    pdp_commitment.manifest_digest = *manifest.manifest_hash.as_bytes();
    let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let receipt = build_receipt(
        &checked_random_keypair(),
        &fixture.request,
        queued_at_unix,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        pdp_bytes.clone(),
        manifest.manifest.rent_quote.clone(),
        stripe_layout_from_manifest(&manifest.manifest),
    )
    .expect("build receipt");
    let record = build_da_commitment_record(
        &fixture.request,
        manifest,
        &fixture.request.retention_policy,
        &receipt.operator_signature,
        &pdp_bytes,
        DaProofScheme::MerkleSha256,
    );
    (pdp_bytes, record)
}
#[test]
fn persist_manifest_for_sorafs_writes_and_is_idempotent() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let (fixture, manifest) =
        resolved_manifest_fixture(sample_request(), 1_701_000_555, "manifest");
    let request = &fixture.request;
    let first_path = persistence::persist_manifest_for_sorafs(
        manifest_dir,
        &manifest.encoded,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist manifest")
    .expect("spool path");
    let ticket_hex = hex::encode(manifest.storage_ticket.as_bytes());
    assert_eq!(
        first_path,
        manifest_dir
            .join("artifacts")
            .join(&ticket_hex[..2])
            .join(&ticket_hex)
            .join("manifest.norito"),
        "manifest persistence must use the direct sharded ticket index"
    );
    let bytes = fs::read(&first_path).expect("read manifest file");
    assert_eq!(bytes, manifest.encoded);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&first_path)
            .expect("read manifest permissions")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o077,
            0,
            "persisted DA artifacts must not be accessible by group or other users"
        );
        let directory_mode = fs::metadata(first_path.parent().expect("ticket artifact directory"))
            .expect("read ticket directory permissions")
            .permissions()
            .mode();
        assert_eq!(
            directory_mode & 0o077,
            0,
            "ticket artifact directories must not be accessible by group or other users"
        );
    }
    let second_path = persistence::persist_manifest_for_sorafs(
        manifest_dir,
        &manifest.encoded,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist manifest idempotent")
    .expect("spool path");
    assert_eq!(first_path, second_path);
}
#[test]
fn persist_pdp_commitment_writes_and_is_idempotent() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let (fixture, manifest) =
        resolved_manifest_fixture(sample_request(), 1_701_000_777, "manifest");
    let request = &fixture.request;
    let commitment = compute_pdp_commitment(
        &manifest.manifest_hash,
        &manifest.manifest,
        &fixture.chunk_store,
        fixture.canonical.as_slice(),
        1_701_000_777,
    )
    .expect("commitment");
    let first_path = persistence::persist_pdp_commitment(
        manifest_dir,
        &commitment,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist commitment")
    .expect("spool path");
    let ticket_hex = hex::encode(manifest.storage_ticket.as_bytes());
    assert_eq!(
        first_path,
        manifest_dir
            .join("artifacts")
            .join(&ticket_hex[..2])
            .join(&ticket_hex)
            .join("pdp-commitment.norito"),
        "PDP persistence must use the direct sharded ticket index"
    );
    let bytes = fs::read(&first_path).expect("read commitment file");
    let archived = from_bytes::<PdpCommitmentV1>(&bytes).expect("decode commitment");
    let decoded = PdpCommitmentV1::deserialize(archived);
    assert_eq!(decoded, commitment);
    let second_path = persistence::persist_pdp_commitment(
        manifest_dir,
        &commitment,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist commitment idempotent")
    .expect("spool path");
    assert_eq!(first_path, second_path);
}
#[test]
fn build_da_commitment_record_reflects_artifacts() {
    let (fixture, manifest) =
        resolved_manifest_fixture(sample_request(), 1_701_500_000, "manifest");
    let request = &fixture.request;
    let (_, record) = commitment_record_fixture(&fixture, &manifest, 1_701_500_000);
    assert_eq!(record.lane_id, request.lane_id);
    assert_eq!(record.epoch, request.epoch);
    assert_eq!(record.sequence, request.sequence);
    assert_eq!(record.client_blob_id, request.client_blob_id);
    assert_eq!(
        record.manifest_hash.as_bytes(),
        manifest.manifest_hash.as_bytes()
    );
    assert_eq!(record.retention_class, request.retention_policy);
    assert_eq!(record.storage_ticket, manifest.storage_ticket);
    assert!(record.proof_digest.is_some(), "expected proof digest");
    assert_eq!(record.proof_scheme, DaProofScheme::MerkleSha256);
}
#[test]
fn persist_da_commitment_record_writes_and_is_idempotent() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let (fixture, manifest) =
        resolved_manifest_fixture(sample_request(), 1_701_600_000, "manifest");
    let request = &fixture.request;
    let (_, record) = commitment_record_fixture(&fixture, &manifest, 1_701_600_000);
    let first_path = persistence::persist_da_commitment_record(
        manifest_dir,
        &record,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist record")
    .expect("spool path");
    let bytes = fs::read(&first_path).expect("read record file");
    let archived = from_bytes::<DaCommitmentRecord>(&bytes).expect("decode record");
    let decoded = DaCommitmentRecord::deserialize(archived);
    assert_eq!(decoded, record);
    let second_path = persistence::persist_da_commitment_record(
        manifest_dir,
        &record,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist record idempotent")
    .expect("spool path");
    assert_eq!(first_path, second_path);
}
#[test]
fn persist_da_commitment_schedule_entry_writes_bundle() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let (fixture, manifest) =
        resolved_manifest_fixture(sample_request(), 1_701_600_000, "manifest");
    let request = &fixture.request;
    let (pdp_bytes, record) = commitment_record_fixture(&fixture, &manifest, 1_701_600_000);
    let schedule_path = persistence::persist_da_commitment_schedule_entry(
        manifest_dir,
        &record,
        &pdp_bytes,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist schedule entry")
    .expect("schedule path");
    let bytes = fs::read(&schedule_path).expect("read schedule entry");
    let archived = from_bytes::<persistence::DaCommitmentScheduleEntry>(&bytes)
        .expect("decode schedule entry");
    let decoded = persistence::DaCommitmentScheduleEntry::deserialize(archived);
    assert_eq!(decoded.record, record);
    assert_eq!(decoded.pdp_commitment, pdp_bytes);
}
#[test]
fn persist_da_pin_intent_writes_file() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let mut request = sample_request();
    request.sequence = 42;
    request.metadata.items.push(MetadataEntry::new(
        META_DA_REGISTRY_ALIAS,
        b"sora/docs".to_vec(),
        MetadataVisibility::Public,
    ));
    resign_sample_request(&mut request);
    let (fixture, manifest) = resolved_manifest_fixture(request, 1_701_700_123, "manifest");
    let request = &fixture.request;
    let alias =
        registry_alias_from_metadata(&request.metadata).expect("alias metadata should parse");
    let intent = signed_pin_intent(
        request,
        manifest.storage_ticket,
        ManifestDigest::new(*manifest.manifest_hash.as_bytes()),
        alias,
    );
    let path = persistence::persist_da_pin_intent(
        manifest_dir,
        &intent,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist pin")
    .expect("path");
    let bytes = fs::read(&path).expect("read pin intent");
    let archived = from_bytes::<DaPinIntent>(&bytes).expect("decode pin intent");
    let decoded: DaPinIntent =
        NoritoDeserialize::try_deserialize(archived).expect("deserialize pin intent");
    assert_eq!(decoded, intent);
    assert_eq!(decoded.alias, Some("sora/docs".to_owned()));
    assert_eq!(decoded.authorization.owner, *ALICE_ID);
    let loaded = persistence::load_da_pin_intent(
        manifest_dir,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("load exact pin intent");
    assert_eq!(loaded, intent);
}

#[test]
fn persist_da_pin_scope_roundtrips_and_rejects_replacement() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let (fixture, manifest) =
        resolved_manifest_fixture(sample_request(), 1_701_700_124, "manifest");
    let request = &fixture.request;
    let scope = build_da_pin_scope(request, manifest.storage_ticket, manifest.manifest_hash)
        .expect("build exact pin scope");
    let path = persistence::persist_da_pin_scope(
        manifest_dir,
        &scope,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist pin scope")
    .expect("pin-scope path");
    assert!(path.exists());
    let loaded = persistence::load_da_pin_scope(
        manifest_dir,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("load exact pin scope");
    assert_eq!(loaded, scope);

    let mut replacement = scope;
    replacement.alias = Some("forged-replacement".to_owned());
    let error = persistence::persist_da_pin_scope(
        manifest_dir,
        &replacement,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect_err("an existing durable scope cannot be replaced");
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn load_da_pin_intent_rejects_filename_body_tuple_mismatch() {
    let temp_dir = tempdir().expect("temp dir");
    let context = sample_manifest_context_for(BlobClass::TaikaiSegment);
    let request = context.request;
    let manifest = context.artifacts;
    let intent = signed_pin_intent_for_manifest(&request, &manifest);
    let wrong_sequence = request.sequence.saturating_add(1);
    let path = spool_artifact_path_for_key(
        temp_dir.path(),
        "da-pin-intent-",
        request.lane_id,
        request.epoch,
        wrong_sequence,
        &manifest.storage_ticket,
        *manifest.fingerprint.as_bytes(),
    );
    fs::write(&path, to_bytes(&intent).expect("encode pin intent"))
        .expect("write mismatched pin-intent fixture");
    let err = persistence::load_da_pin_intent(
        temp_dir.path(),
        request.lane_id,
        request.epoch,
        wrong_sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect_err("pin-intent filename/body mismatch must reject");
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("does not match its filename"),
        "unexpected pin-intent mismatch error: {err}"
    );
}
fn assert_invalid_input<T>(result: std::io::Result<T>, label: &str) {
    let err = match result {
        Ok(_) => panic!("{label} unexpectedly accepted invalid writer inputs"),
        Err(err) => err,
    };
    assert_eq!(
        err.kind(),
        ErrorKind::InvalidInput,
        "{label} should reject invalid writer inputs: {err}"
    );
}
#[test]
fn persist_spool_artifacts_reject_body_tuple_mismatches() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let request = context.request;
    let manifest = context.artifacts;
    let mut pdp_commitment = sample_pdp_commitment_for_tests();
    pdp_commitment.manifest_digest = *manifest.manifest_hash.as_bytes();
    let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let record = DaCommitmentRecord::new(
        request.lane_id,
        request.epoch,
        request.sequence,
        request.client_blob_id.clone(),
        ManifestDigest::new(*manifest.manifest_hash.as_bytes()),
        DaProofScheme::MerkleSha256,
        Hash::prehashed(*manifest.chunk_root.as_bytes()),
        Some(Hash::new(&pdp_bytes)),
        request.retention_policy.clone(),
        manifest.storage_ticket,
        Signature::try_from_bytes(&[0x44; 64])
            .expect("checked Torii DA persistence acknowledgement signature fixture"),
    );
    assert_invalid_input(
        persistence::persist_manifest_for_sorafs(
            manifest_dir,
            &manifest.encoded,
            LaneId::new(request.lane_id.as_u32().saturating_add(1)),
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "manifest lane mismatch",
    );
    let mut invalid_pdp = pdp_commitment.clone();
    invalid_pdp.manifest_digest = [0; 32];
    assert_invalid_input(
        persistence::persist_pdp_commitment(
            manifest_dir,
            &invalid_pdp,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "invalid PDP commitment body",
    );
    let mut wrong_fingerprint = *manifest.fingerprint.as_bytes();
    wrong_fingerprint[0] ^= 0xFF;
    assert_invalid_input(
        persistence::persist_pdp_commitment(
            manifest_dir,
            &pdp_commitment,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &ReplayFingerprint::from(wrong_fingerprint),
        ),
        "PDP ticket fingerprint mismatch",
    );
    assert_invalid_input(
        persistence::persist_da_commitment_record(
            manifest_dir,
            &record,
            request.lane_id,
            request.epoch,
            request.sequence.saturating_add(1),
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "commitment sequence mismatch",
    );
    let mut other_pdp = pdp_commitment.clone();
    other_pdp.sealed_at = other_pdp.sealed_at.saturating_add(1);
    let other_pdp_bytes = encode_pdp_commitment_bytes(&other_pdp).expect("encode other PDP");
    assert_invalid_input(
        persistence::persist_da_commitment_schedule_entry(
            manifest_dir,
            &record,
            &other_pdp_bytes,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "schedule PDP digest mismatch",
    );
    let mut wrong_manifest_pdp = pdp_commitment.clone();
    wrong_manifest_pdp.manifest_digest[0] ^= 0xFF;
    let wrong_manifest_pdp_bytes =
        encode_pdp_commitment_bytes(&wrong_manifest_pdp).expect("encode wrong-manifest PDP");
    let mut wrong_manifest_record = record.clone();
    wrong_manifest_record.proof_digest = Some(Hash::new(&wrong_manifest_pdp_bytes));
    assert_invalid_input(
        persistence::persist_da_commitment_schedule_entry(
            manifest_dir,
            &wrong_manifest_record,
            &wrong_manifest_pdp_bytes,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "schedule PDP manifest digest mismatch",
    );
    let intent = signed_pin_intent(
        request,
        manifest.storage_ticket,
        ManifestDigest::new(*manifest.manifest_hash.as_bytes()),
        None,
    );
    assert_invalid_input(
        persistence::persist_da_pin_intent(
            manifest_dir,
            &intent,
            request.lane_id,
            request.epoch,
            request.sequence.saturating_add(1),
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "pin-intent sequence mismatch",
    );
    assert!(
        fs::read_dir(manifest_dir)
            .expect("read spool dir")
            .next()
            .is_none(),
        "rejected writer inputs must not leave spool artifacts"
    );
}
#[test]
fn persist_spool_artifacts_reject_existing_mismatched_targets() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let request = context.request;
    let manifest = context.artifacts;
    let mut pdp_commitment = sample_pdp_commitment_for_tests();
    pdp_commitment.manifest_digest = *manifest.manifest_hash.as_bytes();
    let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode commitment");
    let signer = checked_random_keypair();
    let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
    let receipt = build_receipt(
        &signer,
        &request,
        1_701_000_999,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        pdp_bytes.clone(),
        manifest.manifest.rent_quote.clone(),
        stripe_layout,
    )
    .expect("build receipt");
    let record = build_da_commitment_record(
        &request,
        &manifest,
        &request.retention_policy,
        &receipt.operator_signature,
        &pdp_bytes,
        DaProofScheme::MerkleSha256,
    );
    let intent = signed_pin_intent(
        &request,
        manifest.storage_ticket,
        ManifestDigest::new(*manifest.manifest_hash.as_bytes()),
        None,
    );
    let fingerprint = *manifest.fingerprint.as_bytes();
    let assert_invalid_data =
        |result: std::io::Result<Option<PathBuf>>, artifact: &str| match result {
            Ok(path) => panic!("{artifact} unexpectedly accepted existing target {path:?}"),
            Err(err) => assert_eq!(
                err.kind(),
                std::io::ErrorKind::InvalidData,
                "{artifact} should reject mismatched existing bytes"
            ),
        };
    let manifest_path = spool_artifact_path_for_key(
        manifest_dir,
        "manifest-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        fingerprint,
    );
    fs::write(&manifest_path, b"poison-manifest").expect("poison manifest");
    assert_invalid_data(
        persistence::persist_manifest_for_sorafs(
            manifest_dir,
            &manifest.encoded,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "manifest",
    );
    let pdp_path = spool_artifact_path_for_key(
        manifest_dir,
        "pdp-commitment-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        fingerprint,
    );
    fs::write(&pdp_path, b"poison-pdp").expect("poison pdp");
    assert_invalid_data(
        persistence::persist_pdp_commitment(
            manifest_dir,
            &pdp_commitment,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "pdp",
    );
    let commitment_path = spool_artifact_path_for_key(
        manifest_dir,
        "da-commitment-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        fingerprint,
    );
    fs::write(&commitment_path, b"poison-commitment").expect("poison commitment");
    assert_invalid_data(
        persistence::persist_da_commitment_record(
            manifest_dir,
            &record,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "commitment",
    );
    let schedule_path = spool_artifact_path_for_key(
        manifest_dir,
        "da-commitment-schedule-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        fingerprint,
    );
    fs::write(&schedule_path, b"poison-schedule").expect("poison schedule");
    assert_invalid_data(
        persistence::persist_da_commitment_schedule_entry(
            manifest_dir,
            &record,
            &pdp_bytes,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "schedule",
    );
    let pin_path = spool_artifact_path_for_key(
        manifest_dir,
        "da-pin-intent-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        fingerprint,
    );
    fs::write(&pin_path, b"poison-pin").expect("poison pin");
    assert_invalid_data(
        persistence::persist_da_pin_intent(
            manifest_dir,
            &intent,
            request.lane_id,
            request.epoch,
            request.sequence,
            &manifest.storage_ticket,
            &manifest.fingerprint,
        ),
        "pin",
    );
    let receipt_path = receipt_spool_path(manifest_dir, &receipt, request.sequence, fingerprint);
    fs::write(&receipt_path, b"poison-receipt").expect("poison receipt");
    assert_invalid_data(
        persistence::persist_da_receipt(
            manifest_dir,
            &receipt,
            request.sequence,
            &manifest.fingerprint,
        ),
        "receipt",
    );
}
#[cfg(unix)]
#[test]
fn persist_spool_artifacts_reject_existing_target_symlink() {
    use std::os::unix::fs::symlink;
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let request = context.request;
    let manifest = context.artifacts;
    let fingerprint = *manifest.fingerprint.as_bytes();
    let target = manifest_dir.join("manifest-target.norito");
    fs::write(&target, &manifest.encoded).expect("write symlink target");
    let manifest_path = spool_artifact_path_for_key(
        manifest_dir,
        "manifest-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        fingerprint,
    );
    symlink(&target, &manifest_path).expect("create manifest artifact symlink");
    let err = persistence::persist_manifest_for_sorafs(
        manifest_dir,
        &manifest.encoded,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect_err("existing manifest target symlink must reject idempotent write");
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    assert!(
        fs::symlink_metadata(&manifest_path)
            .expect("inspect symlink")
            .file_type()
            .is_symlink(),
        "rejected symlink should be left in place for operator inspection"
    );
    assert_eq!(
        fs::read(&target).expect("read symlink target"),
        manifest.encoded,
        "rejected symlink target should not be modified"
    );
}
fn test_receipt(
    signer: &KeyPair,
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    seed: u8,
) -> DaIngestReceipt {
    let mut receipt = DaIngestReceipt {
        client_blob_id: BlobDigest::new([seed; 32]),
        lane_id,
        epoch,
        blob_hash: BlobDigest::new([seed.wrapping_add(1); 32]),
        chunk_root: BlobDigest::new([seed.wrapping_add(2); 32]),
        manifest_hash: BlobDigest::new([seed.wrapping_add(3); 32]),
        storage_ticket: StorageTicketId::new([seed; 32]),
        pdp_commitment: Some(vec![seed]),
        stripe_layout: DaStripeLayout::default(),
        queued_at_unix: 1234,
        rent_quote: DaRentQuote::default(),
        operator_signature: persistence::receipt_signature_placeholder(),
    };
    let unsigned =
        persistence::unsigned_receipt_bytes(&receipt, sequence).expect("test receipt encodes");
    receipt.operator_signature = checked_signature(signer.private_key(), &unsigned);
    receipt
}
fn test_fingerprint(seed: u8) -> ReplayFingerprint {
    ReplayFingerprint::from([seed; blake3::OUT_LEN])
}
fn receipt_fingerprint(receipt: &DaIngestReceipt) -> ReplayFingerprint {
    ReplayFingerprint::from(*receipt.storage_ticket.as_bytes())
}
fn receipt_fingerprint_bytes(receipt: &DaIngestReceipt) -> [u8; 32] {
    *receipt.storage_ticket.as_bytes()
}
fn receipt_spool_file_name(
    receipt: &DaIngestReceipt,
    sequence: u64,
    fingerprint: [u8; 32],
) -> String {
    format!(
        "da-receipt-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket_hex}-{fingerprint_hex}.norito",
        lane = receipt.lane_id.as_u32(),
        epoch = receipt.epoch,
        ticket_hex = hex::encode(receipt.storage_ticket.as_bytes()),
        fingerprint_hex = hex::encode(fingerprint)
    )
}
fn encoded_stored_receipt(receipt: &DaIngestReceipt, sequence: u64, version: u16) -> Vec<u8> {
    to_bytes(&persistence::StoredDaReceipt {
        version,
        sequence,
        receipt: receipt.clone(),
    })
    .expect("encode receipt")
}
fn receipt_spool_path(
    dir: &Path,
    receipt: &DaIngestReceipt,
    sequence: u64,
    fingerprint: [u8; 32],
) -> PathBuf {
    dir.join(receipt_spool_file_name(receipt, sequence, fingerprint))
}
fn canonical_receipt_spool_path(dir: &Path, receipt: &DaIngestReceipt, sequence: u64) -> PathBuf {
    receipt_spool_path(dir, receipt, sequence, receipt_fingerprint_bytes(receipt))
}
fn open_receipt_log(
    dir: &Path,
    cursor_store: &Arc<ReplayCursorStore>,
    signer: &KeyPair,
) -> eyre::Result<DaReceiptLog> {
    DaReceiptLog::open(
        dir.to_path_buf(),
        Arc::clone(cursor_store),
        signer.public_key().clone(),
    )
}
fn receipt_file_count(dir: &Path) -> usize {
    fs::read_dir(dir)
        .expect("read receipt directory")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("da-receipt-") && name.ends_with(".norito"))
        })
        .count()
}
fn temp_artifact_names(dir: &Path) -> Vec<String> {
    if !dir.exists() {
        return Vec::new();
    }
    fs::read_dir(dir)
        .expect("read artifact directory")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str().map(ToOwned::to_owned))
        .filter(|name| name.contains(".tmp-"))
        .collect()
}
#[test]
fn persist_da_receipt_writes_and_is_idempotent() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let signer = checked_random_keypair();
    let lane_id = LaneId::new(3);
    let receipt = test_receipt(&signer, lane_id, 5, 7, 0xAA);
    let fingerprint = receipt_fingerprint(&receipt);
    let first_path = persistence::persist_da_receipt(manifest_dir, &receipt, 7, &fingerprint)
        .expect("persist receipt");
    let first_path = first_path.expect("receipt path");
    let bytes = fs::read(&first_path).expect("read receipt file");
    let decoded =
        decode_from_bytes::<persistence::StoredDaReceipt>(&bytes).expect("decode stored receipt");
    assert_eq!(decoded.version, persistence::STORED_RECEIPT_VERSION);
    assert_eq!(decoded.sequence, 7);
    assert_eq!(decoded.receipt.manifest_hash, receipt.manifest_hash);
    let loaded = persistence::load_da_receipts(manifest_dir).expect("load receipts");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].sequence, 7);
    assert_eq!(loaded[0].receipt.manifest_hash, receipt.manifest_hash);
    let second_path = persistence::persist_da_receipt(manifest_dir, &receipt, 7, &fingerprint)
        .expect("persist again");
    let second_path = second_path.expect("receipt path");
    assert_eq!(first_path, second_path);
}
#[test]
fn persist_da_receipt_rejects_fingerprint_storage_ticket_mismatch() {
    let temp_dir = tempdir().expect("temp dir");
    let signer = checked_fixture_ed25519_keypair(0x60);
    let receipt = test_receipt(&signer, LaneId::new(3), 5, 7, 0xAA);
    let wrong_fingerprint = test_fingerprint(0xCC);
    let err = persistence::persist_da_receipt(temp_dir.path(), &receipt, 7, &wrong_fingerprint)
        .expect_err("fingerprint/storage-ticket mismatch must reject receipt persistence");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("does not match storage ticket"),
        "unexpected receipt persistence error: {err}"
    );
    assert_eq!(
        receipt_file_count(temp_dir.path()),
        0,
        "rejected receipt must not create a durable receipt file"
    );
}
#[cfg(unix)]
#[test]
fn persist_da_receipt_rejects_spool_dir_symlink() {
    use std::os::unix::fs::symlink;
    let temp_dir = tempdir().expect("temp dir");
    let target = temp_dir.path().join("receipt-write-target");
    fs::create_dir(&target).expect("create target directory");
    let spool = temp_dir.path().join("receipt-write-link");
    symlink(&target, &spool).expect("create receipt spool symlink");
    let signer = checked_fixture_ed25519_keypair(0x61);
    let receipt = test_receipt(&signer, LaneId::new(3), 5, 7, 0xAA);
    let fingerprint = receipt_fingerprint(&receipt);
    let err = persistence::persist_da_receipt(&spool, &receipt, 7, &fingerprint)
        .expect_err("symlinked receipt spool root must reject persistence");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("DA spool path"),
        "unexpected receipt persistence error: {err}"
    );
    assert!(
        fs::symlink_metadata(&spool)
            .expect("inspect spool symlink")
            .file_type()
            .is_symlink(),
        "failed persistence should leave spool symlink visible"
    );
    assert_eq!(
        receipt_file_count(&target),
        0,
        "symlink target must not receive receipt artifacts"
    );
}
#[test]
fn persist_da_receipt_converges_under_same_process_writers() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path().to_path_buf();
    let signer = checked_random_keypair();
    let lane_id = LaneId::new(3);
    let receipt = Arc::new(test_receipt(&signer, lane_id, 5, 7, 0xAA));
    let fingerprint = Arc::new(receipt_fingerprint(&receipt));
    let barrier = Arc::new(Barrier::new(4));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let manifest_dir = manifest_dir.clone();
            let receipt = Arc::clone(&receipt);
            let fingerprint = Arc::clone(&fingerprint);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                persistence::persist_da_receipt(&manifest_dir, &receipt, 7, &fingerprint)
                    .expect("concurrent receipt persist")
                    .expect("receipt path")
            })
        })
        .collect();
    let paths: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("writer thread"))
        .collect();
    let first = paths.first().expect("at least one writer");
    assert!(paths.iter().all(|path| path == first));
    assert_eq!(receipt_file_count(&manifest_dir), 1);
    assert!(
        temp_artifact_names(&manifest_dir).is_empty(),
        "concurrent receipt install should not leave temp artifacts"
    );
}
#[test]
fn load_da_receipts_rejects_unsupported_versions() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let signer = checked_random_keypair();
    let lane_id = LaneId::new(3);
    let receipt = test_receipt(&signer, lane_id, 5, 7, 0xAB);
    let bytes = encoded_stored_receipt(&receipt, 7, persistence::STORED_RECEIPT_VERSION + 1);
    let path = canonical_receipt_spool_path(manifest_dir, &receipt, 7);
    fs::write(&path, bytes).expect("write receipt");
    let err = persistence::load_da_receipts(manifest_dir)
        .expect_err("unsupported receipt versions must reject the receipt load");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}
#[test]
fn load_da_receipts_rejects_filename_body_mismatch() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let signer = checked_random_keypair();
    let lane_id = LaneId::new(3);
    let receipt = test_receipt(&signer, lane_id, 5, 7, 0xAC);
    let bytes = encoded_stored_receipt(&receipt, 7, persistence::STORED_RECEIPT_VERSION);
    let path = canonical_receipt_spool_path(manifest_dir, &receipt, 8);
    fs::write(&path, bytes).expect("write receipt");
    let err = persistence::load_da_receipts(manifest_dir)
        .expect_err("filename/body mismatches must reject the receipt load");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}
#[test]
fn load_da_receipts_rejects_filename_ticket_mismatch() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let signer = checked_random_keypair();
    let lane_id = LaneId::new(3);
    let receipt = test_receipt(&signer, lane_id, 5, 7, 0xAD);
    let bytes = encoded_stored_receipt(&receipt, 7, persistence::STORED_RECEIPT_VERSION);
    let mut filename_receipt = receipt;
    filename_receipt.storage_ticket = StorageTicketId::new([0x99; 32]);
    let path = canonical_receipt_spool_path(manifest_dir, &filename_receipt, 7);
    fs::write(&path, bytes).expect("write receipt");
    let err = persistence::load_da_receipts(manifest_dir)
        .expect_err("filename/body ticket mismatches must reject the receipt load");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}
#[test]
fn load_da_receipts_rejects_receipt_shaped_directory() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let signer = checked_random_keypair();
    let receipt = test_receipt(&signer, LaneId::new(3), 5, 7, 0xAE);
    let first_path = canonical_receipt_spool_path(manifest_dir, &receipt, 7);
    let later_path = canonical_receipt_spool_path(manifest_dir, &receipt, 8);
    fs::create_dir(&later_path).expect("create later receipt-shaped directory");
    fs::create_dir(&first_path).expect("create first receipt-shaped directory");
    let err = persistence::load_da_receipts(manifest_dir)
        .expect_err("receipt-shaped directory must reject receipt loading");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    let message = err.to_string();
    assert!(
        message.contains("is not a regular file"),
        "unexpected receipt load error: {err}"
    );
    assert!(
        message.contains(
            first_path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("receipt fixture path is UTF-8")
        ),
        "receipt load should reject the first canonical path: {message}"
    );
    assert!(
        !message.contains(
            later_path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("receipt fixture path is UTF-8")
        ),
        "receipt load should stop at the first canonical path: {message}"
    );
}
#[cfg(unix)]
#[test]
fn load_da_receipts_rejects_spool_dir_symlink() {
    use std::os::unix::fs::symlink;
    let temp_dir = tempdir().expect("temp dir");
    let target = temp_dir.path().join("receipt-spool-target");
    fs::create_dir(&target).expect("create target directory");
    let spool = temp_dir.path().join("receipt-spool-link");
    symlink(&target, &spool).expect("create receipt spool symlink");
    let err = persistence::load_da_receipts(&spool)
        .expect_err("symlinked DA receipt spool root must reject");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("DA spool path"),
        "unexpected receipt load error: {err}"
    );
    assert!(
        fs::symlink_metadata(&spool)
            .expect("inspect spool symlink")
            .file_type()
            .is_symlink(),
        "failed load should leave spool symlink visible"
    );
    assert!(
        target.exists(),
        "spool symlink target should not be removed"
    );
}
#[test]
fn load_da_receipts_rejects_same_manifest_duplicate_with_different_receipt() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let signer = checked_random_keypair();
    let receipt = test_receipt(&signer, LaneId::new(3), 5, 7, 0xAF);
    let mut conflicting = test_receipt(&signer, LaneId::new(3), 5, 7, 0xB0);
    conflicting.manifest_hash = receipt.manifest_hash;
    let unsigned = persistence::unsigned_receipt_bytes(&conflicting, 7).expect("unsigned bytes");
    conflicting.operator_signature = checked_signature(signer.private_key(), &unsigned);
    for receipt in [&receipt, &conflicting] {
        let bytes = encoded_stored_receipt(receipt, 7, persistence::STORED_RECEIPT_VERSION);
        let path = canonical_receipt_spool_path(manifest_dir, receipt, 7);
        fs::write(path, bytes).expect("write duplicate receipt");
    }
    let err = persistence::load_da_receipts(manifest_dir)
        .expect_err("conflicting duplicate receipts must reject the receipt load");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("conflicting duplicate DA receipt"),
        "unexpected receipt load error: {err}"
    );
}
#[test]
fn load_da_receipts_rejects_filename_fingerprint_mismatch() {
    let temp_dir = tempdir().expect("temp dir");
    let manifest_dir = temp_dir.path();
    let signer = checked_random_keypair();
    let receipt = test_receipt(&signer, LaneId::new(3), 5, 7, 0xB1);
    let bytes = encoded_stored_receipt(&receipt, 7, persistence::STORED_RECEIPT_VERSION);
    for fingerprint in [[0xC2; 32], [0xC3; 32]] {
        let path = receipt_spool_path(manifest_dir, &receipt, 7, fingerprint);
        fs::write(path, &bytes).expect("write duplicate receipt");
    }
    let err = persistence::load_da_receipts(manifest_dir)
        .expect_err("filename fingerprint mismatch must reject the receipt load");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("mismatches body storage ticket"),
        "unexpected receipt load error: {err}"
    );
}
#[cfg(unix)]
#[test]
fn da_receipt_log_open_rejects_spool_dir_symlink() {
    use std::os::unix::fs::symlink;
    let temp_dir = tempdir().expect("temp dir");
    let target = temp_dir.path().join("receipt-log-target");
    fs::create_dir(&target).expect("create target directory");
    let spool = temp_dir.path().join("receipt-log-link");
    symlink(&target, &spool).expect("create receipt log symlink");
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_fixture_ed25519_keypair(0x62);
    let err = match open_receipt_log(&spool, &cursor_store, &signer) {
        Ok(_) => panic!("symlinked DA receipt log root must reject"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("DA spool path")
            && format!("{err:?}").contains("not a directory"),
        "unexpected receipt log open error: {err:?}"
    );
    assert!(
        fs::symlink_metadata(&spool)
            .expect("inspect spool symlink")
            .file_type()
            .is_symlink(),
        "failed open should leave spool symlink visible"
    );
    assert!(
        target.exists(),
        "spool symlink target should not be removed"
    );
}
#[test]
fn da_receipt_log_requires_zero_for_a_fresh_lane_epoch() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(4), 8);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_random_keypair();
    let log = open_receipt_log(temp_dir.path(), &cursor_store, &signer).unwrap();

    for sequence in [1, u64::MAX - 1] {
        let receipt = test_receipt(
            &signer,
            lane_epoch.lane_id,
            lane_epoch.epoch,
            sequence,
            0xC0,
        );
        assert_eq!(
            log.append(lane_epoch, sequence, receipt, test_fingerprint(0xC0))
                .unwrap(),
            ReceiptInsertOutcome::SequenceGap {
                expected_next: 0,
                observed: sequence,
            }
        );
    }
    assert_eq!(receipt_file_count(temp_dir.path()), 0);
    assert!(cursor_store.highest_sequences().is_empty());

    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 0, 0xC1);
    assert!(matches!(
        log.append(lane_epoch, 0, receipt, test_fingerprint(0xC1))
            .unwrap(),
        ReceiptInsertOutcome::Stored { .. }
    ));
}
#[test]
fn da_receipt_log_enforces_ordering_and_dedupe() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(4), 9);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_random_keypair();
    let log = open_receipt_log(temp_dir.path(), &cursor_store, &signer).unwrap();
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 0, 1);
    assert!(matches!(
        log.append(lane_epoch, 0, receipt.clone(), test_fingerprint(1))
            .unwrap(),
        ReceiptInsertOutcome::Stored { .. }
    ));
    assert_eq!(
        receipt_file_count(temp_dir.path()),
        1,
        "stored receipt should create one durable receipt file"
    );
    assert!(matches!(
        log.append(lane_epoch, 0, receipt.clone(), test_fingerprint(1))
            .unwrap(),
        ReceiptInsertOutcome::Duplicate { .. }
    ));
    assert_eq!(
        receipt_file_count(temp_dir.path()),
        1,
        "duplicate receipt must not create another durable receipt file"
    );
    let wrong_fingerprint = test_fingerprint(0xD0);
    let err = log
        .append(lane_epoch, 0, receipt.clone(), wrong_fingerprint)
        .expect_err("wrong-fingerprint duplicate must be rejected before durable lookup");
    assert!(
        format!("{err:?}").contains("does not match storage ticket"),
        "unexpected wrong-fingerprint duplicate error: {err:?}"
    );
    assert_eq!(
        receipt_file_count(temp_dir.path()),
        1,
        "wrong-fingerprint duplicate must not create another durable receipt file"
    );
    let mut receipt_conflict = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 0, 0xD1);
    receipt_conflict.manifest_hash = receipt.manifest_hash;
    let unsigned =
        persistence::unsigned_receipt_bytes(&receipt_conflict, 0).expect("unsigned bytes");
    receipt_conflict.operator_signature = checked_signature(signer.private_key(), &unsigned);
    assert!(matches!(
        log.append(lane_epoch, 0, receipt_conflict, test_fingerprint(0xD1))
            .unwrap(),
        ReceiptInsertOutcome::ReceiptConflict { .. }
    ));
    assert_eq!(
        receipt_file_count(temp_dir.path()),
        1,
        "receipt-evidence conflict must not create another durable receipt file"
    );
    let conflict = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 0, 2);
    assert!(matches!(
        log.append(lane_epoch, 0, conflict, test_fingerprint(2))
            .unwrap(),
        ReceiptInsertOutcome::ManifestConflict { .. }
    ));
    assert_eq!(
        receipt_file_count(temp_dir.path()),
        1,
        "conflicting receipt must not be written before validation"
    );
    let gap = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 2, 4);
    assert!(matches!(
        log.append(lane_epoch, 2, gap, test_fingerprint(4)).unwrap(),
        ReceiptInsertOutcome::SequenceGap {
            expected_next: 1,
            observed: 2
        }
    ));
    assert_eq!(
        receipt_file_count(temp_dir.path()),
        1,
        "gap receipt must not be written before validation"
    );
    let second = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 5);
    assert!(matches!(
        log.append(lane_epoch, 1, second, test_fingerprint(5))
            .unwrap(),
        ReceiptInsertOutcome::Stored { .. }
    ));
    assert_eq!(
        receipt_file_count(temp_dir.path()),
        2,
        "contiguous receipt should still be accepted after a rejected gap"
    );
    let stale = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 0, 3);
    assert!(matches!(
        log.append(lane_epoch, 0, stale, test_fingerprint(3))
            .unwrap(),
        ReceiptInsertOutcome::StaleSequence { highest: 1 }
    ));
    assert_eq!(
        receipt_file_count(temp_dir.path()),
        2,
        "stale receipt must not be written before validation"
    );
}
#[test]
fn da_receipt_log_recovery_rejects_filename_fingerprint_mismatch_in_canonical_path_order() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(4), 29);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_fixture_ed25519_keypair(0x63);
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 0xE1);
    let bytes = encoded_stored_receipt(&receipt, 1, persistence::STORED_RECEIPT_VERSION);
    let higher_path = receipt_spool_path(temp_dir.path(), &receipt, 2, [0xE3; 32]);
    let lower_path = receipt_spool_path(temp_dir.path(), &receipt, 1, [0xE2; 32]);
    fs::write(&higher_path, &bytes).expect("write higher-fingerprint receipt");
    fs::write(&lower_path, &bytes).expect("write lower-fingerprint receipt");
    let err = match open_receipt_log(temp_dir.path(), &cursor_store, &signer) {
        Ok(_) => panic!("filename fingerprint mismatch must reject durable recovery"),
        Err(err) => err,
    };
    let message = format!("{err:?}");
    assert!(
        message.contains("mismatches body storage ticket"),
        "unexpected recovery error: {message}"
    );
    message
        .find(&lower_path.display().to_string())
        .expect("recovery error should include the lower canonical path");
    assert!(
        !message.contains(&higher_path.display().to_string()),
        "recovery should stop at the first canonical mismatched path: {message}"
    );
    assert!(
        cursor_store.highest_sequences().is_empty(),
        "failed recovery must not seed receipt cursors"
    );
}
#[test]
fn da_receipt_log_rejected_append_does_not_advance_replay_cursor() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(4), 19);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_fixture_ed25519_keypair(0x64);
    let log = open_receipt_log(temp_dir.path(), &cursor_store, &signer).unwrap();
    let first = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 0, 0xE1);
    assert!(matches!(
        log.append(lane_epoch, 0, first, test_fingerprint(0xE1))
            .unwrap(),
        ReceiptInsertOutcome::Stored { .. }
    ));
    assert_replay_cursor_sequences(&cursor_store, &[(lane_epoch, 0)]);
    let gap = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 2, 0xE3);
    assert!(matches!(
        log.append(lane_epoch, 2, gap, test_fingerprint(0xE3))
            .unwrap(),
        ReceiptInsertOutcome::SequenceGap {
            expected_next: 1,
            observed: 2
        }
    ));
    assert_replay_cursor_sequences(&cursor_store, &[(lane_epoch, 0)]);
    let second = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 0xE2);
    assert!(matches!(
        log.append(lane_epoch, 1, second, test_fingerprint(0xE2))
            .unwrap(),
        ReceiptInsertOutcome::Stored { .. }
    ));
    assert_replay_cursor_sequences(&cursor_store, &[(lane_epoch, 1)]);
}
#[test]
fn da_receipt_log_recovers_after_cursor_failure_post_file_write() {
    let receipt_dir = tempdir().expect("receipt dir");
    let cursor_dir = tempdir().expect("cursor dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(4), 20);
    let cursor_store =
        Arc::new(ReplayCursorStore::empty(cursor_dir.path().to_path_buf()).expect("cursor store"));
    let signer = checked_fixture_ed25519_keypair(0x65);
    let log = open_receipt_log(receipt_dir.path(), &cursor_store, &signer).unwrap();
    let main_path = replay_cursor_main_path(cursor_dir.path());
    let tmp_path = persistence::replay_cursor_temp_path(&main_path);
    fs::create_dir(&tmp_path).expect("block cursor temp path");
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 0, 0xF1);
    let fingerprint = test_fingerprint(0xF1);
    let err = log
        .append(lane_epoch, 0, receipt.clone(), fingerprint)
        .expect_err("blocked cursor persistence should fail append after file write");
    assert!(
        format!("{err:?}").contains("failed to persist receipt cursor"),
        "unexpected cursor persistence error: {err:?}"
    );
    assert_eq!(
        receipt_file_count(receipt_dir.path()),
        1,
        "receipt file is durable even when cursor persistence fails"
    );
    assert!(
        log.receipts_for(lane_epoch).is_empty(),
        "failed append must not update the in-memory receipt index"
    );
    assert_replay_cursor_sequences(&cursor_store, &[]);
    fs::remove_dir(&tmp_path).expect("unblock cursor temp path");
    assert!(matches!(
        log.append(lane_epoch, 0, receipt, fingerprint).unwrap(),
        ReceiptInsertOutcome::Stored {
            cursor_advanced: true
        }
    ));
    assert_eq!(
        receipt_file_count(receipt_dir.path()),
        1,
        "retry should adopt the existing receipt file without duplicating it"
    );
    assert_eq!(log.receipts_for(lane_epoch).len(), 1);
    assert_replay_cursor_sequences(&cursor_store, &[(lane_epoch, 0)]);
}
#[test]
fn da_receipt_log_rejects_conflicting_preexisting_receipt_without_cursor_advance() {
    let receipt_dir = tempdir().expect("receipt dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(4), 21);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_fixture_ed25519_keypair(0x66);
    let log = open_receipt_log(receipt_dir.path(), &cursor_store, &signer).unwrap();
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 0, 0xF2);
    let fingerprint = test_fingerprint(0xF2);
    let poisoned_path = canonical_receipt_spool_path(receipt_dir.path(), &receipt, 0);
    fs::write(&poisoned_path, b"poison-receipt").expect("seed poisoned receipt");
    let err = log
        .append(lane_epoch, 0, receipt.clone(), fingerprint)
        .expect_err("conflicting preexisting receipt file must reject append");
    assert!(
        format!("{err:?}").contains("DA receipt artifact already exists"),
        "unexpected preexisting receipt error: {err:?}"
    );
    assert_eq!(
        fs::read(&poisoned_path).expect("read poisoned receipt"),
        b"poison-receipt",
        "conflicting receipt file must be preserved for operator repair"
    );
    assert_eq!(receipt_file_count(receipt_dir.path()), 1);
    assert!(
        log.receipts_for(lane_epoch).is_empty(),
        "failed append must not update the in-memory receipt index"
    );
    assert_replay_cursor_sequences(&cursor_store, &[]);
    fs::remove_file(&poisoned_path).expect("remove poisoned receipt");
    assert!(matches!(
        log.append(lane_epoch, 0, receipt, fingerprint).unwrap(),
        ReceiptInsertOutcome::Stored {
            cursor_advanced: true
        }
    ));
    assert_eq!(receipt_file_count(receipt_dir.path()), 1);
    assert_eq!(log.receipts_for(lane_epoch).len(), 1);
    assert_replay_cursor_sequences(&cursor_store, &[(lane_epoch, 0)]);
}
#[test]
fn da_receipt_log_in_memory_append_fails_closed() {
    let lane_epoch = LaneEpoch::new(LaneId::new(4), 10);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_random_keypair();
    let log = DaReceiptLog::in_memory(Arc::clone(&cursor_store), signer.public_key().clone());
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 0xA1);
    let err = log
        .append(lane_epoch, 1, receipt, test_fingerprint(0xA1))
        .expect_err("in-memory receipt logs must not acknowledge DA ingest appends");
    assert!(
        format!("{err:?}").contains("not durable"),
        "unexpected in-memory append error: {err:?}"
    );
    assert!(
        log.receipts_for(lane_epoch).is_empty(),
        "failed in-memory append must not update receipt-log memory"
    );
    assert!(
        cursor_store.highest_sequences().is_empty(),
        "failed in-memory append must not advance replay cursors"
    );
    let err = log
        .receipt_for_duplicate(lane_epoch, 1, test_fingerprint(0xA1))
        .expect_err("in-memory duplicate lookup must fail closed");
    assert!(
        format!("{err:?}").contains("not durable"),
        "unexpected in-memory duplicate lookup error: {err:?}"
    );
}
#[cfg(unix)]
#[test]
fn da_receipt_log_duplicate_reload_rejects_receipt_symlink_replacement() {
    use std::os::unix::fs::symlink;
    let receipt_dir = tempdir().expect("receipt dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(4), 10);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_fixture_ed25519_keypair(0x67);
    let log =
        open_receipt_log(receipt_dir.path(), &cursor_store, &signer).expect("open receipt log");
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 0, 0xA2);
    let fingerprint = test_fingerprint(0xA2);
    assert!(matches!(
        log.append(lane_epoch, 0, receipt.clone(), fingerprint)
            .expect("append receipt"),
        ReceiptInsertOutcome::Stored { .. }
    ));
    let receipt_path = canonical_receipt_spool_path(receipt_dir.path(), &receipt, 0);
    let target_path = receipt_dir.path().join("receipt-symlink-target.norito");
    fs::write(
        &target_path,
        fs::read(&receipt_path).expect("read stored receipt"),
    )
    .expect("write receipt symlink target");
    fs::remove_file(&receipt_path).expect("remove stored receipt");
    symlink(&target_path, &receipt_path).expect("replace receipt with symlink");
    let err = log
        .receipt_for_duplicate(lane_epoch, 0, fingerprint)
        .expect_err("symlinked durable receipt must fail duplicate reload");
    assert!(
        format!("{err:?}").contains("not a regular file"),
        "unexpected duplicate receipt reload error: {err:?}"
    );
    assert!(
        fs::symlink_metadata(&receipt_path)
            .expect("inspect receipt symlink")
            .file_type()
            .is_symlink(),
        "failed duplicate reload should leave receipt symlink visible"
    );
    assert!(
        target_path.exists(),
        "receipt symlink target should remain for operator repair"
    );
}
#[test]
fn duplicate_da_ingest_reuses_durable_artifacts_after_timestamp_retry() {
    let temp_dir = tempdir().expect("temp dir");
    let spool_dir = temp_dir.path();
    let context = zero_sequence_manifest_context_for(BlobClass::NexusLaneSidecar);
    let request = context.request;
    let manifest = context.artifacts;
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encryption");
    let rent_policy = DaRentPolicyV1::default();
    let retry_manifest = resolve_manifest(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_001_111,
        &rent_policy,
    )
    .expect("retry manifest");
    assert_eq!(retry_manifest.storage_ticket, manifest.storage_ticket);
    assert_eq!(retry_manifest.fingerprint, manifest.fingerprint);
    assert_ne!(
        retry_manifest.manifest_hash, manifest.manifest_hash,
        "retry timestamp should change the timestamped manifest hash"
    );
    persistence::persist_manifest_for_sorafs(
        spool_dir,
        &manifest.encoded,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist manifest")
    .expect("manifest path");
    let durable_scope =
        build_da_pin_scope(&request, manifest.storage_ticket, manifest.manifest_hash)
            .expect("build durable pin scope");
    persistence::persist_da_pin_scope(
        spool_dir,
        &durable_scope,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist pin scope")
    .expect("pin-scope path");
    let durable_pin_intent = signed_pin_intent_for_manifest(&request, &manifest);
    persistence::persist_da_pin_intent(
        spool_dir,
        &durable_pin_intent,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist pin intent")
    .expect("pin intent path");
    let pdp_commitment = compute_pdp_commitment(
        &manifest.manifest_hash,
        &manifest.manifest,
        &chunk_store,
        canonical.as_slice(),
        1_701_000_999,
    )
    .expect("PDP commitment");
    let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode PDP commitment");
    persistence::persist_pdp_commitment(
        spool_dir,
        &pdp_commitment,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist PDP")
    .expect("PDP path");
    let signer = checked_random_keypair();
    let lane_epoch = LaneEpoch::new(request.lane_id, request.epoch);
    let receipt = build_receipt(
        &signer,
        &request,
        1_701_000_999,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        pdp_bytes.clone(),
        manifest.manifest.rent_quote.clone(),
        stripe_layout_from_manifest(&manifest.manifest),
    )
    .expect("build receipt");
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let log = open_receipt_log(spool_dir, &cursor_store, &signer).expect("open receipt log");
    assert!(matches!(
        log.append(
            lane_epoch,
            request.sequence,
            receipt.clone(),
            manifest.fingerprint
        )
        .expect("append receipt"),
        ReceiptInsertOutcome::Stored { .. }
    ));
    let duplicate = load_duplicate_da_artifacts(
        &log,
        spool_dir,
        lane_epoch,
        request.sequence,
        &retry_manifest.storage_ticket,
        retry_manifest.fingerprint,
        &request,
    )
    .expect("load duplicate artifacts");
    assert_eq!(duplicate.receipt, receipt);
    assert_eq!(duplicate.pdp_commitment_bytes, pdp_bytes);
    assert_eq!(
        receipt_file_count(spool_dir),
        1,
        "duplicate artifact recovery must not write another receipt"
    );
    let reopened_cursor = Arc::new(ReplayCursorStore::in_memory());
    let reopened_log = DaReceiptLog::open(
        spool_dir.to_path_buf(),
        reopened_cursor,
        signer.public_key().clone(),
    )
    .expect("reopen durable receipt log");
    let recovered_after_restart = load_duplicate_da_artifacts_if_receipt_present(
        &reopened_log,
        spool_dir,
        lane_epoch,
        request.sequence,
        &retry_manifest.storage_ticket,
        retry_manifest.fingerprint,
        &request,
    )
    .expect("check durable duplicate after restart")
    .expect("durable duplicate should be present after restart");
    assert_eq!(recovered_after_restart.receipt, receipt);
    assert_eq!(recovered_after_restart.pdp_commitment_bytes, pdp_bytes);
}

#[test]
fn duplicate_retry_finalizes_only_after_exact_pin_scope_signature() {
    let temp_dir = tempdir().expect("temp dir");
    let context = zero_sequence_manifest_context_for(BlobClass::TaikaiSegment);
    let mut request = context.request;
    let manifest = context.artifacts;
    let scope = build_da_pin_scope(&request, manifest.storage_ticket, manifest.manifest_hash)
        .expect("build exact pin scope");
    let operator = checked_fixture_ed25519_keypair(0x69);
    let receipt = build_receipt(
        &operator,
        &request,
        manifest.manifest.issued_at_unix.max(1),
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        Vec::new(),
        manifest.manifest.rent_quote.clone(),
        stripe_layout_from_manifest(&manifest.manifest),
    )
    .expect("build pending receipt fixture");
    let lane_epoch = LaneEpoch::new(request.lane_id, request.epoch);
    let artifacts = DuplicateDaArtifacts {
        receipt_path: temp_dir.path().join("receipt.norito"),
        receipt,
        pdp_commitment_bytes: Vec::new(),
        pin_scope: scope.clone(),
        pin_intent: None,
    };

    let (pending, finalized) = finalize_duplicate_da_pin_intent(
        temp_dir.path(),
        &request,
        lane_epoch,
        manifest.fingerprint,
        artifacts,
    )
    .expect("unsigned scope remains pending");
    assert!(!finalized);
    assert!(pending.pin_intent.is_none());
    assert!(!taikai_ready_path(temp_dir.path(), &request, &manifest).exists());

    request
        .try_add_pin_scope_signature(
            &scope,
            &checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519),
        )
        .expect("authorize exact durable pin scope");
    let (finalized_artifacts, finalized) = finalize_duplicate_da_pin_intent(
        temp_dir.path(),
        &request,
        lane_epoch,
        manifest.fingerprint,
        pending,
    )
    .expect("exact producer scope finalizes");
    assert!(finalized);
    assert!(finalized_artifacts.pin_intent.is_some());
    assert!(taikai_ready_path(temp_dir.path(), &request, &manifest).exists());
    persistence::load_da_pin_intent(
        temp_dir.path(),
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("producer-authorized pin intent must be durable");
}

#[test]
fn submitted_pin_scope_witness_cannot_authorize_a_different_durable_scope() {
    let context = zero_sequence_manifest_context_for(BlobClass::NexusLaneSidecar);
    let request = context.request;
    let manifest = context.artifacts;
    let exact_scope = build_da_pin_scope(&request, manifest.storage_ticket, manifest.manifest_hash)
        .expect("build exact durable pin scope");
    let signer = checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519);

    for forged_scope in [
        DaPinScopeV1 {
            storage_ticket: StorageTicketId::new([0xA1; 32]),
            ..exact_scope.clone()
        },
        DaPinScopeV1 {
            manifest_hash: ManifestDigest::new([0xA2; 32]),
            ..exact_scope.clone()
        },
        DaPinScopeV1 {
            alias: Some("forged-alias".to_owned()),
            ..exact_scope.clone()
        },
    ] {
        let mut forged_request = request.clone();
        forged_request
            .try_add_pin_scope_signature(&forged_scope, &signer)
            .expect("sign forged pin scope fixture");
        let error = submitted_pin_scope_authorization(&forged_request, exact_scope.clone())
            .expect_err("a witness over another scope must reject");
        assert_eq!(error.0, StatusCode::UNAUTHORIZED);
    }
}

fn persist_completed_duplicate_fixture(
    spool_dir: &Path,
    request: &DaIngestRequest,
    manifest: &ManifestArtifacts,
) -> DaReceiptLog {
    let canonical = normalize_payload(request).expect("normalize duplicate fixture payload");
    let chunk_store = build_chunk_store(request, canonical.as_slice());
    persistence::persist_manifest_for_sorafs(
        spool_dir,
        &manifest.encoded,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist duplicate fixture manifest")
    .expect("duplicate fixture manifest path");
    let pin_scope = build_da_pin_scope(request, manifest.storage_ticket, manifest.manifest_hash)
        .expect("build duplicate fixture pin scope");
    persistence::persist_da_pin_scope(
        spool_dir,
        &pin_scope,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist duplicate fixture pin scope")
    .expect("duplicate fixture pin-scope path");
    let pin_intent = signed_pin_intent_for_manifest(request, manifest);
    persistence::persist_da_pin_intent(
        spool_dir,
        &pin_intent,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist duplicate fixture pin intent")
    .expect("duplicate fixture pin-intent path");
    let sealed_at_unix = manifest.manifest.issued_at_unix.max(1);
    let pdp_commitment = compute_pdp_commitment(
        &manifest.manifest_hash,
        &manifest.manifest,
        &chunk_store,
        canonical.as_slice(),
        sealed_at_unix,
    )
    .expect("compute duplicate fixture PDP commitment");
    let pdp_bytes =
        encode_pdp_commitment_bytes(&pdp_commitment).expect("encode duplicate fixture PDP");
    persistence::persist_pdp_commitment(
        spool_dir,
        &pdp_commitment,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
    )
    .expect("persist duplicate fixture PDP")
    .expect("duplicate fixture PDP path");
    let signer = checked_fixture_ed25519_keypair(0x6A);
    let receipt = build_receipt(
        &signer,
        request,
        sealed_at_unix,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        pdp_bytes,
        manifest.manifest.rent_quote.clone(),
        stripe_layout_from_manifest(&manifest.manifest),
    )
    .expect("build duplicate fixture receipt");
    let lane_epoch = LaneEpoch::new(request.lane_id, request.epoch);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let log = open_receipt_log(spool_dir, &cursor_store, &signer)
        .expect("open duplicate fixture receipt log");
    assert!(matches!(
        log.append(lane_epoch, request.sequence, receipt, manifest.fingerprint)
            .expect("append duplicate fixture receipt"),
        ReceiptInsertOutcome::Stored { .. }
    ));
    log
}

fn load_duplicate_da_artifacts_and_publish_taikai_ready(
    receipt_log: &DaReceiptLog,
    spool_dir: &Path,
    request: &DaIngestRequest,
    manifest: &ManifestArtifacts,
    lane_epoch: LaneEpoch,
) -> Result<DuplicateDaArtifacts, DuplicateDaArtifactsError> {
    let artifacts = load_duplicate_da_artifacts(
        receipt_log,
        spool_dir,
        lane_epoch,
        request.sequence,
        &manifest.storage_ticket,
        manifest.fingerprint,
        request,
    )?;
    finalize_duplicate_da_pin_intent(
        spool_dir,
        request,
        lane_epoch,
        manifest.fingerprint,
        artifacts,
    )
    .map(|(artifacts, _)| artifacts)
}

fn resign_duplicate_fixture_request(request: &mut DaIngestRequest) {
    let keypair = checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519);
    let digest = request.signing_digest();
    request.signatures[0].signature = checked_signature(keypair.private_key(), &digest);
}

fn taikai_ready_path(
    spool_dir: &Path,
    request: &DaIngestRequest,
    manifest: &ManifestArtifacts,
) -> PathBuf {
    spool_dir.join(TAIKAI_SPOOL_SUBDIR).join(format!(
        "{TAIKAI_ANCHOR_READY_PREFIX}{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket}-{fingerprint}{TAIKAI_ANCHOR_READY_SUFFIX}",
        lane = request.lane_id.as_u32(),
        epoch = request.epoch,
        sequence = request.sequence,
        ticket = hex::encode(manifest.storage_ticket.as_ref()),
        fingerprint = hex::encode(manifest.fingerprint.as_bytes()),
    ))
}

#[test]
fn completed_taikai_duplicate_rejects_changed_stripped_ssm_identity() {
    let temp_dir = tempdir().expect("temp dir");
    let context = zero_sequence_manifest_context_for(BlobClass::TaikaiSegment);
    let mut request = context.request;
    let manifest = context.artifacts;
    request.norito_manifest = Some(manifest.encoded.clone());
    request.metadata.items.push(MetadataEntry::new(
        taikai::META_TAIKAI_SSM,
        b"first signed Taikai manifest".to_vec(),
        MetadataVisibility::Public,
    ));
    resign_duplicate_fixture_request(&mut request);
    let receipt_log = persist_completed_duplicate_fixture(temp_dir.path(), &request, &manifest);
    load_duplicate_da_artifacts(
        &receipt_log,
        temp_dir.path(),
        LaneEpoch::new(request.lane_id, request.epoch),
        request.sequence,
        &manifest.storage_ticket,
        manifest.fingerprint,
        &request,
    )
    .expect("matching completed Taikai duplicate must recover");

    let mut retry = request.clone();
    retry
        .metadata
        .items
        .iter_mut()
        .find(|entry| entry.key == taikai::META_TAIKAI_SSM)
        .expect("SSM metadata entry")
        .value = b"different signed Taikai manifest".to_vec();
    resign_duplicate_fixture_request(&mut retry);
    assert_ne!(request.signing_digest(), retry.signing_digest());
    let err = load_duplicate_da_artifacts_and_publish_taikai_ready(
        &receipt_log,
        temp_dir.path(),
        &retry,
        &manifest,
        LaneEpoch::new(retry.lane_id, retry.epoch),
    )
    .expect_err("changed stripped SSM must conflict with the completed request identity");
    assert!(matches!(err, DuplicateDaArtifactsError::Conflict(_)));
    assert!(
        !taikai_ready_path(temp_dir.path(), &retry, &manifest).exists(),
        "identity-conflicting duplicate must not publish Taikai readiness"
    );
}

#[test]
fn completed_taikai_duplicate_rejects_changed_caller_manifest_timestamp() {
    let temp_dir = tempdir().expect("temp dir");
    let context = zero_sequence_manifest_context_for(BlobClass::TaikaiSegment);
    let mut request = context.request;
    let manifest = context.artifacts;
    request.norito_manifest = Some(manifest.encoded.clone());
    resign_duplicate_fixture_request(&mut request);
    let receipt_log = persist_completed_duplicate_fixture(temp_dir.path(), &request, &manifest);

    let mut retry = request.clone();
    let mut changed_manifest = manifest.manifest.clone();
    changed_manifest.issued_at_unix = changed_manifest.issued_at_unix.saturating_add(1);
    retry.norito_manifest = Some(to_bytes(&changed_manifest).expect("encode changed manifest"));
    resign_duplicate_fixture_request(&mut retry);
    assert_ne!(request.signing_digest(), retry.signing_digest());
    let err = load_duplicate_da_artifacts_and_publish_taikai_ready(
        &receipt_log,
        temp_dir.path(),
        &retry,
        &manifest,
        LaneEpoch::new(retry.lane_id, retry.epoch),
    )
    .expect_err("changed caller manifest timestamp must conflict with completed identity");
    assert!(matches!(err, DuplicateDaArtifactsError::Conflict(_)));
    assert!(
        !taikai_ready_path(temp_dir.path(), &retry, &manifest).exists(),
        "identity-conflicting duplicate must not publish Taikai readiness"
    );
}

#[test]
fn completed_taikai_duplicate_fails_closed_on_corrupt_pin_intent() {
    let temp_dir = tempdir().expect("temp dir");
    let context = zero_sequence_manifest_context_for(BlobClass::TaikaiSegment);
    let request = context.request;
    let manifest = context.artifacts;
    let receipt_log = persist_completed_duplicate_fixture(temp_dir.path(), &request, &manifest);
    let pin_path = spool_artifact_path_for_key(
        temp_dir.path(),
        "da-pin-intent-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        *manifest.fingerprint.as_bytes(),
    );
    fs::write(&pin_path, b"corrupt durable pin intent").expect("corrupt pin intent");
    let err = load_duplicate_da_artifacts_and_publish_taikai_ready(
        &receipt_log,
        temp_dir.path(),
        &request,
        &manifest,
        LaneEpoch::new(request.lane_id, request.epoch),
    )
    .expect_err("corrupt durable pin intent must fail closed");
    assert!(matches!(err, DuplicateDaArtifactsError::Internal(_)));
    assert!(
        !taikai_ready_path(temp_dir.path(), &request, &manifest).exists(),
        "corrupt duplicate identity must not publish Taikai readiness"
    );
}

#[test]
fn completed_duplicate_identity_conflict_maps_to_http_conflict() {
    let error = duplicate_da_artifacts_response_error(
        DuplicateDaArtifactsError::Conflict("identity mismatch".to_owned()),
        "duplicate recovery",
        ResponseFormat::Json,
    );
    let response = axum::response::IntoResponse::into_response(error);
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let error = duplicate_da_artifacts_response_error(
        DuplicateDaArtifactsError::Internal(eyre!("corrupt durable artifact")),
        "duplicate recovery",
        ResponseFormat::Json,
    );
    let response = axum::response::IntoResponse::into_response(error);
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn duplicate_taikai_ingest_does_not_publish_readiness_before_receipt_validation() {
    let temp_dir = tempdir().expect("temp dir");
    let spool_dir = temp_dir.path();
    let context = sample_manifest_context_for(BlobClass::TaikaiSegment);
    let request = context.request;
    let manifest = context.artifacts;
    taikai_ingest::persist_envelope(
        spool_dir,
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        &manifest.fingerprint,
        b"envelope",
    )
    .expect("persist envelope fixture")
    .expect("envelope path");
    let lane_epoch = LaneEpoch::new(request.lane_id, request.epoch);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_random_keypair();
    let receipt_log =
        open_receipt_log(spool_dir, &cursor_store, &signer).expect("open receipt log");
    load_duplicate_da_artifacts_and_publish_taikai_ready(
        &receipt_log,
        spool_dir,
        &request,
        &manifest,
        lane_epoch,
    )
    .expect_err("missing durable receipt artifacts must reject duplicate recovery");
    let ready_path = taikai_ready_path(spool_dir, &request, &manifest);
    assert!(
        !ready_path.exists(),
        "an invalid in-memory duplicate must not become visible to the anchor worker"
    );
}
#[test]
fn da_receipt_log_rejects_receipt_hash_mismatch_against_ticket_manifest_on_open() {
    let temp_dir = tempdir().expect("temp dir");
    let spool_dir = temp_dir.path();
    let context = sample_manifest_context_for(BlobClass::NexusLaneSidecar);
    let request = context.request;
    let manifest = context.artifacts;
    let canonical = normalize_payload(&request).expect("normalize payload");
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let pdp_commitment = compute_pdp_commitment(
        &manifest.manifest_hash,
        &manifest.manifest,
        &chunk_store,
        canonical.as_slice(),
        1_701_000_999,
    )
    .expect("PDP commitment");
    let pdp_bytes = encode_pdp_commitment_bytes(&pdp_commitment).expect("encode PDP commitment");
    let signer = checked_fixture_ed25519_keypair(0x68);
    let receipt = build_receipt(
        &signer,
        &request,
        1_701_000_999,
        manifest.blob_hash,
        manifest.chunk_root,
        manifest.manifest_hash,
        manifest.storage_ticket,
        pdp_bytes,
        manifest.manifest.rent_quote.clone(),
        stripe_layout_from_manifest(&manifest.manifest),
    )
    .expect("build receipt");
    let correct_fingerprint = *manifest.fingerprint.as_bytes();
    let manifest_path = spool_artifact_path_for_key(
        spool_dir,
        "manifest-",
        request.lane_id,
        request.epoch,
        request.sequence,
        &manifest.storage_ticket,
        correct_fingerprint,
    );
    let mut mismatched_manifest = manifest.manifest.clone();
    mismatched_manifest.issued_at_unix = mismatched_manifest.issued_at_unix.saturating_add(1);
    fs::write(
        &manifest_path,
        to_bytes(&mismatched_manifest).expect("encode mismatched manifest sidecar"),
    )
    .expect("write mismatched manifest sidecar");
    let receipt_path =
        receipt_spool_path(spool_dir, &receipt, request.sequence, correct_fingerprint);
    fs::write(
        &receipt_path,
        encoded_stored_receipt(
            &receipt,
            request.sequence,
            persistence::STORED_RECEIPT_VERSION,
        ),
    )
    .expect("write receipt");
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let err = match open_receipt_log(spool_dir, &cursor_store, &signer) {
        Ok(_) => panic!("receipt/manifest hash mismatch must reject receipt-log recovery"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}")
            .contains("receipt manifest hash does not match ticket-indexed DA manifest artifact"),
        "unexpected receipt-log recovery error: {err:?}"
    );
    assert!(
        cursor_store.highest_sequences().is_empty(),
        "failed recovery must not seed receipt cursors"
    );
}
#[test]
fn da_receipt_log_rejects_invalid_signature() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(5), 7);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_random_keypair();
    let log = open_receipt_log(temp_dir.path(), &cursor_store, &signer).expect("open log");
    let mut receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 4);
    let unsigned = persistence::unsigned_receipt_bytes(&receipt, 1).expect("unsigned bytes");
    let wrong_signer = checked_random_keypair();
    receipt.operator_signature = checked_signature(wrong_signer.private_key(), &unsigned);
    let outcome = log.append(lane_epoch, 1, receipt, test_fingerprint(4));
    assert!(
        outcome.is_err(),
        "receipt with mismatched signature must be rejected"
    );
}
#[test]
fn da_receipt_log_rejects_sequence_rebound_signature() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(5), 8);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_random_keypair();
    let log = open_receipt_log(temp_dir.path(), &cursor_store, &signer).expect("open log");
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 5);
    let outcome = log.append(lane_epoch, 2, receipt, test_fingerprint(5));
    assert!(
        outcome.is_err(),
        "receipt signature must bind the append sequence"
    );
    assert_eq!(
        receipt_file_count(temp_dir.path()),
        0,
        "sequence-rebound receipt must not be persisted"
    );
}
#[test]
fn da_receipt_log_reloads_from_disk() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(5), 11);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_random_keypair();
    {
        let log = open_receipt_log(temp_dir.path(), &cursor_store, &signer).unwrap();
        let first = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 0, 9);
        let second = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 10);
        log.append(lane_epoch, 0, first.clone(), test_fingerprint(9))
            .unwrap();
        log.append(lane_epoch, 1, second.clone(), test_fingerprint(10))
            .unwrap();
    }
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let reopened = open_receipt_log(temp_dir.path(), &cursor_store, &signer).unwrap();
    let entries = reopened.receipts_for(lane_epoch);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].sequence, 0);
    assert_eq!(entries[1].sequence, 1);
    assert_eq!(
        entries[1].manifest_hash,
        BlobDigest::new([10u8.wrapping_add(3); 32])
    );
    assert!(
        cursor_store
            .highest_sequences()
            .iter()
            .any(|(key, seq)| *key == lane_epoch && *seq == 1),
        "cursor store should be seeded from disk"
    );
}
#[test]
fn da_receipt_log_recovery_rejects_nonzero_origin() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(6), 19);
    let signer = checked_fixture_ed25519_keypair(0x6A);
    let receipt = test_receipt(
        &signer,
        lane_epoch.lane_id,
        lane_epoch.epoch,
        u64::MAX - 1,
        0x96,
    );
    persistence::persist_da_receipt(
        temp_dir.path(),
        &receipt,
        u64::MAX - 1,
        &test_fingerprint(0x96),
    )
    .expect("persist nonzero-origin receipt")
    .expect("receipt path");
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let err = match open_receipt_log(temp_dir.path(), &cursor_store, &signer) {
        Ok(_) => panic!("receipt-log recovery must reject a nonzero origin"),
        Err(err) => err,
    };
    let message = format!("{err:?}");
    assert!(
        message.contains(&format!("starts at {}; expected 0", u64::MAX - 1)),
        "unexpected nonzero-origin recovery error: {err:?}"
    );
    assert!(cursor_store.highest_sequences().is_empty());
}
#[test]
fn da_receipt_log_rejects_sequence_gap_on_open() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(6), 18);
    let signer = checked_fixture_ed25519_keypair(0x69);
    for (sequence, seed) in [(0, 0x94), (2, 0x95)] {
        let receipt = test_receipt(
            &signer,
            lane_epoch.lane_id,
            lane_epoch.epoch,
            sequence,
            seed,
        );
        let bytes = encoded_stored_receipt(&receipt, sequence, persistence::STORED_RECEIPT_VERSION);
        let path = canonical_receipt_spool_path(temp_dir.path(), &receipt, sequence);
        fs::write(path, bytes).expect("write receipt");
    }
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let err = match open_receipt_log(temp_dir.path(), &cursor_store, &signer) {
        Ok(_) => panic!("receipt-log recovery must reject missing receipt sequences"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("missing DA receipt sequence"),
        "unexpected receipt-log recovery error: {err:?}"
    );
    assert!(
        cursor_store.highest_sequences().is_empty(),
        "gap receipt logs must not seed replay cursors"
    );
}
#[test]
fn da_receipt_log_rejects_same_manifest_duplicate_with_different_receipt_on_open() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(6), 16);
    let signer = checked_random_keypair();
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 0x91);
    let mut conflicting = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 0x92);
    conflicting.manifest_hash = receipt.manifest_hash;
    let unsigned = persistence::unsigned_receipt_bytes(&conflicting, 1).expect("unsigned bytes");
    conflicting.operator_signature = checked_signature(signer.private_key(), &unsigned);
    for receipt in [&receipt, &conflicting] {
        let bytes = encoded_stored_receipt(receipt, 1, persistence::STORED_RECEIPT_VERSION);
        let path = canonical_receipt_spool_path(temp_dir.path(), receipt, 1);
        fs::write(path, bytes).expect("write duplicate receipt");
    }
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let err = match open_receipt_log(temp_dir.path(), &cursor_store, &signer) {
        Ok(_) => panic!("conflicting duplicate receipt must reject receipt-log recovery"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("conflicting duplicate receipt"),
        "unexpected receipt-log recovery error: {err:?}"
    );
    assert!(
        cursor_store.highest_sequences().is_empty(),
        "conflicting duplicate receipts must not seed replay cursors"
    );
}
#[test]
fn da_receipt_log_rejects_same_receipt_under_wrong_fingerprint_on_open() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(6), 17);
    let signer = checked_random_keypair();
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 0x93);
    let bytes = encoded_stored_receipt(&receipt, 1, persistence::STORED_RECEIPT_VERSION);
    for fingerprint in [receipt_fingerprint_bytes(&receipt), [0xA4; 32]] {
        let path = receipt_spool_path(temp_dir.path(), &receipt, 1, fingerprint);
        fs::write(path, &bytes).expect("write duplicate receipt");
    }
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let err = match open_receipt_log(temp_dir.path(), &cursor_store, &signer) {
        Ok(_) => {
            panic!("same receipt under different fingerprints must reject receipt-log recovery")
        }
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("mismatches body storage ticket"),
        "unexpected receipt-log recovery error: {err:?}"
    );
    assert!(
        cursor_store.highest_sequences().is_empty(),
        "ambiguous duplicate receipts must not seed replay cursors"
    );
}
#[test]
fn da_receipt_log_rejects_sequence_rebound_signature_on_open() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(6), 13);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_random_keypair();
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 9);
    let bytes = encoded_stored_receipt(&receipt, 2, persistence::STORED_RECEIPT_VERSION);
    let path = canonical_receipt_spool_path(temp_dir.path(), &receipt, 2);
    fs::write(&path, bytes).expect("write rebound receipt");
    let err = match open_receipt_log(temp_dir.path(), &cursor_store, &signer) {
        Ok(_) => panic!("sequence-rebound receipt must reject receipt-log recovery"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("failed to verify durable DA receipt"),
        "unexpected receipt-log recovery error: {err:?}"
    );
    assert!(
        cursor_store.highest_sequences().is_empty(),
        "sequence-rebound receipt must not seed replay cursors"
    );
}
#[test]
fn da_receipt_log_rejects_invalid_entries_on_open() {
    let temp_dir = tempdir().expect("temp dir");
    let signer = checked_random_keypair();
    let corrupt_receipt = test_receipt(&signer, LaneId::new(1), 1, 1, 0xAA);
    let bad_path = canonical_receipt_spool_path(temp_dir.path(), &corrupt_receipt, 1);
    fs::write(&bad_path, b"corrupt").expect("write corrupt receipt");
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let err = match open_receipt_log(temp_dir.path(), &cursor_store, &signer) {
        Ok(_) => panic!("corrupt receipt must reject receipt-log recovery"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("failed to load durable DA receipt"),
        "unexpected receipt-log recovery error: {err:?}"
    );
}
#[test]
fn da_receipt_log_rejects_receipt_shaped_directory_on_open() {
    let temp_dir = tempdir().expect("temp dir");
    let signer = checked_random_keypair();
    let receipt = test_receipt(&signer, LaneId::new(1), 1, 1, 0xAB);
    let path = canonical_receipt_spool_path(temp_dir.path(), &receipt, 1);
    fs::create_dir(&path).expect("create receipt-shaped directory");
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let err = match open_receipt_log(temp_dir.path(), &cursor_store, &signer) {
        Ok(_) => panic!("receipt-shaped directory must reject receipt-log recovery"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("is not a regular file"),
        "unexpected receipt-log recovery error: {err:?}"
    );
    assert!(
        cursor_store.highest_sequences().is_empty(),
        "receipt-shaped directories must not seed replay cursors"
    );
}
#[test]
fn da_receipt_log_rejects_filename_body_mismatch_on_open() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(6), 12);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_random_keypair();
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 8);
    let bytes = encoded_stored_receipt(&receipt, 1, persistence::STORED_RECEIPT_VERSION);
    let mismatched_path = canonical_receipt_spool_path(temp_dir.path(), &receipt, 2);
    fs::write(&mismatched_path, bytes).expect("write mismatched receipt");
    let err = match open_receipt_log(temp_dir.path(), &cursor_store, &signer) {
        Ok(_) => panic!("filename/body mismatch must reject receipt-log recovery"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("failed to load durable DA receipt"),
        "unexpected receipt-log recovery error: {err:?}"
    );
    assert!(cursor_store.highest_sequences().is_empty());
}
#[test]
fn da_receipt_log_rejects_filename_ticket_mismatch_on_open() {
    let temp_dir = tempdir().expect("temp dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(6), 14);
    let cursor_store = Arc::new(ReplayCursorStore::in_memory());
    let signer = checked_random_keypair();
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 1, 0x8A);
    let bytes = encoded_stored_receipt(&receipt, 1, persistence::STORED_RECEIPT_VERSION);
    let mut filename_receipt = receipt;
    filename_receipt.storage_ticket = StorageTicketId::new([0x99; 32]);
    let mismatched_path = canonical_receipt_spool_path(temp_dir.path(), &filename_receipt, 1);
    fs::write(&mismatched_path, bytes).expect("write mismatched receipt");
    let err = match open_receipt_log(temp_dir.path(), &cursor_store, &signer) {
        Ok(_) => panic!("filename/body ticket mismatch must reject receipt-log recovery"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("failed to load durable DA receipt"),
        "unexpected receipt-log recovery error: {err:?}"
    );
    assert!(cursor_store.highest_sequences().is_empty());
}
#[test]
fn da_receipt_log_rejects_replay_cursor_seed_failures_on_open() {
    let receipt_dir = tempdir().expect("receipt dir");
    let lane_epoch = LaneEpoch::new(LaneId::new(6), 15);
    let signer = checked_random_keypair();
    let receipt = test_receipt(&signer, lane_epoch.lane_id, lane_epoch.epoch, 0, 0x8B);
    persistence::persist_da_receipt(receipt_dir.path(), &receipt, 0, &test_fingerprint(0x8B))
        .expect("persist receipt")
        .expect("receipt path");
    let cursor_store = Arc::new(ReplayCursorStore::in_memory_with_max_lane_epochs(
        NonZeroUsize::new(1).unwrap(),
    ));
    let retained = LaneEpoch::new(LaneId::new(7), 15);
    cursor_store
        .record(retained, 9)
        .expect("seed the sole bounded replay cursor");
    let err = match open_receipt_log(receipt_dir.path(), &cursor_store, &signer) {
        Ok(_) => panic!("cursor seed failures must reject receipt-log recovery"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("failed to seed receipt cursor from disk"),
        "unexpected cursor seed error: {err:?}"
    );
    assert!(
        cursor_store.highest_sequences() == vec![(retained, 9)],
        "failed cursor seeding must not mutate bounded cursor memory"
    );
}
struct ReplayCursorFixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    main_path: PathBuf,
    temp_path: PathBuf,
    journal_path: PathBuf,
    lane_epoch: LaneEpoch,
}
fn replay_cursor_fixture() -> ReplayCursorFixture {
    let dir = tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let main_path = replay_cursor_main_path(&root);
    let temp_path = persistence::replay_cursor_temp_path(&main_path);
    let journal_path = replay_cursor_journal_path(&root);
    ReplayCursorFixture {
        _dir: dir,
        root,
        main_path,
        temp_path,
        journal_path,
        lane_epoch: LaneEpoch::new(LaneId::new(2), 9),
    }
}
#[test]
fn replay_cursor_store_persists_sequences() {
    let fixture = replay_cursor_fixture();
    let store = ReplayCursorStore::open(fixture.root.clone()).expect("open store");
    store.record(fixture.lane_epoch, 42).expect("record");
    drop(store);
    let reopened = ReplayCursorStore::open(fixture.root).expect("reopen store");
    let mut entries = reopened.highest_sequences();
    assert_eq!(entries.len(), 1);
    entries.sort_by_key(|(lane_epoch, _)| lane_epoch.lane_id.as_u32());
    assert_eq!(entries[0], (fixture.lane_epoch, 42));
}
#[test]
fn replay_cursor_store_persists_first_zero_sequence() {
    let fixture = replay_cursor_fixture();
    let store = ReplayCursorStore::open(fixture.root.clone()).expect("open store");
    store.record(fixture.lane_epoch, 0).expect("record zero");
    drop(store);
    let reopened = ReplayCursorStore::open(fixture.root).expect("reopen store");
    assert_replay_cursor_sequences(&reopened, &[(fixture.lane_epoch, 0)]);
}
#[test]
fn replay_cursor_store_rejects_new_lane_epochs_at_global_capacity() {
    let store = ReplayCursorStore::in_memory_with_max_lane_epochs(NonZeroUsize::new(2).unwrap());
    let first = LaneEpoch::new(LaneId::new(2), 9);
    let second = LaneEpoch::new(LaneId::new(2), 10);
    let rejected = LaneEpoch::new(LaneId::new(2), 11);
    store.record(first, 1).expect("first cursor");
    store.record(second, 2).expect("second cursor");
    let err = store
        .record(rejected, 3)
        .expect_err("third lane/epoch must exceed the global bound");
    assert!(
        format!("{err:?}").contains("capacity 2 is exhausted"),
        "unexpected capacity error: {err:?}"
    );
    assert_replay_cursor_sequences(&store, &[(first, 1), (second, 2)]);
    store
        .record(first, 4)
        .expect("existing lane/epoch may still advance at capacity");
    assert_replay_cursor_sequences(&store, &[(first, 4), (second, 2)]);
}
#[test]
fn replay_cursor_store_uses_journal_without_per_record_snapshot_rewrite() {
    let fixture = replay_cursor_fixture();
    let store = ReplayCursorStore::open(fixture.root.clone()).expect("open store");
    store
        .record(fixture.lane_epoch, 42)
        .expect("append cursor journal");
    assert!(
        !fixture.main_path.exists(),
        "a single cursor update must not rewrite the full snapshot"
    );
    assert!(
        fs::metadata(&fixture.journal_path)
            .expect("journal metadata")
            .len()
            > 0,
        "the constant-size journal entry must be durable"
    );
    drop(store);
    let reopened = ReplayCursorStore::open(fixture.root).expect("recover journal");
    assert_replay_cursor_sequences(&reopened, &[(fixture.lane_epoch, 42)]);
}
#[test]
fn replay_cursor_store_checkpoints_at_bounded_journal_interval() {
    let fixture = replay_cursor_fixture();
    let store = ReplayCursorStore::open_with_max_lane_epochs(
        fixture.root.clone(),
        NonZeroUsize::new(2).unwrap(),
    )
    .expect("open bounded store");
    store
        .record(fixture.lane_epoch, 1)
        .expect("first journal entry");
    assert!(!fixture.main_path.exists());
    store
        .record(fixture.lane_epoch, 2)
        .expect("second journal entry");
    assert!(
        fixture.main_path.exists(),
        "capacity-sized journal must checkpoint"
    );
    assert_eq!(
        fs::metadata(&fixture.journal_path)
            .expect("journal metadata after checkpoint")
            .len(),
        0,
        "checkpoint must truncate the fully applied journal"
    );
    drop(store);
    let reopened =
        ReplayCursorStore::open_with_max_lane_epochs(fixture.root, NonZeroUsize::new(2).unwrap())
            .expect("reopen checkpointed store");
    assert_replay_cursor_sequences(&reopened, &[(fixture.lane_epoch, 2)]);
}
#[test]
fn replay_cursor_store_recovers_torn_final_journal_frame() {
    let fixture = replay_cursor_fixture();
    let store = ReplayCursorStore::open(fixture.root.clone()).expect("open store");
    store
        .record(fixture.lane_epoch, 42)
        .expect("append cursor journal");
    drop(store);
    let valid_len = fs::metadata(&fixture.journal_path)
        .expect("journal metadata")
        .len();
    fs::OpenOptions::new()
        .append(true)
        .open(&fixture.journal_path)
        .expect("open journal for torn-tail fixture")
        .write_all(&[0, 0])
        .expect("append torn length prefix");
    let reopened = ReplayCursorStore::open(fixture.root).expect("recover torn final frame");
    assert_replay_cursor_sequences(&reopened, &[(fixture.lane_epoch, 42)]);
    assert_eq!(
        fs::metadata(&fixture.journal_path)
            .expect("recovered journal metadata")
            .len(),
        valid_len,
        "recovery must truncate only the torn tail"
    );
}
#[test]
fn replay_cursor_store_rejects_corrupt_complete_journal_frame() {
    let fixture = replay_cursor_fixture();
    let store = ReplayCursorStore::open(fixture.root.clone()).expect("open store");
    store
        .record(fixture.lane_epoch, 42)
        .expect("append cursor journal");
    drop(store);
    let mut bytes = fs::read(&fixture.journal_path).expect("read journal");
    let checksum_byte = bytes.last_mut().expect("journal frame is non-empty");
    *checksum_byte ^= 0x80;
    fs::write(&fixture.journal_path, bytes).expect("write corrupt journal fixture");
    let err = match ReplayCursorStore::open(fixture.root) {
        Ok(_) => panic!("corrupt complete journal frame must fail closed"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("checksum mismatch"),
        "unexpected corrupt journal error: {err:?}"
    );
}
#[test]
fn replay_cursor_store_retries_checkpoint_after_persist_failure() {
    let fixture = replay_cursor_fixture();
    let store = ReplayCursorStore::open(fixture.root.clone()).expect("open store");
    store
        .record(fixture.lane_epoch, 42)
        .expect("append journal entry");
    fs::create_dir(&fixture.temp_path).expect("block temp snapshot path");
    let err = store
        .checkpoint()
        .expect_err("blocked temp path should fail snapshot persistence");
    assert!(
        format!("{err:?}").contains("failed to create DA replay snapshot temp file"),
        "unexpected error: {err:?}"
    );
    assert_replay_cursor_sequences(&store, &[(fixture.lane_epoch, 42)]);
    fs::remove_dir(&fixture.temp_path).expect("unblock temp snapshot path");
    store.checkpoint().expect("retry checkpoint");
    drop(store);
    let reopened = ReplayCursorStore::open(fixture.root).expect("reopen store");
    assert_replay_cursor_sequences(&reopened, &[(fixture.lane_epoch, 42)]);
}
#[test]
fn replay_cursor_store_rejects_existing_temp_without_truncating() {
    let fixture = replay_cursor_fixture();
    let store = ReplayCursorStore::open(fixture.root.clone()).expect("open store");
    store
        .record(fixture.lane_epoch, 42)
        .expect("append journal entry");
    fs::write(&fixture.temp_path, b"existing-temp-snapshot").expect("seed temp snapshot");
    let err = store
        .checkpoint()
        .expect_err("existing temp snapshot should reject cursor persistence");
    assert!(
        format!("{err:?}").contains("failed to create DA replay snapshot temp file"),
        "unexpected error: {err:?}"
    );
    assert_replay_cursor_sequences(&store, &[(fixture.lane_epoch, 42)]);
    assert_eq!(
        fs::read(&fixture.temp_path).expect("read temp snapshot after failed record"),
        b"existing-temp-snapshot"
    );
}
#[cfg(unix)]
#[test]
fn replay_cursor_store_open_rejects_dir_symlink() {
    use std::os::unix::fs::symlink;
    let temp = tempdir().expect("tempdir");
    let target = temp.path().join("cursor-root-target");
    fs::create_dir(&target).expect("create cursor target directory");
    let lane_epoch = LaneEpoch::new(LaneId::new(2), 9);
    fs::write(
        replay_cursor_main_path(&target),
        replay_cursor_snapshot_bytes(&[(lane_epoch, 42)]),
    )
    .expect("write target cursor snapshot");
    let link = temp.path().join("cursor-root-link");
    symlink(&target, &link).expect("create cursor root symlink");
    let err = match ReplayCursorStore::open(link.clone()) {
        Ok(_) => panic!("symlinked replay cursor root must reject open"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("DA replay directory"),
        "unexpected cursor root symlink error: {err:?}"
    );
    assert!(
        fs::symlink_metadata(&link)
            .expect("inspect cursor root symlink")
            .file_type()
            .is_symlink(),
        "failed open should leave cursor root symlink visible"
    );
    assert!(
        replay_cursor_main_path(&target).exists(),
        "cursor root symlink target should remain for operator repair"
    );
}
#[cfg(unix)]
#[test]
fn replay_cursor_store_empty_rejects_dir_symlink() {
    use std::os::unix::fs::symlink;
    let temp = tempdir().expect("tempdir");
    let target = temp.path().join("cursor-empty-target");
    fs::create_dir(&target).expect("create cursor target directory");
    let link = temp.path().join("cursor-empty-link");
    symlink(&target, &link).expect("create cursor root symlink");
    let err = match ReplayCursorStore::empty(link.clone()) {
        Ok(_) => panic!("symlinked replay cursor root must reject empty store creation"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("DA replay directory"),
        "unexpected cursor root symlink error: {err:?}"
    );
    assert!(
        fs::symlink_metadata(&link)
            .expect("inspect cursor root symlink")
            .file_type()
            .is_symlink(),
        "failed empty store creation should leave cursor root symlink visible"
    );
}
#[cfg(unix)]
#[test]
fn replay_cursor_store_record_rejects_dir_symlink_replacement() {
    use std::os::unix::fs::symlink;
    let temp = tempdir().expect("tempdir");
    let path = temp.path().join("cursor-root");
    fs::create_dir(&path).expect("create cursor root");
    let store = ReplayCursorStore::open(path.clone()).expect("open store");
    fs::remove_file(replay_cursor_journal_path(&path)).expect("remove open cursor journal path");
    fs::remove_dir(&path).expect("remove cursor root");
    let target = temp.path().join("cursor-root-target");
    fs::create_dir(&target).expect("create cursor target directory");
    symlink(&target, &path).expect("replace cursor root with symlink");
    let lane_epoch = LaneEpoch::new(LaneId::new(2), 9);
    let err = store
        .record(lane_epoch, 42)
        .expect_err("symlinked replay cursor root replacement must reject persistence");
    assert!(
        format!("{err:?}").contains("DA replay snapshot directory"),
        "unexpected cursor root replacement error: {err:?}"
    );
    assert_replay_cursor_sequences(&store, &[]);
    assert!(
        fs::symlink_metadata(&path)
            .expect("inspect replacement root symlink")
            .file_type()
            .is_symlink(),
        "failed record should leave replacement cursor root symlink visible"
    );
    assert!(
        !replay_cursor_main_path(&target).exists(),
        "cursor root symlink target must not receive the main snapshot"
    );
    assert!(
        !persistence::replay_cursor_temp_path(&replay_cursor_main_path(&target)).exists(),
        "cursor root symlink target must not receive the temp snapshot"
    );
    assert!(
        !replay_cursor_journal_path(&target).exists(),
        "cursor root symlink target must not receive a journal entry"
    );
}
#[cfg(unix)]
#[test]
fn replay_cursor_store_open_rejects_main_snapshot_symlink() {
    use std::os::unix::fs::symlink;
    let fixture = replay_cursor_fixture();
    let target_path = fixture.root.join("cursor-symlink-target.json");
    fs::write(
        &target_path,
        replay_cursor_snapshot_bytes(&[(fixture.lane_epoch, 42)]),
    )
    .expect("write cursor symlink target");
    symlink(&target_path, &fixture.main_path).expect("create cursor snapshot symlink");
    let err = match ReplayCursorStore::open(fixture.root.clone()) {
        Ok(_) => panic!("symlinked main replay cursor snapshot must reject open"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("not a regular file"),
        "unexpected cursor symlink error: {err:?}"
    );
    assert!(
        fs::symlink_metadata(&fixture.main_path)
            .expect("inspect cursor symlink")
            .file_type()
            .is_symlink(),
        "failed open should leave main cursor symlink visible"
    );
    assert!(
        target_path.exists(),
        "cursor symlink target should remain for operator repair"
    );
}
#[cfg(unix)]
#[test]
fn replay_cursor_store_open_rejects_journal_symlink() {
    use std::os::unix::fs::symlink;
    let temp = tempdir().expect("tempdir");
    let journal_path = replay_cursor_journal_path(temp.path());
    let target_path = temp.path().join("cursor-journal-symlink-target");
    fs::write(&target_path, []).expect("write journal symlink target");
    symlink(&target_path, &journal_path).expect("create cursor journal symlink");
    let err = match ReplayCursorStore::open(temp.path().to_path_buf()) {
        Ok(_) => panic!("symlinked replay cursor journal must reject open"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("not a regular file"),
        "unexpected cursor journal symlink error: {err:?}"
    );
    assert!(
        fs::symlink_metadata(&journal_path)
            .expect("inspect cursor journal symlink")
            .file_type()
            .is_symlink(),
        "failed open should leave cursor journal symlink visible"
    );
}
#[cfg(unix)]
#[test]
fn replay_cursor_store_open_rejects_temp_snapshot_symlink() {
    use std::os::unix::fs::symlink;
    let fixture = replay_cursor_fixture();
    let target_path = fixture.root.join("cursor-temp-symlink-target.json");
    fs::write(
        &target_path,
        replay_cursor_snapshot_bytes(&[(fixture.lane_epoch, 42)]),
    )
    .expect("write cursor temp symlink target");
    symlink(&target_path, &fixture.temp_path).expect("create cursor temp snapshot symlink");
    let err = match ReplayCursorStore::open(fixture.root.clone()) {
        Ok(_) => panic!("symlinked temp replay cursor snapshot must reject open"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("not a regular file"),
        "unexpected cursor temp symlink error: {err:?}"
    );
    assert!(
        fs::symlink_metadata(&fixture.temp_path)
            .expect("inspect cursor temp symlink")
            .file_type()
            .is_symlink(),
        "failed open should leave temp cursor symlink visible"
    );
    assert!(
        target_path.exists(),
        "cursor temp symlink target should remain for operator repair"
    );
}
fn replay_cursor_main_path(dir: &Path) -> PathBuf {
    dir.join("replay_cursors.norito.json")
}
fn replay_cursor_journal_path(dir: &Path) -> PathBuf {
    dir.join("replay_cursors.journal")
}
fn replay_cursor_snapshot_bytes(entries: &[(LaneEpoch, u64)]) -> Vec<u8> {
    let temp = tempdir().expect("snapshot tempdir");
    let store = ReplayCursorStore::open(temp.path().to_path_buf()).expect("open snapshot store");
    for (lane_epoch, sequence) in entries {
        store.record(*lane_epoch, *sequence).expect("record cursor");
    }
    store.checkpoint().expect("checkpoint cursor snapshot");
    fs::read(replay_cursor_main_path(temp.path())).expect("read cursor snapshot")
}
fn replay_cursor_snapshot_value(entries: &[(LaneEpoch, u64)]) -> Value {
    json::from_slice(&replay_cursor_snapshot_bytes(entries)).expect("decode cursor snapshot")
}
fn replay_cursor_snapshot_order(entries: &[(LaneEpoch, u64)]) -> Vec<(u64, u64)> {
    let value = replay_cursor_snapshot_value(entries);
    let Value::Object(map) = value else {
        panic!("cursor snapshot must be an object");
    };
    let Some(Value::Array(entries)) = map.get("entries") else {
        panic!("cursor snapshot entries must be an array");
    };
    entries
        .iter()
        .map(|entry| {
            let Value::Object(entry) = entry else {
                panic!("cursor snapshot entry must be an object");
            };
            let lane_id = entry
                .get("lane_id")
                .and_then(Value::as_u64)
                .expect("cursor snapshot entry must include lane_id");
            let epoch = entry
                .get("epoch")
                .and_then(Value::as_u64)
                .expect("cursor snapshot entry must include epoch");
            (lane_id, epoch)
        })
        .collect()
}
fn replay_cursor_snapshot_bytes_with_version(
    entries: &[(LaneEpoch, u64)],
    version: i32,
) -> Vec<u8> {
    let mut value = replay_cursor_snapshot_value(entries);
    if let Value::Object(map) = &mut value {
        map.insert("version".into(), Value::from(version));
    } else {
        panic!("cursor snapshot must be an object");
    }
    json::to_vec(&value).expect("encode cursor snapshot")
}
fn replay_cursor_snapshot_bytes_with_duplicate_entry(entries: &[(LaneEpoch, u64)]) -> Vec<u8> {
    let mut value = replay_cursor_snapshot_value(entries);
    if let Value::Object(map) = &mut value {
        let entries = map
            .get_mut("entries")
            .expect("cursor snapshot entries must exist");
        if let Value::Array(entries) = entries {
            let duplicate = entries
                .first()
                .expect("cursor snapshot entry must exist")
                .clone();
            entries.push(duplicate);
        } else {
            panic!("cursor snapshot entries must be an array");
        }
    } else {
        panic!("cursor snapshot must be an object");
    }
    json::to_vec(&value).expect("encode cursor snapshot")
}
fn assert_replay_cursor_sequences(store: &ReplayCursorStore, expected: &[(LaneEpoch, u64)]) {
    let mut actual = store.highest_sequences();
    actual.sort_by_key(|(lane_epoch, _)| (lane_epoch.lane_id.as_u32(), lane_epoch.epoch));
    let mut expected = expected.to_vec();
    expected.sort_by_key(|(lane_epoch, _)| (lane_epoch.lane_id.as_u32(), lane_epoch.epoch));
    assert_eq!(actual, expected);
}
#[test]
fn replay_cursor_store_persists_canonical_snapshot_order() {
    let lane_a = LaneEpoch::new(LaneId::new(2), 9);
    let lane_b = LaneEpoch::new(LaneId::new(0), 9);
    let lane_c = LaneEpoch::new(LaneId::new(0), 8);
    assert_eq!(
        replay_cursor_snapshot_order(&[(lane_a, 42), (lane_b, 43), (lane_c, 44)]),
        vec![(0, 8), (0, 9), (2, 9)]
    );
    let temp = tempdir().expect("tempdir");
    let store = ReplayCursorStore::open(temp.path().to_path_buf()).expect("open store");
    store.record(lane_a, 42).expect("record lane a");
    store.record(lane_b, 43).expect("record lane b");
    store.record(lane_c, 44).expect("record lane c");
    assert_eq!(
        store.highest_sequences(),
        vec![(lane_c, 44), (lane_b, 43), (lane_a, 42)]
    );
}
#[test]
fn replay_cursor_store_open_promotes_temp_snapshot() {
    let fixture = replay_cursor_fixture();
    fs::write(
        &fixture.temp_path,
        replay_cursor_snapshot_bytes(&[(fixture.lane_epoch, 42)]),
    )
    .expect("write temp snapshot");
    let store = ReplayCursorStore::open(fixture.root.clone()).expect("open store");
    assert!(
        fixture.main_path.exists(),
        "temp snapshot should be promoted"
    );
    assert!(
        !fixture.temp_path.exists(),
        "promoted temp snapshot should be removed"
    );
    assert_replay_cursor_sequences(&store, &[(fixture.lane_epoch, 42)]);
}
#[test]
fn replay_cursor_store_open_promotes_newer_temp_snapshot() {
    let fixture = replay_cursor_fixture();
    fs::write(
        &fixture.main_path,
        replay_cursor_snapshot_bytes(&[(fixture.lane_epoch, 41)]),
    )
    .expect("write main snapshot");
    fs::write(
        &fixture.temp_path,
        replay_cursor_snapshot_bytes(&[(fixture.lane_epoch, 42)]),
    )
    .expect("write temp snapshot");
    let store = ReplayCursorStore::open(fixture.root.clone()).expect("open store");
    assert!(fixture.main_path.exists(), "newer temp should be promoted");
    assert!(!fixture.temp_path.exists(), "newer temp should be consumed");
    assert_replay_cursor_sequences(&store, &[(fixture.lane_epoch, 42)]);
}
#[test]
fn replay_cursor_store_open_removes_corrupt_temp_snapshot() {
    let fixture = replay_cursor_fixture();
    fs::write(
        &fixture.main_path,
        replay_cursor_snapshot_bytes(&[(fixture.lane_epoch, 42)]),
    )
    .expect("write main snapshot");
    fs::write(&fixture.temp_path, b"corrupt").expect("write corrupt temp snapshot");
    let store = ReplayCursorStore::open(fixture.root.clone()).expect("open store");
    assert!(
        !fixture.temp_path.exists(),
        "corrupt temp snapshot should be removed"
    );
    assert_replay_cursor_sequences(&store, &[(fixture.lane_epoch, 42)]);
}
#[test]
fn replay_cursor_store_open_rejects_unremovable_corrupt_temp_snapshot() {
    let fixture = replay_cursor_fixture();
    fs::write(
        &fixture.main_path,
        replay_cursor_snapshot_bytes(&[(fixture.lane_epoch, 42)]),
    )
    .expect("write main snapshot");
    fs::create_dir(&fixture.temp_path).expect("block corrupt temp snapshot cleanup");
    let err = match ReplayCursorStore::open(fixture.root.clone()) {
        Ok(_) => panic!("unremovable corrupt temp snapshot should reject recovery"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("failed to remove DA replay cursor temp snapshot"),
        "unexpected error: {err:?}"
    );
    assert!(
        fixture.temp_path.exists(),
        "failed cleanup should leave temp path visible for operator repair"
    );
}
#[test]
fn replay_cursor_store_open_rejects_orphan_corrupt_temp_snapshot() {
    let fixture = replay_cursor_fixture();
    fs::write(&fixture.temp_path, b"corrupt").expect("write corrupt temp snapshot");
    let err = match ReplayCursorStore::open(fixture.root.clone()) {
        Ok(_) => panic!("orphan corrupt temp snapshot should be rejected"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("failed to decode DA replay snapshot"),
        "unexpected error: {err:?}"
    );
    assert!(
        fixture.temp_path.exists(),
        "orphan corrupt temp snapshot should remain for operator inspection"
    );
    assert!(
        !fixture.main_path.exists(),
        "corrupt temp snapshot must not be promoted into the main cursor path"
    );
}
#[test]
fn replay_cursor_store_open_rejects_duplicate_main_snapshot() {
    let fixture = replay_cursor_fixture();
    fs::write(
        &fixture.main_path,
        replay_cursor_snapshot_bytes_with_duplicate_entry(&[(fixture.lane_epoch, 42)]),
    )
    .expect("write duplicate main snapshot");
    let err = match ReplayCursorStore::open(fixture.root.clone()) {
        Ok(_) => panic!("duplicate main snapshot should be rejected"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("duplicate DA replay cursor entry"),
        "unexpected error: {err:?}"
    );
}
#[test]
fn replay_cursor_store_open_rejects_snapshot_over_global_capacity() {
    let fixture = replay_cursor_fixture();
    let first = LaneEpoch::new(LaneId::new(2), 9);
    let second = LaneEpoch::new(LaneId::new(2), 10);
    fs::write(
        &fixture.main_path,
        replay_cursor_snapshot_bytes(&[(first, 41), (second, 42)]),
    )
    .expect("write over-capacity snapshot");
    let err = match ReplayCursorStore::open_with_max_lane_epochs(
        fixture.root.clone(),
        NonZeroUsize::new(1).unwrap(),
    ) {
        Ok(_) => panic!("over-capacity replay cursor snapshot must fail closed"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("exceeding configured maximum 1"),
        "unexpected over-capacity snapshot error: {err:?}"
    );
}
#[test]
fn replay_cursor_store_open_recovers_temp_when_main_version_unsupported() {
    let fixture = replay_cursor_fixture();
    fs::write(
        &fixture.main_path,
        replay_cursor_snapshot_bytes_with_version(&[(fixture.lane_epoch, 41)], 2),
    )
    .expect("write unsupported main snapshot");
    fs::write(
        &fixture.temp_path,
        replay_cursor_snapshot_bytes(&[(fixture.lane_epoch, 42)]),
    )
    .expect("write recoverable temp snapshot");
    let store = ReplayCursorStore::open(fixture.root.clone()).expect("recover from temp");
    assert!(fixture.main_path.exists(), "valid temp should be promoted");
    assert!(
        !fixture.temp_path.exists(),
        "promoted temp should be consumed"
    );
    assert_replay_cursor_sequences(&store, &[(fixture.lane_epoch, 42)]);
}
#[test]
fn replay_cursor_store_open_rejects_unpromotable_temp_snapshot() {
    let fixture = replay_cursor_fixture();
    fs::create_dir(&fixture.main_path).expect("block main snapshot path");
    fs::write(
        &fixture.temp_path,
        replay_cursor_snapshot_bytes(&[(fixture.lane_epoch, 42)]),
    )
    .expect("write temp snapshot");
    let err = match ReplayCursorStore::open(fixture.root.clone()) {
        Ok(_) => panic!("unpromotable temp snapshot should be rejected"),
        Err(err) => err,
    };
    assert!(
        format!("{err:?}").contains("failed to promote DA replay cursor temp snapshot"),
        "unexpected error: {err:?}"
    );
}
#[test]
fn replay_cursor_store_open_discards_conflicting_temp_snapshot() {
    let fixture = replay_cursor_fixture();
    let lane_a = LaneEpoch::new(LaneId::new(2), 9);
    let lane_b = LaneEpoch::new(LaneId::new(3), 9);
    fs::write(
        &fixture.main_path,
        replay_cursor_snapshot_bytes(&[(lane_a, 41), (lane_b, 50)]),
    )
    .expect("write main snapshot");
    fs::write(
        &fixture.temp_path,
        replay_cursor_snapshot_bytes(&[(lane_a, 42), (lane_b, 49)]),
    )
    .expect("write conflicting temp snapshot");
    let store = ReplayCursorStore::open(fixture.root.clone()).expect("open store");
    assert!(
        !fixture.temp_path.exists(),
        "conflicting temp snapshot should be removed"
    );
    assert_replay_cursor_sequences(&store, &[(lane_a, 41), (lane_b, 50)]);
}
#[test]
fn resolve_manifest_emits_parity_chunks() {
    let (fixture, artifacts) = resolved_manifest_fixture(
        sample_request(),
        1_701_000_111,
        "resolve manifest with parity",
    );
    let request = &fixture.request;
    let expected =
        build_chunk_commitments(request, &fixture.chunk_store, fixture.canonical.as_slice())
            .expect("expected chunk commitments");
    assert_eq!(artifacts.manifest.chunks, expected);
    let parity_chunks: Vec<_> = artifacts
        .manifest
        .chunks
        .iter()
        .filter(|chunk| chunk.parity)
        .collect();
    assert_eq!(
        parity_chunks.len(),
        usize::from(request.erasure_profile.parity_shards)
    );
    for (idx, chunk) in parity_chunks.into_iter().enumerate() {
        let expected_offset = request
            .total_size
            .checked_add(
                u64::try_from(idx)
                    .expect("parity index fits into u64")
                    .checked_mul(u64::from(request.chunk_size))
                    .expect("offset within test bounds"),
            )
            .expect("parity offset within test bounds");
        assert_eq!(chunk.offset, expected_offset);
        assert_eq!(chunk.length, request.chunk_size);
        assert!(chunk.parity);
    }
}
#[test]
fn resolve_manifest_rejects_malformed_request_manifest_without_panicking() {
    let mut fixture = ManifestResolutionFixture::new(sample_request());
    let valid = fixture
        .resolve(1_701_000_112)
        .expect("resolve valid manifest");
    let mut malformed = to_bytes(&valid.manifest).expect("encode valid manifest");
    malformed.truncate(malformed.len().saturating_sub(1));
    fixture.request.norito_manifest = Some(malformed);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fixture.resolve(1_701_000_113)
    }));
    let err = result
        .expect("malformed request manifest must not panic")
        .expect_err("malformed request manifest must be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("failed to decode DA manifest"),
        "unexpected malformed manifest error: {}",
        err.1
    );
}
#[test]
fn resolve_manifest_uses_provided_rent_policy() {
    let mut fixture = ManifestResolutionFixture::new(sample_request());
    fixture.rent_policy = DaRentPolicyV1::from_components(
        "0.75".parse().expect("canonical XOR rate"),
        1_500,
        250,
        125,
        "0.002".parse().expect("canonical XOR egress credit"),
    );
    let artifacts = fixture
        .resolve(1_701_001_000)
        .expect("resolve manifest with custom rent policy");
    let request = &fixture.request;
    let (gib, months) = rent_usage_from_request(request.total_size, &request.retention_policy)
        .expect("rent usage should fit test inputs");
    let expected_quote = fixture
        .rent_policy
        .quote(gib, months)
        .expect("rent quote should compute for test inputs");
    assert_eq!(artifacts.manifest.rent_quote, expected_quote);
}
#[test]
fn rent_usage_from_request_rejects_retention_month_overflow() {
    let request = sample_request();
    let mut retention = request.retention_policy.clone();
    retention.cold_retention_secs = u64::from(u32::MAX)
        .checked_mul(SECS_PER_MONTH)
        .and_then(|secs| secs.checked_add(1))
        .expect("overflow threshold fits into u64");
    let err = rent_usage_from_request(request.total_size, &retention)
        .expect_err("oversized retention duration must be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("rent quote month range"),
        "unexpected error: {}",
        err.1
    );
}
#[test]
fn resolve_manifest_rejects_retention_month_overflow() {
    let fixture = ManifestResolutionFixture::new(sample_request());
    let mut retention = fixture.request.retention_policy.clone();
    retention.hot_retention_secs = u64::from(u32::MAX)
        .checked_mul(SECS_PER_MONTH)
        .and_then(|secs| secs.checked_add(1))
        .expect("overflow threshold fits into u64");
    let err = fixture
        .resolve_with_retention(&retention, 1_701_001_001)
        .expect_err("oversized rent duration must reject manifest resolution");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("rent quote month range"),
        "unexpected error: {}",
        err.1
    );
}
#[test]
fn resolve_manifest_applies_enforced_retention_policy() {
    let fixture = ManifestResolutionFixture::new(sample_request());
    let enforced = RetentionPolicy {
        hot_retention_secs: 99,
        cold_retention_secs: 199,
        required_replicas: 9,
        storage_class: StorageClass::Cold,
        governance_tag: GovernanceTag::new("da.test"),
    };
    let artifacts = fixture
        .resolve_with_retention(&enforced, 1_701_000_555)
        .expect("resolve manifest with enforced retention");
    assert_eq!(artifacts.manifest.retention_policy, enforced);
}
#[test]
fn provided_manifest_must_match_enforced_retention_policy() {
    let mut fixture = ManifestResolutionFixture::new(sample_request());
    let artifacts = fixture.resolve(1_701_000_600).expect("resolve manifest");
    fixture.request.norito_manifest = Some(to_bytes(&artifacts.manifest).expect("encode manifest"));
    let strict_policy = RetentionPolicy {
        hot_retention_secs: fixture.request.retention_policy.hot_retention_secs + 1,
        cold_retention_secs: fixture.request.retention_policy.cold_retention_secs,
        required_replicas: fixture.request.retention_policy.required_replicas,
        storage_class: fixture.request.retention_policy.storage_class,
        governance_tag: GovernanceTag::new("da.strict"),
    };
    let err = fixture
        .resolve_with_retention(&strict_policy, 1_701_000_601)
        .expect_err("mismatched retention policy must be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
#[test]
fn provided_manifest_with_wrong_parity_is_rejected() {
    let mut fixture = ManifestResolutionFixture::new(sample_request());
    let artifacts = fixture.resolve(1_701_000_222).expect("resolve manifest");
    let mut tampered = artifacts.manifest.clone();
    let first_parity = tampered
        .chunks
        .iter_mut()
        .find(|chunk| chunk.parity)
        .expect("expected parity chunk to mutate");
    first_parity.parity = false;
    fixture.request.norito_manifest = Some(to_bytes(&tampered).expect("encode tampered manifest"));
    let err = match fixture.resolve(1_701_000_333) {
        Ok(_) => panic!("manifest with mismatched parity flag must be rejected"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
#[test]
fn provided_manifest_with_parity_role_alias_is_rejected() {
    let mut fixture = ManifestResolutionFixture::new(sample_request());
    let artifacts = fixture.resolve(1_701_000_223).expect("resolve manifest");
    let mut tampered = artifacts.manifest.clone();
    let global_parity = tampered
        .chunks
        .iter_mut()
        .find(|chunk| chunk.parity && chunk.role == ChunkRole::GlobalParity)
        .expect("expected global parity chunk to mutate");
    global_parity.role = ChunkRole::Data;
    fixture.request.norito_manifest = Some(to_bytes(&tampered).expect("encode tampered manifest"));
    let err = fixture
        .resolve(1_701_000_334)
        .expect_err("a parity/Data role alias must not bypass IPA field binding");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("role mismatch"));
}
#[test]
fn provided_manifest_with_zero_group_alias_is_rejected() {
    let mut request = sample_request();
    request.payload = vec![0x5A; 9 * usize::try_from(request.chunk_size).unwrap()];
    request.total_size = u64::try_from(request.payload.len()).unwrap();
    request.payload_hash = BlobDigest::from_hash(blake3_hash(&request.payload));
    let mut fixture = ManifestResolutionFixture::new(request);
    let artifacts = fixture.resolve(1_701_000_224).expect("resolve manifest");
    let mut tampered = artifacts.manifest.clone();
    let later_group = tampered
        .chunks
        .iter_mut()
        .find(|chunk| chunk.group_id != 0)
        .expect("expected a non-zero stripe group to mutate");
    later_group.group_id = 0;
    fixture.request.norito_manifest = Some(to_bytes(&tampered).expect("encode tampered manifest"));
    let err = fixture
        .resolve(1_701_000_335)
        .expect_err("group zero must not act as a wildcard for IPA field binding");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("group_id mismatch"));
}
#[test]
fn provided_manifest_with_wrong_ipa_commitment_is_rejected() {
    let mut fixture = ManifestResolutionFixture::new(sample_request());
    let artifacts = fixture.resolve(1_701_000_920).expect("resolve manifest");
    let mut tampered = artifacts.manifest.clone();
    tampered.ipa_commitment = BlobDigest::new([0xAB; 32]);
    fixture.request.norito_manifest = Some(to_bytes(&tampered).expect("encode tampered manifest"));
    let err = fixture
        .resolve(1_701_000_921)
        .expect_err("manifest with mismatched ipa commitment must be rejected");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
#[test]
fn governance_metadata_is_encrypted_with_configured_key() {
    let mut request = sample_request();
    let secret = b"confidential-notes".to_vec();
    request.metadata.items.push(MetadataEntry::new(
        "gov-notes",
        secret.clone(),
        MetadataVisibility::GovernanceOnly,
    ));
    let key = [0x11u8; 32];
    let encrypted = encrypt_governance_metadata(&request.metadata, Some(&key), Some("primary"))
        .expect("encryption");
    let entry = encrypted
        .items
        .iter()
        .find(|item| item.key == "gov-notes")
        .expect("entry present");
    assert_eq!(
        entry.encryption,
        MetadataEncryption::chacha20poly1305_with_label(Some("primary"))
    );
    assert_ne!(entry.value, secret);
    let decryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key).expect("decryptor");
    let plaintext = decryptor
        .decrypt_easy(entry.key.as_bytes(), &entry.value)
        .expect("decrypt");
    assert_eq!(plaintext, secret);
}
#[test]
fn governance_metadata_without_key_is_rejected() {
    let metadata = ExtraMetadata {
        items: vec![MetadataEntry::new(
            "gov-only",
            b"secret".to_vec(),
            MetadataVisibility::GovernanceOnly,
        )],
    };
    let err = match encrypt_governance_metadata(&metadata, None, None) {
        Ok(_) => panic!("expected governance-only metadata to require encryption key"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
}
#[test]
fn public_metadata_cannot_declare_encryption() {
    let metadata = ExtraMetadata {
        items: vec![MetadataEntry::with_encryption(
            "public",
            b"plain".to_vec(),
            MetadataVisibility::Public,
            MetadataEncryption::chacha20poly1305_with_label(Some("public")),
        )],
    };
    let err = match encrypt_governance_metadata(&metadata, Some(&[0u8; 32]), Some("primary")) {
        Ok(_) => panic!("expected public metadata to reject encryption hints"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
#[test]
fn governance_metadata_rejects_label_mismatch() {
    let key = [0x22u8; 32];
    let encryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key).expect("encryptor");
    let ciphertext = encryptor
        .encrypt_easy(b"gov-notes".as_ref(), b"payload".as_ref())
        .expect("encrypt payload");
    let metadata = ExtraMetadata {
        items: vec![MetadataEntry::with_encryption(
            "gov-notes",
            ciphertext,
            MetadataVisibility::GovernanceOnly,
            MetadataEncryption::chacha20poly1305_with_label(Some("secondary")),
        )],
    };
    let err = match encrypt_governance_metadata(&metadata, Some(&key), Some("primary")) {
        Ok(_) => panic!("expected label mismatch to be rejected"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
#[test]
fn governance_metadata_requires_label_when_expected() {
    let key = [0x33u8; 32];
    let encryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key).expect("encryptor");
    let ciphertext = encryptor
        .encrypt_easy(b"gov-notes".as_ref(), b"payload".as_ref())
        .expect("encrypt payload");
    let metadata = ExtraMetadata {
        items: vec![MetadataEntry::with_encryption(
            "gov-notes",
            ciphertext,
            MetadataVisibility::GovernanceOnly,
            MetadataEncryption::chacha20poly1305_with_label(None::<String>),
        )],
    };
    let err = match encrypt_governance_metadata(&metadata, Some(&key), Some("primary")) {
        Ok(_) => panic!("expected missing label to be rejected"),
        Err(err) => err,
    };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}
#[test]
fn governance_metadata_accepts_matching_label_ciphertext() {
    let key = [0x44u8; 32];
    let encryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key).expect("encryptor");
    let ciphertext = encryptor
        .encrypt_easy(b"gov-notes".as_ref(), b"payload".as_ref())
        .expect("encrypt payload");
    let metadata = ExtraMetadata {
        items: vec![MetadataEntry::with_encryption(
            "gov-notes",
            ciphertext.clone(),
            MetadataVisibility::GovernanceOnly,
            MetadataEncryption::chacha20poly1305_with_label(Some("primary")),
        )],
    };
    let processed =
        encrypt_governance_metadata(&metadata, Some(&key), Some("primary")).expect("process");
    let entry = processed
        .items
        .iter()
        .find(|item| item.key == "gov-notes")
        .expect("entry");
    assert_eq!(entry.value, ciphertext);
    assert_eq!(
        entry.encryption,
        MetadataEncryption::chacha20poly1305_with_label(Some("primary"))
    );
}
#[test]
fn streaming_chunk_ingest_matches_fixture() {
    let (request, canonical_payload) = sample_request_with_payload();
    let chunk_profile = chunk_profile_for_request(request.chunk_size);
    let plan = CarBuildPlan::single_file_with_profile(&canonical_payload, chunk_profile)
        .expect("plan derivation succeeds");
    let mut streaming_store = ChunkStore::with_profile(chunk_profile);
    let chunk_dir = tempdir().expect("chunk dir");
    let mut payload_cursor: &[u8] = canonical_payload.as_slice();
    let stream_output = streaming_store
        .ingest_plan_stream_to_directory(&plan, &mut payload_cursor, chunk_dir.path())
        .expect("streaming ingest succeeds");
    assert_eq!(
        stream_output.total_bytes, request.total_size,
        "persisted byte count should match total_size"
    );
    let direct_store = build_chunk_store(&request, canonical_payload.as_slice());
    assert_eq!(
        streaming_store.profile(),
        direct_store.profile(),
        "chunk profiles must match"
    );
    assert_eq!(
        streaming_store.payload_digest(),
        direct_store.payload_digest(),
        "payload digests must match"
    );
    assert_eq!(
        streaming_store.payload_len(),
        direct_store.payload_len(),
        "payload lengths must match"
    );
    assert_eq!(
        streaming_store.chunks(),
        direct_store.chunks(),
        "chunk metadata mismatch between streaming/non-streaming ingestion"
    );
    let expected_records = load_chunk_record_fixture("sample_chunk_records.txt");
    assert_eq!(
        stream_output.records.len(),
        expected_records.len(),
        "chunk record count drifted; regenerate fixtures"
    );
    for (actual, expected) in stream_output.records.iter().zip(expected_records.iter()) {
        assert_eq!(actual.file_name, expected.file_name);
        assert_eq!(actual.offset, expected.offset);
        assert_eq!(actual.length, expected.length);
        assert_eq!(hex::encode(actual.digest), expected.digest_hex);
    }
}
#[test]
fn manifest_persistence_matches_fixture() {
    let context = sample_manifest_context_for(BlobClass::TaikaiSegment);
    let spool_dir = tempdir().expect("spool dir");
    let manifest_path = persistence::persist_manifest_for_sorafs(
        spool_dir.path(),
        &context.artifacts.encoded,
        context.request.lane_id,
        context.request.epoch,
        context.request.sequence,
        &context.artifacts.storage_ticket,
        &context.artifacts.fingerprint,
    )
    .expect("persist manifest")
    .expect("spool path");
    let actual_bytes = fs::read(manifest_path).expect("read manifest");
    let expected_bytes = load_manifest_fixture("manifests/taikai_segment/manifest.norito.hex");
    assert_eq!(
        actual_bytes, expected_bytes,
        "DA manifest drifted; rerun regenerate_da_ingest_fixtures"
    );
}
#[test]
fn manifest_fixtures_cover_all_blob_classes() {
    for case in &MANIFEST_FIXTURE_CASES {
        let context = sample_manifest_context_for(case.blob_class);
        let expected_bytes =
            load_manifest_fixture(&format!("manifests/{}/manifest.norito.hex", case.slug));
        assert_eq!(
            context.artifacts.encoded, expected_bytes,
            "manifest fixture hex drifted for {}; rerun regenerate_da_ingest_fixtures",
            case.slug
        );
        let expected_json =
            load_manifest_json_fixture(&format!("manifests/{}/manifest.json", case.slug));
        let actual_json =
            json::to_value(&context.artifacts.manifest).expect("serialize manifest to JSON");
        assert_eq!(
            actual_json, expected_json,
            "manifest JSON fixture drifted for {}; rerun regenerate_da_ingest_fixtures",
            case.slug
        );
    }
}
#[test]
#[ignore = "regenerates DA ingest fixtures on disk"]
fn regenerate_da_ingest_fixtures() {
    for case in &MANIFEST_FIXTURE_CASES {
        let context = sample_manifest_context_for(case.blob_class);
        write_manifest_fixture_bundle(case, &context).expect("write manifest fixture bundle");
    }
    println!(
        "Regenerated manifest fixtures for {} blob classes under {}/manifests",
        MANIFEST_FIXTURE_CASES.len(),
        fixtures_dir().display()
    );
    let (request, canonical_payload) = sample_request_with_payload();
    let chunk_profile = chunk_profile_for_request(request.chunk_size);
    let plan = CarBuildPlan::single_file_with_profile(&canonical_payload, chunk_profile)
        .expect("plan derivation succeeds");
    let mut streaming_store = ChunkStore::with_profile(chunk_profile);
    let chunk_dir = tempdir().expect("chunk dir");
    let mut payload_cursor: &[u8] = canonical_payload.as_slice();
    let stream_output = streaming_store
        .ingest_plan_stream_to_directory(&plan, &mut payload_cursor, chunk_dir.path())
        .expect("streaming ingest succeeds");
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("encrypt metadata");
    let rent_policy = DaRentPolicyV1::default();
    let manifest = resolve_manifest(
        &request,
        &streaming_store,
        canonical_payload.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_999,
        &rent_policy,
    )
    .expect("resolve manifest");
    let chunk_fixture_path = fixtures_dir().join("sample_chunk_records.txt");
    write_chunk_record_fixture(
        &chunk_fixture_path,
        &stream_output.records,
        stream_output.total_bytes,
    )
    .expect("write chunk fixture");
    println!(
        "Regenerated chunk fixtures at {} (total bytes = {})",
        chunk_fixture_path.display(),
        stream_output.total_bytes
    );
    println!(
        "Manifest hex for reference (taikai segment): {}",
        hex::encode(&manifest.encoded)
    );
}
fn sample_request_with_payload() -> (DaIngestRequest, Vec<u8>) {
    let request = sample_request();
    let canonical_vec = {
        let canonical = normalize_payload(&request).expect("normalize payload");
        canonical.into_vec()
    };
    (request, canonical_vec)
}
#[derive(Clone, Copy)]
struct ManifestFixtureCase {
    slug: &'static str,
    blob_class: BlobClass,
}
const MANIFEST_FIXTURE_CASES: [ManifestFixtureCase; 4] = [
    ManifestFixtureCase {
        slug: "taikai_segment",
        blob_class: BlobClass::TaikaiSegment,
    },
    ManifestFixtureCase {
        slug: "nexus_lane_sidecar",
        blob_class: BlobClass::NexusLaneSidecar,
    },
    ManifestFixtureCase {
        slug: "governance_artifact",
        blob_class: BlobClass::GovernanceArtifact,
    },
    ManifestFixtureCase {
        slug: "custom_0042",
        blob_class: BlobClass::Custom(0x0042),
    },
];
const fn manifest_fixture_variant_guard(class: BlobClass) {
    match class {
        BlobClass::TaikaiSegment
        | BlobClass::NexusLaneSidecar
        | BlobClass::GovernanceArtifact
        | BlobClass::Custom(_) => {}
    }
}
const _: fn(BlobClass) = manifest_fixture_variant_guard;
struct ManifestFixtureContext {
    request: DaIngestRequest,
    artifacts: ManifestArtifacts,
}
fn sample_manifest_context_for(blob_class: BlobClass) -> ManifestFixtureContext {
    manifest_context_for_sequence(blob_class, 7)
}
fn zero_sequence_manifest_context_for(blob_class: BlobClass) -> ManifestFixtureContext {
    manifest_context_for_sequence(blob_class, 0)
}
fn manifest_context_for_sequence(blob_class: BlobClass, sequence: u64) -> ManifestFixtureContext {
    let (mut request, canonical_payload) = sample_request_with_payload();
    request.blob_class = blob_class;
    request.sequence = sequence;
    let keypair = checked_fixture_keypair(vec![0x42; 32], Algorithm::Ed25519);
    let digest = request.signing_digest();
    request.signatures[0].signature = checked_signature(keypair.private_key(), &digest);
    let chunk_store = build_chunk_store(&request, canonical_payload.as_slice());
    let metadata =
        encrypt_governance_metadata(&request.metadata, None, None).expect("metadata encrypt");
    let rent_policy = DaRentPolicyV1::default();
    let artifacts = resolve_manifest(
        &request,
        &chunk_store,
        canonical_payload.as_slice(),
        &metadata,
        &request.retention_policy,
        1_701_000_999,
        &rent_policy,
    )
    .expect("resolve manifest");
    ManifestFixtureContext { request, artifacts }
}
const METRIC_ASSERT_EPSILON: f64 = 1e-6;
#[test]
fn record_taikai_ingest_metrics_updates_histograms() {
    let (metrics, telemetry) = telemetry_handle_for_tests();
    let sample = taikai_ingest::TaikaiTelemetrySample {
        event_id: "event".into(),
        stream_id: "stream-main".into(),
        rendition_id: "1080p".into(),
        segment_sequence: 5,
        wallclock_unix_ms: 1_702_560_000_000,
        ingest_latency_ms: Some(150),
        live_edge_drift_ms: Some(-37),
    };
    taikai::record_taikai_ingest_metrics(&telemetry, "cluster-a", &sample);
    let dump = metrics.try_to_string().expect("metrics text");
    let latency_line = find_metric_line(
        &dump,
        "taikai_ingest_segment_latency_ms_sum{cluster=\"cluster-a\"",
    );
    assert!(latency_line.contains(r#"stream="stream-main""#));
    let latency = parse_metric_value(latency_line);
    assert!(
        (latency - 150.0).abs() < METRIC_ASSERT_EPSILON,
        "expected ingest latency sum to equal 150.0, got {latency}"
    );
    let drift_line = find_metric_line(
        &dump,
        "taikai_ingest_live_edge_drift_ms_sum{cluster=\"cluster-a\"",
    );
    assert!(drift_line.contains(r#"stream="stream-main""#));
    let drift = parse_metric_value(drift_line);
    assert!(
        (drift - 37.0).abs() < METRIC_ASSERT_EPSILON,
        "expected live-edge drift sum to equal 37.0, got {drift}"
    );
    let signed_drift_line = find_metric_line(
        &dump,
        "taikai_ingest_live_edge_drift_signed_ms{cluster=\"cluster-a\"",
    );
    assert!(signed_drift_line.contains(r#"stream="stream-main""#));
    let signed_drift = parse_metric_value(signed_drift_line);
    assert!(
        (signed_drift + 37.0).abs() < METRIC_ASSERT_EPSILON,
        "expected signed live-edge drift gauge to equal -37.0, got {signed_drift}"
    );
}
#[test]
fn record_taikai_ingest_error_counts_by_status() {
    let (metrics, telemetry) = telemetry_handle_for_tests();
    taikai::record_taikai_ingest_error(
        &telemetry,
        "cluster-a",
        "stream-main",
        StatusCode::BAD_REQUEST,
    );
    let dump = metrics.try_to_string().expect("metrics text");
    let error_line = find_metric_line(&dump, "taikai_ingest_errors_total{cluster=\"cluster-a\"");
    assert!(error_line.contains(r#"stream="stream-main""#));
    assert!(error_line.contains(r#"reason="Bad Request""#));
    let errors = parse_metric_value(error_line);
    assert!(
        (errors - 1.0).abs() < METRIC_ASSERT_EPSILON,
        "expected error counter to equal 1.0, got {errors}"
    );
}
#[test]
fn record_taikai_alias_rotation_event_updates_metrics() {
    let (metrics, telemetry) = telemetry_handle_for_tests();
    let manifest = sample_trm_manifest();
    taikai::record_taikai_alias_rotation_event(&telemetry, "cluster-a", &manifest, "deadbeef");
    let dump = metrics.try_to_string().expect("metrics text");
    let metric_line = find_metric_line(
        &dump,
        "taikai_trm_alias_rotations_total{alias_name=\"docs\",alias_namespace=\"sora\"",
    );
    assert!(
        metric_line.contains("cluster=\"cluster-a\"")
            && metric_line.contains("event=\"global-keynote\"")
            && metric_line.contains("stream=\"stage-a\""),
        "metric labels should reflect cluster/event/stream"
    );
    let value = parse_metric_value(metric_line);
    assert!(
        (value - 1.0).abs() < METRIC_ASSERT_EPSILON,
        "expected alias rotation counter to increment"
    );
    let snapshots = metrics.taikai_alias_rotation_status();
    assert_eq!(snapshots.len(), 1);
    let snapshot = &snapshots[0];
    assert_eq!(snapshot.cluster, "cluster-a");
    assert_eq!(snapshot.event, "global-keynote");
    assert_eq!(snapshot.stream, "stage-a");
    assert_eq!(snapshot.alias_namespace, "sora");
    assert_eq!(snapshot.alias_name, "docs");
    assert_eq!(snapshot.window_start_sequence, 0);
    assert_eq!(snapshot.window_end_sequence, 64);
    assert_eq!(snapshot.manifest_digest_hex, "deadbeef");
    assert_eq!(snapshot.rotations_total, 1);
    assert!(snapshot.last_updated_unix > 0);
}
#[test]
fn record_da_rent_quote_metrics_accumulates_values() {
    let (metrics, telemetry) = telemetry_handle_for_tests();
    let quote = DaRentQuote {
        base_rent: XorQuantity::try_from_micro(1_000_000)
            .expect("legacy micro-XOR value is representable"),
        protocol_reserve: XorQuantity::try_from_micro(250_000)
            .expect("legacy micro-XOR value is representable"),
        provider_reward: XorQuantity::try_from_micro(750_000)
            .expect("legacy micro-XOR value is representable"),
        pdp_bonus: XorQuantity::try_from_micro(50_000)
            .expect("legacy micro-XOR value is representable"),
        potr_bonus: XorQuantity::try_from_micro(25_000)
            .expect("legacy micro-XOR value is representable"),
        egress_credit_per_gib: XorQuantity::try_from_micro(1_500)
            .expect("legacy micro-XOR value is representable"),
    };
    record_da_rent_quote_metrics(&telemetry, "cluster-a", StorageClass::Warm, 4, 3, &quote);
    let dump = metrics.try_to_string().expect("metrics text");
    let gib_line = find_metric_line(
        &dump,
        "torii_da_rent_gib_months_total{cluster=\"cluster-a\"",
    );
    assert!(gib_line.contains(r#"storage_class="warm""#));
    let gib_months = parse_metric_value(gib_line);
    assert!(
        (gib_months - 12.0).abs() < METRIC_ASSERT_EPSILON,
        "expected 12 GiB-months recorded"
    );
    for (metric, expected) in [
        ("torii_da_rent_base_micro_total", 1_000_000.0),
        ("torii_da_protocol_reserve_micro_total", 250_000.0),
        ("torii_da_provider_reward_micro_total", 750_000.0),
        ("torii_da_pdp_bonus_micro_total", 50_000.0),
        ("torii_da_potr_bonus_micro_total", 25_000.0),
    ] {
        let line = find_metric_line(
            &dump,
            &format!("{metric}{{cluster=\"cluster-a\",storage_class=\"warm\""),
        );
        let value = parse_metric_value(line);
        assert!(
            (value - expected).abs() < METRIC_ASSERT_EPSILON,
            "metric {metric} expected {expected}, got {value}"
        );
    }
}
#[test]
fn record_da_chunking_metrics_observes_histogram() {
    let (metrics, telemetry) = telemetry_handle_for_tests();
    record_da_chunking_metrics(&telemetry, Duration::from_millis(150));
    let samples = metrics.torii_da_chunking_seconds.get_sample_count();
    assert_eq!(samples, 1);
}
#[cfg(feature = "telemetry")]
#[tokio::test]
async fn da_rent_metrics_exposed_via_metrics_handler_snapshot() {
    let (metrics, telemetry) = telemetry_handle_for_tests_with_profile(TelemetryProfile::Extended);
    let quote = DaRentQuote {
        base_rent: XorQuantity::try_from_micro(1_000_000)
            .expect("legacy micro-XOR value is representable"),
        protocol_reserve: XorQuantity::try_from_micro(250_000)
            .expect("legacy micro-XOR value is representable"),
        provider_reward: XorQuantity::try_from_micro(750_000)
            .expect("legacy micro-XOR value is representable"),
        pdp_bonus: XorQuantity::try_from_micro(50_000)
            .expect("legacy micro-XOR value is representable"),
        potr_bonus: XorQuantity::try_from_micro(25_000)
            .expect("legacy micro-XOR value is representable"),
        egress_credit_per_gib: XorQuantity::try_from_micro(1_500)
            .expect("legacy micro-XOR value is representable"),
    };
    record_da_rent_quote_metrics(&telemetry, "cluster-a", StorageClass::Warm, 4, 3, &quote);
    let prometheus = crate::handle_metrics(&telemetry)
        .await
        .expect("prometheus snapshot");
    let snapshot = da_rent_metric_lines(&prometheus);
    assert_eq!(
        snapshot,
        vec![
            "# HELP torii_da_pdp_bonus_micro_total Aggregate PDP bonuses (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            "# HELP torii_da_potr_bonus_micro_total Aggregate PoTR bonuses (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            "# HELP torii_da_protocol_reserve_micro_total Aggregate protocol reserve (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            "# HELP torii_da_provider_reward_micro_total Aggregate provider rewards (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            "# HELP torii_da_rent_base_micro_total Aggregate base rent (micro XOR) quoted by DA ingest grouped by cluster and storage class",
            "# HELP torii_da_rent_gib_months_total Aggregate GiB-month usage quoted by DA ingest grouped by cluster and storage class",
            "# TYPE torii_da_pdp_bonus_micro_total counter",
            "# TYPE torii_da_potr_bonus_micro_total counter",
            "# TYPE torii_da_protocol_reserve_micro_total counter",
            "# TYPE torii_da_provider_reward_micro_total counter",
            "# TYPE torii_da_rent_base_micro_total counter",
            "# TYPE torii_da_rent_gib_months_total counter",
            "torii_da_pdp_bonus_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 50000",
            "torii_da_potr_bonus_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 25000",
            "torii_da_protocol_reserve_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 250000",
            "torii_da_provider_reward_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 750000",
            "torii_da_rent_base_micro_total{cluster=\"cluster-a\",storage_class=\"warm\"} 1000000",
            "torii_da_rent_gib_months_total{cluster=\"cluster-a\",storage_class=\"warm\"} 12"
        ],
        "DA rent Prometheus payload drifted"
    );
    let dump = metrics.try_to_string().expect("metrics text");
    for line in snapshot {
        assert!(
            dump.contains(&line),
            "metrics text missing `{line}`\n{dump}"
        );
    }
}
#[test]
fn record_da_receipt_metrics_tracks_outcomes_and_cursor() {
    let (metrics, telemetry) = telemetry_handle_for_tests();
    let lane_epoch = LaneEpoch::new(LaneId::new(7), 3);
    record_da_receipt_metrics(
        &telemetry,
        lane_epoch,
        5,
        &ReceiptInsertOutcome::Stored {
            cursor_advanced: true,
        },
    );
    record_da_receipt_metrics(
        &telemetry,
        lane_epoch,
        5,
        &ReceiptInsertOutcome::Duplicate {
            path: std::path::PathBuf::new(),
        },
    );
    record_da_receipt_metrics(
        &telemetry,
        lane_epoch,
        5,
        &ReceiptInsertOutcome::DuplicateFingerprintConflict {
            path: std::path::PathBuf::new(),
            expected: test_fingerprint(0xA1),
            observed: test_fingerprint(0xA2),
        },
    );
    record_da_receipt_metrics(
        &telemetry,
        lane_epoch,
        5,
        &ReceiptInsertOutcome::ReceiptConflict {
            path: std::path::PathBuf::new(),
        },
    );
    record_da_receipt_metrics(
        &telemetry,
        lane_epoch,
        6,
        &ReceiptInsertOutcome::SequenceGap {
            expected_next: 6,
            observed: 7,
        },
    );
    let stored = metrics
        .torii_da_receipts_total
        .with_label_values(&["stored", "7"])
        .get();
    assert_eq!(stored, 1, "stored counter should increment");
    let duplicate = metrics
        .torii_da_receipts_total
        .with_label_values(&["duplicate", "7"])
        .get();
    assert_eq!(duplicate, 1, "duplicate counter should increment");
    let duplicate_fingerprint_conflict = metrics
        .torii_da_receipts_total
        .with_label_values(&["duplicate_fingerprint_conflict", "7"])
        .get();
    assert_eq!(
        duplicate_fingerprint_conflict, 1,
        "duplicate fingerprint conflict counter should increment"
    );
    let receipt_conflict = metrics
        .torii_da_receipts_total
        .with_label_values(&["receipt_conflict", "7"])
        .get();
    assert_eq!(
        receipt_conflict, 1,
        "receipt conflict counter should increment"
    );
    let sequence_gap = metrics
        .torii_da_receipts_total
        .with_label_values(&["sequence_gap", "7"])
        .get();
    assert_eq!(sequence_gap, 1, "sequence gap counter should increment");
    let epoch = metrics
        .torii_da_receipt_epoch
        .with_label_values(&["7"])
        .get();
    assert_eq!(epoch, 3, "epoch gauge should reflect the current epoch");
    let cursor = metrics
        .torii_da_receipt_highest_sequence
        .with_label_values(&["7"])
        .get();
    assert_eq!(cursor, 5, "cursor gauge should reflect stored sequence");
}
include!("tests/receipt_outcome_tests.rs");
