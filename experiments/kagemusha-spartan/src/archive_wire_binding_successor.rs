//! ArchiveSent engineering component for the three wallet-wire evidence digests.
//! The surrounding recursive verifier owns the receiver statement, proof, receipt and
//! exact lineage digests. The component binds their values and the very same opening
//! cells used by membership, including on a false membership branch.
use super::*;
use crate::certificate::field_modulus;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_WALLET_CREDIT_OPENING_DOMAIN_V1, KAGEMUSHA_WALLET_CREDIT_OPENING_TRANSCRIPT_BYTES_V1,
    KAGEMUSHA_WALLET_CREDIT_STATUS_DOMAIN_V1, KAGEMUSHA_WALLET_CREDIT_STATUS_TRANSCRIPT_BYTES_V1,
    KAGEMUSHA_WALLET_CREDITED_DOMAIN_V1, KAGEMUSHA_WALLET_CREDITED_TRANSCRIPT_BYTES_V1,
    KAGEMUSHA_WALLET_PACKED_CHUNK_BYTES_V1, KagemushaWalletCreditOpeningV1,
    kagemusha_wallet_poseidon_bytes_v1,
};
use num_bigint::BigUint;

/// A fixed transcript. Each byte comes from a constant or an existing constraint cell.
pub(crate) struct Transcript {
    cells: Cells,
    indices: Vec<usize>,
    raw: Vec<u8>,
}

impl Transcript {
    pub(crate) fn new(family: usize) -> Self {
        Self {
            cells: Cells::new(family),
            indices: Vec::new(),
            raw: Vec::new(),
        }
    }

    pub(crate) fn fixed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            let k = self.cells.byte(byte);
            self.cells
                .eq(own(k, 1), constant(vec![Fp::from(u64::from(byte))]));
            self.indices.push(k);
            self.raw.push(byte);
        }
    }

    pub(crate) fn integer(&mut self, w: &Wiring<Fp>, src: Source, width: usize) {
        let repr = src.value(w).to_repr();
        let bytes = &repr[..width];
        let indices: Vec<_> = bytes.iter().map(|&byte| self.cells.byte(byte)).collect();
        self.cells.eq(self.cells.bytes(&indices), src.linear());
        if width == 32 {
            // Equality in Fp alone also admits the integer x + p.
            self.cells.at_most(
                &indices,
                &BigUint::from_bytes_le(bytes),
                &(field_modulus() - 1u32),
            );
        }
        self.indices.extend(indices);
        self.raw.extend_from_slice(bytes);
    }

    pub(crate) fn big_endian_limb(&mut self, w: &Wiring<Fp>, src: Source) {
        let at = self.indices.len();
        self.integer(w, src, 16);
        self.indices[at..].reverse();
        self.raw[at..].reverse();
    }

    fn digest(self, w: &mut Wiring<Fp>, family: usize, domain: u64, expected: usize) -> Source {
        self.digest_with_tail(w, family, domain, expected, None)
    }

    /// Append the recursive verifier's exact compressed proof bytes, which already
    /// have a byte range check. A 31-byte chunk may straddle the Ω header and proof.
    pub(crate) fn digest_with_tail(
        mut self,
        w: &mut Wiring<Fp>,
        family: usize,
        domain: u64,
        expected_header: usize,
        tail: Option<&crate::proof_bytes::SharedBytes>,
    ) -> Source {
        assert_eq!(self.raw.len(), expected_header);
        if let Some(tail) = tail {
            for i in 0..tail.len {
                self.raw
                    .push(w.arrays[tail.array].values[0][i].to_repr()[0]);
            }
        }
        let length = Fp::from(self.raw.len() as u64);
        let mut words = vec![(length, constant(vec![length]))];
        for (chunk, raw) in self
            .raw
            .chunks(KAGEMUSHA_WALLET_PACKED_CHUNK_BYTES_V1)
            .enumerate()
        {
            let start = chunk * KAGEMUSHA_WALLET_PACKED_CHUNK_BYTES_V1;
            let mut word = Vec::new();
            for j in 0..raw.len() {
                let i = start + j;
                let byte = if let Some(&k) = self.indices.get(i) {
                    own(k, 1)
                } else {
                    let value = tail.expect("exact proof byte").byte(i - expected_header);
                    Source {
                        array: value.array,
                        cell: value.cell,
                        instance: value.instance,
                    }
                    .linear()
                };
                word = plus(
                    &word,
                    &scaled(&byte, Fp::from(256u64).pow_vartime([j as u64])),
                );
            }
            words.push((certificate::fp(&BigUint::from_bytes_le(raw)), word));
        }
        let hashed = field(&kagemusha_wallet_poseidon_bytes_v1(domain, &self.raw)).unwrap();
        let output = [
            self.cells.cell(Fp::ZERO, 255),
            self.cells.cell(hashed, 255),
            self.cells.cell(Fp::ZERO, 255),
        ];
        let local = w.push(self.cells.a);
        certificate::hash_relation(
            w,
            family,
            local,
            domain,
            &words,
            output,
            &mut SpikeRng::new(b"ArchiveSent wallet wire transcript"),
        );
        source(local, output[1])
    }
}

