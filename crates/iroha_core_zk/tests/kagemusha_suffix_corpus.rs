//! Native suffix Unload: genuine hundred-payer engineering-catalog gate and verifier tests.
//!
//! The retained run (`output/merchant-hundred-payers-20261024`) has the production ordinary
//! Receive/Unload circuits and keys; only its root-dependent Omega keys differ. Its merchant
//! wallet bootstraps (sequence 0) and receives 100 distinct genuine Payments (1..=100).
//! Everything here reads that custody without changing it:
//!
//! - `hundred_genuine_receive_predictions_equal_archived_folded_fields`: an independent
//!   native prediction of every adjusted field before any Receive Omega is read;
//! - `genuine_receive_suffix_receipt_continues_under_installed_unload_and_omega_keys`: the
//!   continuation gate. The shared suffix verifier verifies the 100 Receives from the genuine
//!   Bootstrap Omega, an Unload is derived from the verified tip, proved and signed over a
//!   distinct suffix-authority digest, and only then are the Receive folds read: their fields
//!   equal the verifier's, and the unchanged Unload Q/A/Omega fold that same receipt under the
//!   installed keys to exactly the verifier's successor fields;
//! - `native_suffix_verifier_rejects_adversarial_suffixes`: omission, reorder, duplicate
//!   credit, substituted roots, stale/forked base, wrong predecessor, nullifier replay,
//!   mutated Payment/σ/τ/proof, wrong amount and wrong receipt signer;
//! - `native_suffix_verifier_measures_fifty_distinct_receives`: retired instructions, peak
//!   memory and wall time of 50 distinct genuine Receives in ledger and Native modes.
//!
//! Missing originals fail these explicitly requested lanes; synthetic or newly generated keys
//! never stand in for them. Engineering success is not production release qualification.
#![allow(
    unsafe_code,
    reason = "macOS proc_pid_rusage instruction counter in the bench test"
)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read as _,
    path::{Path, PathBuf},
    time::Instant,
};

use iroha_pasta::msm::MemoryBudget;
use sha2::{Digest as _, Sha256};

use iroha_core_zk::{
    kagemusha_wallet_advance_v1::{
        kagemusha_wallet_archive_object_digest_v1, kagemusha_wallet_provider_digest_v1,
    },
    kagemusha_wallet_archive_bytes_v1::PrivateArchiveBytes,
    kagemusha_wallet_artifacts_v1::{
        InstallationV1, InstalledVerifierPackV1, VERIFIER_PACK_MAX_BYTES_V1,
    },
    kagemusha_wallet_state_v1::{ArchiveKey, FrozenTransition},
    kagemusha_wallet_suffix_unload_v1::*,
};
use iroha_data_model::kagemusha::*;

#[path = "kagemusha_suffix_corpus/adversarial.rs"]
mod adversarial;
#[path = "kagemusha_suffix_corpus/continuation.rs"]
mod continuation;
#[path = "kagemusha_suffix_corpus/measure.rs"]
mod measure;
#[path = "kagemusha_suffix_corpus/stages.rs"]
mod stages;

const FIXTURE: &str = "output/merchant-hundred-payers-20261024";
const SLOT: &str = "9fc4cd3811cd2cbd6fa9c6ab68351c27025f8d72ca9b05e5d80d0c2e3acb2bec";
const RECEIVE_COUNT: usize = 100;
const INSTALLATION_SHA256: &str =
    "6e2937c7bdf109a21ee697fa758955518d135bcf5f718835354c099e9cd953b9";
const VERIFIER_PACK_SHA256: &str =
    "642ad5d1af375fadaa2c9240f27696d20a45e225d4de72716cccff3dea1bc87b";

fn decode<T>(bytes: &[u8]) -> Result<T, norito::Error>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
}

// Read-only projections of provider-private fixture envelopes. Their exact schema/layout
// is checked canonically; these projections create no provider authority or persisted data.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::ArchiveEnvelope")]
struct FixtureArchiveEnvelope {
    version: u16,
    scheme_id: [u8; 32],
    wallet_id: [u8; 32],
    key: ArchiveKey,
    content_digest: [u8; 32],
    content: PrivateArchiveBytes,
}

