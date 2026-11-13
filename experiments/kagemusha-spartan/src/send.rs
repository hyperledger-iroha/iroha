//! Complete future-generation Send relation.
//!
//! Construction requires the sealed result of the recursive predecessor
//! verifier. Every monetary, policy, map, state and own-receipt obligation is
//! composed here before a `CompleteSend` can be returned. The type means the
//! complete relation was constructed; acceptance still requires its proof.

use iroha_pasta::Fp;

use crate::{
    bootstrap::Receipt,
    bootstrap_context as context,
    certificate::Cells,
    credential::Credential,
    fee_schedule::{self, SignedFeeSchedule},
    full::{VerifiedPredecessor, Wiring},
    monetary_state::{self, StateCells},
    operation_auth::{self, CurrentCredential},
    receipt::{self, AuthenticatedReceipt, PaymentDigest},
    request::{self, CompletedRequest, RequestCells, word},
    send_arithmetic::{self, Source},
    send_controls::{self, QuotaWitness},
    send_maps,
    statement::{self, Kind, StatementCells},
};

#[derive(Clone)]
pub struct Witness {
    pub before_core: [Fp; 30],
    pub before_rest: [Fp; 6],
    pub after_core: [Fp; 30],
    pub after_rest: [Fp; 6],
    pub credential: Credential,
    pub request: CompletedRequest,
    pub fee_schedule: SignedFeeSchedule,
    pub quota: QuotaWitness,
    pub lower: u64,
    pub upper: u64,
    pub statement: [Fp; 26],
    pub receipt: Receipt,
    pub maps: send_maps::Witness,
}

pub struct SendOperation<R> {
    before: StateCells,
    after: StateCells,
    request: RequestCells,
    statement: StatementCells,
    credential: CurrentCredential,
    receipt: R,
    lineage: [Source; context::LINEAGE_FIELDS],
    context: [Source; context::FIELDS],
    table: [Source; 4],
}
pub type CompleteSend = SendOperation<AuthenticatedReceipt>;
pub type PreparedSend = SendOperation<receipt::PreparedBody>;

impl<R> SendOperation<R> {
    pub fn before(&self) -> &StateCells {
        &self.before
    }
    pub fn after(&self) -> &StateCells {
        &self.after
    }
    pub fn request(&self) -> &RequestCells {
        &self.request
    }
    pub fn statement(&self) -> &StatementCells {
        &self.statement
    }
    pub fn credential(&self) -> &CurrentCredential {
        &self.credential
    }
    pub fn receipt(&self) -> &R {
        &self.receipt
    }
    pub fn head(&self) -> Source {
        source(self.after.local, self.after.commitment)
    }
    pub fn lineage(&self) -> [Source; context::LINEAGE_FIELDS] {
        self.lineage
    }
    pub fn context(&self) -> [Source; context::FIELDS] {
        self.context
    }
    pub fn table(&self) -> [Source; 4] {
        self.table
    }
    fn with_receipt<T>(self, receipt: T) -> SendOperation<T> {
        SendOperation {
            before: self.before,
            after: self.after,
            request: self.request,
            statement: self.statement,
            credential: self.credential,
            receipt,
            lineage: self.lineage,
            context: self.context,
            table: self.table,
        }
    }
}
impl PreparedSend {
    pub fn body(&self) -> &receipt::PreparedBody {
        &self.receipt
    }
}

fn source(array: usize, cell: usize) -> Source {
    Source {
        array,
        cell,
        instance: 0,
    }
}

pub fn append(
    w: &mut Wiring<Fp>,
    family: usize,
    witness: &Witness,
    predecessor: &VerifiedPredecessor,
) -> Result<CompleteSend, &'static str> {
    let operation = append_relation(w, family, witness, predecessor)?;
    let receipt = receipt::append_own(
        w,
        family,
        &witness.receipt,
        &operation.credential,
        &operation.statement,
        Kind::Send,
        PaymentDigest::Absent,
    )?;
    Ok(operation.with_receipt(receipt))
}

