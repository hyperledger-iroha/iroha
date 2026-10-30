//! Real private-file admission and explicit staging controls; no network/prover qualification.

use super::*;
use iroha_fs::{PrivateDirectory, PublishMode};
use std::num::NonZeroUsize;

fn write(directory: &PrivateDirectory, name: &str, bytes: &[u8]) -> PathBuf {
    directory
        .write_atomic(name, bytes, PublishMode::CreateNew)
        .unwrap();
    directory.path().join(name)
}
fn json<T: norito::json::JsonSerialize>(value: &T) -> Vec<u8> {
    norito::json::to_vec(value).unwrap()
}
fn limits() -> NativePrivateSettlementLimits {
    NativePrivateSettlementLimits {
        maximum_sessions: NonZeroU16::new(1).unwrap(),
        maximum_original_bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
        maximum_artifact_bytes: NonZeroUsize::new(65 * 1024 * 1024).unwrap(),
        maximum_prepared_bytes: NonZeroUsize::new(64 * 1024 * 1024).unwrap(),
    }
}
fn fixture(path: &Path) -> (OpenDescriptor, Vec<u8>) {
    let directory = PrivateDirectory::open_or_create(path.join("sources")).unwrap();
    let root = path.join("custody");
    let (original, wire) = opening::tests::originals(&root);
    let pins = original.genesis_pins;
    let request = &original.request.0;
    let key_file = write(
        &directory,
        "account.key",
        b"802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53\n",
    );
    let config = format!(
        r#"chain = "{}"
network_id = "{}"
torii_url = "http://127.0.0.1:1/"
[account]
chain_discriminant = 753
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
private_key_file = "{}"
"#,
        pins.chain_id,
        pins.network_id,
        key_file.display()
    );
    let client_file = write(&directory, "client.toml", config.as_bytes());
    let request_file = write(
        &directory,
        "request.json",
        &json(&RequestFile {
            layout: "iroha.private-settlement.request.v1".into(),
            intent: request.intent.clone(),
            terms: request
                .terms
                .iter()
                .map(|t| TermsFile {
                    asset: t.asset.clone(),
                    payer: t.payer.clone(),
                    recipient: t.recipient.clone(),
                    amount: t.amount,
                    reimbursement: t.reimbursement,
                    memo: t.memo.clone(),
                })
                .collect(),
            schema: request.schema,
            minimum_context_time_ms: request.minimum_context_time_ms,
            endpoints: request
                .endpoints
                .iter()
                .map(|row| row.iter().map(ToString::to_string).collect())
                .collect(),
            padding: request.padding,
            retention_height: request.retention_height,
            max_carrier_bytes: request.max_carrier_bytes,
            max_artifact_bytes: request.max_artifact_bytes as u64,
            max_total_artifact_bytes: request.max_total_artifact_bytes as u64,
        }),
    );
    let pins_file = write(
        &directory,
        "pins.json",
        &json(&PinsFile {
            layout: "iroha.private-settlement.pins.v1".into(),
            chain_id: pins.chain_id,
            network_id: pins.network_id,
            genesis_hash: pins.genesis_hash,
            signed_genesis_sha256: pins.signed_genesis_sha256,
            genesis_public_key: pins.genesis_public_key,
            roster: pins
                .roster
                .iter()
                .map(|peer| peer.public_key().clone())
                .collect(),
            mode: pins.mode,
            world_schema: original.world_schema,
        }),
    );
    let descriptor = OpenDescriptor {
        layout: "iroha.private-settlement.files.v1".into(),
        client_file,
        request_file,
        pins_file,
        genesis_file: write(&directory, "genesis.norito", &wire),
        custody_key_file: write(&directory, "custody.key", &[61; 32]),
        custody_root: root,
        maximum_wallet_bytes: 1024 * 1024,
    };
    let bytes = json(&descriptor);
    (descriptor, bytes)
}
async fn open(
    bytes: &[u8],
    limits: NativePrivateSettlementLimits,
) -> Result<opening::Opened, OneCallPrivateSettlementError> {
    let input = FileInput::capture(bytes, limits.maximum_original_bytes.get()).unwrap();
    let permit = Arc::new(Semaphore::new(1)).acquire_owned().await.unwrap();
    run_native_worker(permit, (), move || input.open(limits))
        .await
        .unwrap()
        .0
}