// The archive's Fold records are the coordinator's `RecordedFold`: the genuine fold record
// and its burned bit.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::RecordedFold")]
struct FixtureRecordedFold {
    record: KagemushaWalletFoldRecordV1,
    burned: bool,
}

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_wallet_advance_v1::CredentialRecordV1")]
struct FixtureCredentialFile {
    version: u16,
    slot: [u8; 32],
    index: u32,
    credential: Vec<u8>,
}

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_wallet_advance_v1::FrozenFileV1")]
struct FixtureFrozenFile {
    version: u16,
    written_boot_id: [u8; 32],
    selected_generation: u128,
    frame: Vec<u8>,
}

fn bounded(path: &Path, maximum: usize) -> Vec<u8> {
    let metadata = fs::metadata(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    assert!(metadata.is_file(), "{}", path.display());
    assert!(metadata.len() <= maximum as u64, "{}", path.display());
    let mut bytes = Vec::new();
    fs::File::open(path)
        .unwrap()
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= maximum, "{}", path.display());
    bytes
}

fn archived<T>(slot: &Path, binding: ([u8; 32], [u8; 32]), key: ArchiveKey, maximum: usize) -> T
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let identity = kagemusha_wallet_provider_digest_v1(
        "wallet-archive-key",
        &norito::encode_canonical(&key).unwrap(),
    );
    let name = format!("w-{}.rec", hex::encode(identity));
    let primary = bounded(&slot.join("archive").join(&name), maximum + 1056);
    let replica = bounded(
        &slot.join("archive").join(format!("{name}.r")),
        maximum + 1056,
    );
    assert_eq!(
        primary, replica,
        "the fixture has matching immutable copies"
    );
    assert!(primary.len() >= 32);
    assert_eq!(
        primary[..32],
        kagemusha_wallet_archive_object_digest_v1(&primary[32..])
    );
    let envelope: FixtureArchiveEnvelope = decode(&primary[32..]).unwrap();
    assert_eq!(envelope.version, 1);
    assert_eq!((envelope.scheme_id, envelope.wallet_id), binding);
    assert_eq!(envelope.key, key);
    assert!(envelope.content.len() <= maximum);
    assert_eq!(
        envelope.content_digest,
        kagemusha_wallet_provider_digest_v1("wallet-archive-content", &envelope.content)
    );
    decode(&envelope.content).unwrap()
}

fn completion(slot: &Path, frozen: &FrozenTransition) -> KagemushaWalletCompletionRecordV1 {
    let body = frozen.capsule.body();
    let name = format!("{}.cr", hex::encode(body.operation_id));
    let maximum = KAGEMUSHA_WALLET_COMPLETION_RECORD_MAX_BYTES_V1 + 512;
    let primary = bounded(&slot.join("completion").join(&name), maximum);
    let envelope: FixtureFrozenFile = decode(&primary).unwrap();
    let replica: FixtureFrozenFile = decode(&bounded(
        &slot.join("completion").join(format!("{name}.r")),
        maximum,
    ))
    .unwrap();
    assert_eq!(envelope.version, 1);
    assert_eq!(replica.version, 1);
    assert_eq!(envelope.selected_generation, replica.selected_generation);
    // A valid re-adoption may change a copy's boot stamp; the original completion
    // frame and selected generation, including the exact hardware receipt, may not change.
    assert_eq!(envelope.frame, replica.frame);
    let record =
        KagemushaWalletCompletionRecordV1::decode_canonical(&envelope.frame, &body.wallet_id)
            .unwrap();
    record.verify(&frozen.credential, &frozen.capsule).unwrap();
    record
}

fn original_bytes(frozen: &FrozenTransition, role: KagemushaWalletRetainedInputRoleV1) -> Vec<u8> {
    let mut selected = frozen
        .capsule
        .body()
        .retained_inputs
        .iter()
        .filter(|item| item.role == role);
    let first = selected.next().unwrap();
    assert!(selected.next().is_none());
    first.bytes.clone()
}

fn original<T>(frozen: &FrozenTransition, role: KagemushaWalletRetainedInputRoleV1) -> T
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    decode(&original_bytes(frozen, role)).unwrap()
}

fn consumed_insertion(frozen: &FrozenTransition) -> KagemushaWalletIndexedInsertV1 {
    let [low_bytes, slot_bytes] = frozen.capsule.body().map_openings.as_slice() else {
        panic!("the genuine Receive has exactly the two original consumed-map openings")
    };
    let (Some(low), low_opening) =
        KagemushaWalletIndexedOpeningV1::from_transcript(low_bytes).unwrap()
    else {
        panic!("the first opening is the low leaf")
    };
    let (None, slot_opening) =
        KagemushaWalletIndexedOpeningV1::from_transcript(slot_bytes).unwrap()
    else {
        panic!("the second opening is the empty insertion slot")
    };
    KagemushaWalletIndexedInsertV1 {
        low,
        low_opening,
        slot_opening,
    }
}

