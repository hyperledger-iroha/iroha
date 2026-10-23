//! Genuine native custody through the automatic bridge worker; simulated host keys only.
//!
//! The ignored consumer needs a separately completed CoreZK handoff and independently
//! selected current source pins. A locator, a producer's JSON claim or a scripted Wallet
//! never supplies source authority. No explicit drain, fold or resume drives this consumer.

use super::*;
use advance::*;
use iroha_core_zk::kagemusha_wallet_artifacts_v1::{
    InstallationV1, InstalledVerifierPackV1,
    producer_inventory::{
        BlobV1, CATALOG_MAX_BYTES_V1, DirectoryOriginalsV1, PROVING_KEY_MAX_BYTES_V1,
    },
};
use iroha_crypto::{Algorithm, KeyPair, Signature, sha256_reader_bounded};
use iroha_data_model::{
    account::AccountId,
    block::decode_framed_signed_block,
    sumeragi_finality::{FinalityValidator, SumeragiFinalityVerifier, authenticated_genesis},
};
use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::keys::pk::{CosetCachePolicy, artifact::ReadConfig};
use norito::json::Value;
use p256::ecdsa::{Signature as DeviceSignature, SigningKey, signature::Signer as _};
use std::{
    fs::{self, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

const SOURCE_PINS: [&str; 7] = [
    "KAGEMUSHA_SIGNED_GENESIS_FIXTURE_SHA256",
    "KAGEMUSHA_WALLET_CATALOG_PRODUCER_SHA256",
    "KAGEMUSHA_WALLET_CATALOG_SOURCE_SHA256",
    "KAGEMUSHA_WALLET_CATALOG_INVENTORY_SHA256",
    "KAGEMUSHA_WALLET_CATALOG_PACK_SHA256",
    "KAGEMUSHA_WALLET_SCHEME_ID",
    "KAGEMUSHA_WALLET_MANIFEST_DIGEST",
];
const FRAME_NAMES: [&str; 4] = [
    "credential.norito",
    "certificates.norito",
    "account.norito",
    "asset.norito",
];

fn qualification_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/qualification")
        .canonicalize()
        .unwrap()
}
fn selected_path(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} required")));
    require_path(&path);
    path
}
fn require_path(path: &Path) {
    assert!(path.is_absolute());
    assert!(!path.components().any(|part| matches!(
        part,
        std::path::Component::ParentDir | std::path::Component::CurDir
    )));
    assert!(
        path.parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .starts_with(qualification_root())
    );
}
fn pin(name: &str) -> [u8; 32] {
    unhex(&std::env::var(name).unwrap_or_else(|_| panic!("{name} required")))
}
fn unhex(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    assert!(
        value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    );
    hex::decode(value).unwrap().try_into().unwrap()
}
fn read(path: &Path, cap: usize) -> Vec<u8> {
    require_path(path);
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.file_type().is_file());
    assert!(metadata.len() <= cap as u64);
    let mut bytes = Vec::new();
    fs::File::open(path)
        .unwrap()
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= cap);
    bytes
}
fn read_pin(path: &Path, cap: usize, expected: [u8; 32]) -> Vec<u8> {
    let bytes = read(path, cap);
    assert_eq!(BlobV1::of(&bytes).sha256, expected);
    bytes
}
fn decode<T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>>(bytes: &[u8]) -> T {
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
        .unwrap()
}
fn publish(path: &Path, value: &Value) {
    let bytes = norito::json::to_vec(value).unwrap();
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path).unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();
    fs::File::open(path.parent().unwrap())
        .unwrap()
        .sync_all()
        .unwrap();
}
fn original(row: &Value, name: &str, cap: usize) -> Vec<u8> {
    let rows = row["originals"].as_array().unwrap();
    assert_eq!(rows.len(), 6);
    let mut matching = rows
        .iter()
        .filter(|entry| entry["name"].as_str() == Some(name));
    let entry = matching.next().unwrap();
    assert!(matching.next().is_none());
    let path = Path::new(row["originals_directory"].as_str().unwrap()).join(name);
    let bytes = read_pin(&path, cap, unhex(entry["sha256"].as_str().unwrap()));
    assert_eq!(entry["bytes"].as_u64(), Some(bytes.len() as u64));
    bytes
}

