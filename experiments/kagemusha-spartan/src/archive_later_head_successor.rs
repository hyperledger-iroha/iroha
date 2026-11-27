//! Native later receiver-head delivery coverage against retained signed originals.
#[path = "archive_wire_binding_successor.rs"]
pub(super) mod wire_binding;

use super::{credit_opening::StatusOpening, *};
use crate::native::engine::credit_status_wire::later_send::{WORDS, packed_fixture};
use iroha_data_model::kagemusha::{
    KagemushaWalletIndexedInsertV1 as PublicInsert, KagemushaWalletIndexedLeafV1 as PublicLeaf,
    KagemushaWalletIndexedOpeningV1 as PublicPath,
};
const MAX: usize = 10_084 - crate::payment_wire::HEADER_BYTES;
pub(super) struct FixtureRoute {
    root: [u8; 32],
    leaf: PublicLeaf,
    path: PublicPath,
}
impl native::SpartanWalletMaps for FixtureRoute {
    fn member_or_low(
        &mut self,
        map: native::SpartanWalletMap,
        root: [u8; 32],
        key: [u8; 32],
    ) -> Result<(PublicLeaf, PublicPath)> {
        if map != native::SpartanWalletMap::Credit || root != self.root || key != self.leaf.key {
            return Err(Error::AuthorityMismatch("fixture credit route"));
        }
        Ok((self.leaf, self.path))
    }
    fn insert(
        &mut self,
        _: native::SpartanWalletMap,
        _: [u8; 32],
        _: [u8; 32],
        _: [u8; 32],
    ) -> Result<PublicInsert> {
        panic!("read-only delivery must not mutate the indexed tree")
    }
}
fn reference(
    later: &native::VerifiedIncomingPayment,
    o: &StatusOpening,
    outcome: native::VerifiedCreditStatus,
    p: &native::AuthenticatedParameters,
) -> (Vec<u8>, usize, usize) {
    packed_fixture(later, o.next, o.index, o.siblings, outcome, p).unwrap()
}

// The actual later receiver Send, encoded by the native producer as CreditStatus for the
// original Payment from the receiver's credit route at that later head.
pub(super) fn later_send_status(
    payer: &Inputs<'_>,
    root: Fp,
    engine: &NativeEngine,
    parameters: &native::AuthenticatedParameters,
    wallet: &std::path::Path,
) -> (
    native::VerifiedIncomingPayment,
    StatusOpening,
    FixtureRoute,
    Vec<u8>,
) {
    let later = engine
        .verify_payment(
            &std::fs::read(wallet.join("incoming-payment.bin")).unwrap(),
            parameters,
        )
        .unwrap();
    let original = payer.delivery.payment();
    let opening = StatusOpening::from_verified(payer.delivery, root).unwrap();
    let credit = original.statement()[17];
    let leaf = PublicLeaf {
        key: credit.to_repr(),
        value: poseidon::hash_with_domain(
            crate::receive_lineage::CREDIT_DOMAIN,
            &[credit, original.digest(), Fp::ZERO],
        )
        .to_repr(),
        next_key: opening.next.to_repr(),
    };
    let path = PublicPath {
        slot: opening.index,
        siblings: opening.siblings.map(|v| v.to_repr()),
    };
    let mut maps = FixtureRoute {
        root: root.to_repr(),
        leaf,
        path,
    };
    let wire = engine
        .encode_credit_status_at_send_head(
            &later,
            original,
            payer.delivery.open_request(),
            &mut maps,
            parameters,
        )
        .unwrap();
    (later, opening, maps, wire)
}

