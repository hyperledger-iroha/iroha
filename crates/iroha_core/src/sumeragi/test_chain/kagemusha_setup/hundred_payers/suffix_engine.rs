//! Fresh Native suffix engine acceptance on the executed engineering chain.
//!
//! Real provider custody, monetary circuits, receipts, BLS certificates and ledger execution;
//! explicit engineering issuer and software FakePlatform. No phone/attestation/live capacity
//! claim. Existing merchant corpus custody is never opened or rewritten. Only its immutable
//! signed catalog is admitted, with the original keys and exact genesis binding.
use super::*;
use iroha_core_zk::kagemusha_wallet_advance_v1::KagemushaWalletFsV1 as EngineFs;

mod power_loss;
mod release_fault;

type EngineWallet<F> = NativeWalletCoordinatorV1<F, FakePlatformV1, DirectoryOriginalsV1>;

trait EngineDevice: Sized {
    type Fs: EngineFs;
    fn enroll(root: PathBuf, key_seed: u8, fixture: Fixture) -> Self;
    fn fixture(&self) -> &Fixture;
    fn open(&self, sources: &Sources) -> EngineWallet<Self::Fs>;
    fn execute_then_lose(
        &self,
        wallet: &mut EngineWallet<Self::Fs>,
        review: state::ReviewedSuffixUnloadV1,
    ) -> Option<Vec<u8>>;
    fn restart(&self) {}
    fn assert_recovered_release(&self) {}
}

impl EngineDevice for Device {
    type Fs = KagemushaWalletStdFsV1;
    fn enroll(root: PathBuf, key_seed: u8, fixture: Fixture) -> Self {
        Self::enroll(root, key_seed, fixture)
    }
    fn fixture(&self) -> &Fixture {
        &self.fixture
    }
    fn open(&self, sources: &Sources) -> EngineWallet<Self::Fs> {
        Self::open(self, sources)
    }
    fn execute_then_lose(
        &self,
        wallet: &mut EngineWallet<Self::Fs>,
        review: state::ReviewedSuffixUnloadV1,
    ) -> Option<Vec<u8>> {
        match wallet.execute_reviewed_suffix_unload(review).unwrap() {
            state::SuffixUnloadExecutionV1::Released(value) => {
                Some(value.to_canonical_bytes().unwrap())
            }
            state::SuffixUnloadExecutionV1::Pending => None,
        }
    }
}
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::SuffixUnloadReleasedV1;
use sha2::{Digest as _, Sha256};

const RECEIVES: usize = 3;
const PAYER: u8 = 41;
const AMOUNT: u128 = 4;
const MAX_TURNS: usize = 64;
const PACK_SHA256: &str = "642ad5d1af375fadaa2c9240f27696d20a45e225d4de72716cccff3dea1bc87b";
const INSTALLATION_SHA256: &str =
    "6e2937c7bdf109a21ee697fa758955518d135bcf5f718835354c099e9cd953b9";

fn canonical<T>(bytes: &[u8]) -> T
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
        .expect("exact canonical Native original")
}

fn require_existing_catalog() -> PathBuf {
    let root = PathBuf::from(
        std::env::var_os("KAGEMUSHA_WALLET_SIGNED_CATALOG")
            .expect("explicit existing engineering catalog required"),
    )
    .canonicalize()
    .unwrap();
    for (name, expected) in [
        ("engineering-verifier-pack.norito", PACK_SHA256),
        ("engineering-installation.norito", INSTALLATION_SHA256),
    ] {
        let bytes = fs::read(root.join(name)).unwrap();
        assert_eq!(
            hex::encode(Sha256::digest(&bytes)),
            expected,
            "immutable {name}"
        );
    }
    root
}

