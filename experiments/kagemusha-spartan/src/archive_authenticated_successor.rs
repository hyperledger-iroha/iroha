//! Private successor component for an authenticated folded CreditStatus.
//! The host verifies both recursive pairs and all their accumulators before this
//! adapter can be constructed. It issues no receipt, new wallet head or spending capability.
use super::*;
use crate::{
    fs::ProofEntropy,
    ipa::Gens,
    native::{
        self, CompletedOperation, Engine, Error, NativeEngine, PreparedOperation, Result,
        VerifiedSentDelivery, funding,
    },
    r1cs::Instance,
    spartan,
    uniform::UniformCircuit,
};
use iroha_data_model::kagemusha::KagemushaWalletIndexedRemoveV1;
use iroha_pasta::Eq as Vesta;

struct Inputs<'a> {
    delivery: &'a VerifiedSentDelivery<'a>,
    current: &'a PreparedOperation,
    released: &'a CompletedOperation,
    core_removal: &'a KagemushaWalletIndexedRemoveV1,
    adjusted_removal: &'a KagemushaWalletIndexedRemoveV1,
}

// Construct the fixed verifier independently of the proof producer. Only this
// module can create the candidate circuit accepted by verify.
struct Verifier(UniformCircuit<Fp>);

impl Verifier {
    fn new() -> Self {
        let (circuit, _, _) = Case::credited().build_with_binding(Some([Fp::ZERO; 2]));
        Self(circuit.into_verifier_program())
    }
}

impl Inputs<'_> {
    // Bind exact peer and payer originals to the standalone proof transcript. This
    // private candidate domain is separate from the ordinary monetary statement.
    fn binding(&self) -> [Fp; 2] {
        let mut h = blake3::Hasher::new_derive_key(
            "KAGEMUSHA Spartan ArchiveSent accepted pending component v1",
        );
        let delivery = self.delivery;
        let parts: [&[u8]; 7] = [
            delivery.payment().parameters_id(),
            self.current.capsule_digest(),
            delivery.operation_id(),
            delivery.open_request(),
            delivery.payment().exact_bytes(),
            delivery.credit_status(),
            self.released.completion().exact_receipt(),
        ];
        for part in parts {
            h.update(&(part.len() as u64).to_le_bytes());
            h.update(part);
        }
        funding::limbs(h.finalize().as_bytes())
    }

    fn case(&self, nonce: Fp) -> Result<Case> {
        let checked_core = self.released.released_core(self.current)?;
        let (_, rest) = self.current.successor_opening();
        if nonce.is_zero_vartime() || nonce == checked_core[NONCE] {
            return Err(Error::Malformed("archive successor nonce"));
        }
        let sequence = funding::uint(checked_core[SEQUENCE])?
            .checked_add(1)
            .ok_or(Error::Malformed("archive successor sequence"))?;
        let descriptor: [Fp; 7] = self.delivery.payment().statement()[17..24]
            .try_into()
            .unwrap();
        let key = descriptor[0].to_repr();
        let value = poseidon::hash_with_domain(PENDING_DOMAIN, &descriptor).to_repr();
        let lineage = self.released.head().profile()
            [bc::LINEAGE_IO..bc::LINEAGE_IO + bc::LINEAGE_FIELDS]
            .try_into()
            .unwrap();
        let before = State {
            core: *checked_core,
            rest: *rest,
            lineage,
        };
        // The capability checks the selected payer identity, sequence and manifest as
        // well as both adjusted openings. The core must open independently.
        let adjusted_after = self
            .delivery
            .verify_pending_removal(self.released, self.adjusted_removal)?;
        let core_path = reverse_insertion(
            &before.core[PENDING].to_repr(),
            &key,
            &value,
            self.core_removal,
        )?;
        let adjusted_path = reverse_insertion(
            &before.lineage[bc::PENDING_ROOT].to_repr(),
            &key,
            &value,
            self.adjusted_removal,
        )?;
        let mut after = before.clone();
        after.core[SEQUENCE] = Fp::from_u128(sequence);
        after.core[NONCE] = nonce;
        after.core[PENDING] = core_path.old_root;
        after.lineage[bc::PENDING_ROOT] = field(&adjusted_after)?;
        Ok(Case {
            before,
            after,
            tag: Fp::from(u64::from(KagemushaWalletOperationKindV1::ArchiveSent.tag())),
            credit: descriptor[0],
            descriptor,
            results: [Fp::ONE; 4],
            core_path,
            adjusted_path,
        })
    }

    fn prove(&self) -> Result<(Fp, Vec<u8>)> {
        let nonce =
            funding::fresh_state_nonce(Some(self.released.released_core(self.current)?[NONCE]))?;
        let (c, instance, witness) = self.case(nonce)?.build_with_binding(Some(self.binding()));
        let gens = Gens::<Vesta>::new(c.shape.cols());
        let entropy =
            ProofEntropy::fresh().map_err(|_| Error::Unavailable("archive proof entropy"))?;
        let (proof, _) = spartan::prove_circuit(
            &c.shape,
            &gens,
            &instance,
            &witness,
            &mut entropy.stream(b"archive pending effect"),
            &c,
        );
        Ok((nonce, proof.to_bytes()))
    }

    // Only this fixed candidate circuit, verified custody and the exact peer
    // originals supply the public input. No caller-provided instance or result bits.
    fn verify(&self, verifier: &Verifier, nonce: Fp, proof: &[u8]) -> Result<()> {
        let circuit = &verifier.0;
        let case = self.case(nonce)?;
        let mut io = vec![Fp::ZERO; 32];
        io[0] = Fp::ONE;
        io.extend([
            monetary_state::commitment(&case.before.core, &case.before.rest),
            monetary_state::commitment(&case.after.core, &case.after.rest),
        ]);
        io.extend(case.before.lineage);
        io.extend(case.after.lineage);
        io.extend([case.tag, case.credit]);
        io.extend(case.descriptor);
        io.extend(case.results);
        io.extend(self.binding());
        io.resize(circuit.io_len, Fp::ZERO);
        spartan::verify_circuit(
            &circuit.shape,
            &Gens::<Vesta>::new(circuit.shape.cols()),
            &Instance { io },
            proof,
            circuit,
        )
        .map_err(Error::Rejected)
    }
}