#[test]
#[ignore = "requires exact signed Receive and Send head originals"]
fn folded_status_digest_reconstructs_wire_objects_for_both_heads() {
    use iroha_data_model::kagemusha::{
        KAGEMUSHA_WALLET_CREDIT_STATUS_DOMAIN_V1, kagemusha_wallet_poseidon_bytes_v1,
    };
    with_retained_inputs(|payer, send, sent, engine, parameters, wallet| {
        let (_, root) = retained_receiver_receipt(payer.delivery, engine, parameters, wallet);
        let (later, _, _, wire) = later_send_status(&payer, root, engine, parameters, wallet);
        let headed = engine
            .verify_sent_delivery(send, sent, payer.delivery.open_request(), &wire, parameters)
            .unwrap();
        let cap = native::RecoveryCapsule::decode(
            &std::fs::read(wallet.join("counterparty-receive-capsule.norito")).unwrap(),
            &native::device_fixture::scheme().scheme_id(),
        )
        .unwrap();
        let record = native::CompletionRecord::decode(
            &std::fs::read(wallet.join("counterparty-receive-completion.norito")).unwrap(),
            cap.wallet(),
        )
        .unwrap();
        let (_, recv) = engine
            .verify_released_operation(&cap, &record, parameters)
            .unwrap();
        let prior: native::engine::HeadFrame = native::decode(recv.head().exact_pair()).unwrap();

        let mut digests = Vec::new();
        for (delivery, raw_pair, profile, raw_receipt) in [
            (
                payer.delivery,
                prior.pair.as_slice(),
                recv.head().profile(),
                record.exact_receipt(),
            ),
            (
                &headed,
                later.exact_pair(),
                later.profile(),
                &[later.receipt().body.as_slice(), &later.receipt().signature].concat()[..],
            ),
        ] {
            let input = delivery.receiver_evidence();
            let kind = if std::ptr::eq(delivery, payer.delivery) {
                crate::full::RECEIVE
            } else {
                crate::full::SEND
            };
            let selected = &parameters.inner.programs[kind];
            assert_eq!(input.pair.kind, kind);
            assert_eq!(input.pair.ip.io.as_slice(), profile.as_slice());
            assert_eq!(input.exact_pair.as_slice(), raw_pair);
            assert_eq!(
                input.pair.cp.program_digest(),
                selected.primary.program_digest()
            );
            assert_eq!(
                input.pair.cs.program_digest(),
                selected.secondary.program_digest()
            );
            let other = &parameters.inner.programs[if kind == crate::full::RECEIVE {
                crate::full::SEND
            } else {
                crate::full::RECEIVE
            }];
            assert_ne!(
                input.pair.cp.program_digest(),
                other.primary.program_digest()
            );
            assert_ne!(
                input.pair.cs.program_digest(),
                other.secondary.program_digest()
            );
            eprintln!(
                "retained decided receiver kind={kind} primary={} secondary={} original={} B",
                blake3::Hash::from(input.pair.cp.program_digest()).to_hex(),
                blake3::Hash::from(input.pair.cs.program_digest()).to_hex(),
                raw_pair.len()
            );
            let status = delivery.wire_status().unwrap();
            let digest = delivery.status_digest().unwrap();
            assert_eq!(status.lineage.proof, raw_pair);
            assert_eq!(
                status.lineage.public.head.value,
                profile[crate::full::HEAD_IO].to_repr()
            );
            assert_eq!(status.opening.root().unwrap(), root.to_repr());
            assert_eq!(
                status.opening.credit_id,
                payer.delivery.payment().statement()[17].to_repr()
            );
            assert_eq!(
                status.opening.payment_digest,
                payer.delivery.payment().digest_bytes()
            );
            let mut transcript = Vec::from(1u16.to_le_bytes());
            transcript.extend_from_slice(&profile[crate::full::STATEMENT_IO].to_repr());
            transcript.extend_from_slice(&raw_receipt[242..274]);
            transcript.extend_from_slice(&profile[crate::full::RECEIPT_IO].to_repr());
            transcript.extend_from_slice(&status.lineage.lineage_digest());
            transcript.extend_from_slice(&status.opening.opening_digest().unwrap());
            assert_eq!(transcript.len(), 162);
            assert_eq!(
                digest,
                kagemusha_wallet_poseidon_bytes_v1(
                    KAGEMUSHA_WALLET_CREDIT_STATUS_DOMAIN_V1,
                    &transcript
                )
            );
            assert_ne!(
                digest,
                kagemusha_wallet_poseidon_bytes_v1(
                    KAGEMUSHA_WALLET_CREDIT_STATUS_DOMAIN_V1,
                    delivery.credit_status()
                )
            );
            let mut other = status;
            other.opening.payment_digest = [0; 32];
            assert!(other.credit_status_digest().is_err());
            digests.push(digest);
        }
        assert_ne!(digests[0], digests[1]);
    });
}

