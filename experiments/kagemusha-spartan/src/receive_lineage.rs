//! Receive's authenticated map effects, exact burn accounting and prior credentials.
//!
//! Incoming facts must come from the complete original-object/signature/proof
//! predicate. This component derives nonmembership itself and never permits a
//! chosen search failure or a caller's claimed membership bit to cause a burn.

use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};

use crate::{
    certificate::{self, Cells},
    fs::SpikeRng,
    full::Wiring,
    indexed_map::{self, InsertWitness, SearchWitness},
    monetary_state::StateCells,
    request::{RequestCells, word},
    send_arithmetic::Source,
    uniform::{Lin, constant, own, plus, scaled},
};

pub const CONSUMED_DOMAIN: u64 = u64::from_le_bytes(*b"kgwccrd1");
pub const CREDIT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwcdig1");
pub const PRIOR_LEAF_DOMAIN: u64 = u64::from_le_bytes(*b"kgwpcrd1");
pub const PRIOR_NODE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwpcrn1");

fn port(array: usize, cell: usize) -> Source {
    Source {
        array,
        cell,
        instance: 0,
    }
}

struct Hash {
    domain: u64,
    inputs: Vec<(Fp, Lin<Fp>)>,
    outputs: [usize; 3],
}
struct Builder {
    c: Cells,
    hashes: Vec<Hash>,
}
impl Builder {
    fn new(family: usize, name: &'static str) -> Self {
        let mut c = Cells::new(family);
        c.a.name = name;
        Self { c, hashes: vec![] }
    }
    fn value(&self, k: usize) -> Fp {
        self.c.a.values[k][0]
    }
    fn source(&mut self, w: &Wiring<Fp>, source: Source) -> usize {
        let k = self.c.cell(source.value(w), 255);
        self.c.eq(own(k, 1), source.linear());
        k
    }
    fn bounded(&mut self, v: Fp, bytes: usize) -> usize {
        let k = self.c.cell(v, 255);
        let raw = v.to_repr();
        let bs: Vec<_> = raw[..bytes].iter().map(|b| self.c.byte(*b)).collect();
        self.c.eq(own(k, 1), self.c.bytes(&bs));
        k
    }
    fn zero(&mut self, k: usize) -> usize {
        let z = self.c.bit(self.value(k) == Fp::ZERO);
        let inv = self.c.cell(
            Option::<Fp>::from(self.value(k).invert()).unwrap_or(Fp::ZERO),
            255,
        );
        self.c.a.rows.push([own(k, 1), own(z, 1), vec![]]);
        self.c.a.rows.push([
            own(k, 1),
            own(inv, 1),
            plus(&constant(vec![Fp::ONE]), &scaled(&own(z, 1), -Fp::ONE)),
        ]);
        z
    }
    fn equal(&mut self, a: usize, b: usize) -> usize {
        let d = self.c.cell(self.value(a) - self.value(b), 255);
        self.c
            .eq(own(d, 1), plus(&own(a, 1), &scaled(&own(b, 1), -Fp::ONE)));
        self.zero(d)
    }
    fn not(&mut self, a: usize) -> usize {
        let k = self.c.cell(Fp::ONE - self.value(a), 1);
        self.c.eq(
            own(k, 1),
            plus(&constant(vec![Fp::ONE]), &scaled(&own(a, 1), -Fp::ONE)),
        );
        k
    }
    fn mul(&mut self, a: usize, b: usize) -> usize {
        let k = self.c.cell(self.value(a) * self.value(b), 255);
        self.c.a.rows.push([own(a, 1), own(b, 1), own(k, 1)]);
        k
    }
    fn select(&mut self, gate: usize, yes: usize, no: usize) -> usize {
        let k = self.c.cell(
            self.value(no) + self.value(gate) * (self.value(yes) - self.value(no)),
            255,
        );
        self.c.a.rows.push([
            own(gate, 1),
            plus(&own(yes, 1), &scaled(&own(no, 1), -Fp::ONE)),
            plus(&own(k, 1), &scaled(&own(no, 1), -Fp::ONE)),
        ]);
        k
    }
    fn fixed(&mut self, v: Fp) -> usize {
        let k = self.c.cell(v, 255);
        self.c.eq(own(k, 1), constant(vec![v]));
        k
    }
    fn hash(&mut self, domain: u64, inputs: &[usize]) -> usize {
        let values: Vec<_> = inputs.iter().map(|k| self.value(*k)).collect();
        let dg = poseidon::hash_with_domain(domain, &values);
        let outputs = [
            self.c.cell(Fp::ZERO, 255),
            self.c.cell(dg, 255),
            self.c.cell(Fp::ZERO, 255),
        ];
        self.hashes.push(Hash {
            domain,
            inputs: inputs
                .iter()
                .zip(values)
                .map(|(k, v)| (v, own(*k, 1)))
                .collect(),
            outputs,
        });
        outputs[1]
    }
    fn finish(self, w: &mut Wiring<Fp>, family: usize) -> usize {
        let local = w.push(self.c.a);
        let mut rng = SpikeRng::new(b"Receive lineage exact hashes");
        for h in self.hashes {
            certificate::hash_relation(w, family, local, h.domain, &h.inputs, h.outputs, &mut rng);
        }
        local
    }
}

