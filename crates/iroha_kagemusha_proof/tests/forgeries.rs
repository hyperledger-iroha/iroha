//! Consistent-forgery (malicious-witness) tests of the step relations.
//!
//! The per-cell tamper suite changes one cell and keeps every other cell
//! honest, so a free witness that is only copied into a hash looks pinned.
//! These tests instead let the forger choose a value *consistently*: the
//! forged witness goes through the real circuit, which recomputes every
//! copy and every downstream digest. Each test then asks the question a
//! verifier asks: can the forger make a statement that a consumer accepts
//! (the spec section 3.2 checks of `iroha_kagemusha_proof::consumer`) and
//! that the circuit proves?
//!
//! - **Identity** (credential, asset, scheme, wallet): the identity limbs
//!   are opened core cells, so a forged identity changes the predecessor
//!   commitment. A statement that keeps the real head with another identity
//!   is unsatisfiable; the asset, which the lineage proof does not expose,
//!   is caught by the circuit alone.
//! - **Credit identifier**: it is `H(credit, Request body)` in circuit, in
//!   canonical limbs, so a statement or public output with any other credit
//!   is unsatisfiable, and the consumer recomputes it from the Request.
//! - **`burned_total`**: the lineage input is checked against the balance
//!   and bound in the statement, so dropping it to free burned value fails
//!   the consumer's comparison with the lineage proof.
//! - **Self-payment**: payer and receiver wallets must differ.
//! - **Accepted time**: the successor's floor is the accepted lower time, so
//!   a later Send cannot accept an earlier time.
//! - **Commitments and relation identity**: the statement's predecessor is
//!   the opened commitment (there is no prover-chosen other-parity
//!   component), and the two layouts have distinct relation identities.

mod common;

use common::{CHECK_SEED, check_claim, check_witness, smallest_shape};
use ff::{Field, PrimeField};
use iroha_kagemusha_proof::{
    ConsumerError, LineageView, Mutation, PrefixMode, RelationShape, RequestBody, SigmaShape,
    StateLayout, StatementV1, StepInputs, StepPublic, StepRelation, StepWitness, Violation,
    check_receive, check_send, sample_witness,
};
use iroha_pasta::Fp;
use iroha_plonk::check::CheckFailure;
use iroha_plonk_gadgets::statement::bytes_to_limbs;

const LAYOUT: StateLayout = StateLayout::TwoLevel;

fn relation(step: StepRelation, layout: StateLayout) -> RelationShape {
    RelationShape::new(step, layout, PrefixMode::Folded)
}

fn shape(step: StepRelation) -> SigmaShape {
    smallest_shape(relation(step, LAYOUT))
}

/// The Ω(pred) view of the predecessor of a send witness.
fn view_of(witness: &StepWitness<Fp>, layout: StateLayout) -> LineageView<Fp> {
    let StepInputs::Send(send) = &witness.inputs else {
        panic!("a send witness");
    };
    let core = &witness.predecessor.core;
    LineageView {
        head: witness.predecessor.commitment(layout),
        wallet_id: core.identity.wallet_id,
        credential: core.identity.credential,
        scheme_id: core.identity.scheme_id,
        enabled_controls: core.controls.enabled,
        burned_total: send.lineage.burned_total,
        pending_outgoing_root: send.lineage.pending_outgoing_root,
    }
}

/// The public outputs a consumer derives from a (claimed) statement and
/// the Request body it holds.
fn claimed(statement: &StatementV1<Fp>, request: &RequestBody) -> StepPublic<Fp> {
    StepPublic {
        statement: statement.digest().expect("statement digest"),
        credit_id: (statement.step == StepRelation::Send).then(|| request.credit_id::<Fp>()),
    }
}

fn honest(step: StepRelation) -> StepWitness<Fp> {
    sample_witness::<Fp>(CHECK_SEED, step, Mutation::None)
}

