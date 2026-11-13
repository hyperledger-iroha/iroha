//! Future-generation Receive with a total, derived incoming verdict.
//!
//! The complete constructor requires a sealed hard wallet predecessor and the
//! total incoming proof owner's candidate facts. Exact original objects,
//! Request signature, full Payment identity, own receipt, OQ-3 map effects and
//! burn accounting are composed before returning the completed relation.

use ff::Field;
use iroha_pasta::{Fp, poseidon};

use crate::{
    certificate::{self, Cells},
    fs::SpikeRng,
    full::Wiring,
    monetary_state::StateCells,
    request::{RequestCells, word},
    send_arithmetic::Source,
    statement::StatementCells,
    uniform::{constant, own, plus},
};

#[derive(Clone)]
pub struct Witness {
    pub before_core: [Fp; 30],
    pub before_rest: [Fp; 6],
    pub after_core: [Fp; 30],
    pub after_rest: [Fp; 6],
    pub credential: crate::credential::Credential,
    pub request: crate::request::CompletedRequest,
    pub payer: crate::credential::Credential,
    pub quoted_receiver: crate::credential::Credential,
    pub incoming_statement: [Fp; 26],
    pub incoming_receipt: crate::bootstrap::Receipt,
    pub prior_credential: crate::receive_lineage::PriorCredentialOpening,
    pub statement: [Fp; 26],
    pub receipt: crate::bootstrap::Receipt,
    pub maps: crate::receive_lineage::ReceiveMapWitness,
}

/// A fully constructed Receive relation, not a host-side acceptance verdict.
/// Its incoming facts can only be produced by the total recursive verifier.
/// Native admission additionally requires `valid == 1`; the fold must retain
/// the derived invalid/burn branch for an already selected historical Receive.
pub struct ReceiveRelation<R> {
    before: StateCells,
    after: StateCells,
    request: crate::receive_originals::OriginalRequest,
    statement: StatementCells,
    credential: crate::operation_auth::CurrentCredential,
    receipt: R,
    payment: crate::payment::BoundPayment,
    effects: crate::receive_lineage::ReceiveEffects,
    objects: crate::receive_originals::IncomingObjects,
    signature: crate::receipt::SignatureValidity,
    lineage: [Source; crate::bootstrap_context::LINEAGE_FIELDS],
    context: [Source; crate::bootstrap_context::FIELDS],
    table: [Source; 4],
}
pub type CompleteReceive = ReceiveRelation<crate::receipt::AuthenticatedReceipt>;
/// Every monetary, incoming, map and exact unsigned receipt-body constraint,
/// before the canonical provider obtains the actual hardware signature.
pub type PreparedReceive = ReceiveRelation<crate::receipt::PreparedBody>;
impl PreparedReceive {
    pub fn body(&self) -> &crate::receipt::PreparedBody {
        &self.receipt
    }
}

impl<R> ReceiveRelation<R> {
    pub fn before(&self) -> &StateCells {
        &self.before
    }
    pub fn after(&self) -> &StateCells {
        &self.after
    }
    pub fn request(&self) -> &crate::receive_originals::OriginalRequest {
        &self.request
    }
    pub fn statement(&self) -> &StatementCells {
        &self.statement
    }
    pub fn credential(&self) -> &crate::operation_auth::CurrentCredential {
        &self.credential
    }
    pub fn receipt(&self) -> &R {
        &self.receipt
    }
    pub fn payment(&self) -> &crate::payment::BoundPayment {
        &self.payment
    }
    pub fn valid(&self) -> Source {
        self.effects.valid
    }
    pub fn objects_valid(&self) -> Source {
        self.objects.valid()
    }
    pub fn signature_valid(&self) -> Source {
        self.signature.valid()
    }
    pub fn nonmembership(&self) -> Source {
        self.effects.nonmembership
    }
    pub fn head(&self) -> Source {
        port(self.after.local, self.after.commitment)
    }
    pub fn lineage(&self) -> [Source; crate::bootstrap_context::LINEAGE_FIELDS] {
        self.lineage
    }
    pub fn context(&self) -> [Source; crate::bootstrap_context::FIELDS] {
        self.context
    }
    pub fn table(&self) -> [Source; 4] {
        self.table
    }
}