#[test]
fn closed_descriptors_refuse_unknown_duplicate_and_first_over_before_copy() {
    let bytes=br#"{"layout":"iroha.private-settlement.stage.v1","ordinal":0,"reservation_file":"/r","opening_file":"/o"}"#;
    assert!(parse::<StageDescriptor>(bytes).is_ok());
    for suffix in [",\"witness\":[]}", ",\"ordinal\":0}"] {
        let mut changed = bytes[..bytes.len() - 1].to_vec();
        changed.extend_from_slice(suffix.as_bytes());
        assert!(parse::<StageDescriptor>(&changed).is_err());
    }
    assert!(FileInput::capture(bytes, bytes.len()).is_ok());
    assert!(
        FileInput::capture(
            &vec![b' '; NATIVE_PRIVATE_SETTLEMENT_DESCRIPTOR_MAX_BYTES],
            usize::MAX
        )
        .is_ok()
    );
    assert!(FileInput::capture(bytes, bytes.len() - 1).is_err());
    assert!(
        FileInput::capture(
            &vec![b' '; NATIVE_PRIVATE_SETTLEMENT_DESCRIPTOR_MAX_BYTES + 1],
            usize::MAX
        )
        .is_err()
    );
}

#[test]
fn referenced_private_keys_debit_the_same_cumulative_budget_without_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let (descriptor, _) = fixture(temp.path());
    let bytes = iroha_fs::read_private(&descriptor.client_file, 1024 * 1024).unwrap();
    let key_bytes = iroha_fs::read_private(
        descriptor.client_file.parent().unwrap().join("account.key"),
        4096,
    )
    .unwrap()
    .len();
    let complete = SourceReads::new(bytes.len() + key_bytes, bytes.len()).unwrap();
    Config::load_bytes_with_musubi_publication_and_file_source(
        &descriptor.client_file,
        &bytes,
        &complete,
    )
    .unwrap();
    assert_eq!(complete.remaining.get(), 0);
    let short = SourceReads::new(bytes.len() + key_bytes - 1, bytes.len()).unwrap();
    assert!(
        Config::load_bytes_with_musubi_publication_and_file_source(
            &descriptor.client_file,
            &bytes,
            &short
        )
        .is_err()
    );
    assert!(!descriptor.custody_root.exists());
}

#[tokio::test(flavor = "current_thread")]
async fn exact_original_files_reopen_without_staging_and_corrupt_pins_create_no_custody() {
    let temp = tempfile::tempdir().unwrap();
    let (descriptor, bytes) = fixture(temp.path());
    let first = open(&bytes, limits()).await.unwrap();
    let digest = first.operation.driver.session_request().0.digest;
    assert!(first.staging.is_some());
    drop(first);
    let reopened = open(&bytes, limits()).await.unwrap();
    assert_eq!(digest, reopened.operation.driver.session_request().0.digest);
    for name in ["wallet", "artifacts", "workflows"] {
        assert_eq!(
            std::fs::read_dir(descriptor.custody_root.join(name))
                .unwrap()
                .count(),
            0
        );
    }
    let temp2 = tempfile::tempdir().unwrap();
    let (descriptor2, bytes2) = fixture(temp2.path());
    let mut pins: PinsFile =
        parse(&iroha_fs::read_private(&descriptor2.pins_file, 1024 * 1024).unwrap()).unwrap();
    pins.signed_genesis_sha256[0] ^= 1;
    let directory =
        PrivateDirectory::open_or_create(descriptor2.pins_file.parent().unwrap()).unwrap();
    directory
        .write_atomic("pins.json", &json(&pins), PublishMode::Replace)
        .unwrap();
    assert!(open(&bytes2, limits()).await.is_err());
    assert!(!descriptor2.custody_root.exists());
}

#[tokio::test(flavor = "current_thread")]
async fn supervised_file_open_and_zero_read_recovery_do_not_provision() {
    let temp = tempfile::tempdir().unwrap();
    let (descriptor, bytes) = fixture(temp.path());
    let host = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
    let ticket = host.admit().unwrap().open_files(&bytes).unwrap();
    assert_eq!(
        host.wait(ticket).await.unwrap(),
        NativePrivateSettlementStatus::Ready
    );
    assert!(matches!(
        host.take(ticket).unwrap(),
        NativePrivateSettlementOutcome::Opened
    ));
    let mut options = PrivateSettlementRunOptions::default();
    options.max_finality_proof_reads = 0;
    for command in [
        NativePrivateSettlementCommand::Prepared,
        NativePrivateSettlementCommand::Recover,
    ] {
        let call = host.start(ticket.session(), command, options).unwrap();
        host.wait(call).await.unwrap();
        assert!(matches!(
            host.take(call).unwrap(),
            NativePrivateSettlementOutcome::NotPrepared
                | NativePrivateSettlementOutcome::Pending(None)
        ));
    }
    for name in ["wallet", "artifacts", "workflows"] {
        assert_eq!(
            std::fs::read_dir(descriptor.custody_root.join(name))
                .unwrap()
                .count(),
            0
        );
    }
    host.shutdown().await.unwrap();
}

