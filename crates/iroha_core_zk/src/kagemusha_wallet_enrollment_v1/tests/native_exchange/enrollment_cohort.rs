//! Isolated engineering-issuer enrollment cohort with real native proofs and custody.
//! Hardware and issuer evidence are explicit software fixtures. Ledger execution and
//! activation finality must come from the separate Core StateExecutor producer.

use super::*;
use iroha_data_model::{
    sumeragi_finality::SumeragiFinalityProof,
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder},
};
use iroha_version::codec::DecodeVersioned as _;
use std::{
    collections::BTreeSet,
    sync::{
        Barrier, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

const COUNT: usize = 100;
const WORKERS: usize = 8;
const STAGE_DEADLINE: Duration = Duration::from_secs(90);
const MANIFEST_MAX: usize = 1024 * 1024;
const ORIGINAL_MAX: usize = 32 * 1024 * 1024;
const SETUP_SCHEMA: &str = "iroha.kagemusha.enrollment-cohort-setup.v1";
const TARGET_SCHEMA: &str = "iroha.kagemusha.enrollment-cohort-target.v1";
const FINALITY_SCHEMA: &str = "iroha.kagemusha.enrollment-cohort-finality.v1";
const TARGET_ROLES: [&str; 7] = [
    "credential.norito",
    "certificates.norito",
    "account.norito",
    "asset.norito",
    "bootstrap.norito",
    "activation.norito",
    "activate-signed.wire",
];
type Json = norito::json::Value;

// All logical requests become eligible at one common start. The physical worker
// bound limits resident proof work; waiting for a worker counts against the deadline.
fn concurrent_cohort<T: Send>(work: impl Fn(usize, Instant) -> T + Sync) -> (Duration, Vec<T>) {
    let barrier = Barrier::new(WORKERS + 1);
    let start = OnceLock::new();
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..WORKERS)
            .map(|_| {
                let work = &work;
                let barrier = &barrier;
                let start = &start;
                let next = &next;
                scope.spawn(move || {
                    barrier.wait();
                    let began = *start.get().unwrap();
                    let mut results = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= COUNT {
                            break;
                        }
                        results.push((index, work(index, began)));
                    }
                    results
                })
            })
            .collect();
        start.set(Instant::now()).unwrap();
        barrier.wait();
        let mut results: Vec<_> = handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect();
        results.sort_by_key(|(index, _)| *index);
        assert_eq!(results.len(), COUNT);
        (
            start.get().unwrap().elapsed(),
            results.into_iter().map(|(_, result)| result).collect(),
        )
    })
}

fn stage_timing(output: &Path, phase: &str, elapsed: Duration, owners: &[Json]) {
    let value = norito::json!({"phase":phase,"logical_requests":COUNT,"physical_workers":WORKERS,
        "deadline_ms":(STAGE_DEADLINE.as_millis().to_string()),"elapsed_ms":(elapsed.as_millis().to_string()),
        "within_deadline":(elapsed <= STAGE_DEADLINE),"owners":(Json::Array(owners.to_vec())),
        "scope":"One common start including worker queue and native work; shared immutable admitted sources. Isolated engineering issuer and simulated hardware, not HTTP or device timing."});
    publish(
        &output.join("timing.json"),
        &norito::json::to_vec(&value).unwrap(),
    );
    assert!(
        elapsed <= STAGE_DEADLINE,
        "{phase}: 100 logical requests exceeded the common 90s stage deadline: {elapsed:?}"
    );
}

fn selected_manifest(name: &str) -> (PathBuf, Vec<u8>, Json) {
    let path = selected_path(name);
    let raw = read_pin(&path, MANIFEST_MAX, &env_pin(&format!("{name}_SHA256")));
    let value = norito::json::from_slice(&raw).unwrap();
    (path, raw, value)
}

fn original(root: &Path, row: &Json, name: &str, maximum: usize) -> Vec<u8> {
    assert_eq!(row["name"].as_str(), Some(name));
    let bytes = read_pin(&root.join(name), maximum, row["sha256"].as_str().unwrap());
    assert_eq!(row["bytes"].as_u64(), Some(bytes.len() as u64));
    bytes
}