pub fn append(
    w: &mut Wiring<Fp>,
    family: usize,
    witness: &Witness,
    predecessor: &crate::full::VerifiedPredecessor,
    incoming: &crate::full::IncomingProofFacts,
) -> Result<CompleteReceive, &'static str> {
    let p = append_preparation(w, family, witness, predecessor, incoming)?;
    let receipt = crate::receipt::authenticate_prepared(
        w,
        family,
        &p.receipt,
        &witness.receipt,
        &p.credential,
        &p.statement,
        crate::statement::Kind::Receive,
        crate::receipt::PaymentDigest::Present(p.payment.digest()),
    )?;
    Ok(ReceiveRelation {
        before: p.before,
        after: p.after,
        request: p.request,
        statement: p.statement,
        credential: p.credential,
        receipt,
        payment: p.payment,
        effects: p.effects,
        objects: p.objects,
        signature: p.signature,
        lineage: p.lineage,
        context: p.context,
        table: p.table,
    })
}

pub fn append_preparation(
    w: &mut Wiring<Fp>,
    family: usize,
    witness: &Witness,
    predecessor: &crate::full::VerifiedPredecessor,
    incoming: &crate::full::IncomingProofFacts,
) -> Result<PreparedReceive, &'static str> {
    use crate::{
        bootstrap_context as context, monetary_state, operation_auth, payment, receipt,
        receive_lineage, receive_originals,
        statement::{self, Kind},
    };
    let before =
        monetary_state::append_state(w, family, &witness.before_core, &witness.before_rest);
    let after = monetary_state::append_state(w, family, &witness.after_core, &witness.after_rest);
    let credential =
        operation_auth::append_current(w, family, &witness.credential, &before, predecessor)?;
    let request = receive_originals::append_request(w, family, &witness.request);
    let payer = operation_auth::append_opening_total(w, family, &witness.payer);
    let quoted = operation_auth::append_opening_total(w, family, &witness.quoted_receiver);
    let incoming_statement =
        receive_originals::append_statement(w, family, &witness.incoming_statement);
    let incoming_receipt = receipt::append_opening_total(w, family, &witness.incoming_receipt);
    let objects = receive_originals::append_objects(
        w,
        family,
        &request,
        &payer,
        &quoted,
        &incoming_receipt,
        &incoming_statement,
        &credential,
        &before,
        predecessor,
        incoming,
        &witness.prior_credential,
    );
    let payment = payment::append_digest(
        w,
        family,
        request.request(),
        &payer,
        &incoming_statement,
        &incoming_receipt,
        incoming,
    );
    let statement = statement::append_statement(w, family, Kind::Receive, &witness.statement);
    append_arithmetic(w, family, &before, &after, request.request(), &statement);
    let prior = predecessor.lineage();
    let bf = |i| port(before.local, before.core[i]);
    let af = |i| port(after.local, after.core[i]);
    let sf = |i| port(statement.local, statement.fields[i]);
    let mut joins = Cells::new(family);
    joins.a.name = "complete Receive hard predecessor and own scope";
    for (a, b) in [
        (port(before.local, before.commitment), predecessor.head()),
        (bf(0), prior[context::LIFECYCLE]),
        (bf(27), prior[context::POLICY_EPOCH]),
        (bf(22), prior[context::ENABLED_CONTROLS]),
        (sf(1), predecessor.context()[context::RELATION]),
        (sf(2), predecessor.context()[context::RELATION + 1]),
    ] {
        joins.eq(a.linear(), b.linear());
    }
    w.push(joins.a);
    // The unsigned body installs the authenticated wallet table for the total
    // Request signature. No own hardware signature is invented during prepare.
    let receipt = receipt::append_preparation(
        w,
        family,
        &witness.receipt.body,
        &credential,
        &statement,
        Kind::Receive,
        receipt::PaymentDigest::Present(payment.digest()),
    )?;
    let signature = receipt::append_request_signature_total(w, family, &request, &credential);
    let mut c = Cells::new(family);
    c.a.name = "Receive derived incoming proof object and signature conjunction";
    let factors = [incoming.validity(), objects.valid(), signature.valid()];
    let mut predicate = None;
    let mut value = Fp::ONE;
    for factor in factors {
        let bit = c.cell(factor.value(w), 1);
        c.eq(own(bit, 1), factor.linear());
        c.a.rows.push([own(bit, 1), own(bit, 1), own(bit, 1)]);
        value *= factor.value(w);
        let result = c.cell(value, 1);
        c.a.rows.push([
            predicate
                .map(|p| own(p, 1))
                .unwrap_or_else(|| constant(vec![Fp::ONE])),
            own(bit, 1),
            own(result, 1),
        ]);
        predicate = Some(result);
    }
    let local = w.push(c.a);
    let effects = receive_lineage::append_effects(
        w,
        family,
        &before,
        &after,
        request.request(),
        receive_lineage::PreviousLineage {
            burned_total: prior[context::BURNED_TOTAL],
            pending_root: prior[context::PENDING_ROOT],
            credit_root: prior[context::CREDIT_ROOT],
        },
        payment.digest(),
        port(local, predicate.unwrap()),
        &witness.maps,
    );
    let lineage = [
        af(0),
        af(27),
        af(22),
        effects.burned_total,
        effects.pending_root,
        effects.credit_root,
    ];
    Ok(ReceiveRelation {
        before,
        after,
        request,
        statement,
        credential,
        receipt,
        payment,
        effects,
        objects,
        signature,
        lineage,
        context: predecessor.context(),
        table: predecessor.table(),
    })
}

