//! Head statement opening for both authenticated folded CreditStatus forms, against the
//! retained signed originals.
//!
//! Two kinds of substitution are kept apart:
//! - A witness-only substitution changes the private 26-word preimage while every public
//!   word stays authentic. The exact-original opening is a hard constraint, so neither the
//!   removal nor the no-op is satisfiable. A prover with fixed authentic evidence cannot
//!   elect the no-op.
//! - An expectation substitution changes public evidence: the exact statement digest
//!   together with a preimage that opens it, the head receipt, or the expected words. The
//!   evidence result is then derived false, the no-op is required and removal is refused.
//!   This is the original malformed-evidence semantics.
//!
//! A coherent but unsigned head isolates each semantic predicate from the exact original.
//! Its receipt, folded words and digest all agree with a changed statement, and only the
//! signature binds the exact original. The predicates for the form kind, subject, scheme
//! and Receive effect must still derive a false evidence bit. Fields the form does not
//! require must not.
use super::*;
use crate::{payment_wire::limbs, receipt, statement};
use receiver_signature::ReceiverReceipt;

const ACCEPTED: [Fp; 4] = [Fp::ONE; 4];
// Evidence false, receipt signature valid.
const REJECTED: [Fp; 4] = [Fp::ONE, Fp::ZERO, Fp::ONE, Fp::ONE];
// Evidence false, receipt signature invalid.
const REJECTED_UNSIGNED: [Fp; 4] = [Fp::ONE, Fp::ZERO, Fp::ZERO, Fp::ONE];
// Evidence true, receipt signature invalid: only the exact original rejects it.
const UNSIGNED: [Fp; 4] = [Fp::ONE, Fp::ONE, Fp::ZERO, Fp::ONE];
const RECEIVE: u64 = 4;
const SEND: u64 = 3;

struct Form<'a> {
    name: &'static str,
    inputs: Inputs<'a>,
    authentic: ReceiverReceipt,
    accepted: Case,
    noop: Case,
}

enum Expect {
    // Both branches unsatisfiable while the public input stays authentic.
    Unsatisfiable,
    // Exactly these derived result bits, the no-op holds and removal is refused.
    NoOp([Fp; 4]),
}

impl Form<'_> {
    fn build(&self, case: &Case, receipt: &ReceiverReceipt) -> Built {
        case.build_for_receipt(Some(self.inputs.binding()), Some(receipt))
    }

    fn other_kind(&self) -> Fp {
        Fp::from(if self.authentic.send { RECEIVE } else { SEND })
    }

    // The amount field of this form's effect: Receive s[20], Send s[21].
    fn amount(&self) -> usize {
        if self.authentic.send { 21 } else { 20 }
    }

    fn check(&self, program: [u8; 32], label: &str, r: &ReceiverReceipt, expect: Expect) {
        let label = format!("{} form: {label}", self.name);
        let noop = self.build(&self.noop, r);
        assert_eq!(noop.0.program_digest(), program, "{label}: one shape");
        assert_eq!(&noop.1.io[64..79], &r.public_words(), "{label}");
        match expect {
            Expect::Unsatisfiable => {
                assert_eq!(
                    &noop.1.io[64..79],
                    &self.authentic.public_words(),
                    "{label}: witness-only"
                );
                assert!(
                    !holds(&noop),
                    "{label}: private preimage cannot elect the no-op"
                );
            }
            Expect::NoOp(bits) => {
                assert_eq!(&noop.1.io[55..59], &bits, "{label}: derived results");
                assert!(
                    holds(&noop),
                    "{label}: the derived false result forces the no-op"
                );
            }
        }
        drop(noop);
        assert!(
            !holds(&self.build(&self.accepted, r)),
            "{label}: cannot remove the pending descriptor"
        );
    }
}

// A head receipt and folded words that agree with `s` in every field the statement
// supplies. The signature still covers the original body, so it no longer verifies.
fn coherent(base: &ReceiverReceipt, s: [Fp; 26]) -> ReceiverReceipt {
    let mut r = base.clone();
    let digest = statement::digest(&s);
    let [a, b] = limbs(&r.receipt.body[34..66]);
    let id = poseidon::hash_with_domain(u64::from_le_bytes(*b"kgwopid1"), &[a, b, s[16], s[17]]);
    r.receipt.body[98..114].copy_from_slice(&s[9].to_repr()[..16]);
    for (at, word) in [(114, id), (146, s[14]), (178, s[15]), (210, digest)] {
        r.receipt.body[at..at + 32].copy_from_slice(&word.to_repr());
    }
    r.statement = s;
    r.statement_digest = digest;
    r.expected[1] = receipt::digest(&r.receipt);
    r.expected[5] = r.expected[1];
    r.expected[2] = s[15];
    r.expected[3] = digest;
    r
}