struct Sources {
    installation: InstallationV1,
    genesis: Arc<SumeragiFinalityVerifier>,
    originals: PathBuf,
    pack: Vec<u8>,
    inventory: Vec<u8>,
}
impl Sources {
    fn selected(handoff: &Value) -> Self {
        for name in SOURCE_PINS {
            assert_eq!(
                handoff["source_pins"][name].as_str(),
                Some(hex::encode(pin(name)).as_str())
            );
        }
        let fixture = read_pin(
            &selected_path("KAGEMUSHA_SIGNED_GENESIS_FIXTURE"),
            1 << 20,
            pin("KAGEMUSHA_SIGNED_GENESIS_FIXTURE_SHA256"),
        );
        let capture: Value = norito::json::from_slice(&fixture).unwrap();
        let wire = hex::decode(capture["signed_genesis_wire_hex"].as_str().unwrap()).unwrap();
        let genesis = decode_framed_signed_block(&wire).unwrap();
        let epoch = authenticated_genesis(&genesis).unwrap().into_parts().0;
        let roster = epoch
            .committee
            .iter()
            .map(|member| FinalityValidator {
                public_key: member.validator.public_key().clone(),
                proof_of_possession: member.proof_of_possession.clone(),
            })
            .collect();
        let native =
            SumeragiFinalityVerifier::new(&genesis, capture["chain_id"].as_str().unwrap(), roster)
                .unwrap();
        let catalog = selected_path("KAGEMUSHA_WALLET_SIGNED_CATALOG");
        assert_eq!(
            read(&catalog.join("binary.sha256"), 32),
            pin("KAGEMUSHA_WALLET_CATALOG_PRODUCER_SHA256")
        );
        assert_eq!(
            read(&catalog.join("source.sha256"), 32),
            pin("KAGEMUSHA_WALLET_CATALOG_SOURCE_SHA256")
        );
        let pack = read_pin(
            &catalog.join("engineering-verifier-pack.norito"),
            CATALOG_MAX_BYTES_V1 + (64 << 10),
            pin("KAGEMUSHA_WALLET_CATALOG_PACK_SHA256"),
        );
        let inventory = read_pin(
            &catalog.join("producer-inventory.norito"),
            CATALOG_MAX_BYTES_V1,
            pin("KAGEMUSHA_WALLET_CATALOG_INVENTORY_SHA256"),
        );
        assert_eq!(
            handoff["sealed_originals"]["schema"].as_str(),
            Some("iroha.kagemusha.sealed-proving-originals-locator.v1")
        );
        let originals = PathBuf::from(handoff["sealed_originals"]["path"].as_str().unwrap());
        require_path(&originals);
        Self {
            installation: InstallationV1 {
                scheme_id: pin("KAGEMUSHA_WALLET_SCHEME_ID"),
                manifest_digest: pin("KAGEMUSHA_WALLET_MANIFEST_DIGEST"),
            },
            genesis: Arc::new(native),
            originals,
            pack,
            inventory,
        }
    }
    fn config(&self) -> state::NativeInstallationConfigV1 {
        state::NativeInstallationConfigV1 {
            installation: self.installation,
            genesis: Arc::clone(&self.genesis),
            read: ReadConfig {
                maximum_bytes: PROVING_KEY_MAX_BYTES_V1,
                maximum_rows: 1 << 16,
                coset_cache: CosetCachePolicy::OnDemand,
                msm_budget: MemoryBudget::DEFAULT,
            },
            budget: MemoryBudget::DEFAULT,
        }
    }
    fn originals(&self) -> DirectoryOriginalsV1 {
        DirectoryOriginalsV1::open_existing(&self.originals, PROVING_KEY_MAX_BYTES_V1).unwrap()
    }
}

