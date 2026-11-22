//! Host run of the device flow (Mac or Linux), one child process per launch as
//! on a phone. The platform is the canonical software simulator (P-256 keys,
//! randomized signatures) in place of StrongBox or the Secure Enclave, and the
//! Send confirmation accepts
//! the exact Native review without a user. Its attestation chain is opaque
//! DATA, which the fixture issuer does not verify on any platform. Custody, the
//! C session, the engine and the installed programs are the ones phones run.
use super::*;
pub(super) mod local_blacklist;
pub(super) mod ordinary_receive;
pub(super) mod recovery;
pub(super) mod retained_refresh;
use iroha_core_zk::kagemusha_wallet_advance_v1::{
    KagemushaWalletAnchorPolicyV1, KagemushaWalletFreshGenerationV1,
    KagemushaWalletKeyGenerationPolicyV1, KagemushaWalletKeyGenerationRequestV1,
    KagemushaWalletKeyGenerationV1, KagemushaWalletKeyProfileV1, KagemushaWalletPlatformSignatureV1,
    KagemushaWalletProbeV1,
    KagemushaWalletPublishOutcomeV1, KagemushaWalletRemoveOutcomeV1, KagemushaWalletSignMessageV1,
    KagemushaWalletSlotIdV1, KagemushaWalletUnavailableV1, test_support::FakePlatformV1,
};
use iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1;

