//! Bounded pending-carrier progress under interleaved locators and temporary proof loss.
//! Native certificates authenticate synthetic results; these tests do not qualify payments.

use super::*;
use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
use std::{collections::BTreeMap, sync::atomic::AtomicU64};

const TIP: u64 = 70;
const READS: usize = 3;

struct PendingSource {
    proofs: BTreeMap<u64, SumeragiFinalityProof>,
    locations: BTreeMap<[u8; 32], u64>,
    missing: AtomicU64,
    selected: Mutex<Option<[u8; 32]>>,
    requests: Mutex<Vec<([u8; 32], u64)>>,
}
impl Source for PendingSource {
    fn locate(&self, hash: HashOf<SignedTransaction>) -> WorkflowFuture<'_, Result<Location>> {
        Box::pin(async move {
            let hash = *Hash::from(hash).as_ref();
            *self.selected.lock().unwrap() = Some(hash);
            Ok(self
                .locations
                .get(&hash)
                .map_or(Location::Absent, |height| Location::Observed(Some(*height))))
        })
    }
    fn proof(&self, height: NonZeroU64) -> WorkflowFuture<'_, Result<ProofInput>> {
        Box::pin(async move {
            let identity = self
                .selected
                .lock()
                .unwrap()
                .expect("exact carrier selected");
            self.requests.lock().unwrap().push((identity, height.get()));
            if self.missing.load(Ordering::Relaxed) == height.get() {
                return Err(eyre!("selected pending proof is temporarily unavailable"));
            }
            self.proofs
                .get(&height.get())
                .cloned()
                .map(|proof| ProofInput::Fixture(Box::new(proof)))
                .ok_or_else(|| eyre!("no successor or invented checkpoint is available"))
        })
    }
    fn applied_height(&self) -> WorkflowFuture<'_, Result<u64>> {
        Box::pin(async { Ok(TIP) })
    }
}

struct NoEffects;
impl WorkflowLedger for NoEffects {
    fn dispatch<'a>(
        &'a self,
        _: &'a [u8],
        _: &'a TransactionIdentityV1,
    ) -> WorkflowFuture<'a, DispatchOutcome> {
        panic!("pending finality recovery cannot dispatch");
    }
    fn committed_outcome<'a>(
        &'a self,
        _: &'a TransactionIdentityV1,
    ) -> WorkflowFuture<'a, CommittedOutcome> {
        panic!("pending finality recovery cannot delegate outcome authority");
    }
}

fn fixture() -> (
    RelayLedger,
    Arc<PendingSource>,
    BTreeMap<u64, TransactionIdentityV1>,
) {
    let mut native = NativeFinalityFixture::start("amx-bounded-pending-progress");
    let genesis = native.verifier();
    let mut proofs = BTreeMap::from([(1, native.genesis_proof().clone())]);
    let mut locations = BTreeMap::new();
    let mut identities = BTreeMap::new();
    let mut authority = None;
    let mut instruction = None;
    for height in 2..=TIP {
        let block = native.block_with_submitted_work(native.next_header());
        let transaction = block.external_transactions().next().unwrap();
        let identity = TransactionIdentityV1 {
            signed_transaction_hash: *Hash::from(transaction.hash()).as_ref(),
            entrypoint_hash: *Hash::from(transaction.hash_as_entrypoint()).as_ref(),
        };
        authority = Some(transaction.authority().clone());
        let Executable::Instructions(instructions) = transaction.instructions() else {
            panic!("fixture has exactly one native instruction");
        };
        instruction = Some(instruction_digest(&instructions[0]).unwrap());
        locations.insert(identity.signed_transaction_hash, height);
        identities.insert(height, identity);
        proofs.insert(height, native.certify(block));
    }
    let source = Arc::new(PendingSource {
        proofs,
        locations,
        missing: AtomicU64::new(0),
        selected: Mutex::new(None),
        requests: Mutex::new(Vec::new()),
    });
    let ledger = RelayLedger {
        source: source.clone(),
        dispatch: Arc::new(NoEffects),
        custody: PendingCustody::for_test(&genesis, authority.as_ref().unwrap().clone()),
        original: genesis.into_genesis_verifier(),
        network: native.network_id(),
        authority: authority.unwrap(),
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
        instruction: Mutex::new(instruction).into(),
        approval: Mutex::new(None).into(),
        source_plan: Mutex::new(None).into(),
        handoff: Mutex::new(None).into(),
        test_lease: Mutex::new(None).into(),
        max_reads: READS,
        reads: AtomicUsize::new(READS).into(),
    };
    (ledger, source, identities)
}

