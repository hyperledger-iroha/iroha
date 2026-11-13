//! Total original Request decoding for Receive's derived incoming verdict.
//!
//! Every original byte, parsed word and hash is hard bound. Semantic failures
//! produce an iff validity bit rather than an arbitrary failed-witness claim.
//! This fixed-size tape decoder does not supply envelope length provenance or
//! verify the receiver signature; those mandatory owners consume its exact ports.

use crate::{
    certificate::{self, Cells, field_modulus, fp},
    fs::SpikeRng,
    full::Wiring,
    request::{self, CompletedRequest, RequestCells, word},
    send_arithmetic::Source,
    uniform::{Lin, constant, own, plus, scaled},
};
use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};
use num_bigint::BigUint;

pub struct OriginalRequest {
    request: RequestCells,
    valid: Source,
}
impl OriginalRequest {
    pub fn request(&self) -> &RequestCells {
        &self.request
    }
    pub fn valid(&self) -> Source {
        self.valid
    }
    /// Exact original body/signature byte, before safe field decoding.
    pub fn byte(&self, index: usize) -> Source {
        assert!(index < request::BODY_BYTES + request::SIGNATURE_BYTES);
        Source {
            array: self.request.local,
            cell: index,
            instance: 0,
        }
    }
}
fn zero(c: &mut Cells, e: Lin<Fp>, v: Fp) -> usize {
    let z = c.bit(v == Fp::ZERO);
    let inverse = c.cell(Option::<Fp>::from(v.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows.push([e.clone(), own(z, 1), vec![]]);
    c.a.rows.push([
        e,
        own(inverse, 1),
        plus(&constant(vec![Fp::ONE]), &scaled(&own(z, 1), -Fp::ONE)),
    ]);
    z
}
fn not(c: &mut Cells, k: usize) -> usize {
    let n = c.cell(Fp::ONE - c.a.values[k][0], 1);
    c.eq(
        own(n, 1),
        plus(&constant(vec![Fp::ONE]), &scaled(&own(k, 1), -Fp::ONE)),
    );
    n
}
fn mul(c: &mut Cells, a: usize, b: usize) -> usize {
    let k = c.cell(c.a.values[a][0] * c.a.values[b][0], 255);
    c.a.rows.push([own(a, 1), own(b, 1), own(k, 1)]);
    k
}
fn canonical(c: &mut Cells, indices: &[usize], raw: &[u8]) -> usize {
    let mut modulus = field_modulus().to_bytes_le();
    modulus.resize(32, 0);
    let mut borrow = 0i16;
    let mut prior = None;
    for i in 0..32 {
        let difference = raw[i] as i16 - modulus[i] as i16 - borrow;
        let d = c.byte(difference.rem_euclid(256) as u8);
        let b = c.bit(difference < 0);
        let mut equation = plus(
            &own(indices[i], 1),
            &constant(vec![-Fp::from(modulus[i] as u64)]),
        );
        if let Some(p) = prior {
            equation = plus(&equation, &scaled(&own(p, 1), -Fp::ONE));
        }
        equation = plus(&equation, &scaled(&own(b, 1), Fp::from(256)));
        c.eq(equation, own(d, 1));
        prior = Some(b);
        borrow = i16::from(difference < 0);
    }
    prior.unwrap()
}

/// Parse a complete original Request into hard-computed words and hashes with
/// a total self-contained validity result. A noncanonical field maps to zero;
/// its original bytes are retained and its canonicality bit makes validity zero.
pub fn append_request(
    w: &mut Wiring<Fp>,
    family: usize,
    input: &CompletedRequest,
) -> OriginalRequest {
    let mut c = Cells::new(family);
    c.a.name = "total original Request bytes and predicates";
    for byte in input.body.iter().chain(&input.signature) {
        c.byte(*byte);
    }
    let mut checks = vec![];
    let mut native = [Fp::ZERO; 24];
    let words = std::array::from_fn(|i| {
        let (start, len) = request::WORDS[i];
        let raw = &input.body[start..start + len];
        let indices: Vec<_> = (start..start + len).collect();
        if len == 32 {
            let valid = canonical(&mut c, &indices, raw);
            checks.push(valid);
            native[i] =
                Option::<Fp>::from(Fp::from_repr(raw.try_into().unwrap())).unwrap_or(Fp::ZERO);
            let k = c.cell(native[i], 255);
            c.a.rows.push([own(valid, 1), c.bytes(&indices), own(k, 1)]);
            k
        } else {
            native[i] = fp(&BigUint::from_bytes_le(raw));
            let k = c.cell(native[i], (8 * len) as u16);
            c.eq(own(k, 1), c.bytes(&indices));
            k
        }
    });
    checks.push(zero(
        &mut c,
        plus(&own(words[0], 1), &constant(vec![-Fp::ONE])),
        native[0] - Fp::ONE,
    ));
    for [lo, hi] in [
        word::SCHEME,
        word::ASSET,
        word::PAYER_WALLET,
        word::PAYER_ACCOUNT,
        word::RECEIVER_WALLET,
        word::RECEIVER_ACCOUNT,
        word::NONCE,
    ] {
        let z = zero(
            &mut c,
            plus(&own(words[lo], 1), &own(words[hi], 1)),
            native[lo] + native[hi],
        );
        checks.push(not(&mut c, z));
    }
    for i in [word::RECEIVER_CREDENTIAL, word::AMOUNT, word::CERTIFICATES] {
        let z = zero(&mut c, own(words[i], 1), native[i]);
        checks.push(not(&mut c, z));
    }
    let equal: [usize; 2] = std::array::from_fn(|i| {
        let (p, r) = (word::PAYER_WALLET[i], word::RECEIVER_WALLET[i]);
        zero(
            &mut c,
            plus(&own(words[p], 1), &scaled(&own(words[r], 1), -Fp::ONE)),
            native[p] - native[r],
        )
    });
    let same = mul(&mut c, equal[0], equal[1]);
    checks.push(not(&mut c, same));
    let no_epoch = zero(
        &mut c,
        own(words[word::POLICY_EPOCH], 1),
        native[word::POLICY_EPOCH],
    );
    let no_policy = zero(
        &mut c,
        own(words[word::SCHEME_POLICY], 1),
        native[word::SCHEME_POLICY],
    );
    let pair_value = c.a.values[no_epoch][0] - c.a.values[no_policy][0];
    checks.push(zero(
        &mut c,
        plus(&own(no_epoch, 1), &scaled(&own(no_policy, 1), -Fp::ONE)),
        pair_value,
    ));
    let no_schedule = zero(
        &mut c,
        own(words[word::FEE_SCHEDULE], 1),
        native[word::FEE_SCHEDULE],
    );
    let missing_fee = mul(&mut c, no_schedule, words[word::FEE]);
    let missing_value = c.a.values[missing_fee][0];
    checks.push(zero(&mut c, own(missing_fee, 1), missing_value));
    let mut carry = 0u16;
    let mut prior = None;
    for i in 0..16 {
        let sum = input.body[242 + i] as u16 + input.body[290 + i] as u16 + carry;
        let digit = c.byte(sum as u8);
        let next = c.bit(sum > 255);
        let mut e = plus(&own(242 + i, 1), &own(290 + i, 1));
        if let Some(k) = prior {
            e = plus(&e, &own(k, 1));
        }
        e = plus(&e, &scaled(&own(next, 1), -Fp::from(256)));
        c.eq(e, own(digit, 1));
        prior = Some(next);
        carry = sum >> 8;
    }
    checks.push(not(&mut c, prior.unwrap()));
    let signature = std::array::from_fn(|i| {
        let start = request::BODY_BYTES + [16, 0, 48, 32][i];
        let indices: Vec<_> = (start..start + 16).rev().collect();
        let value = fp(&BigUint::from_bytes_be(
            &input.signature[start - request::BODY_BYTES..start - request::BODY_BYTES + 16],
        ));
        let k = c.cell(value, 128);
        c.eq(own(k, 1), c.bytes(&indices));
        k
    });
    let terms: Vec<_> = (0..24)
        .filter(|i| !request::PAYER_WORDS.contains(i))
        .map(|i| (native[i], own(words[i], 1)))
        .collect();
    let message = poseidon::hash_with_domain(
        request::TERMS_DOMAIN,
        &terms.iter().map(|x| x.0).collect::<Vec<_>>(),
    );
    let message_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(message, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let object: Vec<_> = std::iter::once((message, own(message_state[1], 1)))
        .chain(signature.iter().map(|k| (c.a.values[*k][0], own(*k, 1))))
        .collect();
    let digest = poseidon::hash_with_domain(
        request::OBJECT_DOMAIN,
        &object.iter().map(|x| x.0).collect::<Vec<_>>(),
    );
    let digest_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(digest, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let credit = poseidon::hash_with_domain(request::CREDIT_DOMAIN, &native);
    let credit_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(credit, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let credit_inputs: Vec<_> = (0..24).map(|i| (native[i], own(words[i], 1))).collect();
    let mut valid = checks[0];
    for check in checks.into_iter().skip(1) {
        valid = mul(&mut c, valid, check);
    }
    let local = w.push(c.a);
    let mut rng = SpikeRng::new(b"total original Request hashes");
    for (domain, inputs, outputs) in [
        (request::TERMS_DOMAIN, &terms, message_state),
        (request::OBJECT_DOMAIN, &object, digest_state),
        (request::CREDIT_DOMAIN, &credit_inputs, credit_state),
    ] {
        certificate::hash_relation(w, family, local, domain, inputs, outputs, &mut rng);
    }
    OriginalRequest {
        request: RequestCells {
            local,
            words,
            signature,
            message: message_state[1],
            digest: digest_state[1],
            credit: credit_state[1],
        },
        valid: Source {
            array: local,
            cell: valid,
            instance: 0,
        },
    }
}

/// Exact canonical field words carried as the incoming Send statement. This
/// opening is not a validated operation; the incoming proof must authenticate
/// its digest and the complete carrier owner must bind these same fields.
pub struct IncomingStatement {
    fields: [Source; 26],
    digest: Source,
}
impl IncomingStatement {
    pub fn fields(&self) -> [Source; 26] {
        self.fields
    }
    pub fn digest(&self) -> Source {
        self.digest
    }
}
pub fn append_statement(w: &mut Wiring<Fp>, family: usize, values: &[Fp; 26]) -> IncomingStatement {
    let mut c = Cells::new(family);
    c.a.name = "exact incoming Send statement opening";
    let fields = values.map(|v| c.cell(v, 255));
    let dg = crate::statement::digest(values);
    let outputs = [
        c.cell(Fp::ZERO, 255),
        c.cell(dg, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let inputs: Vec<_> = values
        .iter()
        .zip(fields)
        .map(|(v, k)| (*v, own(k, 1)))
        .collect();
    let local = w.push(c.a);
    certificate::hash_relation(
        w,
        family,
        local,
        crate::statement::DOMAIN,
        &inputs,
        outputs,
        &mut SpikeRng::new(b"incoming exact statement"),
    );
    let p = |cell| Source {
        array: local,
        cell,
        instance: 0,
    };
    IncomingStatement {
        fields: fields.map(p),
        digest: p(outputs[1]),
    }
}

/// Exact object/credential/statement predicate. Proof, Request signature and
/// full-carrier predicates are additional mandatory inputs to complete Receive.
pub struct IncomingObjects {
    valid: Source,
    quoted_continuity: Source,
}
impl IncomingObjects {
    pub fn valid(&self) -> Source {
        self.valid
    }
    pub fn quoted_continuity(&self) -> Source {
        self.quoted_continuity
    }
}

/// Derive every same-source object and identity equality. Candidate incoming
/// proof fields are explicitly untrusted until their separate total proof
/// verdict is combined. Mismatches return false; definitions and openings stay
/// hard, so witnesses cannot manufacture a failure for authentic fixed bytes.
#[allow(clippy::too_many_arguments)]
pub fn append_objects(
    w: &mut Wiring<Fp>,
    family: usize,
    original: &OriginalRequest,
    payer: &crate::operation_auth::CredentialOpening,
    quoted: &crate::operation_auth::CredentialOpening,
    receipt: &crate::receipt::ReceiptOpening,
    statement: &IncomingStatement,
    receiver: &crate::operation_auth::CurrentCredential,
    before: &crate::monetary_state::StateCells,
    predecessor: &crate::full::VerifiedPredecessor,
    incoming: &crate::full::IncomingProofFacts,
    prior: &crate::receive_lineage::PriorCredentialOpening,
) -> IncomingObjects {
    let continuity = crate::receive_lineage::append_credential_continuity(
        w,
        family,
        receiver.digest(),
        Source {
            array: before.local,
            cell: before.core[8],
            instance: 0,
        },
        quoted.digest(),
        quoted.renewal_sequence(),
        prior,
    );
    let req = original.request();
    let rq = |i| Source {
        array: req.local,
        cell: req.words[i],
        instance: 0,
    };
    let credit = Source {
        array: req.local,
        cell: req.credit,
        instance: 0,
    };
    let request_digest = Source {
        array: req.local,
        cell: req.digest,
        instance: 0,
    };
    let f = statement.fields();
    let ic = incoming.context();
    let own_context = predecessor.context();
    let mut equalities = vec![];
    for (a, b) in [
        (payer.scheme(), [ic[0], ic[1]]),
        (payer.asset(), [ic[4], ic[5]]),
        (payer.wallet(), [ic[6], ic[7]]),
        (payer.scheme(), receiver.scheme()),
        (payer.asset(), receiver.asset()),
        (payer.provider(), receiver.provider()),
        (quoted.scheme(), receiver.scheme()),
        (quoted.asset(), receiver.asset()),
        (quoted.wallet(), receiver.wallet()),
        (quoted.provider(), receiver.provider()),
        (word::SCHEME.map(rq), payer.scheme()),
        (word::ASSET.map(rq), payer.asset()),
        (word::PAYER_WALLET.map(rq), payer.wallet()),
        (word::PAYER_ACCOUNT.map(rq), payer.account()),
        (word::RECEIVER_WALLET.map(rq), quoted.wallet()),
        (word::RECEIVER_ACCOUNT.map(rq), quoted.account()),
        ([ic[2], ic[3]], [own_context[2], own_context[3]]),
        ([f[1], f[2]], [ic[2], ic[3]]),
        ([f[3], f[4]], payer.scheme()),
        ([f[5], f[6]], payer.asset()),
        ([f[18], f[19]], word::RECEIVER_WALLET.map(rq)),
        (receipt.scheme(), payer.scheme()),
        (receipt.wallet(), payer.wallet()),
        (receipt.provider(), payer.provider()),
    ] {
        equalities.extend(a.into_iter().zip(b));
    }
    for (a, b) in [
        (payer.payment_key(), [ic[9], ic[10], ic[11], ic[12]]),
        (payer.table_commitment(), incoming.table()),
        (quoted.payment_key(), receiver.payment_key()),
        (quoted.table_commitment(), receiver.table_commitment()),
    ] {
        equalities.extend(a.into_iter().zip(b));
    }
    equalities.extend([
        (payer.digest(), ic[8]),
        (f[7], payer.digest()),
        (f[15], incoming.head()),
        (statement.digest(), incoming.statement_digest()),
        (receipt.digest(), incoming.receipt_digest()),
        (rq(word::RECEIVER_CREDENTIAL), quoted.digest()),
        (f[17], credit),
        (f[20], rq(word::ORDINAL)),
        (f[21], rq(word::AMOUNT)),
        (f[22], rq(word::FEE)),
        (f[23], request_digest),
        (receipt.sequence(), f[9]),
        (receipt.predecessor(), f[14]),
        (receipt.successor(), f[15]),
        (receipt.statement_digest(), statement.digest()),
    ]);
    let mut c = Cells::new(family);
    c.a.name = "complete incoming object equality predicate";
    let mut checks = vec![];
    for s in [
        original.valid(),
        payer.valid(),
        quoted.valid(),
        receipt.valid(),
        continuity,
    ] {
        let k = c.cell(s.value(w), 1);
        c.eq(own(k, 1), s.linear());
        c.a.rows.push([own(k, 1), own(k, 1), own(k, 1)]);
        checks.push(k);
    }
    for (a, b) in equalities {
        checks.push(zero(
            &mut c,
            plus(&a.linear(), &scaled(&b.linear(), -Fp::ONE)),
            a.value(w) - b.value(w),
        ));
    }
    for (s, expected) in [
        (f[0], Fp::ONE),
        (f[16], Fp::from(3)),
        (incoming.kind()[0], Fp::ONE),
        (incoming.kind()[1], Fp::ZERO),
        (receipt.payment_digest(), Fp::ZERO),
    ] {
        checks.push(zero(
            &mut c,
            plus(&s.linear(), &constant(vec![-expected])),
            s.value(w) - expected,
        ));
    }
    let payer_wallet = payer.wallet();
    let op_inputs = [payer_wallet[0], payer_wallet[1], credit];
    let op_values = [
        payer_wallet[0].value(w),
        payer_wallet[1].value(w),
        Fp::from(3),
        credit.value(w),
    ];
    let operation = poseidon::hash_with_domain(u64::from_le_bytes(*b"kgwopid1"), &op_values);
    let op_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(operation, 255),
        c.cell(Fp::ZERO, 255),
    ];
    checks.push(zero(
        &mut c,
        plus(
            &own(op_state[1], 1),
            &scaled(&receipt.operation_id().linear(), -Fp::ONE),
        ),
        operation - receipt.operation_id().value(w),
    ));
    let items = vec![
        (op_values[0], op_inputs[0].linear()),
        (op_values[1], op_inputs[1].linear()),
        (op_values[2], constant(vec![Fp::from(3)])),
        (op_values[3], op_inputs[2].linear()),
    ];
    let mut valid = checks[0];
    for k in checks.into_iter().skip(1) {
        valid = mul(&mut c, valid, k);
    }
    let local = w.push(c.a);
    certificate::hash_relation(
        w,
        family,
        local,
        u64::from_le_bytes(*b"kgwopid1"),
        &items,
        op_state,
        &mut SpikeRng::new(b"incoming Send operation id"),
    );
    IncomingObjects {
        valid: Source {
            array: local,
            cell: valid,
            instance: 0,
        },
        quoted_continuity: continuity,
    }
}

#[cfg(test)]
#[path = "receive_originals_tests.rs"]
pub(crate) mod object_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ipa::Gens,
        nonnative,
        r1cs::{Circuit, Instance, Witness},
        spartan,
        uniform::{Array, UniformCircuit, link},
    };
    use iroha_pasta::Eq as Vesta;
    type Built = (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>);
    fn build(input: &CompletedRequest) -> Built {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let original = append_request(&mut w, family, input);
        let req = original.request();
        let p = |k| Source {
            array: req.local,
            cell: k,
            instance: 0,
        };
        let outputs: Vec<_> = req
            .words
            .into_iter()
            .chain(req.signature)
            .chain([req.message, req.digest, req.credit])
            .map(p)
            .chain([original.valid()])
            .collect();
        assert_eq!(outputs.len(), 32);
        let values: Vec<_> = outputs.iter().map(|s| s.value(&w)).collect();
        let io = w.push(Array {
            name: "total Request output ports",
            family,
            rho: 5,
            r_count: 32,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![values],
        });
        w.io_arrays.push((io, 32));
        let mut c = Cells::new(family);
        for (i, s) in outputs.iter().enumerate() {
            c.eq(s.linear(), link(io, 0, vec![Some(i)], Fp::ONE));
        }
        let mut bytes = vec![Fp::ZERO; 512];
        for (i, b) in input.body.iter().chain(&input.signature).enumerate() {
            bytes[i] = Fp::from(*b as u64);
        }
        let raw = w.push(Array {
            name: "total Request original bytes",
            family,
            rho: 9,
            r_count: 482,
            fixed: false,
            lookups: vec![],
            widths: vec![8],
            rows: vec![],
            values: vec![bytes],
        });
        w.io_arrays.push((raw, 512));
        for i in 0..482 {
            c.eq(
                original.byte(i).linear(),
                link(raw, 0, vec![Some(i)], Fp::ONE),
            );
        }
        w.push(c.a);
        let range = w.push(Array {
            name: "total Request range",
            family,
            rho: 11,
            r_count: 2048,
            fixed: true,
            lookups: vec![],
            widths: vec![11],
            rows: vec![],
            values: vec![(0..2048).map(|v| Fp::from(v as u64)).collect()],
        });
        w.tables.push((range, nonnative::RANGE_TABLE));
        let (arrays, pi, counts, tables, copies) = w.finish();
        UniformCircuit::build_with_copies(
            "total Request",
            arrays,
            pi,
            counts,
            tables,
            copies,
            12,
            1024,
            b"total original Request",
        )
    }
    fn holds(b: &Built) -> bool {
        let (c, i, w) = b;
        let mut v = w.w.clone();
        let r = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut v));
        let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: v });
        let (a, b, c) = r.abc(&c.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }
    #[test]
    fn valid_and_invalid_originals_have_one_matrix_and_real_proofs() {
        let good = request::reference_request();
        let mut bad = good.clone();
        bad.body[0] = 2;
        let a = build(&good);
        let b = build(&bad);
        assert!(holds(&a) && holds(&b));
        assert_eq!(a.1.io[63], Fp::ONE);
        assert_eq!(b.1.io[63], Fp::ZERO);
        assert_eq!(
            a.0.entries(&[Fp::from(31), Fp::from(47)]),
            b.0.entries(&[Fp::from(31), Fp::from(47)])
        );
        let gens = Gens::<Vesta>::new(a.0.shape.cols());
        for c in [&a, &b] {
            let (p, _) = spartan::prove_circuit(
                &a.0.shape,
                &gens,
                &c.1,
                &c.2,
                &mut SpikeRng::new(b"total Request proofs"),
                &a.0,
            );
            assert!(spartan::verify_circuit(&a.0.shape, &gens, &c.1, &p.to_bytes(), &a.0).is_ok());
            let mut forged = c.1.clone();
            forged.io[63] = Fp::ONE - forged.io[63];
            assert!(
                spartan::verify_circuit(&a.0.shape, &gens, &forged, &p.to_bytes(), &a.0).is_err()
            );
        }
    }
    #[test]
    fn native_golden_hashes_and_every_original_byte_are_bound() {
        let input = request::reference_request();
        let mut b = build(&input);
        assert!(holds(&b));
        for (offset, hex) in [
            (
                60,
                "cc9d8d9304ad330e1047a839321036e749967a85c1a1d49944fc32f8510c1f38",
            ),
            (
                61,
                "e5464b4cf8d9138c4a067296890c8d90da116a4b3e35394edb61e8d570cdd538",
            ),
            (
                62,
                "e37e9a5312d6fa9c64a62f9ef7fe1477be533854d2146a2dc13b02d7c4a5563f",
            ),
        ] {
            let bytes: Vec<_> = (0..64)
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            assert_eq!(b.1.io[offset].to_repr().as_ref(), bytes);
        }
        for i in 0..482 {
            b.1.io[512 + i] += Fp::ONE;
            assert!(!holds(&b), "raw byte {i}");
            b.1.io[512 + i] -= Fp::ONE;
        }
        for i in 32..64 {
            b.1.io[i] += Fp::ONE;
            assert!(!holds(&b), "derived port {i}");
            b.1.io[i] -= Fp::ONE;
        }
    }
    #[test]
    fn semantic_failures_are_total_and_cannot_be_misreported() {
        let good = request::reference_request();
        for case in 0..9 {
            let mut bad = good.clone();
            match case {
                0 => bad.body[2..34].fill(0),
                1 => bad.body[98..130].fill(0),
                2 => bad.body[242..258].fill(0),
                3 => {
                    let payer = bad.body[66..98].to_vec();
                    bad.body[130..162].copy_from_slice(&payer);
                }
                4 => bad.body[306..314].fill(0),
                5 => bad.body[258..290].fill(0),
                6 => bad.body[386..418].fill(0),
                7 => {
                    bad.body[242..258].fill(255);
                    bad.body[290..306].copy_from_slice(&1u128.to_le_bytes());
                }
                _ => bad.body[354..386].fill(0),
            }
            let mut b = build(&bad);
            assert!(holds(&b), "total failure {case}");
            assert_eq!(b.1.io[63], Fp::ZERO, "case {case}");
            b.1.io[63] = Fp::ONE;
            assert!(!holds(&b));
        }
    }
    #[test]
    fn noncanonical_fields_zero_select_without_losing_original_bytes() {
        let good = request::reference_request();
        let mut modulus = field_modulus().to_bytes_le();
        modulus.resize(32, 0);
        for (index, offset) in [
            (word::RECEIVER_CREDENTIAL, 210),
            (word::FEE_SCHEDULE, 258),
            (word::SCHEME_POLICY, 314),
            (word::CERTIFICATES, 354),
        ] {
            let mut bad = good.clone();
            bad.body[offset..offset + 32].copy_from_slice(&modulus);
            let mut b = build(&bad);
            assert!(holds(&b));
            assert_eq!(b.1.io[63], Fp::ZERO);
            assert_eq!(b.1.io[32 + index], Fp::ZERO);
            for i in 0..32 {
                assert_eq!(b.1.io[512 + offset + i], Fp::from(modulus[i] as u64));
            }
            b.1.io[32 + index] = Fp::ONE;
            assert!(!holds(&b));
        }
        let mut sig = good;
        sig.signature = [0; 64];
        let b = build(&sig);
        assert!(holds(&b));
        assert_eq!(
            b.1.io[63],
            Fp::ONE,
            "signature validity belongs to its mandatory owner"
        );
    }
}