/// Explicit host-only key custody: exactly the producer's retained existing key.
/// New generation/deletion is a test failure, never a fallback to another incarnation.
#[derive(Clone)]
struct HostPlatform {
    slot: KagemushaWalletSlotIdV1,
    key: Arc<SigningKey>,
    signs: Arc<AtomicUsize>,
}
impl HostPlatform {
    fn existing(device: &Path, credential: &KagemushaWalletCredentialV1) -> Self {
        require_path(device);
        let bytes = iroha_fs::read_private(device.join("simulator-key.norito"), 1024).unwrap();
        let keys: Vec<([u8; 32], [u8; 32])> = decode(&bytes);
        assert_eq!(keys.len(), 1);
        let (slot, key) = keys[0];
        let key = SigningKey::from_slice(&key).unwrap();
        assert_eq!(Self::public(&key), credential.body.payment_key);
        Self {
            slot: KagemushaWalletSlotIdV1(slot),
            key: Arc::new(key),
            signs: Arc::default(),
        }
    }
    fn public(key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
        KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap()
    }
    fn provider(
        &self,
        device: &Path,
        scheme: [u8; 32],
    ) -> KagemushaWalletProviderV1<KagemushaWalletStdFsV1, Self> {
        KagemushaWalletProviderV1::open(
            KagemushaWalletStdFsV1::open(device.join("custody")).unwrap(),
            self.clone(),
            scheme,
            // Same explicit host-fixture ballast as its genuine producer; no phone claim.
            KagemushaWalletProviderOptionsV1 {
                ballast_bytes: 2_048,
            },
        )
        .unwrap()
    }
}
impl KagemushaWalletPlatformV1 for HostPlatform {
    fn key_probe(
        &self,
        slot: &KagemushaWalletSlotIdV1,
    ) -> KagemushaWalletProbeV1<KagemushaDevicePublicKeyV1> {
        if *slot == self.slot {
            KagemushaWalletProbeV1::Present(Self::public(&self.key))
        } else {
            KagemushaWalletProbeV1::Absent
        }
    }
    fn key_generate(
        &self,
        _: &KagemushaWalletSlotIdV1,
        _: &KagemushaWalletKeyGenerationRequestV1,
    ) -> KagemushaWalletKeyGenerationV1 {
        panic!("maintenance must never enroll or replace a payment key")
    }
    fn key_sign(
        &self,
        slot: &KagemushaWalletSlotIdV1,
        message: KagemushaWalletSignMessageV1<'_>,
    ) -> std::result::Result<KagemushaWalletPlatformSignatureV1, KagemushaWalletUnavailableV1> {
        assert_eq!(*slot, self.slot);
        self.signs.fetch_add(1, Ordering::SeqCst);
        let signature: DeviceSignature = self.key.sign(message.as_bytes());
        Ok(KagemushaWalletPlatformSignatureV1::Der(
            signature.to_der().as_bytes().to_vec(),
        ))
    }
    fn key_delete(&self, _: &KagemushaWalletSlotIdV1) -> KagemushaWalletRemoveOutcomeV1 {
        panic!("maintenance must never delete its payment key")
    }
    fn anchor_policy(&self) -> KagemushaWalletAnchorPolicyV1 {
        KagemushaWalletAnchorPolicyV1::NotRequired
    }
    fn storage_state(&self) -> std::result::Result<(), KagemushaWalletUnavailableV1> {
        Ok(())
    }
}

