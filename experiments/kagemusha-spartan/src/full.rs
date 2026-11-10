//! Stage A2, step 7: the full Receive pair in the uniform layout.
//!
//! - `Wiring`: a circuit under construction (arrays at absolute indices,
//!   families, tables, the copy argument). An export can be reserved before
//!   its array exists, so blocks may read each other in either order.
//! - The permutation pool: every RP57 permutation of a circuit in one family.
//!   The input array gains the generic input cells (absorbed a, b; a Merkle
//!   node's bit, sibling and swap; the predecessor's unused lanes) and four
//!   rows: `s = bit (sib - in0 + s)`, bit boolean, and at nodes (a constant
//!   selector column) `in1 = sib - s`, `in2 = node domain`. The outputs are
//!   one export; each permutation reads, where it applies (mask columns):
//!   chain `(in0 - a, in1 - b, in2)` from a 3-tuple entry (the previous
//!   output, a start state, a leaf input), lane 0 `a` and lane 1 `b` from
//!   one-value entries (absorbed words, or zero), node `(in0 - s, j1, j2)`
//!   from its predecessor's output.
//! - The own relation (Receive), as stage A's `receive_relation`: the
//!   statement sponge (102 permutations, two values absorbed each), two
//!   indexed-tree insertions (6 leaf hashes, 4 Merkle paths of 32 nodes,
//!   r1 = r1b), two signed messages (sponge digests, SHA-256, the comb), 4
//!   comparisons, 10 u128 checks, 5 byte encodings and 1,000 logic triples.

use std::collections::HashMap;

use ff::{Field, PrimeField};
use iroha_pasta::poseidon::{PoseidonField, permute};
use num_bigint::BigUint;

use crate::fs::{Scalar, SpikeRng};
use crate::nonnative::{
    self, Comb, chunk_values, chunk_widths, fp_u, pow2, range_lookups, recompose,
};
use crate::p256;
use crate::sha::{self, Sha};
use crate::uniform::{
    self, Array, ChallengeRows, Copies, Export, Import, Lin, RP_FULL, RP_INPUT, constant, link,
    own, plus, scaled,
};

pub type Fp = iroha_pasta::Fp;
pub type Fq = crate::verifier::Fq;

// ---------------------------------------------------------------- wiring

/// A circuit under construction.
pub struct Wiring<F> {
    pub arrays: Vec<Array<F>>,
    pub pi: Vec<usize>,
    pub counts: Vec<usize>,
    pub tables: Vec<(usize, u32)>,
    exports: Vec<Option<Export<F>>>,
    imports: Vec<Import<F>>,
    /// Second-phase rows over the LogUp challenges.
    pub challenge: ChallengeRows<F>,
    /// Arrays in z's public half and the late io (see `Copies`).
    pub io_arrays: Vec<(usize, usize)>,
    pub late_io: Vec<(usize, usize, usize, usize)>,
}

impl<F: Scalar> Default for Wiring<F> {
    fn default() -> Self {
        Wiring {
            arrays: Vec::new(),
            pi: Vec::new(),
            counts: Vec::new(),
            tables: Vec::new(),
            exports: Vec::new(),
            imports: Vec::new(),
            challenge: ChallengeRows::default(),
            io_arrays: Vec::new(),
            late_io: Vec::new(),
        }
    }
}

impl<F: Scalar> Wiring<F> {
    pub fn family(&mut self, pi: usize, count: usize) -> usize {
        self.pi.push(pi);
        self.counts.push(count);
        self.pi.len() - 1
    }
    pub fn push(&mut self, arr: Array<F>) -> usize {
        self.arrays.push(arr);
        self.arrays.len() - 1
    }
    pub fn reserve(&mut self) -> usize {
        self.exports.push(None);
        self.exports.len() - 1
    }
    pub fn set_export(&mut self, e: usize, array: usize, tuple: Vec<Lin<F>>) {
        assert!(self.exports[e].is_none(), "export {e} set once");
        self.exports[e] = Some(Export { array, tuple });
    }
    pub fn export(&mut self, array: usize, tuple: Vec<Lin<F>>) -> usize {
        let e = self.reserve();
        self.set_export(e, array, tuple);
        e
    }
    /// A read site; skipped when nothing reads.
    pub fn import(&mut self, array: usize, tuple: Vec<Lin<F>>, from: Vec<Option<(usize, usize)>>) {
        if from.iter().any(|f| f.is_some()) {
            self.imports.push(Import { array, tuple, from });
        }
    }
    /// Slots of array `a` (its family's p range times 2^rho).
    pub fn slots(&self, a: usize) -> usize {
        1usize << (self.pi[self.arrays[a].family] + self.arrays[a].rho)
    }
    pub fn copy_sites(&self) -> (usize, usize) {
        (self.exports.len(), self.imports.len())
    }
    pub fn finish(
        self,
    ) -> (
        Vec<Array<F>>,
        Vec<usize>,
        Vec<usize>,
        Vec<(usize, u32)>,
        Copies<F>,
    ) {
        let exports = self
            .exports
            .into_iter()
            .enumerate()
            .map(|(e, x)| x.unwrap_or_else(|| panic!("export {e} set")))
            .collect();
        (
            self.arrays,
            self.pi,
            self.counts,
            self.tables,
            Copies {
                exports,
                imports: self.imports,
                challenge: self.challenge,
                io_arrays: self.io_arrays,
                late_io: self.late_io,
            },
        )
    }
}

/// A global array of constants only: no cells, one export of `tuples` (one
/// instance each).
pub fn constant_entries<F: Scalar>(
    w: &mut Wiring<F>,
    gfam: usize,
    name: &'static str,
    tuples: &[Vec<F>],
) -> usize {
    let n = tuples.len();
    let rho = n.next_power_of_two().trailing_zeros() as usize;
    let a = w.push(Array {
        name,
        family: gfam,
        rho,
        r_count: n,
        fixed: false,
        lookups: Vec::new(),
        widths: Vec::new(),
        rows: Vec::new(),
        values: Vec::new(),
    });
    let width = tuples[0].len();
    let tuple = (0..width)
        .map(|k| {
            constant(
                (0..1usize << rho)
                    .map(|r| tuples.get(r).map_or(F::ZERO, |t| t[k]))
                    .collect(),
            )
        })
        .collect();
    w.export(a, tuple)
}

// ------------------------------------------------------ permutation pool

/// A 3-tuple entry a permutation reads: a pool output, or another export's
/// entry (start states, leaf inputs).
#[derive(Clone, Copy, Debug)]
pub enum Src3 {
    Out(usize),
    Entry(usize, usize),
}

struct Spec<F> {
    input: [F; 3],
    chain: Option<Src3>,
    lanes: [Option<(usize, usize)>; 2],
    node: Option<Src3>,
    /// a, b, bit, sib, s, j1, j2.
    cells: [F; 7],
}

/// Input array cells after the three lanes.
pub const P_A: usize = 3;
pub const P_B: usize = 4;
pub const P_BIT: usize = 5;
pub const P_SIB: usize = 6;
pub const P_S: usize = 7;
pub const P_J1: usize = 8;
pub const P_J2: usize = 9;

pub struct Pool<F> {
    specs: Vec<Spec<F>>,
    outputs: Vec<[F; 3]>,
    domain: F,
}

/// Where the pool's arrays sit.
#[derive(Clone, Copy, Debug)]
pub struct PoolIdx {
    pub input: usize,
    pub full: usize,
    pub family: usize,
    pub kappa: usize,
    pub perms: usize,
}

impl<F: Scalar + PoseidonField> Pool<F> {
    pub fn new(domain: F) -> Self {
        Pool {
            specs: Vec::new(),
            outputs: Vec::new(),
            domain,
        }
    }
    fn run(
        &mut self,
        input: [F; 3],
        chain: Option<Src3>,
        lanes: [Option<(usize, usize)>; 2],
        node: Option<Src3>,
        cells: [F; 7],
    ) -> usize {
        let mut st = input;
        permute(&mut st);
        self.specs.push(Spec {
            input,
            chain,
            lanes,
            node,
            cells,
        });
        self.outputs.push(st);
        self.outputs.len() - 1
    }
    /// input = src + (a, b, 0); each absorbed lane free (`None`) or read from
    /// a one-value entry.
    pub fn chain(
        &mut self,
        src: Src3,
        src_val: [F; 3],
        ab: [F; 2],
        lanes: [Option<(usize, usize)>; 2],
    ) -> usize {
        let input = [src_val[0] + ab[0], src_val[1] + ab[1], src_val[2]];
        self.run(
            input,
            Some(src),
            lanes,
            None,
            [ab[0], ab[1], F::ZERO, F::ZERO, F::ZERO, F::ZERO, F::ZERO],
        )
    }
    /// A Merkle node: (cur + s, sib - s, domain), cur the predecessor's lane
    /// 0 and s = bit (sib - cur).
    pub fn node(&mut self, prev: Src3, prev_val: [F; 3], bit: bool, sib: F) -> usize {
        let cur = prev_val[0];
        let s = if bit { sib - cur } else { F::ZERO };
        let input = [cur + s, sib - s, self.domain];
        self.run(
            input,
            None,
            [None, None],
            Some(prev),
            [
                F::ZERO,
                F::ZERO,
                F::from(bit as u64),
                sib,
                s,
                prev_val[1],
                prev_val[2],
            ],
        )
    }
    pub fn out(&self, p: usize) -> [F; 3] {
        self.outputs[p]
    }
    pub fn len(&self) -> usize {
        self.outputs.len()
    }

    /// Builds the RP57 arrays and the node selector column into `w`, sets the
    /// outputs' export `e_out` and adds the reads.
    pub fn build(self, w: &mut Wiring<F>, e_out: usize, rng: &mut SpikeRng) -> PoolIdx {
        let n = self.specs.len();
        let log = n.next_power_of_two().trailing_zeros() as usize;
        let inputs: Vec<[F; 3]> = self.specs.iter().map(|s| s.input).collect();
        let (mut arrays, pis, outs) = uniform::rp57_with(log, &inputs, rng);
        assert!(outs == self.outputs, "pool outputs");
        let (at, fam) = (w.arrays.len(), w.pi.len());
        uniform::relocate(&mut arrays, at, fam);
        w.pi.extend([pis[0], pis[1]]);
        w.counts.extend([n, 2]);
        let slots = 1usize << log;
        let kap = at + arrays.len();
        {
            let input = &mut arrays[RP_INPUT];
            let mut extra = vec![vec![F::ZERO; slots]; 7];
            for (p, s) in self.specs.iter().enumerate() {
                for (k, v) in s.cells.iter().enumerate() {
                    extra[k][p] = *v;
                }
            }
            input.values.extend(extra);
            input.widths.extend([255, 255, 1, 255, 255, 255, 255]);
            let o = |k: usize| own::<F>(k, 1);
            let neg = |l: Lin<F>| scaled(&l, -F::ONE);
            let kl = link::<F>(kap, 0, vec![Some(0)], F::ONE);
            input.rows.push([
                o(P_BIT),
                plus(&plus(&o(P_SIB), &o(P_S)), &neg(o(0))),
                o(P_S),
            ]);
            input.rows.push([o(P_BIT), o(P_BIT), o(P_BIT)]);
            input.rows.push([
                kl.clone(),
                plus(&plus(&o(1), &neg(o(P_SIB))), &o(P_S)),
                Vec::new(),
            ]);
            input
                .rows
                .push([kl, plus(&o(2), &constant(vec![-self.domain])), Vec::new()]);
        }
        let kappa: Vec<F> = (0..slots)
            .map(|p| {
                if self.specs.get(p).is_some_and(|s| s.node.is_some()) {
                    F::ONE
                } else {
                    F::ZERO
                }
            })
            .collect();
        w.arrays.extend(arrays);
        let kap2 = w.push(Array {
            name: "pool node selector",
            family: fam,
            rho: 0,
            r_count: 1,
            fixed: true,
            lookups: Vec::new(),
            widths: vec![1],
            rows: Vec::new(),
            values: vec![kappa],
        });
        assert_eq!(kap, kap2);
        let mds = F::rp57().mds();
        let lane = |k: usize| -> Lin<F> {
            (0..3).fold(Vec::new(), |acc, j| {
                plus(
                    &acc,
                    &link(at + RP_FULL, 3 * j + 2, vec![Some(7)], mds[k][j]),
                )
            })
        };
        let ip = at + RP_INPUT;
        w.set_export(e_out, ip, vec![lane(0), lane(1), lane(2)]);
        let resolve = |s: Src3| match s {
            Src3::Out(q) => (e_out, q),
            Src3::Entry(e, i) => (e, i),
        };
        let from = |f: &dyn Fn(&Spec<F>) -> Option<(usize, usize)>| -> Vec<Option<(usize, usize)>> {
            (0..slots).map(|p| self.specs.get(p).and_then(f)).collect()
        };
        let o = |k: usize| own::<F>(k, 1);
        let neg = |l: Lin<F>| scaled(&l, -F::ONE);
        w.import(
            ip,
            vec![plus(&o(0), &neg(o(P_A))), plus(&o(1), &neg(o(P_B))), o(2)],
            from(&|s| s.chain.map(resolve)),
        );
        w.import(ip, vec![o(P_A)], from(&|s| s.lanes[0]));
        w.import(ip, vec![o(P_B)], from(&|s| s.lanes[1]));
        w.import(
            ip,
            vec![plus(&o(0), &neg(o(P_S))), o(P_J1), o(P_J2)],
            from(&|s| s.node.map(resolve)),
        );
        PoolIdx {
            input: ip,
            full: at + RP_FULL,
            family: fam,
            kappa: kap,
            perms: n,
        }
    }
}