async fn observe(ledger: &RelayLedger, identity: &TransactionIdentityV1) -> CommittedOutcome {
    let instruction = ledger.instruction.lock().unwrap().unwrap();
    ledger
        .begin_test_bound_observation(identity.signed_transaction_hash, instruction, None, None)
        .unwrap();
    ledger.committed_outcome(identity).await
}

fn applied(outcome: CommittedOutcome, height: u64) -> bool {
    match outcome {
        CommittedOutcome::Applied { block_height } => {
            assert_eq!(block_height, Some(height));
            true
        }
        CommittedOutcome::ReadFailed { .. } => false,
        unexpected => {
            panic!("proof loss cannot authorize absence, rejection or replacement: {unexpected:?}")
        }
    }
}

fn retain_bounded_window(ledger: &RelayLedger) {
    // Exercise the native bounded-retention contract explicitly. Merely growing the
    // verifier forever must not make the interleaving controls pass accidentally.
    let slot = Slot::Pending(ledger.carrier_key().unwrap());
    let saved = ledger.custody.load(&slot).unwrap().unwrap();
    if let Some(verifier) = saved.verifier {
        let newest = verifier.authenticated_height().unwrap();
        let oldest = verifier.oldest_authenticated_height().unwrap();
        assert!(
            newest - oldest < 3,
            "each pending walk retains at most three native decisions"
        );
    }
}

fn reopen(ledger: &RelayLedger) -> RelayLedger {
    ledger.test_lease.lock().unwrap().take();
    RelayLedger {
        source: ledger.source.clone(),
        dispatch: Arc::new(NoEffects),
        original: ledger.original.clone().into_genesis_verifier(),
        custody: ledger.custody.reopen_for_test(),
        network: ledger.network,
        authority: ledger.authority.clone(),
        fee_payment: ledger.fee_payment.clone(),
        instruction: Mutex::new(*ledger.instruction.lock().unwrap()).into(),
        approval: Mutex::new(None).into(),
        source_plan: Mutex::new(None).into(),
        handoff: Mutex::new(None).into(),
        test_lease: Mutex::new(None).into(),
        max_reads: ledger.max_reads,
        reads: AtomicUsize::new(ledger.max_reads).into(),
    }
}

#[tokio::test]
async fn retired_native_prefix_cannot_reacquire_capacity_ahead_of_waiting_original() {
    use super::super::admission::{AdmissionPosition, FreshAdmissions};
    let (mut ledger, _, identities) = fixture();
    ledger.custody =
        PendingCustody::for_test_with_capacity(&ledger.original, ledger.authority.clone(), 1);
    let instruction = ledger.instruction.lock().unwrap().unwrap();
    let first = identities[&70];
    let waiting = identities[&50];
    let first_position = AdmissionPosition {
        height: 2,
        ordinal: 0,
        plan: first.signed_transaction_hash,
    };
    let waiting_position = AdmissionPosition {
        height: 3,
        ordinal: 0,
        plan: waiting.signed_transaction_hash,
    };
    let mut admissions = FreshAdmissions::default();
    admissions.frontier(3);
    assert!(admissions.consider(first_position, true));
    ledger
        .begin_test_bound_observation(first_position.plan, instruction, None, None)
        .unwrap();
    assert!(admissions.admitted(first_position));
    {
        let lease = ledger.test_lease.lock().unwrap();
        let lease = lease.as_ref().unwrap();
        ledger
            .check_observation(lease, first_position.plan, instruction, None)
            .unwrap();
        assert!(
            ledger
                .check_observation(lease, waiting_position.plan, instruction, None)
                .is_err()
        );
        assert!(
            ledger
                .check_observation(
                    lease,
                    first_position.plan,
                    instruction,
                    Some(Hash::new(b"foreign handoff"))
                )
                .is_err()
        );
    }
    ledger = reopen(&ledger);
    assert!(admissions.consider(waiting_position, true));
    assert!(matches!(
        ledger
            .begin_observation(waiting_position.plan, instruction, None, None)
            .unwrap_err()
            .downcast_ref::<super::super::pending_custody::CustodyError>(),
        Some(super::super::pending_custody::CustodyError::Capacity)
    ));
    let mut completed = false;
    for _ in 0..TIP {
        admissions.frontier(3);
        assert!(
            ledger
                .has_pending(first_position.plan, instruction, None)
                .unwrap()
        );
        ledger
            .begin_test_bound_observation(first_position.plan, instruction, None, None)
            .unwrap();
        if applied(ledger.committed_outcome(&first).await, 70) {
            {
                let lease = ledger.test_lease.lock().unwrap().as_ref().unwrap().clone();
                ledger.finish_observation(&lease).await.unwrap();
            }
            completed = true;
            ledger = reopen(&ledger);
            admissions.finish_sweep();
            break;
        }
        ledger = reopen(&ledger);
        assert!(admissions.consider(waiting_position, true));
        assert!(
            ledger
                .begin_observation(waiting_position.plan, instruction, None, None)
                .is_err()
        );
        admissions.finish_sweep();
    }
    assert!(
        completed,
        "the admitted original must finish its actual native H70 walk"
    );
    assert!(
        !ledger
            .has_pending(first_position.plan, instruction, None)
            .unwrap()
    );
    admissions.frontier(3);
    assert!(
        !admissions.consider(first_position, true),
        "a retired recurring source cannot reacquire before the selected waiter"
    );
    assert!(admissions.consider(waiting_position, true));
    ledger
        .begin_test_bound_observation(waiting_position.plan, instruction, None, None)
        .unwrap();
    assert!(admissions.admitted(waiting_position));
    assert!(
        ledger
            .has_pending(waiting_position.plan, instruction, None)
            .unwrap()
    );
    assert!(
        !ledger
            .has_pending(first_position.plan, instruction, None)
            .unwrap()
    );
}

