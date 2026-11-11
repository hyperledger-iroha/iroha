//! Adversarial suffixes against genuine Receives: every one is refused, the cursor is
//! unchanged by each refusal, and the genuine step still verifies afterwards.
use super::*;

const PREFIX: usize = 4;

struct Probe<'a> {
    verifier: &'a SuffixVerifierV1<'a>,
    cursor: SuffixVerificationCursorV1,
    refused: usize,
}

impl Probe<'_> {
    fn refuse(&mut self, label: &str, step: SuffixStepInputV1<'_>) {
        let before = self.cursor.progress();
        let point = *self.cursor.point().0;
        let error = self
            .cursor
            .verify_step(self.verifier, step, None)
            .err()
            .unwrap_or_else(|| panic!("{label} must be refused"));
        assert!(error.is_rejection(), "{label}: {error}");
        assert_eq!(self.cursor.progress(), before, "{label}: cursor unchanged");
        assert_eq!(*self.cursor.point().0, point, "{label}: tip unchanged");
        eprintln!("SUFFIX_ADVERSARIAL {label} refused={error}");
        self.refused += 1;
    }

    fn accept(&mut self, step: SuffixStepInputV1<'_>) {
        self.cursor.verify_step(self.verifier, step, None).unwrap();
    }
}

fn receive(step: &StepOriginals) -> SuffixReceiveInputV1<'_> {
    let SuffixStepInputV1::Receive(input) = step.input() else {
        unreachable!()
    };
    input
}

