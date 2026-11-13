//! Stage A2, step 1: uniform relations, and the RP57 permutation as round
//! arrays.
//!
//! A circuit is a set of arrays. An array is a small template (R1CS rows over
//! local cells) repeated over instances `i = p * 2^rho + r`; the arrays of one
//! family share the `p` range. A template term reads the constant 1, a cell of
//! its own instance, or a cell of instance `(p, map[r])` of another array of
//! its family (a link; `None` switches the term off for that `r`). A term's
//! coefficient may depend on `r` (round constants).
//!
//! Every (array, cell) is an aligned power-of-two block of the witness and
//! every template row an aligned block of constraints, so each instance bit
//! lands on one index bit. A matrix MLE term then factors into the fixed bits,
//! a product over `p`'s bits and a sum over `r`: the verifier evaluates
//! `A~, B~, C~` in O(terms * 2^rho + bits), and the prover computes `Az, Bz, Cz`
//! and `M(r_x, .)` from the templates without building any matrix.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::{Add, Mul, Sub};
use std::sync::OnceLock;

use ff::Field;
use iroha_pasta::poseidon::{PoseidonField, permute_with};
use rayon::prelude::*;

use crate::fs::{Scalar, SpikeRng};
use crate::r1cs::{Circuit, Instance, Rel, Shape, Witness};
use crate::sparse::{Assembly, SLc};

/// Template rows with each term's LogUp challenge factor.
type SymbolicRows<F> = Vec<Vec<[Vec<(Term<F>, Factor)>; 3]>>;

/// Where a template term reads from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Src {
    One,
    Own(usize),
    /// Cell `cell` of instance `(p, map[r])` of `array` (same family).
    Link {
        array: usize,
        cell: usize,
        map: Vec<Option<usize>>,
    },
    /// The sum of cell `cell` over every instance of `array` (the LogUp sum row).
    All {
        array: usize,
        cell: usize,
    },
}

#[derive(Clone, Debug)]
pub struct Term<F> {
    pub src: Src,
    /// Coefficient per `r` (length `2^rho` of the reading array).
    pub coef: Vec<F>,
}

pub type Lin<F> = Vec<Term<F>>;

pub struct Array<F> {
    pub name: &'static str,
    pub family: usize,
    pub rho: usize,
    /// Instances use `r < r_count`; the rest of the `2^rho` slots are absent.
    pub r_count: usize,
    /// A constant column: public values in the trailing fixed rows, committed
    /// in the verification key, with no template rows of its own.
    pub fixed: bool,
    /// LogUp lookups per instance: (table id, tuple). The builder adds one
    /// second-phase helper cell and one row per lookup.
    pub lookups: Vec<(u32, Vec<Lin<F>>)>,
    pub widths: Vec<u16>,
    /// Template rows `(A, B, C)`.
    pub rows: Vec<[Lin<F>; 3]>,
    /// `values[cell][instance]`.
    pub values: Vec<Vec<F>>,
}

impl<F> Array<F> {
    pub fn cells(&self) -> usize {
        self.widths.len()
    }
}

/// Table ids of copy tuples (an address and `k` values: `COPY_TABLE + 1 + k`)
/// sit above every fixed table's id.
pub const COPY_TABLE: u32 = 1 << 20;

/// A copy-table entry site: `array` exports `tuple` (linear combinations in
/// its own frame) at every present instance, each at its own address.
pub struct Export<F> {
    pub array: usize,
    pub tuple: Vec<Lin<F>>,
}

/// A copy-table read site: slot `i` of `array` reads `tuple` from the entry
/// `from[i] = (export, instance)`. `None` switches the read off; that is
/// decided per r (every present p of an r reads, or none does).
pub struct Import<F> {
    pub array: usize,
    pub tuple: Vec<Lin<F>>,
    pub from: Vec<Option<(usize, usize)>>,
}

/// The copy argument's sites: values exported to the copy table and values
/// read from it. Each entry's address sits in a preprocessed index column
/// (a constant column committed in the verification key), so a read names
/// its entry and the prover never chooses an address.
pub struct Copies<F> {
    /// Independent private product gates; each tuple names an array's a,b,c
    /// cells. The prover refreshes these with its fresh proof entropy.
    pub product_masks: Vec<(usize, [usize; 3])>,

    pub exports: Vec<Export<F>>,
    pub imports: Vec<Import<F>>,
    /// Second-phase rows over the LogUp challenges (the deferred matrix
    /// claims' fold).
    pub challenge: ChallengeRows<F>,
    /// Arrays in z's public half: (array, io index of its instance 0). A
    /// one-cell array of the global family; its block sits at the io indices
    /// `offset..offset + 2^rho` (offset aligned to `2^rho`), so rows read
    /// public inputs through ordinary links.
    pub io_arrays: Vec<(usize, usize)>,
    /// Late io: (io index, source array, cell, instance) per value, in proof
    /// order. The prover fills these after the second-phase challenges.
    pub late_io: Vec<(usize, usize, usize, usize)>,
    /// The link phase (cells that follow the pair link's rho).
    pub link: LinkPhase<F>,
    /// The wallet row: (array, io index of its commitment's four limbs). The
    /// array (one wallet's constants, its key's comb tables) gets the last
    /// row to itself, cell `k` instance `i` at column `k 2^m + i` whatever
    /// the circuit, so the row's unblinded commitment depends on the wallet
    /// alone; it is a fixed row whose commitment the instance names.
    pub wallet: Option<(usize, usize)>,
}

impl<F> Default for Copies<F> {
    fn default() -> Self {
        Copies {
            product_masks: Vec::new(),
            exports: Vec::new(),
            imports: Vec::new(),
            challenge: ChallengeRows::default(),
            io_arrays: Vec::new(),
            late_io: Vec::new(),
            link: LinkPhase::default(),
            wallet: None,
        }
    }
}

/// The values of a builder's link-phase cells once the pair link's rho (its
/// 32-byte word) is known: (array, cell, instance, value).
pub type LinkFill<F> = Box<dyn Fn(&[u8; 32]) -> Vec<(usize, usize, usize, F)> + Send + Sync>;

/// The link's io values from rho and R, one per link io array (in order).
pub type LinkIo<F> = Box<dyn Fn(&[u8; 32], &[u8; 32]) -> Vec<F> + Send + Sync>;

/// A builder's link phase: whole arrays whose values follow the pair link's
/// rho, committed after the link's word and before the second-phase
/// challenges (with the chunk arrays their lookups move into); tables that
/// only link-phase cells read (their multiplicities, counted after the fill,
/// are link-phase cells too); the fill; the link's io (one-cell arrays in z's
/// public half, past the header's io, whose values prover and verifier both
/// take from rho and R); and the cells of the circuit's binding value R.
pub struct LinkPhase<F> {
    pub arrays: Vec<usize>,
    pub tables: Vec<u32>,
    pub fill: Option<LinkFill<F>>,
    /// (array, io index of its instance 0), as `io_arrays`.
    pub io: Vec<(usize, usize)>,
    pub io_values: Option<LinkIo<F>>,
    /// R's limbs as cells (array, cell, instance), low limb first, and the
    /// limb width in bits.
    pub binding: Option<(Vec<(usize, usize, usize)>, u32)>,
}

impl<F> Default for LinkPhase<F> {
    fn default() -> Self {
        LinkPhase {
            arrays: Vec::new(),
            tables: Vec::new(),
            fill: None,
            io: Vec::new(),
            io_values: None,
            binding: None,
        }
    }
}

/// The values of a builder's second-phase cells for challenges `(alpha,
/// beta)`: (array, cell, instance, value).
pub type ChallengeFill<F> = Box<dyn Fn(&[F]) -> Vec<(usize, usize, usize, F)> + Send + Sync>;

/// Second-phase relations a builder declares over the LogUp challenges:
/// template rows whose terms carry a challenge factor, and the cells those
/// rows define, which `fill` computes once the challenges are known. Every
/// cell a row reads with a challenge factor must be fixed by the first phase.
pub struct ChallengeRows<F> {
    /// (array, row): the row's A, B and C over (term, factor).
    pub rows: Vec<(usize, [Vec<(Term<F>, Factor)>; 3])>,
    /// Second-phase cells, (array, cell).
    pub cells: Vec<(usize, usize)>,
    pub fill: Option<ChallengeFill<F>>,
}

impl<F> Default for ChallengeRows<F> {
    fn default() -> Self {
        ChallengeRows {
            rows: Vec::new(),
            cells: Vec::new(),
            fill: None,
        }
    }
}

/// An aligned witness block: instance `i` sits at
/// `base + (i mod 2^lo) + ((i >> lo) << hi)`.
#[derive(Clone, Debug)]
struct Block {
    base: usize,
    lo: usize,
    hi: usize,
    m: usize,
}

impl Block {
    fn index(&self, i: usize) -> usize {
        self.base + (i & ((1 << self.lo) - 1)) + ((i >> self.lo) << self.hi)
    }
    /// The witness-index bit holding instance bit `t`.
    fn wbit(&self, t: usize) -> usize {
        if t < self.lo {
            t
        } else {
            self.hi + (t - self.lo)
        }
    }
}

/// A template term that reads a cell block, as the prover's gather sees it.
struct Reader {
    array: usize,
    row: usize,
    mat: usize,
    term: usize,
}

/// What a verifier of a circuit's proofs depends on besides its key: the
/// constraint variables, the Hyrax rows, the first phase's rows (the
/// transcript absorbs them before the second-phase challenges) and the
/// trailing fixed rows (constants from the key). Two circuits with one layout
/// (and one io length and column count) have verifiers of one structure, so a
/// verifier slot can take a proof of either: Send and Receive share one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub sx: usize,
    pub row_bits: usize,
    pub p1: usize,
    /// The link phase's rows (after the first phase's).
    pub pl: usize,
    pub fixed: usize,
}

pub struct UniformCircuit<F> {
    /// Imported immutable verifier-only programs have no private construction
    /// arrays. Their authenticated challenge metadata accompanies the tape.
    verifier_only: Option<(usize, (usize, usize))>,
    pub label: &'static str,
    pub arrays: Vec<Array<F>>,
    /// log2 of each family's `p` range, and its used count (`p < count`).
    pub pi: Vec<usize>,
    pub counts: Vec<usize>,
    pub shape: Shape,
    pub io_len: usize,
    pub constraints: usize,
    /// Logical cells of independent product-mask instances, before placement.
    product_masks: Vec<(usize, [usize; 3], usize)>,
    row_base: Vec<Vec<usize>>,
    blocks: Vec<Vec<Block>>,
    bounds: Vec<usize>,
    /// Trailing fixed rows and their values (row major).
    fixed_rows: usize,
    fixed_values: Vec<F>,
    /// The wallet row (the last fixed row) and its commitment's io index.
    wallet_rows: usize,
    wallet_io: Option<usize>,
    /// Second-phase cells (LogUp helpers), per array and cell.
    phase2: Vec<Vec<bool>>,
    /// Arrays with lookups: (array, first helper cell, first lookup row).
    lookup_arrays: Vec<(usize, usize, usize)>,
    /// Tables: (values array, table id, multiplicity/helper array).
    tables: Vec<(usize, u32, usize)>,
    /// Builder rows over the challenges: (array, row, the row), and the
    /// fill of the cells they define.
    ch_rows: Vec<(usize, usize, [Vec<(Term<F>, Factor)>; 3])>,
    ch_fill: Option<ChallengeFill<F>>,
    /// Arrays in z's public half, and the late io's sources.
    io_arrays: Vec<(usize, usize)>,
    late_io: Vec<(usize, usize, usize, usize)>,
    /// Copy-table entry sites: (exporting array, entry array holding the
    /// multiplicity and helper cells, table id, tuple in the entry array's
    /// frame, address first).
    exports: Vec<(usize, usize, u32, Vec<Lin<F>>)>,
    /// Lookups switched off for some r (copy reads): (array, lookup) -> the
    /// right-hand side per r (1 reads, 0 off).
    lookup_mask: HashMap<(usize, usize), Vec<F>>,
    /// Copy reads switched off per instance: (array, lookup) -> their mask
    /// column (index array, cell), the right-hand side.
    lookup_mask_col: HashMap<(usize, usize), (usize, usize)>,
    /// The copy sites as declared (the sparse form wires them directly) and
    /// the index columns per site array.
    copy_exports: Vec<(usize, Vec<Lin<F>>)>,
    copy_imports: Vec<(usize, Vec<Lin<F>>, Vec<Option<(usize, usize)>>)>,
    index_arrays: Vec<(usize, usize)>,
    /// Hyrax rows of the first phase.
    p1_rows: usize,
    /// Rows of the link phase (after the first phase's).
    pl_rows: usize,
    /// Second-phase rows in use (after the link phase's).
    p2_rows: usize,
    /// The link phase: its tables (multiplicities counted after the fill),
    /// the fill, the link's io arrays and their values, and R's limb cells.
    link_tables: Vec<u32>,
    link_fill: Option<LinkFill<F>>,
    link_io: Vec<(usize, usize)>,
    link_io_values: Option<LinkIo<F>>,
    link_binding: Option<(Vec<(usize, usize, usize)>, u32)>,
    /// Which r of each array are instances: `(sigma, inner, outer)`, r
    /// present when `r mod 2^sigma < inner` and `r >> sigma < outer` (a chunk
    /// array's `r = j 2^sigma + s` is chunk j of instance s; other arrays
    /// have outer 1).
    rfield: Vec<(usize, usize, usize)>,
    /// Cells moved into chunk arrays: (chunk array, cell, chunk index).
    moved: Vec<Vec<Option<(usize, usize, usize)>>>,
    /// Arrays of split families: (the clone with the instances p >= 2^k, the
    /// slot shift).
    split_of: Vec<Option<(usize, usize)>>,
    /// The prover's gather of `Az, Bz, Cz`, built on first use.
    gather: OnceLock<Vec<GatherJob>>,
    /// The symbolic rows (the terms of every proof's rows, LogUp rows over
    /// their challenge factors), built on first use: the prover's kernels
    /// read them with the factors' values instead of instantiating every
    /// row per proof.
    symbolic: OnceLock<SymbolicRows<F>>,
    /// Readers of every cell block (the gather of `M(r_x, .)`), built on
    /// first use.
    readers: OnceLock<Readers>,
    /// The flat plan of `M(r_x, .)`, built on first use.
    m_plans: OnceLock<Vec<MPlan<F>>>,
    /// The matrix evaluation compiled to a straight-line program, built on
    /// first use.
    plan: OnceLock<Tape<F>>,
}

#[path = "program_artifact.rs"]
mod program_artifact;

/// A linear combination's value at slot `i` of array `a`, from the arrays'
/// values (multiplicities are counted before the layout exists).
fn eval_lin<F: Field>(arrays: &[Array<F>], pi: &[usize], a: usize, lin: &Lin<F>, i: usize) -> F {
    let rho = arrays[a].rho;
    let (p, r) = (i >> rho, i & ((1 << rho) - 1));
    lin.iter()
        .map(|t| {
            let v = match &t.src {
                Src::One => F::ONE,
                Src::Own(k) => arrays[a].values[*k][i],
                Src::Link { array, cell, map } => match map[r] {
                    Some(rr) => {
                        let pp = if pi[arrays[*array].family] == 0 { 0 } else { p };
                        arrays[*array].values[*cell][(pp << arrays[*array].rho) | rr]
                    }
                    None => F::ZERO,
                },
                Src::All { .. } => panic!("sum terms appear only in the sum row"),
            };
            t.coef[r] * v
        })
        .sum()
}

/// Whether r is present under `(sigma, inner, outer)`.
fn field_present(f: (usize, usize, usize), r: usize) -> bool {
    let (sigma, inner, outer) = f;
    (r & ((1 << sigma) - 1)) < inner && (r >> sigma) < outer
}

/// Splits each family whose instance count `n` sits just past a power of
/// two (`2^k < n < 1.5 2^k`, `2^k >= 8`) at `2^k`: its arrays keep the
/// instances `p < 2^k`, and clones in a new family take the rest, shifted down
/// by `2^k`. A block of `2^(k+1)` instances that was up to half empty becomes
/// one dense block and one of `next_pow2(n - 2^k)`.
///
/// Links stay within a family (or read a global array), so the clones' links
/// point at the clones; copy sites on the arrays split with them, reads of a
/// split export are renumbered, and a sum over a split array also sums its
/// clone. Returns, per original array, its clone and the slot shift.
fn split_families<F: Scalar>(
    arrays: &mut Vec<Array<F>>,
    pi: &mut Vec<usize>,
    counts: &mut Vec<usize>,
    copies: &mut Copies<F>,
) -> Vec<Option<(usize, usize)>> {
    let mut split_of: Vec<Option<(usize, usize)>> = vec![None; arrays.len()];
    let mut export_split: HashMap<usize, (usize, usize)> = HashMap::new();
    for f in 0..pi.len() {
        let n = counts[f];
        if pi[f] == 0 {
            continue;
        }
        let k = pi[f] - 1;
        if (1usize << k) >= n || (1usize << k) < 8 || 2 * (n - (1usize << k)) >= (1usize << k) {
            continue;
        }
        let r = n - (1usize << k);
        let (f2, pi2) = (pi.len(), r.next_power_of_two().trailing_zeros() as usize);
        pi.push(pi2);
        counts.push(r);
        pi[f] = k;
        counts[f] = 1 << k;
        let members: Vec<usize> = (0..arrays.len())
            .filter(|&a| arrays[a].family == f)
            .collect();
        let first = arrays.len();
        let clone_of: HashMap<usize, usize> = members
            .iter()
            .enumerate()
            .map(|(i, &a)| (a, first + i))
            .collect();
        let remap = |lin: &Lin<F>| -> Lin<F> {
            lin.iter()
                .map(|t| Term {
                    src: match &t.src {
                        Src::Link { array, cell, map } if clone_of.contains_key(array) => {
                            Src::Link {
                                array: clone_of[array],
                                cell: *cell,
                                map: map.clone(),
                            }
                        }
                        src => src.clone(),
                    },
                    coef: t.coef.clone(),
                })
                .collect()
        };
        for &a in &members {
            let arr = &arrays[a];
            let shift = (1usize << k) << arr.rho;
            let slots2 = (1usize << pi2) << arr.rho;
            let values: Vec<Vec<F>> = arr
                .values
                .iter()
                .map(|col| {
                    (0..slots2)
                        .map(|i| col.get(shift + i).copied().unwrap_or(F::ZERO))
                        .collect()
                })
                .collect();
            let rows = arr
                .rows
                .iter()
                .map(|row| [remap(&row[0]), remap(&row[1]), remap(&row[2])])
                .collect();
            let lookups = arr
                .lookups
                .iter()
                .map(|(t, tuple)| (*t, tuple.iter().map(&remap).collect()))
                .collect();
            let name: &'static str = Box::leak(format!("{} (rest)", arr.name).into_boxed_str());
            let clone = Array {
                name,
                family: f2,
                rho: arr.rho,
                r_count: arr.r_count,
                fixed: arr.fixed,
                lookups,
                widths: arr.widths.clone(),
                rows,
                values,
            };
            split_of[a] = Some((clone_of[&a], shift));
            arrays.push(clone);
            split_of.push(None);
        }
        for &a in &members {
            let shift = (1usize << k) << arrays[a].rho;
            for col in arrays[a].values.iter_mut() {
                col.truncate(shift);
            }
        }
        // Copy sites on the members split with them.
        let n_ex = copies.exports.len();
        for e in 0..n_ex {
            let a = copies.exports[e].array;
            if let Some(&a2) = clone_of.get(&a) {
                let tuple = copies.exports[e].tuple.iter().map(&remap).collect();
                export_split.insert(e, (copies.exports.len(), (1usize << k) << arrays[a].rho));
                copies.exports.push(Export { array: a2, tuple });
            }
        }
        let n_im = copies.imports.len();
        for m in 0..n_im {
            let b = copies.imports[m].array;
            if let Some(&b2) = clone_of.get(&b) {
                let shift = (1usize << k) << arrays[b].rho;
                let slots2 = (1usize << pi2) << arrays[b].rho;
                let rest: Vec<Option<(usize, usize)>> = (0..slots2)
                    .map(|i| copies.imports[m].from.get(shift + i).copied().flatten())
                    .collect();
                copies.imports[m].from.truncate(shift);
                let tuple = copies.imports[m].tuple.iter().map(&remap).collect();
                if rest.iter().any(|x| x.is_some()) {
                    copies.imports.push(Import {
                        array: b2,
                        tuple,
                        from: rest,
                    });
                }
            }
        }
    }
    // Reads of split exports, renumbered.
    for im in copies.imports.iter_mut() {
        for x in im.from.iter_mut() {
            if let Some((e, i)) = *x
                && let Some(&(e2, shift)) = export_split.get(&e)
                && i >= shift
            {
                *x = Some((e2, i - shift));
            }
        }
    }
    // A sum over a split array sums its clone too.
    let clones: Vec<(usize, usize)> = split_of
        .iter()
        .enumerate()
        .filter_map(|(a, s)| s.map(|(a2, _)| (a, a2)))
        .collect();
    if !clones.is_empty() {
        for arr in arrays.iter_mut() {
            for row in arr.rows.iter_mut() {
                for lin in row.iter_mut() {
                    let extra: Vec<Term<F>> = lin
                        .iter()
                        .filter_map(|t| match &t.src {
                            Src::All { array, cell } => {
                                clones.iter().find(|(a, _)| a == array).map(|(_, a2)| Term {
                                    src: Src::All {
                                        array: *a2,
                                        cell: *cell,
                                    },
                                    coef: t.coef.clone(),
                                })
                            }
                            _ => None,
                        })
                        .collect();
                    lin.extend(extra);
                }
            }
        }
    }
    split_of
}

/// Moves each array's chunk lookups into chunk arrays. The lookups of an
/// array into one table whose tuples are single cells with unit coefficients
/// form a group; its cells become the cells of one array whose instance
/// `j 2^rho + r` is lookup j at instance r, so the group costs one lookup row
/// (and its cells one block each) instead of one per chunk. References to the
/// moved cells become links shifted to chunk j; a lookup that reads a moved
/// cell through another coefficient (a narrow chunk's shifted range check)
/// stays with its array.
fn chunk_arrays<F: Scalar>(
    arrays: &mut Vec<Array<F>>,
    rfield: &mut Vec<(usize, usize, usize)>,
    moved: &mut Vec<Vec<Option<(usize, usize, usize)>>>,
) {
    let n_user = arrays.len();
    let single = |lin: &Lin<F>| -> Option<usize> {
        match lin.as_slice() {
            [
                Term {
                    src: Src::Own(k),
                    coef,
                },
            ] if coef.iter().all(|c| *c == F::ONE) => Some(*k),
            _ => None,
        }
    };
    for a in 0..n_user {
        if arrays[a].fixed || arrays[a].lookups.len() < 2 {
            continue;
        }
        // Groups by (table, tuple length) in first-seen order; a cell joins at
        // most one group.
        let mut groups: Vec<((u32, usize), Vec<(usize, Vec<usize>)>)> = Vec::new();
        let mut used: HashSet<usize> = HashSet::new();
        for (li, (tid, tuple)) in arrays[a].lookups.iter().enumerate() {
            let Some(cells) = tuple.iter().map(single).collect::<Option<Vec<usize>>>() else {
                continue;
            };
            if cells.iter().any(|c| used.contains(c))
                || cells.iter().collect::<HashSet<_>>().len() != cells.len()
            {
                continue;
            }
            used.extend(cells.iter().copied());
            let key = (*tid, cells.len());
            match groups.iter_mut().find(|g| g.0 == key) {
                Some(g) => g.1.push((li, cells)),
                None => groups.push((key, vec![(li, cells)])),
            }
        }
        let mut dropped: Vec<usize> = Vec::new();
        for ((tid, width), members) in groups.into_iter().filter(|g| g.1.len() >= 2) {
            let ca = arrays.len();
            let src = &arrays[a];
            let sigma = src.rho;
            let rho = sigma + members.len().next_power_of_two().trailing_zeros() as usize;
            let p_slots = src.values[0].len() >> sigma;
            let mut values = vec![vec![F::ZERO; p_slots << rho]; width];
            for (j, (_, cells)) in members.iter().enumerate() {
                for (k, &c) in cells.iter().enumerate() {
                    for p in 0..p_slots {
                        for r in 0..1usize << sigma {
                            values[k][(p << rho) | (j << sigma) | r] =
                                src.values[c][(p << sigma) | r];
                        }
                    }
                    moved[a][c] = Some((ca, k, j));
                }
            }
            let widths = (0..width)
                .map(|k| {
                    members
                        .iter()
                        .map(|(_, cells)| src.widths[cells[k]])
                        .max()
                        .unwrap()
                })
                .collect();
            let name: &'static str = Box::leak(format!("{} chunks", src.name).into_boxed_str());
            let n = 1usize << rho;
            let chunk = Array {
                name,
                family: src.family,
                rho,
                r_count: members.len(),
                fixed: false,
                lookups: vec![(tid, (0..width).map(|k| own(k, n)).collect())],
                widths,
                rows: Vec::new(),
                values,
            };
            rfield.push((sigma, src.r_count, members.len()));
            moved.push(vec![None; width]);
            dropped.extend(members.iter().map(|(li, _)| *li));
            arrays.push(chunk);
        }
        dropped.sort_unstable();
        for li in dropped.into_iter().rev() {
            arrays[a].lookups.remove(li);
        }
    }
    // References to moved cells become links into their chunk arrays.
    let rhos: Vec<usize> = arrays.iter().map(|a| a.rho).collect();
    let fields = rfield.clone();
    let moved_ro = moved.clone();
    let relink = |a: usize, lin: &mut Lin<F>| {
        for t in lin.iter_mut() {
            let new = match &t.src {
                Src::Own(k) => moved_ro[a][*k].map(|(ca, kc, j)| {
                    let sigma = rhos[a];
                    Src::Link {
                        array: ca,
                        cell: kc,
                        map: (0..1usize << sigma)
                            .map(|r| field_present(fields[a], r).then_some((j << sigma) | r))
                            .collect(),
                    }
                }),
                Src::Link { array, cell, map } => moved_ro[*array][*cell].map(|(ca, kc, j)| {
                    let sigma = rhos[*array];
                    Src::Link {
                        array: ca,
                        cell: kc,
                        map: map.iter().map(|m| m.map(|t| (j << sigma) | t)).collect(),
                    }
                }),
                _ => None,
            };
            if let Some(s) = new {
                t.src = s;
            }
        }
    };
    for (a, arr) in arrays.iter_mut().enumerate() {
        for row in &mut arr.rows {
            for lin in row.iter_mut() {
                relink(a, lin);
            }
        }
        for (_, tuple) in &mut arr.lookups {
            for lin in tuple.iter_mut() {
                relink(a, lin);
            }
        }
    }
}

/// Moves arrays built from array index 0 and family 0 to array index `at`
/// and family `fam_at`: every link and sum term shifts with them (combining
/// separately built circuits into one).
pub fn relocate<F>(arrays: &mut [Array<F>], at: usize, fam_at: usize) {
    let shift = |lin: &mut Lin<F>| {
        for t in lin.iter_mut() {
            match &mut t.src {
                Src::Link { array, .. } | Src::All { array, .. } => *array += at,
                Src::One | Src::Own(_) => {}
            }
        }
    };
    for arr in arrays.iter_mut() {
        arr.family += fam_at;
        for row in &mut arr.rows {
            for lin in row.iter_mut() {
                shift(lin);
            }
        }
        for (_, tuple) in &mut arr.lookups {
            for lin in tuple.iter_mut() {
                shift(lin);
            }
        }
    }
}

/// Copy-argument layout: export sites (exporting array, entry array, table
/// id, tuple in the entry array's frame), the reads switched off for some r
/// (right-hand side per r), the reads switched off per instance (their mask
/// column: index array, cell), the index column array of each site array,
/// and the entry arrays.
type CopyLayout<F> = (
    Vec<(usize, usize, u32, Vec<Lin<F>>)>,
    HashMap<(usize, usize), Vec<F>>,
    HashMap<(usize, usize), (usize, usize)>,
    Vec<(usize, usize)>,
    Vec<usize>,
);