#[tokio::test]
async fn pending_carrier_progress_does_not_retain_all_destination_history() {
    let (ledger, _, identities) = fixture();
    let identity = identities[&TIP];
    let mut complete = false;
    for _ in 0..TIP {
        if applied(observe(&ledger, &identity).await, TIP) {
            complete = true;
            break;
        }
    }
    assert!(
        complete,
        "one pending carrier must advance beyond its per-call read budget"
    );
    let saved = ledger
        .custody
        .load(&Slot::Pending(ledger.carrier_key().unwrap()))
        .unwrap()
        .unwrap();
    let verifier = saved.verifier.unwrap();
    let newest = verifier.authenticated_height().unwrap();
    let oldest = verifier.oldest_authenticated_height().unwrap();
    assert!(
        newest - oldest < 3,
        "pending H{TIP} retained H{oldest}..H{newest}; only the newest three native decisions belong in resident progress"
    );
}

#[tokio::test]
async fn interleaved_pending_carriers_complete_without_resident_history_or_cursor_starvation() {
    let (mut ledger, source, identities) = fixture();
    let mut complete = [false; 3];
    for _ in 0..TIP * 2 {
        for (index, height) in [50, 2, 70].into_iter().enumerate() {
            complete[index] |= applied(observe(&ledger, &identities[&height]).await, height);
            retain_bounded_window(&ledger);
            ledger = reopen(&ledger);
        }
        if complete.into_iter().all(|done| done) {
            break;
        }
    }
    assert_eq!(
        complete, [true; 3],
        "revisiting H2 each source sweep must neither require all historical decisions nor reset the unfinished H50/H70 walks"
    );
    for height in [50, 70] {
        let identity = identities[&height].signed_transaction_hash;
        let reads: Vec<_> = source
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(selected, read)| (*selected == identity).then_some(*read))
            .collect();
        assert!(
            reads.windows(2).all(|pair| pair[0] <= pair[1]),
            "the same exact pending carrier must keep its own authenticated prefix: {reads:?}"
        );
    }
}