struct Handle(u64);
impl Drop for Handle {
    fn drop(&mut self) {
        // Cooperative close/join also runs on a failed assertion; no process is signalled.
        let _ = close(self.0);
    }
}
fn wait_idle(owner: &Owner) {
    let control = owner.background.inner.lock();
    let control = owner
        .background
        .inner
        .ready
        .wait_while(control, |value| {
            value.error.is_none() && (value.running || value.wake)
        })
        .unwrap();
    assert!(
        control.error.is_none(),
        "automatic worker failed: {:?}",
        control.error
    );
    assert!(!control.stopped);
    assert_eq!(control.pending_attempts, 0);
}
fn bound(id: u64, send: [u8; 32], original: &[u8]) -> Response {
    let invocation = exchange::Invocation::setup(id, 0, 42).unwrap();
    setup(
        &invocation,
        setup::Setup::BoundCredited {
            send_request: send,
            original: original.to_vec(),
        },
        &mut review::CapabilityDelivery::default(),
    )
    .unwrap()
}
fn assert_before(actual: state::Snapshot, row: &Value) {
    assert_eq!(actual.wallet_id, unhex(row["wallet_id"].as_str().unwrap()));
    assert_eq!(actual.head, unhex(row["head"].as_str().unwrap()));
    assert_eq!(
        actual.sequence,
        row["sequence"].as_str().unwrap().parse::<u128>().unwrap()
    );
    assert_eq!(
        actual.owned_balance,
        row["owned_balance"]
            .as_str()
            .unwrap()
            .parse::<u128>()
            .unwrap()
    );
    assert_eq!(
        actual.known_burned_total,
        row["known_burned_total"]
            .as_str()
            .unwrap()
            .parse::<u128>()
            .unwrap()
    );
    assert_eq!(actual.fold_backlog, 0);
    assert_eq!(actual.verified_fold.unwrap().sequence, actual.sequence);
}
fn exercise(sources: &Sources, row: &Value, account_seed: u8, expected_branch: &str) -> Value {
    assert_eq!(row["branch"].as_str(), Some(expected_branch));
    let frames = FRAME_NAMES.map(|name| original(row, name, 32768));
    let scheme = sources.installation.scheme_id;
    let credential = KagemushaWalletCredentialV1::decode_canonical(&frames[0], &scheme).unwrap();
    let account: AccountId = decode(&frames[2]);
    let account_key = KeyPair::from_seed(vec![account_seed; 32], Algorithm::Ed25519);
    assert_eq!(account, AccountId::new(account_key.public_key().clone()));
    let device = Path::new(row["custody_device"].as_str().unwrap());
    let platform = HostPlatform::existing(device, &credential);
    let payment = original(row, "payment.norito", KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1);
    let credited = original(
        row,
        "credited.norito",
        KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
    );
    let evidence: KagemushaWalletCreditedV1 = decode(&credited);
    assert!(matches!(
        (expected_branch, &evidence.evidence),
        ("receive", KagemushaWalletCreditedEvidenceV1::Receive { .. })
            | ("status", KagemushaWalletCreditedEvidenceV1::Status { .. })
    ));
    let send = unhex(row["send_request"].as_str().unwrap());
    let id = start_native_wallet(
        sources.config(),
        platform.provider(device, scheme),
        &sources.pack,
        &sources.inventory,
        sources.originals(),
    )
    .unwrap_or_else(|_| panic!("complete signed native source admission failed"));
    let handle = Handle(id);
    let challenge = open::begin(id, frames.each_ref().map(Vec::as_slice)).unwrap();
    assert_eq!(challenge.kind, 15);
    let signature = Signature::try_new(account_key.private_key(), &challenge.bytes).unwrap();
    assert_eq!(open::finish(id, signature.payload()).unwrap().kind, 16);
    let actual_owner = owner(id).unwrap();
    wait_idle(&actual_owner);
    let before = snapshot(id).unwrap();
    assert_before(before, row);
    assert_eq!(bound(id, send, &credited).kind, 11);
    // Native's own hint proves a retained receipt is waiting; no host-authored queue authority.
    let hint = with_wallet_owner(Arc::clone(&actual_owner), Priority::None, |wallet| {
        wallet.deferred_wake_hint()
    })
    .unwrap();
    assert!(matches!(
        hint,
        maintenance::WakeHint::Age(state::DeferredArchiveWakeHintV1::RecheckAfter(_))
    ));
    assert_eq!(platform.signs.load(Ordering::SeqCst), 0);

    let exchange_id = exchange::begin(id).unwrap().delivered();
    activity(id, true, true).unwrap();
    wait_idle(&actual_owner);
    assert_eq!(snapshot(id).unwrap(), before);
    assert_eq!(bound(id, send, &credited).kind, 11);
    assert_eq!(platform.signs.load(Ordering::SeqCst), 0);
    assert_eq!(retry(id, &send).unwrap().bytes, payment);
    // §5.3: charging OR count >= 16 OR age >= 24h, after no exchanges and zero backlog.
    // One fresh receipt is eligible by real scheduler charging state, never a forged age/count.
    assert!(matches!(
        exchange::end(id, exchange_id).unwrap(),
        state::ExchangeEndV1::Ended
    ));
    wait_idle(&actual_owner);
    let archived = bound(id, send, &credited);
    assert_eq!(archived.kind, 1);
    let package: KagemushaWalletPackageV1 = decode(&archived.bytes);
    package.verify(&credential).unwrap();
    assert!(matches!(
        package.statement.effect,
        KagemushaWalletEffectV1::ArchiveSent { .. }
    ));
    let installed = InstalledVerifierPackV1::load(&sources.pack, sources.installation).unwrap();
    installed
        .verifier()
        .verify_package_proofs(&package, MemoryBudget::DEFAULT)
        .unwrap();
    let folded = snapshot(id).unwrap();
    assert_eq!(folded.sequence, before.sequence.checked_add(1).unwrap());
    assert_eq!(package.statement.sequence, folded.sequence);
    assert_eq!(folded.owned_balance, before.owned_balance);
    assert_eq!(folded.known_burned_total, before.known_burned_total);
    assert_eq!(folded.fold_backlog, 0);
    assert_eq!(folded.verified_fold.unwrap().sequence, folded.sequence);
    assert_eq!(folded.verified_fold.unwrap().head, folded.head);
    let signatures = platform.signs.load(Ordering::SeqCst);
    assert_eq!(signatures, 1, "one Archive receipt; folding signs nothing");
    assert_eq!(retry(id, &send).unwrap().bytes, payment);
    assert_eq!(bound(id, send, &credited).bytes, archived.bytes);
    assert_eq!(platform.signs.load(Ordering::SeqCst), signatures);
    close(id).unwrap();
    drop(actual_owner);
    drop(handle);

    // Inspect the actual durable queue under a fresh native owner before duplicate intake.
    // Requalification repeats; no saved capability is transported across this reopen.
    let runtime = state::NativeWalletRuntimeV1::load(
        &sources.config(),
        platform.provider(device, scheme),
        &sources.pack,
        &sources.inventory,
        sources.originals(),
    )
    .unwrap();
    let pending = runtime
        .begin(&frames[0], &frames[1], &frames[2], &frames[3])
        .unwrap();
    let signature = Signature::try_new(account_key.private_key(), pending.challenge()).unwrap();
    let mut reopened = pending.finish(signature.payload()).unwrap();
    assert_eq!(reopened.snapshot().unwrap(), folded);
    assert_eq!(reopened.deferred_archive_summary().unwrap().count, 0);
    assert_eq!(
        reopened.credited_status_for_send(&send, &credited).unwrap(),
        state::RequestStatusV1::Outcome(state::Completion::Complete(archived.bytes.clone()))
    );
    reopened
        .observe_credited_for_send(&send, &credited)
        .unwrap();
    assert_eq!(reopened.deferred_archive_summary().unwrap().count, 0);
    assert_eq!(
        reopened.credited_status_for_send(&send, &credited).unwrap(),
        state::RequestStatusV1::Outcome(state::Completion::Complete(archived.bytes.clone()))
    );
    assert_eq!(reopened.snapshot().unwrap(), folded);
    assert_eq!(platform.signs.load(Ordering::SeqCst), signatures);
    norito::json!({
        "branch": expected_branch,
        "wallet_id": (hex::encode(folded.wallet_id)),
        "sequence_before": (before.sequence.to_string()),
        "sequence_after": (folded.sequence.to_string()),
        "archive_sha256": (hex::encode(BlobV1::of(&archived.bytes).sha256)),
        "payment_sha256": (hex::encode(BlobV1::of(&payment).sha256)),
        "automatic_archive_and_covering_fold": true,
        "open_exchange_prevented_archive": true,
        "same_custody_reopened_queue_empty_before_duplicate_intake": true,
        "exact_archive_and_payment_replay_without_new_signature": true
    })
}