/// Lays out the copy argument. Every present instance of an export gets an
/// address (from 1). Each site array gets one constant array of index
/// columns, one per site: an export's entry addresses, or the address each
/// read names (0 where a read is off). A read becomes a lookup of
/// (address, values) under table id `COPY_TABLE + 1 + k` for `k` values,
/// with right-hand side 0 where it is off: a per-r coefficient when every
/// present p of an r agrees, else a constant mask column (one more index
/// column, 1 where the instance reads). An export becomes an entry array
/// (multiplicities, counted here, and helpers) whose row the LogUp code
/// writes like a table entry's: `g (alpha - t) = m`.
fn copy_layout<F: Scalar>(
    arrays: &mut Vec<Array<F>>,
    pi: &[usize],
    counts: &[usize],
    rfield: &mut Vec<(usize, usize, usize)>,
    moved: &mut Vec<Vec<Option<(usize, usize, usize)>>>,
    copies: &Copies<F>,
) -> CopyLayout<F> {
    let mut sites = Vec::new();
    let mut mask = HashMap::new();
    let mut mask_col = HashMap::new();
    let mut index_arrays = Vec::new();
    let mut helpers = Vec::new();
    if copies.exports.is_empty() && copies.imports.is_empty() {
        return (sites, mask, mask_col, index_arrays, helpers);
    }
    let slots = |arrays: &[Array<F>], a: usize| 1usize << (pi[arrays[a].family] + arrays[a].rho);
    let present = |arrays: &[Array<F>], rfield: &[(usize, usize, usize)], a: usize, i: usize| {
        let rho = arrays[a].rho;
        (i >> rho) < counts[arrays[a].family] && field_present(rfield[a], i & ((1 << rho) - 1))
    };
    // Copy tuples read cells that stay in place (no range chunks, no sums).
    let in_place = |arrays: &[Array<F>],
                    moved: &[Vec<Option<(usize, usize, usize)>>],
                    a: usize,
                    lin: &Lin<F>| {
        for t in lin {
            let ok = match &t.src {
                Src::One => true,
                Src::Own(k) => moved[a][*k].is_none(),
                Src::Link { array, cell, .. } => moved[*array][*cell].is_none(),
                Src::All { .. } => false,
            };
            assert!(ok, "{}: a copy tuple reads cells in place", arrays[a].name);
        }
    };
    let ident = |n: usize| -> Vec<Option<usize>> { (0..n).map(Some).collect() };
    let addr_term = |x: usize, cell: usize, n: usize| -> Lin<F> {
        vec![Term {
            src: Src::Link {
                array: x,
                cell,
                map: ident(n),
            },
            coef: vec![F::ONE; n],
        }]
    };
    // Addresses of the entries.
    let mut addr: Vec<Vec<u64>> = Vec::new();
    let mut next = 1u64;
    for e in &copies.exports {
        let mut v = vec![0u64; slots(arrays, e.array)];
        for (i, x) in v.iter_mut().enumerate() {
            if present(arrays, rfield, e.array, i) {
                *x = next;
                next += 1;
            }
        }
        addr.push(v);
    }
    let abits = (64 - next.leading_zeros()) as u16;
    // Index columns: per site array, its exports' columns, then its reads'.
    let mut columns: BTreeMap<usize, Vec<Vec<F>>> = BTreeMap::new();
    let mut ex_cell = Vec::new();
    for (e, ex) in copies.exports.iter().enumerate() {
        let col = columns.entry(ex.array).or_default();
        ex_cell.push(col.len());
        col.push(addr[e].iter().map(|x| F::from(*x)).collect());
    }
    let mut im_cell = Vec::new();
    let mut im_mask: Vec<Option<usize>> = Vec::new();
    for im in &copies.imports {
        let a = im.array;
        assert_eq!(
            im.from.len(),
            slots(arrays, a),
            "{}: one source per slot",
            arrays[a].name
        );
        let reads = |i: usize| present(arrays, rfield, a, i) && im.from[i].is_some();
        let col: Vec<F> = im
            .from
            .iter()
            .enumerate()
            .map(|(i, f)| match f {
                Some((e, j)) if present(arrays, rfield, a, i) => F::from(addr[*e][*j]),
                _ => F::ZERO,
            })
            .collect();
        // Off per instance when the present p of some r disagree.
        let (rho, fam) = (arrays[a].rho, arrays[a].family);
        let per_r = (0..1usize << rho).all(|r| {
            let on: Vec<bool> = (0..counts[fam])
                .map(|p| (p << rho) | r)
                .filter(|i| present(arrays, rfield, a, *i))
                .map(reads)
                .collect();
            on.iter()
                .all(|b| *b == on.first().copied().unwrap_or(false))
        });
        let mcol: Option<Vec<F>> = (!per_r).then(|| {
            (0..im.from.len())
                .map(|i| if reads(i) { F::ONE } else { F::ZERO })
                .collect()
        });
        let cols = columns.entry(a).or_default();
        im_cell.push(cols.len());
        cols.push(col);
        im_mask.push(mcol.map(|m| {
            cols.push(m);
            cols.len() - 1
        }));
    }
    let mut idx_of: HashMap<usize, usize> = HashMap::new();
    for (a, cols) in columns {
        let x = arrays.len();
        let (family, rho, r_count) = (arrays[a].family, arrays[a].rho, arrays[a].r_count);
        let name: &'static str = Box::leak(format!("{} index", arrays[a].name).into_boxed_str());
        let k = cols.len();
        arrays.push(Array {
            name,
            family,
            rho,
            r_count,
            fixed: true,
            lookups: Vec::new(),
            widths: vec![abits; k],
            rows: Vec::new(),
            values: cols,
        });
        let f = rfield[a];
        rfield.push(f);
        moved.push(vec![None; k]);
        idx_of.insert(a, x);
        index_arrays.push((a, x));
    }
    // Reads: lookups of (address, values), and the entries' multiplicities.
    let mut mult: Vec<Vec<u64>> = copies
        .exports
        .iter()
        .map(|e| vec![0u64; slots(arrays, e.array)])
        .collect();
    for (s, im) in copies.imports.iter().enumerate() {
        let a = im.array;
        for lin in &im.tuple {
            in_place(arrays, moved, a, lin);
        }
        let (rho, fam) = (arrays[a].rho, arrays[a].family);
        let n = 1usize << rho;
        let mut on = vec![F::ZERO; n];
        let mut off = false;
        if im_mask[s].is_none() {
            for (r, o) in on.iter_mut().enumerate() {
                let reads: Vec<bool> = (0..counts[fam])
                    .map(|p| (p << rho) | r)
                    .filter(|i| present(arrays, rfield, a, *i))
                    .map(|i| im.from[i].is_some())
                    .collect();
                if reads.is_empty() {
                    continue;
                }
                if reads[0] {
                    *o = F::ONE;
                } else {
                    off = true;
                }
            }
        }
        for i in (0..slots(arrays, a)).filter(|i| present(arrays, rfield, a, *i)) {
            let Some((e, j)) = im.from[i] else { continue };
            let ex = &copies.exports[e];
            assert!(
                present(arrays, rfield, ex.array, j),
                "{}: a read names a present entry",
                arrays[a].name
            );
            assert_eq!(
                im.tuple.len(),
                ex.tuple.len(),
                "{}: copy tuples of one width",
                arrays[a].name
            );
            for (k, (li, le)) in im.tuple.iter().zip(&ex.tuple).enumerate() {
                assert!(
                    eval_lin(arrays, pi, a, li, i) == eval_lin(arrays, pi, ex.array, le, j),
                    "{} slot {i}: copied value {k} equals {} slot {j}'s",
                    arrays[a].name,
                    arrays[ex.array].name
                );
            }
            mult[e][j] += 1;
        }
        let mut tuple = vec![addr_term(idx_of[&a], im_cell[s], n)];
        tuple.extend(im.tuple.iter().cloned());
        let li = arrays[a].lookups.len();
        arrays[a]
            .lookups
            .push((COPY_TABLE + tuple.len() as u32, tuple));
        if let Some(k) = im_mask[s] {
            mask_col.insert((a, li), (idx_of[&a], k));
        } else if off {
            mask.insert((a, li), on);
        }
    }
    // Entries: per export, an array of multiplicities and helpers that reads
    // the exported values in place.
    for (e, ex) in copies.exports.iter().enumerate() {
        let a = ex.array;
        for lin in &ex.tuple {
            in_place(arrays, moved, a, lin);
        }
        let (family, rho, r_count) = (arrays[a].family, arrays[a].rho, arrays[a].r_count);
        let n = 1usize << rho;
        let reframe = |lin: &Lin<F>| -> Lin<F> {
            lin.iter()
                .map(|t| Term {
                    src: match &t.src {
                        Src::Own(k) => Src::Link {
                            array: a,
                            cell: *k,
                            map: ident(n),
                        },
                        s => s.clone(),
                    },
                    coef: t.coef.clone(),
                })
                .collect()
        };
        let mut tuple = vec![addr_term(idx_of[&a], ex_cell[e], n)];
        tuple.extend(ex.tuple.iter().map(reframe));
        let mbits = (64 - mult[e].iter().max().copied().unwrap_or(0).leading_zeros()).max(1) as u16;
        let wa = arrays.len();
        let name: &'static str = Box::leak(format!("{} entries", arrays[a].name).into_boxed_str());
        let total = slots(arrays, a);
        arrays.push(Array {
            name,
            family,
            rho,
            r_count,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![mbits, 255],
            rows: vec![[Vec::new(), Vec::new(), Vec::new()]],
            values: vec![
                mult[e].iter().map(|m| F::from(*m)).collect(),
                vec![F::ZERO; total],
            ],
        });
        let f = rfield[a];
        rfield.push(f);
        moved.push(vec![None; 2]);
        sites.push((a, wa, COPY_TABLE + tuple.len() as u32, tuple));
        helpers.push(wa);
    }
    (sites, mask, mask_col, index_arrays, helpers)
}

/// Places the cell blocks into stripes of `2^c` active columns. Returns the
/// blocks, the rows used and each row's bit bound.
/// Placement key: fewest wasted cells first (`ZKS_PLACE_KEY=waste`) or the
/// earliest end first (the default).
fn by_waste() -> bool {
    false
}

/// Witness rows being filled: taken stripes per row, each row's class (0
/// while unused) and widest cell.
#[derive(Clone, Default)]
struct Rows {
    occ: Vec<u128>,
    class: Vec<u8>,
    bounds: Vec<usize>,
}

/// Where a block's present instances land in its window, per stripe-width
/// choice `g`: the window rows holding any, each with the stripes they reach
/// (bit `s` for stripe `s` of the block's `2^g`).
type Claim = Vec<(usize, u128)>;

/// The claims of an array of `2^m` instances on stripes of `2^c` columns,
/// for every `g` up to `max_g` (its instance's low `c + g` bits are columns).
fn claims(present: &[usize], m: usize, c: usize, max_g: usize) -> Vec<Claim> {
    if m < c {
        return Vec::new();
    }
    (0..=(m - c).min(max_g))
        .map(|g| {
            let lo = c + g;
            let mut rows: BTreeMap<usize, u128> = BTreeMap::new();
            for &i in present {
                *rows.entry(i >> lo).or_default() |= 1u128 << ((i & ((1usize << lo) - 1)) >> c);
            }
            rows.into_iter().collect()
        })
        .collect()
}

/// Places blocks of `2^m` cells on stripes of `2^c` columns, at rows from
/// `min_row` on, largest first. A block of `2^m >= 2^c` cells takes `2^g`
/// aligned stripes by `2^(m - c - g)` aligned rows (its instance's low `c + g`
/// bits are columns, the rest rows); smaller blocks share stripe rows of
/// their class. A block claims only the stripes of its window that hold a
/// present instance (`claims`): its stripes of absent instances, whole rows
/// or the tail of its last row, take other blocks.
/// A row holds one class (`class` from 1: rows commit at their widest cell).
/// Among the placements whose claimed rows end by `limits.0` and whose
/// windows end by `limits.1`, the earliest end wins (the fewest wasted cells
/// first with `ZKS_PLACE_KEY=waste`). `None` when a block has no such
/// placement.
#[allow(clippy::too_many_arguments)]
fn place(
    sizes: &[(usize, usize, usize, u16, u8)],
    claims: &[Vec<Claim>],
    present: &[usize],
    min_row: usize,
    limits: (usize, usize),
    cols_bits: usize,
    active: usize,
    c: usize,
    rows: &mut Rows,
    blocks_out: &mut [Vec<Block>],
) -> Option<(usize, usize)> {
    let stripes = active >> c;
    assert!((1..=128).contains(&stripes), "1 to 128 stripes");
    let cols = 1usize << cols_bits;
    let free = |rows: &Rows, row: usize, mask: u128, class: u8| {
        rows.occ.get(row).is_none_or(|m| m & mask == 0)
            && rows.class.get(row).is_none_or(|t| *t == 0 || *t == class)
    };
    let take = |rows: &mut Rows, row: usize, mask: u128, width: u16, class: u8| {
        if rows.occ.len() <= row {
            rows.occ.resize(row + 1, 0);
            rows.class.resize(row + 1, 0);
            rows.bounds.resize(row + 1, 1);
        }
        rows.occ[row] |= mask;
        rows.class[row] = class;
        rows.bounds[row] = rows.bounds[row].max(width as usize);
    };
    let (mut end, mut extent) = (min_row, min_row);
    let mut slots: std::collections::HashMap<u8, (usize, usize, usize)> =
        std::collections::HashMap::new();
    for &(a, k, m, width, class) in sizes {
        let block = if m >= c {
            // Key, then g, s0, start, height.
            let mut best: Option<((usize, usize, usize), usize, usize, usize, usize)> = None;
            for (g, claim) in claims[a].iter().enumerate() {
                let w = 1usize << g;
                if w > stripes {
                    break;
                }
                let h = 1usize << (m - c - g);
                let last = claim.last().map_or(0, |(r, _)| r + 1);
                let claimed = claim
                    .iter()
                    .map(|(_, s)| s.count_ones() as usize)
                    .sum::<usize>()
                    << c;
                let waste = claimed - present[a];
                for s0 in (0..stripes).step_by(w).filter(|s0| s0 + w <= stripes) {
                    let mut start = min_row.div_ceil(h) * h;
                    while start + last <= limits.0
                        && start + h <= limits.1
                        && !claim
                            .iter()
                            .all(|&(r, s)| free(rows, start + r, s << s0, class))
                    {
                        start += h;
                    }
                    if start + last > limits.0 || start + h > limits.1 {
                        continue;
                    }
                    let key = if by_waste() {
                        (waste, start + last, start + h)
                    } else {
                        (start + last, start + h, waste)
                    };
                    if best.is_none_or(|bst| key < bst.0) {
                        best = Some((key, g, s0, start, h));
                    }
                }
            }
            let (_, g, s0, start, h) = best?;
            let claim = &claims[a][g];
            for &(r, s) in claim {
                take(rows, start + r, s << s0, width, class);
            }
            end = end.max(start + claim.last().map_or(0, |(r, _)| r + 1));
            extent = extent.max(start + h);
            Block {
                base: start * cols + (s0 << c),
                lo: c + g,
                hi: cols_bits,
                m,
            }
        } else {
            let size = 1usize << m;
            let (row, s, offset) = match slots.get(&class) {
                Some(&(row, s, offset)) if offset + size <= 1 << c => (row, s, offset),
                _ => {
                    let mut row = min_row;
                    let s = loop {
                        if row >= limits.0 {
                            return None;
                        }
                        if let Some(s) = (0..stripes).find(|s| free(rows, row, 1u128 << s, class)) {
                            break s;
                        }
                        row += 1;
                    };
                    (row, s, 0)
                }
            };
            take(rows, row, 1u128 << s, width, class);
            slots.insert(class, (row, s, offset + size));
            end = end.max(row + 1);
            extent = extent.max(row + 1);
            Block {
                base: row * cols + (s << c) + offset,
                lo: m,
                hi: cols_bits,
                m,
            }
        };
        blocks_out[a][k] = block;
    }
    Some((end, extent))
}

impl<F: Scalar> UniformCircuit<F> {
    /// Erase retained construction values after the last prover use. Public
    /// program constants remain available for exact program authentication.
    pub fn zeroize_witness_values(&mut self)
    where F: zeroize::Zeroize {
        for array in &mut self.arrays {
            if !array.fixed {
                for values in &mut array.values {
                    zeroize::Zeroize::zeroize(values);
                }
            }
        }
    }
    fn m(&self, a: usize) -> usize {
        self.pi[self.arrays[a].family] + self.arrays[a].rho
    }

    /// The witness index of cell `k` of instance `i` of array `a` (a moved
    /// chunk cell sits in its chunk array).
    pub fn cell_index(&self, a: usize, k: usize, i: usize) -> usize {
        if let Some(Some((a2, shift))) = self.split_of.get(a)
            && i >= *shift
        {
            return self.cell_index(*a2, k, i - shift);
        }
        match self.moved[a][k] {
            Some((ca, kc, j)) => {
                let rho = self.arrays[a].rho;
                let (p, r) = (i >> rho, i & ((1 << rho) - 1));
                self.blocks[ca][kc].index((p << self.arrays[ca].rho) | (j << rho) | r)
            }
            None => self.blocks[a][k].index(i),
        }
    }

    /// Whether r of array `a` is an instance (for every present p).
    fn r_present(&self, a: usize, r: usize) -> bool {
        field_present(self.rfield[a], r)
    }

    /// Present instances of array `a`.
    pub fn instances(&self, a: usize) -> usize {
        self.counts[self.arrays[a].family]
            * (0..1usize << self.arrays[a].rho)
                .filter(|r| self.r_present(a, *r))
                .count()
    }

    /// Cells of array `a` in the witness (moved chunk cells excluded).
    pub fn placed_cells(&self, a: usize) -> usize {
        if self.is_io(a) {
            return 0;
        }
        self.moved[a].iter().filter(|m| m.is_none()).count()
    }

    /// The matrices' degrees in the LogUp challenges: 1 in alpha if any row
    /// carries `alpha - t`, and the highest power of beta.
    pub fn challenge_degrees(&self) -> (usize, usize) {
        if let Some((_, degrees)) = self.verifier_only { return degrees; }
        let (mut da, mut db) = (0, 0);
        for row in self.symbolic().iter().flatten() {
            for (_, f) in row.iter().flatten() {
                match f {
                    Factor::One => {}
                    Factor::AlphaMinus(_) => da = 1,
                    Factor::NegBeta(e) => db = db.max(*e),
                }
            }
        }
        (da, db)
    }

    /// Whether array `a` sits in z's public half.
    pub fn is_io(&self, a: usize) -> bool {
        self.io_arrays.iter().any(|(x, _)| *x == a)
    }

    /// The constraint row of template row `j` of instance `i` of array `a`.
    pub fn row_index(&self, a: usize, j: usize, i: usize) -> usize {
        self.row_base[a][j] + i
    }

    /// A global array: its family has no p range (any family may link to it).
    fn global(&self, a: usize) -> bool {
        self.pi[self.arrays[a].family] == 0
    }

    /// Whether slot `i` of array `a` is an instance (`p < count`, r present).
    fn present(&self, a: usize, i: usize) -> bool {
        let arr = &self.arrays[a];
        (i >> arr.rho) < self.counts[arr.family] && self.r_present(a, i & ((1 << arr.rho) - 1))
    }