#[tokio::test]
async fn missing_pending_proof_does_not_block_an_independent_earlier_carrier() {
    let (mut ledger, source, identities) = fixture();
    source.missing.store(20, Ordering::Relaxed);
    for _ in 0..TIP {
        assert!(!applied(observe(&ledger, &identities[&50]).await, 50));
        retain_bounded_window(&ledger);
        ledger = reopen(&ledger);
        if source
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|(_, height)| *height == 20)
        {
            break;
        }
    }
    assert!(
        source
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|(_, height)| *height == 20)
    );
    let mut earlier = false;
    for _ in 0..4 {
        earlier |= applied(observe(&ledger, &identities[&2]).await, 2);
        retain_bounded_window(&ledger);
        ledger = reopen(&ledger);
    }
    assert!(
        earlier,
        "missing H20 must not hold the independent exact H2 carrier behind H50"
    );
    for _ in 0..TIP / READS as u64 {
        assert!(!applied(observe(&ledger, &identities[&70]).await, 70));
        retain_bounded_window(&ledger);
        ledger = reopen(&ledger);
        assert!(applied(observe(&ledger, &identities[&2]).await, 2));
        retain_bounded_window(&ledger);
        ledger = reopen(&ledger);
    }
    source.missing.store(0, Ordering::Relaxed);
    let mut complete = [false; 2];
    for _ in 0..TIP * 2 {
        for (index, height) in [50, 70].into_iter().enumerate() {
            complete[index] |= applied(observe(&ledger, &identities[&height]).await, height);
            retain_bounded_window(&ledger);
            ledger = reopen(&ledger);
        }
        assert!(applied(observe(&ledger, &identities[&2]).await, 2));
        retain_bounded_window(&ledger);
        ledger = reopen(&ledger);
        if complete.into_iter().all(|done| done) {
            break;
        }
    }
    assert_eq!(
        complete, [true; 2],
        "restoring H20 resumes both original high carriers without new identities"
    );
}

#[tokio::test]
async fn handoff_and_pending_carriers_reopen_independent_frontiers_without_starvation() {
    use iroha_data_model::{
        isi::sumeragi_amx::RelayAmxHandoffV1,
        sumeragi_amx::{AmxCertifiedBlockV1, AmxHandoffProofV1},
    };
    use iroha_model_base::topology::DataSpaceId;
    let (mut ledger, source, identities) = fixture();
    let ordinary_instruction = ledger.instruction.lock().unwrap().unwrap();
    // This lookup is deliberately absent. No synthetic handoff grants authority.
    let lookup: InstructionBox = RelayAmxHandoffV1 {
        dataspace: DataSpaceId::new(91),
        proof: AmxHandoffProofV1 {
            block: AmxCertifiedBlockV1 {
                consensus_header: vec![91],
                commit_qc: vec![],
                result_preimage: vec![91],
            },
        },
    }
    .into();
    let effect = *handoff_effect(&lookup).unwrap().unwrap().as_ref();
    let mut complete = [false; 2];
    for _ in 0..TIP * 2 {
        ledger.begin_test_handoff(&lookup).unwrap();
        *source.selected.lock().unwrap() = Some(effect);
        match ledger.recover_handoff(&lookup).await {
            Ok(found) => {
                assert!(!found);
                complete[0] = true;
            }
            Err(error) => assert!(
                error.to_string().contains("proof-read budget exhausted"),
                "{error}"
            ),
        }
        let key = ledger.handoff.lock().unwrap().as_ref().unwrap().0.clone();
        let saved = ledger.custody.load(&Slot::Pending(key)).unwrap().unwrap();
        if let Some(verifier) = saved.verifier {
            assert!(
                verifier.authenticated_height().unwrap()
                    - verifier.oldest_authenticated_height().unwrap()
                    < 3
            );
        }
        ledger = reopen(&ledger);
        for height in [70, 2] {
            let identity = &identities[&height];
            ledger
                .begin_test_bound_observation(
                    identity.signed_transaction_hash,
                    ordinary_instruction,
                    None,
                    None,
                )
                .unwrap();
            let found = applied(ledger.committed_outcome(identity).await, height);
            if height == 70 {
                complete[1] |= found;
            } else {
                assert!(found);
            }
            retain_bounded_window(&ledger);
            ledger = reopen(&ledger);
        }
        if complete.into_iter().all(|done| done) {
            break;
        }
    }
    assert_eq!(
        complete, [true; 2],
        "old-carrier replay cannot reset the handoff's inspected frontier"
    );
    let reads: Vec<_> = source
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter_map(|(key, height)| (*key == effect).then_some(*height))
        .collect();
    assert_eq!(
        reads,
        (1..=TIP).collect::<Vec<_>>(),
        "the cold handoff inspects its own contiguous prefix exactly once"
    );
}