fn with_retained_inputs(
    action: impl FnOnce(
        Inputs<'_>,
        &PreparedOperation,
        &CompletedOperation,
        &NativeEngine,
        &native::AuthenticatedParameters,
        &std::path::Path,
    ),
) {
    use std::{fs, path::PathBuf};
    let fixture = |key| PathBuf::from(std::env::var_os(key).expect(key));
    let originals = fixture("KAGEMUSHA_SPARTAN_ARCHIVE_RAW_ORIGINALS");
    let wallet = fixture("KAGEMUSHA_SPARTAN_DEVICE_ROOT");
    let bundle = fixture("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS");
    let load = |name| fs::read(bundle.join(name)).unwrap();
    let params = native::device_fixture::install(
        &load("program-bundle.norito"),
        &load("parameter-manifest.norito"),
        load("parameter-signature.bin").try_into().unwrap(),
    )
    .unwrap();
    let scheme = native::device_fixture::scheme().scheme_id();
    let engine = NativeEngine::default();
    let paths: Vec<_> = fs::read_dir(originals)
        .unwrap()
        .map(|p| p.unwrap().path())
        .collect();
    let mut available = Vec::new();
    for cap_path in paths
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "cap"))
    {
        let bytes = fs::read(cap_path).unwrap();
        let cap = native::RecoveryCapsule::decode(&bytes, &scheme).unwrap();
        let cr = native::CompletionRecord::decode(
            &fs::read(cap_path.with_extension("cr")).unwrap(),
            cap.wallet(),
        )
        .unwrap();
        assert_eq!(
            cr.capsule_digest(),
            &native::RecoveryCapsule::frame_digest(&bytes)
        );
        available.push((cap, cr));
    }
    let release = |kind| {
        let (cap, cr) = available
            .iter()
            .find(|(cap, _)| cap.kind() == kind)
            .unwrap();
        engine.verify_released_operation(cap, cr, &params).unwrap()
    };
    let (send, sent) = release(3);
    let (current, later) = release(4);
    assert!(current.capsule().sequence() > send.capsule().sequence());
    let payment = engine
        .verify_payment(
            &fs::read(wallet.join("complete-payment.bin")).unwrap(),
            &params,
        )
        .unwrap();
    let requests: Vec<_> = paths
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "ipm1"))
        .map(|p| fs::read(p).unwrap())
        .collect();
    let request = requests
        .iter()
        .find(|bytes| {
            native::decode_open_request_peer(bytes)
                .is_ok_and(|r| r.request.ends_with(&payment.request().signature))
        })
        .unwrap();
    let status = fs::read(wallet.join("outgoing-credit-status.bin")).unwrap();
    let delivery = engine
        .verify_sent_delivery(&send, &sent, request, &status, &params)
        .unwrap();
    assert_eq!(delivery.payment().exact_bytes(), payment.exact_bytes());
    let different_request = requests.iter().find(|other| *other != request).unwrap();
    assert!(
        engine
            .verify_sent_delivery(&send, &sent, different_request, &status, &params)
            .is_err(),
        "a different recipient-signed archived Request cannot authorize this pending proof"
    );
    let mut changed_status = status.clone();
    *changed_status.last_mut().unwrap() ^= 1;
    assert!(
        engine
            .verify_sent_delivery(&send, &sent, request, &changed_status, &params)
            .is_err(),
        "a changed receiver proof cannot authorize this pending proof"
    );
    let descriptor = &payment.statement()[17..24];
    let key = descriptor[0].to_repr();
    let mut pending = KagemushaWalletIndexedTreeV1::new();
    pending
        .insert(
            key,
            poseidon::hash_with_domain(PENDING_DOMAIN, descriptor).to_repr(),
        )
        .unwrap();
    let removal = pending.remove(&key).unwrap();
    let later_inputs = Inputs {
        delivery: &delivery,
        current: &current,
        released: &later,
        core_removal: &removal,
        adjusted_removal: &removal,
    };
    action(later_inputs, &send, &sent, &engine, &params, &wallet);
}