pub const CHAIN_DOMAIN: u64 = u64::from_le_bytes(*b"kgwrchn1");

pub struct ReceiveArithmetic {
    pub local: usize,
    pub receive_chain: usize,
}

fn port(array: usize, cell: usize) -> Source {
    Source {
        array,
        cell,
        instance: 0,
    }
}

/// Bind the exact self-contained Receive step. Request/credential admission,
/// original incoming bytes, signatures, map updates and lineage verdicts remain
/// mandatory independent owners. Even a later burned Receive credits its Core
/// amount here; the lineage owner adds the same amount to adjusted burn.
pub fn append_arithmetic(
    w: &mut Wiring<Fp>,
    family: usize,
    before: &StateCells,
    after: &StateCells,
    request: &RequestCells,
    statement: &StatementCells,
) -> ReceiveArithmetic {
    for array in [before.local, after.local, request.local, statement.local] {
        assert_eq!((w.arrays[array].family, w.arrays[array].rho), (family, 0));
    }
    let previous = |i| port(before.local, before.core[i]);
    let next = |i| port(after.local, after.core[i]);
    let req = |i| port(request.local, request.words[i]);
    let stmt = |i| port(statement.local, statement.fields[i]);
    let mut c = Cells::new(family);
    c.a.name = "Receive exact arithmetic and statement";
    c.eq(stmt(16).linear(), constant(vec![Fp::from(4)]));
    c.eq(
        next(9).linear(),
        plus(&previous(9).linear(), &req(word::AMOUNT).linear()),
    );
    c.eq(
        next(11).linear(),
        plus(&previous(11).linear(), &constant(vec![Fp::ONE])),
    );
    // StateCells bounds both balances and sequences to u128; their small sums
    // cannot wrap Fp, so these equalities also enforce integer overflow checks.
    const CHANGED: [usize; 5] = [9, 11, 16, 17, 29];
    for i in 0..30 {
        if !CHANGED.contains(&i) {
            c.eq(next(i).linear(), previous(i).linear());
        }
    }
    for i in 0..6 {
        c.eq(
            port(after.local, after.rest[i]).linear(),
            port(before.local, before.rest[i]).linear(),
        );
    }
    // Scope and receiver mismatches in the original Request are total incoming
    // failures. The object predicate derives those equalities; own scope is
    // independently authenticated by State and the verified predecessor.
    for (s, core) in [
        (3, 1),
        (4, 2),
        (5, 3),
        (6, 4),
        (7, 7),
        (8, 0),
        (9, 11),
        (10, 13),
    ] {
        c.eq(stmt(s).linear(), next(core).linear());
    }
    c.eq(stmt(11).linear(), previous(22).linear());
    c.eq(
        stmt(14).linear(),
        port(before.local, before.commitment).linear(),
    );
    c.eq(
        stmt(15).linear(),
        port(after.local, after.commitment).linear(),
    );
    c.eq(
        stmt(17).linear(),
        port(request.local, request.credit).linear(),
    );
    c.eq(stmt(18).linear(), req(word::PAYER_WALLET[0]).linear());
    c.eq(stmt(19).linear(), req(word::PAYER_WALLET[1]).linear());
    c.eq(stmt(20).linear(), req(word::AMOUNT).linear());
    let inputs = [
        previous(16),
        port(request.local, request.credit),
        req(word::PAYER_WALLET[0]),
        req(word::PAYER_WALLET[1]),
        req(word::AMOUNT),
    ];
    let values: Vec<_> = inputs.iter().map(|p| p.value(w)).collect();
    let chain = poseidon::hash_with_domain(CHAIN_DOMAIN, &values);
    let output = [
        c.cell(Fp::ZERO, 255),
        c.cell(chain, 255),
        c.cell(Fp::ZERO, 255),
    ];
    c.eq(next(16).linear(), own(output[1], 1));
    let items: Vec<_> = inputs
        .into_iter()
        .map(|p| (p.value(w), p.linear()))
        .collect();
    let local = w.push(c.a);
    certificate::hash_relation(
        w,
        family,
        local,
        CHAIN_DOMAIN,
        &items,
        output,
        &mut SpikeRng::new(b"exact Receive chain"),
    );
    ReceiveArithmetic {
        local,
        receive_chain: output[1],
    }
}