fn record(root: &Path, name: &str, bytes: &[u8]) -> Json {
    publish(&root.join(name), bytes);
    norito::json!({"name": name, "bytes": (bytes.len()), "sha256": (sha(bytes))})
}

fn owner_rows(value: &Json) -> &[Json] {
    assert_eq!(value["owner_count"].as_u64(), Some(COUNT as u64));
    let rows = value["owners"].as_array().unwrap();
    assert_eq!(rows.len(), COUNT);
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row["index"].as_u64(), Some(index as u64));
        assert_eq!(row["seed"].as_u64(), Some(100 + index as u64));
    }
    rows
}

struct Cohort {
    sha256: String,
    accounts: Vec<AccountId>,
    activation_time_ms: u64,
}
impl Cohort {
    fn read(sources: &Sources, base: &LedgerSetup) -> Self {
        let (path, raw, value) = selected_manifest("KAGEMUSHA_NATIVE_ENROLLMENT_COHORT_SETUP");
        assert_eq!(value["schema"].as_str(), Some(SETUP_SCHEMA));
        assert_eq!(
            value["base_setup_sha256"].as_str(),
            Some(base.manifest_sha256.as_str())
        );
        assert_eq!(
            value["network_hex"].as_str(),
            Some(hex::encode(sources.genesis.initial_epoch().network_id.as_bytes()).as_str())
        );
        assert_eq!(
            value["instance_hex"].as_str(),
            Some(hex::encode(sources.genesis.instance().0).as_str())
        );
        let height = value["registration_height"].as_u64().unwrap();
        assert_eq!(height, 3);
        let history = value["history"].as_array().unwrap();
        assert_eq!(history.len(), height as usize);
        let mut verifier = sources.genesis.as_ref().clone();
        for (index, row) in history.iter().enumerate() {
            let height = index as u64 + 1;
            assert_eq!(row["height"].as_u64(), Some(height));
            let bytes = original(
                path.parent().unwrap(),
                &row["original"],
                &format!("proof-{height:04}.norito"),
                ORIGINAL_MAX,
            );
            let proof: SumeragiFinalityProof = decode(&bytes);
            assert_eq!(proof.height(), height);
            verifier.verify(&proof).unwrap();
        }
        // Finality authenticates the executed source, while the independently pinned
        // Core producer owns the asserted account-registration membership checks.
        let accounts = owner_rows(&value)
            .iter()
            .enumerate()
            .map(|(index, row)| {
                assert_eq!(row["seed"].as_u64(), Some(100 + index as u64));
                let bytes = original(
                    path.parent().unwrap(),
                    &row["account"],
                    &format!("account-{index:03}.norito"),
                    4096,
                );
                let account: AccountId = decode(&bytes);
                let key = KeyPair::from_seed(vec![(100 + index) as u8; 32], Algorithm::Ed25519);
                assert_eq!(account, AccountId::new(key.public_key().clone()));
                account
            })
            .collect::<Vec<_>>();
        assert_eq!(accounts.iter().collect::<BTreeSet<_>>().len(), COUNT);
        let activation_time_ms = value["activation_time_ms"].as_u64().unwrap();
        assert_eq!(activation_time_ms, 4_999);
        Self {
            sha256: sha(&raw),
            accounts,
            activation_time_ms,
        }
    }

    fn fixture(&self, sources: &Sources, base: &LedgerSetup, index: usize) -> Fixture {
        let mut fixture = sources.fixture(base, 0);
        setup::bind_identity(
            &mut fixture,
            self.accounts[index].clone(),
            base.asset.clone(),
            (100 + index) as u8,
        );
        fixture
    }
}