fn install_scheme(ledger: &mut Ledger, sources: &Sources, catalog: &Path) {
    let scheme = *sources.installed.verifier().scheme();
    let register = KagemushaWalletLedgerV1::new(
        scheme.scheme_id(),
        Action::Register {
            scheme: scheme.to_canonical_bytes().unwrap(),
            asset: norito::encode_canonical(&ledger.setup.asset).unwrap(),
            reserve: ledger.setup.reserve.clone(),
            balance_scope: AssetBalanceScope::Global,
        },
    );
    ledger.commit(vec![ledger.sign(95, vec![register.into()])]);
    let install = KagemushaWalletLedgerV1::new(
        scheme.scheme_id(),
        Action::InstallVerifierPack {
            asset: ledger.setup.asset.asset_digest(),
            manifest_digest: sources.installation.manifest_digest,
            pack: fs::read(catalog.join("engineering-verifier-pack.norito")).unwrap(),
        },
    );
    ledger.commit(vec![ledger.sign(95, vec![install.into()])]);
}

fn activate<F: EngineFs>(
    ledger: &mut Ledger,
    wallet: &mut EngineWallet<F>,
    seed: u8,
    scheme: [u8; 32],
) {
    let instruction =
        KagemushaWalletLedgerV1::new(scheme, Action::Activate(wallet.activation().unwrap()));
    let signed = ledger.sign(seed, vec![instruction.into()]);
    let wire = signed.encode_wire_v1().unwrap();
    assert!(matches!(
        wallet.retain_activation_attempt(&wire).unwrap(),
        state::LedgerAttemptOutcomeV1::Awaiting { .. }
    ));
    let height = ledger.commit(vec![signed.clone()]);
    sync_epochs(wallet, &ledger.certificates(), height);
    let finality = ledger.activation_finality(height, &signed);
    assert!(matches!(
        wallet.ingest_activation_finality(&wire, &finality).unwrap(),
        state::LedgerAttemptIngestV1::Outcome(state::LedgerAttemptOutcomeV1::Confirmed(_))
    ));
}

fn ready<F: EngineFs>(
    wallet: &mut EngineWallet<F>,
    request: &state::SuffixUnloadRequestV1,
) -> state::ReviewedSuffixUnloadV1 {
    for _ in 0..MAX_TURNS {
        let decision = match wallet.suffix_unload_prepare(request.clone()).unwrap() {
            ControlFlow::Break(decision) => decision,
            ControlFlow::Continue(job) => {
                wallet.suffix_unload_publish(job.verify().unwrap()).unwrap()
            }
        };
        match decision {
            state::SuffixUnloadDecisionV1::Ready(review) => return review,
            state::SuffixUnloadDecisionV1::Verifying(_) => {}
            state::SuffixUnloadDecisionV1::NotStarted(_) => {
                panic!("fresh preparation must not return named-resume absence");
            }
            _ => panic!("fresh isolated suffix did not progress to a real review"),
        }
    }
    panic!("bounded three-Receive suffix did not finish preparation");
}

fn released<F: EngineFs>(
    wallet: &mut EngineWallet<F>,
    request: &[u8; 32],
) -> SuffixUnloadReleasedV1 {
    for _ in 0..MAX_TURNS {
        let decision = match wallet.suffix_unload_resume(request).unwrap() {
            ControlFlow::Break(decision) => decision,
            ControlFlow::Continue(job) => {
                wallet.suffix_unload_publish(job.verify().unwrap()).unwrap()
            }
        };
        match decision {
            state::SuffixUnloadDecisionV1::Released(summary) => return summary,
            state::SuffixUnloadDecisionV1::Verifying(_)
            | state::SuffixUnloadDecisionV1::Pending => {}
            state::SuffixUnloadDecisionV1::NotStarted(_) => {
                panic!("the persisted selected request disappeared during recovery");
            }
            // Never approve a new debit or interpret post-Advance verification as not performed.
            _ => panic!("selected suffix recovery attempted a fresh review or lost its selection"),
        }
    }
    panic!("bounded named suffix recovery did not publish the release");
}