fn stage_fixture(path: &Path) -> (OpenDescriptor, Vec<u8>, Vec<u8>) {
    use iroha_core_privacy::privacy_engines::atomic_private_settlement::{
        atomic_private_settlement_reservation_fixture_v1,
        encode_atomic_private_settlement_reservation_v1,
    };
    use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
    let (descriptor, _) = fixture(path);
    let directory =
        PrivateDirectory::open_or_create(descriptor.request_file.parent().unwrap()).unwrap();
    let old: RequestFile =
        parse(&iroha_fs::read_private(&descriptor.request_file, 4 * 1024 * 1024).unwrap()).unwrap();
    let (intent, governance, policy, authorized) =
        atomic_private_settlement_reservation_fixture_v1(Some(old.intent.network_id));
    let plain = authorized.plaintext();
    let terms = (0..intent.legs.len())
        .map(|index| TermsFile {
            asset: plain.asset_definition_id.clone(),
            payer: plain.payer.clone(),
            recipient: plain.recipient.clone(),
            amount: plain.amount,
            reimbursement: if index == 0 {
                plain.sponsor_reimbursement_amount
            } else {
                0
            },
            memo: plain.memo.clone(),
        })
        .collect();
    let (material, _) =
        encode_atomic_private_settlement_reservation_v1(&intent, 0, &authorized).unwrap();
    let request = RequestFile {
        layout: old.layout,
        intent,
        terms,
        schema: old.schema,
        minimum_context_time_ms: 0,
        endpoints: old.endpoints,
        padding: PrivateSettlementCapsulePaddingV1::KiB16,
        retention_height: 100_000,
        max_carrier_bytes: old.max_carrier_bytes,
        max_artifact_bytes: old.max_artifact_bytes,
        max_total_artifact_bytes: old.max_total_artifact_bytes,
    };
    directory
        .write_atomic("request.json", &json(&request), PublishMode::Replace)
        .unwrap();
    let sponsor = KeyPair::from_seed(vec![0x23; 32], Algorithm::Ed25519);
    directory
        .write_atomic(
            "account.key",
            ExposedPrivateKey(sponsor.private_key().clone())
                .to_string()
                .as_bytes(),
            PublishMode::Replace,
        )
        .unwrap();
    let config =
        std::str::from_utf8(&iroha_fs::read_private(&descriptor.client_file, 1024 * 1024).unwrap())
            .unwrap()
            .replace(
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
                &sponsor.public_key().to_string(),
            );
    directory
        .write_atomic("client.toml", config.as_bytes(), PublishMode::Replace)
        .unwrap();
    let stage = StageDescriptor {
        layout: "iroha.private-settlement.stage.v1".into(),
        ordinal: 0,
        reservation_file: write(&directory, "reservation.norito", &material),
        opening_file: write(
            &directory,
            "opening.norito",
            &norito::encode_canonical(&(governance, policy)).unwrap(),
        ),
    };
    let open = json(&descriptor);
    (descriptor, open, json(&stage))
}
fn wallet_snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut entries = std::fs::read_dir(root.join("wallet"))
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}
#[tokio::test(flavor = "current_thread")]
async fn explicit_native_stage_is_bound_idempotent_and_preserved_by_cold_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let (descriptor, open, stage) = stage_fixture(temp.path());
    let host = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
    let original = host.admit().unwrap().open_files(&open).unwrap();
    host.wait(original).await.unwrap();
    assert!(matches!(
        host.take(original).unwrap(),
        NativePrivateSettlementOutcome::Opened
    ));
    assert!(wallet_snapshot(&descriptor.custody_root).is_empty());
    let mut snapshot = None;
    for _ in 0..2 {
        let ticket = host.stage_files(original.session(), &stage).unwrap();
        host.wait(ticket).await.unwrap();
        assert!(matches!(
            host.take(ticket).unwrap(),
            NativePrivateSettlementOutcome::Staged
        ));
        let current = wallet_snapshot(&descriptor.custody_root);
        assert!(!current.is_empty());
        if let Some(saved) = &snapshot {
            assert_eq!(saved, &current);
        }
        snapshot = Some(current);
    }
    let mut wrong: StageDescriptor = parse(&stage).unwrap();
    wrong.ordinal = 1;
    let ticket = host.stage_files(original.session(), &json(&wrong)).unwrap();
    host.wait(ticket).await.unwrap();
    assert!(matches!(
        host.take(ticket).unwrap(),
        NativePrivateSettlementOutcome::Failed(_)
    ));
    assert_eq!(
        snapshot.as_ref().unwrap(),
        &wallet_snapshot(&descriptor.custody_root)
    );
    host.shutdown().await.unwrap();
    let host = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
    let original = host.admit().unwrap().open_files(&open).unwrap();
    host.wait(original).await.unwrap();
    assert!(matches!(
        host.take(original).unwrap(),
        NativePrivateSettlementOutcome::Opened
    ));
    let mut options = PrivateSettlementRunOptions::default();
    options.max_finality_proof_reads = 0;
    let call = host
        .start(
            original.session(),
            NativePrivateSettlementCommand::Recover,
            options,
        )
        .unwrap();
    host.wait(call).await.unwrap();
    assert!(matches!(
        host.take(call).unwrap(),
        NativePrivateSettlementOutcome::Pending(None)
    ));
    assert_eq!(
        snapshot.as_ref().unwrap(),
        &wallet_snapshot(&descriptor.custody_root)
    );
    host.shutdown().await.unwrap();
}
#[test]
fn accepted_stage_survives_discarded_wait_and_close_waiters() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let (descriptor, open, stage) = stage_fixture(temp.path());
        let host = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
        let original = host.admit().unwrap().open_files(&open).unwrap();
        host.wait(original).await.unwrap();
        assert!(matches!(
            host.take(original).unwrap(),
            NativePrivateSettlementOutcome::Opened
        ));
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let blocker = tokio::task::spawn_blocking(move || {
            entered_tx.send(()).unwrap();
            let _ = release_rx.recv();
        });
        entered_rx.await.unwrap();
        let accepted = host.stage_files(original.session(), &stage).unwrap();
        drop(host.wait(accepted));
        drop(host.close(original.session()));
        assert!(matches!(
            host.admit(),
            Err(NativePrivateSettlementSessionError::Capacity)
        ));
        assert!(wallet_snapshot(&descriptor.custody_root).is_empty());
        drop(release_tx);
        blocker.await.unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            host.close(original.session()),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!wallet_snapshot(&descriptor.custody_root).is_empty());
        drop(host.admit().unwrap());
        host.shutdown().await.unwrap();
    });
}