#[allow(clippy::too_many_lines)]
pub(super) fn run(corpus: &Corpus) {
    let budget = MemoryBudget::new(128 << 20);
    let verifier = SuffixVerifierV1::ledger(corpus.installed.verifier(), budget);
    let certificates = corpus.certificates();
    let steps = corpus.steps(PREFIX as u128 + 1);
    // Stale or forked base: a mutated anchor Omega, a state that is not its opening, and a
    // later head presented as the anchor without its own Omega.
    let mut lineage = corpus.anchor.lineage.clone();
    lineage.proof[100] ^= 1;
    let forged = SuffixAnchorInputV1 {
        lineage: &lineage,
        ..corpus.anchor_input(&certificates)
    };
    assert!(verifier.verify_anchor(forged, None).is_err());
    let forked = SuffixAnchorInputV1 {
        state: corpus.state(1),
        ..corpus.anchor_input(&certificates)
    };
    assert!(
        verifier
            .verify_anchor(forked, None)
            .err()
            .unwrap()
            .is_rejection()
    );
    let anchor = verifier
        .verify_anchor(corpus.anchor_input(&certificates), None)
        .unwrap();
    let mut probe = Probe {
        verifier: &verifier,
        cursor: verifier.cursor_at_anchor(&anchor).unwrap(),
        refused: 0,
    };
    let cancelled = iroha_pasta::CancellationToken::new();
    cancelled.cancel();
    let initial = probe.cursor.progress();
    assert!(
        probe
            .cursor
            .verify_step(&verifier, steps[0].input(), Some(&cancelled))
            .unwrap_err()
            .is_cancelled()
    );
    assert_eq!(probe.cursor.progress(), initial);
    probe.accept(steps[0].input());

    // Omission and reorder: step 3 presented where step 2 belongs, then the honest
    // predecessor patched in so the omission is caught by the transition itself.
    probe.refuse("omission", steps[2].input());
    let mut skipped = steps[2].clone();
    skipped.before = steps[1].before;
    probe.refuse("omission_patched_predecessor", skipped.input());
    // Duplicate credit: the already consumed first Receive again, from the current head.
    let mut duplicate = steps[0].clone();
    duplicate.before = steps[1].before;
    probe.refuse("duplicate_credit", duplicate.input());
    // Wrong predecessor and substituted roots.
    let mut wrong = steps[1].clone();
    wrong.before.core.balance += 1;
    probe.refuse("wrong_predecessor", wrong.input());
    let mut wrong = steps[1].clone();
    wrong.after.core.consumed_credit_root = steps[0].after.core.consumed_credit_root;
    probe.refuse("substituted_consumed_root", wrong.input());
    let mut wrong = steps[1].clone();
    wrong.after.core.balance += 1;
    probe.refuse("non_effect_balance_change", wrong.input());
    let mut wrong = steps[1].clone();
    wrong.after.core.pending_outgoing_root = steps[0].after.core.consumed_credit_root;
    probe.refuse("substituted_pending_root", wrong.input());
    let mut wrong = steps[1].clone();
    let last = wrong.consumed[0].len() - 1;
    wrong.consumed[0][last] ^= 1;
    probe.refuse("substituted_consumed_path", wrong.input());
    let mut wrong = steps[1].clone();
    let last = wrong.credit.0.len() - 1;
    wrong.credit.0[last] ^= 1;
    probe.refuse("substituted_credit_path", wrong.input());
    let wrong = steps[1].clone();
    let present = SuffixReceiveInputV1 {
        credit: SuffixCreditWitnessV1::Present {
            membership: &wrong.credit.0,
        },
        ..receive(&wrong)
    };
    probe.refuse(
        "credit_present_claim_for_absent_credit",
        SuffixStepInputV1::Receive(present),
    );
    // Mutated Payment, sigma and receipt; another payer's credential and certificates.
    let mut wrong = steps[1].clone();
    let middle = wrong.payment.len() / 2;
    wrong.payment[middle] ^= 1;
    probe.refuse("mutated_payment", wrong.input());
    let mut wrong = steps[1].clone();
    let proof_at = wrong.payment.len() - 600;
    wrong.payment[proof_at] ^= 1;
    probe.refuse("mutated_payer_proof", wrong.input());
    let mut wrong = steps[1].clone();
    wrong.sigma.bytes[64] ^= 1;
    probe.refuse("mutated_own_sigma", wrong.input());
    let mut wrong = steps[1].clone();
    wrong.receipt.capsule_commitment = steps[0].receipt.capsule_commitment;
    probe.refuse("mutated_own_receipt", wrong.input());
    let mut wrong = steps[1].clone();
    wrong.receipt.payment_digest = steps[0].receipt.payment_digest;
    probe.refuse("receipt_for_another_payment", wrong.input());
    let mut wrong = steps[1].clone();
    wrong
        .payer_credential
        .clone_from(&steps[2].payer_credential);
    probe.refuse("another_payer_credential", wrong.input());
    let mut wrong = steps[1].clone();
    wrong.request.clone_from(&steps[2].request);
    probe.refuse("another_request", wrong.input());
    // A wrong amount: a statement or successor that credits one unit more.
    let mut wrong = steps[1].clone();
    if let KagemushaWalletEffectV1::Receive { amount, .. } = &mut wrong.statement.effect {
        *amount += 1;
    }
    probe.refuse("wrong_amount_statement", wrong.input());
    // A Payment step disguised as a prior Unload replaying the next redemption ordinal.
    let prior = SuffixPriorUnloadInputV1 {
        before: &steps[1].before,
        after: &steps[1].after,
        statement: &steps[1].statement,
        sigma: &steps[1].sigma,
        receipt: &steps[1].receipt,
        proof_digest: steps[1].receipt.payment_digest,
        recovery: [&steps[1].consumed[0], &steps[1].consumed[1]],
        charge: None,
    };
    probe.refuse(
        "receive_as_prior_unload",
        SuffixStepInputV1::PriorUnload(prior),
    );
    // After every refusal the genuine step still verifies from the unchanged cursor.
    probe.accept(steps[1].input());
    // Reorder: step 4 before step 3.
    probe.refuse("reorder", steps[3].input());
    probe.accept(steps[2].input());
    probe.accept(steps[3].input());
    let refused = probe.refused;

    // Segments: a claimed start is authority only after linking from the anchor.
    let segment_one = probe.cursor.segment().unwrap();
    let mut claim = verifier
        .cursor_at_claim(
            anchor.credential(),
            probe.cursor.point().0,
            probe.cursor.point().1,
        )
        .unwrap();
    claim
        .verify_step(&verifier, steps[4].input(), None)
        .unwrap();
    let segment_two = claim.segment().unwrap();
    assert!(
        claim.into_tip().is_err(),
        "a claimed start never becomes a tip"
    );
    let tip = verifier.link(&anchor, &[segment_one, segment_two]).unwrap();
    assert_eq!(tip.steps(), (PREFIX as u32 + 1, PREFIX as u32 + 1, 0));
    assert_eq!(tip.point().0, &steps[4].after);
    let mut links = 0;
    for (label, segments) in [
        ("reordered_segments", vec![segment_two, segment_one]),
        ("omitted_segment", vec![segment_two]),
        (
            "duplicated_segment",
            vec![segment_one, segment_one, segment_two],
        ),
    ] {
        let error = verifier.link(&anchor, &segments).err().unwrap();
        assert!(error.is_rejection(), "{label}: {error}");
        eprintln!("SUFFIX_ADVERSARIAL {label} refused={error}");
        links += 1;
    }
    let mut forged = segment_two;
    forged.start_adjusted.credit_digest_root = segment_one.start_adjusted.credit_digest_root;
    let mut fake = segment_two;
    fake.end.core.balance += 1;
    let mut overcount = segment_two;
    overcount.steps += 1;
    for (label, changed) in [
        ("forged_segment_start_root", forged),
        ("forged_segment_end_state", fake),
        ("segment_count_mismatch", overcount),
    ] {
        let error = verifier
            .link(&anchor, &[segment_one, changed])
            .err()
            .unwrap();
        assert!(error.is_rejection(), "{label}: {error}");
        eprintln!("SUFFIX_ADVERSARIAL {label} refused={error}");
        links += 1;
    }
    // A claimed start that is not an opening of its claimed head.
    let mut adjusted = *probe.cursor.point().1;
    adjusted.head = segment_one.start;
    assert!(
        verifier
            .cursor_at_claim(anchor.credential(), probe.cursor.point().0, &adjusted)
            .err()
            .unwrap()
            .is_rejection()
    );
    // Batched decisions: the same genuine steps settle to the same tip, and a refused step
    // leaves the batched cursor unchanged.
    let batched = SuffixVerifierV1::ledger(corpus.installed.verifier(), budget).batched();
    let mut cursor = batched.cursor_at_anchor(&anchor).unwrap();
    for step in &steps {
        cursor.verify_step(&batched, step.input(), None).unwrap();
    }
    assert_eq!(cursor.progress().settled_steps, 0);
    let mut wrong = steps[4].clone();
    wrong.after.core.balance += 1;
    let before = cursor.progress();
    assert!(
        cursor
            .verify_step(&batched, wrong.input(), None)
            .unwrap_err()
            .is_rejection()
    );
    assert_eq!(cursor.progress(), before);
    assert!(
        cursor.segment().is_err(),
        "an unsettled run is never a segment"
    );
    assert!(
        cursor.settle(&verifier, None).unwrap_err().is_rejection(),
        "an unbatched verifier cannot settle a batched cursor"
    );
    assert_eq!(cursor.progress(), before);
    cursor.settle(&batched, None).unwrap();
    assert_eq!(cursor.progress().settled_steps, PREFIX as u32 + 1);
    let batched_segment = cursor.segment().unwrap();
    let batched_tip = cursor.into_tip().unwrap();
    assert_eq!(batched_tip.point().0, &steps[4].after);
    assert_eq!(batched_segment.end_adjusted, segment_two.end_adjusted);
    assert_eq!(batched_segment.end, segment_two.end);
    let mut interrupted = batched.cursor_at_anchor(&anchor).unwrap();
    interrupted
        .verify_step(&batched, steps[0].input(), None)
        .unwrap();
    assert!(
        interrupted
            .settle(&batched, Some(&cancelled))
            .unwrap_err()
            .is_cancelled(),
        "cancellation is not an invalid-payment verdict"
    );
    assert_eq!(interrupted.progress().completed_steps, 0);
    assert_eq!(interrupted.point(), anchor.point());
    assert!(interrupted.segment().is_err());
    interrupted
        .verify_step(&batched, steps[0].input(), None)
        .unwrap();
    interrupted.settle(&batched, None).unwrap();
    assert_eq!(interrupted.into_tip().unwrap().steps(), (1, 1, 0));
    // Unbatched and batched cursors are separate modes.
    let mut unbatched = verifier.cursor_at_anchor(&anchor).unwrap();
    unbatched
        .verify_step(&verifier, steps[0].input(), None)
        .unwrap();
    assert!(
        unbatched
            .verify_step(&batched, steps[1].input(), None)
            .unwrap_err()
            .is_rejection()
    );
    // A segment row survives its canonical encoding exactly.
    let bytes = segment_two.to_canonical_bytes().unwrap();
    assert_eq!(
        SuffixSegmentV1::decode_canonical(&bytes).unwrap(),
        segment_two
    );
    assert!(
        bytes.len() <= 8 << 10,
        "segment frame {} bytes",
        bytes.len()
    );
    eprintln!(
        "SUFFIX_ADVERSARIAL_RESULT step_refusals={refused} link_refusals={links} anchor_refusals=2 claim_refusals=1 segment_frame_bytes={} all_refused=true cursor_unchanged=true",
        bytes.len()
    );
}