// ---------------------------------------------------------- own relation

pub const NODE_DOMAIN: u64 = 0x6b67_7769_6d6e_6431;
pub const LEAF_DOMAIN: u64 = 0x6b67_7769_6d6c_6631;
pub const DEPTH: usize = 32;
/// Receive's statement sponge: permutations, as stage A (`operations()`).
pub const STATEMENT_PERMS: usize = 102;

fn lane0(a: Fp, b: Fp, c: Fp) -> Fp {
    let mut st = [a, b, c];
    permute(&mut st);
    st[0]
}

/// A sparse Merkle tree (native, for consistent openings), as stage A.
struct Smt {
    depth: usize,
    nodes: HashMap<(usize, u64), Fp>,
    empty: Vec<Fp>,
}

impl Smt {
    fn new(depth: usize) -> Self {
        let mut empty = vec![Fp::ZERO];
        for l in 0..depth {
            let e = empty[l];
            empty.push(lane0(e, e, Fp::from(NODE_DOMAIN)));
        }
        Smt {
            depth,
            nodes: HashMap::new(),
            empty,
        }
    }
    fn get(&self, level: usize, idx: u64) -> Fp {
        self.nodes
            .get(&(level, idx))
            .copied()
            .unwrap_or(self.empty[level])
    }
    fn set_leaf(&mut self, mut idx: u64, leaf: Fp) {
        self.nodes.insert((0, idx), leaf);
        for l in 0..self.depth {
            let (left, right) = (self.get(l, idx & !1), self.get(l, idx | 1));
            idx >>= 1;
            self.nodes
                .insert((l + 1, idx), lane0(left, right, Fp::from(NODE_DOMAIN)));
        }
    }
    fn path(&self, idx: u64) -> Vec<Fp> {
        (0..self.depth)
            .map(|l| self.get(l, (idx >> l) ^ 1))
            .collect()
    }
    fn root(&self) -> Fp {
        self.get(self.depth, 0)
    }
}

fn leaf(k: Fp, v: Fp, n: Fp) -> Fp {
    lane0(k, v, n + Fp::from(LEAF_DOMAIN))
}

/// Indices of the own relation's parts (tests and the full pair).
pub struct OwnIdx {
    pub pool: PoolIdx,
    pub e_out: usize,
    pub statement: Vec<usize>,
    pub digest_perms: Vec<usize>,
    pub insertion_values: usize,
    pub roots: usize,
    pub sha: usize,
    pub comb: usize,
    pub comparisons: usize,
    pub u128s: usize,
    pub bytes: usize,
    pub logic: usize,
    pub gfam: usize,
}

/// Insertion values: k_low, v_low, next, key, value.
const IV_KL: usize = 0;
const IV_VL: usize = 1;
const IV_NL: usize = 2;
const IV_K: usize = 3;
const IV_V: usize = 4;

/// The own relation into `w`. Its permutations go into `pool`, which the
/// caller builds once every permutation of the circuit is known (`e_out` is
/// the pool's output export, reserved by the caller).
pub fn own_relation(
    w: &mut Wiring<Fp>,
    pool: &mut Pool<Fp>,
    e_out: usize,
    e_zero: usize,
    gfam: usize,
) -> OwnIdx {
    let e_start = constant_entries(
        w,
        gfam,
        "pool start states",
        &[vec![Fp::ZERO, Fp::ZERO, Fp::from(7u64)], vec![Fp::ZERO; 3]],
    );
    // The statement sponge: (0, 0, 7), two values absorbed per permutation.
    let mut statement = Vec::with_capacity(STATEMENT_PERMS);
    for i in 0..STATEMENT_PERMS {
        let ab = [
            Fp::from(1000 + i as u64).square(),
            Fp::from(2000 + i as u64).square(),
        ];
        let q = match statement.last() {
            None => pool.chain(
                Src3::Entry(e_start, 0),
                [Fp::ZERO, Fp::ZERO, Fp::from(7u64)],
                ab,
                [None, None],
            ),
            Some(&prev) => pool.chain(Src3::Out(prev), pool.out(prev), ab, [None, None]),
        };
        statement.push(q);
    }
    // Two indexed-tree insertions, as stage A's `insertion`.
    let insertions = 2;
    let e_leaf = w.reserve();
    let mut ins_vals = vec![vec![Fp::ZERO; 2]; 5];
    let mut root_pairs = Vec::new();
    let leaf_kind = |j: usize| -> [usize; 3] {
        match j {
            0 => [IV_KL, IV_VL, IV_NL],
            1 => [IV_KL, IV_VL, IV_K],
            2 => [IV_K, IV_V, IV_NL],
            _ => [IV_K, IV_V, IV_KL],
        }
    };
    for t in 0..insertions {
        let seed = 11 + t as u64;
        let (lo, hi) = (
            seed.wrapping_mul(0x9e37_79b9) & 0xffff_ffff,
            seed.wrapping_mul(0x85eb_ca6b).wrapping_add(7) & 0xffff_ffff,
        );
        let (k_low, v_low, next) = (
            Fp::from(seed * 1000 + 1),
            Fp::from(seed + 17),
            Fp::from(seed * 1000 + 999),
        );
        let (key, value) = (Fp::from(seed * 1000 + 500), Fp::from(seed + 99));
        for (cell, v) in [
            (IV_KL, k_low),
            (IV_VL, v_low),
            (IV_NL, next),
            (IV_K, key),
            (IV_V, value),
        ] {
            ins_vals[cell][t] = v;
        }
        let vals = [k_low, v_low, next, key, value];
        let leaf_in = |j: usize| -> [Fp; 3] {
            let c = leaf_kind(j);
            [vals[c[0]], vals[c[1]], vals[c[2]] + Fp::from(LEAF_DOMAIN)]
        };
        let leaf_perm = |pool: &mut Pool<Fp>, j: usize| {
            pool.chain(
                Src3::Entry(e_leaf, 6 * t + j),
                leaf_in(j),
                [Fp::ZERO; 2],
                [Some((e_zero, 0)); 2],
            )
        };
        let path_root = |pool: &mut Pool<Fp>,
                         start: Src3,
                         start_val: [Fp; 3],
                         idx: u64,
                         sibs: &[Fp]|
         -> usize {
            let (mut src, mut val, mut last) = (start, start_val, 0);
            for (l, sib) in sibs.iter().enumerate() {
                last = pool.node(src, val, (idx >> l) & 1 == 1, *sib);
                src = Src3::Out(last);
                val = pool.out(last);
            }
            last
        };
        let mut smt = Smt::new(DEPTH);
        smt.set_leaf(lo, leaf(k_low, v_low, next));
        let root0 = smt.root();
        let old = leaf_perm(pool, 0);
        let r0 = path_root(pool, Src3::Out(old), pool.out(old), lo, &smt.path(lo));
        assert_eq!(pool.out(r0)[0], root0, "root before the insertion");
        smt.set_leaf(lo, leaf(k_low, v_low, key));
        let new_low = leaf_perm(pool, 1);
        let r1 = path_root(
            pool,
            Src3::Out(new_low),
            pool.out(new_low),
            lo,
            &smt.path(lo),
        );
        let r1b = path_root(
            pool,
            Src3::Entry(e_start, 1),
            [Fp::ZERO; 3],
            hi,
            &smt.path(hi),
        );
        assert_eq!(pool.out(r1)[0], pool.out(r1b)[0], "r1 = r1b");
        smt.set_leaf(hi, leaf(key, value, next));
        let new_leaf = leaf_perm(pool, 2);
        let r2 = path_root(
            pool,
            Src3::Out(new_leaf),
            pool.out(new_leaf),
            hi,
            &smt.path(hi),
        );
        assert_eq!(pool.out(r2)[0], smt.root(), "root after the insertion");
        for j in 3..6 {
            let _ = leaf_perm(pool, j);
        }
        root_pairs.push((r1, r1b));
    }
    // Insertion values and the leaf inputs (links into them, no cells).
    let ins = w.push(Array {
        name: "insertion values",
        family: gfam,
        rho: 1,
        r_count: insertions,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255; 5],
        rows: Vec::new(),
        values: ins_vals,
    });
    let n_leaf = 6 * insertions;
    let rho_l = n_leaf.next_power_of_two().trailing_zeros() as usize;
    let leaves = w.push(Array {
        name: "leaf inputs",
        family: gfam,
        rho: rho_l,
        r_count: n_leaf,
        fixed: false,
        lookups: Vec::new(),
        widths: Vec::new(),
        rows: Vec::new(),
        values: Vec::new(),
    });
    let pick = |lane: usize, cell: usize| -> Lin<Fp> {
        link(
            ins,
            cell,
            (0..1usize << rho_l)
                .map(|r| (r < n_leaf && leaf_kind(r % 6)[lane] == cell).then_some(r / 6))
                .collect(),
            Fp::ONE,
        )
    };
    let lane_of = |lane: usize| -> Lin<Fp> {
        [IV_KL, IV_VL, IV_NL, IV_K, IV_V]
            .iter()
            .fold(Vec::new(), |acc, c| plus(&acc, &pick(lane, *c)))
    };
    let leaf_dom = constant(
        (0..1usize << rho_l)
            .map(|r| {
                if r < n_leaf {
                    Fp::from(LEAF_DOMAIN)
                } else {
                    Fp::ZERO
                }
            })
            .collect(),
    );
    w.set_export(
        e_leaf,
        leaves,
        vec![lane_of(0), lane_of(1), plus(&lane_of(2), &leaf_dom)],
    );
    // r1 = r1b: two reads of the pool's outputs, lane 0 equal.
    let mut rv = vec![vec![Fp::ZERO; 2]; 6];
    for (t, (r1, r1b)) in root_pairs.iter().enumerate() {
        let (a, b) = (pool.out(*r1), pool.out(*r1b));
        for k in 0..3 {
            rv[k][t] = a[k];
            rv[3 + k][t] = b[k];
        }
    }
    let roots = w.push(Array {
        name: "root equality",
        family: gfam,
        rho: 1,
        r_count: insertions,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255; 6],
        rows: vec![[
            plus(&own(0, 2), &scaled(&own(3, 2), -Fp::ONE)),
            constant(vec![Fp::ONE; 2]),
            Vec::new(),
        ]],
        values: rv,
    });
    w.import(
        roots,
        vec![own(0, 2), own(1, 2), own(2, 2)],
        (0..2)
            .map(|t| root_pairs.get(t).map(|p| (e_out, p.0)))
            .collect(),
    );
    w.import(
        roots,
        vec![own(3, 2), own(4, 2), own(5, 2)],
        (0..2)
            .map(|t| root_pairs.get(t).map(|p| (e_out, p.1)))
            .collect(),
    );
    // Signed messages: two statement digests, SHA-256, the comb.
    let digest_perms = vec![
        statement[STATEMENT_PERMS - 2],
        statement[STATEMENT_PERMS - 1],
    ];
    let messages: Vec<Fp> = digest_perms.iter().map(|p| pool.out(*p)[0]).collect();
    let sigs = messages.len();
    let sfam = w.family(1, sigs);
    let sha_at = w.arrays.len();
    let Sha {
        arrays: sha_arrays,
        tables: sha_tables,
        digests,
    } = sha::sha(sha_at, sfam, 1, gfam, &messages);
    w.arrays.extend(sha_arrays);
    w.tables.extend(sha_tables);
    let n = p256::n();
    let d = p256::scalar_below("receiver-d", &n);
    let q = p256::mul(&p256::generator(), &d).unwrap();
    let inputs: Vec<(p256::Point, BigUint, BigUint, BigUint)> = digests
        .iter()
        .enumerate()
        .map(|(k, dg)| {
            let z = BigUint::from_bytes_be(dg);
            let sig = p256::sign(
                &d,
                &z,
                &p256::scalar_below(&format!("receive-nonce-{k}"), &n),
            );
            (q.clone(), z, sig.r, sig.s)
        })
        .collect();
    let comb_at = w.arrays.len();
    let Comb {
        arrays: comb_arrays,
        tables: comb_tables,
        ..
    } = nonnative::comb(comb_at, sfam, 1, gfam, &inputs);
    w.arrays.extend(comb_arrays);
    w.tables.extend(comb_tables);
    let zs: Vec<BigUint> = inputs.iter().map(|x| x.1.clone()).collect();
    crate::wired::digest_limbs(
        &mut w.arrays,
        sha_at + sha::A_OUT,
        comb_at + nonnative::A_VALUE,
        &zs,
    );
    // m reads a digest's output: (m, j1, j2) at the last message word.
    let msg_a = sha_at + sha::A_MSG;
    let (rho_m, msg_acc) = (w.arrays[msg_a].rho, w.arrays[msg_a].cells() - 1);
    let slots_m = w.slots(msg_a);
    let mut junk = vec![vec![Fp::ZERO; slots_m]; 2];
    let mut from = vec![None; slots_m];
    for (p, perm) in digest_perms.iter().enumerate() {
        let i = (p << rho_m) | 7;
        junk[0][i] = pool.out(*perm)[1];
        junk[1][i] = pool.out(*perm)[2];
        from[i] = Some((e_out, *perm));
    }
    let j0 = w.arrays[msg_a].cells();
    w.arrays[msg_a].values.extend(junk);
    w.arrays[msg_a].widths.extend([255, 255]);
    w.import(
        msg_a,
        vec![
            own(msg_acc, 1 << rho_m),
            own(j0, 1 << rho_m),
            own(j0 + 1, 1 << rho_m),
        ],
        from,
    );
    // The small blocks.
    let comparisons = comparisons(w, gfam);
    let u128s = u128_checks(w, gfam);
    let bytes = byte_encodings(w, gfam);
    let logic = logic_rows(w, gfam);
    OwnIdx {
        pool: PoolIdx {
            input: 0,
            full: 0,
            family: 0,
            kappa: 0,
            perms: 0,
        },
        e_out,
        statement,
        digest_perms,
        insertion_values: ins,
        roots,
        sha: sha_at,
        comb: comb_at,
        comparisons,
        u128s,
        bytes,
        logic,
        gfam,
    }
}

