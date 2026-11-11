//! General circuits (the real Receive circuit, stage A): a sparse R1CS
//! assembled from the census gadgets, proved by the spike's prover with a
//! LogUp lookup argument in a second witness phase.
//!
//! - `Assembly` collects constraints `A·B = C` over variables (0 is the
//!   constant 1), each variable's value and bit width, lookups (a table and a
//!   tuple of linear combinations) and the tables themselves.
//! - `General::build` adds the multiplicities and two masking gates to the
//!   first phase, places the first-phase variables into Hyrax rows sorted by
//!   width (so each row commits at its real bound), reserves whole rows for
//!   the second phase, and picks the shape.
//! - After the first phase's rows are committed, the challenges (alpha, beta)
//!   instantiate the relation: each lookup adds `h·(alpha − f) = 1`, each table
//!   entry `g·(alpha − t) = m`, and one row checks `sum h = sum g`, where `f`
//!   and `t` compress (table id, tuple) with powers of beta. The prover fills
//!   the helpers `h`, `g` into the second-phase rows.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use ff::{Field, PrimeField};
use rayon::prelude::*;

use crate::fs::Scalar;
use crate::grid::{eq_table, pow3};
use crate::r1cs::{Circuit, Instance, MASK_COLS, Rel, Shape, Witness};

pub type Term<F> = (u32, F);
pub type SLc<F> = Vec<Term<F>>;

/// Seconds the last `instantiate` spent on the LogUp helpers and on the sparse
/// matrices (one-time preprocessing in a deployed prover), read by the
/// prover's and verifier's timings.
pub static LAST_BUILD: Mutex<(f64, f64)> = Mutex::new((0.0, 0.0));

#[derive(Default)]
pub struct Assembly<F> {
    pub vals: Vec<F>,
    pub widths: Vec<u16>,
    pub rows: Vec<(SLc<F>, SLc<F>, SLc<F>)>,
    pub lookups: Vec<(u32, Vec<SLc<F>>)>,
    pub tables: Vec<Vec<Vec<F>>>,
    pub table_names: Vec<String>,
    /// Rows the census counts but does not synthesize (comparisons with the
    /// modulus): emitted as satisfied filler rows of the same count.
    pub filler: usize,
    /// Gadget scope of each row and lookup (`blocks`; empty unless the block
    /// census recorded scopes).
    pub row_scope: Vec<u32>,
    pub lookup_scope: Vec<u32>,
}

impl<F: PrimeField> Assembly<F> {
    pub fn new() -> Self {
        Assembly {
            vals: vec![F::ONE],
            widths: vec![0],
            ..Default::default()
        }
    }
    pub fn var(&mut self, v: F, width: u16) -> u32 {
        self.vals.push(v);
        self.widths.push(width);
        (self.vals.len() - 1) as u32
    }
    pub fn eval(&self, lc: &SLc<F>) -> F {
        lc.iter().map(|(v, c)| self.vals[*v as usize] * c).sum()
    }
    pub fn row(&mut self, a: SLc<F>, b: SLc<F>, c: SLc<F>) {
        debug_assert!(
            self.eval(&a) * self.eval(&b) == self.eval(&c),
            "unsatisfied row"
        );
        self.rows.push((a, b, c));
    }
    /// A table by name (created on first use).
    pub fn table(&mut self, name: &str, entries: impl FnOnce() -> Vec<Vec<F>>) -> u32 {
        if let Some(i) = self.table_names.iter().position(|n| n == name) {
            return i as u32;
        }
        self.table_names.push(name.to_string());
        self.tables.push(entries());
        (self.tables.len() - 1) as u32
    }
    pub fn lookup(&mut self, table: u32, tuple: Vec<SLc<F>>) {
        self.lookups.push((table, tuple));
    }
    /// Appends another builder's variables (its variable 0 is the constant)
    /// and returns the index map.
    pub fn import_vars(&mut self, vals: &[F], widths: &[u16]) -> Vec<u32> {
        let mut map = vec![0u32; vals.len()];
        for i in 1..vals.len() {
            map[i] = self.var(vals[i], widths[i]);
        }
        map
    }
    pub fn constraints(&self) -> usize {
        self.rows.len()
            + self.filler
            + self.lookups.len()
            + self.tables.iter().map(|t| t.len()).sum::<usize>()
            + 1
            + MASK_COLS
    }
}

