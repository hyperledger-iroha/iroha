//! Shared opener controls over genuine signed genesis; no proof or payment is fabricated here.

use super::*;
use crate::test_support::client_with_base_url;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
use std::{path::Path, time::Duration};

fn limits() -> NativePrivateSettlementLimits {
    NativePrivateSettlementLimits {
        maximum_sessions: NonZeroU16::new(1).unwrap(),
        maximum_original_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        maximum_artifact_bytes: NonZeroUsize::new(65 * 1024 * 1024).unwrap(),
        maximum_prepared_bytes: NonZeroUsize::new(64 * 1024 * 1024).unwrap(),
    }
}

pub(in crate::native_session) fn originals(
    root: &Path,
) -> (NativePrivateSettlementOriginals, Vec<u8>) {
    let native = NativeFinalityFixture::start("private-native-opening");
    let mut builder =
        client_with_base_url(&"http://opening.invalid/".parse().unwrap()).to_builder();
    builder.network_id = native.network_id();
    builder.chain = native.chain_id().parse().unwrap();
    let client = builder.build().unwrap();
    let (context, intent) = crate::cold_context_tests::context(&client);
    let request = crate::cold_context_tests::request(&client, &context, &intent);
    let schema = request.0.schema;
    let wire = native.genesis().encode_wire().unwrap();
    let epoch = native.verifier().initial_epoch().clone();
    let pins = SignedGenesisPinsV1 {
        chain_id: native.chain_id().to_owned(),
        network_id: native.network_id(),
        genesis_hash: native.genesis().hash(),
        signed_genesis_sha256: iroha_crypto::sha256(&wire),
        genesis_public_key: KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
        roster: epoch
            .committee
            .iter()
            .map(|member| member.validator.clone())
            .collect(),
        mode: epoch.mode,
    };
    (
        NativePrivateSettlementOriginals::new(
            client.account_client().unwrap(),
            request,
            pins,
            schema,
            NativePrivateSettlementStores::new(
                root.to_owned(),
                Zeroizing::new([61; 32]),
                NonZeroUsize::new(1024 * 1024).unwrap(),
            ),
        ),
        wire,
    )
}

async fn construct(
    source: OpenSource,
    limits: NativePrivateSettlementLimits,
) -> Result<OneCallPrivateSettlementOperation, OneCallPrivateSettlementError> {
    let permit = Arc::new(Semaphore::new(1)).acquire_owned().await.unwrap();
    run_native_worker(permit, (), move || source.construct(limits))
        .await
        .unwrap()
        .0
}