fn field_modulus() -> BigUint {
    BigUint::from_bytes_le((-Fp::ONE).to_repr().as_ref()) + 1u32
}

/// 4 comparisons a < b of canonical field elements, sound over the integers:
/// a, b, d = b - a - 1 and c = p - 1 - b as three limbs each (88, 88, 79 bits)
/// of range-checked 11-bit chunks; a and b equal their limbs (mod p); and the
/// identities a + d + 1 = b and b + c = p - 1 limb by limb with boolean
/// carries (every limb sum stays far below p, so they hold over the
/// integers). Then b <= p - 1 is canonical and a < b. (Stage A's gadget checks
/// b - a - 1 = d only mod p with d < 2^255, which every field element meets:
/// a = p - 1, b = 0 passes.)
fn comparisons(w: &mut Wiring<Fp>, gfam: usize) -> usize {
    let count = 4;
    let rho = 2;
    let n = 1usize << rho;
    let lw = [88usize, 88, 79];
    let cw: Vec<Vec<usize>> = lw.iter().map(|x| chunk_widths(*x)).collect();
    let per = cw.iter().map(|c| c.len()).sum::<usize>();
    // Cells: a, b, carries c0 c1 e0 e1, then four values' chunks (a, b, d, c).
    let (ca, cb_, car) = (0usize, 1usize, 2usize);
    let chunk0 = 6;
    let cells = chunk0 + 4 * per;
    let limb_first =
        |v: usize, k: usize| chunk0 + v * per + cw[..k].iter().map(|c| c.len()).sum::<usize>();
    let p = field_modulus();
    let pm1 = &p - 1u32;
    let limb = |x: &BigUint, k: usize| -> BigUint {
        (x >> (88 * k)) & ((BigUint::from(1u32) << lw[k]) - 1u32)
    };
    let mut v = vec![vec![Fp::ZERO; n]; cells];
    for i in 0..count {
        let a = p256::scalar_below(&format!("cmp-a-{i}"), &(&p >> 1));
        let b = &a + 1u32 + p256::scalar_below(&format!("cmp-d-{i}"), &pow2(200));
        let d = &b - &a - 1u32;
        let c = &pm1 - &b;
        v[ca][i] = fp_u(&a);
        v[cb_][i] = fp_u(&b);
        for (vi, x) in [&a, &b, &d, &c].into_iter().enumerate() {
            for k in 0..3 {
                for (j, val) in chunk_values(&limb(x, k), &cw[k]).into_iter().enumerate() {
                    v[limb_first(vi, k) + j][i] = val;
                }
            }
        }
        let c0 = (limb(&a, 0) + limb(&d, 0) + 1u32) >> 88;
        let c1 = (limb(&a, 1) + limb(&d, 1) + &c0) >> 88;
        let e0 = (limb(&b, 0) + limb(&c, 0)) >> 88;
        let e1 = (limb(&b, 1) + limb(&c, 1) + &e0) >> 88;
        for (k, x) in [c0, c1, e0, e1].iter().enumerate() {
            v[car + k][i] = fp_u(x);
        }
    }
    let ones = vec![Fp::ONE; n];
    let lin = |l: Lin<Fp>| -> [Lin<Fp>; 3] { [l, constant(ones.clone()), Vec::new()] };
    let lv = |vi: usize, k: usize| recompose(limb_first(vi, k), &cw[k], n);
    let c = |x: &BigUint| constant(vec![fp_u(x); n]);
    let o = |k: usize| own::<Fp>(k, n);
    let neg = |l: &Lin<Fp>| scaled(l, -Fp::ONE);
    let two88 = fp_u(&pow2(88));
    let mut rows = Vec::new();
    for (vi, cell) in [(0usize, ca), (1, cb_)] {
        let mut r = o(cell);
        for k in 0..3 {
            r = plus(&r, &scaled(&lv(vi, k), -fp_u(&pow2(88 * k))));
        }
        rows.push(lin(r));
    }
    // a + d + 1 = b, limb by limb (values a 0, b 1, d 2).
    rows.push(lin(plus(
        &plus(
            &plus(&plus(&lv(0, 0), &lv(2, 0)), &c(&BigUint::from(1u32))),
            &neg(&lv(1, 0)),
        ),
        &scaled(&o(car), -two88),
    )));
    rows.push(lin(plus(
        &plus(&plus(&plus(&lv(0, 1), &lv(2, 1)), &o(car)), &neg(&lv(1, 1))),
        &scaled(&o(car + 1), -two88),
    )));
    rows.push(lin(plus(
        &plus(&plus(&lv(0, 2), &lv(2, 2)), &o(car + 1)),
        &neg(&lv(1, 2)),
    )));
    // b + c = p - 1, limb by limb (c is value 3).
    rows.push(lin(plus(
        &plus(&plus(&lv(1, 0), &lv(3, 0)), &neg(&c(&limb(&pm1, 0)))),
        &scaled(&o(car + 2), -two88),
    )));
    rows.push(lin(plus(
        &plus(
            &plus(&plus(&lv(1, 1), &lv(3, 1)), &o(car + 2)),
            &neg(&c(&limb(&pm1, 1))),
        ),
        &scaled(&o(car + 3), -two88),
    )));
    rows.push(lin(plus(
        &plus(&plus(&lv(1, 2), &lv(3, 2)), &o(car + 3)),
        &neg(&c(&limb(&pm1, 2))),
    )));
    for k in 0..4 {
        rows.push([o(car + k), o(car + k), o(car + k)]);
    }
    let mut lookups = Vec::new();
    for vi in 0..4 {
        for k in 0..3 {
            for (j, width) in cw[k].iter().enumerate() {
                range_lookups(&mut lookups, limb_first(vi, k) + j, *width, n);
            }
        }
    }
    let mut widths = vec![255u16, 255, 1, 1, 1, 1];
    for _ in 0..4 {
        widths.extend(cw.iter().flatten().map(|x| *x as u16));
    }
    w.push(Array {
        name: "comparison",
        family: gfam,
        rho,
        r_count: count,
        fixed: false,
        lookups,
        widths,
        rows,
        values: v,
    })
}

/// 10 u128 values, each recomposed from 128-bit range chunks.
fn u128_checks(w: &mut Wiring<Fp>, gfam: usize) -> usize {
    let count = 10;
    let rho = 4;
    let n = 1usize << rho;
    let wd = chunk_widths(128);
    let mut v = vec![vec![Fp::ZERO; n]; 1 + wd.len()];
    for i in 0..count {
        let x = BigUint::from(0x0123_4567_89ab_cdef_u128.wrapping_mul(i as u128 + 3));
        v[0][i] = fp_u(&x);
        for (c, val) in chunk_values(&x, &wd).into_iter().enumerate() {
            v[1 + c][i] = val;
        }
    }
    let rows = vec![[
        plus(&own(0, n), &scaled(&recompose(1, &wd, n), -Fp::ONE)),
        constant(vec![Fp::ONE; n]),
        Vec::new(),
    ]];
    let mut lookups = Vec::new();
    for (c, width) in wd.iter().enumerate() {
        range_lookups(&mut lookups, 1 + c, *width, n);
    }
    let mut widths = vec![128u16];
    widths.extend(wd.iter().map(|x| *x as u16));
    w.push(Array {
        name: "u128 check",
        family: gfam,
        rho,
        r_count: count,
        fixed: false,
        lookups,
        widths,
        rows,
        values: v,
    })
}

/// 5 field elements re-encoded as their 32 canonical bytes: x equals the
/// bytes' sum (mod p), and the bytes as an integer X stay at most p - 1:
/// c = p - 1 - X in three range-checked limbs (88, 88, 79 bits) with
/// X + c = p - 1 limb by limb over boolean carries (every limb sum is far
/// below p, so the identities hold over the integers). Without it a value
/// below 2^256 - p has a second encoding, x + p (stage A's recorded circuit
/// leaves this out; its cost model counts it).
fn byte_encodings(w: &mut Wiring<Fp>, gfam: usize) -> usize {
    let count = 5;
    let rho = 3;
    let n = 1usize << rho;
    let lw = [88usize, 88, 79];
    let cw: Vec<Vec<usize>> = lw.iter().map(|x| chunk_widths(*x)).collect();
    let (e0, chunk0) = (33usize, 35usize);
    let first = |k: usize| chunk0 + cw[..k].iter().map(|c| c.len()).sum::<usize>();
    let cells = chunk0 + cw.iter().map(|c| c.len()).sum::<usize>();
    let pm1 = field_modulus() - 1u32;
    let limb = |x: &BigUint, k: usize| -> BigUint {
        (x >> (88 * k)) & ((BigUint::from(1u32) << lw[k]) - 1u32)
    };
    let mut v = vec![vec![Fp::ZERO; n]; cells];
    for i in 0..count {
        let x = Fp::from(31 + i as u64).square().square();
        v[0][i] = x;
        for (k, byte) in x.to_repr().as_ref().iter().enumerate() {
            v[1 + k][i] = Fp::from(*byte as u64);
        }
        let xi = BigUint::from_bytes_le(x.to_repr().as_ref());
        let c = &pm1 - &xi;
        for k in 0..3 {
            for (j, val) in chunk_values(&limb(&c, k), &cw[k]).into_iter().enumerate() {
                v[first(k) + j][i] = val;
            }
        }
        let c0 = (limb(&xi, 0) + limb(&c, 0)) >> 88;
        let c1 = (limb(&xi, 1) + limb(&c, 1) + &c0) >> 88;
        v[e0][i] = fp_u(&c0);
        v[e0 + 1][i] = fp_u(&c1);
    }
    let ones = vec![Fp::ONE; n];
    let lin = |l: Lin<Fp>| -> [Lin<Fp>; 3] { [l, constant(ones.clone()), Vec::new()] };
    let o = |k: usize| own::<Fp>(k, n);
    let bytes = |lo: usize, hi: usize| {
        (lo..hi).fold(Vec::new(), |acc, k| {
            plus(&acc, &scaled(&o(1 + k), fp_u(&pow2(8 * (k - lo)))))
        })
    };
    let xl = [bytes(0, 11), bytes(11, 22), bytes(22, 32)];
    let cl = |k: usize| recompose(first(k), &cw[k], n);
    let pl = |k: usize| constant(vec![-fp_u(&limb(&pm1, k)); n]);
    let two88 = -fp_u(&pow2(88));
    let rec = (0..32).fold(Vec::new(), |acc, k| {
        plus(&acc, &scaled(&o(1 + k), fp_u(&pow2(8 * k))))
    });
    let rows = vec![
        lin(plus(&o(0), &scaled(&rec, -Fp::ONE))),
        lin(plus(
            &plus(&plus(&xl[0], &cl(0)), &pl(0)),
            &scaled(&o(e0), two88),
        )),
        lin(plus(
            &plus(&plus(&plus(&xl[1], &cl(1)), &o(e0)), &pl(1)),
            &scaled(&o(e0 + 1), two88),
        )),
        lin(plus(&plus(&plus(&xl[2], &cl(2)), &o(e0 + 1)), &pl(2))),
        [o(e0), o(e0), o(e0)],
        [o(e0 + 1), o(e0 + 1), o(e0 + 1)],
    ];
    let mut lookups = Vec::new();
    for k in 0..32 {
        range_lookups(&mut lookups, 1 + k, 8, n);
    }
    for k in 0..3 {
        for (j, width) in cw[k].iter().enumerate() {
            range_lookups(&mut lookups, first(k) + j, *width, n);
        }
    }
    let mut widths = vec![255u16];
    widths.extend([8u16; 32]);
    widths.extend([1u16, 1]);
    widths.extend(cw.iter().flatten().map(|x| *x as u16));
    w.push(Array {
        name: "byte encoding",
        family: gfam,
        rho,
        r_count: count,
        fixed: false,
        lookups,
        widths,
        rows,
        values: v,
    })
}