#[test]
#[ignore = "requires signed receiver Receive and later Send originals"]
fn successor_archive_status_at_a_later_receiver_send() {
    use crate::payment_wire;
    with_retained_inputs(|payer, send, sent, engine, parameters, wallet| {
        let original = payer.delivery.payment();
        let open = payer.delivery.open_request();
        send.verify_recovered_send_request(open).unwrap();
        assert!(send.verify_recovered_send_request(&[]).is_err());
        let competing = std::fs::read(wallet.join("competing-send-request.bin")).unwrap();
        let framed = competing.strip_prefix(b"KSO1\x03").unwrap();
        let (size, other) = framed.split_at(4);
        assert_eq!(
            u32::from_le_bytes(size.try_into().unwrap()) as usize,
            other.len()
        );
        native::decode_open_request_peer(other).unwrap();
        assert_ne!(other, open);
        assert!(send.verify_recovered_send_request(other).is_err());
        let (earlier, root) = retained_receiver_receipt(payer.delivery, engine, parameters, wallet);
        let (later, opening, mut maps, wire) =
            later_send_status(&payer, root, engine, parameters, wallet);
        assert_eq!(later.statement()[14], earlier.expected[2]);
        assert_ne!(later.statement()[15], earlier.expected[2]);
        assert_ne!(later.statement()[17], original.statement()[17]);
        assert_eq!(later.profile()[bc::LINEAGE_IO + bc::CREDIT_ROOT], root);
        let outcome = native::VerifiedCreditStatus::Credited;
        let credit = original.statement()[17];
        let (independent, compact, maximum) = reference(&later, &opening, outcome, parameters);
        assert_eq!(wire, independent);
        let verify = |wire: &[u8],
                      o: &native::VerifiedIncomingPayment,
                      req: &[u8],
                      p: &native::AuthenticatedParameters| {
            NativeEngine::default().verify_credit_status_peer(wire, req, o, p)
        };
        assert_eq!(verify(&wire, original, open, parameters).unwrap(), outcome);
        let delivery = engine
            .verify_sent_delivery(send, sent, open, &wire, parameters)
            .unwrap();
        assert_eq!(delivery.outcome(), outcome);
        assert_eq!(delivery.payment().exact_bytes(), original.exact_bytes());
        assert_eq!(delivery.credit_status(), wire);
        assert_eq!(
            delivery
                .verify_pending_removal(payer.released, payer.adjusted_removal)
                .unwrap(),
            payer
                .delivery
                .verify_pending_removal(payer.released, payer.adjusted_removal)
                .unwrap()
        );
        // ArchiveSent accepts this later Send head through the ordinary payer proof and
        // verifier. Native verification derived the later head, its signed operation id and
        // statement, and the proven credit opening. The public form bit ties them to the one
        // fixed program.
        let at_later_send = Inputs {
            delivery: &delivery,
            ..payer
        };
        let received = delivered_receipt(payer.delivery).unwrap();
        let headed = delivered_receipt(&delivery).unwrap();
        assert!(!received.send && headed.send);
        assert_eq!(received.subject, credit);
        // Independent oracle: the actual later Payment that this receiver released carries
        // the same signed Send receipt, head, statement and operation subject.
        assert_eq!(headed.subject, later.statement()[17]);
        assert_eq!(headed.receipt.body, later.receipt().body);
        assert_eq!(headed.receipt.signature, later.receipt().signature);
        assert_eq!(
            &headed.expected[1..4],
            &[
                crate::receipt::digest(later.receipt()),
                later.statement()[15],
                crate::statement::digest(later.statement()),
            ]
        );
        assert_eq!(headed.expected[5], headed.expected[1]);
        assert_eq!(&headed.receipt.body[306..], &[0u8; 32]);
        // The receiver credential, Payment digest, receiver wallet and key, completed Request
        // and credit opening are those of the Receive form. Only the head words differ.
        assert_eq!(headed.expected[0], received.expected[0]);
        assert_eq!(headed.expected[4], received.expected[4]);
        assert_eq!(&headed.expected[6..], &received.expected[6..]);
        assert!(headed.request == received.request);
        assert_eq!(
            headed.opening.as_ref().unwrap().words(),
            received.opening.as_ref().unwrap().words()
        );
        drop((received, headed));
        let verifier = Verifier::new();
        let (nonce, proof) = at_later_send.prove().unwrap();
        let (joined, instance, witness) = at_later_send.joined(nonce).unwrap();
        drop(witness);
        assert_eq!(
            joined.program_digest(),
            verifier.0.program_digest(),
            "the one fixed program verifies the later Send form"
        );
        drop(joined);
        let io = at_later_send.public_io(nonce, verifier.0.io_len).unwrap();
        assert_eq!(
            io, instance.io,
            "the verifier recomputes the exact public input"
        );
        assert_eq!(&io[55..59], &[Fp::ONE; 4]);
        assert_eq!(&io[76..78], &[Fp::ONE, later.statement()[17]]);
        at_later_send.verify(&verifier, nonce, &proof).unwrap();
        // At the same payer head and nonce the Receive form differs only in the transcript
        // binding of the exact CreditStatus bytes, the head receipt, head and statement
        // words, the form bit, the operation subject and the exact statement digest.
        // Custody, pending removal,
        // descriptor, Payment, Request and opening words are identical.
        let received_io = payer.public_io(nonce, verifier.0.io_len).unwrap();
        let differing: Vec<_> = (0..io.len()).filter(|&i| io[i] != received_io[i]).collect();
        assert_eq!(differing, [59, 60, 65, 66, 67, 69, 76, 77, 78]);
        assert!(
            payer.verify(&verifier, nonce, &proof).is_err(),
            "a later Send proof is not Receive evidence"
        );
        let (received_nonce, received_proof) = payer.prove().unwrap();
        payer
            .verify(&verifier, received_nonce, &received_proof)
            .unwrap();
        assert!(
            at_later_send
                .verify(&verifier, received_nonce, &received_proof)
                .is_err(),
            "a Receive proof is not later Send evidence"
        );
        let mut as_receive = io.clone();
        as_receive[76] = Fp::ZERO;
        assert!(verify_io(&verifier, as_receive, &proof).is_err());
        // Later receipt, head, statement and raw receipt digest, form bit, operation
        // subject, exact statement digest, credit root and Burned flag are all bound.
        for offset in [65, 66, 67, 69, 76, 77, 78, 128, 129] {
            let mut changed = io.clone();
            changed[offset] += Fp::ONE;
            assert!(
                verify_io(&verifier, changed, &proof).is_err(),
                "later Send public word {offset}"
            );
        }
        let earlier_head = Inputs {
            current: send,
            released: sent,
            ..at_later_send
        };
        assert!(
            earlier_head.verify(&verifier, nonce, &proof).is_err(),
            "proof must bind the actual payer head"
        );
        eprintln!(
            "ArchiveSent later receiver Send form proof bytes={} program={}",
            proof.len(),
            blake3::Hash::from(verifier.0.program_digest()).to_hex()
        );
        drop(verifier);
        if let Some(dir) = std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT") {
            std::fs::write(
                std::path::PathBuf::from(dir).join("later-send-credit-status.ipm1"),
                &wire,
            )
            .unwrap();
        }
        let mut dense = opening.clone();
        dense.siblings = [Fp::from(987654u64); 32];
        let (dense_wire, _, dense_max) = reference(&later, &dense, outcome, parameters);
        assert_eq!(dense_wire.len(), dense_max);
        assert_eq!(dense_max, maximum);
        assert!(maximum <= 10_084);
        assert!(
            {
                maps.path.siblings = dense.siblings.map(|v| v.to_repr());
                engine
                    .encode_credit_status_at_send_head(
                        &later, original, open, &mut maps, parameters,
                    )
                    .is_err()
            },
            "producer refuses unproved route"
        );
        let dense_payload = payment_wire::decode_peer_payload(4, &dense_wire, MAX).unwrap();
        let mut full_budget = dense_payload.to_vec();
        full_budget.resize(MAX, 0);
        assert_eq!(full_budget.len() + payment_wire::HEADER_BYTES, 10_084);
        assert!(
            verify(
                &payment_wire::encode_peer_payload(4, &full_budget),
                original,
                open,
                parameters
            )
            .is_err(),
            "trailing bytes at exact transport maximum"
        );
        assert!(
            verify(&dense_wire, original, open, parameters).is_err(),
            "dense path fits and fails the actual signed credit root"
        );
        eprintln!(
            "ArchiveSent later receiver Send raw_pair={} compact_pair={} ipm1={} dense_ipm1={} limit=10084",
            later.exact_pair().len(),
            compact,
            wire.len(),
            maximum
        );
        let raw = payment_wire::decode_peer_payload(4, &wire, MAX).unwrap();
        let fields_end = 1 + WORDS.iter().map(|&(_, n)| n).sum::<usize>();
        for (name, at) in [
            ("verdict", 0),
            ("later head", 145),
            ("later operation subject", 177),
            ("head receipt signature", fields_end + 64),
            ("index", fields_end + 128 + 40 + 32),
            ("folded proof", raw.len() - 1),
        ] {
            let mut changed = raw.to_vec();
            changed[at] ^= 1;
            assert!(
                verify(
                    &payment_wire::encode_peer_payload(4, &changed),
                    original,
                    open,
                    parameters
                )
                .is_err(),
                "{name}"
            );
        }
        let mut wrong_profile = raw.to_vec();
        wrong_profile[0] ^= 0x80;
        assert!(
            verify(
                &payment_wire::encode_peer_payload(4, &wrong_profile),
                original,
                open,
                parameters
            )
            .is_err()
        );
        assert!(verify(&wire, &later, open, parameters).is_err());
        assert_eq!(
            verify(payer.delivery.credit_status(), original, open, parameters).unwrap(),
            outcome
        );
        assert!(verify(&wire[..wire.len() - 1], original, open, parameters).is_err());
        assert!(verify(&vec![0; 10_085], original, open, parameters).is_err());
    });
}