/// The immutable genuine merchant custody, decoded and canonically checked.
pub(crate) struct Corpus {
    root: PathBuf,
    slot: PathBuf,
    installed: InstalledVerifierPackV1,
    credential: KagemushaWalletCredentialV1,
    frozen: BTreeMap<u128, FrozenTransition>,
    anchor: KagemushaWalletFoldRecordV1,
}

impl Corpus {
    pub(crate) fn load() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(FIXTURE);
        let catalog = root.join("catalog");
        let installation_original = bounded(&catalog.join("engineering-installation.norito"), 1024);
        assert_eq!(
            hex::encode(Sha256::digest(&installation_original)),
            INSTALLATION_SHA256
        );
        let (scheme_id, manifest_digest): ([u8; 32], [u8; 32]) =
            decode(&installation_original).unwrap();
        let verifier_original = bounded(
            &catalog.join("engineering-verifier-pack.norito"),
            VERIFIER_PACK_MAX_BYTES_V1,
        );
        assert_eq!(
            hex::encode(Sha256::digest(&verifier_original)),
            VERIFIER_PACK_SHA256
        );
        let installed = InstalledVerifierPackV1::load(
            &verifier_original,
            InstallationV1 {
                scheme_id,
                manifest_digest,
            },
        )
        .unwrap();
        let slot = root.join("run-hundred/wallet-000/custody/slots").join(SLOT);
        let credential_file: FixtureCredentialFile =
            decode(&bounded(&slot.join("credential-0.norito"), 16 << 10)).unwrap();
        assert_eq!(credential_file.version, 1);
        assert_eq!(credential_file.index, 0);
        assert_eq!(hex::encode(credential_file.slot), SLOT);
        let credential =
            KagemushaWalletCredentialV1::decode_canonical(&credential_file.credential, &scheme_id)
                .unwrap();
        let mut frozen = BTreeMap::new();
        // The bounded capsule directory supplies identities only. Canonical archive records,
        // commitments, signed completions and native proofs independently verify them.
        for entry in fs::read_dir(slot.join("capsules")).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            let Some(stem) = name.strip_prefix("c-").and_then(|v| v.strip_suffix(".cap")) else {
                continue;
            };
            let (generation, digest) = stem.split_once('-').unwrap();
            assert_eq!(generation.len(), 32);
            let digest: [u8; 32] = hex::decode(digest).unwrap().try_into().unwrap();
            let value: FrozenTransition = archived(
                &slot,
                (scheme_id, credential.body.wallet_id),
                ArchiveKey::Capsule(digest),
                KAGEMUSHA_WALLET_CAPSULE_MAX_BYTES_V1 + (16 << 10),
            );
            value.validate().unwrap();
            assert_eq!(value.capsule.custody_digest(), digest);
            assert_eq!(value.credential, credential);
            let sequence = value.capsule.body().statement.sequence;
            assert!(frozen.insert(sequence, value).is_none());
            assert!(frozen.len() <= RECEIVE_COUNT + 2);
        }
        // Bootstrap, the 100 Receives and the later onward Send.
        assert_eq!(frozen.len(), RECEIVE_COUNT + 2);
        let bootstrap = frozen.get(&0).unwrap();
        assert_eq!(
            bootstrap.capsule.body().kind,
            KagemushaWalletOperationKindV1::Bootstrap
        );
        let recorded: FixtureRecordedFold = archived(
            &slot,
            (scheme_id, credential.body.wallet_id),
            ArchiveKey::Fold(0),
            KAGEMUSHA_WALLET_FOLD_RECORD_MAX_BYTES_V1 + 64,
        );
        assert!(!recorded.burned);
        let anchor = recorded.record;
        anchor.validate().unwrap();
        assert_eq!(anchor.sequence, 0);
        assert_eq!(anchor.capsule_digest, bootstrap.capsule.custody_digest());
        for sequence in 1..=RECEIVE_COUNT as u128 {
            assert_eq!(
                frozen[&sequence].capsule.body().kind,
                KagemushaWalletOperationKindV1::Receive
            );
        }
        Self {
            root,
            slot,
            installed,
            credential,
            frozen,
            anchor,
        }
    }

    pub(crate) fn binding(&self) -> ([u8; 32], [u8; 32]) {
        (
            self.credential.body.scheme_id,
            self.credential.body.wallet_id,
        )
    }

    /// Archived genuine fold of `sequence`; the gate reads Receive folds only after signing.
    pub(crate) fn fold(&self, sequence: u128) -> KagemushaWalletFoldRecordV1 {
        let recorded: FixtureRecordedFold = archived(
            &self.slot,
            self.binding(),
            ArchiveKey::Fold(sequence),
            KAGEMUSHA_WALLET_FOLD_RECORD_MAX_BYTES_V1 + 64,
        );
        assert!(!recorded.burned, "every genuine Receive folded unburned");
        let record = recorded.record;
        record.validate().unwrap();
        assert_eq!(record.sequence, sequence);
        assert_eq!(
            record.capsule_digest,
            self.frozen[&sequence].capsule.custody_digest()
        );
        record
    }

    pub(crate) fn state(&self, sequence: u128) -> &KagemushaWalletStateV1 {
        &self.frozen[&sequence].capsule.body().successor_state
    }

    /// The receiver issuer certificate set, as quoted by the first Request.
    pub(crate) fn certificates(&self) -> KagemushaWalletCertificateSetV1 {
        let request: KagemushaWalletRequestV1 = original(
            &self.frozen[&1],
            KagemushaWalletRetainedInputRoleV1::Request,
        );
        assert_eq!(request.receiver_credential, self.credential);
        request.certificates
    }

    /// The anchor input: the genuine Bootstrap Omega and its exact state opening.
    pub(crate) fn anchor_input<'a>(
        &'a self,
        certificates: &'a KagemushaWalletCertificateSetV1,
    ) -> SuffixAnchorInputV1<'a> {
        SuffixAnchorInputV1 {
            credential: &self.credential,
            certificates,
            lineage: &self.anchor.lineage,
            state: self.state(0),
        }
    }

    pub(crate) fn steps(&self, last: u128) -> Vec<StepOriginals> {
        let mut credit_tree = KagemushaWalletIndexedTreeV1::new();
        assert_eq!(
            credit_tree.root(),
            self.anchor.lineage.public.credit_digest_root
        );
        (1..=last)
            .map(|sequence| StepOriginals::receive(self, sequence, &mut credit_tree))
            .collect()
    }
}

