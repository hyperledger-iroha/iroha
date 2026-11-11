//! A uniform toy R1CS whose matrix multilinear extensions the verifier
//! evaluates in O(log n), with Hyrax geometry and a reserved IPA tail.
//!
//! `z = (w || io)`, index bits most significant first: `[sel][row][col]`,
//! `sel = 0` the witness, `sel = 1` the public inputs (`io[0] = 1`). The
//! witness is split into quarters by the top two row bits; the constraint at
//! witness position `(q, rest)` (top `sx - sw` constraint bits zero) is
//!
//! - `q = 0`: `w(0,rest) * w(1,rest) = w(2,rest)` (a 64-bit times 16-bit product)
//! - `q = 1, 2`: `w(q,rest) * 1 = w(q,rest)`
//! - `q = 3`: `w(3,rest)^2 = w(3,rest)` (bits)
//!
//! and the remaining constraints are empty. The last `tail` columns of every
//! row are zero: the opening IPA carries the committed sumcheck round values
//! there. Two `q = 0` gates (row 0, last two active columns) use uniformly
//! random full-width factors: they mask the revealed `v_A, v_B, v_C`.

use ff::Field;
use rayon::prelude::*;

use crate::fs::{Scalar, SpikeRng};
use crate::grid::{eq_eval, pow4};

pub const MASK_COLS: usize = 2;
pub const NUM_PUBLIC: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub label: &'static str,
    /// log2 of the constraint count.
    pub sx: usize,
    pub row_bits: usize,
    pub col_bits: usize,
    /// Variables per committed sumcheck round.
    pub k: usize,
}

impl Shape {
    pub fn sw(&self) -> usize {
        self.row_bits + self.col_bits
    }
    pub fn sz(&self) -> usize {
        self.sw() + 1
    }
    pub fn rows(&self) -> usize {
        1 << self.row_bits
    }
    pub fn cols(&self) -> usize {
        1 << self.col_bits
    }
    fn blocks(&self, vars: usize) -> Vec<usize> {
        let mut blocks = vec![self.k; vars / self.k];
        if vars % self.k != 0 {
            blocks.push(vars % self.k);
        }
        blocks
    }
    pub fn outer_blocks(&self) -> Vec<usize> {
        self.blocks(self.sx)
    }
    pub fn inner_blocks(&self) -> Vec<usize> {
        self.blocks(self.sz())
    }
    /// IPA columns carrying the committed round values: an aligned slot of
    /// 4^k columns per round (grid::slot_positions); a shorter last round
    /// uses its slot's low digits.
    pub fn tail_len(&self) -> usize {
        (self.outer_blocks().len() + self.inner_blocks().len()) * pow4(self.k)
    }
    pub fn active_cols(&self) -> usize {
        self.cols() - self.tail_len()
    }
    /// Exact serialized proof length: rows, outer rounds, `v_A, v_B, v_C`,
    /// inner rounds, IPA `L/R` pairs, folded generator, Schnorr `A, z1, z2`.
    pub fn proof_bytes(&self) -> usize {
        32 * (self.rows()
            + self.outer_blocks().len()
            + 3
            + self.inner_blocks().len()
            + 2 * self.col_bits
            + 4)
    }
    pub fn check(&self) {
        assert!(self.row_bits >= 2, "four quarters of rows");
        assert!(self.k >= 1 && self.k <= 5);
        assert!(self.active_cols() > MASK_COLS, "tail fits the row");
    }
    pub fn quarter(&self, row: usize) -> usize {
        row >> (self.row_bits - 2)
    }
    /// Public width class of a row's bulk values.
    pub fn row_bits_class(&self, row: usize) -> usize {
        [64, 16, 80, 1][self.quarter(row)]
    }
    /// Trailing active columns committed at full width (the masking gates).
    pub fn row_full_tail(&self, row: usize) -> usize {
        if row % (self.rows() / 4) == 0 && self.quarter(row) < 3 {
            MASK_COLS
        } else {
            0
        }
    }
}

#[derive(Clone)]
pub struct Instance<F> {
    pub io: Vec<F>,
}

pub struct Witness<F> {
    /// `rows * cols`, row major, zero tail columns.
    pub w: Vec<F>,
}

/// The relation a proof is about (the toy uniform R1CS, or a sparse circuit).
/// `z = (w || io)` padded to `2^sz`.
pub trait Rel<F: Scalar>: Sync {
    fn abc(&self, shape: &Shape, z: &[F]) -> (Vec<F>, Vec<F>, Vec<F>);
    fn m_table(&self, shape: &Shape, ex: &[F], ra: F, rb: F, rc: F) -> Vec<F>;
    fn matrix_evals(&self, shape: &Shape, rx: &[F], ry: &[F]) -> (F, F, F);
    /// Nonzero entries of A, B and C (0 when the relation is structured).
    fn nnz(&self) -> [usize; 3] {
        [0; 3]
    }
}

pub struct Toy;