#[test]
#[ignore = "requires retained inner capsule, completion and signed Request originals and the device fixture"]
fn successor_archive_authenticated_delivery_proves_at_later_selected_payer_head() {
    with_retained_inputs(|later_inputs, send, sent, _, _, _| {
        let (nonce, proof) = later_inputs.prove().unwrap();
        // Construct the verifier from a deterministic, different state fixture.
        let circuit = Verifier::new();
        later_inputs.verify(&circuit, nonce, &proof).unwrap();
        let earlier = Inputs {
            current: send,
            released: sent,
            ..later_inputs
        };
        assert!(
            earlier.verify(&circuit, nonce, &proof).is_err(),
            "proof must bind the actual payer head"
        );
        assert!(
            later_inputs
                .verify(&circuit, nonce + Fp::ONE, &proof)
                .is_err(),
            "proof must bind the successor opening"
        );
        let mut damaged = proof.clone();
        damaged[0] ^= 1;
        assert!(later_inputs.verify(&circuit, nonce, &damaged).is_err());
        let crossed = Inputs {
            current: send,
            ..later_inputs
        };
        assert!(
            crossed.prove().is_err(),
            "current preparation and completion must match"
        );
        eprintln!(
            "ArchiveSent authenticated folded delivery proof bytes={} program={}",
            proof.len(),
            blake3::Hash::from(circuit.0.program_digest()).to_hex()
        );
    });
}

fn retained_receiver_receipt(
    delivery: &VerifiedSentDelivery<'_>,
    engine: &NativeEngine,
    params: &native::AuthenticatedParameters,
    wallet: &std::path::Path,
) -> (super::receiver_signature::ReceiverReceipt, Fp) {
    use super::receiver_signature::ReceiverReceipt;
    use crate::{bootstrap::Receipt, credential, full, receipt};
    let open = native::decode_open_request_peer(delivery.open_request()).unwrap();
    let quoted = native::engine::read_credential(open.receiver_credential).unwrap();
    // The fixture's receiver supplies an independent oracle for the receipt and the
    // public folded profile. Its recovery witness is never a circuit input.
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
    let bytes = engine
        .encode_credit_status_peer(
            &cap,
            &record,
            delivery.open_request(),
            delivery.payment(),
            params,
        )
        .unwrap();
    assert_eq!(bytes, delivery.credit_status());
    let (_, receiver) = engine
        .verify_released_operation(&cap, &record, params)
        .unwrap();
    let projection = receiver.head().profile();
    let bytes = record.exact_receipt();
    let raw = Receipt {
        body: bytes[..receipt::BODY_BYTES].try_into().unwrap(),
        signature: bytes[receipt::BODY_BYTES..].try_into().unwrap(),
    };
    let expected = [
        credential::digest(&quoted),
        projection[full::RECEIPT_IO],
        projection[full::HEAD_IO],
        projection[full::STATEMENT_IO],
        delivery.payment().digest(),
        receipt::digest(&raw),
        projection[bc::CONTEXT_IO + bc::WALLET],
        projection[bc::CONTEXT_IO + bc::WALLET + 1],
        projection[bc::CONTEXT_IO + bc::PAYMENT_KEY],
        projection[bc::CONTEXT_IO + bc::PAYMENT_KEY + 1],
        projection[bc::CONTEXT_IO + bc::PAYMENT_KEY + 2],
        projection[bc::CONTEXT_IO + bc::PAYMENT_KEY + 3],
    ];
    let receipt = ReceiverReceipt {
        request: None,
        opening: None,
        credential: quoted,
        receipt: raw,
        expected,
    };
    assert_eq!(receipt::digest(&receipt.receipt), expected[1]);
    (receipt, projection[bc::LINEAGE_IO + bc::CREDIT_ROOT])
}