#[test]
#[ignore = "requires a fresh real pre-Archive CoreZK handoff, independently pinned complete catalog and signed genesis; may perform many real proofs"]
fn genuine_native_worker_archives_receive_and_status_after_exchange_end_and_reopens() {
    let handoff_bytes = read_pin(
        &selected_path("KAGEMUSHA_NATIVE_MAINTENANCE_HANDOFF"),
        64 << 10,
        pin("KAGEMUSHA_NATIVE_MAINTENANCE_HANDOFF_SHA256"),
    );
    let handoff: Value = norito::json::from_slice(&handoff_bytes).unwrap();
    assert_eq!(
        handoff["schema"].as_str(),
        Some("iroha.kagemusha.native-maintenance-handoff.v1")
    );
    for (field, selected) in [
        (
            "producer_binary_sha256",
            "KAGEMUSHA_NATIVE_MAINTENANCE_PRODUCER_BINARY_SHA256",
        ),
        ("producer_source_sha256", "KAGEMUSHA_NATIVE_SOURCE_SHA256"),
        ("load_target_sha256", "KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256"),
        (
            "executed_ledger_setup_sha256",
            "KAGEMUSHA_NATIVE_LEDGER_SETUP_SHA256",
        ),
    ] {
        assert_eq!(
            handoff[field].as_str(),
            Some(hex::encode(pin(selected)).as_str())
        );
    }
    let executable = std::env::current_exe().unwrap();
    let (binary_sha256, _) =
        sha256_reader_bounded(fs::File::open(executable).unwrap(), 8 << 30).unwrap();
    assert_eq!(binary_sha256, pin("KAGEMUSHA_BRIDGE_BINARY_SHA256"));
    let output = selected_path("KAGEMUSHA_NATIVE_MAINTENANCE_OUTPUT");
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder
        .create(&output)
        .expect("fresh output; retain every failed occurrence");
    publish(
        &output.join("started.json"),
        &norito::json!({
            "handoff_sha256": (hex::encode(BlobV1::of(&handoff_bytes).sha256)),
            "consumer_binary_sha256": (hex::encode(binary_sha256)),
            "consumer_source_sha256": (hex::encode(pin("KAGEMUSHA_BRIDGE_SOURCE_SHA256"))),
            "scope": "Source identity is independently captured by the invoking runner; this test verifies its actual binary and native source admission."
        }),
    );
    let sources = Sources::selected(&handoff);
    let rows = handoff["occurrences"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0]["custody_device"], rows[1]["custody_device"]);
    assert_ne!(rows[0]["wallet_id"], rows[1]["wallet_id"]);
    let receive = exercise(&sources, &rows[0], 41, "receive");
    publish(&output.join("receive.json"), &receive);
    let status = exercise(&sources, &rows[1], 42, "status");
    publish(&output.join("status.json"), &status);
    publish(
        &output.join("complete.json"),
        &norito::json!({
            "schema": "iroha.kagemusha.native-automatic-maintenance-result.v1",
            "occurrences": [receive, status],
            "physical_device_qualified": false,
            "scope": "Actual native Rust bridge worker and real installed proofs over original host custody, simulated hardware. No C/JNI export, SDK, real-network settlement, physical-phone performance or complete cryptographic qualification claim."
        }),
    );
}
