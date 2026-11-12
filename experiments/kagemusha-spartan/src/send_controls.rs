//! Exact Send time bounds and ordered quota-array updates, research only.
//!
//! This component joins the current Core30 and completed Request. The caller
//! must authenticate that state and install only signed, sorted, non-overlapping
//! quota shares whose windows exceed the maximum response span. Those upstream
//! installation rules make the two boundary segments an exhaustive coverage
//! proof. Native same-boot anchored-time derivation is a separate boundary.
//! This is not a complete Send relation, fee-policy check or admission decision.

use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};

use crate::{
    certificate::{self, Cells},
    fs::SpikeRng,
    full::Wiring,
    monetary_state::StateCells,
    request::{RequestCells, word},
    uniform::{Lin, constant, link, own, plus, scaled},
};

pub const DEPTH: usize = 6;
pub const WINDOW_DOMAIN: u64 = u64::from_le_bytes(*b"kgwqwin1");
pub const WINDOW_NODE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwqwnd1");
pub const USAGE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwquse1");
pub const USAGE_NODE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwqusn1");

#[derive(Clone, Copy, Debug, Default)]
pub struct Window {
    pub kind: u8,
    pub start: u64,
    pub end: u64,
    pub limit: u128,
}
impl Window {
    pub fn leaf(&self) -> Fp {
        poseidon::hash_with_domain(
            WINDOW_DOMAIN,
            &[
                Fp::from(self.kind as u64),
                Fp::from(self.start),
                Fp::from(self.end),
                Fp::from_u128(self.limit),
            ],
        )
    }
    pub fn usage(&self, used: u128) -> Fp {
        poseidon::hash_with_domain(
            USAGE_DOMAIN,
            &[
                Fp::from(self.kind as u64),
                Fp::from(self.start),
                Fp::from(self.end),
                Fp::from_u128(used),
            ],
        )
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowSlot {
    pub window: Window,
    pub siblings: [Fp; DEPTH],
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Segment {
    /// Position 1, in 0..=64. Positions are base-1, base, base+1, base+2.
    pub base: u8,
    pub slots: [WindowSlot; 4],
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Charge {
    pub slot: u8,
    /// Against the root produced by the preceding active charge.
    pub siblings: [Fp; DEPTH],
    pub used: u128,
}
#[derive(Clone, Debug, Default)]
pub struct QuotaWitness {
    /// Daily then Monthly. Boundaries are derived from base, never witness flags.
    pub segments: [Segment; 2],
    /// Daily positions 1/2, then Monthly positions 1/2, never arbitrary windows.
    pub charges: [Charge; 4],
}

/// Stable cells in `local`. Compose lower/upper into the Send statement, and
/// new_floor/new_usage_root into successor Core30[28]/Core30[21].
pub struct ControlCells {
    pub local: usize,
    pub lower: usize,
    pub upper: usize,
    pub gross: usize,
    pub new_floor: usize,
    pub new_usage_root: usize,
    pub taken: [usize; 4],
}

#[derive(Clone)]
struct Uint {
    cell: usize,
    bytes: Vec<usize>,
}
struct Hash {
    domain: u64,
    inputs: Vec<(Fp, Lin<Fp>)>,
    output: [usize; 3],
}
struct Builder {
    c: Cells,
    hashes: Vec<Hash>,
}
impl Builder {
    fn value(&self, k: usize) -> Fp {
        self.c.a.values[k][0]
    }
    fn cell(&mut self, value: Fp) -> usize {
        self.c.cell(value, 255)
    }
    fn fixed(&mut self, value: u64) -> usize {
        let k = self.cell(Fp::from(value));
        self.c.eq(own(k, 1), constant(vec![Fp::from(value)]));
        k
    }
    fn bounded(&mut self, value: Fp, bytes: usize) -> Uint {
        let cell = self.cell(value);
        let indices: Vec<_> = value.to_repr()[..bytes]
            .iter()
            .map(|b| self.c.byte(*b))
            .collect();
        self.c.eq(own(cell, 1), self.c.bytes(&indices));
        Uint {
            cell,
            bytes: indices,
        }
    }
    fn source(&mut self, w: &Wiring<Fp>, array: usize, source: usize) -> usize {
        let k = self.cell(w.arrays[array].values[source][0]);
        self.c
            .eq(own(k, 1), link(array, source, vec![Some(0)], Fp::ONE));
        k
    }
    fn bounded_source(
        &mut self,
        w: &Wiring<Fp>,
        array: usize,
        source: usize,
        bytes: usize,
    ) -> Uint {
        let u = self.bounded(w.arrays[array].values[source][0], bytes);
        self.c
            .eq(own(u.cell, 1), link(array, source, vec![Some(0)], Fp::ONE));
        u
    }
    fn linear(&mut self, value: Fp, expr: Lin<Fp>) -> usize {
        let k = self.cell(value);
        self.c.eq(own(k, 1), expr);
        k
    }
    fn mul(&mut self, a: usize, b: usize) -> usize {
        let k = self.cell(self.value(a) * self.value(b));
        self.c.a.rows.push([own(a, 1), own(b, 1), own(k, 1)]);
        k
    }
    fn not(&mut self, a: usize) -> usize {
        self.linear(
            Fp::ONE - self.value(a),
            plus(&constant(vec![Fp::ONE]), &scaled(&own(a, 1), -Fp::ONE)),
        )
    }
    fn or(&mut self, a: usize, b: usize) -> usize {
        let ab = self.mul(a, b);
        self.linear(
            self.value(a) + self.value(b) - self.value(ab),
            plus(
                &plus(&own(a, 1), &own(b, 1)),
                &scaled(&own(ab, 1), -Fp::ONE),
            ),
        )
    }
    fn zero(&mut self, a: usize) -> usize {
        let z = self.c.bit(self.value(a) == Fp::ZERO);
        let inv = self.cell(Option::<Fp>::from(self.value(a).invert()).unwrap_or(Fp::ZERO));
        self.c.a.rows.push([own(a, 1), own(z, 1), vec![]]);
        self.c.a.rows.push([
            own(a, 1),
            own(inv, 1),
            plus(&constant(vec![Fp::ONE]), &scaled(&own(z, 1), -Fp::ONE)),
        ]);
        z
    }
    fn equal_constant(&mut self, a: usize, value: u64) -> usize {
        let d = self.linear(
            self.value(a) - Fp::from(value),
            plus(&own(a, 1), &constant(vec![-Fp::from(value)])),
        );
        self.zero(d)
    }
    fn require(&mut self, gate: usize, condition: usize) {
        self.c.a.rows.push([
            own(gate, 1),
            plus(&own(condition, 1), &constant(vec![-Fp::ONE])),
            vec![],
        ]);
    }
    fn gated_eq(&mut self, gate: usize, a: usize, b: usize) {
        self.c.a.rows.push([
            own(gate, 1),
            plus(&own(a, 1), &scaled(&own(b, 1), -Fp::ONE)),
            vec![],
        ]);
    }
    /// Exact integer subtraction with constrained byte differences and borrows.
    fn less(&mut self, a: &Uint, b: &Uint) -> usize {
        assert_eq!(a.bytes.len(), b.bytes.len());
        let mut prior = None;
        let mut borrow = 0i16;
        for (&x, &y) in a.bytes.iter().zip(&b.bytes) {
            let d = self.value(x).to_repr()[0] as i16 - self.value(y).to_repr()[0] as i16 - borrow;
            let digit = self.c.byte(d.rem_euclid(256) as u8);
            let next = self.c.bit(d < 0);
            let mut expr = plus(&own(x, 1), &scaled(&own(y, 1), -Fp::ONE));
            if let Some(k) = prior {
                expr = plus(&expr, &scaled(&own(k, 1), -Fp::ONE));
            }
            expr = plus(&expr, &scaled(&own(next, 1), Fp::from(256)));
            self.c.eq(expr, own(digit, 1));
            prior = Some(next);
            borrow = i16::from(d < 0);
        }
        prior.unwrap()
    }
    fn select(&mut self, gate: usize, yes: usize, no: usize) -> usize {
        let delta = self.linear(
            self.value(yes) - self.value(no),
            plus(&own(yes, 1), &scaled(&own(no, 1), -Fp::ONE)),
        );
        let product = self.mul(gate, delta);
        self.linear(
            self.value(no) + self.value(product),
            plus(&own(no, 1), &own(product, 1)),
        )
    }
    fn hash(&mut self, domain: u64, indices: &[usize]) -> usize {
        let inputs: Vec<_> = indices
            .iter()
            .map(|k| (self.value(*k), own(*k, 1)))
            .collect();
        let digest =
            poseidon::hash_with_domain(domain, &inputs.iter().map(|i| i.0).collect::<Vec<_>>());
        let output = [self.cell(Fp::ZERO), self.cell(digest), self.cell(Fp::ZERO)];
        self.hashes.push(Hash {
            domain,
            inputs,
            output,
        });
        output[1]
    }
    fn bits(&mut self, index: usize) -> [usize; DEPTH] {
        let value = self.value(index).to_repr()[0];
        let bits = std::array::from_fn(|i| self.c.bit((value >> i) & 1 == 1));
        let sum = bits.iter().enumerate().fold(vec![], |acc, (i, k)| {
            plus(&acc, &scaled(&own(*k, 1), Fp::from(1u64 << i)))
        });
        self.c.eq(own(index, 1), sum);
        bits
    }
    fn root(
        &mut self,
        domain: u64,
        mut node: usize,
        bits: &[usize; DEPTH],
        siblings: &[usize; DEPTH],
    ) -> usize {
        for (&bit, &sibling) in bits.iter().zip(siblings) {
            let left = self.select(bit, sibling, node);
            let right = self.linear(
                self.value(node) + self.value(sibling) - self.value(left),
                plus(
                    &plus(&own(node, 1), &own(sibling, 1)),
                    &scaled(&own(left, 1), -Fp::ONE),
                ),
            );
            node = self.hash(domain, &[left, right]);
        }
        node
    }
    fn finish(self, w: &mut Wiring<Fp>, family: usize) -> usize {
        let local = w.push(self.c.a);
        let mut rng = SpikeRng::new(b"exact Send controls");
        for h in self.hashes {
            certificate::hash_relation(w, family, local, h.domain, &h.inputs, h.output, &mut rng);
        }
        local
    }
}

struct Position {
    slot: usize,
    present: usize,
    kind_one: usize,
    kind_two: usize,
    empty: usize,
    of_kind: usize,
    touched: usize,
    start: Uint,
    end: Uint,
    limit: Uint,
}
fn above(b: &mut Builder, kind: u8, p: &Position) -> usize {
    let absent = b.not(p.present);
    let empty_or_absent = b.or(p.empty, absent);
    if kind == 1 {
        b.or(p.kind_two, empty_or_absent)
    } else {
        empty_or_absent
    }
}

fn segment(
    b: &mut Builder,
    kind: u8,
    input: &Segment,
    quota: usize,
    windows_root: usize,
    lower: &Uint,
    upper: &Uint,
) -> [Position; 4] {
    let base = b.bounded(Fp::from(input.base as u64), 1);
    let max = b.bounded(Fp::from(64), 1);
    b.c.eq(own(max.cell, 1), constant(vec![Fp::from(64)]));
    let too_large = b.less(&max, &base);
    b.c.zero(own(too_large, 1));
    let at_zero = b.zero(base.cell);
    let first = b.not(at_zero);
    let at_max = b.equal_constant(base.cell, 64);
    let second = b.not(at_max);
    let c63 = b.bounded(Fp::from(63), 1);
    b.c.eq(own(c63.cell, 1), constant(vec![Fp::from(63)]));
    let c62 = b.bounded(Fp::from(62), 1);
    b.c.eq(own(c62.cell, 1), constant(vec![Fp::from(62)]));
    let third = b.less(&base, &c63);
    let fourth = b.less(&base, &c62);
    let present = [first, second, third, fourth];
    let positions = std::array::from_fn(|i| {
        let present = present[i];
        let slot = &input.slots[i];
        let window = slot.window;
        let shifted = b.linear(
            b.value(base.cell) + Fp::from(i as u64) - Fp::ONE,
            plus(
                &own(base.cell, 1),
                &constant(vec![Fp::from(i as u64) - Fp::ONE]),
            ),
        );
        let index = b.mul(present, shifted);
        let bits = b.bits(index);
        let kind_word = b.cell(Fp::from(window.kind as u64));
        let empty = b.zero(kind_word);
        let one = b.equal_constant(kind_word, 1);
        let two = b.equal_constant(kind_word, 2);
        b.c.eq(
            plus(&plus(&own(empty, 1), &own(one, 1)), &own(two, 1)),
            constant(vec![Fp::ONE]),
        );
        let start = b.bounded(Fp::from(window.start), 8);
        let end = b.bounded(Fp::from(window.end), 8);
        let limit = b.bounded(Fp::from_u128(window.limit), 16);
        let leaf = b.hash(
            WINDOW_DOMAIN,
            &[kind_word, start.cell, end.cell, limit.cell],
        );
        let siblings = slot.siblings.map(|v| b.cell(v));
        let opened = b.root(WINDOW_NODE_DOMAIN, leaf, &bits, &siblings);
        let authenticated = b.mul(quota, present);
        b.gated_eq(authenticated, opened, windows_root);
        let empty = b.mul(present, empty);
        let kind_one = b.mul(present, one);
        let kind_two = b.mul(present, two);
        let of_kind = if kind == 1 { kind_one } else { kind_two };
        let starts_late = b.less(upper, &start);
        let started = b.not(starts_late);
        let not_ended = b.less(lower, &end);
        let intersects = b.mul(started, not_ended);
        let touched = b.mul(of_kind, intersects);
        Position {
            slot: index,
            present,
            kind_one,
            kind_two,
            empty,
            of_kind,
            touched,
            start,
            end,
            limit,
        }
    });
    let [first, second, third, last] = &positions;
    let absent = b.not(first.present);
    let below = if kind == 1 {
        absent
    } else {
        b.or(absent, first.kind_one)
    };
    let lower_before_end = b.less(lower, &first.end);
    let ended = b.not(lower_before_end);
    let ended_kind = b.mul(first.of_kind, ended);
    let pre = b.or(below, ended_kind);
    b.require(quota, pre);
    let last_above = above(b, kind, last);
    let starts_after = b.less(upper, &last.start);
    let late_kind = b.mul(last.of_kind, starts_after);
    let post = b.or(last_above, late_kind);
    b.require(quota, post);
    let second_above = above(b, kind, second);
    let no_kind = b.mul(below, second_above);
    let touched = b.or(second.touched, third.touched);
    let covered = b.or(touched, no_kind);
    b.require(quota, covered);
    positions
}

/// Full depth-6 quota path and coverage constraints. All enabled bits are
/// derived from Core30[22]; no-quota always preserves the old usage root.
/// The shared byte-range table must be present when the caller finishes Wiring.
pub fn append_send_controls(
    w: &mut Wiring<Fp>,
    family: usize,
    state: &StateCells,
    request: &RequestCells,
    lower: u64,
    upper: u64,
    witness: &QuotaWitness,
) -> ControlCells {
    for source in [state.local, request.local] {
        assert_eq!(
            (w.arrays[source].family, w.arrays[source].rho),
            (family, 0),
            "shared global state and Request sources"
        );
    }
    let mut b = Builder {
        c: Cells::new(family),
        hashes: vec![],
    };
    b.c.a.name = "exact Send time and quota controls";
    let mask = b.source(w, state.local, state.core[22]);
    let mask_value = b.value(mask).to_repr()[0];
    let quota = b.c.bit(mask_value & 1 != 0);
    let lease = b.c.bit(mask_value & 2 != 0);
    b.c.eq(
        own(mask, 1),
        plus(&own(quota, 1), &scaled(&own(lease, 1), Fp::from(2))),
    );
    let lower = b.bounded(Fp::from(lower), 8);
    let upper = b.bounded(Fp::from(upper), 8);
    let floor = b.bounded_source(w, state.local, state.core[28], 8);
    let request_time = b.bounded_source(w, request.local, request.words[word::ACCEPTED_TIME], 8);
    for previous in [&floor, &request_time, &lower] {
        let next = if previous.cell == lower.cell {
            &upper
        } else {
            &lower
        };
        let backwards = b.less(next, previous);
        b.c.zero(own(backwards, 1));
    }
    let expiry = b.bounded_source(w, state.local, state.core[24], 8);
    let response = b.bounded_source(w, state.local, state.core[25], 8);
    let lease_expiry = b.bounded_source(w, state.local, state.core[26], 8);
    let valid_lease = b.less(&upper, &lease_expiry);
    b.require(lease, valid_lease);
    let valid_quota = b.less(&upper, &expiry);
    b.require(quota, valid_quota);
    let span = b.bounded(b.value(upper.cell) - b.value(lower.cell), 8);
    b.c.eq(
        own(span.cell, 1),
        plus(&own(upper.cell, 1), &scaled(&own(lower.cell, 1), -Fp::ONE)),
    );
    let excessive = b.less(&response, &span);
    b.c.a.rows.push([own(quota, 1), own(excessive, 1), vec![]]);
    let amount = b.bounded_source(w, request.local, request.words[word::AMOUNT], 16);
    let fee = b.bounded_source(w, request.local, request.words[word::FEE], 16);
    let gross = b.bounded(b.value(amount.cell) + b.value(fee.cell), 16);
    b.c.eq(
        own(gross.cell, 1),
        plus(&own(amount.cell, 1), &own(fee.cell, 1)),
    );
    let windows_root = b.source(w, state.local, state.core[23]);
    let mut root = b.source(w, state.local, state.core[21]);
    let mut taken = [0; 4];
    for (k, kind) in [1u8, 2].into_iter().enumerate() {
        let positions = segment(
            &mut b,
            kind,
            &witness.segments[k],
            quota,
            windows_root,
            &lower,
            &upper,
        );
        for (offset, position) in positions[1..3].iter().enumerate() {
            let index = 2 * k + offset;
            let charge = witness.charges[index];
            let active = b.mul(quota, position.touched);
            taken[index] = active;
            let used = b.bounded(Fp::from_u128(charge.used), 16);
            let after = b.bounded(b.value(used.cell) + b.value(gross.cell), 16);
            b.c.eq(
                own(after.cell, 1),
                plus(&own(used.cell, 1), &own(gross.cell, 1)),
            );
            let exceeded = b.less(&position.limit, &after);
            b.c.a.rows.push([own(active, 1), own(exceeded, 1), vec![]]);
            let kind = b.fixed(kind as u64);
            let old_leaf = b.hash(
                USAGE_DOMAIN,
                &[kind, position.start.cell, position.end.cell, used.cell],
            );
            let new_leaf = b.hash(
                USAGE_DOMAIN,
                &[kind, position.start.cell, position.end.cell, after.cell],
            );
            let slot = b.cell(Fp::from(charge.slot as u64));
            let bits = b.bits(slot);
            b.gated_eq(active, slot, position.slot);
            let siblings = charge.siblings.map(|v| b.cell(v));
            let before = b.root(USAGE_NODE_DOMAIN, old_leaf, &bits, &siblings);
            let after = b.root(USAGE_NODE_DOMAIN, new_leaf, &bits, &siblings);
            b.gated_eq(active, before, root);
            root = b.select(active, after, root);
        }
    }
    let outputs = (lower.cell, upper.cell, gross.cell, root);
    let local = b.finish(w, family);
    ControlCells {
        local,
        lower: outputs.0,
        upper: outputs.1,
        gross: outputs.2,
        new_floor: outputs.0,
        new_usage_root: outputs.3,
        taken,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ipa::Gens,
        nonnative,
        r1cs::{Circuit, Instance, Witness},
        spartan,
        uniform::{Array, UniformCircuit},
    };
    use iroha_pasta::Eq as Vesta;

    type Built = (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>);
    #[derive(Clone)]
    struct Fixture {
        core: [Fp; 30],
        request: [Fp; 24],
        quota: QuotaWitness,
        lower: u64,
        upper: u64,
        expected: Fp,
    }
    fn levels(domain: u64, leaves: &[Fp; 64]) -> Vec<Vec<Fp>> {
        let mut out = vec![leaves.to_vec()];
        for depth in 0..DEPTH {
            out.push(
                out[depth]
                    .chunks_exact(2)
                    .map(|p| poseidon::hash_with_domain(domain, p))
                    .collect(),
            );
        }
        out
    }
    fn siblings(levels: &[Vec<Fp>], slot: usize) -> [Fp; DEPTH] {
        std::array::from_fn(|h| levels[h][(slot >> h) ^ 1])
    }
    fn fixture(lower: u64, upper: u64) -> Fixture {
        let mut windows = [Window::default(); 64];
        windows[..4].copy_from_slice(&[
            Window {
                kind: 1,
                start: 0,
                end: 100,
                limit: 100,
            },
            Window {
                kind: 1,
                start: 100,
                end: 200,
                limit: 100,
            },
            Window {
                kind: 1,
                start: 200,
                end: 300,
                limit: 100,
            },
            Window {
                kind: 2,
                start: 0,
                end: 300,
                limit: 1000,
            },
        ]);
        let tree = levels(WINDOW_NODE_DOMAIN, &windows.map(|w| w.leaf()));
        let mut used = [0u128; 64];
        used[1] = 5;
        used[3] = 7;
        let usage = |used: &[u128; 64]| {
            levels(
                USAGE_NODE_DOMAIN,
                &std::array::from_fn(|i| windows[i].usage(used[i])),
            )
        };
        let initial = usage(&used)[DEPTH][0];
        let mut quota = QuotaWitness::default();
        for (kind, base) in [if lower < 100 { 0u8 } else { 1 }, 3]
            .into_iter()
            .enumerate()
        {
            quota.segments[kind].base = base;
            for pos in 0..4 {
                let slot = base as isize - 1 + pos as isize;
                if (0..64).contains(&slot) {
                    let slot = slot as usize;
                    quota.segments[kind].slots[pos] = WindowSlot {
                        window: windows[slot],
                        siblings: siblings(&tree, slot),
                    };
                }
            }
            for off in 0..2 {
                let slot = base as usize + off;
                let window = windows[slot];
                if window.kind == kind as u8 + 1 && window.start <= upper && lower < window.end {
                    quota.charges[2 * kind + off] = Charge {
                        slot: slot as u8,
                        siblings: siblings(&usage(&used), slot),
                        used: used[slot],
                    };
                    used[slot] += 10;
                }
            }
        }
        let mut core = [Fp::ZERO; 30];
        core[21] = initial;
        core[22] = Fp::from(3);
        core[23] = tree[DEPTH][0];
        core[24] = Fp::from(1000);
        core[25] = Fp::from(20);
        core[26] = Fp::from(1001);
        core[28] = Fp::from(lower.saturating_sub(10));
        let mut request = [Fp::ZERO; 24];
        request[word::AMOUNT] = Fp::from(9);
        request[word::FEE] = Fp::ONE;
        request[word::ACCEPTED_TIME] = Fp::from(lower.saturating_sub(5));
        Fixture {
            core,
            request,
            quota,
            lower,
            upper,
            expected: usage(&used)[DEPTH][0],
        }
    }
    /// Isolated component harness: input cells stand for previously verified
    /// StateCells/RequestCells and are bound to public inputs. Their authentication
    /// is intentionally the enclosing relation's job, not mocked admission.
    fn build(f: &Fixture) -> Built {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let mut source = Cells::new(family);
        let core = f.core.map(|v| source.cell(v, 255));
        let words = f.request.map(|v| source.cell(v, 255));
        let sources: Vec<_> = core.into_iter().chain(words).collect();
        let local = w.push(source.a);
        let state = StateCells {
            local,
            core,
            rest: [0; 6],
            rest_digest: 0,
            commitment: 0,
        };
        let request = RequestCells {
            local,
            words,
            signature: [0; 4],
            message: 0,
            digest: 0,
            credit: 0,
        };
        let result =
            append_send_controls(&mut w, family, &state, &request, f.lower, f.upper, &f.quota);
        let range = w.push(Array {
            name: "Send controls range",
            family,
            rho: 11,
            r_count: 2048,
            fixed: true,
            lookups: vec![],
            widths: vec![11],
            rows: vec![],
            values: vec![(0..2048).map(|i| Fp::from(i as u64)).collect()],
        });
        w.tables.push((range, nonnative::RANGE_TABLE));
        let outputs = [
            result.lower,
            result.upper,
            result.gross,
            result.new_usage_root,
            result.taken[0],
            result.taken[1],
            result.taken[2],
            result.taken[3],
        ];
        assert_eq!(result.new_floor, result.lower);
        let ports: Vec<_> = sources
            .into_iter()
            .map(|cell| (local, cell))
            .chain(outputs.into_iter().map(|cell| (result.local, cell)))
            .collect();
        let mut values: Vec<_> = ports
            .iter()
            .map(|(a, k)| w.arrays[*a].values[*k][0])
            .collect();
        values.resize(64, Fp::ZERO);
        let io = w.push(Array {
            name: "Send controls source and result ports",
            family,
            rho: 6,
            r_count: ports.len(),
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![values],
        });
        w.io_arrays.push((io, 64));
        for (i, (a, k)) in ports.into_iter().enumerate() {
            w.arrays[a].rows.push([
                plus(
                    &own(k, 1),
                    &scaled(&link(io, 0, vec![Some(i)], Fp::ONE), -Fp::ONE),
                ),
                constant(vec![Fp::ONE]),
                vec![],
            ]);
        }
        let (arrays, pi, counts, tables, copies) = w.finish();
        UniformCircuit::build_with_copies(
            "exact Send controls",
            arrays,
            pi,
            counts,
            tables,
            copies,
            14,
            128,
            b"exact Send controls",
        )
    }
    fn holds((c, i, w): &Built) -> bool {
        let mut values = w.w.clone();
        let relation = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut values));
        let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: values });
        let (a, b, c) = relation.abc(&c.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }
    fn fp_hex(s: &str) -> Fp {
        let bytes: Vec<_> = (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect();
        Option::<Fp>::from(Fp::from_repr(bytes.try_into().unwrap())).unwrap()
    }
    #[test]
    fn quota_domains_match_native_padding_vectors() {
        // Canonical fixtures/kagemusha/wallet_v1_vectors.json quota_windows and quota_usage_array.
        let empty = Window::default();
        assert_eq!(
            empty.leaf(),
            fp_hex("03afed9abde5429780e6f99ea98a83cfcf2e2349e1ad008062c157d1a3f57605")
        );
        assert_eq!(
            empty.usage(0),
            fp_hex("4a01af3a62621165e124a1d30960cbf738f64942748a08a7ef6bc7b35ef85321")
        );
        assert_eq!(
            levels(USAGE_NODE_DOMAIN, &[empty.usage(0); 64])[DEPTH][0],
            fp_hex("c302b1f14bb4220f4551e9a1f1d6de8ba45c89e01f57a0ce40c816d27e477830")
        );
    }
    #[test]
    fn exact_time_edges_and_all_source_output_joins() {
        let f = fixture(110, 120);
        let mut b = build(&f);
        assert!(holds(&b));
        assert_eq!(b.1.io[64 + 57], f.expected);
        for port in [
            21,
            22,
            23,
            24,
            25,
            26,
            28,
            30 + word::AMOUNT,
            30 + word::FEE,
            30 + word::ACCEPTED_TIME,
            54,
            55,
            56,
            57,
            58,
            59,
            60,
            61,
        ] {
            b.1.io[64 + port] += Fp::ONE;
            assert!(!holds(&b), "unbound port {port}");
            b.1.io[64 + port] -= Fp::ONE;
        }
        for (index, value) in [(28, 111), (24, 120), (26, 120), (25, 9)] {
            let mut bad = f.clone();
            bad.core[index] = Fp::from(value);
            assert!(!holds(&build(&bad)), "time core {index}");
        }
        let mut bad = f.clone();
        bad.request[word::ACCEPTED_TIME] = Fp::from(111);
        assert!(!holds(&build(&bad)));
        let mut bad = f.clone();
        bad.upper = 109;
        assert!(!holds(&build(&bad)));
        for i in [24, 25, 26, 28] {
            let mut bad = f.clone();
            bad.core[i] = Fp::from_u128(1u128 << 64);
            assert!(!holds(&build(&bad)), "u64 overflow {i}");
        }
        let mut exact = f;
        exact.core[25] = Fp::from(10);
        exact.core[24] = Fp::from(121);
        exact.core[26] = Fp::from(121);
        assert!(holds(&build(&exact)));
    }
    #[test]
    fn every_touched_window_is_charged_in_order() {
        for (lower, upper, taken) in [
            (10, 20, [1u64, 0, 1, 0]),
            (110, 120, [1, 0, 1, 0]),
            (195, 205, [1, 1, 1, 0]),
            (200, 200, [0, 1, 1, 0]),
        ] {
            let f = fixture(lower, upper);
            let b = build(&f);
            assert!(holds(&b), "interval {lower}..{upper}");
            assert_eq!(b.1.io[64 + 57], f.expected);
            assert_eq!(&b.1.io[64 + 58..64 + 62], &taken.map(Fp::from));
        }
        let f = fixture(195, 205);
        let mut skipped = f.clone();
        skipped.quota.charges[1] = Charge::default();
        assert!(!holds(&build(&skipped)));
        let mut reordered = f.clone();
        reordered.quota.charges.swap(0, 1);
        assert!(!holds(&build(&reordered)));
        let mut stale = f.clone();
        stale.quota.charges[2].siblings = f.quota.charges[0].siblings;
        assert!(!holds(&build(&stale)));
        let mut unaligned = f.clone();
        unaligned.quota.charges[2].slot = 1;
        assert!(!holds(&build(&unaligned)));
        let mut overflow = f;
        overflow.quota.charges[3].used = u128::MAX;
        assert!(
            !holds(&build(&overflow)),
            "inactive candidate sum still bounded"
        );
    }
    #[test]
    fn authenticated_boundaries_paths_and_limits_reject_forgeries() {
        let f = fixture(110, 120);
        for (segment, position) in [
            (0, 0),
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 0),
            (1, 1),
            (1, 2),
            (1, 3),
        ] {
            let mut bad = f.clone();
            bad.quota.segments[segment].slots[position].siblings[5] += Fp::ONE;
            assert!(!holds(&build(&bad)), "window opening {segment}/{position}");
        }
        let mut skip = f.clone();
        skip.quota.segments[0].base = 2;
        assert!(!holds(&build(&skip)));
        let mut invalid = f.clone();
        invalid.quota.segments[0].base = 65;
        assert!(!holds(&build(&invalid)));
        let mut kind = f.clone();
        kind.quota.segments[0].slots[1].window.kind = 3;
        assert!(!holds(&build(&kind)));
        for candidate in [0, 2] {
            let mut bad = f.clone();
            bad.quota.charges[candidate].siblings[5] += Fp::ONE;
            assert!(!holds(&build(&bad)));
            let mut bad = f.clone();
            bad.quota.charges[candidate].slot = 64;
            assert!(!holds(&build(&bad)));
        }
        let mut exceeded = f.clone();
        exceeded.request[word::AMOUNT] = Fp::from(100);
        assert!(!holds(&build(&exceeded)));
        let mut untouched = f;
        untouched.lower = 310;
        untouched.upper = 320;
        assert!(!holds(&build(&untouched)), "defined kinds must be touched");
    }
    #[test]
    fn disabled_quota_preserves_root_and_empty_kinds_are_proved() {
        let mut f = fixture(110, 120);
        f.core[22] = Fp::ZERO;
        f.core[24] = Fp::ZERO;
        f.core[25] = Fp::ZERO;
        f.core[26] = Fp::ZERO;
        f.quota = QuotaWitness::default();
        let b = build(&f);
        assert!(holds(&b));
        assert_eq!(b.1.io[64 + 57], f.core[21]);
        assert_eq!(&b.1.io[64 + 58..64 + 62], &[Fp::ZERO; 4]);
        let mut lease = f.clone();
        lease.core[22] = Fp::from(2);
        lease.core[26] = Fp::from(120);
        assert!(!holds(&build(&lease)));
        lease.core[26] = Fp::from(121);
        assert!(holds(&build(&lease)));
        let windows = [Window::default(); 64];
        let tree = levels(WINDOW_NODE_DOMAIN, &windows.map(|w| w.leaf()));
        f.core[22] = Fp::ONE;
        f.core[24] = Fp::from(121);
        f.core[25] = Fp::from(10);
        f.core[23] = tree[DEPTH][0];
        f.core[21] = levels(USAGE_NODE_DOMAIN, &windows.map(|w| w.usage(0)))[DEPTH][0];
        for segment in &mut f.quota.segments {
            for pos in 1..4 {
                segment.slots[pos].siblings = siblings(&tree, pos - 1);
            }
        }
        let b = build(&f);
        assert!(holds(&b));
        assert_eq!(b.1.io[64 + 57], f.core[21]);
    }

    #[test]
    fn all_control_masks_and_boundary_crossings_share_one_proof_matrix() {
        let one = fixture(110, 120);
        let two = fixture(195, 205);
        let mut disabled = one.clone();
        disabled.core[22] = Fp::ZERO;
        disabled.core[24] = Fp::ZERO;
        disabled.core[25] = Fp::ZERO;
        disabled.core[26] = Fp::ZERO;
        disabled.quota = QuotaWitness::default();
        let mut lease = disabled.clone();
        lease.core[22] = Fp::from(2);
        lease.core[26] = Fp::from(121);
        let mut quota_only = two.clone();
        quota_only.core[22] = Fp::ONE;
        quota_only.core[26] = Fp::ZERO;
        let (circuit, _, _) = build(&one);
        let matrix = circuit.entries(&[Fp::from(31), Fp::from(47)]);
        let gens = Gens::<Vesta>::new(circuit.shape.cols());
        for fixture in [&disabled, &lease, &quota_only, &one, &two] {
            let candidate = build(fixture);
            assert!(holds(&candidate));
            assert_eq!(
                (
                    candidate.0.shape.sx,
                    candidate.0.shape.row_bits,
                    candidate.0.shape.col_bits,
                    candidate.0.shape.k,
                ),
                (
                    circuit.shape.sx,
                    circuit.shape.row_bits,
                    circuit.shape.col_bits,
                    circuit.shape.k,
                )
            );
            assert_eq!(candidate.0.entries(&[Fp::from(31), Fp::from(47)]), matrix);
            let (_, instance, witness) = candidate;
            let (proof, _) = spartan::prove_circuit(
                &circuit.shape,
                &gens,
                &instance,
                &witness,
                &mut SpikeRng::new(b"Send controls fixed matrix"),
                &circuit,
            );
            let proof = proof.to_bytes();
            assert!(
                spartan::verify_circuit(&circuit.shape, &gens, &instance, &proof, &circuit).is_ok()
            );
            for port in [57, 58, 59, 60, 61] {
                let mut wrong = instance.clone();
                wrong.io[64 + port] += Fp::ONE;
                assert!(
                    spartan::verify_circuit(&circuit.shape, &gens, &wrong, &proof, &circuit)
                        .is_err(),
                    "forged output port {port}"
                );
            }
        }
    }
}