/// Exact owned originals of one retained Receive, as the suffix carriage discloses them.
#[derive(Clone)]
pub(crate) struct StepOriginals {
    before: KagemushaWalletStateV1,
    after: KagemushaWalletStateV1,
    statement: KagemushaWalletStatementV1,
    sigma: KagemushaWalletStepProofV1,
    receipt: KagemushaWalletReceiptV1,
    consumed: [Vec<u8>; 2],
    credit: (Vec<u8>, Option<Vec<u8>>),
    request: Vec<u8>,
    payment: Vec<u8>,
    payer_credential: Vec<u8>,
    payer_certificates: Vec<u8>,
}

impl StepOriginals {
    fn receive(
        corpus: &Corpus,
        sequence: u128,
        credit_tree: &mut KagemushaWalletIndexedTreeV1,
    ) -> Self {
        let step = &corpus.frozen[&sequence];
        let body = step.capsule.body();
        let payment: KagemushaWalletPaymentV1 =
            original(step, KagemushaWalletRetainedInputRoleV1::Payment);
        let digests = payment.digests().unwrap();
        let record = KagemushaWalletCreditDigestLeafV1 {
            credit_id: digests.credit_id,
            payment_digest: digests.payment,
            burned: false,
        }
        .record(credit_tree)
        .unwrap();
        let credit = match record {
            KagemushaWalletCreditDigestRecordV1::Inserted { witness } => (
                witness.low_opening.leaf_transcript(&witness.low),
                Some(witness.slot_opening.empty_transcript()),
            ),
            KagemushaWalletCreditDigestRecordV1::Present { leaf, opening } => {
                (opening.leaf_transcript(&leaf), None)
            }
        };
        let [low, empty] = body.map_openings.as_slice() else {
            panic!("exactly the two consumed-credit openings")
        };
        Self {
            before: *corpus.state(sequence - 1),
            after: body.successor_state,
            statement: body.statement,
            sigma: body.step_proof.clone(),
            receipt: completion(&corpus.slot, step).receipt,
            consumed: [low.clone(), empty.clone()],
            credit,
            request: original_bytes(step, KagemushaWalletRetainedInputRoleV1::Request),
            payment: original_bytes(step, KagemushaWalletRetainedInputRoleV1::Payment),
            payer_credential: original_bytes(step, KagemushaWalletRetainedInputRoleV1::Credential),
            payer_certificates: original_bytes(
                step,
                KagemushaWalletRetainedInputRoleV1::CertificateSet,
            ),
        }
    }