impl<F: Scalar> Rel<F> for Toy {
    fn abc(&self, shape: &Shape, z: &[F]) -> (Vec<F>, Vec<F>, Vec<F>) {
        abc(shape, &z[..1 << shape.sw()])
    }
    fn m_table(&self, shape: &Shape, ex: &[F], ra: F, rb: F, rc: F) -> Vec<F> {
        m_table(shape, ex, ra, rb, rc)
    }
    fn matrix_evals(&self, shape: &Shape, rx: &[F], ry: &[F]) -> (F, F, F) {
        matrix_evals(shape, rx, ry)
    }
}

/// A circuit: its relation, possibly with a second witness phase whose rows
/// and constraints depend on challenges drawn after the first phase's rows
/// are committed (the LogUp argument).
pub trait Circuit<F: Scalar>: Sync {
    fn phase1_rows(&self, shape: &Shape) -> usize {
        shape.rows()
    }
    fn phase2_challenges(&self) -> usize {
        0
    }
    /// The relation once the phase-2 challenges are known; the prover also
    /// fills the phase-2 rows of `w`.
    fn instantiate<'a>(
        &'a self,
        shape: &Shape,
        ch: &[F],
        w: Option<&mut [F]>,
    ) -> Box<dyn Rel<F> + 'a>;
    /// Per-row bit bounds for lane commits; None keeps the toy's row classes.
    fn row_bounds(&self, _shape: &Shape) -> Option<Vec<usize>> {
        None
    }
    /// Trailing Hyrax rows that hold public constant columns. Their
    /// commitments belong to the verification key (unblinded, computed once
    /// by `spartan::fixed_commitments`), so proofs omit them and the verifier
    /// substitutes the key's.
    fn fixed_rows(&self, _shape: &Shape) -> usize {
        0
    }
    /// The fixed rows' values, row major (`fixed_rows * cols`).
    fn fixed_values(&self, _shape: &Shape) -> Vec<F> {
        Vec::new()
    }
    /// Public inputs fixed after the second-phase challenges (late io): their
    /// io indices, in the order the proof carries them. The transcript
    /// absorbs their digest right after those challenges.
    fn late_slots(&self) -> Vec<usize> {
        Vec::new()
    }
    /// The late io values in a witness filled for the second phase.
    fn late_values(&self, _w: &[F]) -> Vec<F> {
        Vec::new()
    }
    /// Hyrax rows committed after the pair link's word and before the
    /// second-phase challenges (the link phase): values that follow the
    /// link's rho, such as the cross-circuit binding chain. 0 without one.
    fn link_rows(&self, _shape: &Shape) -> usize {
        0
    }
    /// The link's io: the io slots the link's rho and R fill, as this
    /// circuit reads them. Prover and verifier both set them from the
    /// transcript (rho) and the link (R), so proofs do not carry them; they
    /// lie past the header's io. The circuit pins its binding value to R
    /// there, so a proof whose chain does not end at R fails.
    fn link_io(&self, _rho: &[u8; 32], _r: &[u8; 32]) -> Vec<(usize, F)> {
        Vec::new()
    }
    /// Fills the link phase's cells of `w` once rho is known.
    fn fill_link(&self, _rho: &[u8; 32], _w: &mut [F]) {}
    /// The binding value R the circuit computed (from its filled witness):
    /// the second proof's prover takes the link's R from it. None without
    /// one.
    fn binding_r(&self, _w: &[F]) -> Option<[u8; 32]> {
        None
    }
}

/// `io` with the link's io slots set.
pub fn io_with_link<S: Scalar>(io: &[S], link: &[(usize, S)]) -> Vec<S> {
    let mut out = io.to_vec();
    for &(s, v) in link {
        if out.len() <= s {
            out.resize(s + 1, S::ZERO);
        }
        out[s] = v;
    }
    out
}

pub struct ToyCircuit;

impl<F: Scalar> Circuit<F> for ToyCircuit {
    fn instantiate<'a>(
        &'a self,
        _shape: &Shape,
        _ch: &[F],
        _w: Option<&mut [F]>,
    ) -> Box<dyn Rel<F> + 'a> {
        Box::new(Toy)
    }
}

pub fn sample<F: Scalar>(shape: &Shape, rng: &mut SpikeRng) -> (Instance<F>, Witness<F>) {
    let (rows, cols, active) = (shape.rows(), shape.cols(), shape.active_cols());
    let quarter_rows = rows / 4;
    let mut w = vec![F::ZERO; rows * cols];
    for r in 0..quarter_rows {
        for c in 0..active {
            let mask = r == 0 && c >= active - MASK_COLS;
            let a: F = if mask { rng.scalar() } else { rng.below(64) };
            let b: F = if mask { rng.scalar() } else { rng.below(16) };
            w[r * cols + c] = a;
            w[(quarter_rows + r) * cols + c] = b;
            w[(2 * quarter_rows + r) * cols + c] = a * b;
        }
    }
    for r in 3 * quarter_rows..rows {
        for c in 0..active {
            w[r * cols + c] = rng.below(1);
        }
    }
    let mut io = vec![F::ONE];
    for _ in 1..NUM_PUBLIC {
        io.push(rng.below(64));
    }
    (Instance { io }, Witness { w })
}