/// 1,000 logic triples: a, b boolean, ab = a b.
fn logic_rows(w: &mut Wiring<Fp>, gfam: usize) -> usize {
    let count = 1000;
    let rho = 10;
    let n = 1usize << rho;
    let mut v = vec![vec![Fp::ZERO; n]; 3];
    for i in 0..count {
        let (a, b) = ((5 + i) % 3 == 0, (5 + i) % 5 == 0);
        v[0][i] = Fp::from(a as u64);
        v[1][i] = Fp::from(b as u64);
        v[2][i] = Fp::from((a && b) as u64);
    }
    let rows = vec![
        [own(0, n), own(0, n), own(0, n)],
        [own(1, n), own(1, n), own(1, n)],
        [own(0, n), own(1, n), own(2, n)],
    ];
    w.push(Array {
        name: "logic",
        family: gfam,
        rho,
        r_count: count,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![1; 3],
        rows,
        values: v,
    })
}

/// The own relation as a circuit by itself (its pool built here).
pub fn own_circuit() -> (Wiring<Fp>, OwnIdx) {
    let mut w = Wiring::<Fp>::default();
    let gfam = w.family(0, 1);
    let e_out = w.reserve();
    let e_zero = constant_entries(&mut w, gfam, "zero word", &[vec![Fp::ZERO]]);
    let mut pool = Pool::new(Fp::from(NODE_DOMAIN));
    let mut own = own_relation(&mut w, &mut pool, e_out, e_zero, gfam);
    let mut rng = SpikeRng::new(b"own relation pool");
    own.pool = pool.build(&mut w, e_out, &mut rng);
    (w, own)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r1cs::{Circuit, Instance, Witness};
    use crate::uniform::UniformCircuit;
    use iroha_pasta::Eq as Vesta;

    fn accepted(
        c: &UniformCircuit<Fp>,
        inst: &Instance<Fp>,
        gens: &crate::ipa::Gens<Vesta>,
        bad: Vec<Fp>,
    ) -> bool {
        let mut prng = SpikeRng::new(b"full tamper");
        let (proof, _) =
            crate::spartan::prove_circuit(&c.shape, gens, inst, &Witness { w: bad }, &mut prng, c);
        crate::spartan::verify_circuit(&c.shape, gens, inst, &proof.to_bytes(), c).is_ok()
    }

    #[test]
    fn own_relation_holds_proves_and_refuses() {
        let (w, own) = own_circuit();
        assert_eq!(
            own.pool.perms,
            STATEMENT_PERMS + 2 * (6 + 4 * DEPTH),
            "statement sponge and two insertions"
        );
        let (arrays, pi, counts, tables, copies) = w.finish();
        let (c, inst, wit) = UniformCircuit::build_with_copies(
            "own-test",
            arrays,
            pi,
            counts,
            tables,
            copies,
            14,
            4,
            b"own test",
        );
        let shape = c.shape;
        let mut rng = SpikeRng::new(b"own points");
        let ch: Vec<Fp> = vec![rng.scalar(), rng.scalar()];
        let mut w2 = wit.w.clone();
        let rel = c.instantiate(&shape, &ch, Some(&mut w2));
        let z = crate::r1cs::z_vector(&shape, &inst, &Witness { w: w2 });
        let (az, bz, cz) = rel.abc(&shape, &z);
        let bad: Vec<usize> = (0..1 << shape.sx)
            .filter(|x| az[*x] * bz[*x] != cz[*x])
            .collect();
        assert!(
            bad.is_empty(),
            "{} rows fail, first {:?}",
            bad.len(),
            &bad[..bad.len().min(5)]
        );
        let rx: Vec<Fp> = (0..shape.sx).map(|_| rng.scalar()).collect();
        let ry: Vec<Fp> = (0..shape.sz()).map(|_| rng.scalar()).collect();
        let (ex, ey) = (crate::grid::eq_table(&rx), crate::grid::eq_table(&ry));
        let mut evals = [Fp::ZERO; 3];
        for (mat, x, y, v) in c.entries(&ch) {
            evals[mat] += v * ex[x] * ey[y];
        }
        assert_eq!(
            rel.matrix_evals(&shape, &rx, &ry),
            (evals[0], evals[1], evals[2]),
            "compiled evaluation"
        );
        let gens = crate::ipa::Gens::<Vesta>::new(shape.cols());
        let mut prng = SpikeRng::new(b"own prove");
        let (proof, _) = crate::spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut prng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_ok(),
            "honest"
        );
        let p = own.pool;
        let tamper = |a: usize, k: usize, i: usize| -> bool {
            let mut bad = wit.w.clone();
            bad[c.cell_index(a, k, i)] += Fp::ONE;
            accepted(&c, &inst, &gens, bad)
        };
        // An absorbed statement value (the chain read), a node's unused
        // predecessor lane (the node read), an insertion value (the leaf
        // reads), the root equality's copy of r1.
        assert!(
            !tamper(p.input, P_A, own.statement[40]),
            "changed absorbed value refused"
        );
        let node = own.statement.len() + 1 + 5;
        assert!(
            !tamper(p.input, P_J1, node),
            "changed node predecessor lane refused"
        );
        assert!(
            !tamper(own.insertion_values, IV_K, 1),
            "changed insertion value refused"
        );
        assert!(!tamper(own.roots, 0, 0), "changed root copy refused");
        // A comparison's carry, and its b (with b's own limbs kept).
        assert!(
            !tamper(own.comparisons, 2, 1),
            "changed comparison carry refused"
        );
        assert!(
            !tamper(own.comparisons, 1, 2),
            "changed comparison operand refused"
        );
        // A byte encoding's carry, and one limb chunk of its p - 1 - X.
        assert!(
            !tamper(own.bytes, 33, 0),
            "changed byte-encoding carry refused"
        );
        assert!(
            !tamper(own.bytes, 40, 3),
            "changed canonicity chunk refused"
        );
    }
}

// ------------------------------------------------- scalar side, in a circuit

