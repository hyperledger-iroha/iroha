//! Payer ArchiveSent of one delivered Send, from a selected wallet head (proposal §5.1,
//! wallet wire v1 §3.3 tag 5).
//!
//! Native rebuilds the complete step from four exact retained originals and two
//! pending-outgoing removal openings of the wallet-owned store:
//!
//! - the released Send capsule and completion, cold-verified again
//!   (`verify_released_operation`): proof pair, hardware receipt and the exact Payment;
//! - the hosted open Request and the Credited CreditStatus (`verify_sent_delivery`): the
//!   receiver head, its accumulators and the credit opening, Credited or Burned;
//! - the Payment's pending-outgoing leaf, removed under the predecessor's committed core
//!   root and under its lineage-adjusted root.
//!
//! The successor changes only the sequence, the committed pending-outgoing root and the
//! state nonce. Balance, burned total, ordinals, chains, the consumed-credit, fee-claim and
//! recovery roots, the policy fields and the rest are carried unchanged, so no value or fee
//! claim moves and the Payment bytes stay retained. The operation identity is
//! `P(kgwopid1, wallet, 5, credited)` with `credited` the Credited digest, so a retry with the
//! same Credited original is the same operation on any later head, and new evidence is a new
//! operation.
//!
//! Native rechecks the retained evidence at preparation and recovery. The authenticated
//! five-kind installation supplies the ArchiveSent program for selection and release.
//! A four-kind installation returns its typed refusal before selection.
use super::*;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_WALLET_CREDITED_DOMAIN_V1, KagemushaWalletIndexedRemoveV1,
    kagemusha_wallet_poseidon_bytes_v1,
};

#[path = "archive_relation.rs"]
pub(in crate::native::engine) mod relation;

/// Canonical ArchiveSent statement for an already selected predecessor and successor.
pub(in crate::native::engine) fn archive_statement(
    relation: [Fp; 2],
    before: &[Fp; monetary_state::CORE_FIELDS],
    after: &[Fp; monetary_state::CORE_FIELDS],
    rest: &[Fp; monetary_state::REST_FIELDS],
    credit: Fp,
    credited: Fp,
) -> [Fp; 26] {
    // No Ω(pred): zero lineage fields, the predecessor's enabled controls.
    let mut s = [Fp::ZERO; 26];
    s[0] = Fp::ONE;
    s[1..3].copy_from_slice(&relation);
    s[3..7].copy_from_slice(&before[1..5]);
    s[7] = before[7];
    s[8] = before[0];
    s[9] = after[SEQUENCE];
    s[10] = after[13];
    s[11] = before[22];
    s[14] = monetary_state::commitment(before, rest);
    s[15] = monetary_state::commitment(after, rest);
    s[16] = Fp::from(TAG);
    s[17] = credit;
    s[18] = credited;
    s
}

/// Operation kind and effect tag of ArchiveSent.
const TAG: u64 = 5;
/// Credited evidence form `Status`: a CreditStatus against a folded receiver head. Both
/// Spartan projections (the receiver's Receive head and a later receiver Send head) are
/// folded-head evidence.
const STATUS_EVIDENCE: u8 = 2;
const SEQUENCE: usize = 11;
const PENDING: usize = 18;
const NONCE: usize = 29;

/// `P_bytes(kgwcrdd1, LE16 1 ‖ u8 2 ‖ credit_id ‖ payment_digest ‖ evidence_digest)`, the
/// wallet wire's 99-byte Credited transcript. The evidence digest comes from the complete
/// decided CreditStatus object, which reconstructs the exact lineage bytes.
pub(crate) fn credited_digest(credit: Fp, payment: Fp, evidence: [u8; 32]) -> Result<Fp> {
    let mut transcript = Vec::with_capacity(99);
    transcript.extend_from_slice(&1u16.to_le_bytes());
    transcript.push(STATUS_EVIDENCE);
    transcript.extend_from_slice(&credit.to_repr());
    transcript.extend_from_slice(&payment.to_repr());
    transcript.extend_from_slice(&evidence);
    let credited = field(&kagemusha_wallet_poseidon_bytes_v1(
        KAGEMUSHA_WALLET_CREDITED_DOMAIN_V1,
        &transcript,
    ))?;
    if credited == Fp::ZERO {
        return Err(Error::Malformed("zero Credited digest"));
    }
    Ok(credited)
}

/// Credited evidence for one released Send, verified natively from its exact originals.
struct Evidence {
    payment: VerifiedIncomingPayment,
    receiver: crate::native::sent_delivery::ReceiverEvidence,
    outcome: VerifiedCreditStatus,
    send_sequence: u128,
    credit: Fp,
    descriptor: Fp,
    credited: Fp,
}

