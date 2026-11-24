//! Payer ArchiveSent custody on the retained signed originals: the production witness
//! builder, the exact codec reopen, the complete native recheck inside `Engine::prepare`,
//! the signed-selector refusal, a kind-5 capsule that cannot be selected, resumed or
//! completed, replanning on another head and new Credited evidence.
use super::{credit_opening::StatusOpening, *};
use crate::native::{ArchiveSentWitness, ExactOperationWitness, engine::ARCHIVE_SENT_UNSIGNED};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_WALLET_CREDIT_STATUS_DOMAIN_V1, KAGEMUSHA_WALLET_CREDITED_DOMAIN_V1,
    KAGEMUSHA_WALLET_CREDITED_TRANSCRIPT_BYTES_V1, KagemushaWalletIndexedInsertV1 as PublicInsert,
    KagemushaWalletIndexedLeafV1 as PublicLeaf, KagemushaWalletIndexedOpeningV1 as PublicPath,
    kagemusha_wallet_operation_id_v1, kagemusha_wallet_poseidon_bytes_v1,
};
use std::{sync::Arc, time::Instant};
use zeroize::Zeroizing;

/// Wallet-owned store fixture: the payer's pending-outgoing tree holding this Send's
/// descriptor, and optionally the receiver's credit route for the later Send evidence.
pub(super) struct Store {
    pending: KagemushaWalletIndexedTreeV1,
    staged: usize,
    credit: Option<([u8; 32], PublicLeaf, PublicPath)>,
}
impl Store {
    pub(super) fn new(pending: &KagemushaWalletIndexedTreeV1) -> Self {
        Self {
            pending: pending.clone(),
            staged: 0,
            credit: None,
        }
    }
}
impl native::SpartanWalletMaps for Store {
    fn member_or_low(
        &mut self,
        map: native::SpartanWalletMap,
        root: [u8; 32],
        key: [u8; 32],
    ) -> Result<(PublicLeaf, PublicPath)> {
        match &self.credit {
            Some((r, leaf, path))
                if map == native::SpartanWalletMap::Credit && root == *r && key == leaf.key =>
            {
                Ok((*leaf, *path))
            }
            _ => Err(Error::AuthorityMismatch("fixture credit route")),
        }
    }
    fn insert(
        &mut self,
        _: native::SpartanWalletMap,
        _: [u8; 32],
        _: [u8; 32],
        _: [u8; 32],
    ) -> Result<PublicInsert> {
        panic!("ArchiveSent and read-only evidence stage no insertion")
    }
    fn remove(
        &mut self,
        map: native::SpartanWalletMap,
        root: [u8; 32],
        key: [u8; 32],
    ) -> Result<KagemushaWalletIndexedRemoveV1> {
        if map != native::SpartanWalletMap::Pending || root != self.pending.root() {
            return Err(Error::AuthorityMismatch("fixture pending root"));
        }
        self.staged += 1;
        // Staging never changes the selected tree.
        self.pending
            .clone()
            .remove(&key)
            .map_err(|_| Error::AuthorityMismatch("fixture pending removal"))
    }
}

fn archived(w: &ExactOperationWitness) -> &ArchiveSentWitness {
    match w {
        ExactOperationWitness::ArchiveSent { step, .. } => step,
        _ => panic!("ArchiveSent witness"),
    }
}
fn gate(r: Result<PreparedOperation>) -> bool {
    matches!(r, Err(Error::AuthorityMismatch(m)) if m == ARCHIVE_SENT_UNSIGNED)
}
/// The same witness with one exact retained original replaced.
fn with_originals(
    w: &ExactOperationWitness,
    capsule: Option<&[u8]>,
    open: Option<&[u8]>,
    status: Option<&[u8]>,
) -> ExactOperationWitness {
    let ExactOperationWitness::ArchiveSent {
        step,
        send_capsule,
        send_completion,
        open_request,
        credit_status,
    } = w
    else {
        panic!("ArchiveSent witness")
    };
    ExactOperationWitness::ArchiveSent {
        step: step.clone(),
        send_capsule: capsule.map_or_else(
            || send_capsule.clone(),
            |bytes| Arc::new(Zeroizing::new(bytes.to_vec())),
        ),
        send_completion: send_completion.clone(),
        open_request: open.map_or_else(|| open_request.clone(), Into::into),
        credit_status: status.map_or_else(|| credit_status.clone(), Into::into),
    }
}
/// The single pending descriptor of the retained payer, as the wallet-owned tree.
pub(super) fn pending_tree(descriptor: &[Fp; 7]) -> KagemushaWalletIndexedTreeV1 {
    let mut pending = KagemushaWalletIndexedTreeV1::new();
    pending
        .insert(
            descriptor[0].to_repr(),
            poseidon::hash_with_domain(PENDING_DOMAIN, descriptor).to_repr(),
        )
        .unwrap();
    pending
}