/// Uses the authenticated statement, sigma proof, receipt and exact lineage digests
/// in wallet-wire order. These four incoming ports require their original verifiers
/// when composing the complete fifth operation.
fn evidence(
    w: &mut Wiring<Fp>,
    family: usize,
    credit: Source,
    payment: Source,
    receiver: [Source; 4],
    opening: &StatusOpening,
    other_evidence: Source,
) -> [Source; 5] {
    let (member, o) = opening.append(w, family, credit, payment, other_evidence);
    let [opening_digest, status, credited] = status_chain(w, family, credit, payment, receiver, &o);
    [o[0], opening_digest, status, credited, member]
}

/// Commit exactly the cells already consumed by the membership predicate.
pub(crate) fn status_chain(
    w: &mut Wiring<Fp>,
    family: usize,
    credit: Source,
    payment: Source,
    receiver: [Source; 4],
    o: &[Source],
) -> [Source; 3] {
    let [_, burned, next, slot, siblings @ ..] = o else {
        unreachable!()
    };
    assert_eq!(siblings.len(), 32);
    let mut t = Transcript::new(family);
    t.integer(w, credit, 32);
    t.integer(w, payment, 32);
    t.integer(w, *burned, 1);
    t.integer(w, *next, 32);
    t.integer(w, *slot, 4);
    for &sibling in siblings {
        t.integer(w, sibling, 32);
    }
    let opening_digest = t.digest(
        w,
        family,
        KAGEMUSHA_WALLET_CREDIT_OPENING_DOMAIN_V1,
        KAGEMUSHA_WALLET_CREDIT_OPENING_TRANSCRIPT_BYTES_V1,
    );
    let mut t = Transcript::new(family);
    t.fixed(&1u16.to_le_bytes());
    for digest in receiver.into_iter().chain([opening_digest]) {
        t.integer(w, digest, 32);
    }
    let status = t.digest(
        w,
        family,
        KAGEMUSHA_WALLET_CREDIT_STATUS_DOMAIN_V1,
        KAGEMUSHA_WALLET_CREDIT_STATUS_TRANSCRIPT_BYTES_V1,
    );
    let mut t = Transcript::new(family);
    t.fixed(&[1, 0, 2]); // LE16(version) followed by the folded Status evidence tag.
    for digest in [credit, payment, status] {
        t.integer(w, digest, 32);
    }
    let credited = t.digest(
        w,
        family,
        KAGEMUSHA_WALLET_CREDITED_DOMAIN_V1,
        KAGEMUSHA_WALLET_CREDITED_TRANSCRIPT_BYTES_V1,
    );
    [opening_digest, status, credited]
}