    /// Lays out the arrays for rows of `2^col_bits` columns and builds the
    /// instance and witness.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        label: &'static str,
        arrays: Vec<Array<F>>,
        pi: Vec<usize>,
        counts: Vec<usize>,
        table_specs: Vec<(usize, u32)>,
        col_bits: usize,
        io_len: usize,
        seed: &[u8],
    ) -> (Self, Instance<F>, Witness<F>) {
        Self::build_with_copies(
            label,
            arrays,
            pi,
            counts,
            table_specs,
            Copies::default(),
            col_bits,
            io_len,
            seed,
        )
    }

    /// [`Self::build`] with a copy argument: values flow between arrays of
    /// any families through the copy table (LogUp over address and values).
    #[allow(clippy::too_many_arguments)]
    pub fn build_with_copies(
        label: &'static str,
        arrays: Vec<Array<F>>,
        pi: Vec<usize>,
        counts: Vec<usize>,
        table_specs: Vec<(usize, u32)>,
        copies: Copies<F>,
        col_bits: usize,
        io_len: usize,
        seed: &[u8],
    ) -> (Self, Instance<F>, Witness<F>) {
        Self::build_with_copies_at(
            label,
            arrays,
            pi,
            counts,
            table_specs,
            copies,
            col_bits,
            io_len,
            seed,
            None,
        )
    }

    /// [`Self::build_with_copies`] at another circuit's layout (`target`):
    /// its constraint variables, rows and first-phase rows, the extra rows
    /// left empty (each an empty row's blinding term in the proof), and its
    /// fixed rows, which must match as they are (an all-zero fixed row would
    /// commit to the identity, which no verifier takes as an input). Panics
    /// when the circuit does not fit the target.
    #[allow(clippy::too_many_arguments)]
    pub fn build_with_copies_at(
        label: &'static str,
        mut arrays: Vec<Array<F>>,
        mut pi: Vec<usize>,
        mut counts: Vec<usize>,
        table_specs: Vec<(usize, u32)>,
        copies: Copies<F>,
        col_bits: usize,
        io_len: usize,
        seed: &[u8],
        target: Option<Layout>,
    ) -> (Self, Instance<F>, Witness<F>) {
        for (f, (&pf, &cf)) in pi.iter().zip(&counts).enumerate() {
            assert!(cf <= 1 << pf, "family {f}: count fits 2^pi");
        }
        let mut copies = copies;
        if copies.product_masks.is_empty() {
            // Every uniform circuit needs private masking even when it has no
            // transcript pool. These gates are real rows of its relation.
            let family = pi.len();
            pi.push(1);
            counts.push(2);
            let array = arrays.len();
            arrays.push(Array {
                name: "independent opening masks", family, rho: 0, r_count: 1,
                fixed: false, lookups: vec![], widths: vec![255; 3],
                rows: vec![[own(0, 1), own(1, 1), own(2, 1)]],
                values: vec![vec![F::ZERO; 2]; 3],
            });
            copies.product_masks.push((array, [0, 1, 2]));
        }
        let mut product_masks = Vec::new();
        for &(a, cells) in &copies.product_masks {
            let mask = &arrays[a];
            assert!(!mask.fixed && mask.lookups.is_empty() && mask.cells() == 3,
                "private masking is an independent three-cell product array");
            assert_eq!(cells, [0, 1, 2]);
            assert_eq!(mask.rows.len(), 1);
            let n = 1usize << mask.rho;
            for k in 0..3 {
                assert_eq!(mask.widths[k], 255);
                let lin = &mask.rows[0][k];
                assert!(lin.len() == 1 && lin[0].src == Src::Own(k)
                    && lin[0].coef == vec![F::ONE; n], "exact private product gate");
            }
            assert!(!copies.io_arrays.iter().any(|(io, _)| *io == a));
            assert!(!copies.exports.iter().any(|e| e.array == a));
            assert!(!copies.imports.iter().any(|e| e.array == a));
            for (reader, array) in arrays.iter().enumerate() {
                if reader == a { continue; }
                for row in &array.rows {
                    for term in row.iter().flatten() {
                        assert!(!matches!(term.src, Src::Link { array, .. } | Src::All { array, .. } if array == a),
                            "a private product mask is not an application input");
                    }
                }
            }
            for p in 0..counts[mask.family] {
                for r in 0..mask.r_count {
                    product_masks.push((a, cells, (p << mask.rho) + r));
                }
            }
        }
        let unique_masks: HashSet<_> = product_masks.iter().copied().collect();
        assert_eq!(unique_masks.len(), product_masks.len(), "distinct private product mask cells");
        assert!(product_masks.len() >= 2, "two independent private product masks");
        let split_of = split_families(&mut arrays, &mut pi, &mut counts, &mut copies);
        let challenge = std::mem::take(&mut copies.challenge);
        let mut io_arrays = std::mem::take(&mut copies.io_arrays);
        let late_io = std::mem::take(&mut copies.late_io);
        let wallet = copies.wallet.take();
        let wallet_rows = usize::from(wallet.is_some());
        let link = std::mem::take(&mut copies.link);
        // The link's io arrays are io arrays whose values come from rho.
        io_arrays.extend(link.io.iter().copied());
        let link_slots: HashSet<usize> = link.io.iter().map(|x| x.1).collect();
        let is_io: Vec<bool> = (0..arrays.len())
            .map(|a| io_arrays.iter().any(|(x, _)| *x == a))
            .collect();
        // Link-phase arrays (with their split families' clones).
        let mut link_array: Vec<bool> = vec![false; arrays.len()];
        for &a in &link.arrays {
            link_array[a] = true;
            if let Some(Some((c, _))) = split_of.get(a) {
                link_array[*c] = true;
            }
        }
        for &(a, off) in &io_arrays {
            let arr = &arrays[a];
            assert!(
                arr.cells() == 1 && arr.lookups.is_empty() && arr.rows.is_empty() && !arr.fixed,
                "{}: a one-cell io array",
                arr.name
            );
            assert!(
                pi[arr.family] == 0 && counts[arr.family] == 1,
                "{}: an io array is global",
                arr.name
            );
            assert_eq!(
                off % (1usize << arr.rho),
                0,
                "{}: io offset aligned",
                arr.name
            );
        }
        let mut rfield: Vec<(usize, usize, usize)> =
            arrays.iter().map(|a| (a.rho, a.r_count, 1)).collect();
        let mut moved: Vec<Vec<Option<(usize, usize, usize)>>> =
            arrays.iter().map(|a| vec![None; a.cells()]).collect();
        let before_chunks = arrays.len();
        chunk_arrays(&mut arrays, &mut rfield, &mut moved);
        // A link array's chunks move into chunk arrays of the link phase.
        link_array.resize(arrays.len(), false);
        for a in 0..before_chunks {
            if link_array[a] {
                for &(ca, _, _) in moved[a].iter().flatten() {
                    link_array[ca] = true;
                }
            }
        }
        // The copy argument: index columns, reads as lookups, entry arrays.
        let (exports, lookup_mask, lookup_mask_col, index_arrays, mut export_helpers) =
            copy_layout(&mut arrays, &pi, &counts, &mut rfield, &mut moved, &copies);
        // LogUp: helper cells and placeholder rows per lookup, a multiplicity
        // and helper array per table, and one sum row.
        let n_user = arrays.len();
        let mut phase2: Vec<Vec<bool>> =
            arrays.iter().map(|arr| vec![false; arr.cells()]).collect();
        for &wa in &export_helpers {
            phase2[wa][1] = true;
        }
        let mut lookup_arrays = Vec::new();
        for a in 0..n_user {
            let lk = arrays[a].lookups.len();
            if lk == 0 {
                continue;
            }
            let slots = 1usize << (pi[arrays[a].family] + arrays[a].rho);
            let (first_cell, first_row) = (arrays[a].cells(), arrays[a].rows.len());
            for _ in 0..lk {
                arrays[a].widths.push(255);
                arrays[a].values.push(vec![F::ZERO; slots]);
                arrays[a].rows.push([Vec::new(), Vec::new(), Vec::new()]);
                phase2[a].push(true);
                moved[a].push(None);
            }
            lookup_arrays.push((a, first_cell, first_row));
        }
        // Builder rows over the challenges: a placeholder row each (the
        // symbolic rows write them), and their cells in the second phase.
        let mut ch_rows: Vec<(usize, usize, [Vec<(Term<F>, Factor)>; 3])> = Vec::new();
        for (a, row) in challenge.rows {
            ch_rows.push((a, arrays[a].rows.len(), row));
            arrays[a].rows.push([Vec::new(), Vec::new(), Vec::new()]);
        }
        for &(a, k) in &challenge.cells {
            phase2[a][k] = true;
        }
        // Helpers of lookups into a fixed table take few distinct values (one
        // per table entry), so their rows can be committed by value; copy
        // reads' helpers, entries' helpers and the tables' own helpers cannot.
        let mut by_value: Vec<Vec<bool>> =
            arrays.iter().map(|arr| vec![false; arr.cells()]).collect();
        for &(a, first_cell, _) in &lookup_arrays {
            for (j, (tid, _)) in arrays[a].lookups.iter().enumerate() {
                by_value[a][first_cell + j] = *tid < COPY_TABLE;
            }
        }
        let mut tables = Vec::new();
        for &(va, tid) in &table_specs {
            let (fam, rho, r_count) = (arrays[va].family, arrays[va].rho, arrays[va].r_count);
            let slots = 1usize << (pi[fam] + rho);
            let fva = rfield[va];
            // Index the entries, then count every lookup into this table.
            let present =
                |i: usize| (i >> rho) < counts[fam] && field_present(fva, i & ((1 << rho) - 1));
            let key = |vals: &[F]| -> Vec<u8> {
                vals.iter()
                    .flat_map(|v| v.to_repr().as_ref().to_vec())
                    .collect()
            };
            let mut index: HashMap<Vec<u8>, usize> = HashMap::new();
            for i in (0..slots).filter(|i| present(*i)) {
                let entry: Vec<F> = arrays[va].values.iter().map(|col| col[i]).collect();
                index.insert(key(&entry), i);
            }
            let mut mult = vec![0u64; slots];
            for &(a, _, _) in &lookup_arrays {
                let arr = &arrays[a];
                let (arho, afam) = (arr.rho, arr.family);
                for i in 0..1usize << (pi[afam] + arho) {
                    if (i >> arho) >= counts[afam]
                        || !field_present(rfield[a], i & ((1 << arho) - 1))
                    {
                        continue;
                    }
                    for (t, tuple) in &arr.lookups {
                        if *t != tid {
                            continue;
                        }
                        let vals: Vec<F> = tuple
                            .iter()
                            .map(|lin| eval_lin(&arrays, &pi, a, lin, i))
                            .collect();
                        let slot = *index.get(&key(&vals)).unwrap_or_else(|| {
                            panic!("{}: a looked-up tuple is in table {tid}", arr.name)
                        });
                        mult[slot] += 1;
                    }
                }
            }
            // The multiplicities' width class from the lookup count, not from
            // the largest multiplicity: which entries the values hit is data,
            // and a width taken from it would move the column between width
            // classes (so the placement and the matrices) from one witness to
            // the next. A verifier slot taking either of two circuits relies
            // on one layout per circuit.
            let lookups_n: u64 = mult.iter().sum();
            let mbits = (64 - lookups_n.leading_zeros()).max(1) as u16;
            let wa = arrays.len();
            arrays.push(Array {
                name: "table helpers",
                family: fam,
                rho,
                r_count,
                fixed: false,
                lookups: Vec::new(),
                widths: vec![mbits, 255],
                rows: vec![[Vec::new(), Vec::new(), Vec::new()]],
                values: vec![
                    mult.iter().map(|m| F::from(*m)).collect(),
                    vec![F::ZERO; slots],
                ],
            });
            phase2.push(vec![false, true]);
            rfield.push(fva);
            moved.push(vec![None; 2]);
            tables.push((va, tid, wa));
            // A link table's multiplicities follow the link phase's values.
            link_array.resize(arrays.len(), false);
            if link.tables.contains(&tid) {
                link_array[wa] = true;
            }
        }
        if !lookup_arrays.is_empty() || !exports.is_empty() {
            let mut sum: Lin<F> = Vec::new();
            for &(a, first, _) in &lookup_arrays {
                for k in 0..arrays[a].lookups.len() {
                    sum.push(Term {
                        src: Src::All {
                            array: a,
                            cell: first + k,
                        },
                        coef: vec![F::ONE],
                    });
                }
            }
            for &(_, _, wa) in &tables {
                sum.push(Term {
                    src: Src::All { array: wa, cell: 1 },
                    coef: vec![-F::ONE],
                });
            }
            for wa in export_helpers.drain(..) {
                sum.push(Term {
                    src: Src::All { array: wa, cell: 1 },
                    coef: vec![-F::ONE],
                });
            }
            pi.push(0);
            counts.push(1);
            arrays.push(Array {
                name: "lookup sum",
                family: pi.len() - 1,
                rho: 0,
                r_count: 1,
                fixed: false,
                lookups: Vec::new(),
                widths: Vec::new(),
                rows: vec![[sum, constant(vec![F::ONE]), Vec::new()]],
                values: Vec::new(),
            });
            phase2.push(Vec::new());
            rfield.push((0, 1, 1));
            moved.push(Vec::new());
        }
        let arrays = arrays;
        let (pi, counts) = (pi, counts);
        let m_of = |a: usize| pi[arrays[a].family] + arrays[a].rho;
        for (a, arr) in arrays.iter().enumerate() {
            assert_eq!(
                arr.values.len(),
                arr.cells(),
                "{}: values per cell",
                arr.name
            );
            for v in &arr.values {
                assert_eq!(
                    v.len(),
                    1 << m_of(a),
                    "{}: one value per instance",
                    arr.name
                );
            }
        }
        // Every link reads an instance of its target: a slot that is not one
        // holds a cell no row constrains.
        let check = |a: usize, lin: &Lin<F>| {
            for t in lin {
                if let Src::Link { array, cell, map } = &t.src {
                    for (r, m) in map.iter().enumerate() {
                        let Some(rr) = m else { continue };
                        if field_present(rfield[a], r) && !bool::from(t.coef[r].is_zero()) {
                            assert!(
                                field_present(rfield[*array], *rr),
                                "{} reads {} cell {cell} at r {rr}, not an instance",
                                arrays[a].name,
                                arrays[*array].name
                            );
                        }
                    }
                }
            }
        };
        for (a, arr) in arrays.iter().enumerate() {
            for row in &arr.rows {
                for lin in row {
                    check(a, lin);
                }
            }
            for (_, tuple) in &arr.lookups {
                for lin in tuple {
                    check(a, lin);
                }
            }
        }
        for (_, wa, _, tuple) in &exports {
            for lin in tuple {
                check(*wa, lin);
            }
        }
        // Constraint blocks, largest first, each aligned to its size. A block's
        // rows of absent instances have zero coefficients, so other blocks'
        // present rows may sit there: each block takes the lowest aligned base
        // where its present rows meet no placed present row.
        let mut cblocks: Vec<(usize, usize, usize)> = arrays
            .iter()
            .enumerate()
            .flat_map(|(a, arr)| (0..arr.rows.len()).map(move |j| (a, j, m_of(a))))
            .collect();
        cblocks.sort_by_key(|(_, _, m)| std::cmp::Reverse(*m));
        let mut row_base: Vec<Vec<usize>> =
            arrays.iter().map(|arr| vec![0; arr.rows.len()]).collect();
        let present_rows: Vec<Vec<usize>> = (0..arrays.len())
            .map(|a| {
                let (rho, fam) = (arrays[a].rho, arrays[a].family);
                (0..1usize << m_of(a))
                    .filter(|i| {
                        (i >> rho) < counts[fam] && field_present(rfield[a], i & ((1 << rho) - 1))
                    })
                    .collect()
            })
            .collect();
        let mut used: Vec<u64> = Vec::new();
        let is_used =
            |used: &Vec<u64>, x: usize| used.get(x >> 6).is_some_and(|w| (w >> (x & 63)) & 1 == 1);
        let mut first_free = 0usize;
        let mut constraints = 0usize;
        for (a, j, m) in &cblocks {
            let size = 1usize << m;
            let rows = &present_rows[*a];
            let mut base = (first_free / size) * size;
            while rows.iter().any(|i| is_used(&used, base + i)) {
                base += size;
            }
            for i in rows {
                let x = base + i;
                if used.len() <= x >> 6 {
                    used.resize((x >> 6) + 1, 0);
                }
                used[x >> 6] |= 1 << (x & 63);
            }
            while is_used(&used, first_free) {
                first_free += 1;
            }
            row_base[*a][*j] = base;
            constraints = constraints.max(base + rows.last().map_or(0, |i| i + 1));
        }
        let constraints = constraints.max(1);
        if std::env::var("ZKS_CPLACE").is_ok() {
            // Constraint blocks by base: size, present rows, end.
            let mut v: Vec<(usize, usize, usize, usize, &str, usize)> = cblocks
                .iter()
                .map(|&(a, j, m)| {
                    (
                        row_base[a][j],
                        1usize << m,
                        present_rows[a].len(),
                        row_base[a][j] + present_rows[a].last().map_or(0, |i| i + 1),
                        arrays[a].name,
                        j,
                    )
                })
                .collect();
            v.sort();
            for (base, size, n, end, name, j) in v {
                eprintln!(
                    "cblock base {base:7} size {size:6} present {n:6} end {end:7} {name} row {j}"
                );
            }
            eprintln!("constraint extent {constraints}");
        }
        for arr in &arrays {
            assert!(
                !arr.fixed || arr.rows.is_empty(),
                "{}: a constant column has no template rows",
                arr.name
            );
        }
        // Regions: 0 first phase, 1 second phase, 2 constant columns, 3 the
        // link phase (between the first and the second).
        let link_array = {
            let mut l = link_array;
            l.resize(arrays.len(), false);
            l
        };
        let collect_sizes = |region: usize| -> Vec<(usize, usize, usize, u16)> {
            let mut v: Vec<(usize, usize, usize, u16)> = arrays
                .iter()
                .enumerate()
                .flat_map(|(a, arr)| (0..arr.cells()).map(move |k| (a, k)))
                .filter(|&(a, k)| {
                    let r = if arrays[a].fixed {
                        2
                    } else if phase2[a][k] {
                        1
                    } else if link_array[a] {
                        3
                    } else {
                        0
                    };
                    r == region
                        && moved[a][k].is_none()
                        && !is_io.get(a).copied().unwrap_or(false)
                        && wallet.is_none_or(|(wa, _)| wa != a)
                })
                .map(|(a, k)| (a, k, m_of(a), arrays[a].widths[k]))
                .collect();
            v.sort_by_key(|(_, _, m, w)| (std::cmp::Reverse(*m), std::cmp::Reverse(*w)));
            v
        };
        let sizes = collect_sizes(0);
        let p2_sizes = collect_sizes(1);
        let fixed_sizes = collect_sizes(2);
        let link_sizes = collect_sizes(3);
        // A block claims only the stripes holding its array's present
        // instances: no row reads an absent instance with a nonzero
        // coefficient, so other blocks' cells may sit on the rest.
        let present_n: Vec<usize> = present_rows.iter().map(Vec::len).collect();
        // First-phase cells by width class: a row holds one class (it commits
        // at its widest cell); a class smaller than about a row joins the next
        // wider one. Second-phase cells are a class of their own.
        // `ZKS_CLASSES` (comma-separated bit bounds, the last 255) overrides
        // the classes.
        let classes: Vec<u16> = vec![1, 11, 20, 88, 180, 255];
        let classes = classes.as_slice();
        let class_of = |w: u16| {
            classes
                .iter()
                .position(|c| w <= *c)
                .unwrap_or(classes.len() - 1)
        };
        let mut class_map: Vec<usize> = (0..classes.len()).collect();
        let row_cells = 1usize << col_bits;
        let mut class_cells = vec![0usize; classes.len()];
        for &(_, _, m, w) in &sizes {
            class_cells[class_map[class_of(w)]] += 1usize << m;
        }
        for i in 0..classes.len() - 1 {
            if class_cells[i] < row_cells {
                class_cells[i + 1] += std::mem::take(&mut class_cells[i]);
                for x in class_map.iter_mut().filter(|x| **x == i) {
                    *x = i + 1;
                }
            }
        }
        let p1_list: Vec<(usize, usize, usize, u16, u8)> = sizes
            .iter()
            .map(|&(a, k, m, w)| (a, k, m, w, 1 + class_map[class_of(w)] as u8))
            .collect();
        // Link-phase cells: a class of their own, on the rows after the first
        // phase's.
        let link_list: Vec<(usize, usize, usize, u16, u8)> = link_sizes
            .iter()
            .map(|&(a, k, m, w)| (a, k, m, w, 3 + classes.len() as u8))
            .collect();
        // Second-phase cells: helpers committed by value on rows of their own
        // first, then the rest.
        // (Arrays added after the lookups, the tables' helpers among them,
        // have no by-value cells.)
        // Only when they fill at least two rows: a row is summed by value at
        // three or more cells per distinct value (`spartan::grouped_row_sum`),
        // and a short class only spreads the second phase over more rows.
        let by_value_of = |a: usize, k: usize| {
            by_value
                .get(a)
                .and_then(|v| v.get(k))
                .copied()
                .unwrap_or(false)
        };
        let by_value_cells: usize = p2_sizes
            .iter()
            .filter(|&&(a, k, _, _)| by_value_of(a, k))
            .map(|&(a, _, _, _)| present_n[a])
            .sum();
        let split = by_value_cells >= 2usize << col_bits;
        let is_by_value = |a: usize, k: usize| !split || by_value_of(a, k);
        let p2_list: Vec<(usize, usize, usize, u16, u8)> = p2_sizes
            .iter()
            .filter(|&&(a, k, _, _)| is_by_value(a, k))
            .map(|&(a, k, m, w)| (a, k, m, w, 1 + classes.len() as u8))
            .collect();
        let p2_rest: Vec<(usize, usize, usize, u16, u8)> = p2_sizes
            .iter()
            .filter(|&&(a, k, _, _)| !is_by_value(a, k))
            .map(|&(a, k, m, w)| (a, k, m, w, 2 + classes.len() as u8))
            .collect();
        let fixed_list: Vec<(usize, usize, usize, u16, u8)> = fixed_sizes
            .iter()
            .map(|&(a, k, m, w)| (a, k, m, w, 1))
            .collect();
        let mut blocks: Vec<Vec<Block>> = arrays
            .iter()
            .map(|arr| {
                vec![
                    Block {
                        base: 0,
                        lo: 0,
                        hi: 0,
                        m: 0
                    };
                    arr.cells()
                ]
            })
            .collect();
        let sx = constraints.next_power_of_two().trailing_zeros() as usize;
        if let Some(t) = target {
            assert!(
                sx <= t.sx,
                "{label}: 2^{sx} constraints fit the target's 2^{}",
                t.sx
            );
        }
        let sx = target.map_or(sx, |t| t.sx);
        let p1_min = target.map_or(0, |t| t.p1);
        let pl_min = target.map_or(0, |t| t.pl);
        // The fixed rows the constants need (a target may ask for more).
        let mut natural_fixed;
        let mut fixed_rows = 0;
        let mut p1_rows = 0;
        let pl_rows;
        let mut p2_rows = 0;
        let mut row_bits = target.map_or(2, |t| t.row_bits);
        let mut stripe_c = 0;
        let (shape, bounds) = 'shape: loop {
            let s = Shape {
                label,
                sx,
                row_bits,
                col_bits,
                k: 3,
            };
            // The last MASK_COLS active columns of some rows hold the masking
            // values: no stripe reaches them.
            let active = s.cols() - s.tail_len() - crate::r1cs::MASK_COLS;
            // Stripes of 2^c columns: narrow stripes waste less of a block's
            // last, partly present row (at most 128 stripes).
            let stripe_bits = 10usize;
            let c = (active.ilog2() as usize)
                .min(stripe_bits)
                .max((active >> 7).max(1).next_power_of_two().trailing_zeros() as usize);
            let max_g = (active >> c).ilog2() as usize;
            let claims_v: Vec<Vec<Claim>> = present_rows
                .iter()
                .enumerate()
                .map(|(a, present)| claims(present, m_of(a), c, max_g))
                .collect();
            // Constant columns on rows of their own, moved to the end: their
            // height is a multiple of their tallest block's, so the move keeps
            // every block aligned.
            let mut frows_ = Rows::default();
            let all = (usize::MAX, usize::MAX);
            let (ef, xf) = if fixed_list.is_empty() {
                (0, 0)
            } else {
                place(
                    &fixed_list,
                    &claims_v,
                    &present_n,
                    0,
                    all,
                    col_bits,
                    active,
                    c,
                    &mut frows_,
                    &mut blocks,
                )
                .expect("unbounded")
            };
            let tallest = fixed_sizes
                .iter()
                .map(|&(a, k, m, _)| {
                    if m > blocks[a][k].lo {
                        1usize << (m - blocks[a][k].lo)
                    } else {
                        1
                    }
                })
                .max()
                .unwrap_or(1);
            let frows = ef.max(xf).div_ceil(tallest) * tallest;
            natural_fixed = frows;
            // At a target layout, more fixed rows than the constants need:
            // the constants keep the first rows of the fixed region (moved
            // by a multiple of their tallest block, so still aligned) and the
            // rest get a padding constant below.
            let room = match target {
                Some(t) => {
                    let room = t.fixed - wallet_rows;
                    assert!(
                        frows <= room && room % tallest == 0,
                        "{label}: {frows} fixed rows (blocks of {tallest}) fit the target's {room} (besides its wallet rows)"
                    );
                    room
                }
                None => frows,
            };
            // The wallet rows sit just before the constants' rows.
            let frows = room + wallet_rows;
            if frows < s.rows() {
                // The fewest rows that hold both phases: a row budget for the
                // marked rows (full blocks may run on to the end), by bisection.
                let attempt = |budget: usize,
                               blocks: &mut Vec<Vec<Block>>|
                 -> Option<(Rows, usize, usize, usize)> {
                    let limits = (budget, s.rows());
                    let mut rows = Rows::default();
                    // Classes on bands of rows, narrowest first.
                    let (mut e1, mut x1) = (0, 0);
                    for class in 1..=classes.len() as u8 {
                        let list: Vec<_> =
                            p1_list.iter().copied().filter(|e| e.4 == class).collect();
                        if !list.is_empty() {
                            let (e, x) = place(
                                &list, &claims_v, &present_n, e1, limits, col_bits, active, c,
                                &mut rows, blocks,
                            )?;
                            e1 = e;
                            x1 = x1.max(x);
                        }
                    }
                    let p1 = e1.max(1).max(p1_min);
                    // The link phase, from the first phase's end.
                    let (el, xl) = if link_list.is_empty() {
                        (p1, p1)
                    } else {
                        place(
                            &link_list, &claims_v, &present_n, p1, limits, col_bits, active, c,
                            &mut rows, blocks,
                        )?
                    };
                    let el = el.max(p1 + pl_min);
                    let (e2, x2) = if p2_list.is_empty() {
                        (el, el)
                    } else {
                        place(
                            &p2_list, &claims_v, &present_n, el, limits, col_bits, active, c,
                            &mut rows, blocks,
                        )?
                    };
                    let (e2, x2) = if p2_rest.is_empty() {
                        (e2, x2)
                    } else {
                        let (e, x) = place(
                            &p2_rest, &claims_v, &present_n, e2, limits, col_bits, active, c,
                            &mut rows, blocks,
                        )?;
                        (e, x2.max(x))
                    };
                    (x1.max(xl).max(x2) <= s.rows()).then_some((rows, p1, el, e2))
                };
                let max_budget = s.rows() - frows;
                if attempt(max_budget, &mut blocks).is_some() {
                    let cap = (active >> c) << c;
                    let need: usize = sizes
                        .iter()
                        .chain(&link_sizes)
                        .chain(&p2_sizes)
                        .map(|&(a, _, _, _)| present_n[a])
                        .sum();
                    let (mut lo, mut hi) = (need.div_ceil(cap).min(max_budget), max_budget);
                    while lo < hi {
                        let mid = (lo + hi) / 2;
                        if attempt(mid, &mut blocks).is_some() {
                            hi = mid;
                        } else {
                            lo = mid + 1;
                        }
                    }
                    let (rows, p1, el, e2) =
                        attempt(hi, &mut blocks).expect("the bisection's budget fits");
                    let shift = (s.rows() - room) * s.cols();
                    for &(a, k, _, _) in &fixed_sizes {
                        blocks[a][k].base += shift;
                    }
                    fixed_rows = frows;
                    p1_rows = p1;
                    pl_rows = el - p1;
                    p2_rows = e2 - el;
                    let mut b = rows.bounds;
                    b.resize(s.rows(), 1);
                    for x in b.iter_mut() {
                        *x = (*x).max(1);
                    }
                    for r in s.rows() - frows..s.rows() {
                        b[r] = 255;
                    }
                    stripe_c = c;
                    break 'shape (s, b);
                }
            }
            row_bits += 1;
        };
        let _ = (&mut fixed_rows, &mut p1_rows);
        if let Some(t) = target {
            let got = Layout {
                sx: shape.sx,
                row_bits: shape.row_bits,
                p1: p1_rows,
                pl: pl_rows,
                fixed: fixed_rows,
            };
            assert_eq!(got, t, "{label}: built at the target layout");
        }
        for &(a, off) in &io_arrays {
            let m = m_of(a);
            blocks[a][0] = Block {
                base: (1usize << shape.sw()) + off,
                lo: m,
                hi: shape.col_bits,
                m,
            };
        }
        if let Some((wa, _)) = wallet {
            let m = m_of(wa);
            let base = (shape.rows() - fixed_rows) * shape.cols();
            assert!(
                (arrays[wa].cells() << m) <= shape.active_cols(),
                "{label}: the wallet row holds its array"
            );
            for (k, b) in blocks[wa].iter_mut().enumerate() {
                *b = Block {
                    base: base + (k << m),
                    lo: m,
                    hi: shape.col_bits,
                    m,
                };
            }
        }
        // Every block is aligned: its base has no bit where an instance bit
        // lands (the matrix evaluation factors on exactly that).
        for (a, arr) in arrays.iter().enumerate() {
            for (k, b) in blocks[a]
                .iter()
                .enumerate()
                .filter(|(k, _)| moved[a][*k].is_none())
            {
                for t in 0..m_of(a) {
                    assert_eq!(
                        (b.base >> b.wbit(t)) & 1,
                        0,
                        "{} cell {k}: block aligned",
                        arr.name
                    );
                }
            }
        }
        if std::env::var("ZKS_PLACE").is_ok() {
            // Placement dump: per region, each block's rows, stripes and use.
            let cols = shape.cols();
            for (region, list) in [(0usize, &sizes), (1, &p2_sizes), (2, &fixed_sizes)] {
                let mut v: Vec<(usize, usize, String)> = list
                    .iter()
                    .map(|&(a, k, m, wd)| {
                        let b = &blocks[a][k];
                        let (row, col) = (b.base / cols, b.base % cols);
                        let h = if m > b.lo { 1usize << (m - b.lo) } else { 1 };
                        let wcols = 1usize << b.lo.min(shape.col_bits);
                        let present = present_rows[a].len();
                        // Positions the block claims: its marked rows' stripes
                        // (a block below one stripe row claims its own cells).
                        let marked: HashSet<usize> = present_rows[a].iter().map(|&i| b.index(i) >> stripe_c).collect();
                        let claimed = if m >= stripe_c { marked.len() << stripe_c } else { 1usize << m };
                        // The fewest positions any choice of column bits would
                        // claim (the low bits of p, of r mod 2^sigma and of
                        // r >> sigma, each a prefix field).
                        let rho = arrays[a].rho;
                        let (sigma, inner, outer) = rfield[a];
                        let fields = [(m - rho, counts[arrays[a].family].max(1)), (sigma, inner), (rho - sigma, outer)];
                        let mut best = usize::MAX;
                        if m >= stripe_c {
                            for ap in 0..=fields[0].0.min(stripe_c) {
                                for asg in 0..=fields[1].0.min(stripe_c - ap) {
                                    let aj = stripe_c - ap - asg;
                                    if aj > fields[2].0 {
                                        continue;
                                    }
                                    let cells = fields[0].1.div_ceil(1 << ap) * fields[1].1.div_ceil(1 << asg) * fields[2].1.div_ceil(1 << aj);
                                    best = best.min(cells << stripe_c);
                                }
                            }
                        } else {
                            best = 1usize << m;
                        }
                        (row, col, format!("r{region} row {row:3}+{h:<3} col {col:5}+{wcols:<6} m {m:2} w {wd:3} {:>28} cell {k:2} present {present}/{} claimed {claimed} best {best}", arrays[a].name, 1usize << m))
                    })
                    .collect();
                v.sort();
                for (_, _, line) in v {
                    eprintln!("{line}");
                }
            }
        }
        let cols = shape.cols();
        let mut w = vec![F::ZERO; shape.rows() * cols];
        // Every present cell has a position of its own.
        let mut taken = vec![0u64; (shape.rows() * cols).div_ceil(64)];
        for (a, arr) in arrays
            .iter()
            .enumerate()
            .filter(|(a, _)| !is_io.get(*a).copied().unwrap_or(false))
        {
            let (rho, fam) = (arr.rho, arr.family);
            for (k, vals) in arr
                .values
                .iter()
                .enumerate()
                .filter(|(k, _)| moved[a][*k].is_none())
            {
                for (i, v) in vals.iter().enumerate() {
                    if (i >> rho) < counts[fam] && field_present(rfield[a], i & ((1 << rho) - 1)) {
                        let x = blocks[a][k].index(i);
                        assert!(
                            taken[x >> 6] >> (x & 63) & 1 == 0,
                            "{} cell {k}: position {x} taken twice",
                            arr.name
                        );
                        taken[x >> 6] |= 1 << (x & 63);
                        w[x] = *v;
                    } else {
                        assert!(
                            bool::from(v.is_zero()),
                            "{}: an absent instance holds zero",
                            arr.name
                        );
                    }
                }
            }
        }
        if std::env::var("ZKS_PLACE").is_ok() {
            // Per row: present cells and the row's bound.
            let mut per_row = vec![0usize; shape.rows()];
            for (a, arr) in arrays
                .iter()
                .enumerate()
                .filter(|(a, _)| !is_io.get(*a).copied().unwrap_or(false))
            {
                for k in (0..arr.cells()).filter(|k| moved[a][*k].is_none()) {
                    for &i in &present_rows[a] {
                        per_row[blocks[a][k].index(i) / shape.cols()] += 1;
                    }
                }
            }
            for (r, n) in per_row.iter().enumerate() {
                eprintln!("row {r:3}: {n:6} present cells, bound {}", bounds[r]);
            }
        }
        // Reserved ordinary IO has one deterministic reconstruction. Concrete
        // relation outputs below replace their declared slots.
        let mut io = vec![F::ZERO; io_len];
        if !io.is_empty() {
            io[0] = F::ONE;
        }
        let _ = seed;
        // Early io arrays (not late io): their values are public inputs at
        // their io indices.
        let late_slots: HashSet<usize> = late_io.iter().map(|x| x.0).collect();
        for &(a, off) in &io_arrays {
            if late_slots.contains(&off) || link_slots.contains(&off) {
                continue;
            }
            for i in 0..arrays[a].r_count {
                if io.len() <= off + i {
                    io.resize(off + i + 1, F::ZERO);
                }
                io[off + i] = arrays[a].values[0][i];
            }
        }
        let cols_n = shape.cols();
        // A fixed row past the constants' (a target layout's padding) holds
        // a 1 in its first column, read by no constraint, so that its
        // commitment is a generator, not the identity (which no verifier
        // takes as an input point).
        for r in shape.rows() - fixed_rows + wallet_rows + natural_fixed..shape.rows() {
            let row = &mut w[r * cols_n..(r + 1) * cols_n];
            assert!(
                row.iter().all(|v| bool::from(v.is_zero())),
                "{label}: a padding fixed row holds no constant"
            );
            row[0] = F::ONE;
        }
        let fixed_values = w[(shape.rows() - fixed_rows + wallet_rows) * cols_n..].to_vec();
        let _ = n_user;
        let circuit = UniformCircuit {
            verifier_only: None,
            label,
            arrays,
            pi,
            counts,
            shape,
            io_len,
            constraints,
            product_masks,
            row_base,
            blocks,
            bounds,
            fixed_rows,
            fixed_values,
            wallet_rows,
            wallet_io: wallet.map(|(_, io)| io),
            phase2,
            lookup_arrays,
            tables,
            ch_rows,
            ch_fill: challenge.fill,
            io_arrays,
            late_io,
            exports,
            lookup_mask,
            lookup_mask_col,
            copy_exports: copies
                .exports
                .into_iter()
                .map(|e| (e.array, e.tuple))
                .collect(),
            copy_imports: copies
                .imports
                .into_iter()
                .map(|im| (im.array, im.tuple, im.from))
                .collect(),
            index_arrays,
            p1_rows,
            pl_rows,
            p2_rows,
            link_tables: link.tables.clone(),
            link_fill: link.fill,
            link_io: link.io.clone(),
            link_io_values: link.io_values,
            link_binding: link.binding.clone(),
            rfield,
            moved,
            split_of,
            gather: OnceLock::new(),
            symbolic: OnceLock::new(),
            readers: OnceLock::new(),
            m_plans: OnceLock::new(),
            plan: OnceLock::new(),
        };
        (circuit, Instance { io }, Witness { w })
    }

    /// The (array, template row, instance) owning constraint row `x`.
    pub fn row_owner(&self, x: usize) -> Option<(usize, usize, usize)> {
        for (a, bases) in self.row_base.iter().enumerate() {
            for (j, b) in bases.iter().enumerate() {
                if x >= *b && x < b + (1usize << self.m(a)) && self.present(a, x - b) {
                    return Some((a, j, x - b));
                }
            }
        }
        None
    }

    /// Whether the circuit runs LogUp (lookups or copy-table entries).
    fn logup(&self) -> bool {
        if let Some((challenges, _)) = self.verifier_only { return challenges != 0; }
        !self.lookup_arrays.is_empty() || !self.exports.is_empty() || !self.ch_rows.is_empty()
    }

    /// The entry array of export site `e` (cells: multiplicity, helper).
    pub fn export_array(&self, e: usize) -> usize {
        self.exports[e].1
    }

    /// The index column array of site array `a` (one cell per site: its
    /// exports, then its imports, in declaration order).
    pub fn index_array(&self, a: usize) -> Option<usize> {
        self.index_arrays
            .iter()
            .find(|(s, _)| *s == a)
            .map(|(_, x)| *x)
    }

    /// Copy-argument size: export entries and reads over present instances.
    pub fn copy_counts(&self) -> (usize, usize) {
        let entries = self
            .copy_exports
            .iter()
            .map(|(a, _)| self.instances(*a))
            .sum();
        let reads = self
            .copy_imports
            .iter()
            .map(|(a, _, from)| {
                (0..from.len())
                    .filter(|i| self.present(*a, *i) && from[*i].is_some())
                    .count()
            })
            .sum();
        (entries, reads)
    }

    /// The prover's gather of `Az, Bz, Cz` (structure only, from the
    /// symbolic rows, built on first use): one job per template row and
    /// matrix, writing that row block.
    fn gather_plan(&self) -> &Vec<GatherJob> {
        self.gather.get_or_init(|| {
            let sw = self.shape.sw();
            let mut jobs = Vec::new();
            for (a, (arr, rows)) in self.arrays.iter().zip(self.symbolic()).enumerate() {
                for (j, row) in rows.iter().enumerate() {
                    for (mat, lin) in row.iter().enumerate() {
                        let mut job = GatherJob {
                            a,
                            j,
                            mat,
                            base: self.row_base[a][j],
                            len: 1 << self.m(a),
                            targets: Vec::new(),
                            rs: (0..1usize << arr.rho)
                                .filter(|r| self.r_present(a, *r))
                                .collect(),
                            entries: Vec::new(),
                            factors: Vec::new(),
                            sums: Vec::new(),
                        };
                        job.entries = vec![Vec::new(); job.rs.len()];
                        job.factors = lin.iter().map(|(_, f)| *f).collect();
                        for (ti, (term, _)) in lin.iter().enumerate() {
                            let (target, map) = match &term.src {
                                Src::One => (
                                    GatherTarget {
                                        base: 1 << sw,
                                        lo: 0,
                                        hi: 0,
                                        rho: 0,
                                        carry: false,
                                    },
                                    None,
                                ),
                                Src::Own(k) => {
                                    let b = &self.blocks[a][*k];
                                    (
                                        GatherTarget {
                                            base: b.base,
                                            lo: b.lo,
                                            hi: b.hi,
                                            rho: arr.rho,
                                            carry: true,
                                        },
                                        None,
                                    )
                                }
                                Src::Link { array, cell, map } => {
                                    let b = &self.blocks[*array][*cell];
                                    (
                                        GatherTarget {
                                            base: b.base,
                                            lo: b.lo,
                                            hi: b.hi,
                                            rho: self.arrays[*array].rho,
                                            carry: !self.global(*array),
                                        },
                                        Some(map),
                                    )
                                }
                                Src::All { .. } => {
                                    job.sums.push(ti);
                                    continue;
                                }
                            };
                            let t = job.targets.len() as u32;
                            for (&r, list) in job.rs.iter().zip(job.entries.iter_mut()) {
                                if bool::from(term.coef[r].is_zero()) {
                                    continue;
                                }
                                let rr = if matches!(term.src, Src::One) {
                                    Some(0)
                                } else {
                                    map.map_or(Some(r), |m| m[r])
                                };
                                if let Some(rr) = rr {
                                    // Unit coefficients add or subtract the value.
                                    let kind = if term.coef[r] == F::ONE {
                                        1
                                    } else if term.coef[r] == -F::ONE {
                                        2
                                    } else {
                                        0
                                    };
                                    list.push((ti as u32, t, target.offset(rr) as u32, kind));
                                }
                            }
                            job.targets.push(target);
                        }
                        jobs.push(job);
                    }
                }
            }
            jobs
        })
    }

    fn ypos(&self, a: usize, src: &Src, i: usize) -> Option<usize> {
        match src {
            Src::One => Some(1 << self.shape.sw()),
            Src::Own(k) => Some(self.blocks[a][*k].index(i)),
            Src::Link { array, cell, map } => {
                let rho = self.arrays[a].rho;
                let (p, r) = (i >> rho, i & ((1 << rho) - 1));
                let p = if self.global(*array) { 0 } else { p };
                map[r]
                    .map(|rr| self.blocks[*array][*cell].index((p << self.arrays[*array].rho) | rr))
            }
            Src::All { .. } => None,
        }
    }

    /// Template terms (LogUp rows written over their challenge factors) and
    /// the compiled matrix evaluation's products of two slots: what the next
    /// proof's verifier synthesizes, one constraint each.
    /// Whether cell `k` of array `a` has its own block (not moved to a chunk array).
    pub fn cell_placed(&self, a: usize, k: usize) -> bool {
        self.moved[a][k].is_none() && !self.is_io(a)
    }

    /// The region of cell `k` of array `a`: 0 first phase, 1 second, 2 constant.
    pub fn cell_region(&self, a: usize, k: usize) -> usize {
        if self.arrays[a].fixed {
            2
        } else if self.phase2[a][k] {
            1
        } else {
            0
        }
    }

    /// Committed entries per width class (the Pixel model's commit input):
    /// first-phase rows count their nonzero values at the row's bound,
    /// second-phase rows their present helper cells at full width; constant
    /// rows are the key's.
    pub fn width_classes(&self, w: &[F]) -> BTreeMap<u16, usize> {
        let (cols, active) = (self.shape.cols(), self.shape.active_cols());
        let mut out: BTreeMap<u16, usize> = BTreeMap::new();
        for r in 0..self.p1_rows {
            let n = w[r * cols..r * cols + active]
                .iter()
                .filter(|x| !bool::from(x.is_zero()))
                .count();
            *out.entry(self.bounds[r] as u16).or_insert(0) += n;
        }
        let helpers: usize = self
            .arrays
            .iter()
            .enumerate()
            .map(|(a, arr)| {
                (0..arr.cells())
                    .filter(|k| self.moved[a][*k].is_none() && self.phase2[a][*k])
                    .count()
                    * self.instances(a)
            })
            .sum();
        *out.entry(255).or_insert(0) += helpers;
        out
    }

    /// The layout a verifier of this circuit's proofs depends on.
    pub fn layout(&self) -> Layout {
        Layout {
            sx: self.shape.sx,
            row_bits: self.shape.row_bits,
            p1: self.p1_rows,
            pl: self.pl_rows,
            fixed: self.fixed_rows,
        }
    }

    /// The link phase's rows.
    pub fn link_phase_rows(&self) -> usize {
        self.pl_rows
    }

    /// Counts the link tables' multiplicities from the witness (after the
    /// link phase's fill), as the build counts the others.
    fn recount_link_tables(&self, w: &mut [F]) {
        let key = |vals: &[F]| -> Vec<u8> {
            vals.iter()
                .flat_map(|v| v.to_repr().as_ref().to_vec())
                .collect()
        };
        for &(va, tid, wa) in self
            .tables
            .iter()
            .filter(|t| self.link_tables.contains(&t.1))
        {
            let mut index: HashMap<Vec<u8>, usize> = HashMap::new();
            for i in (0..1usize << self.m(va)).filter(|i| self.present(va, *i)) {
                let entry: Vec<F> = (0..self.arrays[va].cells())
                    .map(|k| w[self.blocks[va][k].index(i)])
                    .collect();
                index.insert(key(&entry), i);
            }
            let mut mult = vec![0u64; 1usize << self.m(va)];
            for &(a, _, _) in &self.lookup_arrays {
                for i in (0..1usize << self.m(a)).filter(|i| self.present(a, *i)) {
                    for (t, tuple) in &self.arrays[a].lookups {
                        if *t != tid {
                            continue;
                        }
                        let vals: Vec<F> =
                            tuple.iter().map(|lin| self.eval_w(w, a, lin, i)).collect();
                        let slot = *index.get(&key(&vals)).unwrap_or_else(|| {
                            panic!(
                                "{}: a link-phase tuple is in table {tid}",
                                self.arrays[a].name
                            )
                        });
                        mult[slot] += 1;
                    }
                }
            }
            for i in (0..1usize << self.m(va)).filter(|i| self.present(va, *i)) {
                w[self.blocks[wa][0].index(i)] = F::from(mult[i]);
            }
        }
    }

    /// Witness rows: first phase, second phase, constant columns.
    pub fn regions(&self) -> (usize, usize, usize) {
        (self.p1_rows, self.p2_rows, self.fixed_rows)
    }

    pub fn template_size(&self) -> (usize, usize) {
        let terms = self
            .symbolic_rows()
            .iter()
            .flatten()
            .map(|r| r[0].len() + r[1].len() + r[2].len())
            .sum();
        (terms, self.plan().muls)
    }

    /// One array's template terms and the products of two slots of its own
    /// compiled evaluation (factors it shares with other arrays included).
    pub fn array_size(&self, a: usize) -> (usize, usize) {
        let terms = self.symbolic_rows()[a]
            .iter()
            .map(|r| r[0].len() + r[1].len() + r[2].len())
            .sum();
        (terms, self.compile(Some(a)).muls)
    }

    /// The symbolic rows, built once per circuit.
    fn symbolic(&self) -> &SymbolicRows<F> {
        self.symbolic.get_or_init(|| self.symbolic_rows())
    }

    /// A factor's value under the LogUp challenges `(alpha, beta)`; `None`
    /// for the factor 1.
    fn factor_value(ch: &[F], f: Factor) -> Option<F> {
        match f {
            Factor::One => None,
            Factor::AlphaMinus(c) => Some(ch[0] - F::from(c)),
            Factor::NegBeta(e) => Some(-ch[1].pow_vartime([e as u64])),
        }
    }

    /// The template rows with the LogUp rows filled for challenges
    /// `(alpha, beta)`: each symbolic term's coefficients times its factor,
    /// so the terms line up with [`Self::symbolic_rows`] in every proof.
    fn instantiated_rows(&self, ch: &[F]) -> Vec<Vec<[Lin<F>; 3]>> {
        let value = |f: Factor| match f {
            Factor::One => F::ONE,
            Factor::AlphaMinus(c) => ch[0] - F::from(c),
            Factor::NegBeta(e) => -ch[1].pow_vartime([e as u64]),
        };
        self.symbolic_rows()
            .into_iter()
            .map(|rows| {
                rows.into_iter()
                    .map(|row| {
                        row.map(|lin| {
                            lin.into_iter()
                                .map(|(t, f)| {
                                    let v = value(f);
                                    Term {
                                        src: t.src,
                                        coef: t.coef.iter().map(|c| *c * v).collect(),
                                    }
                                })
                                .collect()
                        })
                    })
                    .collect()
            })
            .collect()
    }

    /// The template rows with the LogUp rows written over their challenge
    /// factors: per lookup `h (alpha - f) = 1`, per table entry
    /// `g (alpha - t) = m`, with `f`, `t` compressing (table id + 1, tuple)
    /// by powers of beta. Coefficients never depend on a challenge, so the
    /// compiled evaluation serves every proof.
    fn symbolic_rows(&self) -> Vec<Vec<[Vec<(Term<F>, Factor)>; 3]>> {
        let plain = |lin: &Lin<F>| -> Vec<(Term<F>, Factor)> {
            lin.iter().map(|t| (t.clone(), Factor::One)).collect()
        };
        let ones = |n: usize| Term {
            src: Src::One,
            coef: vec![F::ONE; n],
        };
        let mut rows: Vec<Vec<[Vec<(Term<F>, Factor)>; 3]>> = self
            .arrays
            .iter()
            .map(|arr| {
                arr.rows
                    .iter()
                    .map(|r| [plain(&r[0]), plain(&r[1]), plain(&r[2])])
                    .collect()
            })
            .collect();
        for &(a, first_cell, first_row) in &self.lookup_arrays {
            let arr = &self.arrays[a];
            let n = 1usize << arr.rho;
            for (j, (tid, tuple)) in arr.lookups.iter().enumerate() {
                let mut b = vec![(ones(n), Factor::AlphaMinus(*tid as u64 + 1))];
                for (k, lin) in tuple.iter().enumerate() {
                    b.extend(lin.iter().map(|t| (t.clone(), Factor::NegBeta(k + 1))));
                }
                // A copy read that is off has right-hand side 0 there: per r,
                // or from its mask column.
                let rhs = match (
                    self.lookup_mask.get(&(a, j)),
                    self.lookup_mask_col.get(&(a, j)),
                ) {
                    (_, Some(&(x, k))) => Term {
                        src: Src::Link {
                            array: x,
                            cell: k,
                            map: (0..n).map(Some).collect(),
                        },
                        coef: vec![F::ONE; n],
                    },
                    (Some(on), None) => Term {
                        src: Src::One,
                        coef: on.clone(),
                    },
                    (None, None) => ones(n),
                };
                rows[a][first_row + j] =
                    [plain(&own(first_cell + j, n)), b, vec![(rhs, Factor::One)]];
            }
        }
        for &(va, tid, wa) in &self.tables {
            let n = 1usize << self.arrays[wa].rho;
            let ident: Vec<Option<usize>> = (0..n).map(Some).collect();
            let mut b = vec![(ones(n), Factor::AlphaMinus(tid as u64 + 1))];
            for k in 0..self.arrays[va].cells() {
                b.push((
                    Term {
                        src: Src::Link {
                            array: va,
                            cell: k,
                            map: ident.clone(),
                        },
                        coef: vec![F::ONE; n],
                    },
                    Factor::NegBeta(k + 1),
                ));
            }
            rows[wa][0] = [plain(&own(1, n)), b, plain(&own(0, n))];
        }
        for (_, wa, tid, tuple) in &self.exports {
            let n = 1usize << self.arrays[*wa].rho;
            let mut b = vec![(ones(n), Factor::AlphaMinus(*tid as u64 + 1))];
            for (k, lin) in tuple.iter().enumerate() {
                b.extend(lin.iter().map(|t| (t.clone(), Factor::NegBeta(k + 1))));
            }
            rows[*wa][0] = [plain(&own(1, n)), b, plain(&own(0, n))];
        }
        for (a, j, row) in &self.ch_rows {
            rows[*a][*j] = row.clone();
        }
        rows
    }

    /// A linear combination's value at instance `i` of array `a`, from `w`.
    fn eval_w(&self, w: &[F], a: usize, lin: &Lin<F>, i: usize) -> F {
        let r = i & ((1 << self.arrays[a].rho) - 1);
        lin.iter()
            .map(|t| match &t.src {
                Src::One => t.coef[r],
                _ => self
                    .ypos(a, &t.src, i)
                    .map_or(F::ZERO, |y| t.coef[r] * w[y]),
            })
            .sum()
    }

    /// The prover's LogUp helpers: `h = 1/(alpha - f)`, `g = m/(alpha - t)`.
    fn fill_helpers(&self, ch: &[F], w: &mut [F]) {
        let (alpha, beta) = (ch[0], ch[1]);
        let mut slots: Vec<(usize, F, F)> = Vec::new();
        for &(a, first_cell, _) in &self.lookup_arrays {
            let arr = &self.arrays[a];
            let rmask = (1usize << arr.rho) - 1;
            for i in (0..1usize << self.m(a)).filter(|i| self.present(a, *i)) {
                for (j, (tid, tuple)) in arr.lookups.iter().enumerate() {
                    let mut f = F::from(*tid as u64 + 1);
                    let mut power = beta;
                    for lin in tuple {
                        f += power * self.eval_w(w, a, lin, i);
                        power *= beta;
                    }
                    let rhs = match self.lookup_mask_col.get(&(a, j)) {
                        Some(&(x, k)) => w[self.blocks[x][k].index(i)],
                        None => self
                            .lookup_mask
                            .get(&(a, j))
                            .map_or(F::ONE, |on| on[i & rmask]),
                    };
                    slots.push((self.blocks[a][first_cell + j].index(i), alpha - f, rhs));
                }
            }
        }
        for &(va, tid, wa) in &self.tables {
            for i in (0..1usize << self.m(va)).filter(|i| self.present(va, *i)) {
                let mut t = F::from(tid as u64 + 1);
                let mut power = beta;
                for k in 0..self.arrays[va].cells() {
                    t += power * w[self.blocks[va][k].index(i)];
                    power *= beta;
                }
                slots.push((
                    self.blocks[wa][1].index(i),
                    alpha - t,
                    w[self.blocks[wa][0].index(i)],
                ));
            }
        }
        for (_, wa, tid, tuple) in &self.exports {
            for i in (0..1usize << self.m(*wa)).filter(|i| self.present(*wa, *i)) {
                let mut t = F::from(*tid as u64 + 1);
                let mut power = beta;
                for lin in tuple {
                    t += power * self.eval_w(w, *wa, lin, i);
                    power *= beta;
                }
                slots.push((
                    self.blocks[*wa][1].index(i),
                    alpha - t,
                    w[self.blocks[*wa][0].index(i)],
                ));
            }
        }
        let mut dens: Vec<F> = slots.iter().map(|s| s.1).collect();
        crate::uniform::batch_invert(&mut dens);
        for ((idx, _, m), inv) in slots.iter().zip(dens) {
            w[*idx] = *m * inv;
        }
    }

    /// The flat plan of `M(r_x, .)`: per placed block and target r', the
    /// present constraint rows that read it, with their coefficients (public
    /// structure, built on first use).
    fn m_plans(&self) -> &Vec<MPlan<F>> {
        self.m_plans.get_or_init(|| {
            let (rows, rd) = (self.symbolic(), self.readers());
            let mut plans = Vec::new();
            for (a2, arr2) in self.arrays.iter().enumerate() {
                for k2 in (0..arr2.cells()).filter(|k| self.moved[a2][*k].is_none()) {
                    let (starts, pairs) = &rd.at[a2][k2];
                    let readers: Vec<MReader> = rd.readers[a2][k2]
                        .iter()
                        .map(|r| {
                            let family = self.arrays[r.array].family;
                            MReader {
                                a: r.array,
                                j: r.row,
                                mat: r.mat,
                                t: r.term,
                                rho: self.arrays[r.array].rho,
                                all_p: (family != arr2.family).then(|| self.counts[family]),
                            }
                        })
                        .collect();
                    assert!(
                        readers.len() <= u16::MAX as usize,
                        "{}: readers of one block",
                        arr2.name
                    );
                    let n2 = 1usize << arr2.rho;
                    let mut offsets = Vec::with_capacity(n2 + 1);
                    let (mut entries, mut coefs) = (Vec::new(), Vec::new());
                    offsets.push(0u32);
                    for r2 in 0..n2 {
                        for &(ri, r) in &pairs[starts[r2] as usize..starts[r2 + 1] as usize] {
                            let (reader, r) = (&rd.readers[a2][k2][ri as usize], r as usize);
                            if !self.r_present(reader.array, r) {
                                continue;
                            }
                            let coef = rows[reader.array][reader.row][reader.mat][reader.term]
                                .0
                                .coef[r];
                            if bool::from(coef.is_zero()) {
                                continue;
                            }
                            let row = u32::try_from(self.row_base[reader.array][reader.row] + r)
                                .expect("a constraint row");
                            let (kind, index) = if coef == F::ONE {
                                (1u8, 0u32)
                            } else if coef == -F::ONE {
                                (2, 0)
                            } else {
                                coefs.push(coef);
                                (0, (coefs.len() - 1) as u32)
                            };
                            entries.push((ri as u16, kind, row, index));
                        }
                        offsets.push(u32::try_from(entries.len()).expect("plan entries"));
                    }
                    plans.push(MPlan {
                        a2,
                        k2,
                        offsets,
                        entries,
                        coefs,
                        readers,
                    });
                }
            }
            plans
        })
    }

    /// Readers of every cell block (the prover's gather of `M(r_x, .)`).
    /// The readers, from the symbolic rows (the terms of every proof's rows).
    fn readers(&self) -> &Readers {
        self.readers.get_or_init(|| {
            let rows: Vec<Vec<[Lin<F>; 3]>> = self
                .symbolic_rows()
                .into_iter()
                .map(|rows| {
                    rows.into_iter()
                        .map(|row| row.map(|lin| lin.into_iter().map(|(t, _)| t).collect()))
                        .collect()
                })
                .collect();
            self.readers_for(&rows)
        })
    }

    fn readers_for(&self, rows: &[Vec<[Lin<F>; 3]>]) -> Readers {
        let mut readers: Vec<Vec<Vec<Reader>>> = self
            .arrays
            .iter()
            .map(|arr| (0..arr.cells()).map(|_| Vec::new()).collect())
            .collect();
        let mut one_readers = Vec::new();
        let mut all_readers: Vec<Vec<Vec<(usize, usize, usize, usize)>>> = self
            .arrays
            .iter()
            .map(|arr| vec![Vec::new(); arr.cells()])
            .collect();
        // Store only actual reads, then bucket them into one CSR per target
        // block. A dense inverse map per reader multiplies a large lookup
        // helper's extent by the number of scalar readers (hundreds of GB for
        // the total ECDSA relation), even if every reader maps one instance.
        let mut edges: Vec<Vec<Vec<(u32, u32, u32)>>> = self
            .arrays
            .iter()
            .map(|arr| vec![Vec::new(); arr.cells()])
            .collect();
        for (a, arr) in self.arrays.iter().enumerate() {
            for (j, row) in rows[a].iter().enumerate() {
                for (mat, lin) in row.iter().enumerate() {
                    for (t, term) in lin.iter().enumerate() {
                        match &term.src {
                            Src::One => one_readers.push((a, j, mat, t)),
                            Src::Own(k) => {
                                let ri = u32::try_from(readers[a][*k].len()).expect("reader index");
                                readers[a][*k].push(Reader { array: a, row: j, mat, term: t });
                                edges[a][*k].extend((0..1usize << arr.rho).map(|r| {
                                    let r = u32::try_from(r).expect("reader instance");
                                    (r, ri, r)
                                }));
                            }
                            Src::Link { array, cell, map } => {
                                let ri = u32::try_from(readers[*array][*cell].len())
                                    .expect("reader index");
                                for (r, target) in map.iter().enumerate() {
                                    if let Some(rr) = target {
                                        assert!(*rr < 1usize << self.arrays[*array].rho,
                                            "target instance in block");
                                        edges[*array][*cell].push((
                                            u32::try_from(*rr).expect("target instance"),
                                            ri,
                                            u32::try_from(r).expect("reader instance"),
                                        ));
                                    }
                                }
                                let tf = self.arrays[*array].family;
                                assert!(
                                    tf == arr.family || self.pi[tf] == 0,
                                    "links stay inside a family or read a global array"
                                );
                                readers[*array][*cell].push(Reader {
                                    array: a,
                                    row: j,
                                    mat,
                                    term: t,
                                });
                            }
                            Src::All { array, cell } => {
                                all_readers[*array][*cell].push((a, j, mat, t))
                            }
                        }
                    }
                }
            }
        }
        let at = edges
            .into_iter()
            .enumerate()
            .map(|(a2, cells)| {
                cells
                    .into_iter()
                    .map(|list| {
                        let n2 = 1usize << self.arrays[a2].rho;
                        let mut offsets = vec![0u32; n2 + 1];
                        for &(target, _, _) in &list {
                            offsets[target as usize + 1] = offsets[target as usize + 1]
                                .checked_add(1).expect("reads per target block");
                        }
                        for r2 in 0..n2 {
                            offsets[r2 + 1] = offsets[r2 + 1].checked_add(offsets[r2])
                                .expect("total reads per target block");
                        }
                        let mut next = offsets[..n2].to_vec();
                        let mut pairs = vec![(0u32, 0u32); list.len()];
                        // Readers and their source instances were visited in
                        // ascending order. Stable bucketing retains precisely
                        // that order within each target's CSR segment.
                        for (target, reader, r) in list {
                            let at = &mut next[target as usize];
                            pairs[*at as usize] = (reader, r);
                            *at += 1;
                        }
                        (offsets, pairs)
                    })
                    .collect()
            })
            .collect();
        Readers {
            readers,
            at,
            one_readers,
            all_readers,
        }
    }

    /// A term with its coefficients zeroed where array `a` has no instance.
    fn masked(&self, t: &Term<F>, a: usize) -> Term<F> {
        if matches!(t.src, Src::All { .. }) {
            return t.clone();
        }
        Term {
            src: t.src.clone(),
            coef: t
                .coef
                .iter()
                .enumerate()
                .map(|(r, c)| if self.r_present(a, r) { *c } else { F::ZERO })
                .collect(),
        }
    }

    /// A term as structured pieces: itself when its coefficients are
    /// constant, geometric or split into slot x step fields; otherwise its
    /// most common coefficient on the active r plus the remainder (e.g. a
    /// constant with corrections at step 0).
    fn pieces(&self, term: &Term<F>, a: usize) -> Vec<Term<F>> {
        let arr = &self.arrays[a];
        let rho = arr.rho;
        let (map, rho_t): (Option<&Vec<Option<usize>>>, usize) = match &term.src {
            Src::One => (None, 0),
            Src::Own(_) => (None, rho),
            Src::Link { array, map, .. } => (Some(map), self.arrays[*array].rho),
            Src::All { .. } => return vec![term.clone()],
        };
        let one = matches!(term.src, Src::One);
        let map_at = |r: usize| -> Option<usize> {
            if one {
                Some(0)
            } else {
                map.map_or(Some(r), |mp| mp[r])
            }
        };
        let active: Vec<usize> = (0..1usize << rho)
            .filter(|r| map_at(*r).is_some() && !bool::from(term.coef[*r].is_zero()))
            .collect();
        let constant_coef = active.iter().all(|r| term.coef[*r] == term.coef[active[0]]);
        if active.len() <= 1 || constant_coef {
            return vec![term.clone()];
        }
        if coef_pattern(&term.coef, &active).is_some() {
            return vec![term.clone()];
        }
        if split_term(rho, rho_t, &active, &term.coef, &map_at).is_some() {
            return vec![term.clone()];
        }
        // The most common coefficient, the first seen on a tie (the compiled
        // program is the same on every build).
        let mut freq: Vec<(usize, F)> = Vec::new();
        for &r in &active {
            match freq.iter_mut().find(|(_, c)| *c == term.coef[r]) {
                Some(e) => e.0 += 1,
                None => freq.push((1, term.coef[r])),
            }
        }
        let (count, mode) = freq
            .iter()
            .copied()
            .fold((0, F::ZERO), |best, e| if e.0 > best.0 { e } else { best });
        if count < 2 {
            return vec![term.clone()];
        }
        let mut base = term.clone();
        let mut rest = term.clone();
        for r in 0..term.coef.len() {
            if active.contains(&r) {
                base.coef[r] = mode;
                rest.coef[r] = term.coef[r] - mode;
            } else {
                base.coef[r] = F::ZERO;
            }
        }
        let mut out = vec![base];
        if rest.coef.iter().any(|c| !bool::from(c.is_zero())) {
            out.push(rest);
        }
        out
    }

    /// The compiled matrix evaluation.
    fn plan(&self) -> &Tape<F> {
        self.plan.get_or_init(|| self.compile(None))
    }

    /// Exact program identity for authenticated parameter distribution. Hashes
    /// the complete compiled A/B/C evaluation program, fixed (non-wallet) rows,
    /// phase layout and verifier IO ABI. No witness value or sampled evaluation
    /// substitutes for a program instruction.
    pub fn program_digest(&self) -> [u8; 32] {
        let mut h=blake3::Hasher::new();
        h.update(b"kagemusha-spartan/program/v1\0");
        let word=|h:&mut blake3::Hasher,v:usize|{h.update(&(v as u64).to_le_bytes());};
        word(&mut h,self.shape.label.len());h.update(self.shape.label.as_bytes());
        for v in [self.shape.sx,self.shape.row_bits,self.shape.col_bits,self.shape.k,self.io_len,self.constraints,
            self.p1_rows,self.pl_rows,self.p2_rows,self.fixed_rows,self.wallet_rows,
            self.wallet_io.unwrap_or(usize::MAX)] {word(&mut h,v);}
        let degree=self.challenge_degrees();
        for v in [self.phase2_challenges(),degree.0,degree.1] {word(&mut h,v);}
        let tape=self.plan();word(&mut h,tape.inputs);word(&mut h,tape.ops.len());word(&mut h,tape.pool.len());
        for value in &tape.pool {h.update(&value.to_repr());}
        for op in &tape.ops {
            let(tag,a,b)=match *op {Op::Add(a,b)=>(0,a,b),Op::Sub(a,b)=>(1,a,b),Op::Mul(a,b)=>(2,a,b),Op::Scale(a,b)=>(3,a,b),Op::Const(a)=>(4,a,0)};
            h.update(&[tag]);h.update(&a.to_le_bytes());h.update(&b.to_le_bytes());
        }
        for out in tape.out {h.update(&out.to_le_bytes());}
        // The wallet's omitted row is instance-owned and issuer-certified.
        // fixed_values contains only the trusted global tail.
        word(&mut h,self.fixed_values.len());
        for value in &self.fixed_values {h.update(&value.to_repr());}
        word(&mut h,self.late_io.len());
        for &(io,a,c,i) in &self.late_io {for v in [io,a,c,i]{word(&mut h,v);}}
        word(&mut h,self.link_io.len());
        for &(a,io) in &self.link_io {word(&mut h,a);word(&mut h,io);}
        *h.finalize().as_bytes()
    }

    /// The compiled matrix evaluation's products of two slots, products by
    /// a constant, and instructions.
    pub fn plan_stats(&self) -> (usize, usize, usize) {
        let p = self.plan();
        (p.muls, p.scales, p.ops.len())
    }

    /// One array's part of `A~, B~, C~` (its own compiled program).
    pub fn evals_of(&self, a: usize, rx: &[F], ry: &[F], ch: &[F]) -> (F, F, F) {
        let k = if self.logup() { 2 } else { 0 };
        let inputs: Vec<F> = rx.iter().chain(ry).chain(&ch[..k]).copied().collect();
        self.compile(Some(a)).run(&inputs)
    }

    /// `A~, B~, C~` at `(r_x, r_y)` under the LogUp challenges `ch`: one pass
    /// over the compiled program.
    pub fn evals(&self, rx: &[F], ry: &[F], ch: &[F]) -> (F, F, F) {
        let k = if self.logup() { 2 } else { 0 };
        let inputs: Vec<F> = rx.iter().chain(ry).chain(&ch[..k]).copied().collect();
        self.plan().run(&inputs)
    }

    /// `evals` at many points `(ch, r_x, r_y)`: the compiled program over
    /// groups of lanes, in parallel (native q's evaluations).
    pub fn evals_many(&self, points: &[(&[F], &[F], &[F])]) -> Vec<(F, F, F)> {
        let k = if self.logup() { 2 } else { 0 };
        let inputs: Vec<Vec<F>> = points
            .iter()
            .map(|(ch, rx, ry)| rx.iter().chain(*ry).chain(&ch[..k]).copied().collect())
            .collect();
        self.plan().run_many(&inputs)
    }

    /// The compiled program's batched form: its registers against its slots.
    pub fn plan_registers(&self) -> (usize, usize) {
        let p = self.plan();
        (p.registers(), p.inputs + p.ops.len())
    }

    /// The fixed witness bits of block `(a2, k2)` against `r_y`, times the
    /// witness half `1 - r_y[0]`, memoized by prefix from the top bit.
    fn fixed_y<'t>(
        &self,
        cache: &mut HashMap<(usize, usize), V<'t, F>>,
        memo: &mut HashMap<Vec<(usize, bool)>, V<'t, F>>,
        ry: &[V<'t, F>],
        a2: usize,
        k2: usize,
    ) -> V<'t, F> {
        if let Some(v) = cache.get(&(a2, k2)) {
            return *v;
        }
        let sw = self.shape.sw();
        let block = &self.blocks[a2][k2];
        let inst: Vec<usize> = (0..self.m(a2)).map(|t| block.wbit(t)).collect();
        let pairs: Vec<(usize, bool)> = (0..sw)
            .rev()
            .filter(|b| !inst.contains(b))
            .map(|b| (b, (block.base >> b) & 1 == 1))
            .collect();
        // A block in z's public half (an io array) has selector r_y[0]; the
        // shared prefix memo assumes the witness half, so it uses its own.
        let v = if (block.base >> sw) & 1 == 1 {
            memo_product(&mut HashMap::new(), &pairs, ry[0], &|b| ry[sw - b])
        } else {
            memo_product(memo, &pairs, ry[0].c(F::ONE) - ry[0], &|b| ry[sw - b])
        };
        cache.insert((a2, k2), v);
        v
    }

    /// Compiles `A~, B~, C~` (all arrays, or one) into a straight-line
    /// program over `(r_x, r_y, alpha, beta)`. A matrix-MLE term factors into
    /// its row's fixed bits, a sum over `r`, and its target block's fixed
    /// bits and `p` sum. Each array multiplies the block factors once per
    /// target block, sums over `r` in closed form where the coefficients
    /// have a pattern, and builds r-bit tables only for the rest; equal
    /// subexpressions share one slot.
    fn compile(&self, only: Option<usize>) -> Tape<F> {
        let (sx, sw) = (self.shape.sx, self.shape.sw());
        let lookups = self.logup();
        let cell = RefCell::new(Builder::<F>::new(sx + sw + 1 + if lookups { 2 } else { 0 }));
        let tb = &cell;
        let input = |i: usize| V {
            t: tb,
            s: i as Slot,
        };
        let rx: Vec<V<'_, F>> = (0..sx).map(input).collect();
        let ry: Vec<V<'_, F>> = (0..=sw).map(|i| input(sx + i)).collect();
        let (one, zero) = (V::lift(tb, F::ONE), V::lift(tb, F::ZERO));
        let muls = || tb.borrow().muls;
        let factor_of = |f: Factor| -> V<'_, F> {
            match f {
                Factor::One => one,
                Factor::AlphaMinus(c) => input(sx + sw + 1) - one.c(F::from(c)),
                Factor::NegBeta(e) => {
                    let beta = input(sx + sw + 2);
                    zero - (1..e).fold(beta, |p, _| p * beta)
                }
            }
        };
        // Products per stage: fx, fy, sum sides, sum terms, single r,
        // pattern (closed forms), -, points, split, enumeration, per term,
        // block finals, p sums.
        let mut cat = [0usize; 13];
        let one_eq = ry[1..].iter().fold(ry[0], |p, r| p * (one - *r));
        let mut acc = [zero; 3];
        // Shared by every array: fixed y bits per block, r-bit tables of
        // target positions, fixed-bit products by prefix, single-point y
        // products, p sums and sum-row sides.
        let mut fixed_y: HashMap<(usize, usize), V<'_, F>> = HashMap::new();
        let mut ty: HashMap<Vec<usize>, Vec<V<'_, F>>> = HashMap::new();
        let mut memo_x: HashMap<Vec<(usize, bool)>, V<'_, F>> = HashMap::new();
        let mut memo_y: HashMap<Vec<(usize, bool)>, V<'_, F>> = HashMap::new();
        let mut memo_ty: HashMap<Vec<(usize, bool)>, V<'_, F>> = HashMap::new();
        let mut psums: HashMap<(usize, usize, usize, bool, Vec<usize>), V<'_, F>> = HashMap::new();
        let mut sides: HashMap<(Vec<usize>, usize, (usize, usize, usize)), V<'_, F>> =
            HashMap::new();
        let rows = self.symbolic_rows();
        for (a, arr) in self.arrays.iter().enumerate() {
            if only.is_some_and(|o| o != a) {
                continue;
            }
            let (m, rho) = (self.m(a), arr.rho);
            let (pi, n) = (m - rho, 1usize << rho);
            let count = self.counts[arr.family];
            let xs: Vec<V<'_, F>> = (0..rho).map(|t| rx[sx - 1 - t]).collect();
            // Per target block (and per factor for the constant): the sum of
            // fx * s over the array's terms, multiplied by the block's factors
            // once.
            let mut blk: BTreeMap<(usize, usize, usize), V<'_, F>> = BTreeMap::new();
            let mut ones: BTreeMap<(usize, Factor), V<'_, F>> = BTreeMap::new();
            let mut memo_tx: HashMap<Vec<(usize, bool)>, V<'_, F>> = HashMap::new();
            // Factors depend on the target's r-bit positions, not its cell:
            // the cells of one array share them.
            let mut factors: HashMap<String, V<'_, F>> = HashMap::new();
            let mut closed: HashMap<String, Option<V<'_, F>>> = HashMap::new();
            let mut tx: Option<Vec<V<'_, F>>> = None;
            let mut tparts: HashMap<usize, Vec<V<'_, F>>> = HashMap::new();
            for (j, row) in rows[a].iter().enumerate() {
                let base = self.row_base[a][j];
                let pairs: Vec<(usize, bool)> =
                    (m..sx).rev().map(|b| (b, (base >> b) & 1 == 1)).collect();
                let m0 = muls();
                let fx = memo_product(&mut memo_x, &pairs, one, &|b| rx[sx - 1 - b]);
                cat[0] += muls() - m0;
                let mut row_all = [zero; 3];
                // Per (matrix, block, factor, r sum): the sum of the terms'
                // chunk points times their constants (linear, so free in a
                // circuit), multiplied by the row factor and the r sum once.
                let mut grouped: BTreeMap<
                    (usize, usize, usize, Factor, Slot),
                    (V<'_, F>, V<'_, F>),
                > = BTreeMap::new();
                for (mat, lin) in row.iter().enumerate() {
                    let lin: Vec<(Term<F>, Factor)> = lin
                        .iter()
                        .flat_map(|(t, f)| {
                            self.pieces(&self.masked(t, a), a)
                                .into_iter()
                                .map(move |p| (p, *f))
                        })
                        .collect();
                    for (term, fac) in &lin {
                        let (a2, map): (Option<(usize, usize)>, Option<&Vec<Option<usize>>>) =
                            match &term.src {
                                Src::One => (None, None),
                                Src::Own(k) => (Some((a, *k)), None),
                                Src::Link { array, cell, map } => {
                                    (Some((*array, *cell)), Some(map))
                                }
                                Src::All { array, cell } => {
                                    // Sum over every instance of the block: y-only
                                    // prefix sums over its p and r bits.
                                    let m0 = muls();
                                    let fy =
                                        self.fixed_y(&mut fixed_y, &mut memo_y, &ry, *array, *cell);
                                    cat[1] += muls() - m0;
                                    let block = &self.blocks[*array][*cell];
                                    let arr2 = &self.arrays[*array];
                                    let (rho2, pi2) = (arr2.rho, self.m(*array) - arr2.rho);
                                    let wbits: Vec<usize> =
                                        (0..pi2 + rho2).map(|t| block.wbit(t)).collect();
                                    let m0 = muls();
                                    let (s2, inner2, outer2) = self.rfield[*array];
                                    let side = *sides
                                        .entry((
                                            wbits,
                                            self.counts[arr2.family],
                                            self.rfield[*array],
                                        ))
                                        .or_insert_with(|| {
                                            let side = |bits: Vec<usize>, limit: usize| {
                                                let g: Vec<(V<'_, F>, V<'_, F>)> = bits
                                                    .into_iter()
                                                    .map(|b| (one - ry[sw - b], ry[sw - b]))
                                                    .collect();
                                                prefix_sum(&g, limit, one)
                                            };
                                            side(
                                                (0..pi2).map(|u| block.wbit(rho2 + u)).collect(),
                                                self.counts[arr2.family],
                                            ) * side(
                                                (0..s2).map(|t| block.wbit(t)).collect(),
                                                inner2,
                                            ) * side(
                                                (s2..rho2).map(|t| block.wbit(t)).collect(),
                                                outer2,
                                            )
                                        });
                                    cat[2] += muls() - m0;
                                    let m0 = muls();
                                    row_all[mat] =
                                        row_all[mat] + fy * side * factor_of(*fac) * term.coef[0];
                                    cat[3] += muls() - m0;
                                    continue;
                                }
                            };
                        // The target block's r bits.
                        let wb: Vec<usize> = a2.map_or(Vec::new(), |(a2, k2)| {
                            (0..self.arrays[a2].rho)
                                .map(|t| self.blocks[a2][k2].wbit(t))
                                .collect()
                        });
                        let ys: Vec<V<'_, F>> = wb.iter().map(|b| ry[sw - b]).collect();
                        let active: Vec<usize> = (0..n)
                            .filter(|r| {
                                map.map_or(true, |mp| mp[*r].is_some())
                                    && !bool::from(term.coef[*r].is_zero())
                            })
                            .collect();
                        if active.is_empty() {
                            continue;
                        }
                        // Target bits that every active r shares (a chunk array's
                        // chunk index) are one point product, memoized by prefix
                        // from the top bit; the rest of the term sees the low
                        // bits only, so every chunk shares its closed form.
                        let mut high = one;
                        let mut low_map: Option<Vec<Option<usize>>> = None;
                        let (wb, ys) = match map {
                            Some(mp) => {
                                let t0 = mp[active[0]].unwrap();
                                let diff = active
                                    .iter()
                                    .fold(0usize, |d, r| d | (mp[*r].unwrap() ^ t0));
                                let hy = (usize::BITS - diff.leading_zeros()) as usize;
                                if hy < wb.len() {
                                    let pairs: Vec<(usize, bool)> = (hy..wb.len())
                                        .rev()
                                        .map(|t| (wb[t], (t0 >> t) & 1 == 1))
                                        .collect();
                                    high = memo_product(&mut memo_ty, &pairs, one, &|b| ry[sw - b]);
                                    let mask = (1usize << hy) - 1;
                                    low_map =
                                        Some(mp.iter().map(|m| m.map(|t| t & mask)).collect());
                                    (wb[..hy].to_vec(), ys[..hy].to_vec())
                                } else {
                                    (wb, ys)
                                }
                            }
                            None => (wb, ys),
                        };
                        let map: Option<&Vec<Option<usize>>> = low_map.as_ref().or(map);
                        let map_at =
                            |r: usize| -> Option<usize> { map.map_or(Some(r), |mp| mp[r]) };
                        let tag = match &term.src {
                            Src::One => 0u8,
                            Src::Own(_) => 1,
                            _ => 2,
                        };
                        let m0 = muls();
                        let full = active.len() > 1;
                        let pattern: Option<(F, V<'_, F>)> = match if full {
                            coef_pattern(&term.coef, &active)
                        } else {
                            None
                        } {
                            None => None,
                            Some((c0, q)) => {
                                // The whole active set: terms with the same
                                // ends but different gaps differ.
                                let key = format!(
                                    "pattern|{tag}|{wb:?}|{map:?}|{:?}|{active:?}",
                                    q.to_repr()
                                );
                                let found = *closed.entry(key).or_insert_with(|| {
                                    let target = match (&term.src, map) {
                                        (Src::One, _) => Target::One,
                                        (_, None) => Target::Own(&ys),
                                        (_, Some(mp)) => Target::Link(mp, &ys),
                                    };
                                    let unit: Vec<F> = (0..n)
                                        .map(|r| {
                                            if active.contains(&r) {
                                                q.pow_vartime([r as u64])
                                            } else {
                                                F::ZERO
                                            }
                                        })
                                        .collect();
                                    r_closed(rho, n, &unit, &target, &xs, one)
                                });
                                found.map(|f| (c0, f))
                            }
                        };
                        // Few active r: points, unless a slot x step split has
                        // a closed-form step sum (then its factors are cheap).
                        let structured = active.len() == 1 || pattern.is_some();
                        let sp_opt = if structured {
                            None
                        } else {
                            split_term(rho, wb.len(), &active, &term.coef, &|r| {
                                if tag == 0 { Some(0) } else { map_at(r) }
                            })
                        };
                        let step_closed = sp_opt.as_ref().is_some_and(|sp| {
                            let ind: Vec<F> = sp
                                .ls
                                .iter()
                                .map(|l| if l.is_some() { F::ONE } else { F::ZERO })
                                .collect();
                            let target = if tag == 0 {
                                Target::One
                            } else {
                                Target::Link(&sp.ls, &ys[..sp.lo_t.min(ys.len())])
                            };
                            r_closed(sp.lo, 1 << sp.lo, &ind, &target, &xs[..sp.lo], one).is_some()
                        });
                        let use_points = active.len() <= POINTS && !step_closed;
                        let path;
                        // The r sum as a slot times a constant: terms of a row that
                        // share the slot share its product with fx.
                        let (s, sc): (V<'_, F>, F) = if active.len() == 1 || use_points {
                            // E_x(r) and E_y(r') memoized by bit prefix (shared
                            // by every term).
                            path = if active.len() == 1 { 4 } else { 7 };
                            let mut sum = zero;
                            for &r in &active {
                                let px: Vec<(usize, bool)> =
                                    (0..rho).rev().map(|t| (t, (r >> t) & 1 == 1)).collect();
                                let ex = memo_product(&mut memo_tx, &px, one, &|t| xs[t]);
                                let ey = match map_at(r) {
                                    Some(rr) if a2.is_some() => {
                                        let py: Vec<(usize, bool)> = (0..wb.len())
                                            .rev()
                                            .map(|t| (wb[t], (rr >> t) & 1 == 1))
                                            .collect();
                                        memo_product(&mut memo_ty, &py, one, &|b| ry[sw - b])
                                    }
                                    _ => one,
                                };
                                sum = sum + ex * ey * term.coef[r];
                            }
                            (sum, F::ONE)
                        } else if let Some((c0, factor)) = pattern {
                            path = 5;
                            (factor, c0)
                        } else if let Some(sp) = sp_opt {
                            // Two fields: a slot sum times a step sum.
                            path = 8;
                            let (lo, lo_t) = (sp.lo, sp.lo_t);
                            let (xs_lo, xs_hi) = xs.split_at(lo);
                            let (ys_lo, ys_hi) = ys.split_at(lo_t.min(ys.len()));
                            let (wb_lo, wb_hi) = wb.split_at(lo_t.min(wb.len()));
                            let skey = format!("step|{tag}|{wb_lo:?}|{lo}|{:?}", sp.ls);
                            let step = *factors.entry(skey).or_insert_with(|| {
                                let ind: Vec<F> = sp
                                    .ls
                                    .iter()
                                    .map(|l| if l.is_some() { F::ONE } else { F::ZERO })
                                    .collect();
                                let target = if tag == 0 {
                                    Target::One
                                } else {
                                    Target::Link(&sp.ls, ys_lo)
                                };
                                r_sum(lo, 1 << lo, &ind, &target, xs_lo, one)
                            });
                            let first = sp
                                .hs
                                .iter()
                                .position(|h| h.is_some())
                                .expect("an active slot");
                            let c0 = sp.cs[first];
                            let c0_inv = c0.invert().unwrap();
                            let norm: Vec<F> = sp.cs.iter().map(|c| *c * c0_inv).collect();
                            let nkey: Vec<[u8; 32]> = norm.iter().map(|c| c.to_repr()).collect();
                            let hkey = format!("slot|{tag}|{wb_hi:?}|{lo}|{:?}|{nkey:?}", sp.hs);
                            let slot = match factors.get(&hkey) {
                                Some(f) => *f,
                                None => {
                                    let target = if tag == 0 {
                                        Target::One
                                    } else {
                                        Target::Link(&sp.hs, ys_hi)
                                    };
                                    let f = match r_closed(
                                        rho - lo,
                                        sp.hs.len(),
                                        &norm,
                                        &target,
                                        xs_hi,
                                        one,
                                    ) {
                                        Some(f) => f,
                                        None => {
                                            let txh = tparts
                                                .entry(lo)
                                                .or_insert_with(|| table(xs_hi, one))
                                                .clone();
                                            let tyh = if tag == 0 {
                                                Vec::new()
                                            } else {
                                                ty.entry(wb_hi.to_vec())
                                                    .or_insert_with(|| table(ys_hi, one))
                                                    .clone()
                                            };
                                            (0..sp.hs.len()).filter(|h| sp.hs[*h].is_some()).fold(
                                                zero,
                                                |s, h| {
                                                    let ey = if tag == 0 {
                                                        one
                                                    } else {
                                                        tyh[sp.hs[h].unwrap()]
                                                    };
                                                    s + txh[h] * ey * norm[h]
                                                },
                                            )
                                        }
                                    };
                                    factors.insert(hkey, f);
                                    f
                                }
                            };
                            (slot * step, c0)
                        } else {
                            // No structure: r-bit tables, then one product per active r.
                            path = 9;
                            let txv = tx.get_or_insert_with(|| table(&xs, one)).clone();
                            let tyv: Vec<V<'_, F>> = if a2.is_some() {
                                ty.entry(wb.clone())
                                    .or_insert_with(|| table(&ys, one))
                                    .clone()
                            } else {
                                Vec::new()
                            };
                            let sum = active.iter().fold(zero, |s, &r| {
                                let ey = if a2.is_some() {
                                    tyv[map_at(r).unwrap()]
                                } else {
                                    one
                                };
                                s + txv[r] * ey * term.coef[r]
                            });
                            (sum, F::ONE)
                        };
                        cat[path] += muls() - m0;
                        if (path == 9 || (path == 5 && muls() - m0 > 30))
                            && std::env::var("ZKS_EVALS_TRACE").is_ok()
                        {
                            let kind = match &term.src {
                                Src::One => "one".to_string(),
                                Src::Own(k) => format!("own {k}"),
                                Src::Link { array, cell, map } => format!(
                                    "link {}:{cell} map {:?}",
                                    self.arrays[*array].name,
                                    &map[..map.len().min(40)]
                                ),
                                Src::All { .. } => "all".to_string(),
                            };
                            let coefs: String = term
                                .coef
                                .iter()
                                .take(40)
                                .map(|c| if bool::from(c.is_zero()) { '0' } else { '1' })
                                .collect();
                            eprintln!(
                                "trace path {path} cost {} array {} row {j} mat {mat}: {kind}; active {} of r_count {}; nonzero coef {coefs}",
                                muls() - m0,
                                arr.name,
                                active.len(),
                                arr.r_count
                            );
                        }
                        let m0 = muls();
                        match a2 {
                            None => {
                                let e = ones.entry((mat, *fac)).or_insert(zero);
                                *e = *e + fx * s * high * sc;
                            }
                            Some((a2, k2)) => {
                                let e =
                                    grouped.entry((mat, a2, k2, *fac, s.s)).or_insert((s, zero));
                                e.1 = e.1 + high * sc;
                            }
                        }
                        cat[10] += muls() - m0;
                    }
                }
                let m0 = muls();
                for (&(mat, a2, k2, fac, _), &(s, sum)) in &grouped {
                    let e = blk.entry((mat, a2, k2)).or_insert(zero);
                    *e = *e + fx * factor_of(fac) * s * sum;
                }
                cat[10] += muls() - m0;
                let m0 = muls();
                for mat in 0..3 {
                    acc[mat] = acc[mat] + fx * row_all[mat];
                }
                cat[3] += muls() - m0;
            }
            if !ones.is_empty() {
                let m0 = muls();
                let g: Vec<(V<'_, F>, V<'_, F>)> = (0..pi)
                    .map(|u| {
                        let x = rx[sx - 1 - (rho + u)];
                        (one - x, x)
                    })
                    .collect();
                let f = one_eq * prefix_sum(&g, count, one);
                for (&(mat, fac), sum) in &ones {
                    acc[mat] = acc[mat] + *sum * (f * factor_of(fac));
                }
                cat[11] += muls() - m0;
            }
            for (&(mat, a2, k2), sum) in &blk {
                let m0 = muls();
                let fy = self.fixed_y(&mut fixed_y, &mut memo_y, &ry, a2, k2);
                cat[1] += muls() - m0;
                let block = &self.blocks[a2][k2];
                let rho2 = self.arrays[a2].rho;
                let global =
                    self.pi[self.arrays[a2].family] == 0 && self.arrays[a2].family != arr.family;
                let pbits: Vec<usize> = if global {
                    Vec::new()
                } else {
                    (0..pi).map(|u| block.wbit(rho2 + u)).collect()
                };
                let m0 = muls();
                let ps = *psums
                    .entry((rho, pi, count, global, pbits))
                    .or_insert_with(|| {
                        let g: Vec<(V<'_, F>, V<'_, F>)> = (0..pi)
                            .map(|u| {
                                let x = rx[sx - 1 - (rho + u)];
                                if global {
                                    (one - x, x)
                                } else {
                                    eq_pair(x, ry[sw - block.wbit(rho2 + u)], one)
                                }
                            })
                            .collect();
                        prefix_sum(&g, count, one)
                    });
                cat[12] += muls() - m0;
                let m0 = muls();
                acc[mat] = acc[mat] + *sum * (fy * ps);
                cat[11] += muls() - m0;
            }
        }
        if only.is_none() && std::env::var("ZKS_EVALS_BREAKDOWN").is_ok() {
            eprintln!(
                "evals breakdown (products of two slots before dead-code removal): fx {} fy {} sum-sides {} sum-terms {} single-r {} pattern {} points {} split {} enumerate {} per-term {} block-finals {} p-sums {} = {}",
                cat[0],
                cat[1],
                cat[2],
                cat[3],
                cat[4],
                cat[5],
                cat[7],
                cat[8],
                cat[9],
                cat[10],
                cat[11],
                cat[12],
                muls()
            );
        }
        let out = acc.map(|v| v.s);
        let builder = std::mem::replace(&mut *tb.borrow_mut(), Builder::new(0));
        builder.finish(out)
    }

    /// Cells and template rows over all instances.
    pub fn sizes(&self) -> (usize, usize) {
        let mut cells = 0;
        let mut rows = 0;
        for (a, arr) in self.arrays.iter().enumerate() {
            let n = self.instances(a);
            cells += self.placed_cells(a) * n;
            rows += arr.rows.len() * n;
        }
        (cells, rows)
    }

    /// Every matrix entry `(matrix, x, y, value)` (tests and the sparse
    /// comparison).
    pub fn entries(&self, ch: &[F]) -> Vec<(usize, usize, usize, F)> {
        self.entries_of(ch, None)
    }

    /// The entries of every array, or of one.
    pub fn entries_of(&self, ch: &[F], only: Option<usize>) -> Vec<(usize, usize, usize, F)> {
        let rows = self.instantiated_rows(ch);
        let mut out = Vec::new();
        for (a, arr) in self.arrays.iter().enumerate() {
            if only.is_some_and(|o| o != a) {
                continue;
            }
            let rmask = (1 << arr.rho) - 1;
            for (j, row) in rows[a].iter().enumerate() {
                for (mat, lin) in row.iter().enumerate() {
                    for i in (0..1 << self.m(a)).filter(|i| self.present(a, *i)) {
                        for term in lin {
                            if let Src::All { array, cell } = &term.src {
                                for i2 in
                                    (0..1 << self.m(*array)).filter(|i2| self.present(*array, *i2))
                                {
                                    out.push((
                                        mat,
                                        self.row_base[a][j] + i,
                                        self.blocks[*array][*cell].index(i2),
                                        term.coef[0],
                                    ));
                                }
                            } else if let Some(y) = self.ypos(a, &term.src, i) {
                                out.push((mat, self.row_base[a][j] + i, y, term.coef[i & rmask]));
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// The same circuit as a sparse assembly (stage A's representation).
    pub fn to_assembly(&self) -> Assembly<F> {
        let mut asm = Assembly::<F>::new();
        // The sparse form runs its own LogUp: no helper cells, helper arrays,
        // sum row or placeholder rows. Copies wire directly (equality rows
        // below), so it has no index columns or entry arrays either.
        let helper_arrays: Vec<usize> = self
            .tables
            .iter()
            .map(|t| t.2)
            .chain(self.exports.iter().map(|e| e.1))
            .chain(self.index_arrays.iter().map(|x| x.1))
            .chain(self.arrays.iter().position(|a| a.name == "lookup sum"))
            .collect();
        let first_helper = |a: usize| {
            self.lookup_arrays
                .iter()
                .find(|l| l.0 == a)
                .map(|l| (l.1, l.2))
        };
        let mut var: Vec<Vec<Vec<u32>>> = Vec::new();
        for (a, arr) in self.arrays.iter().enumerate() {
            let keep = |k: usize| {
                !helper_arrays.contains(&a)
                    && first_helper(a).is_none_or(|(fc, _)| k < fc)
                    && self.moved[a][k].is_none()
            };
            var.push(
                arr.values
                    .iter()
                    .enumerate()
                    .map(|(k, vals)| {
                        (0..1 << self.m(a))
                            .map(|i| {
                                if self.present(a, i) && keep(k) {
                                    asm.var(vals[i], arr.widths[k])
                                } else {
                                    u32::MAX
                                }
                            })
                            .collect()
                    })
                    .collect(),
            );
        }
        for (a, arr) in self
            .arrays
            .iter()
            .enumerate()
            .filter(|(a, arr)| arr.fixed && !helper_arrays.contains(a))
        {
            for (k, vals) in arr.values.iter().enumerate() {
                for i in (0..1 << self.m(a)).filter(|i| self.present(a, *i)) {
                    asm.row(
                        vec![(var[a][k][i], F::ONE), (0, -vals[i])],
                        vec![(0, F::ONE)],
                        Vec::new(),
                    );
                }
            }
        }
        // A sum term of an ordinary row reads every present instance (the
        // LogUp sum row is not exported: the sparse form runs its own LogUp).
        let lc = |a: usize, i: usize, lin: &Lin<F>| -> SLc<F> {
            let arr = &self.arrays[a];
            let (p, r) = (i >> arr.rho, i & ((1 << arr.rho) - 1));
            lin.iter()
                .flat_map(|t| -> Vec<(u32, F)> {
                    match &t.src {
                        Src::One => vec![(0, t.coef[r])],
                        Src::Own(k) => vec![(var[a][*k][i], t.coef[r])],
                        Src::Link { array, cell, map } => {
                            let pp = if self.global(*array) { 0 } else { p };
                            map[r]
                                .map(|rr| {
                                    (
                                        var[*array][*cell][(pp << self.arrays[*array].rho) | rr],
                                        t.coef[r],
                                    )
                                })
                                .into_iter()
                                .collect()
                        }
                        Src::All { array, cell } => (0..1usize << self.m(*array))
                            .filter(|i2| self.present(*array, *i2))
                            .map(|i2| (var[*array][*cell][i2], t.coef[0]))
                            .collect(),
                    }
                })
                .collect()
        };
        for (a, arr) in self.arrays.iter().enumerate() {
            if helper_arrays.contains(&a) {
                continue;
            }
            let rows = first_helper(a).map_or(arr.rows.len(), |(_, fr)| fr);
            for i in (0..1 << self.m(a)).filter(|i| self.present(a, *i)) {
                for row in &arr.rows[..rows] {
                    asm.row(lc(a, i, &row[0]), lc(a, i, &row[1]), lc(a, i, &row[2]));
                }
            }
        }
        for &(va, tid, _) in &self.tables {
            let entries: Vec<Vec<F>> = (0..1usize << self.m(va))
                .filter(|i| self.present(va, *i))
                .map(|i| self.arrays[va].values.iter().map(|col| col[i]).collect())
                .collect();
            let t = asm.table(&format!("table {tid}"), || entries);
            for &(a, _, _) in &self.lookup_arrays {
                for i in (0..1 << self.m(a)).filter(|i| self.present(a, *i)) {
                    for (lt, tuple) in &self.arrays[a].lookups {
                        if *lt == tid {
                            asm.lookup(t, tuple.iter().map(|lin| lc(a, i, lin)).collect());
                        }
                    }
                }
            }
        }
        // Each copy read equals its entry, value by value.
        for (a, tuple, from) in &self.copy_imports {
            for (i, f) in from.iter().enumerate() {
                let Some((e, j)) = f else { continue };
                if !self.present(*a, i) {
                    continue;
                }
                let (ea, et) = &self.copy_exports[*e];
                for (li, le) in tuple.iter().zip(et) {
                    let mut d = lc(*a, i, li);
                    d.extend(lc(*ea, *j, le).into_iter().map(|(v, c)| (v, -c)));
                    asm.row(d, vec![(0, F::ONE)], Vec::new());
                }
            }
        }
        asm
    }
}

/// `sum_{p < count} prod_u g_u(p_u)` for `g_u = (g_u(0), g_u(1))`, bit `u`
/// of `p` (least significant first): the p range of a family that does not
/// fill its power of two.
fn prefix_sum<'t, F: Scalar>(g: &[(V<'t, F>, V<'t, F>)], count: usize, one: V<'t, F>) -> V<'t, F> {
    if count >= 1 << g.len() {
        return g.iter().fold(one, |p, (a, b)| p * (*a + *b));
    }
    let mut low = vec![one; g.len() + 1];
    for (u, (a, b)) in g.iter().enumerate() {
        low[u + 1] = low[u] * (*a + *b);
    }
    let (mut sum, mut prefix) = (one.c(F::ZERO), one);
    for u in (0..g.len()).rev() {
        if (count >> u) & 1 == 1 {
            sum = sum + prefix * g[u].0 * low[u];
            prefix = prefix * g[u].1;
        } else {
            prefix = prefix * g[u].0;
        }
    }
    sum
}

/// `((1 - x)(1 - y), x y)`: one product.
fn eq_pair<'t, F: Scalar>(x: V<'t, F>, y: V<'t, F>, one: V<'t, F>) -> (V<'t, F>, V<'t, F>) {
    let xy = x * y;
    (one - x - y + xy, xy)
}

/// `prod_t eq(bits[t], bit t of r)`.
fn e_at<'t, F: Scalar>(bits: &[V<'t, F>], r: usize, one: V<'t, F>) -> V<'t, F> {
    bits.iter().enumerate().fold(one, |p, (t, x)| {
        let hi = p * *x;
        if (r >> t) & 1 == 1 { hi } else { p - hi }
    })
}

/// `E(r)` for every `r` (bit `t` of `r` against `bits[t]`), one product per
/// entry.
fn table<'t, F: Scalar>(bits: &[V<'t, F>], one: V<'t, F>) -> Vec<V<'t, F>> {
    let mut tab = vec![one];
    for (t, b) in bits.iter().enumerate() {
        let mut next = vec![one; tab.len() * 2];
        for (r, v) in tab.iter().enumerate() {
            let high = *v * *b;
            next[r] = *v - high;
            next[r | (1 << t)] = high;
        }
        tab = next;
    }
    tab
}

/// Terms with at most this many active r are summed point by point.
const POINTS: usize = 8;

/// `root * prod eqbit(value(pos), bit)` over `(pos, bit)` pairs from the most
/// significant position down, memoized by prefix: rows and blocks that share
/// high bits share their products.
fn memo_product<'t, F: Scalar>(
    memo: &mut HashMap<Vec<(usize, bool)>, V<'t, F>>,
    pairs: &[(usize, bool)],
    root: V<'t, F>,
    value: &dyn Fn(usize) -> V<'t, F>,
) -> V<'t, F> {
    let mut cur = root;
    let mut key: Vec<(usize, bool)> = Vec::with_capacity(pairs.len());
    for &(pos, bit) in pairs {
        key.push((pos, bit));
        cur = match memo.get(&key) {
            Some(v) => *v,
            None => {
                let x = value(pos);
                let hi = cur * x;
                let v = if bit { hi } else { cur - hi };
                memo.insert(key.clone(), v);
                v
            }
        };
    }
    cur
}