#[test]
#[ignore = "requires the signed sender and counterparty fixture originals"]
fn successor_archive_receiver_receipt_constrains_actual_signature_and_pending_branch() {
    use super::receiver_signature::ReceiverReceipt;
    use crate::receipt;
    with_retained_inputs(|payer, _, _, engine, params, wallet| {
        let (receipt, _) = retained_receiver_receipt(payer.delivery, engine, params, wallet);
        let nonce =
            funding::fresh_state_nonce(Some(payer.current.successor_opening().0[NONCE])).unwrap();
        let accepted = payer.case(nonce).unwrap();
        let build = |state: &Case, data: &ReceiverReceipt| {
            state.build_for_receipt(Some(payer.binding()), Some(data))
        };
        let positive = build(&accepted, &receipt);
        assert_eq!(&positive.1.io[55..59], &[Fp::ONE; 4]);
        assert_eq!(&positive.1.io[64..76], &receipt.expected);
        assert!(
            holds(&positive),
            "the actual receiver signature and receipt select removal"
        );
        let (c, i, witness) = positive;
        let program = c.program_digest();
        let gens = Gens::<Vesta>::new(c.shape.cols());
        let entropy = ProofEntropy::fresh().unwrap();
        let (p, _) = spartan::prove_circuit(
            &c.shape,
            &gens,
            &i,
            &witness,
            &mut entropy.stream(b"archive receiver receipt"),
            &c,
        );
        drop(witness);
        let v = c.into_verifier_program();
        let proof = p.to_bytes();
        spartan::verify_circuit(&v.shape, &gens, &i, &proof, &v).unwrap();
        let mut offered_wrong = i.clone();
        offered_wrong.io[68] += Fp::ONE;
        assert!(spartan::verify_circuit(&v.shape, &gens, &offered_wrong, &proof, &v).is_err());
        eprintln!(
            "ArchiveSent receiver signature proof bytes={} program={} sx={} sz={} cols={}",
            proof.len(),
            blake3::Hash::from(program).to_hex(),
            v.shape.sx,
            v.shape.sz(),
            v.shape.cols()
        );
        drop(v);
        let mut bad_signature = receipt.clone();
        bad_signature.receipt.signature = [0; 64];
        bad_signature.receipt.signature[31] = 1;
        bad_signature.receipt.signature[63] = 1;
        // Isolate the signature result even if a claimed incoming head commits to these exact bad bytes.
        bad_signature.expected[1] = receipt::digest(&bad_signature.receipt);
        let substituted_original = bad_signature.clone();
        bad_signature.expected[5] = bad_signature.expected[1];
        let mut noop = accepted.clone();
        noop.after.lineage[bc::PENDING_ROOT] = noop.before.lineage[bc::PENDING_ROOT];
        assert!(
            !holds(&build(&noop, &substituted_original)),
            "the original evidence still commits to the valid signature"
        );
        let invalid = build(&noop, &bad_signature);
        assert_eq!(invalid.0.program_digest(), program);
        assert_eq!(
            &invalid.1.io[55..59],
            &[Fp::ONE, Fp::ONE, Fp::ZERO, Fp::ONE]
        );
        assert!(
            holds(&invalid),
            "the derived false signature forces the adjusted no-op"
        );
        drop(invalid);
        assert!(
            !holds(&build(&accepted, &bad_signature)),
            "a caller's true signature flag cannot spend the pending descriptor"
        );
        let mut wrong_folded_key = receipt.clone();
        wrong_folded_key.expected[11] += Fp::ONE;
        let wrong_key = build(&noop, &wrong_folded_key);
        assert_eq!(
            &wrong_key.1.io[55..59],
            &[Fp::ONE, Fp::ZERO, Fp::ONE, Fp::ONE]
        );
        assert!(
            holds(&wrong_key),
            "a signature under the quoted key cannot authorize another folded receiver key"
        );
        drop(wrong_key);
        let mut wrong_payment = receipt;
        wrong_payment.expected[4] += Fp::ONE;
        let mismatch = build(&noop, &wrong_payment);
        assert_eq!(mismatch.0.program_digest(), program);
        assert_eq!(
            &mismatch.1.io[55..59],
            &[Fp::ONE, Fp::ZERO, Fp::ONE, Fp::ONE]
        );
        assert!(
            holds(&mismatch),
            "a valid signature on a different Payment cannot remove the descriptor"
        );
    });
}
#[test]
#[ignore = "requires a verified folded status, receiver head and retained originals"]
fn successor_archive_receiver_credit_opening_constrains_status_and_exact_branch() {
    use super::credit_opening::StatusOpening;
    with_retained_inputs(|payer, _, _, engine, params, wallet| {
        let (mut receipt, folded_root) =
            retained_receiver_receipt(payer.delivery, engine, params, wallet);
        let original = StatusOpening::from_verified(payer.delivery, folded_root).unwrap();
        assert!(!original.burned);
        receipt.opening = Some(original.clone());
        let nonce =
            funding::fresh_state_nonce(Some(payer.current.successor_opening().0[NONCE])).unwrap();
        let accepted = payer.case(nonce).unwrap();
        let build = |case: &Case, receipt: &super::receiver_signature::ReceiverReceipt| {
            case.build_for_receipt(Some(payer.binding()), Some(receipt))
        };
        let (c, i, w) = build(&accepted, &receipt);
        assert_eq!(&i.io[128..164], &original.words());
        assert_eq!(&i.io[55..59], &[Fp::ONE; 4]);
        let c_digest = c.program_digest();
        let gens = Gens::<Vesta>::new(c.shape.cols());
        let entropy = ProofEntropy::fresh().unwrap();
        let (p, _) = spartan::prove_circuit(
            &c.shape,
            &gens,
            &i,
            &w,
            &mut entropy.stream(b"archive credit path"),
            &c,
        );
        drop(w);
        let v = c.into_verifier_program();
        let proof = p.to_bytes();
        spartan::verify_circuit(&v.shape, &gens, &i, &proof, &v).unwrap();
        // The verifier supplies the folded root and every decoded field of this exact
        // peer opening. A proof for different public originals must fail.
        for (label, offset) in [("verdict", 129), ("slot", 131), ("last sibling", 163)] {
            let mut changed = i.clone();
            changed.io[offset] += Fp::ONE;
            assert!(
                spartan::verify_circuit(&v.shape, &gens, &changed, &proof, &v).is_err(),
                "{label}"
            );
        }
        eprintln!(
            "ArchiveSent credit opening proof bytes={} program={} sx={} sz={} cols={}",
            proof.len(),
            blake3::Hash::from(c_digest).to_hex(),
            v.shape.sx,
            v.shape.sz(),
            v.shape.cols()
        );
        drop(v);
        let mut noop = accepted.clone();
        noop.after.lineage[bc::PENDING_ROOT] = noop.before.lineage[bc::PENDING_ROOT];
        let changes: &[(&str, fn(&mut StatusOpening))] = &[
            ("changed folded credit root", |o| o.root += Fp::ONE),
            ("claimed Burned for the retained Credited leaf", |o| {
                o.burned = true
            }),
            ("changed indexed next key", |o| o.next += Fp::ONE),
            ("changed high slot bit", |o| o.index ^= 1 << 31),
            ("changed high sibling", |o| o.siblings[31] += Fp::ONE),
        ];
        for (label, change) in changes {
            let mut bad = receipt.clone();
            change(bad.opening.as_mut().unwrap());
            let different = build(&noop, &bad);
            assert_eq!(different.0.program_digest(), c_digest);
            assert_eq!(
                &different.1.io[55..59],
                &[Fp::ONE, Fp::ZERO, Fp::ONE, Fp::ONE],
                "{label}"
            );
            assert!(holds(&different), "{label}");
        }
        let mut bad = receipt;
        bad.opening.as_mut().unwrap().burned = true;
        assert!(
            !holds(&build(&accepted, &bad)),
            "wrong burned flag cannot retire the pending descriptor"
        );
    });
}