fn enroll(device: &HostDevice, fixture: &Fixture) -> ([Vec<u8>; 4], Vec<Json>) {
    let mut owner: HostEnrollment = EnrollmentOwnerV1::new(
        device.provider(fixture.config.scheme.scheme_id()),
        fixture.config.clone(),
    )
    .unwrap_or_else(|(_, error)| panic!("{error}"));
    let dispatch = owner
        .begin(
            &[11; 32],
            &norito::encode_canonical(&fixture.account).unwrap(),
            &norito::encode_canonical(&fixture.asset).unwrap(),
        )
        .unwrap();
    let permit = permit(fixture, &PreKeyDispatchV1::decode(&dispatch).unwrap())
        .encode_canonical()
        .unwrap();
    let challenge = owner.accept_permit(&permit).unwrap();
    assert!(matches!(
        owner.authorize(&sign(fixture, &challenge)).unwrap(),
        EnrollmentProgressV1::Evidence { .. }
    ));
    let RequestPreparationV1::AccountChallenge(challenge) =
        owner.prepare_request(&fixture.evidence).unwrap()
    else {
        panic!("fresh E5 challenge")
    };
    let request = owner.retain_request(&sign(fixture, &challenge)).unwrap();
    let response = issuer_result(fixture, &RequestV1::decode(&request).unwrap())
        .encode()
        .unwrap();
    owner.accept_credential(&response).unwrap();
    let frames = owner.open_originals().unwrap();
    let records = [
        ("e1-dispatch.norito", dispatch),
        ("e1-permit.norito", permit),
        ("e5-request.norito", request),
        ("e6-result.norito", response),
    ]
    .into_iter()
    .map(|(name, bytes)| record(&device.root, name, &bytes))
    .collect();
    (frames, records)
}

fn signed_activation(
    sources: &Sources,
    fixture: &Fixture,
    activation: &[u8],
    time_ms: u64,
) -> SignedTransaction {
    let mut transaction = TransactionBuilder::new(
        sources.genesis.initial_epoch().network_id,
        fixture.account.clone(),
        FeePaymentIntent::authority(vec![], None),
    );
    transaction.set_creation_time(Duration::from_millis(time_ms));
    transaction.set_ttl(Duration::from_millis(100_000));
    transaction
        .with_instructions([KagemushaWalletLedgerV1::new(
            fixture.config.scheme.scheme_id(),
            KagemushaWalletLedgerActionV1::Activate(activation.to_vec()),
        )])
        .sign(fixture.account_key.private_key())
}