/// Where one proof's scalar side sits, and its outputs' expected values
/// (absolute array indices).
pub struct ScalarIdx<S> {
    pub side: usize,
    /// Slot instances of the slot data array (the slots are the last ones).
    pub slot_instances: usize,
    pub endo: usize,
    pub limbs: usize,
    pub io: usize,
    pub inputs: usize,
    pub expected: Vec<(&'static str, usize, usize, usize, S)>,
}

/// The endo value table (one per circuit): (chunk, its endoscalar share) for
/// 10-bit chunks.
pub fn endo_table<S: Scalar + ff::WithSmallOrderMulGroup<3>>(
    w: &mut Wiring<S>,
    gfam: usize,
) -> usize {
    let t = w.push(Array {
        name: "endo table",
        family: gfam,
        rho: 10,
        r_count: 1 << 10,
        fixed: true,
        lookups: Vec::new(),
        widths: vec![10, 255],
        rows: Vec::new(),
        values: vec![
            (0..1u64 << 10).map(S::from).collect(),
            (0..1usize << 10).map(endo_share::<S>).collect(),
        ],
    });
    w.tables.push((t, crate::wired::ENDO_TABLE));
    t
}

fn pw<S: PrimeField>(e: i64) -> S {
    let x = S::from(2u64).pow_vartime([e.unsigned_abs()]);
    if e >= 0 { x } else { x.invert().unwrap() }
}

/// One 10-bit chunk's share of a Halo endoscalar (wired.rs, any field).
fn endo_share<S: PrimeField + ff::WithSmallOrderMulGroup<3>>(c: usize) -> S {
    let zeta = S::ZETA;
    (0..5).fold(S::ZERO, |acc, i| {
        let (s, e) = (
            S::from(((c >> (2 * i)) & 1) as u64),
            S::from(((c >> (2 * i + 1)) & 1) as u64),
        );
        acc + pw::<S>(i as i64)
            * (s.double() - S::ONE + (zeta - S::ONE).double() * s * e + (S::ONE - zeta) * e)
    })
}

fn endo_constant<S: PrimeField + ff::WithSmallOrderMulGroup<3>>() -> S {
    pw::<S>(64) * (S::ZETA + S::ONE) + pw::<S>(60) * S::from(24u64)
}

/// One proof's scalar side (scalar.rs) placed in `w` (global family `gfam`),
/// its inputs bound to the binding chain's shared array `shared_a` (cell 0)
/// from the proof's first shared value `base`: raw challenges as they are,
/// endo challenges through endo values (chunks looked up in `ENDO_TABLE`),
/// full-width values through limb splits; kappa' and the Schnorr scalars
/// bound the same way. A paired proof's link challenge is not used here.
#[allow(clippy::too_many_arguments)]
pub fn scalar_into<C: iroha_pasta::PastaCurve>(
    w: &mut Wiring<crate::ipa::F<C>>,
    gfam: usize,
    shared_a: usize,
    base: usize,
    shape: &crate::r1cs::Shape,
    inst: &crate::r1cs::Instance<crate::ipa::F<C>>,
    proof: &crate::spartan::Proof<C>,
    claims: &[crate::ipa::Claim<C>],
    tr: &crate::verifier::Trace<C>,
    evals: &dyn Fn(
        &[crate::ipa::F<C>],
        &[crate::ipa::F<C>],
    ) -> (crate::ipa::F<C>, crate::ipa::F<C>, crate::ipa::F<C>),
) -> ScalarIdx<crate::ipa::F<C>>
where
    crate::ipa::F<C>: Scalar,
{
    use crate::ipa::F;
    use crate::scalar;
    use crate::verifier::Kind;
    type S<C> = F<C>;
    let lean = crate::fs::lean_schedule();
    let first = w.arrays.len();
    let side =
        scalar::scalar_side_with::<C>(first, gfam, shape, lean, inst, proof, claims, tr, evals);
    let (q_pad, n_s, expected) = (side.q_pad, side.slot_instances, side.expected);
    w.arrays.extend(side.arrays);
    let values = crate::verifier::shared_values(tr, proof, claims);
    let ch = &tr.challenges;
    let slots = crate::spartan::slots(shape);
    let (mo, ns, l) = (shape.outer_blocks().len(), slots.len(), shape.col_bits);
    let x_of = |s: usize| n_s - ns + s;
    let mut shared_at = Vec::with_capacity(ch.len());
    let mut at = 0;
    for (_, kind, _) in ch {
        shared_at.push(at);
        at += if *kind == Kind::Full { 2 } else { 1 };
    }
    let full_base = at;
    #[derive(Clone, Copy)]
    enum Feed {
        Raw(usize),
        Endo(usize),
        Full(usize),
    }
    let mut next = 0;
    let mut take = |want: Kind| -> usize {
        assert_eq!(ch[next].1, want, "challenge {next}'s kind");
        next += 1;
        next - 1
    };
    let mut endo_ch: Vec<usize> = Vec::new();
    let mut endo_of = |i: usize| -> Feed {
        endo_ch.push(i);
        Feed::Endo(endo_ch.len() - 1)
    };
    let p128 = pw::<S<C>>(128);
    let full = |lo: usize| -> (usize, usize, S<C>) {
        (
            lo,
            lo + 1,
            S::<C>::from_u128(values[lo]) + p128 * S::<C>::from_u128(values[lo + 1]),
        )
    };
    let mut limb_src: Vec<(usize, usize, S<C>)> = Vec::new();
    let mut slot_feed: Vec<[Option<Feed>; 3]> = vec![[None; 3]; n_s];
    let mut tau_feed: Vec<[Option<Feed>; 3]> = vec![[None; 3]; n_s];
    let mut glue_feed: Vec<(usize, Feed)> = Vec::new();
    let mut u_feed: Vec<Feed> = Vec::new();
    // A circuit proof's second-phase challenges come first (after its first
    // phase's rows), then the pair link.
    if let Some(ct) = &tr.circuit {
        for k in 0..ct.phase2.len() {
            glue_feed.push((scalar::PH0 + k, Feed::Raw(shared_at[take(Kind::Raw)])));
        }
    }
    if tr.link.is_some() {
        let _link = take(Kind::Full);
    }
    if lean {
        let i = take(Kind::Full);
        limb_src.push(full(shared_at[i]));
        assert!(limb_src[0].2 == tr.tau[0], "tau from its limbs");
        tau_feed[x_of(0)][3 - slots[0].k] = Some(Feed::Full(0));
    } else {
        for s in 0..mo {
            for v in 0..slots[s].k {
                tau_feed[x_of(s)][3 - slots[s].k + v] = Some(Feed::Raw(shared_at[take(Kind::Raw)]));
            }
        }
    }
    for s in 0..mo {
        for v in 0..if lean { 1 } else { slots[s].k } {
            slot_feed[x_of(s)][3 - slots[s].k + v] = Some(Feed::Raw(shared_at[take(Kind::Raw)]));
        }
    }
    for k in 0..if lean { 1 } else { 3 } {
        glue_feed.push((scalar::R1 + k, Feed::Raw(shared_at[take(Kind::Raw)])));
    }
    let mut g = 0;
    for (s, sl) in slots.iter().enumerate().skip(mo) {
        assert_eq!(sl.first, g, "inner slot order");
        let d0 = 3 - sl.k;
        if lean && g > shape.row_bits {
            slot_feed[x_of(s)][d0] = Some(Feed::Raw(shared_at[take(Kind::Raw)]));
            g += sl.k;
            continue;
        }
        for v in 0..sl.k {
            let feed = if (1..=shape.row_bits).contains(&g) {
                endo_of(take(Kind::Endo))
            } else {
                Feed::Raw(shared_at[take(Kind::Raw)])
            };
            slot_feed[x_of(s)][d0 + v] = Some(feed);
            g += 1;
        }
    }
    glue_feed.push((scalar::ALPHA, Feed::Raw(shared_at[take(Kind::Raw)])));
    glue_feed.push((scalar::GAMMA, endo_of(take(Kind::Endo))));
    if !claims.is_empty() {
        glue_feed.push((scalar::XI, endo_of(take(Kind::Endo))));
    }
    glue_feed.push((scalar::ZETA, endo_of(take(Kind::Endo))));
    for _ in 0..l {
        u_feed.push(endo_of(take(Kind::Endo)));
    }
    let _c = take(Kind::Endo);
    assert_eq!(next, ch.len(), "every challenge placed");
    let sh = crate::verifier::shared_scalars(tr, proof);
    for (pos, cell, want) in [
        (1, scalar::V0, proof.v[0]),
        (2, scalar::V0 + 1, proof.v[1]),
        (3, scalar::V0 + 2, proof.v[2]),
        (4, scalar::KAPPA2, tr.kappa_claims),
        (5, scalar::KZ, sh.kappa_zeta),
        (6, scalar::CZ, sh.c_z1),
        (7, scalar::ZBZ, sh.z1_bf_zeta),
    ] {
        let src = full(full_base + 2 * pos);
        assert!(src.2 == want, "full scalar {pos} from its limbs");
        limb_src.push(src);
        glue_feed.push((cell, Feed::Full(limb_src.len() - 1)));
    }
    // Endo values: the chunks recompose the shared raw value; the value is
    // the constant plus the chunks' shares.
    let endo_bits: Vec<&Vec<bool>> = endo_ch.iter().map(|i| &ch[*i].0).collect();
    let ne = endo_bits.len();
    let rho_e = ne.next_power_of_two().trailing_zeros() as usize;
    let n_e = 1usize << rho_e;
    const CHUNKS: usize = 13;
    let (e_ch, e_ev, e_val) = (0usize, CHUNKS, 2 * CHUNKS);
    let mut ev = vec![vec![S::<C>::ZERO; n_e]; e_val + 1];
    for (e, bits) in endo_bits.iter().enumerate() {
        let raw: u128 = bits
            .iter()
            .enumerate()
            .fold(0, |a, (i, b)| a | ((*b as u128) << i));
        let mut val = endo_constant::<S<C>>();
        for j in 0..CHUNKS {
            let c = ((raw >> (10 * j)) & 1023) as usize;
            ev[e_ch + j][e] = S::<C>::from(c as u64);
            ev[e_ev + j][e] = endo_share(c);
            val += pw::<S<C>>((5 * j) as i64) * endo_share::<S<C>>(c);
        }
        ev[e_val][e] = val;
        assert!(
            val == crate::fs::endo_scalar::<S<C>>(bits),
            "endoscalar from its chunks"
        );
    }
    let endo_shared: Vec<Option<usize>> = (0..n_e)
        .map(|e| endo_ch.get(e).map(|i| base + shared_at[*i]))
        .collect();
    let ones_e = vec![S::<C>::ONE; n_e];
    let raw_sum = (0..CHUNKS).fold(Vec::new(), |acc, j| {
        plus(
            &acc,
            &scaled(&own(e_ch + j, n_e), pw::<S<C>>((10 * j) as i64)),
        )
    });
    let val_sum = (0..CHUNKS).fold(Vec::new(), |acc, j| {
        plus(
            &acc,
            &scaled(&own(e_ev + j, n_e), pw::<S<C>>((5 * j) as i64)),
        )
    });
    let erows = vec![
        [
            plus(
                &link(shared_a, 0, endo_shared, S::<C>::ONE),
                &scaled(&raw_sum, -S::<C>::ONE),
            ),
            constant(ones_e.clone()),
            Vec::new(),
        ],
        [
            plus(
                &plus(&own(e_val, n_e), &scaled(&val_sum, -S::<C>::ONE)),
                &constant(vec![-endo_constant::<S<C>>(); n_e]),
            ),
            constant(ones_e),
            Vec::new(),
        ],
    ];
    let elookups = (0..CHUNKS)
        .map(|j| {
            (
                crate::wired::ENDO_TABLE,
                vec![own(e_ch + j, n_e), own(e_ev + j, n_e)],
            )
        })
        .collect();
    let mut ewidths = vec![10u16; CHUNKS];
    ewidths.extend(vec![255u16; CHUNKS]);
    ewidths.push(255);
    let endo_a = w.push(Array {
        name: "endo value",
        family: gfam,
        rho: rho_e,
        r_count: ne,
        fixed: false,
        lookups: elookups,
        widths: ewidths,
        rows: erows,
        values: ev,
    });
    // Limb splits: value = lo + 2^128 hi of two shared values.
    let nl = limb_src.len();
    let rho_l = nl.next_power_of_two().trailing_zeros() as usize;
    let n_l = 1usize << rho_l;
    let mut lv = vec![vec![S::<C>::ZERO; n_l]];
    for (k, (_, _, val)) in limb_src.iter().enumerate() {
        lv[0][k] = *val;
    }
    let to_shared = |f: &dyn Fn(&(usize, usize, S<C>)) -> usize| -> Vec<Option<usize>> {
        (0..n_l)
            .map(|k| limb_src.get(k).map(|x| base + f(x)))
            .collect()
    };
    let split = plus(
        &plus(
            &own(0, n_l),
            &link(shared_a, 0, to_shared(&|s| s.0), -S::<C>::ONE),
        ),
        &link(shared_a, 0, to_shared(&|s| s.1), -p128),
    );
    let limbs_a = w.push(Array {
        name: "limb split",
        family: gfam,
        rho: rho_l,
        r_count: nl,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255],
        rows: vec![[split, constant(vec![S::<C>::ONE; n_l]), Vec::new()]],
        values: lv,
    });
    // Wiring rows: an input cell (where fed) minus its source.
    let source = |f: Feed| -> (usize, usize, usize) {
        match f {
            Feed::Raw(i) => (shared_a, 0, base + i),
            Feed::Endo(e) => (endo_a, e_val, e),
            Feed::Full(k) => (limbs_a, 0, k),
        }
    };
    let wire = |n: usize, cell: usize, feeds: &[Option<Feed>]| -> [Lin<S<C>>; 3] {
        let mask: Vec<S<C>> = feeds
            .iter()
            .map(|f| {
                if f.is_some() {
                    S::<C>::ONE
                } else {
                    S::<C>::ZERO
                }
            })
            .collect();
        let mut row: Lin<S<C>> = vec![crate::uniform::Term {
            src: crate::uniform::Src::Own(cell),
            coef: mask,
        }];
        for kind in 0..3 {
            let mut src = None;
            let map: Vec<Option<usize>> = feeds
                .iter()
                .map(|f| {
                    f.filter(|f| {
                        matches!(
                            (kind, f),
                            (0, Feed::Raw(_)) | (1, Feed::Endo(_)) | (2, Feed::Full(_))
                        )
                    })
                    .map(|f| {
                        let (a, k, i) = source(f);
                        src = Some((a, k));
                        i
                    })
                })
                .collect();
            if let Some((a, k)) = src {
                row = plus(&row, &link(a, k, map, -S::<C>::ONE));
            }
        }
        [row, constant(vec![S::<C>::ONE; n]), Vec::new()]
    };
    let mut inputs = 0;
    {
        let sd = &mut w.arrays[first + scalar::S_SLOT];
        for (feed, cells) in [(&slot_feed, scalar::T), (&tau_feed, scalar::TAU)] {
            for d in 0..3 {
                let feeds: Vec<Option<Feed>> = (0..n_s).map(|x| feed[x][d]).collect();
                if feeds.iter().any(|f| f.is_some()) {
                    inputs += feeds.iter().filter(|f| f.is_some()).count();
                    sd.rows.push(wire(n_s, cells[d], &feeds));
                }
            }
        }
    }
    {
        let bits = &mut w.arrays[first + scalar::S_BITS];
        let n_b = 1usize << bits.rho;
        let feeds: Vec<Option<Feed>> = (0..n_b)
            .map(|r| (r % q_pad == 0 && r / q_pad < l).then(|| u_feed[r / q_pad]))
            .collect();
        bits.rows.push(wire(n_b, scalar::U, &feeds));
        inputs += l;
    }
    {
        let glue = &mut w.arrays[first + scalar::S_GLUE];
        for (cell, feed) in &glue_feed {
            glue.rows.push(wire(1, *cell, &[Some(*feed)]));
        }
        inputs += glue_feed.len();
    }
    ScalarIdx {
        side: first,
        slot_instances: n_s,
        endo: endo_a,
        limbs: limbs_a,
        io: first + scalar::S_IO,
        inputs,
        expected: expected
            .into_iter()
            .map(|(n, a, k, i, v)| (n, a + first, k, i, v))
            .collect(),
    }
}

// ------------------------------------------------------- the full P-recv

/// One incoming pair, proved with its pairing link and traced.
pub struct TracedPair {
    pub shape_p: crate::r1cs::Shape,
    pub inst_p: crate::r1cs::Instance<Fp>,
    pub claims_p: Vec<crate::ipa::Claim<iroha_pasta::Eq>>,
    pub proof_p: crate::spartan::Proof<iroha_pasta::Eq>,
    pub tr_p: crate::verifier::Trace<iroha_pasta::Eq>,
    pub shape_s: crate::r1cs::Shape,
    pub inst_s: crate::r1cs::Instance<crate::verifier::Fq>,
    pub claims_s: Vec<crate::ipa::Claim<iroha_pasta::Ep>>,
    pub proof_s: crate::spartan::Proof<iroha_pasta::Ep>,
    pub tr_s: crate::verifier::Trace<iroha_pasta::Ep>,
    /// The proofs' accumulated matrix claims (their last io values).
    pub acc_p: crate::deferral::Claim<Fp>,
    pub acc_s: crate::deferral::Claim<crate::verifier::Fq>,
}

/// Generators of the two proof shapes (P: Vesta 2^14, S: Pallas 2^13).
pub struct PairGens {
    pub p: crate::ipa::Gens<iroha_pasta::Eq>,
    pub s: crate::ipa::Gens<iroha_pasta::Ep>,
    /// The combined matrix polynomials of the P kinds (P-recv, P-send) and
    /// the S kinds (S-recv, S-send) the accumulators are claims on.
    pub defer_p: crate::deferral::MatPoly<Fp>,
    pub defer_s: crate::deferral::MatPoly<crate::verifier::Fq>,
    /// The base case: the predecessor's accumulator replaced by the genesis
    /// claim (a public flag and value; see `defer_into`).
    pub genesis: bool,
}

/// A toy circuit kind of the combined matrix polynomial.
fn toy_kind<S: Scalar>(shape: crate::r1cs::Shape) -> crate::deferral::Kind<S> {
    crate::deferral::Kind {
        sx: shape.sx,
        sz: shape.sz(),
        deg: (0, 0),
        eval: Box::new(move |_: &[S], rx: &[S], ry: &[S]| {
            crate::r1cs::matrix_evals(&shape, rx, ry)
        }),
        eval_many: None,
    }
}

/// A stand-in for an earlier hop's accumulator: a random point and its true
/// value.
fn sample_acc<S: Scalar>(
    poly: &crate::deferral::MatPoly<S>,
    seed: &[u8],
) -> crate::deferral::Claim<S> {
    let mut rng = SpikeRng::new(seed);
    let point: Vec<S> = (0..poly.n()).map(|_| rng.scalar()).collect();
    crate::deferral::Claim {
        value: poly.eval(&point),
        point,
    }
}