pub fn remap<F: Copy>(lc: &[(usize, F)], map: &[u32]) -> SLc<F> {
    lc.iter().map(|(v, c)| (map[*v], *c)).collect()
}

/// One sparse matrix in both orientations.
struct Mat<F> {
    row_ptr: Vec<u32>,
    row_cols: Vec<u32>,
    row_vals: Vec<F>,
    col_ptr: Vec<u32>,
    col_rows: Vec<u32>,
    col_vals: Vec<F>,
}

impl<F: Field> Mat<F> {
    fn new(n_rows: usize, n_cols: usize, mut t: Vec<(u32, u32, F)>) -> Self {
        t.par_sort_unstable_by_key(|(r, c, _)| (*r, *c));
        let mut row_ptr = vec![0u32; n_rows + 1];
        for (r, _, _) in &t {
            row_ptr[*r as usize + 1] += 1;
        }
        for i in 0..n_rows {
            row_ptr[i + 1] += row_ptr[i];
        }
        let row_cols = t.iter().map(|(_, c, _)| *c).collect();
        let row_vals: Vec<F> = t.iter().map(|(_, _, v)| *v).collect();
        let mut tc: Vec<(u32, u32, F)> = t;
        tc.par_sort_unstable_by_key(|(r, c, _)| (*c, *r));
        let mut col_ptr = vec![0u32; n_cols + 1];
        for (_, c, _) in &tc {
            col_ptr[*c as usize + 1] += 1;
        }
        for i in 0..n_cols {
            col_ptr[i + 1] += col_ptr[i];
        }
        let col_rows = tc.iter().map(|(r, _, _)| *r).collect();
        let col_vals = tc.iter().map(|(_, _, v)| *v).collect();
        Mat {
            row_ptr,
            row_cols,
            row_vals,
            col_ptr,
            col_rows,
            col_vals,
        }
    }
    fn nnz(&self) -> usize {
        self.row_vals.len()
    }
    fn mul_row(&self, r: usize, z: &[F]) -> F {
        let (s, e) = (self.row_ptr[r] as usize, self.row_ptr[r + 1] as usize);
        (s..e)
            .map(|i| self.row_vals[i] * z[self.row_cols[i] as usize])
            .sum()
    }
    fn col_dot(&self, c: usize, ex: &[F]) -> F {
        let (s, e) = (self.col_ptr[c] as usize, self.col_ptr[c + 1] as usize);
        (s..e)
            .map(|i| self.col_vals[i] * ex[self.col_rows[i] as usize])
            .sum()
    }
    fn eval(&self, ex: &[F], ey: &[F]) -> F {
        (0..self.row_ptr.len() - 1)
            .into_par_iter()
            .map(|r| {
                let (s, e) = (self.row_ptr[r] as usize, self.row_ptr[r + 1] as usize);
                ex[r]
                    * (s..e)
                        .map(|i| self.row_vals[i] * ey[self.row_cols[i] as usize])
                        .sum::<F>()
            })
            .sum()
    }
}

pub struct Sparse<F> {
    a: Mat<F>,
    b: Mat<F>,
    c: Mat<F>,
}

impl<F: Scalar> Rel<F> for Sparse<F> {
    fn abc(&self, shape: &Shape, z: &[F]) -> (Vec<F>, Vec<F>, Vec<F>) {
        let m = 1 << shape.sx;
        let one = |mat: &Mat<F>| -> Vec<F> {
            (0..m).into_par_iter().map(|r| mat.mul_row(r, z)).collect()
        };
        (one(&self.a), one(&self.b), one(&self.c))
    }
    fn m_table(&self, shape: &Shape, ex: &[F], ra: F, rb: F, rc: F) -> Vec<F> {
        let n = 1 << shape.sz();
        (0..n)
            .into_par_iter()
            .map(|c| {
                ra * self.a.col_dot(c, ex) + rb * self.b.col_dot(c, ex) + rc * self.c.col_dot(c, ex)
            })
            .collect()
    }
    fn matrix_evals(&self, _shape: &Shape, rx: &[F], ry: &[F]) -> (F, F, F) {
        let ex = eq_table(rx);
        let ey = eq_table(ry);
        (
            self.a.eval(&ex, &ey),
            self.b.eval(&ex, &ey),
            self.c.eval(&ex, &ey),
        )
    }
    fn nnz(&self) -> [usize; 3] {
        [self.a.nnz(), self.b.nnz(), self.c.nnz()]
    }
}