struct HostPlatform(FakePlatformV1, std::path::PathBuf);
impl HostPlatform {
    /// The simulator keeps keys in memory and derives each from a fixed seed.
    /// A later launch replays every recorded generation request (slot,
    /// challenge digest, profile) into a new simulator, which returns the same
    /// key, as a phone's key store survives a process restart.
    fn open(root: &Path) -> Self {
        let platform = FakePlatformV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, 0x5a);
        let record = root.join("host-software-keys.bin");
        for entry in std::fs::read(&record).unwrap_or_default().chunks(65) {
            let slot = KagemushaWalletSlotIdV1(entry[..32].try_into().unwrap());
            let request = KagemushaWalletKeyGenerationRequestV1 {
                challenge_digest: entry[32..64].try_into().unwrap(),
                profile: KagemushaWalletKeyProfileV1::from_tag(entry[64]).unwrap(),
            };
            assert!(matches!(
                platform.key_generate(&slot, &request),
                KagemushaWalletKeyGenerationV1::Generated(_)
            ));
        }
        Self(platform, record)
    }
}
impl KagemushaWalletPlatformV1 for HostPlatform {
    fn key_enumerate(&self) -> std::result::Result<Vec<KagemushaWalletSlotIdV1>, KagemushaWalletUnavailableV1> {
        self.0.key_enumerate()
    }
    fn key_probe(&self, s: &KagemushaWalletSlotIdV1) -> KagemushaWalletProbeV1<KagemushaDevicePublicKeyV1> {
        self.0.key_probe(s)
    }
    /// Labelled software DATA in the Android evidence shape (2..=8 non-empty
    /// originals). The fixture issuer signs E6 without verifying it.
    fn key_attestation_chain(&self, s: &KagemushaWalletSlotIdV1) -> KagemushaWalletProbeV1<Vec<Vec<u8>>> {
        match self.0.key_probe(s) {
            KagemushaWalletProbeV1::Present(_) => KagemushaWalletProbeV1::Present(vec![
                b"host software payment key: no attestation".to_vec(),
                b"host software issuer: no attestation".to_vec(),
            ]),
            KagemushaWalletProbeV1::Absent => KagemushaWalletProbeV1::Absent,
            KagemushaWalletProbeV1::Unavailable(reason) => {
                KagemushaWalletProbeV1::Unavailable(reason)
            }
        }
    }
    fn key_generate(
        &self,
        s: &KagemushaWalletSlotIdV1,
        r: &KagemushaWalletKeyGenerationRequestV1,
    ) -> KagemushaWalletKeyGenerationV1 {
        let generated = self.0.key_generate(s, r);
        if matches!(generated, KagemushaWalletKeyGenerationV1::Generated(_)) {
            use std::io::Write;
            let mut entry = s.0.to_vec();
            entry.extend_from_slice(&r.challenge_digest);
            entry.push(r.profile.tag());
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.1)
                .unwrap();
            file.write_all(&entry).unwrap();
            file.sync_all().unwrap();
        }
        generated
    }
    fn key_recover_generation_reply(
        &self,
        s: &KagemushaWalletSlotIdV1,
        r: &KagemushaWalletKeyGenerationRequestV1,
    ) -> std::result::Result<Option<KagemushaDevicePublicKeyV1>, KagemushaWalletUnavailableV1> {
        self.0.key_recover_generation_reply(s, r)
    }
    fn key_generation_policy(
        &self,
    ) -> std::result::Result<KagemushaWalletKeyGenerationPolicyV1, KagemushaWalletUnavailableV1> {
        self.0.key_generation_policy()
    }
    fn key_generate_fresh(&self, g: KagemushaWalletFreshGenerationV1<'_>) -> KagemushaWalletKeyGenerationV1 {
        self.0.key_generate_fresh(g)
    }
    fn key_sign(
        &self,
        s: &KagemushaWalletSlotIdV1,
        m: KagemushaWalletSignMessageV1<'_>,
    ) -> std::result::Result<KagemushaWalletPlatformSignatureV1, KagemushaWalletUnavailableV1> {
        self.0.key_sign(s, m)
    }
    fn key_delete(&self, s: &KagemushaWalletSlotIdV1) -> KagemushaWalletRemoveOutcomeV1 {
        self.0.key_delete(s)
    }
    fn anchor_policy(&self) -> KagemushaWalletAnchorPolicyV1 {
        self.0.anchor_policy()
    }
    fn anchor_create(&self, s: &KagemushaWalletSlotIdV1, v: &[u8]) -> KagemushaWalletPublishOutcomeV1 {
        self.0.anchor_create(s, v)
    }
    fn anchor_read(&self, s: &KagemushaWalletSlotIdV1) -> KagemushaWalletProbeV1<Vec<u8>> {
        self.0.anchor_read(s)
    }
    fn anchor_update(&self, s: &KagemushaWalletSlotIdV1, v: &[u8]) -> KagemushaWalletPublishOutcomeV1 {
        self.0.anchor_update(s, v)
    }
    fn storage_state(&self) -> std::result::Result<(), KagemushaWalletUnavailableV1> {
        self.0.storage_state()
    }
    fn boot_id(&self) -> std::result::Result<[u8; 32], KagemushaWalletUnavailableV1> {
        self.0.boot_id()
    }
    fn monotonic_ms(&self) -> std::result::Result<u64, KagemushaWalletUnavailableV1> {
        self.0.monotonic_ms()
    }
    /// Native admission needs the platform's answer. Linux reads its kernel
    /// estimate; the Mac host answers like an Apple adapter would, with
    /// free, speculative, purgeable and file-backed pageable pages.
    fn available_memory(&self) -> Option<u64> {
        #[cfg(target_vendor = "apple")]
        {
            mac_available_memory()
        }
        #[cfg(not(target_vendor = "apple"))]
        {
            self.0.available_memory()
        }
    }
}

#[cfg(target_vendor = "apple")]
fn mac_available_memory() -> Option<u64> {
    use std::ffi::{CStr, c_char, c_void};
    unsafe extern "C" {
        fn sysctlbyname(
            name: *const c_char,
            old: *mut c_void,
            old_len: *mut usize,
            new: *mut c_void,
            new_len: usize,
        ) -> i32;
    }
    let read = |name: &CStr| -> Option<u64> {
        let mut value = [0u8; 8];
        let mut len = value.len();
        // SAFETY: an 8-byte writable buffer with its length; no new value.
        let status = unsafe {
            sysctlbyname(name.as_ptr(), value.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0)
        };
        match (status, len) {
            (0, 4) => Some(u64::from(u32::from_ne_bytes(value[..4].try_into().ok()?))),
            (0, 8) => Some(u64::from_ne_bytes(value)),
            _ => None,
        }
    };
    let pages = read(c"vm.page_free_count")?
        + read(c"vm.page_speculative_count")?
        + read(c"vm.page_purgeable_count")?
        + read(c"vm.page_pageable_external_count")?;
    pages.checked_mul(read(c"hw.pagesize")?)
}