/// The r-side of a term: the constant (no y bits), the own instance, or a link.
enum Target<'a, T> {
    One,
    Own(&'a [T]),
    Link(&'a [Option<usize>], &'a [T]),
}

/// `(c0, q)` with `coef[r] = c0 * q^r` on every active `r`, if the
/// coefficients are constant (`q = 1`) or geometric.
fn coef_pattern<F: Field>(coef: &[F], active: &[usize]) -> Option<(F, F)> {
    let first = *active.first()?;
    if active.iter().all(|r| coef[*r] == coef[first]) {
        return Some((coef[first], F::ONE));
    }
    let second = *active.get(1)?;
    if second != first + 1 || bool::from(coef[first].is_zero()) {
        return None;
    }
    let q = coef[second] * coef[first].invert().unwrap();
    let mut expect = coef[first];
    let mut at = first;
    for &r in active {
        while at < r {
            expect *= q;
            at += 1;
        }
        if coef[r] != expect {
            return None;
        }
    }
    let q_inv = q.invert().unwrap();
    let c0 = (0..first).fold(coef[first], |acc, _| acc * q_inv);
    Some((c0, q))
}

/// `sum_r coef(r) E_x(r) E_y(map(r))` over the reading array's `r`, with
/// `xs`/`ys` the r bits (least significant first) of the row and the target
/// block: the closed forms of [`r_closed`], or one product chain per active
/// `r`.
fn r_sum<'t, F: Scalar>(
    rho: usize,
    r_count: usize,
    coef: &[F],
    target: &Target<'_, V<'t, F>>,
    xs: &[V<'t, F>],
    one: V<'t, F>,
) -> V<'t, F> {
    if let Some(v) = r_closed(rho, r_count, coef, target, xs, one) {
        return v;
    }
    let map_at = |r: usize| match target {
        Target::One | Target::Own(_) => Some(r),
        Target::Link(map, _) => map[r],
    };
    (0..r_count)
        .filter(|r| map_at(*r).is_some() && !bool::from(coef[*r].is_zero()))
        .fold(one.c(F::ZERO), |s, r| {
            let y = match target {
                Target::One => one,
                Target::Own(ys) => e_at(ys, r, one),
                Target::Link(map, ys) => e_at(ys, map[r].unwrap(), one),
            };
            s + e_at(xs, r, one) * y * coef[r]
        })
}