fn build(credit: Fp, payment: Fp, receiver: [Fp; 4], opening: &StatusOpening) -> Built {
    let mut w = Wiring::default();
    let family = w.family(0, 1);
    let mut c = Cells::new(family);
    let inputs = [credit, payment]
        .into_iter()
        .chain(receiver)
        .map(|v| c.cell(v, 255))
        .collect::<Vec<_>>();
    let valid = c.cell(Fp::ONE, 1);
    c.eq(own(valid, 1), constant(vec![Fp::ONE]));
    let local = w.push(c.a);
    let values = evidence(
        &mut w,
        family,
        source(local, inputs[0]),
        source(local, inputs[1]),
        std::array::from_fn(|i| source(local, inputs[2 + i])),
        opening,
        source(local, valid),
    );
    // These are component ports, not an assigned fifth FoldProof profile.
    expose(&mut w, family, 32, &values);
    let range = w.push(Array {
        name: "ArchiveSent opening byte range",
        family,
        rho: 11,
        r_count: 2048,
        fixed: true,
        lookups: vec![],
        widths: vec![11],
        rows: vec![],
        values: vec![(0..2048).map(Fp::from).collect()],
    });
    w.tables.push((range, crate::nonnative::RANGE_TABLE));
    let (arrays, pi, counts, tables, copies) = w.finish();
    UniformCircuit::build_with_copies(
        "ArchiveSent wire evidence",
        arrays,
        pi,
        counts,
        tables,
        copies,
        12,
        64,
        b"ArchiveSent wire evidence",
    )
}

fn native_credited(credit: Fp, payment: Fp, status: Fp) -> Fp {
    let mut bytes = vec![1, 0, 2];
    for v in [credit, payment, status] {
        bytes.extend(v.to_repr());
    }
    field(&kagemusha_wallet_poseidon_bytes_v1(
        KAGEMUSHA_WALLET_CREDITED_DOMAIN_V1,
        &bytes,
    ))
    .unwrap()
}

fn proof_with_refusals(built: Built, changes: &[(usize, Fp)]) -> [u8; 32] {
    let (circuit, instance, witness) = built;
    let gens = Gens::<Vesta>::new(circuit.shape.cols());
    let entropy = ProofEntropy::fresh().unwrap();
    let (proof, _) = spartan::prove_circuit(
        &circuit.shape,
        &gens,
        &instance,
        &witness,
        &mut entropy.stream(b"ArchiveSent compact wire evidence proof"),
        &circuit,
    );
    drop(witness);
    let proof = proof.to_bytes();
    let verifier = circuit.into_verifier_program();
    let verify =
        |i: &Instance<Fp>| spartan::verify_circuit(&verifier.shape, &gens, i, &proof, &verifier);
    verify(&instance).unwrap();
    for &(offset, replacement) in changes {
        assert_ne!(instance.io[offset], replacement);
        let mut replaced = instance.clone();
        replaced.io[offset] = replacement;
        assert!(verify(&replaced).is_err());
    }
    let digest = verifier.program_digest();
    eprintln!(
        "ArchiveSent wire evidence ordinary={} proof_bytes={} program={} original_refusals={}",
        instance.io.len(),
        proof.len(),
        blake3::Hash::from(digest),
        changes.len()
    );
    digest
}

fn changed_originals(wire: &KagemushaWalletCreditOpeningV1, receiver: [Fp; 4]) -> Vec<(usize, Fp)> {
    let credited = |wire: &KagemushaWalletCreditOpeningV1, receiver: [Fp; 4]| {
        let mut bytes = Vec::from(1u16.to_le_bytes());
        for digest in receiver {
            bytes.extend(digest.to_repr());
        }
        bytes.extend(wire.opening_digest().unwrap());
        let status = field(&kagemusha_wallet_poseidon_bytes_v1(
            KAGEMUSHA_WALLET_CREDIT_STATUS_DOMAIN_V1,
            &bytes,
        ))
        .unwrap();
        native_credited(
            field(&wire.credit_id).unwrap(),
            field(&wire.payment_digest).unwrap(),
            status,
        )
    };
    let change: [fn(&mut KagemushaWalletCreditOpeningV1); 7] = [
        |o| o.credit_id = Fp::from(6).to_repr(),
        |o| o.payment_digest = Fp::from(9).to_repr(),
        |o| o.burned = !o.burned,
        |o| o.next_key = Fp::from(14).to_repr(),
        |o| o.slot ^= 1,
        |o| o.siblings[..32].copy_from_slice(&Fp::from(77).to_repr()),
        |o| o.siblings[31 * 32..].copy_from_slice(&Fp::from(78).to_repr()),
    ];
    let mut alternatives = vec![(36, Fp::ONE)]; // Refused membership cannot retire the descriptor.
    for modify in change {
        let mut other = wire.clone();
        modify(&mut other);
        alternatives.push((35, credited(&other, receiver)));
    }
    for index in 0..receiver.len() {
        let mut other = receiver;
        other[index] += Fp::ONE;
        alternatives.push((35, credited(wire, other)));
    }
    alternatives
}