#[tokio::test(flavor = "current_thread")]
async fn typed_custom_operation_cannot_acquire_opener_staging_capability() {
    let temp = tempfile::tempdir().unwrap();
    let (descriptor, files, stage) = stage_fixture(temp.path());
    let opened = open(&files, limits()).await.unwrap();
    let host = NativePrivateSettlementHost::new(Handle::current(), limits()).unwrap();
    let ticket = host.admit().unwrap().open(opened.operation).unwrap();
    host.wait(ticket).await.unwrap();
    assert!(matches!(
        host.take(ticket).unwrap(),
        NativePrivateSettlementOutcome::Opened
    ));
    assert!(matches!(
        host.stage_files(ticket.session(), &stage),
        Err(NativePrivateSettlementSessionError::StagingUnavailable)
    ));
    assert!(wallet_snapshot(&descriptor.custody_root).is_empty());
    host.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn complete_file_budget_refuses_before_any_custody_publication() {
    let temp = tempfile::tempdir().unwrap();
    let (descriptor, files) = fixture(temp.path());
    let directory = descriptor.client_file.parent().unwrap();
    let total = files.len()
        + std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().metadata().unwrap().len() as usize)
            .sum::<usize>();
    let mut narrow = limits();
    narrow.maximum_original_bytes = NonZeroUsize::new(total - 1).unwrap();
    assert!(open(&files, narrow).await.is_err());
    assert!(!descriptor.custody_root.exists());
    // Byte admission is independent of authority authentication and the typed original size.
    // A larger local allowance retains the exact selected files and identity.
    let wider = open(&files, limits()).await.unwrap();
    assert!(wider.staging.is_some());
}