/// Closed forms of `sum_r coef(r) E_x(r) E_y(map(r))` over an interval of r
/// (any `r_count`) with constant or geometric coefficients, in O(rho) or
/// O(rho^2): the same instance, a broadcast of one target instance, a shift to
/// `r - 1` or `r + 1`, or one point. Interval sums are differences of prefix
/// sums over r's bits. `None` where only enumeration works.
fn r_closed<'t, F: Scalar>(
    rho: usize,
    r_count: usize,
    coef: &[F],
    target: &Target<'_, V<'t, F>>,
    xs: &[V<'t, F>],
    one: V<'t, F>,
) -> Option<V<'t, F>> {
    let n = 1usize << rho;
    let zero = one.c(F::ZERO);
    let map_at = |r: usize| -> Option<usize> {
        match target {
            Target::One | Target::Own(_) => Some(r),
            Target::Link(map, _) => map[r],
        }
    };
    let mapped: Vec<usize> = (0..r_count).filter(|r| map_at(*r).is_some()).collect();
    let active: Vec<usize> = mapped
        .iter()
        .copied()
        .filter(|r| !bool::from(coef[*r].is_zero()))
        .collect();
    if active.is_empty() {
        return Some(zero);
    }
    // A single active r: one point.
    if active.len() == 1 {
        let r = active[0];
        let y = match target {
            Target::One => one,
            Target::Own(ys) => e_at(ys, r, one),
            Target::Link(map, ys) => e_at(ys, map[r].unwrap(), one),
        };
        return Some(e_at(xs, r, one) * y * coef[r]);
    }
    // Closed forms sum the active r, which must form one interval [lo, hi)
    // (zero coefficients and unmapped r outside it contribute nothing).
    let (lo, hi) = (active[0], active[active.len() - 1] + 1);
    if hi - lo != active.len() {
        return None;
    }
    let (c0, q) = coef_pattern(coef, &active)?;
    let mut qpow = Vec::with_capacity(rho);
    let mut acc = q;
    for _ in 0..rho {
        qpow.push(acc);
        acc = acc.square();
    }
    // Bit t's weights for r_t = 0, 1: x alone, or x and y on equal bits
    // ((1 - x)(1 - y) = 1 - x - y + xy), with q^(2^t) where r_t = 1.
    let x_pairs = || -> Vec<(V<'t, F>, V<'t, F>)> {
        (0..rho).map(|t| (one - xs[t], xs[t] * qpow[t])).collect()
    };
    let same = |t: usize, ys: &[V<'t, F>]| -> (V<'t, F>, V<'t, F>) {
        let xy = xs[t] * ys[t];
        (one - xs[t] - ys[t] + xy, xy * qpow[t])
    };
    let interval = |pairs: &[(V<'t, F>, V<'t, F>)]| -> V<'t, F> {
        let below = if lo == 0 {
            zero
        } else {
            prefix_sum(pairs, lo, one)
        };
        prefix_sum(pairs, hi, one) - below
    };
    // sum_{r < count} q^r E_x(r) E_y(r -+ 1), by the lowest set bit k of r
    // (down: r = [H] 1 0^k, y = [H] 0 1^k) or the lowest zero bit (up:
    // r = [H] 0 1^k, y = [H] 1 0^k). H ranges over the bits above k with
    // r < count: H below count >> (k + 1), or equal to it when r's low part is
    // below the remainder.
    let shifted = |ys: &[V<'t, F>], down: bool, count: usize| -> V<'t, F> {
        let mut total = zero;
        let mut low = one;
        for k in 0..rho {
            let (at_k, below) = if down {
                (xs[k] * (one - ys[k]) * qpow[k], (one - xs[k]) * ys[k])
            } else {
                ((one - xs[k]) * ys[k], xs[k] * (one - ys[k]) * qpow[k])
            };
            let low_val = if down { 1usize << k } else { (1usize << k) - 1 };
            let (c_hi, rem) = (count >> (k + 1), count & ((1usize << (k + 1)) - 1));
            let bits_hi = rho - 1 - k;
            let pairs: Vec<(V<'t, F>, V<'t, F>)> = (k + 1..rho).map(|t| same(t, ys)).collect();
            let mut high = prefix_sum(&pairs, c_hi, one);
            if low_val < rem && c_hi < (1usize << bits_hi) {
                high = high
                    + pairs.iter().enumerate().fold(one, |p, (u, (g0, g1))| {
                        p * if (c_hi >> u) & 1 == 1 { *g1 } else { *g0 }
                    });
            }
            total = total + high * at_k * low;
            low = low * below;
        }
        total
    };
    match target {
        Target::One => Some(interval(&x_pairs()) * c0),
        Target::Own(ys) => Some(interval(&(0..rho).map(|t| same(t, ys)).collect::<Vec<_>>()) * c0),
        Target::Link(map, ys) => {
            let all_same = active.iter().all(|r| map[*r] == map[lo]);
            let identity = ys.len() == rho && active.iter().all(|r| map[*r] == Some(*r));
            let down = ys.len() == rho && lo >= 1 && active.iter().all(|r| map[*r] == Some(*r - 1));
            let up = ys.len() == rho && hi < n && active.iter().all(|r| map[*r] == Some(*r + 1));
            if all_same {
                Some(e_at(ys, map[lo].unwrap(), one) * interval(&x_pairs()) * c0)
            } else if identity {
                Some(interval(&(0..rho).map(|t| same(t, ys)).collect::<Vec<_>>()) * c0)
            } else if down || up {
                let below = if lo == 0 { zero } else { shifted(ys, down, lo) };
                Some((shifted(ys, down, hi) - below) * c0)
            } else {
                None
            }
        }
    }
}