/// Receive's two incoming pairs as `real-receive` builds them: the
/// predecessor Receive (P-recv, S-recv, two inherited claims) and the payer's
/// Send (P-send, S-send, one).
pub fn receive_pairs() -> (PairGens, Vec<TracedPair>) {
    use iroha_pasta::{Ep as Pallas, Eq as Vesta};
    let specs = toy_specs();
    let (dp, ds) = FP_DIMS;
    let defer_p = crate::deferral::MatPoly::with_min(
        [toy_kind(specs[0].0), toy_kind(specs[1].0)],
        dp.0,
        dp.1,
    );
    let defer_s = crate::deferral::MatPoly::with_min(
        [toy_kind(specs[0].1), toy_kind(specs[1].1)],
        ds.0,
        ds.1,
    );
    let gens = PairGens {
        p: crate::ipa::Gens::<Vesta>::new(1 << 14),
        s: crate::ipa::Gens::<Pallas>::new(1 << 13),
        defer_p,
        defer_s,
        genesis: false,
    };
    let out = (0..2).map(|i| toy_pair(&gens, i)).collect();
    (gens, out)
}

/// The accumulators' matrix dimensions (x, y) on the P and S sides, so that
/// every hop's accumulators share one space: P-recv verifying its own 64-row
/// proofs packs past 2^19 constraint rows (2^20, 2^21 z entries), S-recv
/// stays at 2^19 and 2^20.
pub const FP_DIMS: ((usize, usize), (usize, usize)) = ((20, 21), (19, 20));

type ToySpec = (crate::r1cs::Shape, crate::r1cs::Shape, usize, &'static [u8]);

/// Receive's two incoming pairs' toy shapes: the predecessor Receive, the
/// payer's Send (P, S, inherited claims, seed).
fn toy_specs() -> [ToySpec; 2] {
    let shape = |label: &'static str, sx, row_bits, col_bits| crate::r1cs::Shape {
        label,
        sx,
        row_bits,
        col_bits,
        k: 3,
    };
    [
        (
            shape("P-recv", 19, 5, 14),
            shape("S-recv", 18, 5, 13),
            2usize,
            &b"pred-receive"[..],
        ),
        (
            shape("P-send", 19, 5, 14),
            shape("S-send", 17, 4, 13),
            1,
            &b"payer-send"[..],
        ),
    ]
}

/// Toy pair `i` (0 the predecessor Receive, 1 the payer's Send), its io
/// ending with a stand-in accumulator on `gens`' polynomials.
fn toy_pair(gens: &PairGens, i: usize) -> TracedPair {
    use crate::spartan::Link;
    use crate::verifier::{PairSpec, instance, prove_pair, rlc_value, trace};
    use iroha_pasta::{Ep as Pallas, Eq as Vesta};
    let (sp, ss, inh, seed) = toy_specs()[i];
    // Each proof's io ends with its accumulated matrix claim (point, then
    // value). A stand-in: production carries it as io absorbed after the
    // producing proof's second-phase challenge (it depends on it); the cost
    // of binding it, the io digest's absorptions and the io fold's leaves, is
    // the same.
    let acc_p = sample_acc(&gens.defer_p, &[seed, b"/acc-p"].concat());
    let acc_s = sample_acc(&gens.defer_s, &[seed, b"/acc-s"].concat());
    let io_p = crate::census::STATEMENT_IO + inh * (2 + sp.col_bits);
    let io_s = crate::census::STATEMENT_IO + inh * (2 + ss.col_bits);
    let (mut inst_p, wit_p, claims_p) = instance::<Vesta>(
        sp,
        &gens.p,
        inh,
        io_p + acc_p.point.len() + 1,
        &[seed, b"/p"].concat(),
    );
    let (mut inst_s, wit_s, claims_s) = instance::<Pallas>(
        ss,
        &gens.s,
        inh,
        io_s + acc_s.point.len() + 1,
        &[seed, b"/s"].concat(),
    );
    for (k, v) in acc_p.point.iter().chain([&acc_p.value]).enumerate() {
        inst_p.io[io_p + k] = *v;
    }
    for (k, v) in acc_s.point.iter().chain([&acc_s.value]).enumerate() {
        inst_s.io[io_s + k] = *v;
    }
    let spec = PairSpec {
        shape_p: sp,
        gens_p: &gens.p,
        inst_p,
        wit_p,
        claims_p,
        shape_s: ss,
        gens_s: &gens.s,
        inst_s,
        wit_s,
        claims_s,
    };
    let pr = prove_pair(&spec, &[b"pair", &[i as u8][..]].concat(), &|rho| {
        rlc_value(rho, crate::verifier::Fq::from(3), &[1, 2, 3]).to_repr()
    });
    let r = pr.r;
    let fixed = move |_: &[u8; 32]| r;
    let tr_p = trace(
        &sp,
        &gens.p,
        &spec.inst_p,
        &pr.p,
        &spec.claims_p,
        &Link::First {
            rho: pr.rho,
            r: pr.r,
        },
    );
    let tr_s = trace(
        &ss,
        &gens.s,
        &spec.inst_s,
        &pr.s,
        &spec.claims_s,
        &Link::Second {
            d: pr.d,
            rlc: &fixed,
        },
    );
    assert!(tr_p.accepts && tr_s.accepts, "the pair verifies natively");
    TracedPair {
        shape_p: sp,
        inst_p: spec.inst_p,
        claims_p: spec.claims_p,
        proof_p: pr.p,
        tr_p,
        shape_s: ss,
        inst_s: spec.inst_s,
        claims_s: spec.claims_s,
        proof_s: pr.s,
        tr_s,
        acc_p,
        acc_s,
    }
}

/// A uniform kind of the combined matrix polynomial: the circuit's compiled
/// matrix evaluation at its second-phase challenges, one point at a time or
/// batched over lanes (native q).
pub fn uniform_kind<S: Scalar>(
    c: std::sync::Arc<crate::uniform::UniformCircuit<S>>,
) -> crate::deferral::Kind<S> {
    let (sx, sz, deg) = (c.shape.sx, c.shape.sz(), c.challenge_degrees());
    let many = c.clone();
    crate::deferral::Kind {
        sx,
        sz,
        deg,
        eval: Box::new(move |ch: &[S], rx: &[S], ry: &[S]| c.evals(rx, ry, ch)),
        eval_many: Some(Box::new(move |pts: &[(&[S], &[S], &[S])]| {
            many.evals_many(pts)
        })),
    }
}

/// A proved uniform pair: the circuits, their instances (early io), the
/// linked proofs and the link's words.
pub struct UniformPair {
    pub cp: std::sync::Arc<crate::uniform::UniformCircuit<Fp>>,
    pub ip: crate::r1cs::Instance<Fp>,
    pub pp: crate::spartan::Proof<iroha_pasta::Eq>,
    pub cs: std::sync::Arc<crate::uniform::UniformCircuit<crate::verifier::Fq>>,
    pub is: crate::r1cs::Instance<crate::verifier::Fq>,
    pub ps: crate::spartan::Proof<iroha_pasta::Ep>,
    pub d: [u8; 32],
    pub rho: [u8; 32],
    pub r: [u8; 32],
}

/// The next hop's incoming pairs: the uniform pair (as the predecessor
/// Receive, traced as circuit proofs, its accumulators its late io) and a
/// fresh toy Send pair; the polynomials' kinds are the uniform circuits and
/// the toy Send shapes.
pub fn next_pairs(u: UniformPair) -> (PairGens, Vec<TracedPair>) {
    // TODO: replace the toy Send predecessor with the complete uniform
    // monetary Send circuit before any production integration.
    use crate::spartan::Link;
    use crate::verifier::trace_circuit;
    use iroha_pasta::{Ep as Pallas, Eq as Vesta};
    let specs = toy_specs();
    let (dp, ds) = FP_DIMS;
    let defer_p = crate::deferral::MatPoly::with_min(
        [uniform_kind(u.cp.clone()), toy_kind(specs[1].0)],
        dp.0,
        dp.1,
    );
    let defer_s = crate::deferral::MatPoly::with_min(
        [uniform_kind(u.cs.clone()), toy_kind(specs[1].1)],
        ds.0,
        ds.1,
    );
    assert_eq!(
        (defer_p.x, defer_p.y, defer_s.x, defer_s.y),
        (dp.0, dp.1, ds.0, ds.1),
        "the uniform pair has the accumulators' dimensions"
    );
    let gens = PairGens {
        p: crate::ipa::Gens::<Vesta>::new(u.cp.shape.cols()),
        s: crate::ipa::Gens::<Pallas>::new(u.cs.shape.cols()),
        defer_p,
        defer_s,
        genesis: false,
    };
    let fixed_p = crate::spartan::fixed_commitments(&u.cp.shape, &gens.p, u.cp.as_ref());
    let fixed_s = crate::spartan::fixed_commitments(&u.cs.shape, &gens.s, u.cs.as_ref());
    let r = u.r;
    let rlc = move |_: &[u8; 32]| r;
    let tr_p = trace_circuit(
        &u.cp.shape,
        &gens.p,
        &u.ip,
        &u.pp,
        &[],
        &Link::First { rho: u.rho, r: u.r },
        u.cp.as_ref(),
        &fixed_p,
    );
    let tr_s = trace_circuit(
        &u.cs.shape,
        &gens.s,
        &u.is,
        &u.ps,
        &[],
        &Link::Second { d: u.d, rlc: &rlc },
        u.cs.as_ref(),
        &fixed_s,
    );
    assert!(
        tr_p.accepts && tr_s.accepts,
        "the uniform pair's traces accept"
    );
    let pred = TracedPair {
        shape_p: u.cp.shape,
        acc_p: crate::deferral::claim_of(&u.pp.late),
        inst_p: u.ip,
        claims_p: Vec::new(),
        proof_p: u.pp,
        tr_p,
        shape_s: u.cs.shape,
        acc_s: crate::deferral::claim_of(&u.ps.late),
        inst_s: u.is,
        claims_s: Vec::new(),
        proof_s: u.ps,
        tr_s,
    };
    let send = toy_pair(&gens, 1);
    (gens, vec![pred, send])
}

/// Where P-recv's parts sit.
pub struct PRecvIdx {
    pub own: OwnIdx,
    pub points: crate::points::PointsIdx,
    pub scalars: Vec<ScalarIdx<Fp>>,
    pub shared: usize,
    pub values: usize,
    pub io_digest: usize,
    pub defer: DeferIdx<Fp>,
}