#[test]
#[ignore = "requires the exact signed payer Request and authorized retained release originals"]
fn successor_archive_receiver_request_signature_and_payer_specialization() {
    use super::{credit_opening::StatusOpening, request_original::expected_words};
    with_retained_inputs(|payer, _, _, engine, params, wallet| {
        let (mut receipt, root) = retained_receiver_receipt(payer.delivery, engine, params, wallet);
        receipt.opening = Some(StatusOpening::from_verified(payer.delivery, root).unwrap());
        let completed = payer.delivery.payment().request().clone();
        let open = native::decode_open_request_peer(payer.delivery.open_request()).unwrap();
        let open_words = expected_words(open.request);
        let complete_original = [completed.body.as_slice(), &completed.signature].concat();
        let completed_words = expected_words(&complete_original);
        assert_ne!(open_words, completed_words);
        receipt.request = Some(completed);
        let nonce =
            funding::fresh_state_nonce(Some(payer.current.successor_opening().0[NONCE])).unwrap();
        let case = payer.case(nonce).unwrap();
        let build = |receipt: &super::receiver_signature::ReceiverReceipt| {
            case.build_for_receipt(Some(payer.binding()), Some(receipt))
        };
        let (c, mut instance, witness) = build(&receipt);
        assert_eq!(&instance.io[80..96], &open_words);
        assert_eq!(&instance.io[96..112], &completed_words);
        // Independently rebuild the expected public tapes from payer and receiver originals.
        instance.io[80..96].copy_from_slice(&open_words);
        instance.io[96..112].copy_from_slice(&completed_words);
        assert_eq!(&instance.io[55..59], &[Fp::ONE; 4]);
        let digest = c.program_digest();
        let gens = Gens::<Vesta>::new(c.shape.cols());
        let entropy = ProofEntropy::fresh().unwrap();
        let (p, _) = spartan::prove_circuit(
            &c.shape,
            &gens,
            &instance,
            &witness,
            &mut entropy.stream(b"archive original request and receipt"),
            &c,
        );
        drop(witness);
        let verifier = c.into_verifier_program();
        let proof = p.to_bytes();
        spartan::verify_circuit(&verifier.shape, &gens, &instance, &proof, &verifier).unwrap();
        for changed in [82, 85, 95, 98, 102, 111] {
            let mut other = instance.clone();
            other.io[changed] += Fp::ONE;
            assert!(
                spartan::verify_circuit(&verifier.shape, &gens, &other, &proof, &verifier).is_err(),
                "substituted open/completed word {changed}"
            );
        }
        eprintln!(
            "ArchiveSent signed Request proof bytes={} program={} sx={} sz={} cols={}",
            proof.len(),
            blake3::Hash::from(digest).to_hex(),
            verifier.shape.sx,
            verifier.shape.sz(),
            verifier.shape.cols()
        );
        drop(verifier);
        let mut payer_swap = receipt.clone();
        // The signed receiver terms omit the writable payer area, but the original credit
        // and completed Payment bind it, so an otherwise valid quote cannot be retargeted.
        payer_swap.request.as_mut().unwrap().body[98] ^= 1;
        let replaced = build(&payer_swap);
        assert_eq!(replaced.0.program_digest(), digest);
        assert_eq!(
            replaced.1.io[57],
            Fp::ONE,
            "original receipt and quote still verify"
        );
        assert!(
            !holds(&replaced),
            "replaced payer account must fail the original pending credit"
        );
        drop(replaced);
        let mut bad_terms = receipt.clone();
        bad_terms.request.as_mut().unwrap().body[242] ^= 1;
        let changed = build(&bad_terms);
        assert_eq!(changed.0.program_digest(), digest);
        assert_eq!(
            changed.1.io[57],
            Fp::ZERO,
            "receiver signature must reject the changed amount"
        );
        assert!(
            !holds(&changed),
            "changed amount cannot match the retained descriptor"
        );
        drop(changed);
        let r = receipt.request.as_mut().unwrap();
        r.signature = [0; 64];
        r.signature[31] = 1;
        r.signature[63] = 1;
        let wrong = build(&receipt);
        assert_eq!(wrong.0.program_digest(), digest);
        assert_eq!(
            wrong.1.io[57],
            Fp::ZERO,
            "canonical but incorrect Request signature"
        );
        assert!(
            !holds(&wrong),
            "altered signed Request cannot replace the selected Send original"
        );
    });
}

#[path = "archive_later_head_successor.rs"]
mod later_receiver_head;