/// A two-field factorization of a term over its active `r`: `r = h 2^lo + l`
/// on a product set `H x L`, target `r' = hs(h) 2^lo_t + ls(l)`, coefficient
/// `cs(h)`. The sum is then a slot sum times a step sum.
struct Split<F> {
    lo: usize,
    lo_t: usize,
    hs: Vec<Option<usize>>,
    ls: Vec<Option<usize>>,
    cs: Vec<F>,
}

/// The cheapest two-field factorization of a term, if one exists.
fn split_term<F: Field>(
    rho: usize,
    rho_t: usize,
    active: &[usize],
    coef: &[F],
    map_at: &dyn Fn(usize) -> Option<usize>,
) -> Option<Split<F>> {
    let mut best: Option<(usize, Split<F>)> = None;
    for lo in 1..rho {
        let (nh, nl) = (1usize << (rho - lo), 1usize << lo);
        let lmask = nl - 1;
        let mut hset = vec![false; nh];
        let mut lset = vec![false; nl];
        for &r in active {
            hset[r >> lo] = true;
            lset[r & lmask] = true;
        }
        let hn = hset.iter().filter(|b| **b).count();
        let ln = lset.iter().filter(|b| **b).count();
        if hn * ln != active.len() {
            continue;
        }
        'targets: for lo_t in 0..=rho_t {
            let tmask = (1usize << lo_t) - 1;
            let mut hs = vec![None; nh];
            let mut ls = vec![None; nl];
            let mut cs = vec![F::ZERO; nh];
            for &r in active {
                let (h, l) = (r >> lo, r & lmask);
                let t = map_at(r).expect("active r is mapped");
                let (th, tl) = (t >> lo_t, t & tmask);
                match hs[h] {
                    None => {
                        hs[h] = Some(th);
                        cs[h] = coef[r];
                    }
                    Some(x) => {
                        if x != th || cs[h] != coef[r] {
                            continue 'targets;
                        }
                    }
                }
                match ls[l] {
                    None => ls[l] = Some(tl),
                    Some(x) => {
                        if x != tl {
                            continue 'targets;
                        }
                    }
                }
            }
            let cost = nh + nl + (1usize << lo_t) + (1usize << (rho_t - lo_t));
            if best.as_ref().is_none_or(|(c, _)| cost < *c) {
                best = Some((
                    cost,
                    Split {
                        lo,
                        lo_t,
                        hs,
                        ls,
                        cs,
                    },
                ));
            }
        }
    }
    best.map(|(_, s)| s)
}

/// One reader of a cell block in the `M(r_x, .)` plan: its template term
/// (array, row, matrix, term), its array's rho, and its family's instance
/// count when it reads the block from another family (every p reads it).
struct MReader {
    a: usize,
    j: usize,
    mat: usize,
    t: usize,
    rho: usize,
    all_p: Option<usize>,
}

/// One placed cell block's part of the `M(r_x, .)` plan.
struct MPlan<F> {
    a2: usize,
    k2: usize,
    /// Entry range of each target r' (`2^rho + 1` offsets).
    offsets: Vec<u32>,
    /// The present rows reading each r', a reader's rows together: (reader,
    /// coefficient kind: 1 for +1, 2 for -1, 0 for `coefs[index]`, the
    /// constraint row of `(p = 0, r)`, index).
    entries: Vec<(u16, u8, u32, u32)>,
    coefs: Vec<F>,
    readers: Vec<MReader>,
}

/// Readers of the cell blocks: template terms, constant terms, sum terms.
struct Readers {
    readers: Vec<Vec<Vec<Reader>>>,
    /// Per block, the (reader, reader r) pairs reading each r', as offsets
    /// per r' into one list (a chunk block's readers each cover one chunk).
    at: Vec<Vec<(Vec<u32>, Vec<(u32, u32)>)>>,
    one_readers: Vec<(usize, usize, usize, usize)>,
    all_readers: Vec<Vec<Vec<(usize, usize, usize, usize)>>>,
}

/// Where a gathered term reads: instance bits `x` of a cell block sit at
/// `base + offset(x)`; the read instance keeps the reader's `p` within the
/// family (`carry`) and uses `p = 0` for a global array or the constant.
#[derive(Clone, Copy)]
struct GatherTarget {
    base: usize,
    lo: usize,
    hi: usize,
    rho: usize,
    carry: bool,
}

impl GatherTarget {
    /// Block offset of instance bits `x` (additive over disjoint bits).
    fn offset(&self, x: usize) -> usize {
        (x & ((1 << self.lo) - 1)) + ((x >> self.lo) << self.hi)
    }
    /// Witness position of the read instance `(p, 0)`.
    fn p_base(&self, p: usize) -> usize {
        self.base
            + if self.carry {
                self.offset(p << self.rho)
            } else {
                0
            }
    }
}

/// Rows of one gather task: larger row blocks are cut into runs of `p`.
const GATHER_SPAN: usize = 1 << 11;

/// One template row and matrix of the prover's gather: per present instance
/// `(p, r)`, `sums + sum_t coef_t(r) z[p_base_t(p) + offset_t(r')]`.
struct GatherJob {
    a: usize,
    j: usize,
    mat: usize,
    /// The row block: first constraint row and length.
    base: usize,
    len: usize,
    targets: Vec<GatherTarget>,
    /// The present r, and per present r: (term of the row, its target, the
    /// target offset of r').
    rs: Vec<usize>,
    /// (term, target, offset of r', coefficient kind: 1 for +1, 2 for -1, 0
    /// for any other value).
    entries: Vec<Vec<(u32, u32, u32, u8)>>,
    /// Each term's LogUp challenge factor.
    factors: Vec<Factor>,
    /// Sum terms (the same value at every instance).
    sums: Vec<usize>,
}

impl GatherJob {
    /// Instances `p` of `ps`, written to `out` (the row block from `ps.start`).
    /// `lin` holds the symbolic terms and `fv` each term's factor value
    /// (`None` for 1).
    #[allow(clippy::too_many_arguments)]
    fn run<F: Scalar>(
        &self,
        ps: std::ops::Range<usize>,
        rho: usize,
        lin: &[(Term<F>, Factor)],
        fv: &[Option<F>],
        z: &[F],
        sums: F,
        out: OutPtr<F>,
    ) {
        let mut pb = vec![0usize; self.targets.len()];
        for p in ps {
            for (b, t) in pb.iter_mut().zip(&self.targets) {
                *b = t.p_base(p);
            }
            let row0 = self.base + (p << rho);
            for (&r, list) in self.rs.iter().zip(&self.entries) {
                let mut acc = sums;
                for &(ti, t, off, kind) in list {
                    let v = z[pb[t as usize] + off as usize];
                    match (fv[ti as usize], kind) {
                        (None, 1) => acc += v,
                        (None, 2) => acc -= v,
                        (None, _) => acc += lin[ti as usize].0.coef[r] * v,
                        (Some(f), 1) => acc += f * v,
                        (Some(f), 2) => acc -= f * v,
                        (Some(f), _) => acc += f * (lin[ti as usize].0.coef[r] * v),
                    }
                }
                // SAFETY: present rows of different jobs never coincide (the
                // layout places them apart), so writes do not overlap.
                unsafe { out.0.add(row0 + r).write(acc) };
            }
        }
    }
}

/// The gather's output column, written by jobs at disjoint rows.
#[derive(Clone, Copy)]
struct OutPtr<F>(*mut F);
unsafe impl<F: Send> Send for OutPtr<F> {}
unsafe impl<F: Sync> Sync for OutPtr<F> {}

/// Montgomery batch inversion in parallel chunks.
pub(crate) fn batch_invert<F: Field>(v: &mut [F]) {
    v.par_chunks_mut(4096).for_each(|chunk| {
        let mut prefix = Vec::with_capacity(chunk.len());
        let mut acc = F::ONE;
        for x in chunk.iter() {
            prefix.push(acc);
            acc *= x;
        }
        let mut inv = acc.invert().expect("no zero denominators");
        for (x, p) in chunk.iter_mut().zip(prefix).rev() {
            let next = inv * *x;
            *x = inv * p;
            inv = next;
        }
    });
}

// ---------------------------------------------------------------------------
// The compiled matrix evaluation: a straight-line program over the challenge
// point, built once per circuit. Its products of two slots are what a
// verifier circuit synthesizes (one constraint each); products by a
// constant, sums and differences are free there.

/// The challenge factor of a LogUp row term.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Factor {
    One,
    /// `alpha - c` (a table id + 1).
    AlphaMinus(u64),
    /// `-beta^e`.
    NegBeta(usize),
}

/// A program slot: the inputs, then one per instruction.
type Slot = u32;

#[derive(Clone, Copy, Debug)]
enum Op {
    Add(Slot, Slot),
    Sub(Slot, Slot),
    Mul(Slot, Slot),
    /// Times a constant of the pool.
    Scale(Slot, u32),
    /// A constant of the pool.
    Const(u32),
}

/// A straight-line program: `inputs` slots, then one slot per instruction.
pub struct Tape<F> {
    inputs: usize,
    ops: Vec<Op>,
    pool: Vec<F>,
    out: [Slot; 3],
    /// Products of two slots.
    pub muls: usize,
    /// Products by a constant.
    pub scales: usize,
    /// The same program over reused registers, for many points at once.
    regs: Regs,
}

/// A tape in register form: a slot's register is free again once its last
/// reader has run, so a batch of points keeps `count` values per point
/// rather than one per slot.
struct Regs {
    count: usize,
    /// The register of each input slot.
    inputs: Vec<Slot>,
    /// Each instruction over registers, with the register it writes.
    ops: Vec<(Op, Slot)>,
    out: [Slot; 3],
}

/// An instruction's distinct operand slots.
fn operands(op: Op) -> ([Slot; 2], usize) {
    match op {
        Op::Add(a, b) | Op::Sub(a, b) | Op::Mul(a, b) => ([a, b], if a == b { 1 } else { 2 }),
        Op::Scale(a, _) => ([a, a], 1),
        Op::Const(_) => ([0, 0], 0),
    }
}

impl Regs {
    fn of(inputs: usize, ops: &[Op], out: [Slot; 3]) -> Regs {
        let n = inputs + ops.len();
        // The last instruction reading each slot; the outputs live to the end.
        let mut last: Vec<Option<usize>> = vec![None; n];
        for (i, op) in ops.iter().enumerate() {
            let (src, k) = operands(*op);
            for a in &src[..k] {
                last[*a as usize] = Some(inputs + i);
            }
        }
        for o in out {
            last[o as usize] = Some(usize::MAX);
        }
        let alloc = |free: &mut Vec<Slot>, count: &mut Slot| -> Slot {
            free.pop().unwrap_or_else(|| {
                *count += 1;
                *count - 1
            })
        };
        let (mut reg, mut free, mut count) = (vec![0 as Slot; n], Vec::new(), 0 as Slot);
        for r in reg.iter_mut().take(inputs) {
            *r = alloc(&mut free, &mut count);
        }
        for j in 0..inputs {
            if last[j].is_none() {
                free.push(reg[j]);
            }
        }
        let mut rops = Vec::with_capacity(ops.len());
        for (i, op) in ops.iter().enumerate() {
            let s = inputs + i;
            let r = |x: Slot| reg[x as usize];
            let rop = match *op {
                Op::Add(a, b) => Op::Add(r(a), r(b)),
                Op::Sub(a, b) => Op::Sub(r(a), r(b)),
                Op::Mul(a, b) => Op::Mul(r(a), r(b)),
                Op::Scale(a, k) => Op::Scale(r(a), k),
                Op::Const(k) => Op::Const(k),
            };
            // Operands read for the last time give their registers back
            // first, so the result may take one (each lane reads its
            // operands before it writes).
            let (src, k) = operands(*op);
            for a in &src[..k] {
                if last[*a as usize] == Some(s) {
                    free.push(reg[*a as usize]);
                }
            }
            let d = alloc(&mut free, &mut count);
            reg[s] = d;
            if last[s].is_none() {
                free.push(d);
            }
            rops.push((rop, d));
        }
        Regs {
            count: count as usize,
            inputs: reg[..inputs].to_vec(),
            ops: rops,
            out: out.map(|o| reg[o as usize]),
        }
    }
}

/// A lane width set by `set_q_lanes` (0: none).
static Q_LANES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Sets the lane width of `Tape::run_many` for timing (4, 8 or 16; 0 goes
/// back to `ZKS_Q_LANES` or the default).
pub fn set_q_lanes(lanes: usize) {
    Q_LANES.store(lanes, std::sync::atomic::Ordering::Relaxed);
}

/// Points per lane group of `Tape::run_many` (`set_q_lanes`, else
/// `ZKS_Q_LANES`: 4, 8 or 16; default 4, selected from earlier interleaved
/// measurements). The benchmark compares widths because timings depend on load.
fn q_lanes() -> usize {
    match Q_LANES.load(std::sync::atomic::Ordering::Relaxed) {
        0 => std::env::var("ZKS_Q_LANES")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|l| [4, 8, 16].contains(l))
            .unwrap_or(4),
        lanes => lanes,
    }
}

impl<F: Scalar> Tape<F> {
    /// Registers of the batched form.
    pub fn registers(&self) -> usize {
        self.regs.count
    }

    /// `run` at up to `L` points at once: every instruction over the lanes
    /// of its registers (independent products, no dispatch per point).
    fn run_lanes<const L: usize>(&self, points: &[Vec<F>]) -> Vec<(F, F, F)> {
        assert!(points.len() <= L, "at most one point per lane");
        let rg = &self.regs;
        let mut r: Vec<[F; L]> = vec![[F::ZERO; L]; rg.count];
        for (j, &g) in rg.inputs.iter().enumerate() {
            for (l, p) in points.iter().enumerate() {
                r[g as usize][l] = p[j];
            }
        }
        for &(op, d) in &rg.ops {
            let v: [F; L] = match op {
                Op::Add(a, b) => {
                    let (x, y) = (&r[a as usize], &r[b as usize]);
                    std::array::from_fn(|l| x[l] + y[l])
                }
                Op::Sub(a, b) => {
                    let (x, y) = (&r[a as usize], &r[b as usize]);
                    std::array::from_fn(|l| x[l] - y[l])
                }
                Op::Mul(a, b) => {
                    let (x, y) = (&r[a as usize], &r[b as usize]);
                    std::array::from_fn(|l| x[l] * y[l])
                }
                Op::Scale(a, k) => {
                    let (x, c) = (&r[a as usize], self.pool[k as usize]);
                    std::array::from_fn(|l| x[l] * c)
                }
                Op::Const(k) => [self.pool[k as usize]; L],
            };
            r[d as usize] = v;
        }
        let o = rg.out.map(|g| g as usize);
        (0..points.len())
            .map(|l| (r[o[0]][l], r[o[1]][l], r[o[2]][l]))
            .collect()
    }

    /// `run` at many points: groups of lanes over the register form, the
    /// groups in parallel; the same values as one `run` per point.
    pub fn run_many(&self, points: &[Vec<F>]) -> Vec<(F, F, F)> {
        use rayon::prelude::*;
        assert!(
            points.iter().all(|p| p.len() == self.inputs),
            "one value per input slot"
        );
        let groups: Vec<Vec<(F, F, F)>> = match q_lanes() {
            8 => points
                .par_chunks(8)
                .map(|g| self.run_lanes::<8>(g))
                .collect(),
            16 => points
                .par_chunks(16)
                .map(|g| self.run_lanes::<16>(g))
                .collect(),
            _ => points
                .par_chunks(4)
                .map(|g| self.run_lanes::<4>(g))
                .collect(),
        };
        groups.concat()
    }

    pub fn run(&self, inputs: &[F]) -> (F, F, F) {
        assert_eq!(inputs.len(), self.inputs, "one value per input slot");
        let mut v: Vec<F> = Vec::with_capacity(self.inputs + self.ops.len());
        v.extend_from_slice(inputs);
        for op in &self.ops {
            let x = match *op {
                Op::Add(a, b) => v[a as usize] + v[b as usize],
                Op::Sub(a, b) => v[a as usize] - v[b as usize],
                Op::Mul(a, b) => v[a as usize] * v[b as usize],
                Op::Scale(a, k) => v[a as usize] * self.pool[k as usize],
                Op::Const(k) => self.pool[k as usize],
            };
            v.push(x);
        }
        (
            v[self.out[0] as usize],
            v[self.out[1] as usize],
            v[self.out[2] as usize],
        )
    }
}

/// Builds a program: folds constants, shares equal instructions, and drops
/// what the outputs never read.
struct Builder<F> {
    inputs: usize,
    ops: Vec<Op>,
    pool: Vec<F>,
    pool_index: HashMap<[u8; 32], u32>,
    const_slot: HashMap<u32, Slot>,
    /// The value of each slot that holds a constant.
    known: Vec<Option<F>>,
    shared: HashMap<(u8, Slot, u32), Slot>,
    /// Products of two slots emitted so far (before dead-code removal).
    muls: usize,
}

impl<F: Scalar> Builder<F> {
    fn new(inputs: usize) -> Self {
        Builder {
            inputs,
            ops: Vec::new(),
            pool: Vec::new(),
            pool_index: HashMap::new(),
            const_slot: HashMap::new(),
            known: vec![None; inputs],
            shared: HashMap::new(),
            muls: 0,
        }
    }

    fn op(&self, s: Slot) -> Option<Op> {
        (s as usize).checked_sub(self.inputs).map(|i| self.ops[i])
    }

    fn known(&self, s: Slot) -> Option<F> {
        self.known[s as usize]
    }

    fn pool(&mut self, c: F) -> u32 {
        let key = c.to_repr();
        if let Some(&k) = self.pool_index.get(&key) {
            return k;
        }
        let k = self.pool.len() as u32;
        self.pool.push(c);
        self.pool_index.insert(key, k);
        k
    }

    fn push(&mut self, op: Op, value: Option<F>) -> Slot {
        let s = (self.inputs + self.ops.len()) as Slot;
        self.ops.push(op);
        self.known.push(value);
        s
    }

    fn konst(&mut self, c: F) -> Slot {
        let k = self.pool(c);
        if let Some(&s) = self.const_slot.get(&k) {
            return s;
        }
        let s = self.push(Op::Const(k), Some(c));
        self.const_slot.insert(k, s);
        s
    }

    fn shared(&mut self, key: (u8, Slot, u32), op: Op) -> Slot {
        if let Some(&s) = self.shared.get(&key) {
            return s;
        }
        let s = self.push(op, None);
        self.shared.insert(key, s);
        if key.0 == 2 {
            self.muls += 1;
        }
        s
    }

    fn add(&mut self, a: Slot, b: Slot) -> Slot {
        match (self.known(a), self.known(b)) {
            (Some(x), Some(y)) => return self.konst(x + y),
            (Some(x), _) if bool::from(x.is_zero()) => return b,
            (_, Some(y)) if bool::from(y.is_zero()) => return a,
            _ => {}
        }
        // (p - q) + q = p.
        if let Some(Op::Sub(p, q)) = self.op(a)
            && q == b
        {
            return p;
        }
        if let Some(Op::Sub(p, q)) = self.op(b)
            && q == a
        {
            return p;
        }
        let (a, b) = (a.min(b), a.max(b));
        self.shared((0, a, b), Op::Add(a, b))
    }

    fn sub(&mut self, a: Slot, b: Slot) -> Slot {
        if a == b {
            return self.konst(F::ZERO);
        }
        match (self.known(a), self.known(b)) {
            (Some(x), Some(y)) => return self.konst(x - y),
            (_, Some(y)) if bool::from(y.is_zero()) => return a,
            _ => {}
        }
        self.shared((1, a, b), Op::Sub(a, b))
    }

    fn mul(&mut self, a: Slot, b: Slot) -> Slot {
        match (self.known(a), self.known(b)) {
            (Some(x), Some(y)) => return self.konst(x * y),
            (Some(x), _) => return self.scale(b, x),
            (_, Some(y)) => return self.scale(a, y),
            _ => {}
        }
        let (a, b) = (a.min(b), a.max(b));
        self.shared((2, a, b), Op::Mul(a, b))
    }