/// P-recv in the uniform layout: the own relation; for each incoming pair the
/// P proof's scalar side (with its io digest) and the S proof's points side;
/// the binding chain over every shared value; the fold of the verified P
/// proofs' matrix claims. One permutation pool holds the statement sponge,
/// the insertions, both S transcripts and both io digests.
pub fn p_recv(gens: &PairGens, pairs: &[TracedPair]) -> (Wiring<Fp>, PRecvIdx) {
    use crate::points::{PointsBuilder, SharedMap};
    use iroha_pasta::Ep as Pallas;
    let mut w = Wiring::<Fp>::default();
    let gfam = w.family(0, 1);
    let e_out = w.reserve();
    let e_v = w.reserve();
    let e_zero = constant_entries(&mut w, gfam, "zero word", &[vec![Fp::ZERO]]);
    let mut pool = Pool::new(Fp::from(NODE_DOMAIN));
    let own_idx = own_relation(&mut w, &mut pool, e_out, e_zero, gfam);
    // Shared values: per pair, the P proof's then the S proof's.
    let (values, offsets) = binding_values(pairs);
    // The S proofs' points sides.
    let mut pb = PointsBuilder::<Pallas>::new(&mut w, e_out, e_v, e_zero);
    for (pr, &(off_p, off_s, len_p)) in pairs.iter().zip(&offsets) {
        let w = off_p + link_word_at(&pr.tr_p);
        let sm = SharedMap {
            base: off_s,
            other: Some([w, w + 1]),
            r: Some([off_p + len_p - 2, off_p + len_p - 1]),
        };
        pb.verify(
            &mut pool,
            &pr.shape_s,
            &gens.s,
            &pr.proof_s,
            &pr.claims_s,
            &pr.tr_s,
            sm,
        );
    }
    // The binding chain (rho and mu stand-ins, as stage A).
    let (rho, mu) = binding_rho_mu();
    let first = w.arrays.len();
    let b = nonnative::binding_chain(
        first,
        gfam,
        gfam,
        &BigUint::from_bytes_le(rho.to_repr().as_ref()),
        &BigUint::from_bytes_le(mu.to_repr().as_ref()),
        &values,
    );
    w.arrays.extend(b.arrays);
    w.tables.extend(b.tables);
    let shared = first + nonnative::B_SHARED;
    let n_sh = 1usize << w.arrays[shared].rho;
    w.set_export(e_v, shared, vec![own(0, n_sh)]);
    // The P proofs' scalar sides and their io digests.
    let _table = endo_table(&mut w, gfam);
    let mut scalars = Vec::new();
    for (i, (pr, &(off_p, _, _))) in pairs.iter().zip(&offsets).enumerate() {
        let ch2: Vec<Fp> = pr
            .tr_p
            .circuit
            .as_ref()
            .map_or(vec![Fp::ZERO; 2], |c| c.phase2.clone());
        let kind = &gens.defer_p.kinds[i];
        let evals = |rx: &[Fp], ry: &[Fp]| (kind.eval)(&ch2, rx, ry);
        scalars.push(scalar_into::<iroha_pasta::Eq>(
            &mut w,
            gfam,
            shared,
            off_p,
            &pr.shape_p,
            &pr.inst_p,
            &pr.proof_p,
            &pr.claims_p,
            &pr.tr_p,
            &evals,
        ));
    }
    let items: Vec<IoDigest<Fp>> = pairs
        .iter()
        .zip(&offsets)
        .zip(&scalars)
        .flat_map(|((pr, &(off_p, _, _)), sc)| {
            let mut v = vec![IoDigest {
                io: &pr.inst_p.io,
                limb: io_limb(&pr.tr_p, off_p),
                fold: sc.io,
                start: 0,
            }];
            if pr
                .tr_p
                .circuit
                .as_ref()
                .is_some_and(|c| c.late_digest.is_some())
            {
                v.push(IoDigest {
                    io: &pr.proof_p.late,
                    limb: late_limb(&pr.tr_p, &pr.claims_p, off_p),
                    fold: sc.io,
                    start: 64,
                });
            }
            v
        })
        .collect();
    let io_digest = io_digests(&mut w, &mut pool, gfam, e_out, e_zero, shared, &items);
    // The verified P proofs' matrix claims, deferred: folded into P-recv's
    // accumulator (no in-circuit matrix evaluation).
    let proofs: Vec<_> = pairs
        .iter()
        .zip(&scalars)
        .map(|(pr, sc)| (&pr.shape_p, &pr.inst_p, &pr.tr_p, sc, &pr.acc_p))
        .collect();
    let defer = defer_into::<iroha_pasta::Eq>(&mut w, gfam, &gens.defer_p, &proofs, gens.genesis);
    let mut rng = SpikeRng::new(b"p-recv pool");
    let pidx = pool.build(&mut w, e_out, &mut rng);
    let points = pb.build(&mut w, gfam, shared, false);
    let mut seen = std::collections::HashSet::new();
    w.tables.retain(|(_, t)| seen.insert(*t));
    let mut own_idx = own_idx;
    own_idx.pool = pidx;
    (
        w,
        PRecvIdx {
            own: own_idx,
            points,
            scalars,
            shared,
            values: values.len(),
            io_digest,
            defer,
        },
    )
}

/// The deferred matrix claims of the verified proofs whose scalar sides a
/// circuit holds (the pairs' P proofs in P-recv, their S proofs in S-recv),
/// folded into the circuit's accumulator.
pub struct DeferIdx<S> {
    pub fold: crate::deferral::FoldArrays,
    /// The io array of the folded claim (late io from io index 64 on).
    pub late: usize,
    /// The four claims in curve order: proof 0's accumulator and fresh claim,
    /// proof 1's.
    pub claims: [crate::deferral::Claim<S>; 4],
    pub q: Vec<S>,
    /// Seconds to compute q natively (deg q + 1 evaluations of P).
    pub q_secs: f64,
}

/// Per verified proof (kind = its index): the incoming accumulator, read
/// from the last `n + 1` leaves of its scalar side's io fold, and the fresh
/// claim: kind, `r` (glue R1), `r_x` and `r_y` (the slot data's challenge
/// digits) and `r A + r^2 B + r^3 C` (glue M1 + M2 + M3, the value its check
/// uses). Links carry all of them: the scalar sides share the global family.
#[allow(clippy::type_complexity)]
pub fn defer_into<C: iroha_pasta::PastaCurve>(
    w: &mut Wiring<crate::ipa::F<C>>,
    gfam: usize,
    poly: &crate::deferral::MatPoly<crate::ipa::F<C>>,
    proofs: &[(
        &crate::r1cs::Shape,
        &crate::r1cs::Instance<crate::ipa::F<C>>,
        &crate::verifier::Trace<C>,
        &ScalarIdx<crate::ipa::F<C>>,
        &crate::deferral::Claim<crate::ipa::F<C>>,
    )],
    genesis: bool,
) -> DeferIdx<crate::ipa::F<C>>
where
    crate::ipa::F<C>: Scalar,
{
    use crate::deferral::{self, C_P, Claim, V_V};
    use crate::scalar;
    type S<C> = crate::ipa::F<C>;
    assert!(
        crate::fs::lean_schedule(),
        "the claim P(r, r_x, r_y) needs r_A, r_B, r_C = r, r^2, r^3"
    );
    assert_eq!(proofs.len(), 2, "two verified proofs, one per kind");
    let one = S::<C>::ONE;
    let n = poly.n();
    // TODO: bind this research-only flag to authenticated first-state
    // admission; allowing a wallet to set it would discard predecessor claims.
    // The base case: a public flag g (io 62) and value v0 (io 63); with g = 1
    // the predecessor's accumulator gives way to the genesis claim (g0, v0),
    // g0 a fixed point of the template (a native verifier checks v0 = P(g0)).
    // Who may set g is the protocol's (a first offline state).
    let g0: Vec<S<C>> = (0..n).map(|j| S::<C>::from((j + 3) as u64)).collect();
    let v0 = if genesis {
        poly.eval(&g0)
    } else {
        S::<C>::ZERO
    };
    let gen_a = w.push(Array {
        name: "defer genesis io",
        family: gfam,
        rho: 1,
        r_count: 2,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255],
        rows: Vec::new(),
        values: vec![vec![if genesis { one } else { S::<C>::ZERO }, v0]],
    });
    w.io_arrays.push((gen_a, 62));
    let mut claims: Vec<Claim<S<C>>> = Vec::new();
    for (kind, (shape, inst, tr, _, acc)) in proofs.iter().enumerate() {
        let io = tr.circuit.as_ref().map_or(&inst.io[..], |c| &c.io_full[..]);
        let tail = &io[io.len() - n - 1..];
        assert!(
            tail[..n] == acc.point[..] && tail[n] == acc.value,
            "proof {kind}'s io ends with its accumulator"
        );
        let r = tr.rabc[0];
        let ch2: Vec<S<C>> = tr.circuit.as_ref().map_or(Vec::new(), |c| c.phase2.clone());
        let (rx, ry): (Vec<S<C>>, Vec<S<C>>) = (tr.rx.concat(), tr.ry.concat());
        let chv = [
            ch2.first().copied().unwrap_or(S::<C>::ZERO),
            ch2.get(1).copied().unwrap_or(S::<C>::ZERO),
        ];
        let (ma, mb, mc) = (poly.kinds[kind].eval)(&chv, &rx, &ry);
        assert_eq!(
            shape.sx, poly.kinds[kind].sx,
            "the kind is the verified proof's circuit"
        );
        let value = r * ma + r.square() * mb + r.square() * r * mc;
        let point = poly.fresh_point(kind, r, &ch2, &rx, &ry);
        assert!(poly.eval(&point) == value, "fresh claim {kind} holds");
        claims.push(if kind == 0 && genesis {
            Claim {
                point: g0.clone(),
                value: v0,
            }
        } else {
            (*acc).clone()
        });
        claims.push(Claim { point, value });
    }
    let claims: [Claim<S<C>>; 4] = claims.try_into().map_err(|_| ()).unwrap();
    let q_clock = std::time::Instant::now();
    let mut q = deferral::restrict(poly, &claims);
    let q_secs = q_clock.elapsed().as_secs_f64();
    // A claim false on this polynomial (an accumulator from a hop whose
    // polynomial differs, before a base case exists): q bent through it, as a
    // prover would have to; the folded claim is then false.
    let l = deferral::lagrange4::<S<C>>();
    for (t, cl) in claims.iter().enumerate() {
        let off = cl.value - deferral::horner(&q, S::<C>::from(t as u64));
        if !bool::from(off.is_zero()) {
            eprintln!(
                "deferral: claim {t} is false on this circuit's polynomial (bootstrap); q bent through it"
            );
            for (e, c) in l[t].iter().enumerate() {
                q[e] += off * c;
            }
        }
    }
    let fold = deferral::fold_arrays(w, gfam, &claims, &q);
    let nc = 1usize << w.arrays[fold.coords].rho;
    let on: Vec<S<C>> = (0..nc)
        .map(|j| if j < n { one } else { S::<C>::ZERO })
        .collect();
    let mut vrows: Vec<[Lin<S<C>>; 3]> = Vec::new();
    for (kind, (shape, inst, tr, sc, _)) in proofs.iter().enumerate() {
        // The accumulator: io leaves (a circuit proof's late io).
        let io_len = tr
            .circuit
            .as_ref()
            .map_or(inst.io.len(), |c| c.io_full.len());
        let n_io = 1usize << w.arrays[sc.io].rho;
        let (leaves, base) = (n_io / 2, io_len - n - 1);
        let io_map: Vec<Option<usize>> = (0..nc)
            .map(|j| (j < n).then(|| leaves + base + j))
            .collect();
        let acc_row = plus(&own(C_P[2 * kind], nc), &link(sc.io, 0, io_map, -one));
        let val_row = plus(
            &own(V_V[2 * kind], 1),
            &link(sc.io, 0, vec![Some(leaves + base + n)], -one),
        );
        if kind == 0 {
            // (P0 - io) (1 - g) = 0 and (P0 - g0) g = 0; likewise the value.
            let g_at = |len: usize| {
                link(
                    gen_a,
                    0,
                    (0..len).map(|j| (j < n || len == 1).then_some(0)).collect(),
                    one,
                )
            };
            let not_g = |len: usize| plus(&constant(vec![one; len]), &scaled(&g_at(len), -one));
            let g0c: Vec<S<C>> = (0..nc)
                .map(|j| if j < n { -g0[j] } else { S::<C>::ZERO })
                .collect();
            w.arrays[fold.coords]
                .rows
                .push([acc_row, not_g(nc), Vec::new()]);
            w.arrays[fold.coords].rows.push([
                plus(&own(C_P[0], nc), &constant(g0c)),
                g_at(nc),
                Vec::new(),
            ]);
            vrows.push([val_row, not_g(1), Vec::new()]);
            vrows.push([
                plus(&own(V_V[0], 1), &link(gen_a, 0, vec![Some(1)], -one)),
                g_at(1),
                Vec::new(),
            ]);
            vrows.push([g_at(1), g_at(1), g_at(1)]);
        } else {
            w.arrays[fold.coords]
                .rows
                .push([acc_row, constant(vec![one; nc]), Vec::new()]);
            vrows.push([val_row, constant(vec![one]), Vec::new()]);
        }
        // The fresh claim: constants (kind, padding), r, and the challenge
        // digits r_x, r_y.
        let (slot_a, glue) = (sc.side + scalar::S_SLOT, sc.side + scalar::S_GLUE);
        let slots = crate::spartan::slots(shape);
        let x_of = |s: usize| sc.slot_instances - slots.len() + s;
        let mut digit_maps: Vec<Vec<Option<usize>>> = vec![vec![None; nc]; 3];
        let mut place = |outer: bool, g: usize, j: usize| {
            let (s, sl) = slots
                .iter()
                .enumerate()
                .find(|(_, sl)| sl.outer == outer && sl.first <= g && g < sl.first + sl.k)
                .expect("a slot per variable");
            digit_maps[g - sl.first + 3 - sl.k][j] = Some(x_of(s));
        };
        for g in 0..shape.sx {
            place(true, g, 4 + g);
        }
        for g in 0..shape.sz() {
            place(false, g, 4 + poly.x + g);
        }
        let fixed = poly.fixed_coords(kind);
        let consts: Vec<S<C>> = (0..nc)
            .map(|j| -fixed.get(j).copied().flatten().unwrap_or(S::<C>::ZERO))
            .collect();
        let mut row = plus(
            &vec![crate::uniform::Term {
                src: crate::uniform::Src::Own(C_P[2 * kind + 1]),
                coef: on.clone(),
            }],
            &constant(consts),
        );
        for (d, map) in digit_maps.into_iter().enumerate() {
            row = plus(&row, &link(slot_a, scalar::T[d], map, -one));
        }
        row = plus(
            &row,
            &link(
                glue,
                scalar::R1,
                (0..nc).map(|j| (j == 1).then_some(0)).collect(),
                -one,
            ),
        );
        // alpha, beta: the verified circuit's second-phase challenges (a toy
        // kind fixes them to zero above).
        if poly.kinds[kind].deg != (0, 0) {
            for k in 0..2 {
                row = plus(
                    &row,
                    &link(
                        glue,
                        scalar::PH0 + k,
                        (0..nc).map(|j| (j == 2 + k).then_some(0)).collect(),
                        -one,
                    ),
                );
            }
        }
        w.arrays[fold.coords]
            .rows
            .push([row, constant(vec![one; nc]), Vec::new()]);
        let m_sum = (0..3).fold(own(V_V[2 * kind + 1], 1), |acc, i| {
            plus(&acc, &link(glue, scalar::M1 + i, vec![Some(0)], -one))
        });
        vrows.push([m_sum, constant(vec![one]), Vec::new()]);
    }
    w.arrays[fold.values].rows.extend(vrows);
    // The folded claim is the proof's late io, after the early io (64 values
    // at most).
    let late = deferral::expose(w, gfam, &fold, 64);
    DeferIdx {
        fold,
        late,
        claims,
        q,
        q_secs,
    }
}