/// `z = (w || io)` padded to `2^sz`.
pub fn z_vector<F: Scalar>(shape: &Shape, inst: &Instance<F>, wit: &Witness<F>) -> Vec<F> {
    let half = 1 << shape.sw();
    let mut z = vec![F::ZERO; 2 * half];
    z[..half].copy_from_slice(&wit.w);
    z[half..half + inst.io.len()].copy_from_slice(&inst.io);
    z
}

/// `(Az, Bz, Cz)` over all `2^sx` constraints.
pub fn abc<F: Scalar>(shape: &Shape, w: &[F]) -> (Vec<F>, Vec<F>, Vec<F>) {
    let m = 1 << shape.sx;
    let nw = 1 << shape.sw();
    let qsz = nw / 4;
    let mut a = vec![F::ZERO; m];
    let mut b = vec![F::ZERO; m];
    let mut c = vec![F::ZERO; m];
    a[..nw]
        .par_iter_mut()
        .zip(b[..nw].par_iter_mut())
        .zip(c[..nw].par_iter_mut())
        .enumerate()
        .for_each(|(x, ((a, b), c))| {
            let (q, rest) = (x / qsz, x % qsz);
            match q {
                0 => {
                    *a = w[rest];
                    *b = w[qsz + rest];
                    *c = w[2 * qsz + rest];
                }
                1 | 2 => {
                    *a = w[x];
                    *b = F::ONE;
                    *c = w[x];
                }
                _ => {
                    *a = w[x];
                    *b = w[x];
                    *c = w[x];
                }
            }
        });
    (a, b, c)
}

/// `M(y) = sum_x eq(r_x, x) (rA A + rB B + rC C)(x, y)` for every `y`.
pub fn m_table<F: Scalar>(shape: &Shape, ex: &[F], ra: F, rb: F, rc: F) -> Vec<F> {
    let nw = 1 << shape.sw();
    let qsz = nw / 4;
    let mut m = vec![F::ZERO; 2 * nw];
    let rac = ra + rc;
    let rabc = ra + rb + rc;
    m[..nw].par_iter_mut().enumerate().for_each(|(y, value)| {
        let (q, rest) = (y / qsz, y % qsz);
        *value = match q {
            0 => ra * ex[rest],
            1 => rb * ex[rest] + rac * ex[qsz + rest],
            2 => rc * ex[rest] + rac * ex[2 * qsz + rest],
            _ => rabc * ex[3 * qsz + rest],
        };
    });
    let ones: F = ex[qsz..3 * qsz]
        .par_iter()
        .copied()
        .reduce(|| F::ZERO, |x, y| x + y);
    m[nw] = rb * ones;
    m
}

fn bit<F: Field>(r: F, b: usize) -> F {
    if b == 1 { r } else { F::ONE - r }
}

fn eq_quarter<F: Field>(r: &[F], q: usize) -> F {
    bit(r[0], (q >> 1) & 1) * bit(r[1], q & 1)
}

/// `(A~(rx, ry), B~(rx, ry), C~(rx, ry))` in O(log n).
pub fn matrix_evals<F: Scalar>(shape: &Shape, rx: &[F], ry: &[F]) -> (F, F, F) {
    let (rx_hi, rx_w) = rx.split_at(shape.sx - shape.sw());
    let (rx_q, rx_rest) = rx_w.split_at(2);
    let r_sel = ry[0];
    let ry_w = &ry[1..];
    let (ry_q, ry_rest) = ry_w.split_at(2);
    let z_hi: F = rx_hi.iter().map(|r| F::ONE - r).product();
    let e = eq_eval(rx_rest, ry_rest);
    let ws = F::ONE - r_sel;
    let io0: F = ry_w.iter().map(|r| F::ONE - r).product();
    let diag: F = (1..4)
        .map(|q| eq_quarter(rx_q, q) * eq_quarter(ry_q, q))
        .sum();
    let a = z_hi * ws * e * (eq_quarter(rx_q, 0) * eq_quarter(ry_q, 0) + diag);
    let b = z_hi
        * (ws
            * e
            * (eq_quarter(rx_q, 0) * eq_quarter(ry_q, 1)
                + eq_quarter(rx_q, 3) * eq_quarter(ry_q, 3))
            + r_sel * io0 * (eq_quarter(rx_q, 1) + eq_quarter(rx_q, 2)));
    let c = z_hi * ws * e * (eq_quarter(rx_q, 0) * eq_quarter(ry_q, 2) + diag);
    (a, b, c)
}

/// `io~(r)` over the `sw` witness-index variables.
pub fn io_eval<F: Scalar>(shape: &Shape, io: &[F], r: &[F]) -> F {
    let sw = shape.sw();
    io.iter()
        .enumerate()
        .map(|(i, value)| {
            let weight: F = (0..sw)
                .map(|j| bit(r[j], (i >> (sw - 1 - j)) & 1))
                .product();
            *value * weight
        })
        .sum()
}