#[test]
#[ignore = "requires current signed proving originals and executed 100-account cohort; engineering issuer and simulated hardware only"]
fn export_hundred_native_enrollment_activation_targets() {
    let output = fresh_output("KAGEMUSHA_NATIVE_ENROLLMENT_COHORT_OUTPUT");
    let sources = Sources::admit(&output.join("source-admission"));
    let base = LedgerSetup::read(&sources);
    let cohort = Cohort::read(&sources, &base);
    let (elapsed, owners) = concurrent_cohort(|index, common_start| {
        let began = Instant::now();
        let queue_ms = began.duration_since(common_start).as_millis();
        let fixture = cohort.fixture(&sources, &base, index);
        let device =
            HostDevice::create(&output.join(format!("owner-{index:03}")), (index + 1) as u8);
        let (frames, enrollment_originals) = enroll(&device, &fixture);
        let enrolled_ms = began.elapsed().as_millis();
        let mut wallet = sources.open(&device, &fixture, &frames);
        let bootstrap = complete(wallet.bootstrap().unwrap());
        assert_eq!(complete(wallet.bootstrap().unwrap()), bootstrap);
        let activation = wallet.activation().unwrap();
        assert_eq!(wallet.activation().unwrap(), activation);
        let signed = signed_activation(&sources, &fixture, &activation, cohort.activation_time_ms);
        let wire = signed.encode_wire_v1().unwrap();
        let entrypoint = *signed.hash_as_entrypoint().as_ref();
        assert_eq!(
            wallet.retain_activation_attempt(&wire).unwrap(),
            state::LedgerAttemptOutcomeV1::Awaiting { entrypoint }
        );
        let snapshot = wallet.snapshot().unwrap();
        assert_eq!((snapshot.sequence, snapshot.owned_balance), (0, 0));
        let records = FRAME_NAMES
            .into_iter()
            .zip(&frames)
            .chain([
                ("bootstrap.norito", &bootstrap),
                ("activation.norito", &activation),
                ("activate-signed.wire", &wire),
            ])
            .map(|(name, bytes)| record(&device.root, name, bytes))
            .collect::<Vec<_>>();
        drop(wallet);
        device.save_simulator();
        let mut restored = sources.open(&device, &fixture, &frames);
        assert_eq!(
            restored.activation_attempt_outcome(&wire).unwrap(),
            state::LedgerAttemptOutcomeV1::Awaiting { entrypoint }
        );
        assert_eq!(restored.activation().unwrap(), activation);
        eprintln!(
            "native enrollment cohort owner={index} complete queue_ms={queue_ms} service_ms={} cohort_elapsed_ms={}",
            began.elapsed().as_millis(),
            common_start.elapsed().as_millis()
        );
        norito::json!({"index":index, "seed":(100+index), "account_digest_hex":(hex::encode(kagemusha_wallet_account_digest_v1(&fixture.account).unwrap())), "wallet_id_hex":(hex::encode(snapshot.wallet_id)), "entrypoint_hex":(hex::encode(entrypoint)), "originals":records, "enrollment_originals":enrollment_originals, "queue_ms":(queue_ms.to_string()), "enrollment_ms":(enrolled_ms.to_string()), "service_ms":(began.elapsed().as_millis().to_string()), "cohort_elapsed_ms":(common_start.elapsed().as_millis().to_string())})
    });
    assert_eq!(
        owners
            .iter()
            .map(|row| row["wallet_id_hex"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        COUNT
    );
    stage_timing(
        &output,
        "native_enrollment_and_activation_preparation",
        elapsed,
        &owners,
    );
    let result = norito::json!({
        "schema":TARGET_SCHEMA, "owner_count":COUNT, "cohort_setup_sha256":(cohort.sha256),
        "base_setup_sha256":(base.manifest_sha256), "source_pins":(source_pins()),
        "sealed_originals":(source_reuse::locator(&sources.originals)),
        "network_hex":(hex::encode(sources.genesis.initial_epoch().network_id.as_bytes())),
        "instance_hex":(hex::encode(sources.genesis.instance().0)), "chain_id":(sources.genesis.chain_id()),
        "scheme_id":(hex::encode(sources.qualified.installation().0)), "manifest_digest":(hex::encode(sources.qualified.installation().1)),
        "native_source_sha256":(env_pin("KAGEMUSHA_NATIVE_SOURCE_SHA256")), "native_binary_sha256":(executable_hash()),
        "scope":"100 distinct executed accounts, engineering issuer and simulated hardware, real native Bootstrap/Activate and pre-submit retention; no physical attestation, live HTTP load, payment or ledger confirmation claim.",
        "owners":owners,
    });
    publish(
        &output.join("target.json"),
        &norito::json::to_vec(&result).unwrap(),
    );
}

fn target_originals(root: &Path, owner: &Json) -> Vec<Vec<u8>> {
    let rows = owner["originals"].as_array().unwrap();
    assert_eq!(rows.len(), TARGET_ROLES.len());
    TARGET_ROLES
        .iter()
        .map(|name| {
            let selected = rows
                .iter()
                .filter(|row| row["name"].as_str() == Some(*name))
                .collect::<Vec<_>>();
            assert_eq!(selected.len(), 1);
            original(root, selected[0], name, ORIGINAL_MAX)
        })
        .collect()
}

fn verified_history(sources: &Sources, root: &Path, value: &Json) -> Vec<(u64, Vec<u8>)> {
    let last = value["final_height"].as_u64().unwrap();
    assert!((6..=256).contains(&last));
    let rows = value["history"].as_array().unwrap();
    assert_eq!(rows.len(), last as usize);
    let mut verifier = sources.genesis.as_ref().clone();
    let mut certificates = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let height = index as u64 + 1;
        assert_eq!(row["height"].as_u64(), Some(height));
        let bytes = original(
            root,
            &row["original"],
            &format!("proof-{height:04}.norito"),
            ORIGINAL_MAX,
        );
        let proof: SumeragiFinalityProof = decode(&bytes);
        assert_eq!(proof.height(), height);
        let verified = verifier.verify(&proof).unwrap();
        if height > 1 {
            let certificate =
                iroha_data_model::sumeragi_finality::SumeragiCommitCertificateV1::from_verified(
                    &verified,
                )
                .unwrap();
            certificates.push((height, certificate.to_canonical_bytes().unwrap()));
        }
    }
    certificates
}

#[test]
#[ignore = "requires executed cohort activation finality and same retained native custody; never synthetic execution or 100 physical devices"]
fn confirm_hundred_native_activations_from_executed_finality() {
    let output = fresh_output("KAGEMUSHA_NATIVE_ENROLLMENT_COHORT_CONFIRMATION_OUTPUT");
    let sources = Sources::admit(&output.join("source-admission"));
    let base = LedgerSetup::read(&sources);
    let cohort = Cohort::read(&sources, &base);
    let (target_path, target_raw, target) =
        selected_manifest("KAGEMUSHA_NATIVE_ENROLLMENT_COHORT_TARGET");
    let (finality_path, _, finality) =
        selected_manifest("KAGEMUSHA_NATIVE_ENROLLMENT_COHORT_FINALITY");
    assert_eq!(target["schema"].as_str(), Some(TARGET_SCHEMA));
    assert_eq!(
        target["base_setup_sha256"].as_str(),
        Some(base.manifest_sha256.as_str())
    );
    assert_eq!(
        target["chain_id"].as_str(),
        Some(sources.genesis.chain_id())
    );
    assert_eq!(
        target["scheme_id"].as_str(),
        Some(hex::encode(sources.qualified.installation().0).as_str())
    );
    assert_eq!(
        target["manifest_digest"].as_str(),
        Some(hex::encode(sources.qualified.installation().1).as_str())
    );
    assert_eq!(target["source_pins"], source_pins());
    assert_eq!(
        target["sealed_originals"],
        source_reuse::locator(&sources.originals)
    );
    assert_eq!(
        target["cohort_setup_sha256"].as_str(),
        Some(cohort.sha256.as_str())
    );
    assert_eq!(finality["schema"].as_str(), Some(FINALITY_SCHEMA));
    assert_eq!(
        finality["cohort_setup_sha256"].as_str(),
        Some(cohort.sha256.as_str())
    );
    assert_eq!(
        finality["target_sha256"].as_str(),
        Some(sha(&target_raw).as_str())
    );
    for value in [&target, &finality] {
        assert_eq!(
            value["network_hex"].as_str(),
            Some(hex::encode(sources.genesis.initial_epoch().network_id.as_bytes()).as_str())
        );
        assert_eq!(
            value["instance_hex"].as_str(),
            Some(hex::encode(sources.genesis.instance().0).as_str())
        );
    }
    assert_eq!(finality["scheme_registration_height"].as_u64(), Some(4));
    assert_eq!(finality["installation_height"].as_u64(), Some(5));
    let certificates = verified_history(&sources, finality_path.parent().unwrap(), &finality);
    let targets = owner_rows(&target);
    let receipts = owner_rows(&finality);
    let (elapsed, confirmed) = concurrent_cohort(|index, common_start| {
        let began = Instant::now();
        let queue_ms = began.duration_since(common_start).as_millis();
        let fixture = cohort.fixture(&sources, &base, index);
        let root = target_path
            .parent()
            .unwrap()
            .join(format!("owner-{index:03}"));
        let originals = target_originals(&root, &targets[index]);
        let frames: [Vec<u8>; 4] = std::array::from_fn(|at| originals[at].clone());
        assert_eq!(decode::<AccountId>(&frames[2]), cohort.accounts[index]);
        let credential = KagemushaWalletCredentialV1::decode_canonical(
            &frames[0],
            &fixture.config.scheme.scheme_id(),
        )
        .unwrap();
        let device = HostDevice::restore(&root, &credential);
        let mut wallet = sources.open(&device, &fixture, &frames);
        assert_eq!(complete(wallet.bootstrap().unwrap()), originals[4]);
        assert_eq!(wallet.activation().unwrap(), originals[5]);
        let signed = SignedTransaction::decode_all_versioned(&originals[6]).unwrap();
        assert_eq!(signed.encode_wire_v1().unwrap(), originals[6]);
        assert_eq!(
            targets[index]["entrypoint_hex"].as_str(),
            Some(hex::encode(signed.hash_as_entrypoint().as_ref()).as_str())
        );
        let returned_wire = original(
            finality_path.parent().unwrap(),
            &receipts[index]["signed_transaction"],
            &format!("activation-{index:03}.signed.norito"),
            ORIGINAL_MAX,
        );
        assert_eq!(
            returned_wire, originals[6],
            "Core must execute the exact pre-retained wire"
        );
        let account_digest =
            hex::encode(kagemusha_wallet_account_digest_v1(&fixture.account).unwrap());
        for row in [&targets[index], &receipts[index]] {
            assert_eq!(
                row["account_digest_hex"].as_str(),
                Some(account_digest.as_str())
            );
            assert_eq!(
                row["wallet_id_hex"].as_str(),
                Some(hex::encode(credential.body.wallet_id).as_str())
            );
        }
        // This method requires the prior retained transaction; it cannot create it here.
        let prior = wallet.activation_attempt_outcome(&originals[6]).unwrap();
        assert!(matches!(
            prior,
            state::LedgerAttemptOutcomeV1::Awaiting { .. }
                | state::LedgerAttemptOutcomeV1::Confirmed(_)
        ));
        let activation_height = receipts[index]["activation_height"].as_u64().unwrap();
        assert!((6..=finality["final_height"].as_u64().unwrap()).contains(&activation_height));
        loop {
            let current = wallet.epoch_progress().unwrap();
            if activation_height <= current.boundary_height {
                break;
            }
            let (_, bytes) = certificates
                .iter()
                .find(|(height, _)| *height == current.boundary_height)
                .expect("the selected epoch's actual boundary must be in verified history");
            let next = wallet.ingest_epoch_boundary(current.epoch, bytes).unwrap();
            assert!(next.epoch > current.epoch);
        }
        let bytes = original(
            finality_path.parent().unwrap(),
            &receipts[index]["finality"],
            &format!("activation-{index:03}.finality.norito"),
            ORIGINAL_MAX,
        );
        let receipt: iroha_data_model::kagemusha::KagemushaWalletLedgerFinalityV1 = decode(&bytes);
        let (_, expected_certificate) = certificates
            .iter()
            .find(|(height, _)| *height == activation_height)
            .expect("activation height must belong to the independently verified executed history");
        assert_eq!(
            &receipt.certificate.to_canonical_bytes().unwrap(),
            expected_certificate
        );
        let calls = device.platform.with(|state| state.sign_calls);
        let state::LedgerAttemptIngestV1::Outcome(state::LedgerAttemptOutcomeV1::Confirmed(commit)) =
            wallet
                .ingest_activation_finality(&originals[6], &bytes)
                .unwrap()
        else {
            panic!("actual successful activation finality required")
        };
        assert_eq!(commit.height, activation_height);
        assert_eq!(
            wallet
                .ingest_activation_finality(&originals[6], &bytes)
                .unwrap(),
            state::LedgerAttemptIngestV1::Outcome(state::LedgerAttemptOutcomeV1::Confirmed(commit))
        );
        assert_eq!(
            wallet.activation_attempt_outcome(&originals[6]).unwrap(),
            state::LedgerAttemptOutcomeV1::Confirmed(commit)
        );
        let snapshot = wallet.snapshot().unwrap();
        assert_eq!((snapshot.sequence, snapshot.owned_balance), (0, 0));
        assert_eq!(device.platform.with(|state| state.sign_calls), calls);
        drop(wallet);
        let mut reopened = sources.open(&device, &fixture, &frames);
        assert_eq!(
            reopened.activation_attempt_outcome(&originals[6]).unwrap(),
            state::LedgerAttemptOutcomeV1::Confirmed(commit)
        );
        assert_eq!(device.platform.with(|state| state.sign_calls), calls);
        norito::json!({"index":index, "wallet_id_hex":(hex::encode(snapshot.wallet_id)), "height":(commit.height), "commit_hash_hex":(hex::encode(commit.commit_hash)), "activation_finality_sha256":(sha(&bytes)), "confirmed":true, "owned_balance":"0", "queue_ms":(queue_ms.to_string()), "service_ms":(began.elapsed().as_millis().to_string()), "cohort_elapsed_ms":(common_start.elapsed().as_millis().to_string())})
    });
    stage_timing(
        &output,
        "native_activation_finality_confirmation",
        elapsed,
        &confirmed,
    );
    publish(&output.join("confirmed.json"), &norito::json::to_vec(&norito::json!({"schema":"iroha.kagemusha.enrollment-cohort-confirmation.v1", "cohort_target_sha256":(sha(&target_raw)), "owner_count":COUNT, "confirmed":confirmed, "scope":"Fresh native confirmation of Core-executed activations; simulated hardware and engineering issuer, no wallet payments or physical-device claim."})).unwrap());
}