/// Construct the identical complete monetary relation before hardware Advance.
/// The private returned body is an unsigned intent, never receipt authority.
pub fn append_preparation(
    w: &mut Wiring<Fp>,
    family: usize,
    witness: &Witness,
    predecessor: &VerifiedPredecessor,
) -> Result<PreparedSend, &'static str> {
    let operation = append_relation(w, family, witness, predecessor)?;
    let receipt = receipt::append_preparation(
        w,
        family,
        &witness.receipt.body,
        &operation.credential,
        &operation.statement,
        Kind::Send,
        PaymentDigest::Absent,
    )?;
    Ok(operation.with_receipt(receipt))
}

fn append_relation(
    w: &mut Wiring<Fp>,
    family: usize,
    witness: &Witness,
    predecessor: &VerifiedPredecessor,
) -> Result<SendOperation<()>, &'static str> {
    let before =
        monetary_state::append_state(w, family, &witness.before_core, &witness.before_rest);
    let after = monetary_state::append_state(w, family, &witness.after_core, &witness.after_rest);
    let credential =
        operation_auth::append_current(w, family, &witness.credential, &before, predecessor)?;
    let request = request::append_completed(w, family, &witness.request);
    let statement = statement::append_statement(w, family, Kind::Send, &witness.statement);
    let prior = predecessor.lineage();
    let arithmetic = send_arithmetic::append_send_arithmetic(
        w,
        family,
        &before,
        &after,
        &request,
        prior[context::BURNED_TOTAL],
    );
    // Mandatory even for a zero fee: absence is derived from held state.
    fee_schedule::append_held(w, family, &witness.fee_schedule, &before, &request);
    let controls = send_controls::append_send_controls(
        w,
        family,
        &before,
        &request,
        witness.lower,
        witness.upper,
        &witness.quota,
    );
    let maps = send_maps::append(
        w,
        family,
        &before,
        &after,
        &request,
        &statement,
        prior[context::PENDING_ROOT],
        &witness.maps,
    );
    let bf = |k| source(before.local, before.core[k]);
    let af = |k| source(after.local, after.core[k]);
    let sf = |k| source(statement.local, statement.fields[k]);
    let rq = |k| source(request.local, request.words[k]);
    let mut joins = Cells::new(family);
    joins.a.name = "complete Send state statement and authority joins";
    for (a, b) in [
        (source(before.local, before.commitment), predecessor.head()),
        (bf(0), prior[context::LIFECYCLE]),
        (bf(27), prior[context::POLICY_EPOCH]),
        (bf(22), prior[context::ENABLED_CONTROLS]),
        (sf(1), predecessor.context()[context::RELATION]),
        (sf(2), predecessor.context()[context::RELATION + 1]),
        (sf(3), af(1)),
        (sf(4), af(2)),
        (sf(5), af(3)),
        (sf(6), af(4)),
        (sf(7), af(7)),
        (sf(8), af(0)),
        (sf(9), af(11)),
        (sf(10), af(13)),
        (sf(11), bf(22)),
        (sf(12), prior[context::BURNED_TOTAL]),
        (sf(13), prior[context::PENDING_ROOT]),
        (sf(14), source(before.local, before.commitment)),
        (sf(15), source(after.local, after.commitment)),
        (sf(17), source(request.local, request.credit)),
        (sf(18), rq(word::RECEIVER_WALLET[0])),
        (sf(19), rq(word::RECEIVER_WALLET[1])),
        (sf(20), rq(word::ORDINAL)),
        (sf(21), rq(word::AMOUNT)),
        (sf(22), rq(word::FEE)),
        (sf(23), source(request.local, request.digest)),
        (sf(24), source(controls.local, controls.lower)),
        (sf(25), source(controls.local, controls.upper)),
        (af(28), source(controls.local, controls.new_floor)),
        (af(21), source(controls.local, controls.new_usage_root)),
        (
            source(arithmetic.local, arithmetic.gross),
            source(controls.local, controls.gross),
        ),
        (rq(word::PAYER_ACCOUNT[0]), credential.account()[0]),
        (rq(word::PAYER_ACCOUNT[1]), credential.account()[1]),
    ] {
        joins.eq(a.linear(), b.linear());
    }
    w.push(joins.a);
    let lineage = [
        af(0),
        af(27),
        af(22),
        source(arithmetic.local, arithmetic.adjusted_burn),
        maps.pending_root,
        prior[context::CREDIT_ROOT],
    ];
    Ok(SendOperation {
        before,
        after,
        request,
        statement,
        credential,
        receipt: (),
        lineage,
        context: predecessor.context(),
        table: predecessor.table(),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        bootstrap,
        certificate::fp,
        full::{self, WalletKey},
        indexed_map,
        ipa::Gens,
        r1cs::{Circuit, Instance, Witness as ProofWitness},
        spartan,
        uniform::{Array, UniformCircuit, constant, link, plus, scaled},
    };
    use ff::{Field, PrimeField};
    use iroha_pasta::{Eq as Vesta, poseidon};
    use num_bigint::BigUint;

    pub(crate) struct Fixture {
        pub(crate) input: Witness,
        pub(crate) predecessor: [Fp; 64],
        pub(crate) key: WalletKey,
    }

    fn limbs(bytes: &[u8]) -> [Fp; 2] {
        [
            fp(&BigUint::from_bytes_le(&bytes[..16])),
            fp(&BigUint::from_bytes_le(&bytes[16..])),
        ]
    }

    /// A genuinely signed current credential/own receipt with a separately
    /// authenticated funded predecessor port. This is a transition test, not
    /// a claim that Bootstrap is permitted to mint a starting balance.
    pub(crate) fn fixture(fee: u128, retiring: bool) -> Fixture {
        let key = WalletKey::labelled("complete Send payer fixture");
        let gens = Gens::<Vesta>::new(1 << 16);
        let (context, initial) = bootstrap::fixture(&key, &gens);
        let credential = initial.credential;
        let mut before_core = initial.core;
        let mut before_rest = initial.rest;
        before_core[0] = Fp::from(if retiring { 2 } else { 1 });
        before_core[9] = Fp::from(2000);
        before_core[10] = Fp::from(10);
        before_core[11] = Fp::from(8);
        before_core[12] = Fp::from(7);
        before_core[13] = Fp::ONE;
        let mut schedule = SignedFeeSchedule {
            body: [255; 191],
            signature: [1; 64],
        };
        if fee != 0 {
            schedule.body.fill(0);
            schedule.body[0] = 1;
            schedule.body[2..66].copy_from_slice(&credential.body[2..66]);
            schedule.body[66] = 1;
            schedule.body[74] = 1;
            schedule.body[110..126].copy_from_slice(&fee.to_le_bytes());
            schedule.body[142..158].fill(255);
            schedule.body[158] = 1;
            schedule.body[159] = 1;
            before_core[27] = Fp::from(5);
            before_rest[1] = Fp::from(9);
            before_rest[2] = fee_schedule::digest(&schedule);
        }
        let mut request = CompletedRequest {
            body: [0; 418],
            signature: [1; 64],
        };
        request.body[0] = 1;
        request.body[2..130].copy_from_slice(&credential.body[2..130]);
        request.body[130..162].fill(0x88);
        request.body[162..194].fill(0x99);
        request.body[194..210].copy_from_slice(&7u128.to_le_bytes());
        request.body[210] = 7;
        request.body[242..258].copy_from_slice(&1000u128.to_le_bytes());
        request.body[258..290].copy_from_slice(before_rest[2].to_repr().as_ref());
        request.body[290..306].copy_from_slice(&fee.to_le_bytes());
        request.body[306..314].copy_from_slice(&before_core[27].to_repr()[..8]);
        request.body[314..346].copy_from_slice(before_rest[1].to_repr().as_ref());
        request.body[346..354].copy_from_slice(&110u64.to_le_bytes());
        request.body[354] = 1;
        request.body[386..].fill(0x66);
        let mut scratch = Wiring::default();
        let family = scratch.family(0, 1);
        let req = request::append_completed(&mut scratch, family, &request);
        let rv = |cell: usize| scratch.arrays[req.local].values[cell][0];
        let credit = rv(req.credit);
        let request_digest = rv(req.digest);
        let mut statement = initial.statement;
        statement[8] = before_core[0];
        statement[9] = before_core[11] + Fp::ONE;
        statement[10] = before_core[13];
        statement[11] = before_core[22];
        statement[12] = Fp::from(30);
        statement[13] = before_core[18];
        statement[14] = monetary_state::commitment(&before_core, &before_rest);
        statement[16] = Fp::from(3);
        statement[17] = credit;
        statement[18..20].copy_from_slice(&limbs(&request.body[130..162]));
        statement[20] = before_core[12];
        statement[21] = Fp::from(1000);
        statement[22] = Fp::from_u128(fee);
        statement[23] = request_digest;
        statement[24] = Fp::from(110);
        statement[25] = Fp::from(120);
        let descriptor = poseidon::hash_with_domain(send_maps::PENDING_DOMAIN, &statement[17..24]);
        let (pending, pending_root) = indexed_map::empty_insert(credit, descriptor);
        let fee_value = poseidon::hash_with_domain(
            send_maps::FEE_DOMAIN,
            &[credit, Fp::from_u128(fee), before_rest[2]],
        );
        let (fee_path, fee_root) = indexed_map::empty_insert(credit, fee_value);
        let mut after_core = before_core;
        after_core[9] -= Fp::from_u128(1000 + fee);
        after_core[10] = statement[12];
        after_core[11] += Fp::ONE;
        after_core[12] += Fp::ONE;
        after_core[15] = poseidon::hash_with_domain(
            send_arithmetic::CHAIN_DOMAIN,
            &[
                before_core[15],
                credit,
                statement[18],
                statement[19],
                before_core[12],
                statement[21],
                statement[22],
                request_digest,
            ],
        );
        after_core[18] = pending_root;
        after_core[20] = if fee == 0 { before_core[20] } else { fee_root };
        after_core[28] = Fp::from(110);
        after_core[29] += Fp::ONE;
        statement[15] = monetary_state::commitment(&after_core, &before_rest);
        let mut predecessor = [Fp::ZERO; 64];
        predecessor[full::HEAD_IO] = statement[14];
        predecessor[full::STATEMENT_IO] = Fp::from(77);
        predecessor[full::RECEIPT_IO] = Fp::from(78);
        predecessor[16..18].copy_from_slice(&limbs(&credential.body[2..34]));
        predecessor[18..20].copy_from_slice(&limbs(&context.relation));
        predecessor[20..22].copy_from_slice(&limbs(&credential.body[34..66]));
        predecessor[22..24].copy_from_slice(&limbs(&credential.body[66..98]));
        predecessor[24] = before_core[7];
        for (i, start) in [147, 131, 179, 163].into_iter().enumerate() {
            predecessor[25 + i] = fp(&BigUint::from_bytes_be(&credential.body[start..start + 16]));
        }
        predecessor[32..38].copy_from_slice(&[
            before_core[0],
            before_core[27],
            before_core[22],
            statement[12],
            before_core[18],
            before_core[17],
        ]);
        for i in 0..4 {
            predecessor[56 + i] = fp(&BigUint::from_bytes_le(
                &credential.body[195 + 16 * i..211 + 16 * i],
            ));
        }
        let mut input = Witness {
            before_core,
            before_rest,
            after_core,
            after_rest: before_rest,
            credential,
            request,
            fee_schedule: schedule,
            quota: QuotaWitness::default(),
            lower: 110,
            upper: 120,
            statement,
            receipt: initial.receipt,
            maps: send_maps::Witness {
                pending,
                fee: fee_path,
            },
        };
        sign_receipt(&mut input, &key);
        Fixture {
            input,
            predecessor,
            key,
        }
    }

    fn sign_receipt(input: &mut Witness, key: &WalletKey) {
        let operation = poseidon::hash_with_domain(
            u64::from_le_bytes(*b"kgwopid1"),
            &[
                input.before_core[5],
                input.before_core[6],
                Fp::from(3),
                input.statement[17],
            ],
        );
        input.receipt.body[98..114].copy_from_slice(&input.statement[9].to_repr()[..16]);
        for (at, value) in [
            (114, operation),
            (146, input.statement[14]),
            (178, input.statement[15]),
            (210, statement::digest(&input.statement)),
        ] {
            input.receipt.body[at..at + 32].copy_from_slice(value.to_repr().as_ref());
        }
        input.receipt.signature = bootstrap::sign(
            bootstrap::receipt_message(&input.receipt),
            &key.d,
            "complete Send own receipt",
        );
    }

    type Built = (
        UniformCircuit<Fp>,
        Instance<Fp>,
        ProofWitness<Fp>,
        CompleteSend,
    );
    fn build(f: &Fixture) -> Result<Built, &'static str> {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let previous = VerifiedPredecessor::fixture(&mut w, family, f.predecessor);
        // Bind every fixture leaf to a separate public block. The production
        // constructor instead sources these leaves from the recursive verifier.
        let public = w.push(Array {
            name: "Send test predecessor public ports",
            family,
            rho: 6,
            r_count: 64,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![f.predecessor.to_vec()],
        });
        w.io_arrays.push((public, 128));
        let mut joins = Cells::new(family);
        let predecessor_array = previous.head().array;
        for i in 0..64 {
            joins.eq(
                link(predecessor_array, 0, vec![Some(64 + i)], Fp::ONE),
                link(public, 0, vec![Some(i)], Fp::ONE),
            );
        }
        w.push(joins.a);
        let complete = append(&mut w, family, &f.input, &previous)?;
        let mut sources = vec![
            (full::HEAD_IO, complete.head()),
            (
                full::STATEMENT_IO,
                source(complete.statement.local, complete.statement.digest),
            ),
            (full::RECEIPT_IO, complete.receipt.digest()),
        ];
        sources.extend(
            complete
                .lineage()
                .into_iter()
                .enumerate()
                .map(|(i, p)| (32 + i, p)),
        );
        sources.extend(
            complete
                .context()
                .into_iter()
                .enumerate()
                .map(|(i, p)| (16 + i, p)),
        );
        let mut output_bindings = Cells::new(family);
        output_bindings.a.name = "complete Send public output bindings";
        for (offset, source) in sources {
            let value = source.value(&w);
            let io = w.push(Array {
                name: "complete Send public output",
                family,
                rho: 0,
                r_count: 1,
                fixed: false,
                lookups: vec![],
                widths: vec![255],
                rows: vec![],
                values: vec![vec![value]],
            });
            w.io_arrays.push((io, offset));
            output_bindings.a.rows.push([
                plus(
                    &source.linear(),
                    &scaled(&link(io, 0, vec![Some(0)], Fp::ONE), -Fp::ONE),
                ),
                constant(vec![Fp::ONE]),
                vec![],
            ]);
        }
        w.push(output_bindings.a);
        let (a, pi, n, t, copies) = w.finish();
        let (c, i, witness) = UniformCircuit::build_with_copies(
            "complete Send",
            a,
            pi,
            n,
            t,
            copies,
            16,
            256,
            b"complete Send",
        );
        Ok((c, i, witness, complete))
    }
    fn holds(b: &Built) -> bool {
        let (c, i, w, _) = b;
        let mut values = w.w.clone();
        let relation = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut values));
        let z = crate::r1cs::z_vector(&c.shape, i, &ProofWitness { w: values });
        let (a, b, c) = relation.abc(&c.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }

    #[test]
    fn complete_send_zero_fee_paid_fee_and_retiring_share_one_verified_relation() {
        let a = build(&fixture(0, false)).unwrap();
        let b = build(&fixture(6, false)).unwrap();
        let c = build(&fixture(6, true)).unwrap();
        let mut adjusted = fixture(6, false);
        // A folded lineage may have removed an entry still present in the
        // core's historical pending root. Send consumes the adjusted root.
        adjusted.input.before_core[18] = indexed_map::empty_insert::<32>(Fp::ONE, Fp::from(7)).1;
        let before_head =
            monetary_state::commitment(&adjusted.input.before_core, &adjusted.input.before_rest);
        adjusted.input.statement[14] = before_head;
        adjusted.predecessor[full::HEAD_IO] = before_head;
        sign_receipt(&mut adjusted.input, &adjusted.key);
        let d = build(&adjusted).unwrap();
        let matrix = a.0.entries(&[Fp::from(31), Fp::from(47)]);
        let gens = Gens::<Vesta>::new(a.0.shape.cols());
        for candidate in [&a, &b, &c, &d] {
            assert!(holds(candidate));
            assert_eq!(candidate.0.entries(&[Fp::from(31), Fp::from(47)]), matrix);
            let (_, instance, witness, _) = candidate;
            let (proof, _) = spartan::prove_circuit(
                &a.0.shape,
                &gens,
                instance,
                witness,
                &mut crate::fs::SpikeRng::new(b"complete Send proof"),
                &a.0,
            );
            assert!(
                spartan::verify_circuit(&a.0.shape, &gens, instance, &proof.to_bytes(), &a.0)
                    .is_ok()
            );
            for slot in [
                4,
                5,
                6,
                24,
                32,
                33,
                34,
                35,
                36,
                37,
                56,
                128 + 4,
                128 + 24,
                128 + 35,
                128 + 36,
            ] {
                let mut wrong = instance.clone();
                wrong.io[slot] += Fp::ONE;
                assert!(
                    spartan::verify_circuit(&a.0.shape, &gens, &wrong, &proof.to_bytes(), &a.0)
                        .is_err(),
                    "unbound port {slot}"
                );
            }
        }
    }

    #[test]
    fn complete_send_rejects_individually_valid_but_mismatched_obligations() {
        let base = fixture(6, false);
        let good = build(&base).unwrap();
        assert!(holds(&good));
        for (array, cell) in [
            (good.3.before.local, good.3.before.core[9]),
            (good.3.after.local, good.3.after.core[9]),
            (good.3.after.local, good.3.after.core[10]),
            (good.3.after.local, good.3.after.core[17]),
            (good.3.after.local, good.3.after.core[18]),
            (good.3.after.local, good.3.after.core[20]),
            (good.3.after.local, good.3.after.core[21]),
            (good.3.after.local, good.3.after.core[28]),
            (
                good.3.request.local,
                good.3.request.words[word::PAYER_ACCOUNT[0]],
            ),
            (good.3.request.local, good.3.request.words[word::FEE]),
            (good.3.statement.local, good.3.statement.fields[13]),
            (good.3.statement.local, good.3.statement.fields[24]),
        ] {
            let mut bad = ProofWitness {
                w: good.2.w.clone(),
            };
            bad.w[good.0.cell_index(array, cell, 0)] += Fp::ONE;
            let tuple = (&good.0, &good.1, &bad);
            let mut values = tuple.2.w.clone();
            let rel = tuple.0.instantiate(
                &tuple.0.shape,
                &[Fp::from(31), Fp::from(47)],
                Some(&mut values),
            );
            let z = crate::r1cs::z_vector(&tuple.0.shape, tuple.1, &ProofWitness { w: values });
            let (a, b, c) = rel.abc(&tuple.0.shape, &z);
            assert!(
                a.iter().zip(b).zip(c).any(|((a, b), c)| *a * b != c),
                "unbound obligation {array}/{cell}"
            );
        }
        let mut wrong = fixture(6, false);
        wrong.input.maps.pending.old_root += Fp::ONE;
        assert!(
            !holds(&build(&wrong).unwrap()),
            "pending insertion must use adjusted predecessor root"
        );
        let mut wrong = fixture(6, false);
        wrong.input.receipt.body[306] = 1;
        wrong.input.receipt.signature = bootstrap::sign(
            bootstrap::receipt_message(&wrong.input.receipt),
            &wrong.key.d,
            "wrong Send payment digest",
        );
        assert!(
            !holds(&build(&wrong).unwrap()),
            "even a valid own signature cannot add a Receive payment digest to Send"
        );
    }
}