fn fold<F: EngineFs>(wallet: &mut EngineWallet<F>) {
    wallet.scheduler().set_activity(true, false);
    for _ in 0..1 << 16 {
        match wallet.fold_once().unwrap() {
            FoldStatus::CaughtUp => {
                let snapshot = wallet.snapshot().unwrap();
                assert_eq!(snapshot.fold_backlog, 0);
                assert_eq!(snapshot.folded_balance, Some(snapshot.owned_balance));
                return;
            }
            FoldStatus::Checkpoint { .. } | FoldStatus::Folded(_) => {}
            status @ FoldStatus::Idle => panic!("unexpected fold status {status:?}"),
        }
    }
    panic!("bounded native fold schedule did not complete");
}

fn sync_epochs<F: EngineFs>(
    wallet: &mut EngineWallet<F>,
    certificates: &BTreeMap<u64, Vec<u8>>,
    height: u64,
) {
    loop {
        let current = wallet.epoch_progress().unwrap();
        if height <= current.boundary_height {
            return;
        }
        let boundary = certificates
            .get(&current.boundary_height)
            .expect("verified epoch boundary certificate");
        let next = wallet
            .ingest_epoch_boundary(current.epoch, boundary)
            .unwrap();
        assert!(next.epoch > current.epoch);
    }
}

/// Retain exact Native wire first, execute the real instruction, then ingest the certified
/// inclusion. Neither a transaction hash nor successful Core execution is Native finality.
fn relay<F: EngineFs>(
    ledger: &mut Ledger,
    wallet: &mut EngineWallet<F>,
    request: [u8; 32],
    ordinal: u128,
) -> (Vec<u8>, state::SuffixStageOutcomeV1, Duration) {
    let original = wallet.suffix_unload_instruction(request, ordinal).unwrap();
    let instruction: KagemushaWalletLedgerV1 = canonical(&original);
    let draft = ledger.sign(MERCHANT_SEED, vec![instruction.into()]);
    let mut builder =
        iroha_data_model::transaction::TransactionBuilder::from_payload(draft.payload().clone())
            .unwrap();
    builder.set_ttl(Duration::from_millis(2));
    let signed = builder.sign(key(MERCHANT_SEED).private_key());
    let valid_until = signed
        .creation_time()
        .checked_add(signed.time_to_live().unwrap())
        .unwrap();
    let wire = signed.encode_wire_v1().unwrap();
    assert!(matches!(
        wallet
            .suffix_unload_attempt_outcome(request, ordinal, &wire)
            .unwrap(),
        state::SuffixStageOutcomeV1::Awaiting { .. }
    ));
    let height = ledger.commit(vec![signed.clone()]);
    let inclusion_time = ledger.verified[&height].header().creation_time();
    assert!(signed.creation_time() <= inclusion_time && inclusion_time < valid_until);
    sync_epochs(wallet, &ledger.certificates(), height);
    let finality = ledger.activation_finality(height, &signed);
    if ordinal == 0 {
        let mut wrong = finality.clone();
        *wrong.last_mut().unwrap() ^= 1;
        assert!(
            wallet
                .suffix_unload_ingest_finality(request, ordinal, &wire, &wrong)
                .is_err()
        );
        assert!(matches!(
            wallet
                .suffix_unload_attempt_outcome(request, ordinal, &wire)
                .unwrap(),
            state::SuffixStageOutcomeV1::Awaiting { .. }
        ));
    }
    let state::SuffixStageIngestV1::Outcome(outcome) = wallet
        .suffix_unload_ingest_finality(request, ordinal, &wire, &finality)
        .unwrap()
    else {
        panic!("already synchronized epoch must accept its real certificate")
    };
    assert_eq!(
        wallet
            .suffix_unload_attempt_outcome(request, ordinal, &wire)
            .unwrap(),
        outcome
    );
    assert_eq!(
        wallet
            .suffix_unload_signed_wire(request, ordinal, 0)
            .unwrap()
            .as_deref(),
        Some(wire.as_slice())
    );
    (wire, outcome, valid_until)
}

#[test]
#[ignore = "requires immutable engineering catalog; real fresh Bootstrap/Receive/Unload/Fold proofs, no phone claim"]
fn fresh_three_receive_suffix_survives_lost_publication_and_pays_once() {
    run::<Device>();
}