#[test]
fn opening_identity_is_required_even_when_membership_is_false() {
    let credit = Fp::from(5);
    let payment = -Fp::ONE;
    let receiver = [Fp::from(17), Fp::from(18), Fp::from(19), Fp::from(20)];
    let siblings: [Fp; 32] = std::array::from_fn(|i| Fp::from((i + 3) as u64));
    let wire = KagemushaWalletCreditOpeningV1 {
        credit_id: credit.to_repr(),
        payment_digest: payment.to_repr(),
        burned: true,
        next_key: Fp::from(9).to_repr(),
        slot: 1 << 31,
        siblings: siblings.iter().flat_map(|v| v.to_repr()).collect(),
    };
    let mut opening = StatusOpening {
        root: field(&wire.root().unwrap()).unwrap(),
        burned: wire.burned,
        next: field(&wire.next_key).unwrap(),
        index: wire.slot,
        siblings,
    };
    let accepted = build(credit, payment, receiver, &opening);
    assert_eq!(accepted.0.io_len, 64);
    let accepted_io = accepted.1.io.clone();
    assert_eq!(accepted_io[33].to_repr(), wire.opening_digest().unwrap());
    assert_eq!(accepted_io[36], Fp::ONE);
    assert!(holds(&accepted));
    drop(accepted);
    opening.root += Fp::ONE;
    let mut refused = build(credit, payment, receiver, &opening);
    assert_eq!(&refused.1.io[33..36], &accepted_io[33..36]);
    assert_eq!(refused.1.io[36], Fp::ZERO);
    assert!(holds(&refused));
    refused.1.io[36] = Fp::ONE;
    assert!(!holds(&refused));
    refused.1.io[36] = Fp::ZERO;
    proof_with_refusals(refused, &changed_originals(&wire, receiver));
    let mut changed = opening.clone();
    changed.siblings[31] += Fp::ONE;
    let mut other = build(credit, payment, receiver, &changed);
    assert_eq!(other.1.io[36], Fp::ZERO);
    assert_ne!(other.1.io[35], accepted_io[35]);
    assert!(holds(&other));
    other.1.io[35] = accepted_io[35];
    assert!(
        !holds(&other),
        "a false opening must still be the exact committed original"
    );
}

#[test]
#[ignore = "requires both authentic signed receiver head fixtures"]
fn wire_identity_matches_both_verified_receiver_heads() {
    with_retained_inputs(|payer, send, sent, engine, params, wallet| {
        let (_, root) = retained_receiver_receipt(payer.delivery, engine, params, wallet);
        let (_, _, _, later) = later_send_status(&payer, root, engine, params, wallet);
        let later = engine
            .verify_sent_delivery(send, sent, payer.delivery.open_request(), &later, params)
            .unwrap();
        let mut statuses = Vec::new();
        let mut programs = Vec::new();
        for delivered in [payer.delivery, &later] {
            let status = delivered.wire_status().unwrap();
            let credit = delivered.payment().statement()[17];
            let payment = delivered.payment().digest();
            let receiver = [
                field(&status.statement.statement_digest().unwrap()).unwrap(),
                field(&status.proof_digest).unwrap(),
                crate::receipt::digest(&delivered.receiver_evidence().receipt),
                field(&status.lineage.lineage_digest()).unwrap(),
            ];
            let receipt = delivered_receipt(delivered).unwrap();
            let built = build(credit, payment, receiver, receipt.opening.as_ref().unwrap());
            let digest = field(&delivered.status_digest().unwrap()).unwrap();
            assert_eq!(
                &built.1.io[32..37],
                &[
                    root,
                    field(&status.opening.opening_digest().unwrap()).unwrap(),
                    digest,
                    native_credited(credit, payment, digest),
                    Fp::ONE
                ]
            );
            assert!(holds(&built));
            programs.push(proof_with_refusals(built, &[(36, Fp::ZERO)]));
            statuses.push(digest);
        }
        assert_ne!(statuses[0], statuses[1]);
        assert_eq!(programs[0], programs[1]);
    });
}