/// No user on the host: accepts exactly one non-empty Native review per Send.
struct HostConfirmation(std::sync::Mutex<Vec<Vec<u8>>>);

/// Recheck an already-completed two-wallet run using a new wire encoder.
/// The current released Receive and its immutable hosted Request must already exist.
pub(super) fn verify_retained_credit(
    root: &Path,
    runtime: &mut SessionAdapter,
    slot: &[u8; 32],
    parameters: &native::AuthenticatedParameters,
) -> Result<String> {
    if let Some(wire) = std::env::var_os("KAGEMUSHA_SPARTAN_HOST_LATER_STATUS") {
        return verify_later_credit(root, runtime, slot, parameters, Path::new(&wire));
    }
    let head = runtime.output(slot, &[0; 32], 3)?;
    if head.status != 101 || head.bytes.len() != 196 || head.bytes[2] != 3 {
        return Err("retained credit needs the current Released head".into());
    }
    let operation = head.bytes[68..100].try_into().expect("released operation");
    let status = runtime.credit_status(*slot, [0x39; 32], operation)?;
    let request = runtime.open_request(slot, &[0x39; 32], 10)?;
    let engine = native::NativeEngine::default();
    let exact = std::fs::read(root.join("incoming-payment.bin")).map_err(fail)?;
    if runtime.credit_status_for_payment(*slot, [0x39; 32], operation, &exact)? != status {
        return Err("carrier Payment acknowledgement differs from the retained Receive".into());
    }
    let other = std::fs::read(root.join("complete-payment.bin")).map_err(fail)?;
    if exact == other || runtime.credit_status_for_payment(*slot, [0x39; 32], operation, &other).is_ok() {
        return Err("a different genuine Payment obtained this Receive's acknowledgement".into());
    }
    let payment = engine
        .verify_payment(&exact, parameters)
        .map_err(fail)?;
    if engine.verify_credit_status_peer(&status, &request, &payment, parameters).map_err(fail)?
        != native::VerifiedCreditStatus::Credited
    {
        return Err("retained genuine Receive did not prove a credit".into());
    }
    let mut changed = status.clone();
    *changed.last_mut().ok_or("empty credit status")? ^= 1;
    if engine.verify_credit_status_peer(&changed, &request, &payment, parameters).is_ok() {
        return Err("changed retained credit receipt was accepted".into());
    }
    save_credit_measurement(root, "incoming", &status)?;
    if root.join("counterparty-receive-capsule.norito").exists() {
        verify_retained_counterparty_credit(root, runtime, slot, parameters)?;
    }
    runtime.close()?;
    Ok(format!("Verified retained credited receipt: {} full IPM1 bytes, {} payload bytes", status.len(), status.len()-84))
}