#[test]
#[ignore = "requires retained signed Send, later head, Request and CreditStatus originals and the device fixture"]
fn payer_archive_sent_rechecks_retained_custody_and_refuses_without_a_signed_program() {
    with_retained_inputs(|payer, send, sent, engine, params, _| {
        let delivery = payer.delivery;
        let payment = delivery.payment();
        let send_capsule = send.capsule().encode().unwrap();
        let send_completion = sent.completion().encode().unwrap();
        let (open, status) = (delivery.open_request(), delivery.credit_status());
        let head = payer.released.head();
        let (before, rest) = payer.current.successor_opening();
        let descriptor: [Fp; 7] = payment.statement()[17..24].try_into().unwrap();
        let key = descriptor[0].to_repr();
        let pending = pending_tree(&descriptor);
        let lineage_root = head.profile()[bc::LINEAGE_IO + bc::PENDING_ROOT];
        assert_eq!(
            pending.root(),
            before[PENDING].to_repr(),
            "committed core root"
        );
        assert_eq!(
            pending.root(),
            lineage_root.to_repr(),
            "lineage-adjusted root"
        );
        let mut store = Store::new(&pending);

        // Production witness from custody, the exact peer originals and the store.
        let started = Instant::now();
        let witness = engine
            .wallet_archive_sent_witness(
                payer.current,
                head,
                &send_capsule,
                &send_completion,
                open,
                status,
                &mut store,
                params,
            )
            .unwrap();
        eprintln!(
            "ArchiveSent witness from retained custody {:.2} s",
            started.elapsed().as_secs_f64()
        );
        assert_eq!(store.staged, 2, "committed and lineage-adjusted removals");
        assert_eq!(
            store.pending.root(),
            pending.root(),
            "staging selects nothing"
        );
        let ExactOperationWitness::ArchiveSent {
            step,
            send_capsule: kept_capsule,
            send_completion: kept_completion,
            open_request: kept_open,
            credit_status: kept_status,
        } = &witness
        else {
            unreachable!()
        };
        assert_eq!(&kept_capsule[..], &send_capsule[..]);
        assert_eq!(&kept_completion[..], &send_completion[..]);
        assert_eq!(&kept_open[..], open);
        assert_eq!(&kept_status[..], status);
        let retained =
            native::CompletionRecord::decode(kept_completion, send.capsule().wallet()).unwrap();
        assert_eq!(
            retained.output_part(1).unwrap(),
            payment.exact_bytes(),
            "the exact released Payment stays retained"
        );

        // Statement, identity and successor.
        let s = &step.statement;
        let credited = s[18];
        let prefix = crate::native::engine::terms(payer.current.witness.as_ref()).1;
        assert_eq!(&s[..7], &prefix[..7], "version, relation, scheme and asset");
        assert_eq!(s[7], before[7]);
        assert_eq!(s[8], before[0]);
        assert_eq!(s[9], before[SEQUENCE] + Fp::ONE);
        assert_eq!(
            (s[12], s[13]),
            (Fp::ZERO, Fp::ZERO),
            "no Ω(pred) lineage fields"
        );
        assert_eq!(s[14], head.profile()[crate::full::HEAD_IO]);
        assert_eq!(
            s[15],
            monetary_state::commitment(&step.after_core, &step.after_rest)
        );
        assert_eq!(
            s[16],
            Fp::from(u64::from(KagemushaWalletOperationKindV1::ArchiveSent.tag()))
        );
        assert_eq!(s[17], descriptor[0]);
        assert_eq!(&s[19..], &[Fp::ZERO; 7]);
        // Independent Credited transcript and wallet wire operation identity.
        let evidence = delivery
            .wire_status()
            .unwrap()
            .credit_status_digest()
            .unwrap();
        assert_eq!(evidence, delivery.status_digest().unwrap());
        assert_ne!(
            evidence,
            kagemusha_wallet_poseidon_bytes_v1(KAGEMUSHA_WALLET_CREDIT_STATUS_DOMAIN_V1, status)
        );
        eprintln!(
            "Receive status {} credited {}",
            blake3::Hash::from(evidence).to_hex(),
            blake3::Hash::from(credited.to_repr()).to_hex()
        );
        let transcript = [
            &1u16.to_le_bytes()[..],
            &[2],
            &descriptor[0].to_repr(),
            &payment.digest().to_repr(),
            &evidence,
        ]
        .concat();
        assert_eq!(
            transcript.len(),
            KAGEMUSHA_WALLET_CREDITED_TRANSCRIPT_BYTES_V1
        );
        assert_eq!(
            credited.to_repr(),
            kagemusha_wallet_poseidon_bytes_v1(KAGEMUSHA_WALLET_CREDITED_DOMAIN_V1, &transcript)
        );
        let wallet_id: [u8; 32] = head.credential().body[66..98].try_into().unwrap();
        let operation = kagemusha_wallet_operation_id_v1(
            &wallet_id,
            KagemushaWalletOperationKindV1::ArchiveSent,
            &credited.to_repr(),
        )
        .unwrap();
        let r = &step.receipt.body;
        assert_eq!(&r[114..146], &operation);
        assert_eq!(&r[146..178], &s[14].to_repr());
        assert_eq!(&r[178..210], &s[15].to_repr());
        assert_eq!(&r[210..242], &crate::statement::digest(s).to_repr());
        assert!(
            r[242..].iter().all(|b| *b == 0),
            "no proof, capsule or Payment digest"
        );
        assert_eq!(step.receipt.signature, [0; 64], "never signed");
        // Every value and fee claim is carried; only sequence, pending root and nonce change.
        for i in 0..30 {
            if !matches!(i, SEQUENCE | PENDING | NONCE) {
                assert_eq!(step.after_core[i], before[i], "core {i}");
            }
        }
        assert_eq!(&step.after_rest, rest);
        assert_ne!(step.after_core[NONCE], before[NONCE]);
        assert_eq!(
            step.after_core[PENDING].to_repr(),
            step.core_removal
                .verify(&before[PENDING].to_repr(), &key)
                .unwrap()
        );
        let adjusted = step
            .adjusted_removal
            .verify(&lineage_root.to_repr(), &key)
            .unwrap();
        assert_eq!(
            adjusted,
            delivery
                .verify_pending_removal(payer.released, &step.adjusted_removal)
                .unwrap(),
            "the delivery's own pending check agrees"
        );

        // The joined σ binds exactly this transition for this nonce.
        let joined = Inputs {
            core_removal: &step.core_removal,
            adjusted_removal: &step.adjusted_removal,
            ..payer
        };
        let case = joined.case(step.after_core[NONCE]).unwrap();
        assert!(case.before.core == *before && case.before.rest == *rest);
        assert!(case.after.core == step.after_core && case.after.rest == step.after_rest);
        assert_eq!(
            case.after.lineage[bc::PENDING_ROOT],
            field(&adjusted).unwrap()
        );
        assert_eq!((case.tag, case.credit), (s[16], s[17]));
        assert_eq!(case.descriptor, descriptor);

        // Exact reopen after restart, then the complete recheck and the selector refusal.
        let bytes = witness.encode().unwrap();
        let reopened = ExactOperationWitness::decode(&bytes).unwrap();
        assert_eq!(reopened.encode().unwrap(), bytes, "exact reopen");
        let started = Instant::now();
        assert!(
            gate(engine.prepare(&reopened, head, params)),
            "a valid ArchiveSent reaches only the signed-selector refusal"
        );
        eprintln!(
            "ArchiveSent prepare recheck and refusal {:.2} s, witness {} B",
            started.elapsed().as_secs_f64(),
            bytes.len()
        );

        // Each changed derived word keeps its own refusal before the gate.
        let edits: [(&str, fn(&mut ArchiveSentWitness)); 5] = [
            ("balance", |w| w.after_core[9] += Fp::ONE),
            ("descriptor kept pending", |w| {
                w.after_core[PENDING] = w.before_core[PENDING]
            }),
            ("Credited digest", |w| w.statement[18] += Fp::ONE),
            ("operation identity", |w| w.receipt.body[114] ^= 1),
            ("rest", |w| w.after_rest[1] += Fp::ONE),
        ];
        for (label, edit) in edits {
            let mut w = ExactOperationWitness::decode(&bytes).unwrap();
            let ExactOperationWitness::ArchiveSent { step, .. } = &mut w else {
                unreachable!()
            };
            edit(step);
            assert!(
                matches!(
                    engine.prepare(&w, head, params),
                    Err(Error::AuthorityMismatch("ArchiveSent exact successor"))
                ),
                "{label}"
            );
        }
        let mut w = ExactOperationWitness::decode(&bytes).unwrap();
        if let ExactOperationWitness::ArchiveSent { step, .. } = &mut w {
            step.after_core[NONCE] = step.before_core[NONCE];
        }
        assert!(matches!(
            engine.prepare(&w, head, params),
            Err(Error::Unavailable("fresh state nonce"))
        ));
        assert!(
            matches!(
                engine.prepare(&reopened, sent.head(), params),
                Err(Error::AuthorityMismatch("ArchiveSent selected predecessor"))
            ),
            "the witness binds its exact predecessor head"
        );
        let receive_capsule = payer.current.capsule().encode().unwrap();
        assert!(matches!(
            engine.prepare(
                &with_originals(&reopened, Some(&receive_capsule), None, None),
                head,
                params
            ),
            Err(Error::AuthorityMismatch("ArchiveSent released Send"))
        ));
        let mut changed_status = status.to_vec();
        *changed_status.last_mut().unwrap() ^= 1;
        let refused = engine.prepare(
            &with_originals(&reopened, None, None, Some(&changed_status)),
            head,
            params,
        );
        assert!(
            refused.is_err() && !gate(refused),
            "changed Credited original"
        );
        let originals = std::path::PathBuf::from(
            std::env::var_os("KAGEMUSHA_SPARTAN_ARCHIVE_RAW_ORIGINALS").unwrap(),
        );
        let other = std::fs::read_dir(originals)
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "ipm1"))
            .map(|p| std::fs::read(p).unwrap())
            .find(|bytes| bytes != open)
            .unwrap();
        let refused = engine.prepare(
            &with_originals(&reopened, None, Some(&other), None),
            head,
            params,
        );
        assert!(
            refused.is_err() && !gate(refused),
            "another signed open Request"
        );

        // Both generations share the structural frame. Authorization at selection,
        // resume and release uses the installed producer and its signed kind count.
        let mut forged = send.capsule().clone();
        forged.frame.kind = 5;
        let frame = forged.encode().unwrap();
        let forged = native::RecoveryCapsule::decode(&frame, forged.scheme()).unwrap();
        assert!(matches!(
            engine.prepare_selection(&forged, params),
            Err(Error::AuthorityMismatch(m)) if m == ARCHIVE_SENT_UNSIGNED
        ));
        assert!(engine.resume(&forged, params).is_err());
        assert!(
            engine
                .verify_released_operation(&forged, sent.completion(), params)
                .is_err()
        );
    });
}