/// Montgomery batch inversion in parallel chunks (one inversion per chunk).
fn batch_invert<F: Field>(v: &mut [F]) {
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

/// A general circuit ready to prove.
pub struct General<F> {
    pub asm: Assembly<F>,
    /// Multiplicity variable of each table entry.
    pub mult: Vec<Vec<u32>>,
    /// w position of each variable (index 0 unused: the constant is io[0]).
    pub pos: Vec<u32>,
    /// w positions of the second-phase helpers: lookups, then table entries.
    pub helpers: Vec<u32>,
    pub p1_rows: usize,
    pub bounds: Vec<usize>,
    pub mask: [(u32, u32, u32); MASK_COLS],
    pub filler_vars: Vec<u32>,
}

/// Places `count` items into rows of `active` usable columns from `first_row`.
fn place(first_row: usize, count: usize, cols: usize, active: usize) -> Vec<u32> {
    (0..count)
        .map(|i| ((first_row + i / active) * cols + i % active) as u32)
        .collect()
}

impl<F: Scalar> General<F> {
    /// Adds multiplicities and masking gates, lays out the witness by width
    /// class, and picks the shape for `col_bits`-column rows.
    pub fn build(
        mut asm: Assembly<F>,
        label: &'static str,
        col_bits: usize,
        io_len: usize,
        seed: &[u8],
    ) -> (Self, Shape, Instance<F>, Witness<F>) {
        let mut rng = crate::fs::SpikeRng::new(seed);
        // Declared widths must hold: a value that does not fit its width (for
        // example a negative integer, a full field element) commits at full width.
        let mut bumped = 0usize;
        for v in 1..asm.vals.len() {
            let w = asm.widths[v] as usize;
            if w < 255 {
                let repr = asm.vals[v].to_repr();
                let top = (0..256)
                    .rev()
                    .find(|i| (repr[i / 8] >> (i % 8)) & 1 == 1)
                    .map_or(0, |i| i + 1);
                if top > w {
                    asm.widths[v] = 255;
                    bumped += 1;
                }
            }
        }
        if bumped > 0 {
            eprintln!(
                "layout: {bumped} variables exceed their declared width; committed at full width"
            );
        }
        // Multiplicities (first phase, committed before alpha and beta).
        let mut counts: Vec<Vec<u64>> = asm.tables.iter().map(|t| vec![0; t.len()]).collect();
        let index: Vec<HashMap<Vec<[u8; 32]>, usize>> = asm
            .tables
            .iter()
            .map(|t| {
                t.iter()
                    .enumerate()
                    .map(|(i, e)| (e.iter().map(|x| x.to_repr()).collect(), i))
                    .collect()
            })
            .collect();
        for (tid, tuple) in &asm.lookups {
            let key: Vec<[u8; 32]> = tuple.iter().map(|lc| asm.eval(lc).to_repr()).collect();
            let i = *index[*tid as usize]
                .get(&key)
                .expect("looked-up tuple is in its table");
            counts[*tid as usize][i] += 1;
        }
        let mult: Vec<Vec<u32>> = counts
            .iter()
            .map(|cs| {
                cs.iter()
                    .map(|c| asm.var(F::from(*c), (64 - c.leading_zeros()).max(1) as u16))
                    .collect()
            })
            .collect();
        // Masking gates: two random full-width products (hide v_A, v_B, v_C).
        let mask: [(u32, u32, u32); MASK_COLS] = std::array::from_fn(|_| {
            let (x, y): (F, F) = (rng.scalar(), rng.scalar());
            let a = asm.var(x, 255);
            let b = asm.var(y, 255);
            let c = asm.var(x * y, 255);
            (a, b, c)
        });
        // Filler rows stand for counted constraints (transcript permutations
        // counted rather than synthesized, the matrix-MLE budget): each gets a
        // fresh full-width cell, as the census counts them (x * 1 = x).
        let filler_vars: Vec<u32> = (0..asm.filler)
            .map(|_| asm.var(rng.scalar(), 255))
            .collect();
        // Shape: iterate on the tail (it depends on sx and sz).
        let n1 = asm.vals.len() - 1;
        let n2 = asm.lookups.len() + asm.tables.iter().map(|t| t.len()).sum::<usize>();
        let cons = asm.constraints();
        let mut sx = cons.next_power_of_two().trailing_zeros() as usize;
        let shape = loop {
            let mut row_bits = 2;
            let shape = loop {
                let s = Shape {
                    label,
                    sx,
                    row_bits,
                    col_bits,
                    k: 3,
                };
                let active = s.cols() - s.tail_len();
                if n1.div_ceil(active) + n2.div_ceil(active) <= s.rows() {
                    break s;
                }
                row_bits += 1;
            };
            if shape.sw() <= sx {
                break shape;
            }
            sx = shape.sw();
        };
        let (cols, active) = (shape.cols(), shape.active_cols());
        // First phase sorted by width, so rows are homogeneous.
        let mut order: Vec<u32> = (1..=n1 as u32).collect();
        order.sort_by_key(|v| asm.widths[*v as usize]);
        let slots = place(0, n1, cols, active);
        let mut pos = vec![0u32; asm.vals.len()];
        for (v, p) in order.iter().zip(&slots) {
            pos[*v as usize] = *p;
        }
        let p1_rows = n1.div_ceil(active);
        let helpers = place(p1_rows, n2, cols, active);
        let mut bounds = vec![1usize; shape.rows()];
        for v in 1..asm.vals.len() {
            let r = pos[v] as usize / cols;
            bounds[r] = bounds[r].max(asm.widths[v] as usize);
        }
        for h in &helpers {
            bounds[*h as usize / cols] = 255;
        }
        let mut w = vec![F::ZERO; shape.rows() * cols];
        for v in 1..asm.vals.len() {
            w[pos[v] as usize] = asm.vals[v];
        }
        let mut io = vec![F::ONE];
        while io.len() < io_len {
            io.push(rng.below(64));
        }
        let general = General {
            asm,
            mult,
            pos,
            helpers,
            p1_rows,
            bounds,
            mask,
            filler_vars,
        };
        (general, shape, Instance { io }, Witness { w })
    }

    fn zcol(&self, v: u32, shape: &Shape) -> u32 {
        if v == 0 {
            (1u32) << shape.sw()
        } else {
            self.pos[v as usize]
        }
    }

    fn compress(&self, tid: u32, tuple: &[F], beta: F) -> F {
        let mut acc = F::from(tid as u64 + 1);
        let mut power = beta;
        for x in tuple {
            acc += power * x;
            power *= beta;
        }
        acc
    }

    pub fn phase2_count(&self) -> usize {
        self.helpers.len()
    }
}

impl<F: Scalar> Circuit<F> for General<F> {
    fn phase1_rows(&self, _shape: &Shape) -> usize {
        self.p1_rows
    }
    fn phase2_challenges(&self) -> usize {
        2
    }
    fn row_bounds(&self, _shape: &Shape) -> Option<Vec<usize>> {
        Some(self.bounds.clone())
    }
    fn instantiate<'a>(
        &'a self,
        shape: &Shape,
        ch: &[F],
        w: Option<&mut [F]>,
    ) -> Box<dyn Rel<F> + 'a> {
        let clock = Instant::now();
        let mut helpers_s = 0.0;
        let (alpha, beta) = (ch[0], ch[1]);
        let asm = &self.asm;
        let n_rows = 1usize << shape.sx;
        let n_cols = 1usize << shape.sz();
        let one = (1u32) << shape.sw();
        let mut ta: Vec<(u32, u32, F)> = Vec::new();
        let mut tb: Vec<(u32, u32, F)> = Vec::new();
        let mut tc: Vec<(u32, u32, F)> = Vec::new();
        let mut r = 0u32;
        let push = |t: &mut Vec<(u32, u32, F)>, r: u32, lc: &SLc<F>, zc: &dyn Fn(u32) -> u32| {
            for (v, c) in lc {
                t.push((r, zc(*v), *c));
            }
        };
        let zc = |v: u32| self.zcol(v, shape);
        for (a, b, c) in &asm.rows {
            push(&mut ta, r, a, &zc);
            push(&mut tb, r, b, &zc);
            push(&mut tc, r, c, &zc);
            r += 1;
        }
        for i in 0..asm.filler {
            let v = self.filler_vars[i % self.filler_vars.len().max(1)];
            ta.push((r, zc(v), F::ONE));
            tb.push((r, one, F::ONE));
            tc.push((r, zc(v), F::ONE));
            r += 1;
        }
        for (a, b, c) in &self.mask {
            ta.push((r, zc(*a), F::ONE));
            tb.push((r, zc(*b), F::ONE));
            tc.push((r, zc(*c), F::ONE));
            r += 1;
        }
        // Helper values (prover): h = 1/(alpha - f), g = m/(alpha - t).
        if let Some(w) = w {
            let hclock = Instant::now();
            let mut hs: Vec<F> = asm
                .lookups
                .par_iter()
                .map(|(tid, tuple)| {
                    let vals: Vec<F> = tuple.iter().map(|lc| asm.eval(lc)).collect();
                    alpha - self.compress(*tid, &vals, beta)
                })
                .collect();
            let entries: Vec<(u32, usize)> = asm
                .tables
                .iter()
                .enumerate()
                .flat_map(|(tid, t)| (0..t.len()).map(move |j| (tid as u32, j)))
                .collect();
            let mut gs: Vec<F> = entries
                .par_iter()
                .map(|(tid, j)| alpha - self.compress(*tid, &asm.tables[*tid as usize][*j], beta))
                .collect();
            batch_invert(&mut hs);
            batch_invert(&mut gs);
            gs.par_iter_mut()
                .zip(&entries)
                .for_each(|(g, (tid, j))| *g *= asm.vals[self.mult[*tid as usize][*j] as usize]);
            for (p, v) in self.helpers.iter().zip(hs.iter().chain(&gs)) {
                w[*p as usize] = *v;
            }
            helpers_s = hclock.elapsed().as_secs_f64();
        }
        // Lookup rows: h (alpha - f) = 1.
        let mut sum_row: SLc<F> = Vec::new();
        for (i, (tid, tuple)) in asm.lookups.iter().enumerate() {
            let h = self.helpers[i];
            ta.push((r, h, F::ONE));
            tb.push((r, one, alpha - F::from(*tid as u64 + 1)));
            let mut power = beta;
            for lc in tuple {
                for (v, c) in lc {
                    tb.push((r, zc(*v), -(power * c)));
                }
                power *= beta;
            }
            tc.push((r, one, F::ONE));
            sum_row.push((h, F::ONE));
            r += 1;
        }
        // Table rows: g (alpha - t) = m.
        let mut k = asm.lookups.len();
        for (tid, table) in asm.tables.iter().enumerate() {
            for (j, entry) in table.iter().enumerate() {
                let g = self.helpers[k];
                ta.push((r, g, F::ONE));
                tb.push((r, one, alpha - self.compress(tid as u32, entry, beta)));
                tc.push((r, zc(self.mult[tid][j]), F::ONE));
                sum_row.push((g, -F::ONE));
                k += 1;
                r += 1;
            }
        }
        // sum h - sum g = 0.
        for (p, c) in &sum_row {
            ta.push((r, *p, *c));
        }
        tb.push((r, one, F::ONE));
        r += 1;
        assert!(r as usize <= n_rows, "constraints fit 2^sx");
        let rel = Sparse {
            a: Mat::new(n_rows, n_cols, ta),
            b: Mat::new(n_rows, n_cols, tb),
            c: Mat::new(n_rows, n_cols, tc),
        };
        *LAST_BUILD.lock().unwrap() = (helpers_s, clock.elapsed().as_secs_f64() - helpers_s);
        Box::new(rel)
    }
}