/// Run on a private copy of the software wallet. A first checks B's retained later-head
/// status. A then releases its own independent Send after Receive and issues evidence for
/// that Receive from its new head using the normal C entry and its protected map index.
fn verify_later_credit(
    root: &Path,
    runtime: &mut SessionAdapter,
    slot: &[u8; 32],
    parameters: &native::AuthenticatedParameters,
    remote_path: &Path,
) -> Result<String> {
    let read = |name: &str| std::fs::read(root.join(name)).map_err(fail);
    let engine = native::NativeEngine::default();
    let original = read("complete-payment.bin")?;
    let outgoing = engine.verify_payment(&original, parameters).map_err(fail)?;
    let send = outgoing.receipt().body[114..146]
        .try_into()
        .expect("Send operation");
    let remote = std::fs::read(remote_path).map_err(fail)?;
    let request = runtime.open_request(slot, &[0x39; 32], 10)?;
    if !runtime.verify_credit_status(*slot, send, &[], &remote)?
        || runtime
            .verify_credit_status(*slot, send, &request, &remote)
            .is_ok()
    {
        return Err(
            "payer C status verification did not bind the original Send and Request".into(),
        );
    }
    let mut damaged = remote.clone();
    *damaged.last_mut().ok_or("empty later-head status")? ^= 1;
    if runtime
        .verify_credit_status(*slot, send, &[], &damaged)
        .is_ok()
    {
        return Err("payer C verification accepted a changed later-head proof".into());
    }
    eprintln!(
        "Payer C entry verified {} bytes from the counterparty's later Send",
        remote.len()
    );

    let before = runtime.output(slot, &[0; 32], 3)?;
    if before.status != 101 || before.bytes.len() != 196 || before.bytes[2] != 3 {
        return Err("later-head fixture requires a released wallet".into());
    }
    let previous = before.bytes[68..100]
        .try_into()
        .expect("previous operation");
    let exact = read("incoming-payment.bin")?;
    let incoming = engine.verify_payment(&exact, parameters).map_err(fail)?;
    let earlier = runtime.credit_status(*slot, [0x39; 32], previous)?;
    if engine
        .verify_credit_status_peer(&earlier, &request, &incoming, parameters)
        .map_err(fail)?
        != native::VerifiedCreditStatus::Credited
    {
        return Err("previous released Receive did not prove this original credit".into());
    }
    let action = read("competing-send-request.bin")?;
    let review = runtime.prepare(slot, &action, &[])?;
    if review.review.len() != 291 || review.review[2] != 3 {
        return Err("unexpected later Send review".into());
    }
    let later = review.review[67..99].try_into().expect("review operation");
    if before.bytes[68..100] == later || send == later {
        return Err("later-head fixture requires an independent fresh Send after Receive".into());
    }
    HostConfirmation(Default::default()).confirm(&review.review)?;
    let completed = runtime.execute(review.token)?;
    if completed.status != 1 {
        return Err(format!(
            "later Send did not release, status {} detail {}",
            completed.status, completed.detail
        ));
    }
    runtime.discard(review.token)?;
    let head = runtime.output(slot, &[0; 32], 3)?;
    if head.status != 101 || head.bytes.len() != 196 || head.bytes[68..100] != later {
        return Err("later Send is not the released head".into());
    }
    let wire = runtime.credit_status_for_payment(*slot, [0x39; 32], later, &exact)?;
    let checked = engine
        .verify_credit_status_peer(&wire, &request, &incoming, parameters)
        .map_err(fail)?;
    if checked != native::VerifiedCreditStatus::Credited
        || wire == remote
        || wire.len() > 10_084
        || runtime.credit_status(*slot, [0x39; 32], later).is_ok()
        || runtime
            .credit_status_for_payment(*slot, [0x39; 32], later, &original)
            .is_ok()
        || runtime.output(slot, &[0; 32], 3)?.bytes != head.bytes
    {
        return Err(
            "later Send C status did not bind its receiver head and exact original Payment".into(),
        );
    }
    save_credit_measurement(root, "local-later-send", &wire)?;
    runtime.close()?;
    Ok(format!(
        "Verified both C delivery paths, {} bytes from remote later Send and {} bytes from local later Send",
        remote.len(),
        wire.len()
    ))
}

fn save_credit_measurement(root: &Path, direction: &str, bytes: &[u8]) -> Result<()> {
    use sha2::{Digest, Sha256};
    let digest = format!("{:x}", Sha256::digest(bytes));
    let file = root.join(format!("verified-{direction}-credit-status-{digest}.ipm1"));
    save_fixture_input(&file, bytes)
}