    fn scale(&mut self, a: Slot, c: F) -> Slot {
        if bool::from(c.is_zero()) {
            return self.konst(F::ZERO);
        }
        if c == F::ONE {
            return a;
        }
        if let Some(x) = self.known(a) {
            return self.konst(x * c);
        }
        if let Some(Op::Scale(b, k)) = self.op(a) {
            let c2 = self.pool[k as usize] * c;
            return self.scale(b, c2);
        }
        if c == -F::ONE {
            let z = self.konst(F::ZERO);
            return self.sub(z, a);
        }
        let k = self.pool(c);
        self.shared((3, a, k), Op::Scale(a, k))
    }

    /// The program computing `out`: unread instructions dropped, slots
    /// renumbered.
    fn finish(self, out: [Slot; 3]) -> Tape<F> {
        let (ni, n) = (self.inputs, self.inputs + self.ops.len());
        let mut live = vec![false; n];
        for o in out {
            live[o as usize] = true;
        }
        for s in (ni..n).rev() {
            if !live[s] {
                continue;
            }
            match self.ops[s - ni] {
                Op::Add(a, b) | Op::Sub(a, b) | Op::Mul(a, b) => {
                    live[a as usize] = true;
                    live[b as usize] = true;
                }
                Op::Scale(a, _) => live[a as usize] = true,
                Op::Const(_) => {}
            }
        }
        let mut at: Vec<Slot> = (0..n as Slot).collect();
        let mut ops = Vec::new();
        let (mut muls, mut scales) = (0, 0);
        for s in ni..n {
            if !live[s] {
                continue;
            }
            let op = {
                let r = |x: Slot| at[x as usize];
                match self.ops[s - ni] {
                    Op::Add(a, b) => Op::Add(r(a), r(b)),
                    Op::Sub(a, b) => Op::Sub(r(a), r(b)),
                    Op::Mul(a, b) => {
                        muls += 1;
                        Op::Mul(r(a), r(b))
                    }
                    Op::Scale(a, k) => {
                        scales += 1;
                        Op::Scale(r(a), k)
                    }
                    Op::Const(k) => Op::Const(k),
                }
            };
            at[s] = (ni + ops.len()) as Slot;
            ops.push(op);
        }
        let out = out.map(|o| at[o as usize]);
        let regs = Regs::of(ni, &ops, out);
        Tape {
            inputs: ni,
            ops,
            pool: self.pool,
            out,
            muls,
            scales,
            regs,
        }
    }
}

/// A slot of the program being built, with the arithmetic operators.
#[derive(Clone, Copy)]
struct V<'t, F: Scalar> {
    t: &'t RefCell<Builder<F>>,
    s: Slot,
}

impl<'t, F: Scalar> V<'t, F> {
    fn lift(t: &'t RefCell<Builder<F>>, c: F) -> Self {
        V {
            t,
            s: t.borrow_mut().konst(c),
        }
    }
    /// A constant of the same program.
    fn c(self, c: F) -> Self {
        Self::lift(self.t, c)
    }
}

impl<'t, F: Scalar> Add for V<'t, F> {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        V {
            t: self.t,
            s: self.t.borrow_mut().add(self.s, o.s),
        }
    }
}

impl<'t, F: Scalar> Sub for V<'t, F> {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        V {
            t: self.t,
            s: self.t.borrow_mut().sub(self.s, o.s),
        }
    }
}

impl<'t, F: Scalar> Mul for V<'t, F> {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        V {
            t: self.t,
            s: self.t.borrow_mut().mul(self.s, o.s),
        }
    }
}

impl<'t, F: Scalar> Mul<F> for V<'t, F> {
    type Output = Self;
    fn mul(self, c: F) -> Self {
        V {
            t: self.t,
            s: self.t.borrow_mut().scale(self.s, c),
        }
    }
}

/// One proof's relation: its LogUp challenges, and the template rows with
/// the LogUp rows filled in, which only the prover's kernels read (built on
/// first use).
pub struct UniformRel<'a, F> {
    c: &'a UniformCircuit<F>,
    ch: Vec<F>,
    rows: OnceLock<Vec<Vec<[Lin<F>; 3]>>>,
}

impl<F: Scalar> UniformRel<'_, F> {
    fn rows(&self) -> &Vec<Vec<[Lin<F>; 3]>> {
        self.rows.get_or_init(|| self.c.instantiated_rows(&self.ch))
    }
}

impl<F: Scalar> Rel<F> for UniformRel<'_, F> {
    fn abc(&self, shape: &Shape, z: &[F]) -> (Vec<F>, Vec<F>, Vec<F>) {
        let c = self.c;
        let rows = c.symbolic();
        let jobs = c.gather_plan();
        let n = 1 << shape.sx;
        let mut out = [vec![F::ZERO; n], vec![F::ZERO; n], vec![F::ZERO; n]];
        // Row blocks may overlap at absent instances; every job writes only
        // its present rows, which no other job writes.
        let ptrs = [
            OutPtr(out[0].as_mut_ptr()),
            OutPtr(out[1].as_mut_ptr()),
            OutPtr(out[2].as_mut_ptr()),
        ];
        jobs.par_iter().for_each(|g| {
            let optr = ptrs[g.mat];
            let arr = &c.arrays[g.a];
            let lin = &rows[g.a][g.j][g.mat];
            let fv: Vec<Option<F>> = g
                .factors
                .iter()
                .map(|f| UniformCircuit::<F>::factor_value(&self.ch, *f))
                .collect();
            // Sum terms take the same value at every instance.
            let sums: F = g
                .sums
                .iter()
                .map(|&ti| match &lin[ti].0.src {
                    Src::All { array, cell } => {
                        let coef = fv[ti].map_or(lin[ti].0.coef[0], |f| f * lin[ti].0.coef[0]);
                        coef * (0..1usize << c.m(*array))
                            .filter(|i2| c.present(*array, *i2))
                            .map(|i2| z[c.blocks[*array][*cell].index(i2)])
                            .sum::<F>()
                    }
                    _ => unreachable!("a sum term"),
                })
                .sum();
            // Instances in runs of p, so a large block is shared by workers.
            let count = c.counts[arr.family];
            let span = (GATHER_SPAN >> arr.rho).max(1);
            if count <= span {
                g.run(0..count, arr.rho, lin, &fv, z, sums, optr);
            } else {
                (0..count.div_ceil(span)).into_par_iter().for_each(|i| {
                    let p0 = i * span;
                    g.run(p0..(p0 + span).min(count), arr.rho, lin, &fv, z, sums, optr);
                });
            }
        });
        let [a, b, cc] = out;
        (a, b, cc)
    }

    fn m_table(&self, shape: &Shape, ex: &[F], ra: F, rb: F, rc: F) -> Vec<F> {
        let c = self.c;
        let rm = [ra, rb, rc];
        // The symbolic rows with each term's factor value: no row is
        // instantiated per proof.
        let (rows, rd) = (c.symbolic(), self.c.readers());
        let weight = |a: usize, j: usize, mat: usize, t: usize| -> F {
            UniformCircuit::<F>::factor_value(&self.ch, rows[a][j][mat][t].1)
                .map_or(rm[mat], |f| rm[mat] * f)
        };
        let mut out = vec![F::ZERO; 1 << shape.sz()];
        // One job per placed block, large blocks in ranges of instances; the
        // rows that read each instance come from the circuit's flat plan.
        const SPAN: usize = 2048;
        let plans = c.m_plans();
        let jobs: Vec<(usize, usize)> = plans
            .iter()
            .enumerate()
            .flat_map(|(b, plan)| {
                (0..(1usize << c.m(plan.a2)).div_ceil(SPAN)).map(move |s| (b, s * SPAN))
            })
            .collect();
        // Each reader's matrix challenge times its factor's value.
        let weights: Vec<Vec<F>> = plans
            .iter()
            .map(|plan| {
                plan.readers
                    .iter()
                    .map(|r| weight(r.a, r.j, r.mat, r.t))
                    .collect()
            })
            .collect();
        let writes: Vec<Vec<(usize, F)>> = jobs
            .par_iter()
            .map(|&(b, start)| {
                let (plan, weights) = (&plans[b], &weights[b]);
                let (a2, k2) = (plan.a2, plan.k2);
                let rho2 = c.arrays[a2].rho;
                let block = &c.blocks[a2][k2];
                // A sum row adds the same weight to every instance of its block.
                let all: F = rd.all_readers[a2][k2]
                    .iter()
                    .map(|&(a, j, mat, t)| {
                        ex[c.row_base[a][j]] * weight(a, j, mat, t) * rows[a][j][mat][t].0.coef[0]
                    })
                    .sum();
                (start..(start + SPAN).min(1usize << c.m(a2)))
                    .filter(|i2| c.present(a2, *i2))
                    .map(|i2| {
                        let (p, r2) = (i2 >> rho2, i2 & ((1 << rho2) - 1));
                        let mut acc = all;
                        let entries =
                            &plan.entries[plan.offsets[r2] as usize..plan.offsets[r2 + 1] as usize];
                        let mut at = 0;
                        while at < entries.len() {
                            // One reader's rows: their sum, then one product
                            // by the reader's weight.
                            let ri = entries[at].0;
                            let reader = &plan.readers[ri as usize];
                            let mut sum = F::ZERO;
                            while at < entries.len() && entries[at].0 == ri {
                                let (_, kind, row, index) = entries[at];
                                let e = match reader.all_p {
                                    None => ex[row as usize + (p << reader.rho)],
                                    Some(count) => (0..count)
                                        .map(|pp| ex[row as usize + (pp << reader.rho)])
                                        .sum(),
                                };
                                match kind {
                                    1 => sum += e,
                                    2 => sum -= e,
                                    _ => sum += plan.coefs[index as usize] * e,
                                }
                                at += 1;
                            }
                            acc += weights[ri as usize] * sum;
                        }
                        (block.index(i2), acc)
                    })
                    .collect()
            })
            .collect();
        for list in writes {
            for (y, v) in list {
                out[y] = v;
            }
        }
        let one: F = rd
            .one_readers
            .par_iter()
            .map(|&(a, j, mat, t)| {
                let rmask = (1 << c.arrays[a].rho) - 1;
                let base = c.row_base[a][j];
                let coef = &rows[a][j][mat][t].0.coef;
                (0..1usize << c.m(a))
                    .filter(|i| c.present(a, *i))
                    .map(|i| ex[base + i] * coef[i & rmask])
                    .sum::<F>()
                    * weight(a, j, mat, t)
            })
            .sum();
        out[1 << shape.sw()] += one;
        out
    }

    fn matrix_evals(&self, shape: &Shape, rx: &[F], ry: &[F]) -> (F, F, F) {
        assert_eq!(
            (shape.sx, shape.sw()),
            (self.c.shape.sx, self.c.shape.sw()),
            "the circuit's shape"
        );
        self.c.evals(rx, ry, &self.ch)
    }

    fn nnz(&self) -> [usize; 3] {
        let c = self.c;
        let mut out = [0usize; 3];
        for (a, arr) in c.arrays.iter().enumerate() {
            let p = c.counts[arr.family];
            let rs: Vec<usize> = (0..1usize << arr.rho)
                .filter(|r| c.r_present(a, *r))
                .collect();
            for row in &self.rows()[a] {
                for (mat, lin) in row.iter().enumerate() {
                    for term in lin {
                        out[mat] += match &term.src {
                            Src::Link { map, .. } => {
                                p * rs.iter().filter(|r| map[**r].is_some()).count()
                            }
                            Src::All { array, .. } => c.instances(*array),
                            _ => p * rs.len(),
                        };
                    }
                }
            }
        }
        out
    }
}

impl<F: Scalar> Circuit<F> for UniformCircuit<F> {
    fn randomize_masks(&self, w: &mut [F], rng: &mut SpikeRng) {
        assert!(self.verifier_only.is_none(), "an imported public program has no prover witness");
        for &(array, cells, instance) in &self.product_masks {
            let a = rng.scalar();
            let b = rng.scalar();
            for (cell, value) in cells.into_iter().zip([a, b, a * b]) {
                w[self.cell_index(array, cell, instance)] = value;
            }
        }
    }

    fn phase2_challenges(&self) -> usize {
        if self.logup() { 2 } else { 0 }
    }
    fn instantiate<'a>(
        &'a self,
        _shape: &Shape,
        ch: &[F],
        w: Option<&mut [F]>,
    ) -> Box<dyn Rel<F> + 'a> {
        if let Some(w) = w
            && self.logup()
        {
            assert!(self.verifier_only.is_none(), "an imported public program cannot fill private witnesses");
            self.fill_helpers(ch, w);
            if let Some(fill) = &self.ch_fill {
                for (a, k, i, v) in fill(ch) {
                    w[self.cell_index(a, k, i)] = v;
                }
            }
        }
        Box::new(UniformRel {
            c: self,
            ch: ch.to_vec(),
            rows: OnceLock::new(),
        })
    }
    fn row_bounds(&self, _shape: &Shape) -> Option<Vec<usize>> {
        Some(self.bounds.clone())
    }
    fn phase1_rows(&self, _shape: &Shape) -> usize {
        self.p1_rows
    }
    fn fixed_rows(&self, _shape: &Shape) -> usize {
        self.fixed_rows
    }
    fn fixed_values(&self, _shape: &Shape) -> Vec<F> {
        self.fixed_values.clone()
    }
    fn wallet_rows(&self) -> usize {
        self.wallet_rows
    }
    fn wallet_io(&self) -> Option<usize> {
        self.wallet_io
    }
    fn late_slots(&self) -> Vec<usize> {
        self.late_io.iter().map(|x| x.0).collect()
    }
    fn late_values(&self, w: &[F]) -> Vec<F> {
        self.late_io
            .iter()
            .map(|&(_, a, k, i)| w[self.cell_index(a, k, i)])
            .collect()
    }
    fn link_rows(&self, _shape: &Shape) -> usize {
        self.pl_rows
    }
    fn link_io(&self, rho: &[u8; 32], r: &[u8; 32]) -> Vec<(usize, F)> {
        match &self.link_io_values {
            None => Vec::new(),
            Some(values) => {
                let v = values(rho, r);
                assert_eq!(v.len(), self.link_io.len(), "a value per link io array");
                self.link_io.iter().map(|&(_, slot)| slot).zip(v).collect()
            }
        }
    }
    fn fill_link(&self, rho: &[u8; 32], w: &mut [F]) {
        if let Some(fill) = &self.link_fill {
            for (a, k, i, v) in fill(rho) {
                w[self.cell_index(a, k, i)] = v;
            }
        }
        self.recount_link_tables(w);
    }
    fn binding_r(&self, w: &[F]) -> Option<[u8; 32]> {
        use num_bigint::BigUint;
        let (cells, bits) = self.link_binding.as_ref()?;
        let mut r = BigUint::default();
        for (j, &(a, k, i)) in cells.iter().enumerate() {
            let v = w[self.cell_index(a, k, i)];
            r += BigUint::from_bytes_le(v.to_repr().as_ref()) << (*bits as usize * j);
        }
        let bytes = r.to_bytes_le();
        // A value past 32 bytes is no word: it never equals the link's R.
        let mut word = [0xff_u8; 32];
        if bytes.len() <= 32 {
            word = [0_u8; 32];
            word[..bytes.len()].copy_from_slice(&bytes);
        }
        Some(word)
    }
}

// ---------------------------------------------------------------------------
// Template building: linear combinations over sources, coefficients per r.

pub(crate) fn compact<F: Field>(lin: Lin<F>) -> Lin<F> {
    let mut out: Vec<Term<F>> = Vec::new();
    let mut index: HashMap<Src, usize> = HashMap::new();
    for t in lin {
        match index.get(&t.src) {
            Some(&k) => {
                for (a, b) in out[k].coef.iter_mut().zip(&t.coef) {
                    *a += b;
                }
            }
            None => {
                index.insert(t.src.clone(), out.len());
                out.push(t);
            }
        }
    }
    out.retain(|t| t.coef.iter().any(|c| !bool::from(c.is_zero())));
    out
}

pub(crate) fn plus<F: Field>(a: &Lin<F>, b: &Lin<F>) -> Lin<F> {
    compact(a.iter().chain(b).cloned().collect())
}

pub(crate) fn scaled<F: Field>(a: &Lin<F>, s: F) -> Lin<F> {
    compact(
        a.iter()
            .map(|t| Term {
                src: t.src.clone(),
                coef: t.coef.iter().map(|c| *c * s).collect(),
            })
            .collect(),
    )
}

pub(crate) fn own<F: Field>(k: usize, n: usize) -> Lin<F> {
    vec![Term {
        src: Src::Own(k),
        coef: vec![F::ONE; n],
    }]
}

pub(crate) fn constant<F: Field>(values: Vec<F>) -> Lin<F> {
    vec![Term {
        src: Src::One,
        coef: values,
    }]
}

pub(crate) fn link<F: Field>(
    array: usize,
    cell: usize,
    map: Vec<Option<usize>>,
    coef: F,
) -> Lin<F> {
    let n = map.len();
    vec![Term {
        src: Src::Link { array, cell, map },
        coef: vec![coef; n],
    }]
}

/// Array indices of the RP57 encoding.
pub const RP_INPUT: usize = 0;
pub const RP_FULL: usize = 1;
pub const RP_PART1: usize = 2;
pub const RP_PART7: usize = 3;
pub const RP_MASK: usize = 4;

/// `2^log` RP57 permutations as round arrays, on random inputs:
///
/// - INPUT (p): the three input lanes (free cells here; copy lookups later);
/// - FULL (p, f), f < 8: full rounds 0-3 and 61-64, three x^5 boxes (9 rows);
/// - PART1 (p): partial round 4, materialising its output state (6 rows);
/// - PART7 (p, b), b < 8: partial rounds 5 + 7b .. 11 + 7b, lanes 1-2 kept as
///   linear combinations inside the block, the state materialised at its end
///   (24 rows);
/// - MASK: the two random product gates that hide v_A, v_B, v_C.
///
/// Rounds link by maps on r; round constants are per-r coefficients.
pub fn rp57<F: Scalar + PoseidonField>(
    log: usize,
    seed: &[u8],
) -> (Vec<Array<F>>, Vec<usize>, Vec<[F; 3]>) {
    let mut rng = SpikeRng::new(seed);
    let inputs: Vec<[F; 3]> = (0..1usize << log)
        .map(|_| std::array::from_fn(|_| rng.scalar()))
        .collect();
    rp57_with(log, &inputs, &mut rng)
}