/// The successor derived from a predecessor state, nonce, evidence and removal openings.
struct Successor {
    after: [Fp; 30],
    statement: [Fp; 26],
    receipt: bootstrap::Receipt,
}

impl NativeEngine {
    fn archive_evidence(
        &self,
        wallet: &[u8],
        send_capsule: &[u8],
        send_completion: &[u8],
        open_request: &[u8],
        credit_status: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<Evidence> {
        let capsule = RecoveryCapsule::decode(send_capsule, &parameters.inner.scheme)?;
        if capsule.kind() != 3 || capsule.wallet().as_slice() != wallet {
            return Err(Error::AuthorityMismatch("ArchiveSent released Send"));
        }
        let record = CompletionRecord::decode(send_completion, capsule.wallet())?;
        let (send, sent) = self.verify_released_operation(&capsule, &record, parameters)?;
        let delivery =
            self.verify_sent_delivery(&send, &sent, open_request, credit_status, parameters)?;
        let payment = delivery.payment();
        let credit = payment.statement[17];
        let descriptor = poseidon::hash_with_domain(
            crate::send_maps::PENDING_DOMAIN,
            &payment.statement[17..24],
        );
        let credited = credited_digest(credit, payment.digest(), delivery.status_digest()?)?;
        let outcome = delivery.outcome();
        let (payment, receiver) = delivery.into_archive_parts();
        Ok(Evidence {
            receiver,
            outcome,
            send_sequence: capsule.sequence(),
            credit,
            descriptor,
            credited,
            payment,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn archive_successor(
        &self,
        before: &[Fp; 30],
        rest: &[Fp; 6],
        nonce: Fp,
        c: &Credential,
        head: &VerifiedWalletHead,
        evidence: &Evidence,
        core_removal: &KagemushaWalletIndexedRemoveV1,
        adjusted_removal: &KagemushaWalletIndexedRemoveV1,
        parameters: &AuthenticatedParameters,
    ) -> Result<Successor> {
        let payment = &evidence.payment;
        // The delivery's own pending rule: this wallet (scheme, asset, id survive a
        // credential renewal), the Send's manifest, a head at or after the Send.
        if head.parameters_id != payment.parameters_id
            || head.manifest_digest != payment.manifest_digest
            || c.body[2..98] != payment.payer.body[2..98]
            || before[7] != credential::digest(c)
            || integer(before[SEQUENCE])? < evidence.send_sequence
        {
            return Err(Error::AuthorityMismatch("ArchiveSent pending wallet"));
        }
        if nonce == Fp::ZERO || nonce == before[NONCE] {
            return Err(Error::Unavailable("fresh state nonce"));
        }
        let key = evidence.credit.to_repr();
        let value = evidence.descriptor.to_repr();
        let open = |removal: &KagemushaWalletIndexedRemoveV1,
                    root: Fp,
                    refusal: &'static str|
         -> Result<Fp> {
            if removal.leaf.value != value {
                return Err(Error::AuthorityMismatch("ArchiveSent pending descriptor"));
            }
            field(
                &removal
                    .verify(&root.to_repr(), &key)
                    .map_err(|_| Error::AuthorityMismatch(refusal))?,
            )
        };
        let committed = open(
            core_removal,
            before[PENDING],
            "ArchiveSent committed pending opening",
        )?;
        // Λ carries this lineage-adjusted root; the head commits only the core root.
        open(
            adjusted_removal,
            head.profile[bc::LINEAGE_IO + bc::PENDING_ROOT],
            "ArchiveSent lineage pending opening",
        )?;
        let mut after = *before;
        after[SEQUENCE] = add(before[SEQUENCE], 1)?;
        after[PENDING] = committed;
        after[NONCE] = nonce;
        let s = archive_statement(
            payment_wire::limbs(&parameters.inner.relation),
            before,
            &after,
            rest,
            evidence.credit,
            evidence.credited,
        );
        let mut receipt = receipt(c, &s, TAG, Fp::ZERO)?;
        // ArchiveSent's operation input is the Credited digest, not the credit id.
        receipt.body[114..146].copy_from_slice(&op_id(c, TAG, evidence.credited).to_repr());
        Ok(Successor {
            after,
            statement: s,
            receipt,
        })
    }

    /// Construct the exact private ArchiveSent of a delivered Send at the selected head.
    ///
    /// `prepared` and `head` are the selected release, as for Send and Receive. The four
    /// originals are the exact retained Send capsule and completion frames, the hosted open
    /// Request and the Credited CreditStatus. `maps` stages both removal openings without
    /// changing any selected root.
    ///
    /// # Errors
    /// Rejects a foreign scope or head, a Send of another wallet, malformed or
    /// unauthenticated originals, any refusal of the Send release or the delivery evidence,
    /// a descriptor not pending at this head, unavailable removal staging and entropy.
    #[allow(clippy::too_many_arguments)]
    pub fn wallet_archive_sent_witness(
        &self,
        prepared: &PreparedOperation,
        head: &VerifiedWalletHead,
        send_capsule: &[u8],
        send_completion: &[u8],
        open_request: &[u8],
        credit_status: &[u8],
        maps: &mut dyn SpartanWalletMaps,
        parameters: &AuthenticatedParameters,
    ) -> Result<ExactOperationWitness> {
        self.require_minimum_memory(
            parameters,
            if parameters.inner.programs.len() == 5 {
                full::ARCHIVE_SENT
            } else {
                full::SEND
            },
        )?;
        let (before, rest) = checked_source(prepared, head, parameters)?;
        let c = head.credential();
        let evidence = self.archive_evidence(
            &c.body[66..98],
            send_capsule,
            send_completion,
            open_request,
            credit_status,
            parameters,
        )?;
        let key = evidence.credit.to_repr();
        let core_removal =
            maps.remove(SpartanWalletMap::Pending, before[PENDING].to_repr(), key)?;
        let adjusted_removal = maps.remove(
            SpartanWalletMap::Pending,
            head.profile[bc::LINEAGE_IO + bc::PENDING_ROOT].to_repr(),
            key,
        )?;
        let nonce = super::super::funding::fresh_state_nonce(Some(before[NONCE]))?;
        let next = self.archive_successor(
            before,
            rest,
            nonce,
            c,
            head,
            &evidence,
            &core_removal,
            &adjusted_removal,
            parameters,
        )?;
        Ok(ExactOperationWitness::ArchiveSent {
            step: Box::new(ArchiveSentWitness {
                before_core: *before,
                before_rest: *rest,
                after_core: next.after,
                after_rest: *rest,
                credential: c.clone(),
                statement: next.statement,
                receipt: next.receipt,
                core_removal,
                adjusted_removal,
            }),
            send_capsule: Arc::new(Zeroizing::new(send_capsule.to_vec())),
            send_completion: send_completion.into(),
            open_request: open_request.into(),
            credit_status: credit_status.into(),
        })
    }

    /// Recheck an exact ArchiveSent witness against its released predecessor and its own
    /// retained originals with the builder's derivation. Every derived word must agree.
    pub(in crate::native) fn check_archive_sent(
        &self,
        witness: &ExactOperationWitness,
        head: &VerifiedWalletHead,
        parameters: &AuthenticatedParameters,
    ) -> Result<()> {
        self.checked_archive(witness, head, parameters, false)
            .map(drop)
    }

    fn checked_archive(
        &self,
        witness: &ExactOperationWitness,
        head: &VerifiedWalletHead,
        parameters: &AuthenticatedParameters,
        frozen: bool,
    ) -> Result<Evidence> {
        let ExactOperationWitness::ArchiveSent {
            step,
            send_capsule,
            send_completion,
            open_request,
            credit_status,
        } = witness
        else {
            return Err(Error::AuthorityMismatch("ArchiveSent witness"));
        };
        self.require_minimum_memory(
            parameters,
            if parameters.inner.programs.len() == 5 {
                full::ARCHIVE_SENT
            } else {
                full::SEND
            },
        )?;
        head.require_parameters(parameters)?;
        let c = &step.credential;
        let (before, rest) = (&step.before_core, &step.before_rest);
        // A released recursive predecessor whose custody is bound to its capsule.
        if head.enrollment_marker.is_some()
            || head.pair.is_empty()
            || head.custody_digest == [0; 32]
            || !exact_credential(c, &head.credential)
            || head.profile[full::HEAD_IO] != monetary_state::commitment(before, rest)
            || head.profile[bc::LINEAGE_IO + bc::LIFECYCLE] != before[0]
        {
            return Err(Error::AuthorityMismatch("ArchiveSent selected predecessor"));
        }
        let nonce = step.after_core[NONCE];
        if nonce == Fp::ZERO || nonce == before[NONCE] {
            return Err(Error::Unavailable("fresh state nonce"));
        }
        authenticate_credential(&parameters.inner, c, &head.certificate)?;
        let evidence = self.archive_evidence(
            &c.body[66..98],
            send_capsule,
            send_completion,
            open_request,
            credit_status,
            parameters,
        )?;
        let next = self.archive_successor(
            before,
            rest,
            nonce,
            c,
            head,
            &evidence,
            &step.core_removal,
            &step.adjusted_removal,
            parameters,
        )?;
        if next.after != step.after_core
            || step.after_rest != *rest
            || next.statement != step.statement
            || next.receipt.body[..242] != step.receipt.body[..242]
            || next.receipt.body[306..] != step.receipt.body[306..]
            || (!frozen && next.receipt.body[242..306] != step.receipt.body[242..306])
            || step.receipt.signature != [0; 64]
        {
            return Err(Error::AuthorityMismatch("ArchiveSent exact successor"));
        }
        Ok(evidence)
    }

    pub(in crate::native::engine) fn archive_proving_inputs(
        &self,
        witness: &ExactOperationWitness,
        head: &VerifiedWalletHead,
        parameters: &AuthenticatedParameters,
    ) -> Result<Box<PreparedArchive>> {
        let evidence = self.checked_archive(witness, head, parameters, true)?;
        let receiver = evidence.receiver;
        let payment = evidence.payment;
        let receipt = relation::receiver_receipt_fields(
            &receiver,
            payment.statement(),
            payment.request(),
            payment.digest(),
            evidence.outcome,
        )?;
        Ok(Box::new(PreparedArchive {
            pair: receiver.pair,
            pair_bytes: receiver.exact_pair,
            receipt,
            descriptor: payment.statement()[17..24]
                .try_into()
                .expect("seven descriptor words"),
        }))
    }
}

pub(in crate::native) struct PreparedArchive {
    pub(in crate::native::engine) pair: full::UniformPair,
    pub(in crate::native::engine) pair_bytes: Vec<u8>,
    receipt: relation::receiver_signature::ReceiverReceipt,
    descriptor: [Fp; 7],
}

impl PreparedArchive {
    pub(in crate::native::engine) fn primary(
        &self,
        step: &ArchiveSentWitness,
        head: &VerifiedWalletHead,
        recursive: &full::RecursiveInputs<'_>,
        folds: &full::GeneratorFolds,
    ) -> Result<full::Wiring<Fp>> {
        let before = relation::State {
            core: step.before_core,
            rest: step.before_rest,
            lineage: head.profile[bc::LINEAGE_IO..bc::LINEAGE_IO + bc::LINEAGE_FIELDS]
                .try_into()
                .unwrap(),
        };
        let key = self.descriptor[0].to_repr();
        let value = poseidon::hash_with_domain(crate::send_maps::PENDING_DOMAIN, &self.descriptor)
            .to_repr();
        let core_path = relation::reverse_insertion(
            &before.core[PENDING].to_repr(),
            &key,
            &value,
            &step.core_removal,
        )?;
        let adjusted_path = relation::reverse_insertion(
            &before.lineage[bc::PENDING_ROOT].to_repr(),
            &key,
            &value,
            &step.adjusted_removal,
        )?;
        let mut after = relation::State {
            core: step.after_core,
            rest: step.after_rest,
            lineage: before.lineage,
        };
        after.lineage[bc::PENDING_ROOT] = adjusted_path.old_root;
        let case = relation::Case {
            before,
            after,
            tag: Fp::from(TAG),
            credit: self.descriptor[0],
            descriptor: self.descriptor,
            results: [Fp::ONE; 4],
            core_path,
            adjusted_path,
        };
        let relation = relation::Composed {
            case: &case,
            binding: [Fp::ZERO; 2],
            receiver: &self.receipt,
            derived: Default::default(),
            projection: relation::Projection::Completed {
                credential: &step.credential,
                receipt: &step.receipt,
            },
        };
        let (w, _) = recursive
            .primary(full::OperationWitness::ArchiveSent(&relation), folds)
            .map_err(Error::Malformed)?;
        if relation.derived.into_inner() != Some([Fp::ONE; 4]) {
            return Err(Error::AuthorityMismatch(
                "ArchiveSent receiver evidence decision",
            ));
        }
        Ok(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::kagemusha::{
        KagemushaWalletIndexedTreeV1, KagemushaWalletOperationKindV1,
        kagemusha_wallet_operation_id_v1,
    };

    fn synthetic() -> ExactOperationWitness {
        let send = crate::send::tests::fixture(0, false).input;
        let mut tree = KagemushaWalletIndexedTreeV1::new();
        for k in [3u64, 9, 27] {
            tree.insert(Fp::from(k).to_repr(), Fp::from(k + 100).to_repr())
                .unwrap();
        }
        let core_removal = tree.clone().remove(&Fp::from(9).to_repr()).unwrap();
        let adjusted_removal = tree.remove(&Fp::from(27).to_repr()).unwrap();
        ExactOperationWitness::ArchiveSent {
            step: Box::new(ArchiveSentWitness {
                before_core: send.before_core,
                before_rest: send.before_rest,
                after_core: send.after_core,
                after_rest: send.after_rest,
                credential: send.credential.clone(),
                statement: send.statement,
                receipt: send.receipt.clone(),
                core_removal,
                adjusted_removal,
            }),
            send_capsule: Arc::new(Zeroizing::new(vec![1; 300])),
            send_completion: vec![2; 200].into(),
            open_request: vec![3; 100].into(),
            credit_status: vec![4; 50].into(),
        }
    }

    #[test]
    fn archive_sent_witness_codec_is_exact_and_canonical() {
        let witness = synthetic();
        let bytes = witness.encode().unwrap();
        let decoded = ExactOperationWitness::decode(&bytes).unwrap();
        assert_eq!(decoded.encode().unwrap(), bytes, "exact reopen");
        let (
            ExactOperationWitness::ArchiveSent {
                step: a,
                send_capsule: a1,
                send_completion: a2,
                open_request: a3,
                credit_status: a4,
            },
            ExactOperationWitness::ArchiveSent {
                step: b,
                send_capsule: b1,
                send_completion: b2,
                open_request: b3,
                credit_status: b4,
            },
        ) = (&witness, &decoded)
        else {
            panic!("ArchiveSent kind survives the frame");
        };
        assert_eq!(
            (a1, a2, a3, a4),
            (b1, b2, b3, b4),
            "originals byte for byte"
        );
        let cloned = witness.clone();
        let ExactOperationWitness::ArchiveSent { send_capsule, .. } = &cloned else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(a1, send_capsule));
        assert!(
            !Arc::ptr_eq(a1, b1),
            "decoding owns its own private capsule"
        );
        drop(cloned);
        assert_eq!(a1.as_slice(), b1.as_slice());
        assert_eq!(a.core_removal, b.core_removal);
        assert_eq!(a.adjusted_removal, b.adjusted_removal);
        assert_eq!(a.after_core, b.after_core);
        assert_eq!(a.statement, b.statement);
        assert_eq!(a.receipt.body, b.receipt.body);
        // A truncated or extended frame never decodes.
        assert!(ExactOperationWitness::decode(&bytes[..bytes.len() - 1]).is_err());
        let mut extended = bytes.clone();
        extended.push(0);
        assert!(ExactOperationWitness::decode(&extended).is_err());
    }

    #[test]
    fn archive_sent_has_no_program_in_the_signed_generation() {
        let witness = synthetic();
        assert!(matches!(
            super::super::super::kind(&witness, 4),
            Err(Error::AuthorityMismatch(m)) if m == ARCHIVE_SENT_UNSIGNED
        ));
        assert_eq!(
            super::super::super::kind(&witness, 5).unwrap(),
            full::ARCHIVE_SENT
        );
        // No other kind changes its program.
        let send =
            ExactOperationWitness::Send(Box::new(crate::send::tests::fixture(0, false).input));
        assert_eq!(super::super::super::kind(&send, 4).unwrap(), full::SEND);
    }

    #[test]
    fn credited_digest_binds_credit_payment_and_the_status_digest() {
        let original = [7u8; 32];
        let base = credited_digest(Fp::from(11), Fp::from(13), original).unwrap();
        assert_ne!(
            credited_digest(Fp::from(12), Fp::from(13), original).unwrap(),
            base
        );
        assert_ne!(
            credited_digest(Fp::from(11), Fp::from(14), original).unwrap(),
            base
        );
        for i in [0, 15, 31] {
            let mut changed = original.clone();
            changed[i] ^= 1;
            assert_ne!(
                credited_digest(Fp::from(11), Fp::from(13), changed).unwrap(),
                base,
                "{i}"
            );
        }
        // The wallet wire's operation identity for the ArchiveSent effect input `credited`.
        let mut credential = crate::send::tests::fixture(0, false).input.credential;
        credential.body[66..98].copy_from_slice(&[0x5a; 32]);
        let expected = kagemusha_wallet_operation_id_v1(
            &[0x5a; 32],
            KagemushaWalletOperationKindV1::ArchiveSent,
            &base.to_repr(),
        )
        .unwrap();
        assert_eq!(op_id(&credential, TAG, base).to_repr(), expected);
    }
}