/// The three adjusted fields from the verified wallet predecessor. Core burn
/// and pending roots may be stale and must not be substituted for these ports.
#[derive(Clone, Copy)]
pub struct PreviousLineage {
    pub burned_total: Source,
    pub pending_root: Source,
    pub credit_root: Source,
}

#[derive(Clone)]
pub struct ReceiveMapWitness<const D: usize = 32> {
    pub consumed_search: SearchWitness<D>,
    pub consumed_insert: InsertWitness<D>,
    /// OQ-3's committed structural insertion choice, constrained Boolean.
    pub insert: bool,
    pub credit_search: SearchWitness<D>,
    pub credit_insert: InsertWitness<D>,
}

pub struct ReceiveEffects {
    pub valid: Source,
    pub nonmembership: Source,
    pub burned_total: Source,
    pub pending_root: Source,
    pub credit_root: Source,
}

/// Apply OQ-3 and the exact iff burn rule after deriving consumed absence.
/// `incoming_facts` must be the sealed complete incoming predicate, excluding
/// this function's own authenticated nonmembership predicate. The helper itself
/// does not expose a completed Receive acceptance interface.
#[allow(clippy::too_many_arguments)]
pub fn append_effects<const D: usize>(
    w: &mut Wiring<Fp>,
    family: usize,
    before: &StateCells,
    after: &StateCells,
    request: &RequestCells,
    previous: PreviousLineage,
    payment_digest: Source,
    incoming_facts: Source,
    witness: &ReceiveMapWitness<D>,
) -> ReceiveEffects {
    let search = indexed_map::append_search(w, family, &witness.consumed_search);
    let mut b = Builder::new(family, "Receive exact iff burn and map joins");
    let credit = b.source(w, port(request.local, request.credit));
    let amount = b.source(w, port(request.local, request.words[word::AMOUNT]));
    let sequence = b.source(w, port(after.local, after.core[11]));
    let incoming = b.source(w, incoming_facts);
    b.c.a
        .rows
        .push([own(incoming, 1), own(incoming, 1), own(incoming, 1)]);
    b.c.eq(
        port(search.local, search.root).linear(),
        port(before.local, before.core[17]).linear(),
    );
    b.c.eq(port(search.local, search.key).linear(), own(credit, 1));
    let present = b.source(w, port(search.local, search.present));
    let absent = b.not(present);
    let valid = b.mul(incoming, absent);
    let burned = b.not(valid);
    let insert = b.c.bit(witness.insert);
    b.c.a
        .rows
        .push([own(valid, 1), own(insert, 1), own(valid, 1)]);
    let inserted_key = b.c.cell(witness.consumed_insert.key, 255);
    let inserted_value = b.c.cell(witness.consumed_insert.value, 255);
    let consumed_value = b.hash(CONSUMED_DOMAIN, &[credit, amount, sequence]);
    for (a, c) in [(inserted_key, credit), (inserted_value, consumed_value)] {
        b.c.a.rows.push([
            own(valid, 1),
            plus(&own(a, 1), &scaled(&own(c, 1), -Fp::ONE)),
            vec![],
        ]);
    }
    let payment = b.source(w, payment_digest);
    let no_payment = b.zero(payment);
    b.c.zero(own(no_payment, 1));
    let credit_value = b.hash(CREDIT_DOMAIN, &[credit, payment, burned]);
    let old_burn = b.source(w, previous.burned_total);
    let bounded_old = b.bounded(b.value(old_burn), 16);
    b.c.eq(own(old_burn, 1), own(bounded_old, 1));
    let delta = b.mul(burned, amount);
    let burn = b.bounded(b.value(old_burn) + b.value(delta), 16);
    b.c.eq(own(burn, 1), plus(&own(old_burn, 1), &own(delta, 1)));
    // Core values remain the committed step's values; only adjusted burn changes.
    for i in [10, 18, 19, 20] {
        b.c.eq(
            port(before.local, before.core[i]).linear(),
            port(after.local, after.core[i]).linear(),
        );
    }
    let local = b.finish(w, family);
    let consumed = indexed_map::append_insert_if(
        w,
        family,
        &witness.consumed_insert,
        port(local, insert),
        port(before.local, before.core[17]),
        port(local, inserted_key),
        port(local, inserted_value),
    );
    let (_, record) = indexed_map::append_record_if_absent(
        w,
        family,
        &witness.credit_search,
        &witness.credit_insert,
        previous.credit_root,
        port(local, credit),
        port(local, credit_value),
    );
    let mut joins = Cells::new(family);
    joins.eq(
        port(consumed.local, consumed.new_root).linear(),
        port(after.local, after.core[17]).linear(),
    );
    w.push(joins.a);
    ReceiveEffects {
        valid: port(local, valid),
        nonmembership: port(local, absent),
        burned_total: port(local, burn),
        pending_root: previous.pending_root,
        credit_root: port(record.local, record.new_root),
    }
}