#[test]
#[ignore = "requires retained signed Send, later head, Request, CreditStatus and receiver originals and the device fixture"]
fn payer_archive_sent_replans_on_another_head_and_new_evidence_is_a_new_operation() {
    with_retained_inputs(|payer, send, sent, engine, params, wallet| {
        let delivery = payer.delivery;
        let payment = delivery.payment();
        let send_capsule = send.capsule().encode().unwrap();
        let send_completion = sent.completion().encode().unwrap();
        let (open, status) = (delivery.open_request(), delivery.credit_status());
        let descriptor: [Fp; 7] = payment.statement()[17..24].try_into().unwrap();
        let pending = pending_tree(&descriptor);
        let build = |prepared: &PreparedOperation,
                     released: &CompletedOperation,
                     status: &[u8],
                     store: &mut Store| {
            engine
                .wallet_archive_sent_witness(
                    prepared,
                    released.head(),
                    &send_capsule,
                    &send_completion,
                    open,
                    status,
                    store,
                    params,
                )
                .unwrap()
        };
        let current = build(
            payer.current,
            payer.released,
            status,
            &mut Store::new(&pending),
        );
        let c = archived(&current);
        let key = crate::native::wallet_archive_sent_replay_key(
            send.capsule_digest(),
            send.capsule().wallet(),
            status,
        )
        .unwrap();
        assert_eq!(current.transfer_replay_key().unwrap(), Some(key));
        // A lost unselected attempt retried on the same head: same operation, fresh nonce.
        let retry = build(
            payer.current,
            payer.released,
            status,
            &mut Store::new(&pending),
        );
        let r = archived(&retry);
        assert_eq!(&r.receipt.body[114..146], &c.receipt.body[114..146]);
        assert_eq!(r.statement[14], c.statement[14]);
        assert_ne!(r.after_core[NONCE], c.after_core[NONCE]);
        // Planned on the Send's own released head under the same Credited original: the same
        // operation identity, another predecessor and sequence.
        let earlier = build(send, sent, status, &mut Store::new(&pending));
        let e = archived(&earlier);
        assert_eq!(earlier.transfer_replay_key().unwrap(), Some(key));
        assert_eq!(&e.receipt.body[114..146], &c.receipt.body[114..146]);
        assert_eq!(
            (e.statement[17], e.statement[18]),
            (c.statement[17], c.statement[18])
        );
        assert_eq!(e.statement[14], sent.head().profile()[crate::full::HEAD_IO]);
        assert_ne!(e.statement[14], c.statement[14]);
        assert!(e.statement[9] != c.statement[9]);
        assert!(gate(engine.prepare(&earlier, sent.head(), params)));

        // New evidence: the later receiver-Send projection of the same credit.
        let later = engine
            .verify_payment(
                &std::fs::read(wallet.join("incoming-payment.bin")).unwrap(),
                params,
            )
            .unwrap();
        let (_, root) = retained_receiver_receipt(delivery, engine, params, wallet);
        let opening = StatusOpening::from_verified(delivery, root).unwrap();
        let credit = descriptor[0];
        let leaf = PublicLeaf {
            key: credit.to_repr(),
            value: poseidon::hash_with_domain(
                crate::receive_lineage::CREDIT_DOMAIN,
                &[credit, payment.digest(), Fp::ZERO],
            )
            .to_repr(),
            next_key: opening.next.to_repr(),
        };
        let path = PublicPath {
            slot: opening.index,
            siblings: opening.siblings.map(|v| v.to_repr()),
        };
        let mut routed = Store::new(&pending);
        routed.credit = Some((root.to_repr(), leaf, path));
        let later_status = engine
            .encode_credit_status_at_send_head(&later, payment, open, &mut routed, params)
            .unwrap();
        assert_ne!(later_status.as_slice(), status);
        let renewed = build(payer.current, payer.released, &later_status, &mut routed);
        let n = archived(&renewed);
        assert_ne!(renewed.transfer_replay_key().unwrap(), Some(key));
        assert_eq!(n.statement[17], c.statement[17], "the same credit");
        assert_eq!(
            n.core_removal, c.core_removal,
            "the same pending descriptor"
        );
        assert_ne!(n.statement[18], c.statement[18], "another Credited digest");
        let verified = engine
            .verify_sent_delivery(send, sent, open, &later_status, params)
            .unwrap();
        for (label, expected, chosen, step) in [
            (
                "Receive",
                "8e0ce04c1431e4eb00260e63b543f3f35f837b0bc935756eaca62ae37b092b18",
                delivery,
                c,
            ),
            (
                "later Send",
                "be6560bd3ef0fcb260d13357d93507f288d84af7b3b4571708b97f561e92a828",
                &verified,
                n,
            ),
        ] {
            let wire = chosen.wire_status().unwrap();
            let evidence = wire.credit_status_digest().unwrap();
            iroha_data_model::kagemusha::KagemushaWalletCreditedV1::from_status(wire).unwrap();
            assert_eq!(blake3::Hash::from(evidence).to_hex().as_str(), expected);
            let mut bytes = Vec::from(1u16.to_le_bytes());
            bytes.push(2);
            bytes.extend_from_slice(&descriptor[0].to_repr());
            bytes.extend_from_slice(&payment.digest().to_repr());
            bytes.extend_from_slice(&evidence);
            assert_eq!(bytes.len(), KAGEMUSHA_WALLET_CREDITED_TRANSCRIPT_BYTES_V1);
            let credited =
                kagemusha_wallet_poseidon_bytes_v1(KAGEMUSHA_WALLET_CREDITED_DOMAIN_V1, &bytes);
            let wallet_id = payer.released.head().credential().body[66..98]
                .try_into()
                .unwrap();
            let operation = kagemusha_wallet_operation_id_v1(
                &wallet_id,
                KagemushaWalletOperationKindV1::ArchiveSent,
                &credited,
            )
            .unwrap();
            assert_eq!(credited, step.statement[18].to_repr());
            assert_eq!(operation, step.receipt.body[114..146]);
            eprintln!(
                "{label} status {} credited {} operation {}",
                blake3::Hash::from(evidence).to_hex(),
                blake3::Hash::from(credited).to_hex(),
                blake3::Hash::from(operation).to_hex()
            );
        }
        assert_ne!(&n.receipt.body[114..146], &c.receipt.body[114..146]);
        assert!(gate(engine.prepare(
            &renewed,
            payer.released.head(),
            params
        )));
        eprintln!(
            "ArchiveSent operations: Receive projection {}, later Send projection {}",
            blake3::Hash::from(<[u8; 32]>::try_from(&c.receipt.body[114..146]).unwrap()).to_hex(),
            blake3::Hash::from(<[u8; 32]>::try_from(&n.receipt.body[114..146]).unwrap()).to_hex(),
        );
    });
}