/// Bytes the shape's proof takes, and its witness and constraint counts.
pub fn describe<F: Scalar>(g: &General<F>, shape: &Shape) -> String {
    format!(
        "2^{} constraints ({} used: {} rows, {} filler, {} lookups, {} table entries), witness {} first phase + {} helpers in {} + {} rows of {} (active {}), proof {} B",
        shape.sx,
        g.asm.constraints(),
        g.asm.rows.len(),
        g.asm.filler,
        g.asm.lookups.len(),
        g.asm.tables.iter().map(|t| t.len()).sum::<usize>(),
        g.asm.vals.len() - 1,
        g.phase2_count(),
        g.p1_rows,
        shape.rows() - g.p1_rows,
        shape.cols(),
        shape.active_cols(),
        shape.proof_bytes()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipa::Gens;
    use iroha_pasta::Eq as Vesta;

    type Fv = <Vesta as iroha_pasta::PastaCurve>::ScalarExt;

    /// A small circuit with products, a range table and a tuple table: proved
    /// and verified; a broken product or an out-of-table cell is refused.
    #[test]
    fn general_circuit_proves_and_refuses() {
        let build = |bad_row: bool| {
            let mut asm = Assembly::<Fv>::new();
            let range = asm.table("range8", || {
                (0..256u64).map(|v| vec![Fv::from(v)]).collect()
            });
            let pairs = asm.table("square4", || {
                (0..16u64)
                    .map(|v| vec![Fv::from(v), Fv::from(v * v)])
                    .collect()
            });
            for i in 0..3000u64 {
                let x = asm.var(Fv::from(i % 251), 8);
                let y = asm.var(Fv::from(i % 13), 4);
                let z = asm.var(
                    Fv::from((i % 251) * (i % 13) + u64::from(bad_row && i == 7)),
                    12,
                );
                asm.rows
                    .push((vec![(x, Fv::ONE)], vec![(y, Fv::ONE)], vec![(z, Fv::ONE)]));
                asm.lookup(range, vec![vec![(x, Fv::ONE)]]);
                let a = asm.var(Fv::from(i % 16), 4);
                let b = asm.var(Fv::from((i % 16) * (i % 16)), 8);
                asm.lookup(pairs, vec![vec![(a, Fv::ONE)], vec![(b, Fv::ONE)]]);
            }
            General::build(asm, "general-test", 13, 8, b"general")
        };
        let gens = Gens::<Vesta>::new(1 << 13);
        let mut rng = crate::fs::SpikeRng::new(b"gp");
        let (g, shape, inst, wit) = build(false);
        let (proof, _) = crate::spartan::prove_circuit(&shape, &gens, &inst, &wit, &mut rng, &g);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &g).is_ok(),
            "honest"
        );
        let (g2, shape2, inst2, wit2) = build(true);
        let (proof, _) =
            crate::spartan::prove_circuit(&shape2, &gens, &inst2, &wit2, &mut rng, &g2);
        assert!(
            crate::spartan::verify_circuit(&shape2, &gens, &inst2, &proof.to_bytes(), &g2).is_err(),
            "broken product refused"
        );
        // A cell used only in a lookup, set outside its table: the lookup row
        // and the sum cannot both hold.
        let (_, tuple) = g.asm.lookups.last().unwrap().clone();
        let a = tuple[0][0].0;
        let mut bad = wit.w.clone();
        bad[g.pos[a as usize] as usize] = Fv::from(17u64);
        let (proof, _) =
            crate::spartan::prove_circuit(&shape, &gens, &inst, &Witness { w: bad }, &mut rng, &g);
        assert!(
            crate::spartan::verify_circuit(&shape, &gens, &inst, &proof.to_bytes(), &g).is_err(),
            "out-of-table refused"
        );
    }
}