/// [`rp57`] on given inputs (`inputs.len() <= 2^log` permutations; the rest
/// of the p range is absent), the mask gates drawn from `rng`.
pub fn rp57_with<F: Scalar + PoseidonField>(
    log: usize,
    inputs: &[[F; 3]],
    rng: &mut SpikeRng,
) -> (Vec<Array<F>>, Vec<usize>, Vec<[F; 3]>) {
    let params = F::rp57();
    let rc = params.round_constants();
    let mds = params.mds();
    let n = 1usize << log;
    assert!(inputs.len() <= n, "permutations fit 2^log");
    let full_round = |f: usize| if f < 4 { f } else { 57 + f };
    let pow5 = |x: F| {
        let x2 = x.square();
        let x4 = x2.square();
        (x2, x4, x4 * x)
    };
    let mix = |u: [F; 3]| -> [F; 3] {
        std::array::from_fn(|k| mds[k][0] * u[0] + mds[k][1] * u[1] + mds[k][2] * u[2])
    };
    // Witness: every round's values, natively.
    let mut input = vec![vec![F::ZERO; n]; 3];
    let mut full = vec![vec![F::ZERO; n * 8]; 9];
    let mut part1 = vec![vec![F::ZERO; n]; 6];
    let mut part7 = vec![vec![F::ZERO; n * 8]; 24];
    let mut outputs = Vec::with_capacity(n);
    let full_rounds = |s: &mut [F; 3], cells: &mut [Vec<F>], at: usize, round: usize| {
        let mut x5 = [F::ZERO; 3];
        for j in 0..3 {
            let (x2, x4, x5j) = pow5(s[j] + rc[round][j]);
            cells[3 * j][at] = x2;
            cells[3 * j + 1][at] = x4;
            cells[3 * j + 2][at] = x5j;
            x5[j] = x5j;
        }
        *s = mix(x5);
    };
    for (p, start) in inputs.iter().enumerate() {
        let start = *start;
        let mut s = start;
        for k in 0..3 {
            input[k][p] = s[k];
        }
        for f in 0..4 {
            full_rounds(&mut s, &mut full, p * 8 + f, full_round(f));
        }
        let (x2, x4, x5) = pow5(s[0] + rc[4][0]);
        s = mix([x5, s[1] + rc[4][1], s[2] + rc[4][2]]);
        for (k, v) in [x2, x4, x5, s[0], s[1], s[2]].into_iter().enumerate() {
            part1[k][p] = v;
        }
        for b in 0..8 {
            for t in 0..7 {
                let round = 5 + 7 * b + t;
                let (x2, x4, x5) = pow5(s[0] + rc[round][0]);
                part7[3 * t][p * 8 + b] = x2;
                part7[3 * t + 1][p * 8 + b] = x4;
                part7[3 * t + 2][p * 8 + b] = x5;
                s = mix([x5, s[1] + rc[round][1], s[2] + rc[round][2]]);
            }
            for k in 0..3 {
                part7[21 + k][p * 8 + b] = s[k];
            }
        }
        for f in 4..8 {
            full_rounds(&mut s, &mut full, p * 8 + f, full_round(f));
        }
        let mut check = start;
        permute_with(&mut check, params);
        assert_eq!(check, s, "round arrays reproduce the permutation");
        outputs.push(s);
    }
    // Templates.
    let one8 = || constant::<F>(vec![F::ONE; 8]);
    let full_rows: Vec<[Lin<F>; 3]> = (0..3)
        .flat_map(|j| {
            let mut t: Lin<F> = Vec::new();
            for k in 0..3 {
                let map = (0..8)
                    .map(|f| if f == 0 || f == 4 { None } else { Some(f - 1) })
                    .collect();
                t = plus(&t, &link(RP_FULL, 3 * k + 2, map, mds[j][k]));
            }
            t = plus(
                &t,
                &link(
                    RP_PART7,
                    21 + j,
                    (0..8)
                        .map(|f| if f == 4 { Some(7) } else { None })
                        .collect(),
                    F::ONE,
                ),
            );
            t = plus(
                &t,
                &link(
                    RP_INPUT,
                    j,
                    (0..8)
                        .map(|f| if f == 0 { Some(0) } else { None })
                        .collect(),
                    F::ONE,
                ),
            );
            t = plus(
                &t,
                &constant((0..8).map(|f| rc[full_round(f)][j]).collect()),
            );
            [
                [t.clone(), t.clone(), own(3 * j, 8)],
                [own(3 * j, 8), own(3 * j, 8), own(3 * j + 1, 8)],
                [own(3 * j + 1, 8), t, own(3 * j + 2, 8)],
            ]
        })
        .collect();
    let mut part1_rows: Vec<[Lin<F>; 3]> = Vec::new();
    {
        let s_in: Vec<Lin<F>> = (0..3)
            .map(|k| {
                (0..3).fold(Vec::new(), |acc, l| {
                    plus(&acc, &link(RP_FULL, 3 * l + 2, vec![Some(3)], mds[k][l]))
                })
            })
            .collect();
        let t0 = plus(&s_in[0], &constant(vec![rc[4][0]]));
        part1_rows.push([t0.clone(), t0.clone(), own(0, 1)]);
        part1_rows.push([own(0, 1), own(0, 1), own(1, 1)]);
        part1_rows.push([own(1, 1), t0, own(2, 1)]);
        let u1 = plus(&s_in[1], &constant(vec![rc[4][1]]));
        let u2 = plus(&s_in[2], &constant(vec![rc[4][2]]));
        for k in 0..3 {
            let out = plus(
                &plus(&scaled(&own(2, 1), mds[k][0]), &scaled(&u1, mds[k][1])),
                &scaled(&u2, mds[k][2]),
            );
            part1_rows.push([
                plus(&out, &scaled(&own(3 + k, 1), -F::ONE)),
                constant(vec![F::ONE]),
                Vec::new(),
            ]);
        }
    }
    let mut part7_rows: Vec<[Lin<F>; 3]> = Vec::new();
    {
        let mut s: Vec<Lin<F>> = (0..3)
            .map(|k| {
                plus(
                    &link(
                        RP_PART1,
                        3 + k,
                        (0..8)
                            .map(|b| if b == 0 { Some(0) } else { None })
                            .collect(),
                        F::ONE,
                    ),
                    &link(
                        RP_PART7,
                        21 + k,
                        (0..8)
                            .map(|b| if b == 0 { None } else { Some(b - 1) })
                            .collect(),
                        F::ONE,
                    ),
                )
            })
            .collect();
        for t in 0..7 {
            let rcv = |j: usize| constant::<F>((0..8).map(|b| rc[5 + 7 * b + t][j]).collect());
            let t0 = plus(&s[0], &rcv(0));
            part7_rows.push([t0.clone(), t0.clone(), own(3 * t, 8)]);
            part7_rows.push([own(3 * t, 8), own(3 * t, 8), own(3 * t + 1, 8)]);
            part7_rows.push([own(3 * t + 1, 8), t0, own(3 * t + 2, 8)]);
            let u1 = plus(&s[1], &rcv(1));
            let u2 = plus(&s[2], &rcv(2));
            s = (0..3)
                .map(|k| {
                    plus(
                        &plus(
                            &scaled(&own(3 * t + 2, 8), mds[k][0]),
                            &scaled(&u1, mds[k][1]),
                        ),
                        &scaled(&u2, mds[k][2]),
                    )
                })
                .collect();
        }
        for k in 0..3 {
            part7_rows.push([
                plus(&s[k], &scaled(&own(21 + k, 8), -F::ONE)),
                one8(),
                Vec::new(),
            ]);
        }
    }
    let mask_vals: [Vec<F>; 3] = {
        let a: Vec<F> = (0..2).map(|_| rng.scalar()).collect();
        let b: Vec<F> = (0..2).map(|_| rng.scalar()).collect();
        let c = a.iter().zip(&b).map(|(x, y)| *x * y).collect();
        [a, b, c]
    };
    let arrays = vec![
        Array {
            name: "input",
            family: 0,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; 3],
            rows: Vec::new(),
            values: input,
        },
        Array {
            name: "full round",
            family: 0,
            rho: 3,
            r_count: 8,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; 9],
            rows: full_rows,
            values: full,
        },
        Array {
            name: "partial round 4",
            family: 0,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; 6],
            rows: part1_rows,
            values: part1,
        },
        Array {
            name: "7 partial rounds",
            family: 0,
            rho: 3,
            r_count: 8,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; 24],
            rows: part7_rows,
            values: part7,
        },
        Array {
            name: "mask",
            family: 1,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; 3],
            rows: vec![[own(0, 1), own(1, 1), own(2, 1)]],
            values: mask_vals.to_vec(),
        },
    ];
    (arrays, vec![log, 1], outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::eq_table;
    use iroha_pasta::Eq as Vesta;

    type Fv = <Vesta as iroha_pasta::PastaCurve>::ScalarExt;

    #[test]
    fn sparse_reader_csr_preserves_cross_family_and_repeated_maps() {
        let n = 1usize << 15;
        let target = Array {
            name: "large sparse reader target", family: 0, rho: 15,
            r_count: n, fixed: false, lookups: vec![], widths: vec![255], rows: vec![],
            values: vec![(0..n).map(|i| Fv::from((i + 1) as u64)).collect()],
        };
        let mut readers = Array {
            name: "many small cross-family readers", family: 1, rho: 2,
            r_count: 3, fixed: false, lookups: vec![], widths: vec![255],
            rows: vec![], values: vec![vec![Fv::ONE; 16]],
        };
        for i in 0..128 {
            let repeated = (i * 127) % n;
            readers.rows.push([
                link(0, 0, vec![Some(repeated), Some(repeated), None, Some(n - 1)], Fv::from((i + 1) as u64)),
                own(0, 4),
                constant(vec![Fv::from(9); 4]),
            ]);
        }
        let (c, _, _) = UniformCircuit::build(
            "sparse readers", vec![target, readers], vec![0, 2], vec![1, 3],
            vec![], 12, 4, b"sparse readers",
        );
        let rd = c.readers();
        let symbolic = c.symbolic();
        for (a, cells) in rd.readers.iter().enumerate() {
            for (cell, list) in cells.iter().enumerate() {
                let (offsets, pairs) = &rd.at[a][cell];
                assert_eq!(offsets.len(), (1usize << c.arrays[a].rho) + 1);
                let mut expected = std::collections::BTreeMap::<usize, Vec<(u32,u32)>>::new();
                for (ri, reader) in list.iter().enumerate() {
                    let term = &symbolic[reader.array][reader.row][reader.mat][reader.term].0;
                    match &term.src {
                        Src::Own(_) => for r in 0..1usize << c.arrays[reader.array].rho {
                            expected.entry(r).or_default().push((ri as u32,r as u32));
                        },
                        Src::Link { map, .. } => for (r, target) in map.iter().enumerate() {
                            if let Some(target) = target {
                                expected.entry(*target).or_default().push((ri as u32,r as u32));
                            }
                        },
                        _ => unreachable!(),
                    }
                }
                assert_eq!(pairs.len(), expected.values().map(Vec::len).sum::<usize>());
                for r in 0..offsets.len()-1 {
                    let actual = &pairs[offsets[r] as usize..offsets[r+1] as usize];
                    assert_eq!(actual, expected.get(&r).map_or(&[][..], Vec::as_slice), "block {a}:{cell}, target {r}");
                }
            }
        }
        // Compare the complete gather against direct sparse matrix expansion,
        // including all source-family repetitions and inactive source slots.
        let mut rng = SpikeRng::new(b"sparse reader m-table");
        let rx: Vec<Fv> = (0..c.shape.sx).map(|_| rng.scalar()).collect();
        let ex = eq_table(&rx);
        let weights = [rng.scalar(),rng.scalar(),rng.scalar()];
        let ch = [Fv::from(31), Fv::from(47)];
        let rel = c.instantiate(&c.shape,&ch,None);
        let actual = rel.m_table(&c.shape,&ex,weights[0],weights[1],weights[2]);
        let mut expected = vec![Fv::ZERO;1usize << c.shape.sz()];
        for (matrix,row,col,value) in c.entries(&ch) {
            expected[col] += weights[matrix] * ex[row] * value;
        }
        assert_eq!(actual,expected);
    }

    fn small() -> (UniformCircuit<Fv>, Instance<Fv>, Witness<Fv>) {
        let (arrays, pi, _) = rp57::<Fv>(2, b"uniform-small");
        let counts = pi.iter().map(|p| 1 << p).collect();
        UniformCircuit::build(
            "uniform-test",
            arrays,
            pi,
            counts,
            Vec::new(),
            11,
            4,
            b"uniform-small",
        )
    }

    #[test]
    fn proof_entropy_refreshes_real_product_masks_and_replays() {
        let (c, inst, wit) = small();
        let mut a = wit.w.clone();
        let mut b = wit.w.clone();
        c.randomize_masks(&mut a, &mut SpikeRng::new(b"private mask proof A"));
        c.randomize_masks(&mut b, &mut SpikeRng::new(b"private mask proof B"));
        assert_ne!(a, b, "fresh entropy changes the actual relation witness");
        let positions: std::collections::HashSet<_> = c.product_masks.iter()
            .flat_map(|(array, cells, i)| cells.iter().map(|cell| c.cell_index(*array, *cell, *i)))
            .collect();
        for i in 0..a.len() {
            if !positions.contains(&i) { assert_eq!(a[i], wit.w[i], "application witness preserved"); }
        }
        for w in [a, b] {
            let z = crate::r1cs::z_vector(&c.shape, &inst, &Witness { w });
            let (aa, bb, cc) = c.instantiate(&c.shape, &[], None).abc(&c.shape, &z);
            assert!(aa.into_iter().zip(bb).zip(cc).all(|((a,b),c)| a*b==c));
        }
        let gens = crate::ipa::Gens::<Vesta>::new(c.shape.cols());
        let prove = |seed: &[u8]| crate::spartan::prove_circuit(
            &c.shape, &gens, &inst, &wit, &mut SpikeRng::new(seed), &c).0;
        let pa = prove(b"private mask proof A");
        let pb = prove(b"private mask proof B");
        let replay = prove(b"private mask proof A");
        assert_ne!(pa.v, pb.v, "fresh masked scalar disclosures");
        assert_eq!(pa.to_bytes(), replay.to_bytes(), "same-operation preview replay");
        for proof in [pa, pb] {
            assert!(crate::spartan::verify_circuit(&c.shape, &gens, &inst, &proof.to_bytes(), &c).is_ok());
        }
    }

    /// The structured Az/Bz/Cz, M(r_x, .) and A~, B~, C~ equal the expanded
    /// matrices', and the honest witness satisfies the relation.
    #[test]
    fn uniform_relation_matches_its_expansion() {
        let (c, inst, wit) = small();
        let shape = c.shape;
        let rel = c.instantiate(&shape, &[], None);
        let z = crate::r1cs::z_vector(&shape, &inst, &wit);
        let (az, bz, cz) = rel.abc(&shape, &z);
        for x in 0..1 << shape.sx {
            assert_eq!(az[x] * bz[x], cz[x], "row {x} holds");
        }
        let entries = c.entries(&[]);
        let mut abc = [
            vec![Fv::ZERO; 1 << shape.sx],
            vec![Fv::ZERO; 1 << shape.sx],
            vec![Fv::ZERO; 1 << shape.sx],
        ];
        for &(mat, x, y, v) in &entries {
            abc[mat][x] += v * z[y];
        }
        assert_eq!(
            (az, bz, cz),
            (abc[0].clone(), abc[1].clone(), abc[2].clone())
        );
        let mut rng = SpikeRng::new(b"points");
        let rx: Vec<Fv> = (0..shape.sx).map(|_| rng.scalar()).collect();
        let ry: Vec<Fv> = (0..shape.sz()).map(|_| rng.scalar()).collect();
        let (ex, ey) = (eq_table(&rx), eq_table(&ry));
        let mut evals = [Fv::ZERO; 3];
        for &(mat, x, y, v) in &entries {
            evals[mat] += v * ex[x] * ey[y];
        }
        assert_eq!(
            rel.matrix_evals(&shape, &rx, &ry),
            (evals[0], evals[1], evals[2])
        );
        let (ra, rb, rcc) = (rng.scalar::<Fv>(), rng.scalar::<Fv>(), rng.scalar::<Fv>());
        let mut m = vec![Fv::ZERO; 1 << shape.sz()];
        for &(mat, x, y, v) in &entries {
            m[y] += ex[x] * v * [ra, rb, rcc][mat];
        }
        assert_eq!(rel.m_table(&shape, &ex, ra, rb, rcc), m);
    }

    /// The interval closed forms (one, own, identity, broadcast, shifts by
    /// -1 and +1, constant and geometric coefficients) equal brute-force sums.
    #[test]
    fn closed_forms_match_enumeration() {
        use ff::PrimeField;
        let mut rng = SpikeRng::new(b"closed forms");
        let mut checked = 0;
        for rho in 1..=5usize {
            let n = 1usize << rho;
            let xv: Vec<Fv> = (0..rho).map(|_| rng.scalar()).collect();
            let yv: Vec<Fv> = (0..rho).map(|_| rng.scalar()).collect();
            let e = |bits: &[Fv], r: usize| -> Fv {
                bits.iter()
                    .enumerate()
                    .map(|(t, b)| if (r >> t) & 1 == 1 { *b } else { Fv::ONE - *b })
                    .product()
            };
            let mut intervals: Vec<(usize, usize)> =
                vec![(0, n), (0, n - 1), (1, n), (0, 1), (1, 2)];
            for _ in 0..6 {
                let a = (rng.scalar::<Fv>().to_repr()[0] as usize) % (n + 1);
                let b = (rng.scalar::<Fv>().to_repr()[0] as usize) % (n + 1);
                intervals.push((a.min(b), a.max(b)));
            }
            for (lo, hi) in intervals {
                if hi <= lo {
                    continue;
                }
                for q in [Fv::ONE, rng.scalar()] {
                    let c0: Fv = rng.scalar();
                    let coef: Vec<Fv> = (0..n)
                        .map(|r| {
                            if (lo..hi).contains(&r) {
                                c0 * q.pow_vartime([r as u64])
                            } else {
                                Fv::ZERO
                            }
                        })
                        .collect();
                    // (kind, target per r): one and own need the interval to start at 0
                    // (they map every r below r_count).
                    let k = (rng.scalar::<Fv>().to_repr()[0] as usize) % n;
                    let kinds: Vec<(&str, Vec<Option<usize>>)> = vec![
                        (
                            "identity",
                            (0..n).map(|r| (lo..hi).contains(&r).then_some(r)).collect(),
                        ),
                        (
                            "broadcast",
                            (0..n).map(|r| (lo..hi).contains(&r).then_some(k)).collect(),
                        ),
                        (
                            "down",
                            (0..n)
                                .map(|r| ((lo..hi).contains(&r) && r >= 1).then(|| r - 1))
                                .collect(),
                        ),
                        (
                            "up",
                            (0..n)
                                .map(|r| ((lo..hi).contains(&r) && r + 1 < n).then_some(r + 1))
                                .collect(),
                        ),
                    ];
                    let mut cases: Vec<(String, Option<Vec<Option<usize>>>, bool)> = kinds
                        .into_iter()
                        .map(|(name, map)| (name.to_string(), Some(map), false))
                        .collect();
                    if lo == 0 {
                        cases.push(("one".into(), None, false));
                        cases.push(("own".into(), None, true));
                    }
                    for (name, map, own_target) in cases {
                        let cell = RefCell::new(Builder::<Fv>::new(2 * rho));
                        let tb = &cell;
                        let xs: Vec<V<'_, Fv>> = (0..rho)
                            .map(|i| V {
                                t: tb,
                                s: i as Slot,
                            })
                            .collect();
                        let ys: Vec<V<'_, Fv>> = (0..rho)
                            .map(|i| V {
                                t: tb,
                                s: (rho + i) as Slot,
                            })
                            .collect();
                        let one = V::lift(tb, Fv::ONE);
                        let r_count = if map.is_none() { hi } else { n };
                        let target = match (&map, own_target) {
                            (None, false) => Target::One,
                            (None, true) => Target::Own(&ys),
                            (Some(m), _) => Target::Link(m, &ys),
                        };
                        let Some(v) = r_closed(rho, r_count, &coef, &target, &xs, one) else {
                            // Shifts that leave the interval empty or broken fall back.
                            continue;
                        };
                        let out = v.s;
                        let tape = std::mem::replace(&mut *tb.borrow_mut(), Builder::new(0))
                            .finish([out, out, out]);
                        let got = tape
                            .run(&xv.iter().chain(&yv).copied().collect::<Vec<_>>())
                            .0;
                        let mut want = Fv::ZERO;
                        for r in 0..r_count {
                            let y = match (&map, own_target) {
                                (None, false) => Some(Fv::ONE),
                                (None, true) => Some(e(&yv, r)),
                                (Some(m), _) => m[r].map(|t| e(&yv, t)),
                            };
                            if let Some(y) = y {
                                want += coef[r] * e(&xv, r) * y;
                            }
                        }
                        assert_eq!(
                            got,
                            want,
                            "rho {rho} [{lo}, {hi}) {name} q {}",
                            q == Fv::ONE
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 150, "{checked} closed forms checked");
    }

    /// LogUp: range lookups into a 2^11-entry constant column and pair
    /// lookups into a 16-entry table, from a partial family (6 of 8 ladders).
    fn lookup_circuit() -> (UniformCircuit<Fv>, Instance<Fv>, Witness<Fv>) {
        let mut rng = SpikeRng::new(b"logup");
        let slots = 8 * 4;
        let mut x = vec![Fv::ZERO; slots];
        let mut sv = vec![Fv::ZERO; slots];
        let mut s2 = vec![Fv::ZERO; slots];
        for i in 0..slots {
            if (i >> 2) < 6 && (i & 3) < 3 {
                x[i] = rng.below(11);
                let s: Fv = rng.below(4);
                sv[i] = s;
                s2[i] = s * s;
            }
        }
        let arrays = vec![
            Array {
                name: "values",
                family: 0,
                rho: 2,
                r_count: 3,
                fixed: false,
                lookups: vec![(0, vec![own(0, 4)]), (1, vec![own(1, 4), own(2, 4)])],
                widths: vec![11, 4, 8],
                rows: vec![[own(1, 4), own(1, 4), own(2, 4)]],
                values: vec![x, sv, s2],
            },
            Array {
                name: "range table",
                family: 1,
                rho: 11,
                r_count: 2048,
                fixed: true,
                lookups: Vec::new(),
                widths: vec![11],
                rows: Vec::new(),
                values: vec![(0..2048u64).map(Fv::from).collect()],
            },
            Array {
                name: "square table",
                family: 1,
                rho: 4,
                r_count: 16,
                fixed: true,
                lookups: Vec::new(),
                widths: vec![4, 8],
                rows: Vec::new(),
                values: vec![
                    (0..16u64).map(Fv::from).collect(),
                    (0..16u64).map(|a| Fv::from(a * a)).collect(),
                ],
            },
        ];
        UniformCircuit::build(
            "logup-test",
            arrays,
            vec![3, 0],
            vec![6, 1],
            vec![(1, 0), (2, 1)],
            12,
            4,
            b"logup",
        )
    }

    #[test]
    fn logup_relation_matches_its_expansion_and_refuses() {
        let (c, inst, wit) = lookup_circuit();
        let shape = c.shape;
        let mut rng = SpikeRng::new(b"logup points");
        let ch: Vec<Fv> = vec![rng.scalar(), rng.scalar()];
        let mut w = wit.w.clone();
        let rel = c.instantiate(&shape, &ch, Some(&mut w));
        let z = crate::r1cs::z_vector(&shape, &inst, &Witness { w: w.clone() });
        let (az, bz, cz) = rel.abc(&shape, &z);
        for x in 0..1 << shape.sx {
            assert_eq!(
                az[x] * bz[x],
                cz[x],
                "row {x} holds (lookup rows, table rows, the sum row)"
            );
        }
        let entries = c.entries(&ch);
        let rx: Vec<Fv> = (0..shape.sx).map(|_| rng.scalar()).collect();
        let ry: Vec<Fv> = (0..shape.sz()).map(|_| rng.scalar()).collect();
        let (ex, ey) = (eq_table(&rx), eq_table(&ry));
        let mut evals = [Fv::ZERO; 3];
        let mut m = vec![Fv::ZERO; 1 << shape.sz()];
        let (ra, rb, rcc) = (rng.scalar::<Fv>(), rng.scalar::<Fv>(), rng.scalar::<Fv>());
        for &(mat, x, y, v) in &entries {
            evals[mat] += v * ex[x] * ey[y];
            m[y] += ex[x] * v * [ra, rb, rcc][mat];
        }
        assert_eq!(
            rel.matrix_evals(&shape, &rx, &ry),
            (evals[0], evals[1], evals[2])
        );
        assert_eq!(rel.m_table(&shape, &ex, ra, rb, rcc), m);
        // Proved and verified.
        let gens = crate::ipa::Gens::<Vesta>::new(shape.cols());
        let mut prng = SpikeRng::new(b"logup prove");
        let (proof, _) = crate::spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut prng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_ok(),
            "honest"
        );
        // A looked-up value outside its table (2048) is refused.
        let mut bad = wit.w.clone();
        bad[c.cell_index(0, 0, 5)] = Fv::from(2048u64);
        let (proof, _) =
            crate::spartan::prove_circuit(&shape, &gens, &inst, &Witness { w: bad }, &mut prng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_err(),
            "out-of-table refused"
        );
        // A pair that satisfies its row (s * s = s2) but lies outside the 16-entry
        // table (s = 16) is refused by the pair lookup alone.
        let mut bad = wit.w.clone();
        bad[c.cell_index(0, 1, 6)] = Fv::from(16u64);
        bad[c.cell_index(0, 2, 6)] = Fv::from(256u64);
        let (proof, _) =
            crate::spartan::prove_circuit(&shape, &gens, &inst, &Witness { w: bad }, &mut prng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_err(),
            "a pair outside its table is refused"
        );
    }

    /// Copies across families: `src` (family 0, p < 3) exports (x, y = x^2);
    /// the global `dst` reads five of them as (v, w) (one entry twice, one
    /// never, its sixth read off) and exports w; `back` (family 0) reads w,
    /// except at (p 1, r 0), so its read is off per instance (a mask column).
    fn copy_circuit() -> (UniformCircuit<Fv>, Instance<Fv>, Witness<Fv>) {
        let mut rng = SpikeRng::new(b"copies");
        let src_slot = |p: usize, r: usize| (p << 1) | r;
        let (mut x, mut y) = (vec![Fv::ZERO; 8], vec![Fv::ZERO; 8]);
        for p in 0..3 {
            for r in 0..2 {
                let v: Fv = rng.scalar();
                x[src_slot(p, r)] = v;
                y[src_slot(p, r)] = v * v;
            }
        }
        let reads = [
            Some(src_slot(0, 0)),
            Some(src_slot(0, 1)),
            Some(src_slot(1, 0)),
            Some(src_slot(0, 0)),
            Some(src_slot(2, 1)),
            None,
        ];
        let (mut v, mut w) = (vec![Fv::ZERO; 8], vec![Fv::ZERO; 8]);
        for (r, s) in reads.iter().enumerate() {
            let val = match s {
                Some(s) => x[*s],
                None => rng.scalar(),
            };
            v[r] = val;
            w[r] = val * val;
        }
        let back_src = |p: usize, r: usize| 2 * p + r;
        let mut u = vec![Fv::ZERO; 8];
        for p in 0..3 {
            for r in 0..2 {
                u[src_slot(p, r)] = w[back_src(p, r)];
            }
        }
        let arrays = vec![
            Array {
                name: "src",
                family: 0,
                rho: 1,
                r_count: 2,
                fixed: false,
                lookups: Vec::new(),
                widths: vec![255, 255],
                rows: vec![[own(0, 2), own(0, 2), own(1, 2)]],
                values: vec![x, y],
            },
            Array {
                name: "dst",
                family: 1,
                rho: 3,
                r_count: 6,
                fixed: false,
                lookups: Vec::new(),
                widths: vec![255, 255],
                rows: vec![[own(0, 8), own(0, 8), own(1, 8)]],
                values: vec![v, w],
            },
            Array {
                name: "back",
                family: 0,
                rho: 1,
                r_count: 2,
                fixed: false,
                lookups: Vec::new(),
                widths: vec![255],
                rows: Vec::new(),
                values: vec![u],
            },
        ];
        let mut a_from = vec![None; 8];
        for (r, s) in reads.iter().enumerate() {
            a_from[r] = s.map(|s| (0, s));
        }
        let mut b_from = vec![None; 8];
        for p in 0..3 {
            for r in 0..2 {
                if (p, r) != (1, 0) {
                    b_from[src_slot(p, r)] = Some((1, back_src(p, r)));
                }
            }
        }
        let copies = Copies {
            exports: vec![
                Export {
                    array: 0,
                    tuple: vec![own(0, 2), own(1, 2)],
                },
                Export {
                    array: 1,
                    tuple: vec![own(1, 8)],
                },
            ],
            imports: vec![
                Import {
                    array: 1,
                    tuple: vec![own(0, 8), own(1, 8)],
                    from: a_from,
                },
                Import {
                    array: 2,
                    tuple: vec![own(0, 2)],
                    from: b_from,
                },
            ],
            ..Default::default()
        };
        UniformCircuit::build_with_copies(
            "copy-test",
            arrays,
            vec![2, 0],
            vec![3, 1],
            Vec::new(),
            copies,
            10,
            4,
            b"copies",
        )
    }

    #[test]
    fn copies_match_expansion_prove_and_refuse() {
        let (c, inst, wit) = copy_circuit();
        let shape = c.shape;
        assert_eq!(c.copy_counts(), (6 + 6, 5 + 5), "entries and reads");
        let mut rng = SpikeRng::new(b"copy points");
        let ch: Vec<Fv> = vec![rng.scalar(), rng.scalar()];
        let mut w = wit.w.clone();
        let rel = c.instantiate(&shape, &ch, Some(&mut w));
        let z = crate::r1cs::z_vector(&shape, &inst, &Witness { w: w.clone() });
        let (az, bz, cz) = rel.abc(&shape, &z);
        for x in 0..1 << shape.sx {
            assert_eq!(
                az[x] * bz[x],
                cz[x],
                "row {x} holds (reads, entries, the sum row)"
            );
        }
        let entries = c.entries(&ch);
        let rx: Vec<Fv> = (0..shape.sx).map(|_| rng.scalar()).collect();
        let ry: Vec<Fv> = (0..shape.sz()).map(|_| rng.scalar()).collect();
        let (ex, ey) = (eq_table(&rx), eq_table(&ry));
        let mut evals = [Fv::ZERO; 3];
        let mut m = vec![Fv::ZERO; 1 << shape.sz()];
        let (ra, rb, rcc) = (rng.scalar::<Fv>(), rng.scalar::<Fv>(), rng.scalar::<Fv>());
        for &(mat, x, y, v) in &entries {
            evals[mat] += v * ex[x] * ey[y];
            m[y] += ex[x] * v * [ra, rb, rcc][mat];
        }
        assert_eq!(
            rel.matrix_evals(&shape, &rx, &ry),
            (evals[0], evals[1], evals[2]),
            "compiled evaluation"
        );
        assert_eq!(rel.m_table(&shape, &ex, ra, rb, rcc), m, "M(r_x, .)");
        // The register form over lanes: 11 points (partial groups) give the
        // per-point values for every lane width.
        let pts: Vec<(Vec<Fv>, Vec<Fv>, Vec<Fv>)> = (0..11)
            .map(|_| {
                (
                    (0..2).map(|_| rng.scalar()).collect(),
                    (0..shape.sx).map(|_| rng.scalar()).collect(),
                    (0..shape.sz()).map(|_| rng.scalar()).collect(),
                )
            })
            .collect();
        let one: Vec<(Fv, Fv, Fv)> = pts.iter().map(|(ch, rx, ry)| c.evals(rx, ry, ch)).collect();
        let refs: Vec<(&[Fv], &[Fv], &[Fv])> = pts
            .iter()
            .map(|(ch, rx, ry)| (&ch[..], &rx[..], &ry[..]))
            .collect();
        assert_eq!(c.evals_many(&refs), one, "evals_many");
        let inputs: Vec<Vec<Fv>> = pts
            .iter()
            .map(|(ch, rx, ry)| rx.iter().chain(ry).chain(&ch[..2]).copied().collect())
            .collect();
        let tape = c.plan();
        assert_eq!(
            inputs
                .chunks(4)
                .flat_map(|g| tape.run_lanes::<4>(g))
                .collect::<Vec<_>>(),
            one,
            "4 lanes"
        );
        assert_eq!(
            inputs
                .chunks(8)
                .flat_map(|g| tape.run_lanes::<8>(g))
                .collect::<Vec<_>>(),
            one,
            "8 lanes"
        );
        assert_eq!(
            inputs
                .chunks(16)
                .flat_map(|g| tape.run_lanes::<16>(g))
                .collect::<Vec<_>>(),
            one,
            "16 lanes"
        );
        let gens = crate::ipa::Gens::<Vesta>::new(shape.cols());
        let mut prng = SpikeRng::new(b"copy prove");
        let (proof, _) = crate::spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut prng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_ok(),
            "honest"
        );
        let verdict = |bad: Vec<Fv>| -> bool {
            let mut prng = SpikeRng::new(b"copy tamper");
            let (proof, _) = crate::spartan::prove_circuit(
                &shape,
                &gens,
                &inst,
                &Witness { w: bad },
                &mut prng,
                &c,
            );
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_ok()
        };
        let val = |a: usize, k: usize, i: usize| wit.w[c.cell_index(a, k, i)];
        // A read changed with its own row kept (v + 1, (v + 1)^2).
        let mut bad = wit.w.clone();
        let nv = val(1, 0, 2) + Fv::ONE;
        bad[c.cell_index(1, 0, 2)] = nv;
        bad[c.cell_index(1, 1, 2)] = nv * nv;
        assert!(!verdict(bad), "changed read refused");
        // An entry changed with its own row kept: its reader holds the old value.
        let mut bad = wit.w.clone();
        let nx = val(0, 0, 2) + Fv::ONE;
        bad[c.cell_index(0, 0, 2)] = nx;
        bad[c.cell_index(0, 1, 2)] = nx * nx;
        assert!(!verdict(bad), "changed entry refused");
        // A multiplicity: the entry read twice claimed once.
        let mut bad = wit.w.clone();
        assert_eq!(
            val(c.export_array(0), 0, 0),
            Fv::from(2u64),
            "entry (0, 0) is read twice"
        );
        bad[c.cell_index(c.export_array(0), 0, 0)] = Fv::ONE;
        assert!(!verdict(bad), "changed multiplicity refused");
        // An index column cell (a read pointed at another entry): the key
        // commits the constant rows.
        let mut bad = wit.w.clone();
        let idx = c.index_array(1).expect("dst has index columns");
        bad[c.cell_index(idx, 1, 1)] += Fv::ONE;
        assert!(!verdict(bad), "changed index column refused");
        // Two reads swapped: each value is in the table, at the other address.
        let mut bad = wit.w.clone();
        bad[c.cell_index(2, 0, 0)] = val(2, 0, 1);
        bad[c.cell_index(2, 0, 1)] = val(2, 0, 0);
        assert!(!verdict(bad), "swapped reads refused");
        // dst's sixth read is off: its (v, w) may change, as long as back's
        // read of that w follows.
        let mut bad = wit.w.clone();
        let nv = val(1, 0, 5) + Fv::ONE;
        bad[c.cell_index(1, 0, 5)] = nv;
        bad[c.cell_index(1, 1, 5)] = nv * nv;
        bad[c.cell_index(2, 0, (2 << 1) | 1)] = nv * nv;
        assert!(verdict(bad), "a read that is off does not constrain");
        // back's read at (p 1, r 0) is off by its mask column: its cell may
        // change, but the mask column is constant (in the key).
        let mut bad = wit.w.clone();
        bad[c.cell_index(2, 0, 1 << 1)] += Fv::ONE;
        assert!(verdict(bad), "a read off by its mask does not constrain");
        let bidx = c.index_array(2).expect("back has index columns");
        let mut bad = wit.w.clone();
        assert_eq!(val(bidx, 1, 1 << 1), Fv::ZERO, "mask 0 where off");
        bad[c.cell_index(bidx, 1, 1 << 1)] = Fv::ONE;
        assert!(!verdict(bad), "a changed mask column is refused");
        let mut bad = wit.w.clone();
        bad[c.cell_index(2, 0, (2 << 1) | 1)] += Fv::ONE;
        assert!(!verdict(bad), "a masked-on read still binds");
        // The sparse form wires the copies directly.
        let (g, gshape, ginst, gwit) =
            crate::sparse::General::build(c.to_assembly(), "copy-sparse", 10, 4, b"copy sparse");
        let ggens = crate::ipa::Gens::<Vesta>::new(gshape.cols());
        let (proof, _) =
            crate::spartan::prove_circuit(&gshape, &ggens, &ginst, &gwit, &mut prng, &g);
        assert!(
            crate::spartan::verify_circuit(&gshape, &ggens, &ginst, &proof.to_bytes(), &g).is_ok(),
            "sparse form"
        );
    }

    /// Register reuse must retain inputs and outputs, including aliases,
    /// while releasing dead inputs and last-use operands exactly once.
    #[test]
    fn batched_register_tapes_match_scalar_on_both_fields() {
        fn check<F: Scalar>() {
            let mut rng = SpikeRng::new(b"register lifetime parity");
            for inputs in [0, 1, 7] {
                for seed in 0..24 {
                    let pool: Vec<F> = (0..3).map(|_| rng.scalar()).collect();
                    let mut ops = vec![Op::Const(0)];
                    for i in 1..64 {
                        let slots = inputs + ops.len();
                        let a = ((seed + 7 * i) % slots) as Slot;
                        let b = if i % 3 == 0 {
                            a
                        } else {
                            ((3 * seed + i) % slots) as Slot
                        };
                        ops.push(match i % 5 {
                            0 => Op::Add(a, b),
                            1 => Op::Sub(a, b),
                            2 => Op::Mul(a, b),
                            3 => Op::Scale(a, (i % pool.len()) as u32),
                            _ => Op::Const((i % pool.len()) as u32),
                        });
                    }
                    let last = (inputs + ops.len() - 1) as Slot;
                    let out = if seed % 2 == 0 {
                        [0, last, last]
                    } else {
                        [last, last - 1, last - 2]
                    };
                    let regs = Regs::of(inputs, &ops, out);
                    let tape = Tape {
                        inputs,
                        ops,
                        pool,
                        out,
                        muls: 0,
                        scales: 0,
                        regs,
                    };
                    for count in [0, 1, 3, 4, 5, 8, 9, 16, 17] {
                        let points: Vec<Vec<F>> = (0..count)
                            .map(|_| (0..inputs).map(|_| rng.scalar()).collect())
                            .collect();
                        let expected: Vec<_> = points.iter().map(|p| tape.run(p)).collect();
                        assert_eq!(
                            tape.run_many(&points),
                            expected,
                            "inputs={inputs}, seed={seed}, count={count}"
                        );
                        assert_eq!(
                            points
                                .chunks(4)
                                .flat_map(|p| tape.run_lanes::<4>(p))
                                .collect::<Vec<_>>(),
                            expected
                        );
                        assert_eq!(
                            points
                                .chunks(8)
                                .flat_map(|p| tape.run_lanes::<8>(p))
                                .collect::<Vec<_>>(),
                            expected
                        );
                        assert_eq!(
                            points
                                .chunks(16)
                                .flat_map(|p| tape.run_lanes::<16>(p))
                                .collect::<Vec<_>>(),
                            expected
                        );
                    }
                }
            }
        }
        check::<iroha_pasta::Fp>();
        check::<iroha_pasta::Fq>();
    }

    /// Proved and verified; a changed round cell or a changed link is refused.
    #[test]
    fn uniform_permutations_prove_and_refuse() {
        let (c, inst, wit) = small();
        let shape = c.shape;
        let gens = crate::ipa::Gens::<Vesta>::new(shape.cols());
        let mut rng = SpikeRng::new(b"uniform-prove");
        let (proof, _) = crate::spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut rng, &c);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c).is_ok(),
            "honest"
        );
        for (array, cell, instance) in [(RP_PART7, 5, 9), (RP_PART7, 22, 3), (RP_FULL, 8, 30)] {
            let mut bad = wit.w.clone();
            bad[c.blocks[array][cell].index(instance)] += Fv::ONE;
            let (proof, _) = crate::spartan::prove_circuit(
                &shape,
                &gens,
                &inst,
                &Witness { w: bad },
                &mut rng,
                &c,
            );
            assert!(
                crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &c)
                    .is_err(),
                "{} cell {cell} instance {instance} refused",
                c.arrays[array].name
            );
        }
    }
}