#[tokio::test(flavor = "current_thread")]
async fn independently_pinned_originals_refuse_before_any_filesystem_open() {
    let directory = tempfile::tempdir().unwrap();
    for changed in 0..7 {
        let root = directory.path().join(format!("refused-{changed}"));
        let (mut original, mut wire) = originals(&root);
        match changed {
            0 => original.world_schema = Hash::new(b"independently different schema"),
            1 => original.genesis_pins.signed_genesis_sha256[0] ^= 1,
            2 => original.genesis_pins.roster.swap(0, 1),
            3 => wire[0] ^= 1,
            4 => original.genesis_pins.chain_id.push_str("-foreign"),
            5 => original.stores.key.fill(0),
            6 => {
                original.account =
                    client_with_base_url(&"http://foreign.invalid/".parse().unwrap())
                        .account_client()
                        .unwrap();
            }
            _ => unreachable!(),
        }
        let source = OpenSource::capture(original, &wire, limits()).unwrap();
        assert!(
            matches!(
                construct(source, limits()).await,
                Err(OneCallPrivateSettlementError::Binding
                    | OneCallPrivateSettlementError::FinalityRead(_))
            ),
            "changed original {changed}"
        );
        assert!(
            !root.exists(),
            "refused original {changed} must create no custody"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn complete_original_and_custody_allowances_refuse_before_open_at_first_over() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("capacity");
    let (original, wire) = originals(&root);
    let exact = original.original_bytes(wire.len()).unwrap();
    let mut narrow = limits();
    narrow.maximum_original_bytes = NonZeroUsize::new(exact - 1).unwrap();
    assert!(matches!(
        OpenSource::capture(original, &wire, narrow),
        Err(NativePrivateSettlementSessionError::Capacity)
    ));
    assert!(!root.exists());
    let (original, wire) = originals(&root);
    narrow = limits();
    narrow.maximum_artifact_bytes = NonZeroUsize::new(65 * 1024 * 1024 - 1).unwrap();
    assert!(matches!(
        OpenSource::capture(original, &wire, narrow),
        Err(NativePrivateSettlementSessionError::Capacity)
    ));
    assert!(!root.exists());
    let (original, wire) = originals(&root);
    let mut exact_limits = limits();
    exact_limits.maximum_original_bytes = NonZeroUsize::new(exact).unwrap();
    let source = OpenSource::capture(original, &wire, exact_limits).unwrap();
    let operation = construct(source, exact_limits).await.unwrap();
    let options = PrivateSettlementRunOptions {
        max_finality_proof_reads: 0,
        ..PrivateSettlementRunOptions::default()
    };
    assert!(
        operation
            .prepared_request(&options)
            .await
            .unwrap()
            .is_none()
    );
    let recovered = operation.recover(&options).await.unwrap();
    assert!(recovered.outcome.is_none() && recovered.progress.is_none());
    for leaf in ["workflows", "artifacts", "wallet"] {
        assert_eq!(std::fs::read_dir(root.join(leaf)).unwrap().count(), 0);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn admitted_open_source_and_cold_reopen_share_supervision_without_provisioning() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("custody");
    let (original, wire) = originals(&root);
    let mut request_identity = None;
    for _ in 0..2 {
        let host = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
        let admission = host.admit().unwrap();
        // Reopen the independently selected originals, including the same restricted request.
        // Re-running a fixture producer can legitimately select fresh authorization randomness.
        let reopening = NativePrivateSettlementOriginals::new(
            original.account.clone(),
            original.request.clone(),
            original.genesis_pins.clone(),
            original.world_schema,
            NativePrivateSettlementStores::new(
                original.stores.root.clone(),
                Zeroizing::new(*original.stores.key),
                original.stores.maximum_wallet_bytes,
            ),
        );
        let expected = original.request.0.digest;
        // This fixture has a transport that panics on every request. Opening and recovering
        // a never-started original must neither fetch context nor create wallet records.
        let ticket = admission.open_source(reopening, &wire).unwrap();
        drop(host.wait(ticket));
        assert_eq!(
            host.wait(ticket).await.unwrap(),
            NativePrivateSettlementStatus::Ready
        );
        assert!(matches!(
            host.take(ticket).unwrap(),
            NativePrivateSettlementOutcome::Opened
        ));
        assert_eq!(request_identity.get_or_insert(expected), &expected);
        let recovery = host
            .start(
                ticket.session(),
                NativePrivateSettlementCommand::Recover,
                PrivateSettlementRunOptions {
                    max_finality_proof_reads: 0,
                    ..PrivateSettlementRunOptions::default()
                },
            )
            .unwrap();
        host.wait(recovery).await.unwrap();
        assert!(matches!(
            host.take(recovery).unwrap(),
            NativePrivateSettlementOutcome::Pending(None)
        ));
        host.close(ticket.session()).await.unwrap();
        host.shutdown().await.unwrap();
        for leaf in ["workflows", "artifacts", "wallet"] {
            assert_eq!(std::fs::read_dir(root.join(leaf)).unwrap().count(), 0);
        }
    }
}

#[test]
fn queued_open_source_close_keeps_original_admission_until_native_cleanup() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("queued");
        let host = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
        let admission = host.admit().unwrap();
        let (original, wire) = originals(&root);
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let blocker = tokio::task::spawn_blocking(move || {
            entered_tx.send(()).unwrap();
            let _ = release_rx.recv();
        });
        entered_rx.await.unwrap();
        let ticket = admission.open_source(original, &wire).unwrap();
        drop(host.wait(ticket));
        drop(host.close(ticket.session()));
        assert_eq!(
            host.status(ticket).unwrap(),
            NativePrivateSettlementStatus::Closing
        );
        assert!(matches!(
            host.admit(),
            Err(NativePrivateSettlementSessionError::Capacity)
        ));
        assert!(!root.exists());
        drop(release_tx);
        blocker.await.unwrap();
        tokio::time::timeout(Duration::from_secs(10), host.close(ticket.session()))
            .await
            .unwrap()
            .unwrap();
        // This accepted construction was drained despite both discarded waiters. Opening
        // alone writes no journal, reservation, preparation or outbound transaction.
        for leaf in ["workflows", "artifacts", "wallet"] {
            assert_eq!(std::fs::read_dir(root.join(leaf)).unwrap().count(), 0);
        }
        drop(host.admit().unwrap());
        host.shutdown().await.unwrap();
    });
}

#[test]
fn genesis_capture_and_original_debug_are_bounded_and_redacted() {
    let directory = tempfile::tempdir().unwrap();
    let (original, wire) = originals(directory.path());
    assert_eq!(
        format!("{original:?}"),
        "NativePrivateSettlementOriginals(<restricted>)"
    );
    assert_eq!(
        format!("{:?}", original.stores),
        "NativePrivateSettlementStores(<private>)"
    );
    let mut narrow = limits();
    narrow.maximum_original_bytes = NonZeroUsize::new(wire.len() - 1).unwrap();
    assert!(matches!(
        OpenSource::capture(original, &wire, narrow),
        Err(NativePrivateSettlementSessionError::Capacity)
    ));
    let (original, _) = originals(directory.path());
    assert!(matches!(
        OpenSource::capture(original, &[], limits()),
        Err(NativePrivateSettlementSessionError::Capacity)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn closed_admission_refuses_before_source_count_copy_or_queue() {
    let directory = tempfile::tempdir().unwrap();
    for whole_host in [false, true] {
        let root = directory
            .path()
            .join(if whole_host { "host" } else { "session" });
        let host = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
        let admission = host.admit().unwrap();
        let (original, _) = originals(&root);
        let closed = if whole_host {
            Some(host.shutdown())
        } else {
            // Both close and shutdown mark the original synchronously before their wait.
            drop(host.close(admission.id()));
            None
        };
        // An empty frame would be Capacity if capture were entered. Closing must win before
        // the borrowed source is counted, copied or accepted as a command.
        assert!(matches!(
            admission.open_source(original, &[]),
            Err(NativePrivateSettlementSessionError::Closing)
        ));
        if let Some(closed) = closed {
            closed.await.unwrap();
        } else {
            host.shutdown().await.unwrap();
        }
        assert!(!root.exists());
    }
}