    pub(crate) fn input(&self) -> SuffixStepInputV1<'_> {
        SuffixStepInputV1::Receive(SuffixReceiveInputV1 {
            before: &self.before,
            after: &self.after,
            statement: &self.statement,
            sigma: &self.sigma,
            receipt: &self.receipt,
            consumed: [&self.consumed[0], &self.consumed[1]],
            credit: match &self.credit {
                (low, Some(empty)) => SuffixCreditWitnessV1::Inserted { low, empty },
                (membership, None) => SuffixCreditWitnessV1::Present { membership },
            },
            request: &self.request,
            payment: &self.payment,
            payer_credential: &self.payer_credential,
            payer_certificates: &self.payer_certificates,
        })
    }
}

/// The independent differential: predict every adjusted field before reading any Receive
/// Omega, then fully verify and compare all archived adjusted public fields.
#[test]
#[ignore = "requires the retained genuine hundred-payer engineering corpus; verifies 201 Omegas"]
fn hundred_genuine_receive_predictions_equal_archived_folded_fields() {
    let corpus = Corpus::load();
    let verifier = corpus.installed.verifier();
    let scheme = verifier.scheme();
    let credential = corpus.credential;
    let frozen = &corpus.frozen;
    let anchor = &corpus.anchor;
    let budget = MemoryBudget::new(128 << 20);
    verifier.verify_lineage(&anchor.lineage, budget).unwrap();
    let mut credit_tree = KagemushaWalletIndexedTreeV1::new();
    let anchor_state = corpus.state(0);
    assert_eq!(
        anchor.lineage.public,
        KagemushaWalletLineagePublicV1 {
            version: 1,
            scheme_id: scheme.scheme_id(),
            relation_id: scheme.relation_id,
            head: anchor_state.commitment().unwrap(),
            wallet_id: credential.body.wallet_id,
            credential_digest: credential.credential_digest(),
            payment_key: credential.body.payment_key,
            lifecycle: anchor_state.core.lifecycle,
            policy_epoch: anchor_state.core.policy_epoch,
            enabled_controls: anchor_state.core.enabled_controls,
            burned_total: 0,
            pending_outgoing_root: anchor_state.core.pending_outgoing_root,
            credit_digest_root: credit_tree.root(),
        }
    );
    let mut predicted = anchor.lineage.public;
    let mut predictions = Vec::with_capacity(RECEIVE_COUNT);
    let mut payers = BTreeSet::new();
    let mut credits = BTreeSet::new();
    let prediction_started = Instant::now();
    // Do not read Fold(1..=100) until all predictions have been finalized.
    for sequence in 1..=RECEIVE_COUNT as u128 {
        let previous = frozen.get(&(sequence - 1)).unwrap();
        let step = frozen.get(&sequence).unwrap();
        let body = step.capsule.body();
        assert_eq!(
            body.predecessor_capsule_digest,
            previous.capsule.custody_digest()
        );
        assert_eq!(body.statement.predecessor, predicted.head);
        assert_eq!(
            body.statement.successor,
            body.successor_state.commitment().unwrap()
        );
        let request: KagemushaWalletRequestV1 =
            original(step, KagemushaWalletRetainedInputRoleV1::Request);
        let payment: KagemushaWalletPaymentV1 =
            original(step, KagemushaWalletRetainedInputRoleV1::Payment);
        let payer: KagemushaWalletCredentialV1 =
            original(step, KagemushaWalletRetainedInputRoleV1::Credential);
        let certificates: KagemushaWalletCertificateSetV1 =
            original(step, KagemushaWalletRetainedInputRoleV1::CertificateSet);
        assert_eq!(request.receiver_credential, step.credential);
        let prepared = payment
            .prepare_receive(
                scheme,
                &payer,
                &certificates,
                &request,
                &step.credential,
                &previous.capsule.body().successor_state,
                None,
            )
            .unwrap();
        assert_eq!(body.statement.effect, prepared.effect);
        assert_eq!(body.payment_digest, prepared.digests.payment);
        assert!(payers.insert(payer.body.wallet_id));
        assert!(credits.insert(prepared.digests.credit_id));
        verifier
            .verify_package_proofs(&payment.send, budget)
            .unwrap();
        verifier
            .verify_capsule_proofs(&step.capsule, &step.credential, budget)
            .unwrap();
        completion(&corpus.slot, step);
        let KagemushaWalletEffectV1::Receive {
            amount,
            payer_wallet_id,
            ..
        } = prepared.effect
        else {
            unreachable!()
        };
        let entry = KagemushaWalletConsumedCreditLeafV1 {
            credit_id: prepared.digests.credit_id,
            amount,
            receive_sequence: sequence,
        };
        let before = &previous.capsule.body().successor_state;
        let after = &body.successor_state;
        assert_eq!(
            consumed_insertion(step)
                .verify(
                    &before.core.consumed_credit_root,
                    &entry.credit_id,
                    &entry.leaf_value().unwrap()
                )
                .unwrap(),
            after.core.consumed_credit_root,
        );
        let mut derived_state = *before;
        derived_state.core.balance = before.core.balance.checked_add(amount).unwrap();
        derived_state.core.sequence = before.core.sequence.checked_add(1).unwrap();
        derived_state.core.state_nonce = after.core.state_nonce;
        derived_state.core.consumed_credit_root = after.core.consumed_credit_root;
        derived_state.core.recv_chain = KagemushaWalletRecvChainEntryV1 {
            credit_id: entry.credit_id,
            payer_wallet_id,
            amount,
        }
        .append(&before.core.recv_chain)
        .unwrap();
        assert_eq!(
            &derived_state, after,
            "the exact state/rest opening at {sequence}"
        );
        let credit = KagemushaWalletCreditDigestLeafV1 {
            credit_id: prepared.digests.credit_id,
            payment_digest: prepared.digests.payment,
            burned: false,
        };
        let update = credit.record(&mut credit_tree).unwrap();
        assert!(matches!(
            update,
            KagemushaWalletCreditDigestRecordV1::Inserted { .. }
        ));
        assert_eq!(
            update
                .verify(&predicted.credit_digest_root, &credit)
                .unwrap(),
            credit_tree.root()
        );
        predicted.head = body.statement.successor;
        predicted.credit_digest_root = credit_tree.root();
        // Every genuine Payment accepts; Receive preserves adjusted pending and burned fields.
        predictions.push(predicted);
    }
    let prediction_ms = prediction_started.elapsed().as_millis();
    assert_eq!(payers.len(), RECEIVE_COUNT);
    let verify_started = Instant::now();
    for (index, expected) in predictions.iter().enumerate() {
        let sequence = (index + 1) as u128;
        let record = corpus.fold(sequence);
        verifier.verify_lineage(&record.lineage, budget).unwrap();
        assert_eq!(
            &record.lineage.public, expected,
            "all adjusted fields, sequence {sequence}"
        );
        if index == 0 {
            for mutation in 0..4 {
                let mut changed = record.lineage.clone();
                match mutation {
                    0 => {
                        changed.public.credit_digest_root = anchor.lineage.public.credit_digest_root
                    }
                    1 => changed.public.pending_outgoing_root = expected.credit_digest_root,
                    2 => changed.public.burned_total += 1,
                    _ => changed.proof[0] ^= 1,
                }
                assert!(
                    verifier.verify_lineage(&changed, budget).is_err(),
                    "mutation {mutation}"
                );
            }
        }
    }
    eprintln!(
        "SUFFIX_ARCHIVED_RECEIVE_DIFFERENTIAL receives={RECEIVE_COUNT} distinct_payers={} prediction_with_original_verification_ms={prediction_ms} archived_omega_verification_ms={} all_public_fields_equal=true new_fold=false suffix_unload=false installed_phone_gate=false",
        payers.len(),
        verify_started.elapsed().as_millis(),
    );
}

#[test]
#[ignore = "requires retained engineering catalog; verifies 100 Receives natively, then proves the suffix-bound Unload Q/A/Omega"]
fn genuine_receive_suffix_receipt_continues_under_installed_unload_and_omega_keys() {
    continuation::run(&Corpus::load());
}

#[test]
#[ignore = "requires retained engineering catalog; adversarial suffixes against genuine Receives"]
fn native_suffix_verifier_rejects_adversarial_suffixes() {
    adversarial::run(&Corpus::load());
}

#[test]
#[ignore = "requires retained engineering catalog; run alone: process-wide instruction counter"]
fn native_suffix_verifier_measures_fifty_distinct_receives() {
    measure::run(&Corpus::load());
}