#[test]
#[ignore = "requires signed receiver Receive and later Send originals"]
fn successor_archive_later_send_receipt_form_selects_only_its_signed_head() {
    use super::receiver_signature::ReceiverReceipt;
    with_retained_inputs(|payer, send, sent, engine, parameters, wallet| {
        let (_, root) = retained_receiver_receipt(payer.delivery, engine, parameters, wallet);
        let (later, _, _, wire) = later_send_status(&payer, root, engine, parameters, wallet);
        let open = payer.delivery.open_request();
        let delivery = engine
            .verify_sent_delivery(send, sent, open, &wire, parameters)
            .unwrap();
        let at_later_send = Inputs {
            delivery: &delivery,
            ..payer
        };
        let headed = delivered_receipt(&delivery).unwrap();
        let received = delivered_receipt(payer.delivery).unwrap();
        let credit = payer.delivery.payment().statement()[17];
        let nonce =
            funding::fresh_state_nonce(Some(payer.current.successor_opening().0[NONCE])).unwrap();
        let accepted = at_later_send.case(nonce).unwrap();
        let mut noop = accepted.clone();
        noop.after.lineage[bc::PENDING_ROOT] = noop.before.lineage[bc::PENDING_ROOT];
        let build = |case: &Case, receipt: &ReceiverReceipt| {
            case.build_for_receipt(Some(at_later_send.binding()), Some(receipt))
        };
        let positive = build(&accepted, &headed);
        let program = positive.0.program_digest();
        assert_eq!(&positive.1.io[55..59], &[Fp::ONE; 4]);
        assert_eq!(&positive.1.io[64..79], &headed.public_words());
        assert!(
            holds(&positive),
            "the exact later Send head selects removal"
        );
        drop(positive);
        assert!(
            !holds(&build(&noop, &headed)),
            "authentic later Send evidence cannot elect the no-op"
        );
        let mut receive_named = headed.clone();
        receive_named.send = false;
        assert!(
            !holds(&build(&noop, &receive_named)),
            "the Receive form names only this Payment's credit"
        );
        let rejected = [Fp::ONE, Fp::ZERO, Fp::ONE, Fp::ONE];
        let mut substitutions: Vec<(&str, ReceiverReceipt, [Fp; 4])> = Vec::new();
        let mut r = headed.clone();
        r.send = false;
        r.subject = credit;
        substitutions.push((
            "Receive form bit on the later Send head receipt",
            r,
            rejected,
        ));
        let mut r = headed.clone();
        r.subject += Fp::ONE;
        substitutions.push(("another later operation subject", r, rejected));
        // A genuine receipt signed by the same receiver key, from its Receive of this Payment.
        let mut r = headed.clone();
        r.receipt = received.receipt.clone();
        r.expected[5] = received.expected[5];
        substitutions.push((
            "Receive receipt as the later Send head receipt",
            r,
            rejected,
        ));
        let mut r = received.clone();
        r.send = true;
        r.subject = later.statement()[17];
        substitutions.push(("Send form bit on the Receive evidence", r, rejected));
        let mut r = headed.clone();
        r.expected[2] += Fp::ONE;
        substitutions.push(("another later head", r, rejected));
        let mut r = headed.clone();
        r.expected[3] += Fp::ONE;
        substitutions.push(("another later statement", r, rejected));
        // In the Send form only the credit opening binds the original Payment.
        let mut r = headed.clone();
        r.expected[4] += Fp::ONE;
        substitutions.push((
            "another Payment under the covering credit root",
            r,
            rejected,
        ));
        let mut r = headed.clone();
        r.receipt.signature = [0; 64];
        r.receipt.signature[31] = 1;
        r.receipt.signature[63] = 1;
        r.expected[1] = crate::receipt::digest(&r.receipt);
        r.expected[5] = r.expected[1];
        substitutions.push((
            "unsigned later head receipt",
            r,
            [Fp::ONE, Fp::ONE, Fp::ZERO, Fp::ONE],
        ));
        for (label, bad, bits) in &substitutions {
            let refused = build(&noop, bad);
            assert_eq!(refused.0.program_digest(), program, "{label}");
            assert_eq!(&refused.1.io[55..59], bits, "{label}");
            assert_eq!(&refused.1.io[64..79], &bad.public_words(), "{label}");
            assert!(
                holds(&refused),
                "{label}: the derived false result forces the no-op"
            );
            drop(refused);
            assert!(
                !holds(&build(&accepted, bad)),
                "{label}: cannot remove the pending descriptor"
            );
        }
        eprintln!(
            "ArchiveSent later Send receipt form: {} substitutions refused, program={}",
            substitutions.len(),
            blake3::Hash::from(program).to_hex()
        );
    });
}

#[path = "archive_receiver_statement_successor.rs"]
mod receiver_statement;