/// An identity field of the core, with a substitute value.
type IdentityEdit = (&'static str, fn(&mut StepWitness<Fp>));

/// Substitutions of each identity field (another credential, asset, scheme
/// or wallet), applied to the predecessor state: the circuit recomputes
/// everything downstream.
fn identity_edits() -> [IdentityEdit; 4] {
    [
        ("credential", |w| {
            w.predecessor.core.identity.credential[0] ^= 0x5a
        }),
        ("asset", |w| w.predecessor.core.identity.asset[7] ^= 0x5a),
        ("scheme", |w| {
            w.predecessor.core.identity.scheme_id[3] ^= 0x5a
        }),
        ("wallet", |w| {
            w.predecessor.core.identity.wallet_id[9] ^= 0x5a
        }),
    ]
}

/// Finding: two-level identity limbs were fresh witnesses never hashed
/// into the state commitment. Now a forged credential, asset, scheme or
/// payer wallet cannot be claimed for the real head.
#[test]
fn identity_substitution_cannot_keep_the_real_head() {
    let honest = honest(StepRelation::Send);
    let view = view_of(&honest, LAYOUT);
    let shape = shape(StepRelation::Send);
    for (name, edit) in identity_edits() {
        let mut forged = honest.clone();
        edit(&mut forged);
        // The forged witness is a valid step from another state: the circuit
        // accepts it for its own statement.
        assert!(forged.evaluate(LAYOUT).is_honest(), "{name}");
        assert!(check_witness(&shape, &forged).is_satisfied(), "{name}");
        let own = forged.statement(LAYOUT).expect("statement");
        assert_ne!(own.predecessor, view.head, "{name}: the head moved");
        let request = forged.request_body();
        assert_eq!(
            check_send(&view, LAYOUT, &request, &own),
            Err(ConsumerError::Predecessor),
            "{name}"
        );
        // The forger's claim: the forged identity with the real head.
        let mut claim = own.clone();
        claim.predecessor = view.head;
        let verdict = check_send(&view, LAYOUT, &request, &claim);
        match name {
            "credential" => assert_eq!(verdict, Err(ConsumerError::Credential)),
            "scheme" => assert_eq!(verdict, Err(ConsumerError::Scheme)),
            "wallet" => assert_eq!(verdict, Err(ConsumerError::Payer)),
            // The lineage proof exposes no asset: only the circuit binds it.
            _ => assert!(verdict.is_ok(), "{name}: {verdict:?}"),
        }
        // Neither the forged nor the honest opening proves the claim.
        let public = claimed(&claim, &request);
        for witness in [&forged, &honest] {
            let report = check_claim(&shape, witness, &public);
            assert!(!report.is_satisfied(), "{name}: claim accepted");
            assert!(
                report
                    .failures()
                    .iter()
                    .all(|failure| matches!(failure, CheckFailure::CopyMismatch { .. }))
            );
        }
    }
}

/// The receive side: a receiver statement that claims another credential
/// or asset than the Request it answers is rejected.
#[test]
fn receive_identity_substitution_is_rejected() {
    let honest = honest(StepRelation::Receive);
    let request = honest.request_body();
    let shape = shape(StepRelation::Receive);
    assert!(
        check_receive(
            LAYOUT,
            &request,
            &honest.statement(LAYOUT).expect("statement")
        )
        .is_ok()
    );
    for (name, edit) in identity_edits() {
        let mut forged = honest.clone();
        edit(&mut forged);
        assert!(check_witness(&shape, &forged).is_satisfied(), "{name}");
        let own = forged.statement(LAYOUT).expect("statement");
        // Against the payer's copy of the Request.
        assert!(check_receive(LAYOUT, &request, &own).is_err(), "{name}");
        // The forger's statement with the honest Request's identity fields.
        let mut claim = own.clone();
        claim.credential = request.receiver_credential;
        claim.asset = request.asset;
        claim.scheme_id = request.scheme_id;
        claim.effect = honest.statement(LAYOUT).expect("statement").effect;
        let public = claimed(&claim, &request);
        assert!(
            !check_claim(&shape, &forged, &public).is_satisfied(),
            "{name}"
        );
    }
}

/// Replaces the credit limbs of a statement effect.
fn with_credit(statement: &StatementV1<Fp>, credit: Fp) -> StatementV1<Fp> {
    let mut claim = statement.clone();
    let [lo, hi] = iroha_plonk_gadgets::statement::foreign_limbs(&credit);
    claim.effect[0] = Fp::from_u128(lo);
    claim.effect[1] = Fp::from_u128(hi);
    claim
}

/// Finding: `credit_id` and the Request digest were free limb pairs. Now
/// the credit identifier is the in-circuit Request digest, in canonical
/// limbs, in the chain, the effect and the public output.
#[test]
fn credit_identifier_is_the_in_circuit_request_digest() {
    for step in [StepRelation::Send, StepRelation::Receive] {
        let honest = honest(step);
        let shape = shape(step);
        let request = honest.request_body();
        let native = honest.evaluate(LAYOUT);
        let credit = request.credit_id::<Fp>();
        assert_eq!(native.digests.credit, credit);
        // The chain entry and the effect carry its canonical limbs.
        let [lo, hi] = native.credit_limbs.map(Fp::from_u128);
        assert_eq!(native.chain_entry[1..3], [lo, hi]);
        assert_eq!(native.statement[18..20], [lo, hi]);
        assert_eq!(native.credit_limbs, bytes_to_limbs(&credit.to_repr()));
        let statement = honest.statement(LAYOUT).expect("statement");
        // Any other credit: a random one, or one the wallet already used.
        let used = sample_witness::<Fp>(CHECK_SEED + 1, step, Mutation::None)
            .request_body()
            .credit_id::<Fp>();
        for other in [used, credit + Fp::ONE] {
            let claim = with_credit(&statement, other);
            let verdict = match step {
                StepRelation::Send => {
                    check_send(&view_of(&honest, LAYOUT), LAYOUT, &request, &claim).map(|_| ())
                }
                StepRelation::Receive => check_receive(LAYOUT, &request, &claim).map(|_| ()),
            };
            assert_eq!(verdict, Err(ConsumerError::Effect), "{step:?}");
            let mut public = claimed(&claim, &request);
            if let Some(credit_id) = &mut public.credit_id {
                *credit_id = other;
            }
            assert!(
                !check_claim(&shape, &honest, &public).is_satisfied(),
                "{step:?}"
            );
        }
        // The honest statement with another public credit identifier.
        if step == StepRelation::Send {
            let public = StepPublic {
                statement: statement.digest().expect("digest"),
                credit_id: Some(used),
            };
            assert!(!check_claim(&shape, &honest, &public).is_satisfied());
        }
    }
}

/// Finding: `sigma_send` ignored `burned_total`. The example of the review:
/// a balance of 100 whose lineage burned 40 cannot send 100.
#[test]
fn burned_value_cannot_be_spent() {
    let shape = shape(StepRelation::Send);
    let mut witness = honest(StepRelation::Send);
    witness.predecessor.core.balance = 100;
    witness.predecessor.core.burned_total = 0;
    let StepInputs::Send(send) = &mut witness.inputs else {
        panic!("send");
    };
    send.lineage.burned_total = 40;
    send.request.amount = 100;
    send.request.fee = 0;
    // The honest witness (the lineage input is Ω(pred)'s 40) overdraws.
    assert_eq!(
        witness.evaluate(LAYOUT).violations,
        vec![Violation::Overdraft]
    );
    let report = check_witness(&shape, &witness);
    assert!(!report.is_satisfied());
    assert!(
        report
            .failures()
            .iter()
            .all(|failure| matches!(failure, CheckFailure::LookupInputMissing { lookup: 0, .. }))
    );
    // 60 is spendable.
    let mut sixty = witness.clone();
    if let StepInputs::Send(send) = &mut sixty.inputs {
        send.request.amount = 60;
    }
    let native = sixty.evaluate(LAYOUT);
    assert!(native.is_honest());
    let successor = native.successor_state.expect("successor");
    assert_eq!(
        (successor.core.balance, successor.core.burned_total),
        (40, 40)
    );
    assert!(check_witness(&shape, &sixty).is_satisfied());
    // The forger drops burned_total to 0: the step is valid, but its
    // statement carries 0, which the consumer compares with Ω(pred)'s 40.
    let view = view_of(&witness, LAYOUT);
    let mut forged = witness.clone();
    if let StepInputs::Send(send) = &mut forged.inputs {
        send.lineage.burned_total = 0;
    }
    assert!(forged.evaluate(LAYOUT).is_honest());
    assert!(check_witness(&shape, &forged).is_satisfied());
    let own = forged.statement(LAYOUT).expect("statement");
    let request = forged.request_body();
    assert_eq!(
        check_send(&view, LAYOUT, &request, &own),
        Err(ConsumerError::BurnedTotal)
    );
    // The statement the consumer would accept (burned_total 40) is not
    // provable from the forged witness.
    let mut claim = own;
    claim.burned_total = 40;
    let public = check_send(&view, LAYOUT, &request, &claim).expect("consumer view");
    assert!(!check_claim(&shape, &forged, &public).is_satisfied());
}

/// Finding: nothing required the payer to differ from the receiver.
#[test]
fn self_payment_has_no_witness() {
    for layout in [StateLayout::TwoLevel, StateLayout::Flat] {
        for step in [StepRelation::Send, StepRelation::Receive] {
            let witness = sample_witness::<Fp>(CHECK_SEED, step, Mutation::SelfPayment);
            assert_eq!(
                witness.evaluate(layout).violations,
                vec![Violation::SelfPayment]
            );
            let report = check_witness(&smallest_shape(relation(step, layout)), &witness);
            assert!(!report.is_satisfied(), "{step:?} {layout:?}");
            assert!(
                report
                    .failures()
                    .iter()
                    .all(|failure| matches!(failure, CheckFailure::CopyMismatch { .. })),
                "{step:?} {layout:?}: {:?}",
                report.failures()
            );
        }
    }
}

/// Finding: the successor kept the predecessor's accepted-time floor. Now a
/// second Send cannot accept a time before the first one's lower bound.
#[test]
fn accepted_time_never_goes_back() {
    let shape = shape(StepRelation::Send);
    let first = honest(StepRelation::Send);
    let StepInputs::Send(first_send) = &first.inputs else {
        panic!("send");
    };
    let lower = first_send.accepted_lower;
    assert!(lower > first.predecessor.core.accepted_time_floor);
    let after = first
        .evaluate(LAYOUT)
        .successor_state
        .expect("honest successor");
    assert_eq!(after.core.accepted_time_floor, lower);
    // The second Send, from the first one's successor.
    let mut second = sample_witness::<Fp>(CHECK_SEED + 2, StepRelation::Send, Mutation::None);
    second.predecessor = after;
    let StepInputs::Send(send) = &mut second.inputs else {
        panic!("send");
    };
    send.lineage.burned_total = after.core.burned_total;
    send.receiver_wallet = first_send.receiver_wallet;
    send.request.policy_epoch = after.core.policy_epoch;
    send.request.request_time = lower - 10;
    send.accepted_lower = lower - 1;
    send.accepted_upper = lower + 600;
    assert_eq!(
        second.evaluate(LAYOUT).violations,
        vec![Violation::AcceptedTimeBelowFloor]
    );
    assert!(!check_witness(&shape, &second).is_satisfied());
    // At the first one's lower bound it is accepted.
    if let StepInputs::Send(send) = &mut second.inputs {
        send.accepted_lower = lower;
    }
    assert!(second.evaluate(LAYOUT).is_honest());
    assert!(check_witness(&shape, &second).is_satisfied());
}

/// Finding: the statement's other-parity components were prover-chosen
/// limbs. The statement now carries only the opened commitments.
#[test]
fn the_statement_predecessor_is_the_opened_commitment() {
    for step in [StepRelation::Send, StepRelation::Receive] {
        let witness = honest(step);
        let shape = shape(step);
        let statement = witness.statement(LAYOUT).expect("statement");
        assert_eq!(
            statement.predecessor,
            witness.predecessor.commitment(LAYOUT)
        );
        let request = witness.request_body();
        let edits: [fn(&mut StatementV1<Fp>); 2] =
            [|s| s.predecessor += Fp::ONE, |s| s.successor += Fp::ONE];
        for edit in edits {
            let mut claim = statement.clone();
            edit(&mut claim);
            assert!(!check_claim(&shape, &witness, &claimed(&claim, &request)).is_satisfied());
        }
    }
}

/// Finding: both layouts shared one relation identity. A flat statement is
/// neither accepted by a two-level consumer nor provable by the two-level
/// circuit, and vice versa.
#[test]
fn layouts_have_distinct_relation_identities() {
    for step in [StepRelation::Send, StepRelation::Receive] {
        let witness = honest(step);
        let request = witness.request_body();
        let two_level = witness.statement(StateLayout::TwoLevel).expect("statement");
        let flat = witness.statement(StateLayout::Flat).expect("statement");
        assert_ne!(two_level.relation_id, flat.relation_id);
        for (layout, foreign) in [
            (StateLayout::TwoLevel, &flat),
            (StateLayout::Flat, &two_level),
        ] {
            let verdict = match step {
                StepRelation::Send => {
                    check_send(&view_of(&witness, layout), layout, &request, foreign).map(|_| ())
                }
                StepRelation::Receive => check_receive(layout, &request, foreign).map(|_| ()),
            };
            assert_eq!(verdict, Err(ConsumerError::Relation));
            // The foreign statement relabelled with this layout's identity is
            // still not this circuit's statement (the commitments differ).
            let mut relabelled = foreign.clone();
            relabelled.relation_id = iroha_kagemusha_proof::relation_id(step, layout);
            let shape = smallest_shape(relation(step, layout));
            assert!(!check_claim(&shape, &witness, &claimed(&relabelled, &request)).is_satisfied());
        }
    }
}