#[test]
#[ignore = "requires immutable engineering catalog; genuine release-retirement power loss, no phone claim"]
fn fresh_three_receive_suffix_recovers_after_release_retirement_power_loss() {
    run::<power_loss::PowerLossDevice>();
}

#[allow(clippy::too_many_lines)]
fn run<D: EngineDevice>() {
    let catalog = require_existing_catalog();
    let output = PathBuf::from(
        std::env::var_os("KAGEMUSHA_SUFFIX_ENGINE_OUTPUT")
            .expect("fresh exclusive fixture output required"),
    );
    assert!(
        !output.exists(),
        "never reopen or rewrite the retained corpus as a new fixture"
    );
    private_dir(&output);
    let mut ledger = Ledger::new(start());
    // Sources::admit requires the unchanged signed catalog's exact network/genesis and
    // strictly qualifies its entire producer graph. No new catalog or key is generated.
    let sources = Sources::admit(&ledger.setup, &output);
    install_scheme(&mut ledger, &sources, &catalog);
    let scheme = sources.installation.scheme_id;
    let payer_device = D::enroll(
        output.join("payer"),
        1,
        sources.fixture(&ledger.setup.asset, PAYER),
    );
    let merchant_device = D::enroll(
        output.join("merchant"),
        MERCHANT_KEY_SEED,
        sources.fixture(&ledger.setup.asset, MERCHANT_SEED),
    );
    let mut payer = payer_device.open(&sources);
    let mut merchant = merchant_device.open(&sources);
    complete(payer.bootstrap().unwrap());
    complete(merchant.bootstrap().unwrap());
    fold(&mut payer);
    fold(&mut merchant);
    merchant.scheduler().set_activity(false, false);
    activate(&mut ledger, &mut payer, PAYER, scheme);
    // The recipient must be an actual active ledger wallet for Begin reservation.
    activate(&mut ledger, &mut merchant, MERCHANT_SEED, scheme);
    let funded = RECEIVES as u128 * PAY;
    let load_id = id(230, 0);
    let instruction: KagemushaWalletLedgerV1 =
        canonical(&payer.prepare_ledger_load(load_id, funded).unwrap());
    let signed = ledger.sign(PAYER, vec![instruction.into()]);
    let height = ledger.commit(vec![signed]);
    let (receipt, finality) = ledger.load_evidence(
        &payer_device.fixture().account,
        &scheme,
        &payer.snapshot().unwrap().wallet_id,
        &load_id,
        height,
    );
    sync_epochs(&mut payer, &ledger.certificates(), height);
    complete(
        payer
            .execute(action(
                id(231, 0),
                OperationActionV1::Load { receipt, finality },
            ))
            .unwrap(),
    );
    fold(&mut payer);
    assert_eq!(payer.snapshot().unwrap().owned_balance, funded);

    // No merchant fold worker is launched after Fold0. Every Request and Receive goes
    // through Native, which retains real map/source-custody snapshots and exact originals.
    for index in 1..=RECEIVES {
        let request = merchant.request(id(232, index), PAY, None).unwrap();
        let payment = complete(
            payer
                .execute(action(id(233, index), OperationActionV1::Send { request }))
                .unwrap(),
        );
        let companion = payer.payment_companion(&payment).unwrap();
        let received = complete(
            merchant
                .execute(receive(&payment, &companion, id(234, index)))
                .unwrap(),
        );
        assert!(!received.is_empty());
        assert_eq!(
            complete(
                merchant
                    .execute(receive(&payment, &companion, id(234, index)))
                    .unwrap()
            ),
            received
        );
        let snapshot = merchant.snapshot().unwrap();
        assert_eq!(snapshot.sequence, index as u128);
        assert_eq!(snapshot.verified_fold.unwrap().sequence, 0);
        assert_eq!(snapshot.fold_backlog, index as u128);
        assert_eq!(snapshot.folded_balance, None);
        assert_eq!(snapshot.owned_balance, index as u128 * PAY);
        if index != RECEIVES {
            fold(&mut payer);
        }
    }
    let before = merchant.snapshot().unwrap();
    let request = state::SuffixUnloadRequestV1 {
        request_id: id(235, 0),
        amount: AMOUNT,
        charge: None,
        beneficiary: None,
    };

    // Named absence is authenticated DATA for this exact unstarted request. It permits
    // the caller to prepare, but cannot stand in for a proof or a persisted debit.
    let ControlFlow::Break(state::SuffixUnloadDecisionV1::NotStarted(absent)) =
        merchant.suffix_unload_resume(&request.request_id).unwrap()
    else {
        panic!("fresh request must have an authenticated named absence");
    };
    assert_eq!(absent.state, 7);
    assert_eq!(absent.request_id, Some(request.request_id));
    assert_eq!(merchant.snapshot().unwrap(), before);

    // Lose a genuine proved preparation result before its publication. Reopen the same
    // provider and keys; no progress DATA is substituted for a verified cursor.
    let mut lost = false;
    for _ in 0..MAX_TURNS {
        match merchant.suffix_unload_prepare(request.clone()).unwrap() {
            ControlFlow::Continue(job) => {
                drop(job.verify().unwrap());
                lost = true;
                break;
            }
            ControlFlow::Break(state::SuffixUnloadDecisionV1::Verifying(_)) => {}
            _ => panic!("fresh suffix must require genuine verification"),
        }
    }
    assert!(lost);
    drop(merchant);
    let mut merchant = merchant_device.open(&sources);
    assert_eq!(merchant.snapshot().unwrap(), before);
    let review = ready(&mut merchant, &request);
    let projection = review.projection().clone();
    assert_eq!(projection.anchor_sequence, 0);
    assert_eq!(projection.suffix_step_count, RECEIVES as u128);
    assert_eq!(projection.amount, AMOUNT);
    let begin_original = merchant
        .suffix_unload_instruction(request.request_id, 0)
        .unwrap();
    // A genuine review alone does not permit Advance before a certified Begin.
    let failure = merchant
        .execute_reviewed_suffix_unload(review)
        .err()
        .expect("certified Begin is required before Advance");
    let (error, retained_review) = failure.into_parts();
    assert!(matches!(
        error,
        state::Error::Invalid("suffix Begin finality required before Advance")
    ));
    assert!(
        retained_review.is_none(),
        "a missing Begin is not a maintenance wait"
    );
    assert_eq!(merchant.snapshot().unwrap(), before);
    let review = ready(&mut merchant, &request);
    assert_eq!(review.projection(), &projection);
    assert_eq!(
        merchant
            .suffix_unload_instruction(request.request_id, 0)
            .unwrap(),
        begin_original
    );
    let (begin_wire, begin, _) = relay(&mut ledger, &mut merchant, request.request_id, 0);
    assert!(matches!(
        begin,
        state::SuffixStageOutcomeV1::StageConfirmed(_)
    ));
    assert_eq!(merchant.snapshot().unwrap(), before);
    assert_eq!(ledger.balance(MERCHANT_SEED), quantity(0));

    // Lose the result after the real reviewed hardware Advance. Named resume is the only
    // recovery path; it may perform bounded Verifying passes and must never ask for approval.
    let first_summary = merchant_device.execute_then_lose(&mut merchant, review);
    drop(merchant);
    merchant_device.restart();
    let mut merchant = merchant_device.open(&sources);
    let summary = released(&mut merchant, &request.request_id);
    summary.validate().unwrap();
    assert_eq!(summary.authority_digest, projection.authority_digest);
    assert_eq!(summary.step_count, RECEIVES as u128);
    assert_eq!(summary.amount, AMOUNT);
    if let Some(first) = first_summary {
        assert_eq!(first, summary.to_canonical_bytes().unwrap());
    }
    let after = merchant.snapshot().unwrap();
    assert_eq!(after.sequence, before.sequence + 1);
    assert_eq!(after.owned_balance, funded - AMOUNT);
    assert_eq!(ledger.balance(MERCHANT_SEED), quantity(0));
    assert_eq!(
        merchant
            .suffix_unload_signed_wire(request.request_id, 0, 0)
            .unwrap(),
        Some(begin_wire)
    );
    let finalize = merchant
        .suffix_unload_instruction(request.request_id, summary.instruction_count - 1)
        .unwrap();
    drop(merchant);
    merchant_device.assert_recovered_release();
    let mut merchant = merchant_device.open(&sources);
    assert_eq!(
        released(&mut merchant, &request.request_id)
            .to_canonical_bytes()
            .unwrap(),
        summary.to_canonical_bytes().unwrap()
    );
    assert_eq!(
        merchant
            .suffix_unload_instruction(request.request_id, summary.instruction_count - 1)
            .unwrap(),
        finalize
    );
    assert_eq!(merchant.snapshot().unwrap(), after);

    let mut confirmed_append = None;
    for ordinal in 1..summary.instruction_count {
        let (wire, outcome, valid_until) =
            relay(&mut ledger, &mut merchant, request.request_id, ordinal);
        if ordinal + 1 == summary.instruction_count {
            assert!(matches!(
                outcome,
                state::SuffixStageOutcomeV1::UnloadConfirmed(_)
            ));
            assert_eq!(ledger.balance(MERCHANT_SEED), quantity(AMOUNT));
        } else {
            assert!(matches!(
                outcome,
                state::SuffixStageOutcomeV1::StageConfirmed(_)
            ));
            assert_eq!(ledger.balance(MERCHANT_SEED), quantity(0));
            if confirmed_append.is_none() {
                confirmed_append = Some((ordinal, wire, outcome, valid_until));
            }
        }
    }
    // A new outer transaction containing the exact same inner Finalize is an idempotent
    // ledger retry. Replaying the same outer transaction may instead be deduplicated.
    let same_finalize: KagemushaWalletLedgerV1 = canonical(&finalize);
    ledger.commit(vec![ledger.sign(MERCHANT_SEED, vec![same_finalize.into()])]);
    assert_eq!(ledger.balance(MERCHANT_SEED), quantity(AMOUNT));
    assert_eq!(merchant.snapshot().unwrap().owned_balance, funded - AMOUNT);

    // Genuine ordinary Q/A/Omega folding must consume this same suffix receipt, after
    // which an ordinary reviewed Unload can again authenticate the exact current source.
    fold(&mut merchant);
    let folded = merchant.snapshot().unwrap();
    assert_eq!(folded.fold_backlog, 0);
    assert_eq!(folded.folded_balance, Some(funded - AMOUNT));
    let ordinary = merchant
        .review_unload(id(236, 0), 1, None, None, state::ReviewExpectationV1::Fresh)
        .unwrap();
    assert!(matches!(ordinary, state::ReviewDecisionV1::Ready(_)));
    drop(ordinary);
    assert_eq!(merchant.snapshot().unwrap().owned_balance, funded - AMOUNT);

    // A later certified block is past this exact Append envelope's TTL. Completion
    // and genuine ordinary publication must not strand the original needed to repair
    // a lost Core receipt. The recovery reads neither sign nor submit another wire.
    let (ordinal, wire, outcome, valid_until) = confirmed_append.expect("a genuine Append");
    assert!(
        ledger
            .verified
            .last_key_value()
            .unwrap()
            .1
            .header()
            .creation_time()
            > valid_until
    );
    drop(merchant);
    let mut merchant = merchant_device.open(&sources);
    assert_eq!(merchant.snapshot().unwrap(), folded);
    assert_eq!(
        merchant
            .suffix_unload_signed_wire(request.request_id, ordinal, 0)
            .unwrap(),
        Some(wire.clone())
    );
    assert_eq!(
        merchant
            .suffix_unload_attempt_outcome(request.request_id, ordinal, &wire)
            .unwrap(),
        outcome
    );
    assert!(
        merchant
            .suffix_unload_signed_wire(request.request_id, ordinal, 1)
            .unwrap()
            .is_none(),
        "readback must not create a fresh signed attempt"
    );
    assert_eq!(merchant.snapshot().unwrap(), folded);
    assert_eq!(ledger.balance(MERCHANT_SEED), quantity(AMOUNT));
}