/// Re-encode B's previously released receipt using only A's actual outgoing
/// Payment and B's exact saved capsule and completion. No operation is signed.
fn verify_retained_counterparty_credit(
    root: &Path,
    runtime: &mut SessionAdapter,
    slot: &[u8; 32],
    parameters: &native::AuthenticatedParameters,
) -> Result<()> {
    let read = |name: &str| std::fs::read(root.join(name)).map_err(fail);
    let engine = native::NativeEngine::default();
    let payment = engine.verify_payment(&read("complete-payment.bin")?, parameters).map_err(fail)?;
    // The two original quotes differ in nonce and signature, and carry the
    // same receiver credential, issuer certificate and exact account companion.
    let alternate = read("competing-send-request.bin")?;
    let framed = alternate.strip_prefix(b"KSO1\x03").ok_or("competing Send frame")?;
    let (len, quote) = framed.split_at_checked(4).ok_or("competing Send length")?;
    if u32::from_le_bytes(len.try_into().unwrap()) as usize != quote.len() {
        return Err("competing Send original length changed".into());
    }
    let companion = native::decode_open_request_peer(quote).map_err(fail)?;
    let issued = payment.request();
    let mut body = issued.body;
    body[66..130].fill(0);
    body[194..210].fill(0);
    let original = [body.as_slice(), &issued.signature].concat();
    let request = native::encode_open_request_peer(
        &original,
        companion.receiver_credential,
        companion.certificates,
        companion.fee_schedule,
        companion.destination_account,
    ).map_err(fail)?;
    let capsule = native::RecoveryCapsule::decode(
        &read("counterparty-receive-capsule.norito")?,
        &native::device_fixture::scheme().scheme_id(),
    ).map_err(fail)?;
    let completion = native::CompletionRecord::decode(
        &read("counterparty-receive-completion.norito")?, capsule.wallet(),
    ).map_err(fail)?;
    let wire = engine.encode_credit_status_peer(&capsule, &completion, &request, &payment, parameters)
        .map_err(fail)?;
    let send = payment.receipt().body[114..146].try_into().expect("receipt operation");
    // C compares the supplied original with the exact selected payer-side Request.
    if !runtime.verify_credit_status(*slot, send, &request, &wire)?
        || !runtime.verify_credit_status(*slot, send, &[], &wire)?
    {
        return Err("retained counterparty receipt did not prove a credit".into());
    }
    let mut changed = wire.clone();
    *changed.last_mut().ok_or("empty counterparty status")? ^= 1;
    if runtime.verify_credit_status(*slot, send, &request, &changed).is_ok() {
        return Err("payer accepted altered counterparty receipt".into());
    }
    save_credit_measurement(root, "outgoing", &wire)?;
    eprintln!("Verified counterparty receipt: {} full IPM1 bytes", wire.len());
    Ok(())
}
impl Confirmation for HostConfirmation {
    fn confirm(&self, review: &[u8]) -> Result<()> {
        if review.is_empty() {
            return Err("empty Native review".into());
        }
        self.0.lock().map_err(fail)?.push(review.to_vec());
        Ok(())
    }
}

fn measurements(root: &Path) -> norito::json::Value {
    let text = std::fs::read_to_string(root.join("device-measurements.json")).unwrap();
    norito::json::from_str(&text).unwrap()
}
fn text(value: &norito::json::Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_owned)
}