fn form<'a>(name: &'static str, inputs: Inputs<'a>, nonce: Fp) -> Form<'a> {
    let authentic = delivered_receipt(inputs.delivery).unwrap();
    let accepted = inputs.case(nonce).unwrap();
    let mut noop = accepted.clone();
    noop.after.lineage[bc::PENDING_ROOT] = noop.before.lineage[bc::PENDING_ROOT];
    Form {
        name,
        inputs,
        authentic,
        accepted,
        noop,
    }
}

fn with_forms(action: impl FnOnce(&[Form<'_>; 2], &native::VerifiedIncomingPayment, [u8; 32])) {
    with_retained_inputs(|payer, send, sent, engine, parameters, wallet| {
        let (oracle, root) = retained_receiver_receipt(payer.delivery, engine, parameters, wallet);
        let (later, _, _, wire) = later_send_status(&payer, root, engine, parameters, wallet);
        let delivery = engine
            .verify_sent_delivery(send, sent, payer.delivery.open_request(), &wire, parameters)
            .unwrap();
        let nonce =
            funding::fresh_state_nonce(Some(payer.current.successor_opening().0[NONCE])).unwrap();
        let later_inputs = Inputs {
            delivery: &delivery,
            ..payer
        };
        let forms = [
            form("Receive", payer, nonce),
            form("later Send", later_inputs, nonce),
        ];
        let [received, headed] = [&forms[0].authentic, &forms[1].authentic];
        // Independent oracles for the two private originals. The counterparty's own
        // released Receive head fixes the Receive statement digest. The actual later
        // Payment that the receiver released carries the later Send statement.
        assert!(!received.send && headed.send);
        assert_eq!(received.statement_digest, oracle.statement_digest);
        assert_eq!(received.statement, oracle.statement);
        assert_eq!(&headed.statement, later.statement());
        for r in [received, headed] {
            // For authentic evidence the exact original and the folded statement agree.
            assert_eq!(r.statement_digest, statement::digest(&r.statement));
            assert_eq!(r.statement_digest, r.expected[3]);
            assert_eq!(r.subject, r.statement[17]);
        }
        assert_ne!(received.statement_digest, headed.statement_digest);
        // One fixed shape for both forms and every substitution below.
        let positive = forms[0].build(&forms[0].accepted, received);
        let program = positive.0.program_digest();
        drop(positive);
        action(&forms, &later, program);
    });
}

#[test]
#[ignore = "requires signed receiver Receive and later Send originals"]
fn successor_archive_statement_opening_proves_both_forms_under_the_default_verifier() {
    with_forms(|forms, _, program| {
        let verifier = Verifier::new();
        assert_eq!(verifier.0.program_digest(), program, "default verifier");
        let mut ios = Vec::new();
        for form in forms {
            let r = &form.authentic;
            let positive = form.build(&form.accepted, r);
            assert_eq!(positive.0.program_digest(), program, "{}", form.name);
            assert_eq!(&positive.1.io[55..59], &ACCEPTED, "{}", form.name);
            assert!(holds(&positive), "{} form selects removal", form.name);
            drop(positive);
            assert!(
                !holds(&form.build(&form.noop, r)),
                "{} form: authentic evidence cannot elect the no-op",
                form.name
            );
            let (nonce, proof) = form.inputs.prove().unwrap();
            let (joined, instance, witness) = form.inputs.joined(nonce).unwrap();
            drop(witness);
            assert_eq!(joined.program_digest(), program, "{}", form.name);
            drop(joined);
            let io = form.inputs.public_io(nonce, verifier.0.io_len).unwrap();
            assert_eq!(
                io, instance.io,
                "{}: verifier recomputes the input",
                form.name
            );
            assert_eq!(&io[55..59], &ACCEPTED);
            assert_eq!(io[76], Fp::from(u64::from(r.send)));
            assert_eq!(io[77], r.statement[17]);
            assert_eq!(io[78], statement::digest(&r.statement));
            assert_eq!(io[78], io[67]);
            form.inputs.verify(&verifier, nonce, &proof).unwrap();
            // Form bit, subject and exact statement digest are bound.
            for offset in [76, 77, 78] {
                let mut changed = io.clone();
                changed[offset] += Fp::ONE;
                assert!(
                    verify_io(&verifier, changed, &proof).is_err(),
                    "{} form public word {offset}",
                    form.name
                );
            }
            eprintln!(
                "ArchiveSent {} form statement opening proof bytes={} program={} sx={} sz={} cols={} io={}",
                form.name,
                proof.len(),
                blake3::Hash::from(program).to_hex(),
                verifier.0.shape.sx,
                verifier.0.shape.sz(),
                verifier.0.shape.cols(),
                verifier.0.io_len
            );
            ios.push((io, proof));
        }
        // Neither form's statement digest verifies the other form's proof.
        for (i, (io, proof)) in ios.iter().enumerate() {
            let mut crossed = io.clone();
            crossed[78] = ios[1 - i].0[78];
            assert!(verify_io(&verifier, crossed, proof).is_err());
        }
    });
}

#[test]
#[ignore = "requires signed receiver Receive and later Send originals"]
fn successor_archive_statement_private_substitution_satisfies_neither_branch() {
    with_forms(|forms, _, program| {
        let mut count = 0;
        for (i, form) in forms.iter().enumerate() {
            let authentic = &form.authentic;
            let other = forms[1 - i].authentic.statement;
            let mut cases: Vec<(&str, [Fp; 26])> =
                vec![("the other form's genuine statement", other)];
            let mut change = |label, index: usize, value: Fp| {
                let mut s = authentic.statement;
                s[index] = value;
                cases.push((label, s));
            };
            change("kind", 16, form.other_kind());
            change("subject", 17, authentic.statement[17] + Fp::ONE);
            change(
                "amount",
                form.amount(),
                authentic.statement[form.amount()] + Fp::ONE,
            );
            change("predecessor", 14, authentic.statement[14] + Fp::ONE);
            change("successor", 15, authentic.statement[15] + Fp::ONE);
            if !authentic.send {
                change("payer wallet", 18, authentic.statement[18] + Fp::ONE);
            }
            for (label, s) in cases {
                assert_ne!(s, authentic.statement);
                let mut r = authentic.clone();
                r.statement = s;
                form.check(program, label, &r, Expect::Unsatisfiable);
                count += 1;
            }
        }
        eprintln!(
            "ArchiveSent statement witness-only substitutions refused on both branches: {count}"
        );
    });
}

#[test]
#[ignore = "requires signed receiver Receive and later Send originals"]
fn successor_archive_statement_expectation_and_genuine_wrong_evidence_derive_the_noop() {
    with_forms(|forms, later, program| {
        let mut count = 0;
        for (i, form) in forms.iter().enumerate() {
            let authentic = &form.authentic;
            let other = &forms[1 - i].authentic;
            // The exact statement digest follows the substituted preimage. The signed
            // receipt and folded words stay authentic and name the original statement.
            let mut cases: Vec<(&str, [Fp; 26])> =
                vec![("the other form's genuine statement", other.statement)];
            let mut change = |label, index: usize, value: Fp| {
                let mut s = authentic.statement;
                s[index] = value;
                cases.push((label, s));
            };
            change("kind", 16, form.other_kind());
            change("subject", 17, authentic.statement[17] + Fp::ONE);
            change(
                "amount",
                form.amount(),
                authentic.statement[form.amount()] + Fp::ONE,
            );
            change("predecessor", 14, authentic.statement[14] + Fp::ONE);
            change("successor", 15, authentic.statement[15] + Fp::ONE);
            if !authentic.send {
                change("payer wallet", 18, authentic.statement[18] + Fp::ONE);
            }
            for (label, s) in cases {
                let mut r = authentic.clone();
                r.statement = s;
                r.statement_digest = statement::digest(&s);
                form.check(program, label, &r, Expect::NoOp(REJECTED));
                count += 1;
            }
            // Genuine evidence for another Payment: the later Payment's digest.
            let mut r = authentic.clone();
            r.expected[4] = later.digest();
            form.check(
                program,
                "genuine other Payment digest",
                &r,
                Expect::NoOp(REJECTED),
            );
            // The other form's complete genuine evidence under this form's bit. The
            // Receive form must name this Payment's credit, so its subject stays that.
            let mut r = other.clone();
            r.send = authentic.send;
            r.subject = authentic.subject;
            form.check(
                program,
                "the other form's complete genuine evidence",
                &r,
                Expect::NoOp(REJECTED),
            );
            count += 2;
        }
        // The payer's own genuine Send of this exact credit, signed by the payer, offered
        // as the receiver's later Send head. Kind and subject match semantically. The
        // receiver identity and signature do not, so it is refused as well.
        let form = &forms[1];
        let payment = form.inputs.delivery.payment();
        let mut r = form.authentic.clone();
        r.statement = *payment.statement();
        r.statement_digest = statement::digest(payment.statement());
        r.subject = payment.statement()[17];
        r.receipt = payment.receipt().clone();
        r.expected[5] = receipt::digest(payment.receipt());
        form.check(
            program,
            "payer's own genuine Send as the receiver head",
            &r,
            Expect::NoOp(REJECTED_UNSIGNED),
        );
        r.expected[1] = r.expected[5];
        r.expected[2] = payment.statement()[15];
        r.expected[3] = r.statement_digest;
        form.check(
            program,
            "payer's own genuine Send with its folded head words",
            &r,
            Expect::NoOp(REJECTED_UNSIGNED),
        );
        count += 2;
        eprintln!(
            "ArchiveSent statement expectation and genuine wrong-evidence substitutions: {count}"
        );
    });
}

#[test]
#[ignore = "requires signed receiver Receive and later Send originals"]
fn successor_archive_statement_semantic_predicates_and_native_refusal() {
    with_forms(|forms, _, program| {
        let mut count = 0;
        for form in forms {
            let authentic = &form.authentic;
            let receive = !authentic.send;
            let s = authentic.statement;
            let changed = |index: usize, value: Fp| {
                let mut t = s;
                t[index] = value;
                t
            };
            // (label, statement, the form's semantic predicates reject it)
            let mut cases = vec![
                ("kind", changed(16, form.other_kind()), true),
                ("subject", changed(17, s[17] + Fp::ONE), true),
                ("scheme", changed(3, s[3] + Fp::ONE), true),
                ("predecessor", changed(14, s[14] + Fp::ONE), false),
                (
                    "amount",
                    changed(form.amount(), s[form.amount()] + Fp::ONE),
                    receive,
                ),
                ("effect word 18", changed(18, s[18] + Fp::ONE), receive),
            ];
            if receive {
                cases.push(("zero-filled effect word", changed(21, Fp::ONE), true));
            }
            for (label, t, rejected) in cases {
                let r = coherent(authentic, t);
                let bits = if rejected {
                    REJECTED_UNSIGNED
                } else {
                    UNSIGNED
                };
                let label = format!("coherent unsigned head, {label}");
                // Only one removal check per form: an invalid signature already refuses it.
                if label.ends_with("kind") {
                    form.check(program, &label, &r, Expect::NoOp(bits));
                } else {
                    let noop = form.build(&form.noop, &r);
                    assert_eq!(noop.0.program_digest(), program, "{label}");
                    assert_eq!(&noop.1.io[55..59], &bits, "{} form: {label}", form.name);
                    assert!(holds(&noop), "{} form: {label}", form.name);
                }
                count += 1;
            }

            // Native derivation over the same evidence is typed before proving or Advance.
            let delivery = form.inputs.delivery;
            let evidence = delivery.receiver_evidence();
            let derive = |e: &crate::native::sent_delivery::ReceiverEvidence| {
                receiver_receipt(e, delivery.payment(), delivery.outcome())
            };
            derive(evidence).unwrap();
            let refused = |result: Result<ReceiverReceipt>, reason: &str, label: &str| match result
            {
                Err(Error::AuthorityMismatch(m)) => assert_eq!(m, reason, "{label}"),
                Err(e) => panic!("{} form {label}: untyped refusal {e:?}", form.name),
                Ok(_) => panic!("{} form {label}: accepted", form.name),
            };
            // Exact original: any changed word fails the decided statement digest.
            for (label, index) in [
                ("sequence", 9),
                ("predecessor", 14),
                ("successor", 15),
                ("subject", 17),
                ("effect word 18", 18),
                ("amount", form.amount()),
                ("last effect word", 25),
            ] {
                let mut e = evidence.clone();
                e.statement[index] += Fp::ONE;
                refused(derive(&e), "ArchiveSent receiver head evidence", label);
            }
            // Semantic match: a coherent profile and receipt for a changed statement.
            let semantic = |t: [Fp; 26]| {
                let r = coherent(authentic, t);
                let mut e = evidence.clone();
                e.statement = t;
                e.receipt = r.receipt;
                e.profile[crate::full::HEAD_IO] = t[15];
                e.profile[crate::full::STATEMENT_IO] = r.statement_digest;
                derive(&e)
            };
            refused(
                semantic(changed(3, s[3] + Fp::ONE)),
                "ArchiveSent receiver head statement",
                "scheme",
            );
            refused(
                semantic(changed(5, s[5] + Fp::ONE)),
                "ArchiveSent receiver head statement",
                "asset",
            );
            semantic(changed(14, s[14] + Fp::ONE)).unwrap();
            if receive {
                for (label, t) in [
                    ("amount", changed(20, s[20] + Fp::ONE)),
                    ("payer wallet", changed(18, s[18] + Fp::ONE)),
                    ("zero-filled effect word", changed(21, Fp::ONE)),
                ] {
                    refused(semantic(t), "ArchiveSent receiver head statement", label);
                }
            } else {
                semantic(changed(21, s[21] + Fp::ONE)).unwrap();
                semantic(changed(18, s[18] + Fp::ONE)).unwrap();
            }
        }
        eprintln!("ArchiveSent statement semantic predicates isolated: {count}");
    });
}