#[cfg(test)]
#[path = "receive_integration_tests.rs"]
mod integration_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ipa::Gens,
        monetary_state, nonnative,
        r1cs::{Circuit, Instance, Witness},
        request, spartan,
        statement::{self, Kind},
        uniform::{Array, UniformCircuit, link},
    };
    use ff::PrimeField;
    use iroha_pasta::Eq as Vesta;
    type Built = (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>);
    fn build(
        change: impl FnOnce(&mut [Fp; 30], &mut [Fp; 30], &mut [Fp; 6], &mut [Fp; 6], &mut [Fp; 26]),
    ) -> Built {
        let mut w = Wiring::default();
        let f = w.family(0, 1);
        let req = request::append_completed(&mut w, f, &request::reference_request());
        let rv = |i: usize| w.arrays[req.local].values[req.words[i]][0];
        let mut before = [Fp::ZERO; 30];
        before[0] = Fp::ONE;
        for (ri, ci) in [
            (word::SCHEME, 1),
            (word::ASSET, 3),
            (word::RECEIVER_WALLET, 5),
        ] {
            for i in 0..2 {
                before[ci + i] = rv(ri[i]);
            }
        }
        for i in [7, 8, 17, 18, 19, 20, 21, 29] {
            before[i] = Fp::from(50 + i as u64);
        }
        before[7] = rv(word::RECEIVER_CREDENTIAL);
        before[9] = Fp::from(2000);
        before[10] = Fp::from(30);
        before[11] = Fp::from(5);
        before[16] = Fp::from(91);
        let mut after = before;
        after[9] += rv(word::AMOUNT);
        after[11] += Fp::ONE;
        after[16] = poseidon::hash_with_domain(
            CHAIN_DOMAIN,
            &[
                before[16],
                w.arrays[req.local].values[req.credit][0],
                rv(word::PAYER_WALLET[0]),
                rv(word::PAYER_WALLET[1]),
                rv(word::AMOUNT),
            ],
        );
        after[17] += Fp::ONE;
        after[29] += Fp::ONE;
        let mut br = [Fp::ZERO; 6];
        let mut ar = br;
        let mut s = [Fp::ZERO; 26];
        s[0] = Fp::ONE;
        s[1] = Fp::from(33);
        s[2] = Fp::from(44);
        s[3..7].copy_from_slice(&before[1..5]);
        s[7] = after[7];
        s[8] = after[0];
        s[9] = after[11];
        s[10] = after[13];
        s[11] = before[22];
        s[14] = monetary_state::commitment(&before, &br);
        s[15] = monetary_state::commitment(&after, &ar);
        s[16] = Fp::from(4);
        s[17] = w.arrays[req.local].values[req.credit][0];
        s[18] = rv(word::PAYER_WALLET[0]);
        s[19] = rv(word::PAYER_WALLET[1]);
        s[20] = rv(word::AMOUNT);
        change(&mut before, &mut after, &mut br, &mut ar, &mut s);
        // Source mutation tests can request coherent statement commitments by
        // changing fields before this rebuild; explicit header tests mutate s.
        let old = monetary_state::append_state(&mut w, f, &before, &br);
        let new = monetary_state::append_state(&mut w, f, &after, &ar);
        let st = statement::append_statement(&mut w, f, Kind::Receive, &s);
        let arithmetic = append_arithmetic(&mut w, f, &old, &new, &req, &st);
        let range = w.push(Array {
            name: "Receive range",
            family: f,
            rho: 11,
            r_count: 2048,
            fixed: true,
            lookups: vec![],
            widths: vec![11],
            rows: vec![],
            values: vec![(0..2048).map(|v| Fp::from(v as u64)).collect()],
        });
        w.tables.push((range, nonnative::RANGE_TABLE));
        let ports = [
            port(old.local, old.commitment),
            port(new.local, new.commitment),
            port(st.local, st.digest),
            port(arithmetic.local, arithmetic.receive_chain),
        ];
        let values = ports.map(|p| p.value(&w)).to_vec();
        let io = w.push(Array {
            name: "Receive arithmetic public",
            family: f,
            rho: 2,
            r_count: 4,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![values],
        });
        w.io_arrays.push((io, 4));
        let mut c = Cells::new(f);
        for (i, p) in ports.into_iter().enumerate() {
            c.eq(p.linear(), link(io, 0, vec![Some(i)], Fp::ONE));
        }
        w.push(c.a);
        let (arrays, pi, counts, tables, copies) = w.finish();
        UniformCircuit::build_with_copies(
            "Receive arithmetic",
            arrays,
            pi,
            counts,
            tables,
            copies,
            12,
            16,
            b"Receive arithmetic tests",
        )
    }
    fn holds(b: &Built) -> bool {
        let (c, i, w) = b;
        let mut values = w.w.clone();
        let relation = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut values));
        let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: values });
        let (a, b, c) = relation.abc(&c.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }
    fn rehash(b: &[Fp; 30], a: &[Fp; 30], br: &[Fp; 6], ar: &[Fp; 6], s: &mut [Fp; 26]) {
        s[14] = monetary_state::commitment(b, br);
        s[15] = monetary_state::commitment(a, ar);
    }
    #[test]
    fn active_and_retiring_receives_verify_under_one_matrix() {
        let a = build(|_, _, _, _, _| {});
        let r = build(|b, a, br, ar, s| {
            b[0] = Fp::from(2);
            a[0] = Fp::from(2);
            s[8] = Fp::from(2);
            rehash(b, a, br, ar, s);
        });
        assert!(holds(&a) && holds(&r));
        assert_eq!(
            a.0.entries(&[Fp::from(31), Fp::from(47)]),
            r.0.entries(&[Fp::from(31), Fp::from(47)])
        );
        let gens = Gens::<Vesta>::new(a.0.shape.cols());
        for (_, i, w) in [&a, &r] {
            let (p, _) = spartan::prove_circuit(
                &a.0.shape,
                &gens,
                i,
                w,
                &mut SpikeRng::new(b"Receive arithmetic proofs"),
                &a.0,
            );
            assert!(spartan::verify_circuit(&a.0.shape, &gens, i, &p.to_bytes(), &a.0).is_ok());
            for k in 4..8 {
                let mut bad = i.clone();
                bad.io[k] += Fp::ONE;
                assert!(
                    spartan::verify_circuit(&a.0.shape, &gens, &bad, &p.to_bytes(), &a.0).is_err()
                );
            }
        }
    }
    #[test]
    fn amount_sequence_and_chain_cannot_change_or_overflow() {
        for k in [9, 11, 16] {
            let bad = build(|b, a, br, ar, s| {
                a[k] += Fp::ONE;
                rehash(b, a, br, ar, s);
            });
            assert!(!holds(&bad), "changed update {k}");
        }
        let overflow = build(|b, a, br, ar, s| {
            b[9] = Fp::from_u128(u128::MAX);
            a[9] = b[9] + Fp::from(1000);
            rehash(b, a, br, ar, s);
        });
        assert!(!holds(&overflow));
        let overflow = build(|b, a, br, ar, s| {
            b[11] = Fp::from_u128(u128::MAX);
            a[11] = b[11] + Fp::ONE;
            s[9] = a[11];
            rehash(b, a, br, ar, s);
        });
        assert!(!holds(&overflow));
    }
    #[test]
    fn every_unchanged_state_and_statement_join_is_mandatory() {
        for k in 0..30 {
            if [9, 11, 16, 17, 29].contains(&k) {
                continue;
            }
            let bad = build(|b, a, br, ar, s| {
                a[k] += Fp::ONE;
                rehash(b, a, br, ar, s);
            });
            assert!(!holds(&bad), "changed core {k}");
        }
        for k in 0..6 {
            let bad = build(|b, a, br, ar, s| {
                ar[k] += Fp::ONE;
                rehash(b, a, br, ar, s);
            });
            assert!(!holds(&bad), "changed rest {k}");
        }
        for k in [3, 4, 5, 6, 7, 8, 9, 10, 11, 14, 15, 16, 17, 18, 19, 20] {
            let bad = build(|_, _, _, _, s| s[k] += Fp::ONE);
            assert!(!holds(&bad), "statement {k}");
        }
    }
}