/// The complete two-wallet device flow in the launches a phone makes, each in
/// its own child process: the first proves A's Bootstrap, Load and Send and
/// B's three operations and retains B's Payment; the second receives it; a
/// third launch on the same custody must return every retained output without
/// proving or confirming again. `KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS` names the
/// authenticated bundle, `KAGEMUSHA_SPARTAN_DEVICE_ROOT` a new empty directory.
#[test]
#[ignore = "proves seven operations (several GB, minutes); needs the authenticated native-device bundle"]
fn two_wallet_device_flow_on_host() {
    const LAUNCH: &str = "KAGEMUSHA_SPARTAN_HOST_LAUNCH";
    let var = |name: &str| {
        std::path::PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required")))
    };
    let artifacts = var("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS");
    let root = var("KAGEMUSHA_SPARTAN_DEVICE_ROOT");
    if let Some(launch) = std::env::var_os(LAUNCH) {
        let launch = launch.to_string_lossy().into_owned();
        let confirmation = HostConfirmation(Default::default());
        let summary = exercise(
            &root,
            &artifacts,
            Arc::new(HostPlatform::open(&root)),
            None,
            Some(&confirmation),
        )
        .unwrap_or_else(|error| panic!("launch {launch} failed: {error}"));
        eprintln!("LAUNCH {launch}\n{summary}");
        std::fs::write(root.join(format!("summary-launch-{launch}.txt")), &summary).unwrap();
        return;
    }
    assert!(
        root.is_absolute() && std::fs::read_dir(&root).expect("device root exists").next().is_none(),
        "device root must be an existing empty absolute directory"
    );
    let mut payments = None;
    for launch in 1..=3 {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "host_tests::two_wallet_device_flow_on_host",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(LAUNCH, launch.to_string())
            .status()
            .unwrap();
        assert!(status.success(), "launch {launch}: {status}");
        std::fs::copy(
            root.join("device-measurements.json"),
            root.join(format!("device-measurements-launch-{launch}.json")),
        )
        .unwrap();
        let summary = std::fs::read_to_string(root.join(format!("summary-launch-{launch}.txt"))).unwrap();
        let observed = measurements(&root);
        assert_eq!(observed.get("succeeded").and_then(|v| v.as_bool()), Some(true));
        let sent = (
            text(&observed, "outgoing_payment_sha256"),
            text(&observed, "incoming_payment_sha256"),
        );
        assert!(sent.0.is_some() && sent.1.is_some() && sent.0 != sent.1);
        assert_eq!(*payments.get_or_insert_with(|| sent.clone()), sent, "retained Payments");
        let retained: Vec<_> = observed
            .get("retained_operations")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        let events = observed.get("events").and_then(|v| v.as_array()).unwrap();
        let phase = |name: &str| -> Vec<&str> {
            events
                .iter()
                .filter(|event| event.get("phase").and_then(|v| v.as_str()) == Some(name))
                .filter_map(|event| event.get("operation").and_then(|v| v.as_str()))
                .collect()
        };
        let (executed, confirmed) = (phase("execute"), phase("confirmation_wait"));
        match launch {
            1 => {
                assert!(summary.starts_with("stage 1 of 2:"), "{summary}");
                assert!(retained.is_empty());
                assert_eq!(executed, ["Bootstrap", "Load", "Send"]);
                assert_eq!(confirmed, ["Send"]);
            }
            2 => {
                assert!(!summary.starts_with("stage 1 of 2:"), "{summary}");
                assert_eq!(retained, ["Bootstrap", "Load", "Send"]);
                assert_eq!(executed, ["Receive"]);
                assert!(confirmed.is_empty());
            }
            _ => {
                assert_eq!(retained, ["Bootstrap", "Load", "Send", "Receive"]);
                assert!(executed.is_empty() && confirmed.is_empty(), "a retained launch executes nothing");
            }
        }
    }
}

/// Re-issue a device bundle whose manifest is in the retired v1 format for
/// the current manifest format (`device_fixture::reissue_bundle`): the
/// programs stay byte-identical. `KAGEMUSHA_SPARTAN_RETIRED_ARTIFACTS` names
/// the source bundle, `KAGEMUSHA_SPARTAN_REISSUED_ARTIFACTS` a new empty
/// directory for the result.
#[test]
#[ignore = "reads and re-installs a 69 MB authenticated bundle"]
fn reissue_device_bundle() {
    let var = |name: &str| {
        std::path::PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required")))
    };
    let source = var("KAGEMUSHA_SPARTAN_RETIRED_ARTIFACTS");
    let destination = var("KAGEMUSHA_SPARTAN_REISSUED_ARTIFACTS");
    native::device_fixture::reissue_bundle(&source, &destination).unwrap();
    let bundle = |dir: &Path| std::fs::read(dir.join("program-bundle.norito")).unwrap();
    assert_eq!(bundle(&source), bundle(&destination), "programs unchanged");
}