/// An authenticated actual array value at the quoted credential's sequence.
/// Supplying a wrong value/path cannot manufacture an absence verdict.
#[derive(Clone, Debug)]
pub struct PriorCredentialOpening {
    pub digest: Fp,
    pub siblings: [Fp; 16],
}

/// Derive current-or-prior membership with an exact 16-bit array index.
/// Out-of-range noncurrent sequences return false; current credentials may have
/// a later sequence because only replacing them requires array capacity.
pub fn append_credential_continuity(
    w: &mut Wiring<Fp>,
    family: usize,
    current_digest: Source,
    prior_root: Source,
    quoted_digest: Source,
    renewal_sequence: Source,
    opening: &PriorCredentialOpening,
) -> Source {
    let mut b = Builder::new(family, "quoted credential current or prior array");
    let current = b.source(w, current_digest);
    let quoted = b.source(w, quoted_digest);
    let same = b.equal(current, quoted);
    let different = b.not(same);
    let sequence = b.source(w, renewal_sequence);
    let raw = b.value(sequence).to_repr();
    let lo = b.bounded(Fp::from(u16::from_le_bytes([raw[0], raw[1]]) as u64), 2);
    let hi = b.bounded(Fp::from(u16::from_le_bytes([raw[2], raw[3]]) as u64), 2);
    b.c.eq(
        own(sequence, 1),
        plus(&own(lo, 1), &scaled(&own(hi, 1), Fp::from(65536))),
    );
    let fits = b.zero(hi);
    let use_prior = b.mul(different, fits);
    let zero = b.fixed(Fp::ZERO);
    let index = b.select(use_prior, lo, zero);
    let bits: Vec<_> = (0..16)
        .map(|i| {
            b.c.bit(
                (u16::from_le_bytes([raw[0], raw[1]]) >> i) & 1 != 0
                    && b.value(use_prior) == Fp::ONE,
            )
        })
        .collect();
    let packed = bits.iter().enumerate().fold(vec![], |a, (i, k)| {
        plus(&a, &scaled(&own(*k, 1), Fp::from(1u64 << i)))
    });
    b.c.eq(own(index, 1), packed);
    let actual = b.c.cell(opening.digest, 255);
    let leaf_value = b.select(use_prior, actual, zero);
    let mut node = b.hash(PRIOR_LEAF_DOMAIN, &[leaf_value]);
    let mut padding = poseidon::hash_with_domain(PRIOR_LEAF_DOMAIN, &[Fp::ZERO]);
    for (i, bit) in bits.into_iter().enumerate() {
        let supplied = b.c.cell(opening.siblings[i], 255);
        let empty = b.fixed(padding);
        let sibling = b.select(use_prior, supplied, empty);
        let left = b.select(bit, sibling, node);
        let right = b.select(bit, node, sibling);
        node = b.hash(PRIOR_NODE_DOMAIN, &[left, right]);
        padding = poseidon::hash_with_domain(PRIOR_NODE_DOMAIN, &[padding, padding]);
    }
    let actual_root = b.source(w, prior_root);
    let empty_root = b.fixed(padding);
    let expected = b.select(use_prior, actual_root, empty_root);
    b.c.eq(own(node, 1), own(expected, 1));
    let member = b.equal(actual, quoted);
    let prior = b.mul(use_prior, member);
    let valid = b.c.cell(b.value(same) + b.value(prior), 1);
    b.c.eq(own(valid, 1), plus(&own(same, 1), &own(prior, 1)));
    let local = b.finish(w, family);
    port(local, valid)
}

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
    fn finish(mut w: Wiring<Fp>, family: usize, outputs: &[Source]) -> Built {
        let n = outputs.len().next_power_of_two();
        let mut values = vec![Fp::ZERO; n];
        for (i, p) in outputs.iter().enumerate() {
            values[i] = p.value(&w);
        }
        let io = w.push(Array {
            name: "Receive effects outputs",
            family,
            rho: n.trailing_zeros() as usize,
            r_count: outputs.len(),
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![values],
        });
        w.io_arrays.push((io, 256));
        let mut c = Cells::new(family);
        for (i, p) in outputs.iter().enumerate() {
            c.eq(p.linear(), link(io, 0, vec![Some(i)], Fp::ONE));
        }
        w.push(c.a);
        let range = w.push(Array {
            name: "Receive effects range",
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
            "Receive lineage effects",
            arrays,
            pi,
            counts,
            tables,
            copies,
            12,
            512,
            b"Receive lineage effects",
        )
    }
    fn source_array(w: &mut Wiring<Fp>, family: usize, values: &[Fp]) -> usize {
        let local = w.push(Array {
            name: "Receive effect source ports",
            family,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![],
            widths: vec![255; values.len()],
            rows: vec![],
            values: values.iter().map(|v| vec![*v]).collect(),
        });
        // Explicit source ports, not authenticated production state.
        let count = values.len().next_power_of_two();
        let mut io_values = vec![Fp::ZERO; count];
        io_values[..values.len()].copy_from_slice(values);
        let io = w.push(Array {
            name: "Receive effect original inputs",
            family,
            rho: count.trailing_zeros() as usize,
            r_count: values.len(),
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![io_values],
        });
        w.io_arrays.push((io, 128));
        let mut c = Cells::new(family);
        for i in 0..values.len() {
            c.eq(port(local, i).linear(), link(io, 0, vec![Some(i)], Fp::ONE));
        }
        w.push(c.a);
        local
    }
    fn holds(b: &Built) -> bool {
        let (c, i, w) = b;
        let mut v = w.w.clone();
        let r = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut v));
        let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: v });
        let (a, b, c) = r.abc(&c.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }
    fn search_empty<const D: usize>(insert: &InsertWitness<D>, key: Fp) -> SearchWitness<D> {
        SearchWitness {
            root: insert.old_root,
            key,
            leaf: insert.low,
            path: insert.low_path.clone(),
        }
    }
    fn search_inserted<const D: usize>(insert: &InsertWitness<D>, root: Fp) -> SearchWitness<D> {
        SearchWitness {
            root,
            key: insert.key,
            leaf: indexed_map::Leaf {
                key: insert.key,
                value: insert.value,
                next: insert.low.next,
            },
            path: insert.slot.clone(),
        }
    }
    fn maps(
        incoming: bool,
        duplicate: bool,
        insert: bool,
        recorded: bool,
        old_burn: Fp,
        change: impl FnOnce(&mut ReceiveMapWitness<3>, &mut Vec<Fp>),
    ) -> Built {
        let credit = Fp::from(7);
        let amount = Fp::from(100);
        let payment = Fp::from(1234);
        let sequence = Fp::from(6);
        let valid = incoming && !duplicate;
        let descriptor = poseidon::hash_with_domain(CONSUMED_DOMAIN, &[credit, amount, sequence]);
        let (old_insert, old_filled) = indexed_map::empty_insert::<3>(credit, Fp::from(111));
        let old_consumed = if duplicate {
            old_filled
        } else {
            old_insert.old_root
        };
        let key = if valid { credit } else { Fp::from(42) };
        let value = if valid { descriptor } else { Fp::from(999) };
        let (mut consumed, new_consumed) = indexed_map::empty_insert::<3>(key, value);
        consumed.old_root = old_consumed;
        let consumed_search = if duplicate {
            search_inserted(&old_insert, old_filled)
        } else {
            search_empty(&consumed, credit)
        };
        let credit_value = poseidon::hash_with_domain(
            CREDIT_DOMAIN,
            &[credit, payment, Fp::from((!valid) as u64)],
        );
        let (credit_insert, new_credit) = indexed_map::empty_insert::<3>(credit, credit_value);
        let (old_credit_insert, recorded_root) =
            indexed_map::empty_insert::<3>(credit, Fp::from(888));
        let old_credit = if recorded {
            recorded_root
        } else {
            credit_insert.old_root
        };
        let credit_search = if recorded {
            search_inserted(&old_credit_insert, recorded_root)
        } else {
            search_empty(&credit_insert, credit)
        };
        let mut witness = ReceiveMapWitness {
            consumed_search,
            consumed_insert: consumed,
            insert,
            credit_search,
            credit_insert,
        };
        let mut values = vec![Fp::ZERO; 96];
        values[10] = Fp::from(30);
        values[17] = old_consumed;
        values[18] = Fp::from(64);
        values[19] = Fp::from(65);
        values[20] = Fp::from(66);
        values[36 + word::AMOUNT] = amount;
        values[60] = credit;
        for k in 0..30 {
            values[61 + k] = values[k];
        }
        values[61 + 11] = sequence;
        values[61 + 17] = if insert { new_consumed } else { old_consumed };
        values[91] = old_burn;
        values[92] = Fp::from(555);
        values[93] = old_credit;
        values[94] = payment;
        values[95] = Fp::from(incoming as u64);
        change(&mut witness, &mut values);
        let mut w = Wiring::default();
        let f = w.family(0, 1);
        let local = source_array(&mut w, f, &values);
        let before = StateCells {
            local,
            core: std::array::from_fn(|i| i),
            rest: std::array::from_fn(|i| 30 + i),
            rest_digest: 0,
            commitment: 0,
        };
        let after = StateCells {
            local,
            core: std::array::from_fn(|i| 61 + i),
            rest: [0; 6],
            rest_digest: 0,
            commitment: 0,
        };
        let request = RequestCells {
            local,
            words: std::array::from_fn(|i| 36 + i),
            signature: [0; 4],
            message: 0,
            digest: 0,
            credit: 60,
        };
        let e = append_effects(
            &mut w,
            f,
            &before,
            &after,
            &request,
            PreviousLineage {
                burned_total: port(local, 91),
                pending_root: port(local, 92),
                credit_root: port(local, 93),
            },
            port(local, 94),
            port(local, 95),
            &witness,
        );
        let built = finish(
            w,
            f,
            &[
                e.valid,
                e.nonmembership,
                e.burned_total,
                e.pending_root,
                e.credit_root,
            ],
        );
        if (valid || !insert) && old_burn == Fp::from(30) {
            // Output expectations are checked in dedicated positive tests.
            let _ = new_credit;
        }
        built
    }
    #[test]
    fn accept_burn_and_duplicate_record_preservation_share_one_matrix() {
        let accepted = maps(true, false, true, false, Fp::from(30), |_, _| {});
        let burned = maps(false, false, false, false, Fp::from(30), |_, _| {});
        let duplicate = maps(true, true, false, true, Fp::from(30), |_, _| {});
        let entries = accepted.0.entries(&[Fp::from(31), Fp::from(47)]);
        let gens = Gens::<Vesta>::new(accepted.0.shape.cols());
        for b in [&accepted, &burned, &duplicate] {
            assert!(holds(b));
            assert_eq!(entries, b.0.entries(&[Fp::from(31), Fp::from(47)]));
            let (p, _) = spartan::prove_circuit(
                &accepted.0.shape,
                &gens,
                &b.1,
                &b.2,
                &mut SpikeRng::new(b"Receive map proof"),
                &accepted.0,
            );
            assert!(
                spartan::verify_circuit(&accepted.0.shape, &gens, &b.1, &p.to_bytes(), &accepted.0)
                    .is_ok()
            );
        }
        assert_eq!(
            &accepted.1.io[256..260],
            &[Fp::ONE, Fp::ONE, Fp::from(30), Fp::from(555)]
        );
        assert_eq!(
            &burned.1.io[256..260],
            &[Fp::ZERO, Fp::ONE, Fp::from(130), Fp::from(555)]
        );
        assert_eq!(
            &duplicate.1.io[256..260],
            &[Fp::ZERO, Fp::ZERO, Fp::from(130), Fp::from(555)]
        );
        assert_eq!(duplicate.1.io[260], duplicate.1.io[128 + 93]);
    }
    #[test]
    fn oq3_structural_choice_and_exact_accepting_descriptor_are_constraints() {
        assert!(holds(&maps(
            false,
            false,
            true,
            false,
            Fp::from(30),
            |_, _| {}
        )));
        assert!(!holds(&maps(
            true,
            false,
            false,
            false,
            Fp::from(30),
            |_, _| {}
        )));
        for kind in 0..5 {
            let b = maps(
                true,
                false,
                true,
                false,
                Fp::from(30),
                |wit, v| match kind {
                    0 => wit.consumed_search.path.siblings[1] += Fp::ONE,
                    1 => wit.consumed_insert.key += Fp::ONE,
                    2 => wit.consumed_insert.value += Fp::ONE,
                    3 => v[61 + 17] += Fp::ONE,
                    _ => wit.credit_search.path.siblings[1] += Fp::ONE,
                },
            );
            assert!(!holds(&b), "forgery {kind}");
        }
        assert!(!holds(&maps(
            false,
            false,
            false,
            false,
            Fp::from_u128(u128::MAX),
            |_, _| {}
        )));
        assert!(!holds(&maps(
            false,
            false,
            false,
            false,
            Fp::from(30),
            |_, v| v[94] = Fp::ZERO
        )));
    }
    fn prior_open(digest: Fp, index: u16) -> (PriorCredentialOpening, Fp) {
        let mut empty = poseidon::hash_with_domain(PRIOR_LEAF_DOMAIN, &[Fp::ZERO]);
        let mut node = poseidon::hash_with_domain(PRIOR_LEAF_DOMAIN, &[digest]);
        let mut siblings = [Fp::ZERO; 16];
        for i in 0..16 {
            siblings[i] = empty;
            node = if (index >> i) & 1 == 0 {
                poseidon::hash_with_domain(PRIOR_NODE_DOMAIN, &[node, empty])
            } else {
                poseidon::hash_with_domain(PRIOR_NODE_DOMAIN, &[empty, node])
            };
            empty = poseidon::hash_with_domain(PRIOR_NODE_DOMAIN, &[empty, empty]);
        }
        (PriorCredentialOpening { digest, siblings }, node)
    }
    fn prior(
        current: Fp,
        quoted: Fp,
        sequence: u32,
        opening: &PriorCredentialOpening,
        root: Fp,
    ) -> Built {
        let mut w = Wiring::default();
        let f = w.family(0, 1);
        let local = source_array(
            &mut w,
            f,
            &[current, root, quoted, Fp::from(sequence as u64)],
        );
        let valid = append_credential_continuity(
            &mut w,
            f,
            port(local, 0),
            port(local, 1),
            port(local, 2),
            port(local, 3),
            opening,
        );
        finish(w, f, &[valid])
    }
    #[test]
    fn quoted_current_prior_absent_and_out_of_range_have_exact_verdicts() {
        let (open, root) = prior_open(Fp::from(7), 65535);
        let current = prior(Fp::from(7), Fp::from(7), u32::MAX, &open, root);
        let old = prior(Fp::from(8), Fp::from(7), 65535, &open, root);
        let absent = prior(Fp::from(8), Fp::from(9), 65535, &open, root);
        let excessive = prior(Fp::from(8), Fp::from(7), 65536, &open, root);
        let entries = current.0.entries(&[Fp::from(31), Fp::from(47)]);
        for (b, v) in [(&current, 1u64), (&old, 1), (&absent, 0), (&excessive, 0)] {
            assert!(holds(b));
            assert_eq!(b.1.io[256], Fp::from(v));
            assert_eq!(entries, b.0.entries(&[Fp::from(31), Fp::from(47)]));
        }
        let gens = Gens::<Vesta>::new(current.0.shape.cols());
        for b in [&current, &old, &absent, &excessive] {
            let (p, _) = spartan::prove_circuit(
                &current.0.shape,
                &gens,
                &b.1,
                &b.2,
                &mut SpikeRng::new(b"prior credential proof"),
                &current.0,
            );
            assert!(
                spartan::verify_circuit(&current.0.shape, &gens, &b.1, &p.to_bytes(), &current.0)
                    .is_ok()
            );
        }
        let mut bad = open.clone();
        bad.siblings[15] += Fp::ONE;
        assert!(!holds(&prior(Fp::from(8), Fp::from(7), 65535, &bad, root)));
        assert!(!holds(&prior(Fp::from(8), Fp::from(7), 65534, &open, root)));
        let mut bad = open;
        bad.digest += Fp::ONE;
        assert!(!holds(&prior(Fp::from(8), Fp::from(7), 65535, &bad, root)));
    }
}