/// One proof whose io digest the circuit computes: its io values, the
/// binding index of its io digest's low limb, its scalar side's io fold.
pub struct IoDigest<'a, S> {
    pub io: &'a [S],
    pub limb: usize,
    pub fold: usize,
    /// The io index of `io[0]` (0 for the early io; a circuit proof's late io
    /// sits further on).
    pub start: usize,
}

/// The io digests (rate-2 sponge from (0, 0, len) over the io values, as
/// `fs::io_digest`): permutations in the pool reading the io values from each
/// scalar side's io fold leaves; each digest (lane 0) equals lo + 2^128 hi of
/// its shared limbs.
pub fn io_digests<S: Scalar + PoseidonField>(
    w: &mut Wiring<S>,
    pool: &mut Pool<S>,
    gfam: usize,
    e_out: usize,
    e_zero: usize,
    shared: usize,
    items: &[IoDigest<S>],
) -> usize {
    let mut digests = Vec::new();
    let starts: Vec<Vec<S>> = items
        .iter()
        .map(|it| vec![S::ZERO, S::ZERO, S::from(it.io.len() as u64)])
        .collect();
    let e_start = constant_entries(w, gfam, "io digest starts", &starts);
    for (k, it) in items.iter().enumerate() {
        let n_io = 1usize << w.arrays[it.fold].rho;
        let leaves = n_io / 2;
        let e_io = w.export(it.fold, vec![own(0, n_io)]);
        let (mut src, mut val) = (
            Src3::Entry(e_start, k),
            [starts[k][0], starts[k][1], starts[k][2]],
        );
        let mut last = 0;
        for (j, pair) in it.io.chunks(2).enumerate() {
            let i0 = leaves + it.start + 2 * j;
            let b = pair.get(1).copied().unwrap_or(S::ZERO);
            let lanes = [
                Some((e_io, i0)),
                if pair.len() > 1 {
                    Some((e_io, i0 + 1))
                } else {
                    Some((e_zero, 0))
                },
            ];
            last = pool.chain(src, val, [pair[0], b], lanes);
            src = Src3::Out(last);
            val = pool.out(last);
        }
        assert!(
            val[0] == crate::fs::io_digest(it.io),
            "io digest in the pool"
        );
        digests.push((last, val, it.limb));
    }
    // The digests: (d, j1, j2) read from the pool, d = lo + 2^128 hi.
    let n = digests.len();
    let rho = n.next_power_of_two().trailing_zeros() as usize;
    let nn = 1usize << rho;
    let mut v = vec![vec![S::ZERO; nn]; 3];
    let (mut lo, mut hi) = (vec![None; nn], vec![None; nn]);
    let mut from = vec![None; nn];
    for (k, (perm, out, limb)) in digests.iter().enumerate() {
        for c in 0..3 {
            v[c][k] = out[c];
        }
        lo[k] = Some(*limb);
        hi[k] = Some(limb + 1);
        from[k] = Some((e_out, *perm));
    }
    let row = plus(
        &plus(&own(0, nn), &link(shared, 0, lo, -S::ONE)),
        &link(shared, 0, hi, -pw::<S>(128)),
    );
    let a = w.push(Array {
        name: "io digest",
        family: gfam,
        rho,
        r_count: n,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255; 3],
        rows: vec![[row, constant(vec![S::ONE; nn]), Vec::new()]],
        values: v,
    });
    w.import(a, vec![own(0, nn), own(1, nn), own(2, nn)], from);
    a
}

/// The binding index of a proof's io digest low limb (the first full scalar
/// after its challenges).
/// The binding index of a circuit proof's late io digest's low limb (last in
/// its shared values).
pub fn late_limb<C: iroha_pasta::PastaCurve>(
    tr: &crate::verifier::Trace<C>,
    claims: &[crate::ipa::Claim<C>],
    base: usize,
) -> usize {
    io_limb(tr, base) + 18 + 4 * claims.len()
}

/// Shared slots before a proof's pair-link challenge: a circuit proof's
/// second-phase challenges come first.
pub fn link_word_at<C: iroha_pasta::PastaCurve>(tr: &crate::verifier::Trace<C>) -> usize {
    tr.circuit.as_ref().map_or(0, |c| c.phase2.len())
}

pub fn io_limb<C: iroha_pasta::PastaCurve>(tr: &crate::verifier::Trace<C>, base: usize) -> usize {
    base + tr
        .challenges
        .iter()
        .map(|c| {
            if c.1 == crate::verifier::Kind::Full {
                2
            } else {
                1
            }
        })
        .sum::<usize>()
}

/// Shared values of the pairs in binding order (per pair the P proof's, then
/// the S proof's) and each pair's (P offset, S offset, P count).
fn binding_values(pairs: &[TracedPair]) -> (Vec<u128>, Vec<(usize, usize, usize)>) {
    let mut values: Vec<u128> = Vec::new();
    let mut offsets = Vec::new();
    for pr in pairs {
        let vp = crate::verifier::shared_values(&pr.tr_p, &pr.proof_p, &pr.claims_p);
        let vs = crate::verifier::shared_values(&pr.tr_s, &pr.proof_s, &pr.claims_s);
        offsets.push((values.len(), values.len() + vp.len(), vp.len()));
        values.extend(vp);
        values.extend(vs);
    }
    (values, offsets)
}

/// The binding chain's stand-ins for the phase-2 rho and the mask mu (as
/// stage A's `real-receive`).
fn binding_rho_mu() -> (crate::verifier::Fq, crate::verifier::Fq) {
    // TODO: wire the real phase-2 challenge and hiding mask into both
    // circuits; these stand-ins do not implement monetary pair binding.
    (
        crate::verifier::Fq::from(0x0123_4567_u64).square().square(),
        crate::verifier::Fq::from(0x0bad_cafe_u64).square(),
    )
}

/// S-recv's binding chain, native: `acc_(r+1) = acc_r rho + v_r` from
/// `acc_0 = mu`. Cells: the shared value v (cell 0), acc; rho one global cell.
fn native_binding<S: Scalar>(
    w: &mut Wiring<S>,
    gfam: usize,
    rho: S,
    mu: S,
    values: &[u128],
) -> usize {
    let n = values.len();
    let rb = (n + 1).next_power_of_two().trailing_zeros() as usize;
    let nn = 1usize << rb;
    let rho_a = w.push(Array {
        name: "binding rho",
        family: gfam,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255],
        rows: Vec::new(),
        values: vec![vec![rho]],
    });
    let mut v = vec![vec![S::ZERO; nn]; 2];
    let mut acc = mu;
    for r in 0..=n {
        v[1][r] = acc;
        if r < n {
            v[0][r] = S::from_u128(values[r]);
            acc = acc * rho + v[0][r];
        }
    }
    let a = w.arrays.len();
    let mask: Vec<S> = (0..nn)
        .map(|r| if r < n { S::ONE } else { S::ZERO })
        .collect();
    let next: Vec<Option<usize>> = (0..nn).map(|r| (r < n).then_some(r + 1)).collect();
    let row = [
        vec![crate::uniform::Term {
            src: crate::uniform::Src::Own(1),
            coef: mask.clone(),
        }],
        link(rho_a, 0, vec![Some(0); nn], S::ONE),
        plus(
            &link(a, 1, next, S::ONE),
            &vec![crate::uniform::Term {
                src: crate::uniform::Src::Own(0),
                coef: mask.iter().map(|c| -*c).collect(),
            }],
        ),
    ];
    w.push(Array {
        name: "binding native",
        family: gfam,
        rho: rb,
        r_count: n + 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![128, 255],
        rows: vec![row],
        values: v,
    })
}

/// Where S-recv's parts sit.
pub struct SRecvIdx {
    pub points: crate::points::PointsIdx,
    pub scalars: Vec<ScalarIdx<crate::verifier::Fq>>,
    pub shared: usize,
    pub io_digest: usize,
    pub perms: usize,
    pub values: usize,
    pub defer: DeferIdx<crate::verifier::Fq>,
}

/// S-recv in the uniform layout: for each incoming pair the P proof's points
/// side and the S proof's scalar side (with its io digest), the native
/// binding chain over every shared value, the fold of the verified S proofs'
/// matrix claims.
pub fn s_recv(gens: &PairGens, pairs: &[TracedPair]) -> (Wiring<crate::verifier::Fq>, SRecvIdx) {
    use crate::points::{PointsBuilder, SharedMap};
    use crate::verifier::Fq;
    use iroha_pasta::{Ep as Pallas, Eq as Vesta};
    let mut w = Wiring::<Fq>::default();
    let gfam = w.family(0, 1);
    let e_out = w.reserve();
    let e_v = w.reserve();
    let e_zero = constant_entries(&mut w, gfam, "zero word", &[vec![Fq::ZERO]]);
    let mut pool = Pool::new(Fq::from(NODE_DOMAIN));
    let (values, offsets) = binding_values(pairs);
    let mut pb = PointsBuilder::<Vesta>::new(&mut w, e_out, e_v, e_zero);
    for (pr, &(off_p, off_s, len_p)) in pairs.iter().zip(&offsets) {
        let w = off_s + link_word_at(&pr.tr_s);
        let sm = SharedMap {
            base: off_p,
            other: Some([w, w + 1]),
            r: Some([off_p + len_p - 2, off_p + len_p - 1]),
        };
        pb.verify(
            &mut pool,
            &pr.shape_p,
            &gens.p,
            &pr.proof_p,
            &pr.claims_p,
            &pr.tr_p,
            sm,
        );
    }
    let (rho, mu) = binding_rho_mu();
    let shared = native_binding(&mut w, gfam, rho, mu, &values);
    let n_sh = 1usize << w.arrays[shared].rho;
    w.set_export(e_v, shared, vec![own(0, n_sh)]);
    let _table = endo_table(&mut w, gfam);
    let mut scalars = Vec::new();
    for (i, (pr, &(_, off_s, _))) in pairs.iter().zip(&offsets).enumerate() {
        let ch2: Vec<Fq> = pr
            .tr_s
            .circuit
            .as_ref()
            .map_or(vec![Fq::ZERO; 2], |c| c.phase2.clone());
        let kind = &gens.defer_s.kinds[i];
        let evals = |rx: &[Fq], ry: &[Fq]| (kind.eval)(&ch2, rx, ry);
        scalars.push(scalar_into::<Pallas>(
            &mut w,
            gfam,
            shared,
            off_s,
            &pr.shape_s,
            &pr.inst_s,
            &pr.proof_s,
            &pr.claims_s,
            &pr.tr_s,
            &evals,
        ));
    }
    let items: Vec<IoDigest<Fq>> = pairs
        .iter()
        .zip(&offsets)
        .zip(&scalars)
        .flat_map(|((pr, &(_, off_s, _)), sc)| {
            let mut v = vec![IoDigest {
                io: &pr.inst_s.io,
                limb: io_limb(&pr.tr_s, off_s),
                fold: sc.io,
                start: 0,
            }];
            if pr
                .tr_s
                .circuit
                .as_ref()
                .is_some_and(|c| c.late_digest.is_some())
            {
                v.push(IoDigest {
                    io: &pr.proof_s.late,
                    limb: late_limb(&pr.tr_s, &pr.claims_s, off_s),
                    fold: sc.io,
                    start: 64,
                });
            }
            v
        })
        .collect();
    let io_digest = io_digests(&mut w, &mut pool, gfam, e_out, e_zero, shared, &items);
    let proofs: Vec<_> = pairs
        .iter()
        .zip(&scalars)
        .map(|(pr, sc)| (&pr.shape_s, &pr.inst_s, &pr.tr_s, sc, &pr.acc_s))
        .collect();
    let defer = defer_into::<Pallas>(&mut w, gfam, &gens.defer_s, &proofs, gens.genesis);
    let perms = pool.len();
    let mut rng = SpikeRng::new(b"s-recv pool");
    let _pidx = pool.build(&mut w, e_out, &mut rng);
    let points = pb.build(&mut w, gfam, shared, true);
    let mut seen = std::collections::HashSet::new();
    w.tables.retain(|(_, t)| seen.insert(*t));
    (
        w,
        SRecvIdx {
            points,
            scalars,
            shared,
            io_digest,
            perms,
            values: values.len(),
            defer,
        },
    )
}
