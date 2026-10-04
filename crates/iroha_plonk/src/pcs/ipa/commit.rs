//! Vector commitments and verifier-side linear combinations of points.
//!
//! A commitment to `a` of length `m <= n` is `sum_{i<m} a_i B_i + blind * W`,
//! where `B` is `g` (coefficient form, [`commit`]) or `g_lagrange`
//! (evaluation form, [`commit_lagrange`]). The vendored `Blind::default()` is
//! one, so key-generation commitments are `... + W` ([`DEFAULT_BLIND`]); this
//! keeps every commitment of an all-zero column away from the identity.
//!
//! [`Secrecy`] selects the MSM posture of the prover's commitments:
//! witness-dependent scalars use [`iroha_pasta::msm::msm_secret`] and
//! constant-time scalar multiplication of `W`; public data (fixed columns,
//! instances) uses the variable-time paths. Both return the same point.
//!
//! # The verifier MSM (S10)
//!
//! Every verifier-side combination ([`Msm::evaluate`], so the instance
//! commitments, the opening equation and the batch equation, plus the
//! folded generator, `decide` and `batch_decide`) runs [`msm_complete`]: a
//! portable Pippenger whose buckets, running sums and window combination use
//! only the complete projective formulas (complete mixed addition of the
//! affine bases, complete addition and doubling). It has no batch-affine or
//! incomplete-formula path, so prover-chosen bases (`L_j`, `R_j`, `G`,
//! commitments) never meet an exceptional case, and no failure: the memory
//! budget only changes its window and wave sizes, never its result. The
//! optimized batch-affine `iroha_pasta::msm::msm_public` stays prover-only
//! until it is audited and differentially qualified for consensus.
//!
//! The scalars are public, so [`msm_complete`] splits each one with the GLV
//! endomorphism (`s = k1 + k2 ZETA`, `|k1|, |k2| < 2^128`, so `s B = k1 B +
//! k2 phi(B)` with `phi(x, y) = (beta x, y)`) and recodes the halves into
//! signed base-`2^c` digits: half the buckets of unsigned digits, and the
//! negated affine base for a negative digit (free, and still a complete mixed
//! addition). The scalar recoding changes no formula and no result; when a
//! split were unavailable (excluded by the Pasta constants) the unsigned
//! full-width windows run instead.

use ff::{Field, PrimeField};
use group::prime::PrimeCurveAffine;
use iroha_pasta::{
    PastaAffine, PastaCurve, PastaField,
    msm::{FixedBaseTable, MemoryBudget, MsmError, msm_public, msm_secret},
    params::ParamsIpa,
};
use rayon::prelude::*;

/// The key-generation blind (`Blind::default()` in the vendored code).
pub const DEFAULT_BLIND: u64 = 1;

/// Whether the committed scalars are secret.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Secrecy {
    /// Public scalars (fixed columns, instances, verifier data).
    Public,
    /// Witness-dependent scalars.
    Secret,
}

/// `sum_i scalars[i] * bases[i] + blind * w` with `scalars.len() <= bases.len()`.
fn commit_with_bases<C: PastaCurve>(
    bases: &[C::AffineExt],
    table: Option<&FixedBaseTable<C>>,
    scalars: &[C::ScalarExt],
    blind: &C::ScalarExt,
    w: &C::AffineExt,
    secrecy: Secrecy,
    budget: MemoryBudget,
) -> Result<C, MsmError> {
    let bases = bases.get(..scalars.len()).ok_or(MsmError::LengthMismatch(
        iroha_pasta::LengthMismatch {
            left: scalars.len(),
            right: bases.len(),
        },
    ))?;
    let sum = match (table, secrecy) {
        (Some(table), Secrecy::Public) if table.len() == scalars.len() => {
            table.msm_public(scalars, budget)?
        }
        (Some(table), Secrecy::Secret) if table.len() == scalars.len() => {
            table.msm_secret(scalars, budget)?
        }
        (_, Secrecy::Public) => msm_public::<C>(scalars, bases, budget)?,
        (_, Secrecy::Secret) => msm_secret::<C>(scalars, bases, budget)?,
    };
    let blind_term = match secrecy {
        Secrecy::Public => w.to_curve().mul_vartime(blind),
        Secrecy::Secret => w.to_curve() * *blind,
    };
    Ok(sum + blind_term)
}

/// Commits to coefficients: `sum_i coeffs[i] g[i] + blind * W`.
///
/// # Errors
///
/// [`MsmError::LengthMismatch`] when `coeffs` is longer than `g`;
/// [`MsmError::Budget`] when the MSM does not fit `budget`.
pub fn commit<C: PastaCurve>(
    params: &ParamsIpa<C>,
    coeffs: &[C::ScalarExt],
    blind: &C::ScalarExt,
    secrecy: Secrecy,
    budget: MemoryBudget,
) -> Result<C, MsmError> {
    commit_with_bases(
        params.g(),
        None,
        coeffs,
        blind,
        &params.w(),
        secrecy,
        budget,
    )
}

/// Commits to evaluations: `sum_i values[i] g_lagrange[i] + blind * W`.
///
/// # Errors
///
/// As [`commit`].
pub fn commit_lagrange<C: PastaCurve>(
    params: &ParamsIpa<C>,
    values: &[C::ScalarExt],
    blind: &C::ScalarExt,
    secrecy: Secrecy,
    budget: MemoryBudget,
) -> Result<C, MsmError> {
    commit_with_bases(
        params.g_lagrange(),
        None,
        values,
        blind,
        &params.w(),
        secrecy,
        budget,
    )
}

/// Commitment-key tables (precomputed window multiples of `g` and
/// `g_lagrange`) that a proving key may own. Results equal [`commit`] and
/// [`commit_lagrange`] bit for bit.
#[derive(Clone, Debug)]
pub struct CommitmentTables<C: PastaCurve> {
    g: Option<FixedBaseTable<C>>,
    g_lagrange: Option<FixedBaseTable<C>>,
}

impl<C: PastaCurve> Default for CommitmentTables<C> {
    fn default() -> Self {
        Self::none()
    }
}

impl<C: PastaCurve> CommitmentTables<C> {
    /// No tables: every commitment uses the variable-base MSM.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            g: None,
            g_lagrange: None,
        }
    }

    /// Builds the tables that fit `budget` (`g_lagrange` first, then `g`,
    /// each charged the whole budget separately); a table that does not fit
    /// is simply absent.
    #[must_use]
    pub fn build(params: &ParamsIpa<C>, budget: MemoryBudget) -> Self {
        Self {
            g_lagrange: FixedBaseTable::new(params.g_lagrange(), budget).ok(),
            g: FixedBaseTable::new(params.g(), budget).ok(),
        }
    }

    /// Whether the `g` and `g_lagrange` tables exist.
    #[must_use]
    pub fn present(&self) -> (bool, bool) {
        (self.g.is_some(), self.g_lagrange.is_some())
    }

    /// [`commit`] through the `g` table when it covers `coeffs`.
    ///
    /// # Errors
    ///
    /// As [`commit`].
    pub fn commit(
        &self,
        params: &ParamsIpa<C>,
        coeffs: &[C::ScalarExt],
        blind: &C::ScalarExt,
        secrecy: Secrecy,
        budget: MemoryBudget,
    ) -> Result<C, MsmError> {
        commit_with_bases(
            params.g(),
            self.g.as_ref(),
            coeffs,
            blind,
            &params.w(),
            secrecy,
            budget,
        )
    }

    /// [`commit_lagrange`] through the `g_lagrange` table when it covers
    /// `values`.
    ///
    /// # Errors
    ///
    /// As [`commit`].
    pub fn commit_lagrange(
        &self,
        params: &ParamsIpa<C>,
        values: &[C::ScalarExt],
        blind: &C::ScalarExt,
        secrecy: Secrecy,
        budget: MemoryBudget,
    ) -> Result<C, MsmError> {
        commit_with_bases(
            params.g_lagrange(),
            self.g_lagrange.as_ref(),
            values,
            blind,
            &params.w(),
            secrecy,
            budget,
        )
    }
}

/// Bytes of one projective bucket (three 32-byte coordinates).
const BUCKET_BYTES: usize = 96;
/// The widest window of [`msm_complete`].
const MAX_WINDOW: usize = 16;

/// The window width of [`msm_complete`] for `n` points: about `ln n`
/// (halo2's choice), narrowed until one window's buckets fit `budget`.
fn complete_window(n: usize, budget: MemoryBudget) -> usize {
    let mut window = if n < 4 {
        1
    } else if n < 32 {
        3
    } else {
        // ceil(log2(n) * ln 2), with ln 2 ~ 0.693.
        let log2 = usize::try_from(n.ilog2()).unwrap_or(usize::MAX);
        log2.saturating_mul(693).saturating_add(999) / 1000
    };
    window = window.clamp(1, MAX_WINDOW);
    while window > 1 && (BUCKET_BYTES << window) > budget.bytes() {
        window -= 1;
    }
    window
}

/// Bits `[start, start + width)` of a little-endian 256-bit integer.
fn window_digit(limbs: &[u64; 4], start: usize, width: usize) -> usize {
    let limb = start / 64;
    let offset = start % 64;
    let Some(low) = limbs.get(limb) else {
        return 0;
    };
    let mut value = low >> offset;
    if offset != 0
        && offset + width > 64
        && let Some(high) = limbs.get(limb + 1)
    {
        value |= high << (64 - offset);
    }
    // width <= MAX_WINDOW < 64, so the mask and the cast are exact.
    usize::try_from(value & ((1_u64 << width) - 1)).unwrap_or(0)
}

/// The sum of one window: buckets by digit, then the running-sum reduction.
fn complete_window_sum<C: PastaCurve>(
    limbs: &[[u64; 4]],
    bases: &[C::AffineExt],
    start: usize,
    width: usize,
) -> C {
    let mut buckets = vec![C::identity(); (1_usize << width) - 1];
    for (scalar, base) in limbs.iter().zip(bases) {
        let digit = window_digit(scalar, start, width);
        if let Some(bucket) = digit
            .checked_sub(1)
            .and_then(|index| buckets.get_mut(index))
        {
            // Complete mixed addition: correct for equal, opposite and
            // identity inputs.
            *bucket += base;
        }
    }
    let mut running = C::identity();
    let mut sum = C::identity();
    for bucket in buckets.iter().rev() {
        running += bucket;
        sum += &running;
    }
    sum
}

/// The most bits of a GLV half (`|k1|, |k2| < 2^128`).
const GLV_HALF_BITS: usize = 128;
/// The widest signed window of the GLV path (`2^15` buckets).
const MAX_SIGNED_WINDOW: usize = 16;

/// The signed windows of a [`GLV_HALF_BITS`]-bit magnitude at width `width`:
/// one more than `ceil(128 / width)` takes the last carry.
const fn signed_windows(width: usize) -> usize {
    GLV_HALF_BITS.div_ceil(width) + 1
}

/// The signed window width of the GLV path for `terms` split terms: the
/// minimum of a cost model counting field multiplications (11 per complete
/// mixed addition into a bucket, 12 per complete addition of the running
/// sums, 8 per doubling), narrowed until one window's `2^(width - 1)`
/// buckets fit `budget`.
fn signed_window(terms: usize, budget: MemoryBudget) -> usize {
    let terms = u128::try_from(terms).unwrap_or(u128::MAX);
    let cost = |width: usize| -> u128 {
        let windows = u128::try_from(signed_windows(width)).unwrap_or(u128::MAX);
        let buckets = 1_u128 << (width - 1);
        let per_window = terms
            .saturating_mul(11)
            .saturating_add(buckets.saturating_mul(24))
            .saturating_add(u128::try_from(width).unwrap_or(0).saturating_mul(8));
        windows.saturating_mul(per_window)
    };
    let mut width = (1..=MAX_SIGNED_WINDOW)
        .min_by_key(|width| (cost(*width), *width))
        .unwrap_or(1);
    while width > 1 && (BUCKET_BYTES << (width - 1)) > budget.bytes() {
        width -= 1;
    }
    width
}

/// Recodes `value < 2^128` into `out.len()` signed digits of `width <= 16`
/// bits, least significant first: `value = sum_w out[w] 2^(width w)` with
/// every digit in `(-2^(width - 1), 2^(width - 1)]`. Returns the carry left
/// after the last digit (zero when `out` has [`signed_windows`] digits).
fn signed_digits(value: u128, width: usize, out: &mut [i32]) -> i32 {
    let full = 1_i32 << width;
    let half = 1_i32 << (width - 1);
    let mask = (1_u128 << width) - 1;
    let mut carry = 0_i32;
    for (window, digit) in out.iter_mut().enumerate() {
        let shift = window.saturating_mul(width);
        let bits = if shift < GLV_HALF_BITS {
            i32::try_from((value >> shift) & mask).unwrap_or(0)
        } else {
            0
        };
        let mut signed = bits + carry;
        carry = 0;
        if signed > half {
            signed -= full;
            carry = 1;
        }
        *digit = signed;
    }
    carry
}

/// The GLV split of every term: `2n` affine bases (`±B`, `±phi(B)` for each
/// base `B`) and their signed digits (row-major, [`signed_windows`] per
/// split base), or `None` when a split or an endomorphism image is
/// unavailable.
fn glv_split<C: PastaCurve>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    width: usize,
) -> Option<(Vec<C::AffineExt>, Vec<i32>)> {
    let windows = signed_windows(width);
    let beta = <C::AffineExt as PastaAffine>::endo_beta();
    let terms = scalars.len().min(bases.len());
    let mut split_bases = vec![C::AffineExt::identity(); terms.checked_mul(2)?];
    let mut digits = vec![0_i32; terms.checked_mul(2)?.checked_mul(windows)?];
    split_bases
        .par_chunks_mut(2)
        .zip(digits.par_chunks_mut(2 * windows))
        .zip(scalars.par_iter().zip(bases))
        .try_for_each(|((split, split_digits), (scalar, base))| {
            let halves = C::glv_decompose(scalar)?;
            // phi(x, y) = (beta x, y) = [ZETA] (x, y); the identity (0, 0)
            // maps to itself.
            let image: C::AffineExt =
                Option::from(C::AffineExt::from_xy(base.x() * beta, base.y()))?;
            split[0] = if halves.k1_neg { -*base } else { *base };
            split[1] = if halves.k2_neg { -image } else { image };
            let (first, second) = split_digits.split_at_mut(windows);
            let carry =
                signed_digits(halves.k1, width, first) | signed_digits(halves.k2, width, second);
            (carry == 0).then_some(())
        })?;
    Some((split_bases, digits))
}

/// The sum of one signed window: `2^(width - 1)` buckets by digit magnitude
/// (the negated base for a negative digit), then the running-sum reduction.
fn signed_window_sum<C: PastaCurve>(
    digits: &[i32],
    bases: &[C::AffineExt],
    window: usize,
    windows: usize,
    width: usize,
) -> C {
    let mut buckets = vec![C::identity(); 1_usize << (width - 1)];
    for (term, base) in bases.iter().enumerate() {
        let digit = digits
            .get(term.saturating_mul(windows).saturating_add(window))
            .copied()
            .unwrap_or(0);
        let Some(bucket) = usize::try_from(digit.unsigned_abs())
            .ok()
            .and_then(|magnitude| magnitude.checked_sub(1))
            .and_then(|index| buckets.get_mut(index))
        else {
            continue;
        };
        // Complete mixed addition of the (possibly negated) affine base:
        // correct for equal, opposite and identity inputs.
        if digit > 0 {
            *bucket += base;
        } else {
            *bucket += &(-*base);
        }
    }
    let mut running = C::identity();
    let mut sum = C::identity();
    for bucket in buckets.iter().rev() {
        running += bucket;
        sum += &running;
    }
    sum
}

/// Runs `window_sum` for every window in waves of at most `concurrency`
/// windows and combines them from the top: `sum_w 2^(width w) S_w` with
/// complete doublings and additions.
fn combine_windows<C: PastaCurve>(
    windows: usize,
    width: usize,
    concurrency: usize,
    window_sum: impl Fn(usize) -> C + Sync,
) -> C {
    let indices: Vec<usize> = (0..windows).collect();
    let mut sums: Vec<C> = Vec::with_capacity(windows);
    for wave in indices.chunks(concurrency.clamp(1, windows.max(1))) {
        let wave_sums: Vec<C> = wave.par_iter().map(|window| window_sum(*window)).collect();
        sums.extend(wave_sums);
    }
    let mut total = C::identity();
    for sum in sums.iter().rev() {
        for _ in 0..width {
            total = total.double();
        }
        total += sum;
    }
    total
}

/// [`msm_complete`] on GLV-split, signed-digit windows, or `None` when a
/// split is unavailable.
fn msm_complete_glv<C: PastaCurve>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
) -> Option<C> {
    let width = signed_window(scalars.len().saturating_mul(2), budget);
    let windows = signed_windows(width);
    let (split_bases, digits) = glv_split::<C>(scalars, bases, width)?;
    let per_window = BUCKET_BYTES << (width - 1);
    let concurrency = budget.bytes() / per_window;
    Some(combine_windows(windows, width, concurrency, |window| {
        signed_window_sum::<C>(&digits, &split_bases, window, windows, width)
    }))
}

/// `sum_i scalars[i] * bases[i]` for public data with complete formulas only
/// (see the module documentation, S10). The scalars and bases must have
/// equal lengths; extra entries of the longer one are ignored. Each scalar is
/// split with the GLV endomorphism into two signed-digit halves (see the
/// module documentation). Windows run in waves on the caller's Rayon pool,
/// at most as many at once as `budget` holds buckets for (always at least
/// one), and are combined in window order, so the result is the same group
/// element for every thread count and budget. It never fails.
#[must_use]
pub fn msm_complete<C: PastaCurve>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
) -> C {
    debug_assert_eq!(scalars.len(), bases.len());
    let n = scalars.len().min(bases.len());
    if n == 0 {
        return C::identity();
    }
    let (scalars, bases) = (&scalars[..n], &bases[..n]);
    msm_complete_glv::<C>(scalars, bases, budget)
        .unwrap_or_else(|| msm_complete_unsigned::<C>(scalars, bases, budget))
}

/// [`msm_complete`] on unsigned full-width windows: the fallback when a GLV
/// split is unavailable, and the differential reference of the tests.
fn msm_complete_unsigned<C: PastaCurve>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
) -> C {
    let n = scalars.len().min(bases.len());
    if n == 0 {
        return C::identity();
    }
    let width = complete_window(n, budget);
    let bits = usize::try_from(<C::ScalarExt as PrimeField>::NUM_BITS).unwrap_or(256);
    let windows = bits.div_ceil(width);
    let limbs: Vec<[u64; 4]> = scalars[..n]
        .par_iter()
        .map(|scalar| {
            let repr = scalar.to_repr();
            let mut limbs = [0_u64; 4];
            for (limb, chunk) in limbs.iter_mut().zip(repr.as_ref().chunks_exact(8)) {
                let mut word = [0_u8; 8];
                word.copy_from_slice(chunk);
                *limb = u64::from_le_bytes(word);
            }
            limbs
        })
        .collect();
    let bases = &bases[..n];
    let concurrency = budget.bytes() / (BUCKET_BYTES << width);
    combine_windows(windows, width, concurrency, |window| {
        complete_window_sum::<C>(&limbs, bases, window * width, width)
    })
}

/// A linear combination of points `sum_i scalars[i] * bases[i]` (public
/// data): the commitment a verifier opens, or the left-hand side it checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Msm<C: PastaCurve> {
    scalars: Vec<C::ScalarExt>,
    bases: Vec<C::AffineExt>,
}

impl<C: PastaCurve> Default for Msm<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: PastaCurve> Msm<C> {
    /// The empty combination (the identity).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scalars: Vec::new(),
            bases: Vec::new(),
        }
    }

    /// The single point `point`.
    #[must_use]
    pub fn from_point(point: C::AffineExt) -> Self {
        Self {
            scalars: vec![C::ScalarExt::ONE],
            bases: vec![point],
        }
    }

    /// Adds `scalar * point`.
    pub fn push(&mut self, scalar: C::ScalarExt, point: C::AffineExt) {
        self.scalars.push(scalar);
        self.bases.push(point);
    }

    /// Multiplies every term by `factor`.
    pub fn scale(&mut self, factor: &C::ScalarExt) {
        for scalar in &mut self.scalars {
            *scalar *= factor;
        }
    }

    /// Adds every term of `other`.
    pub fn add_msm(&mut self, other: &Self) {
        self.scalars.extend_from_slice(&other.scalars);
        self.bases.extend_from_slice(&other.bases);
    }

    /// The number of terms.
    #[must_use]
    pub fn len(&self) -> usize {
        self.scalars.len()
    }

    /// Whether there are no terms.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.scalars.is_empty()
    }

    /// The terms as `(scalar, base)` pairs.
    pub fn terms(&self) -> impl Iterator<Item = (&C::ScalarExt, &C::AffineExt)> {
        self.scalars.iter().zip(self.bases.iter())
    }

    /// Evaluates the combination with [`msm_complete`] (complete formulas
    /// only; never fails for lack of memory).
    #[must_use]
    pub fn evaluate(&self, budget: MemoryBudget) -> C {
        msm_complete::<C>(&self.scalars, &self.bases, budget)
    }

    /// Whether the combination is the identity.
    #[must_use]
    pub fn is_identity(&self, budget: MemoryBudget) -> bool {
        bool::from(self.evaluate(budget).is_identity())
    }

    /// The evaluated combination in affine form.
    #[must_use]
    pub fn to_affine(&self, budget: MemoryBudget) -> C::AffineExt {
        self.evaluate(budget).to_affine()
    }
}

/// The key-generation blind as a field element.
pub(crate) fn default_blind<F: PastaField>() -> F {
    F::from(DEFAULT_BLIND)
}

#[cfg(test)]
mod tests {
    use group::{Curve, Group};
    use iroha_pasta::{Ep, Eq, Fq, msm::msm_naive};
    use rand_chacha::ChaCha20Rng;
    use rand_core_06::SeedableRng;

    use super::*;

    /// Adversarial MSM inputs of `n` terms: random, repeated, opposite and
    /// identity bases; zero, one, minus one, small, all-ones-window and
    /// random scalars; terms that cancel to the identity.
    fn adversarial<C: PastaCurve>(
        n: usize,
        rng: &mut ChaCha20Rng,
    ) -> (Vec<C::ScalarExt>, Vec<C::AffineExt>) {
        let base = C::random(&mut *rng).to_affine();
        let scalars = (0..n)
            .map(|i| match i % 7 {
                0 => C::ScalarExt::ZERO,
                1 => C::ScalarExt::ONE,
                2 => -C::ScalarExt::ONE,
                3 => C::ScalarExt::from(u64::MAX),
                4 => C::ScalarExt::from(0xffff),
                _ => C::ScalarExt::random(&mut *rng),
            })
            .collect();
        let bases = (0..n)
            .map(|i| match i % 5 {
                0 => base,
                1 => -base,
                2 => C::AffineExt::identity(),
                _ => C::random(&mut *rng).to_affine(),
            })
            .collect();
        (scalars, bases)
    }

    fn complete_matches_naive<C: PastaCurve>(seed: u64) {
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        for n in [0_usize, 1, 2, 3, 4, 5, 8, 31, 32, 33, 100, 257] {
            let (scalars, bases) = adversarial::<C>(n, &mut rng);
            let expected = msm_naive::<C>(&scalars, &bases);
            for budget in [
                MemoryBudget::DEFAULT,
                MemoryBudget::new(0),
                MemoryBudget::new(BUCKET_BYTES * 8),
            ] {
                assert_eq!(
                    msm_complete::<C>(&scalars, &bases, budget),
                    expected,
                    "n = {n}, budget = {budget:?}"
                );
            }
            for threads in [1, 2, 4, 7] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .expect("pool");
                let pooled =
                    pool.install(|| msm_complete::<C>(&scalars, &bases, MemoryBudget::DEFAULT));
                assert_eq!(pooled, expected, "n = {n}, {threads} threads");
            }
        }
        // Terms that cancel: P + (-1) P, 2P - P - P, equal bases summed.
        let p = C::random(&mut rng).to_affine();
        let one = C::ScalarExt::ONE;
        let cancel = msm_complete::<C>(
            &[one, -one, one + one, -one, -one],
            &[p, p, p, p, p],
            MemoryBudget::DEFAULT,
        );
        assert!(bool::from(cancel.is_identity()));
        let doubled = msm_complete::<C>(&[one, one], &[p, p], MemoryBudget::DEFAULT);
        assert_eq!(doubled, p.to_curve().double());
    }

    #[test]
    fn complete_msm_matches_the_naive_msm_on_adversarial_inputs() {
        complete_matches_naive::<Ep>(11);
        complete_matches_naive::<Eq>(12);
    }

    #[test]
    fn signed_digits_reconstruct_their_magnitudes() {
        let values = [
            0_u128,
            1,
            2,
            u128::from(u64::MAX),
            1 << 127,
            (1 << 127) - 1,
            u128::MAX,
            u128::MAX - 1,
            0x5555_5555_5555_5555_5555_5555_5555_5555,
            0xaaaa_aaaa_aaaa_aaaa_aaaa_aaaa_aaaa_aaaa,
            0x8000_0000_0000_0001_8000_0000_0000_0001,
        ];
        for width in 1..=MAX_SIGNED_WINDOW {
            let windows = signed_windows(width);
            let half = 1_i64 << (width - 1);
            for value in values {
                let mut digits = vec![0_i32; windows];
                assert_eq!(signed_digits(value, width, &mut digits), 0, "{width}");
                // sum_w d_w 2^(width w) == value, digits in (-half, half].
                let mut total = Fq::ZERO;
                let radix = Fq::from(2_u64).pow_vartime([u64::try_from(width).expect("width")]);
                for digit in digits.iter().rev() {
                    assert!(i64::from(*digit) > -half && i64::from(*digit) <= half);
                    let magnitude = Fq::from(u64::from(digit.unsigned_abs()));
                    total = total * radix + if *digit < 0 { -magnitude } else { magnitude };
                }
                assert_eq!(
                    total,
                    Fq::from_u128(value),
                    "width {width} value {value:#x}"
                );
            }
            // Too few digits leave a carry (or drop high bits).
            let mut short = vec![0_i32; 1];
            if width < 128 {
                let carry = signed_digits(u128::MAX, width, &mut short);
                assert!(carry != 0 || short[0] != 0);
            }
        }
    }

    #[test]
    fn signed_windows_follow_the_cost_model_and_the_budget() {
        assert_eq!(signed_windows(10), 14);
        assert_eq!(signed_windows(16), 9);
        assert_eq!(signed_windows(1), 129);
        // About 2^12 split terms (k = 11) choose 9- to 10-bit windows.
        let width = signed_window(1 << 12, MemoryBudget::DEFAULT);
        assert!((9..=10).contains(&width), "{width}");
        assert!(signed_window(1 << 20, MemoryBudget::DEFAULT) > width);
        assert!(signed_window(2, MemoryBudget::DEFAULT) <= 4);
        assert_eq!(signed_window(1 << 20, MemoryBudget::new(0)), 1);
        assert_eq!(
            signed_window(1 << 20, MemoryBudget::new(BUCKET_BYTES << 4)),
            5
        );
    }

    /// The GLV path equals the unsigned path and the naive MSM on inputs
    /// built around the endomorphism: `B` with `[ZETA] B` and `-[ZETA] B` in
    /// one MSM, scalars `±ZETA`, `±ZETA^2`, halves at `2^127` and `2^128`.
    fn glv_matches_unsigned<C: PastaCurve>(seed: u64) {
        use ff::WithSmallOrderMulGroup;
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        let zeta = <C::ScalarExt as WithSmallOrderMulGroup<3>>::ZETA;
        let p = C::random(&mut rng);
        let endo = p.endo();
        let two_127 = C::ScalarExt::from_u128(1 << 127);
        let two_128 = two_127.double();
        let special = [
            zeta,
            -zeta,
            zeta.square(),
            -zeta.square(),
            C::ScalarExt::ONE,
            -C::ScalarExt::ONE,
            two_127,
            two_128,
            two_128 - C::ScalarExt::ONE,
            -two_128,
            C::ScalarExt::ZERO,
            zeta + C::ScalarExt::ONE,
        ];
        let bases_cycle = [
            p.to_affine(),
            endo.to_affine(),
            (-endo).to_affine(),
            endo.endo().to_affine(),
            C::AffineExt::identity(),
        ];
        for n in [1_usize, 2, 5, 12, 60, 300] {
            let scalars: Vec<C::ScalarExt> = (0..n)
                .map(|i| {
                    if i % 3 == 2 {
                        C::ScalarExt::random(&mut rng)
                    } else {
                        special[i % special.len()]
                    }
                })
                .collect();
            let bases: Vec<C::AffineExt> =
                (0..n).map(|i| bases_cycle[i % bases_cycle.len()]).collect();
            let expected = msm_naive::<C>(&scalars, &bases);
            for budget in [
                MemoryBudget::DEFAULT,
                MemoryBudget::new(0),
                MemoryBudget::new(BUCKET_BYTES * 8),
            ] {
                assert_eq!(
                    msm_complete_glv::<C>(&scalars, &bases, budget),
                    Some(expected),
                    "n = {n}"
                );
                assert_eq!(
                    msm_complete_unsigned::<C>(&scalars, &bases, budget),
                    expected
                );
            }
        }
        // `zeta P - [ZETA] P` cancels inside the split.
        let cancel = msm_complete::<C>(
            &[zeta, -C::ScalarExt::ONE],
            &[p.to_affine(), endo.to_affine()],
            MemoryBudget::DEFAULT,
        );
        assert!(bool::from(cancel.is_identity()));
    }

    #[test]
    fn glv_path_matches_the_unsigned_path_around_the_endomorphism() {
        glv_matches_unsigned::<Ep>(21);
        glv_matches_unsigned::<Eq>(22);
    }

    #[test]
    #[ignore = "random differential sweep; run in release"]
    fn glv_path_matches_the_unsigned_path_on_random_inputs() {
        let mut rng = ChaCha20Rng::seed_from_u64(23);
        for round in 0..64 {
            let n = 1 + (round * 37) % 700;
            let scalars: Vec<Fq> = (0..n).map(|_| Fq::random(&mut rng)).collect();
            let bases: Vec<_> = (0..n).map(|_| Ep::random(&mut rng).to_affine()).collect();
            assert_eq!(
                msm_complete_glv::<Ep>(&scalars, &bases, MemoryBudget::DEFAULT),
                Some(msm_complete_unsigned::<Ep>(
                    &scalars,
                    &bases,
                    MemoryBudget::DEFAULT
                )),
                "round {round}"
            );
        }
    }

    /// Release timing of the verifier MSM against the prover-only
    /// batch-affine `msm_public` (run with `--release -- --ignored
    /// --nocapture`).
    #[test]
    #[ignore = "timing measurement; run in release"]
    fn measure_complete_msm() {
        use std::time::Instant;

        let mut rng = ChaCha20Rng::seed_from_u64(1);
        for log in [10_u32, 12, 14, 16] {
            let n = 1_usize << log;
            let bases: Vec<_> = (0..n).map(|_| Ep::random(&mut rng).to_affine()).collect();
            let scalars: Vec<Fq> = (0..n).map(|_| Fq::random(&mut rng)).collect();
            for threads in [1, 4] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .expect("pool");
                let started = Instant::now();
                let complete =
                    pool.install(|| msm_complete::<Ep>(&scalars, &bases, MemoryBudget::DEFAULT));
                let complete_time = started.elapsed();
                let started = Instant::now();
                let affine = pool
                    .install(|| msm_public::<Ep>(&scalars, &bases, MemoryBudget::DEFAULT))
                    .expect("msm");
                let affine_time = started.elapsed();
                assert_eq!(complete, affine);
                println!(
                    "n=2^{log} threads={threads}: complete {complete_time:?}, \
                     batch-affine {affine_time:?}"
                );
            }
        }
    }

    #[test]
    fn complete_msm_windows_and_digits() {
        assert_eq!(complete_window(1, MemoryBudget::DEFAULT), 1);
        assert_eq!(complete_window(10, MemoryBudget::DEFAULT), 3);
        assert_eq!(complete_window(1 << 16, MemoryBudget::DEFAULT), 12);
        assert_eq!(complete_window(1 << 30, MemoryBudget::DEFAULT), MAX_WINDOW);
        // The window narrows until one window's buckets fit the budget.
        assert_eq!(complete_window(1 << 16, MemoryBudget::new(0)), 1);
        assert_eq!(
            complete_window(1 << 16, MemoryBudget::new(BUCKET_BYTES << 5)),
            5
        );
        let limbs = [0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210, 1, u64::MAX];
        assert_eq!(window_digit(&limbs, 0, 8), 0xef);
        assert_eq!(window_digit(&limbs, 60, 8), 0x00);
        assert_eq!(window_digit(&limbs, 56, 12), 0x001);
        assert_eq!(window_digit(&limbs, 120, 12), 0x1fe);
        assert_eq!(window_digit(&limbs, 252, 8), 0x0f);
        assert_eq!(window_digit(&limbs, 256, 8), 0);
    }

    fn params() -> ParamsIpa<Ep> {
        ParamsIpa::new(4).expect("k = 4")
    }

    #[test]
    fn commitments_agree_across_postures_and_tables() {
        let params = params();
        let mut rng = ChaCha20Rng::seed_from_u64(7);
        let coeffs: Vec<Fq> = (0..16).map(|_| Fq::random(&mut rng)).collect();
        let blind = Fq::random(&mut rng);
        let budget = MemoryBudget::DEFAULT;
        let expected = msm_naive::<Ep>(&coeffs, params.g()) + params.w() * blind;
        let public = commit(&params, &coeffs, &blind, Secrecy::Public, budget).expect("msm");
        let secret = commit(&params, &coeffs, &blind, Secrecy::Secret, budget).expect("msm");
        assert_eq!(public, expected);
        assert_eq!(secret, expected);
        let tables = CommitmentTables::build(&params, budget);
        assert_eq!(tables.present(), (true, true));
        assert_eq!(
            tables.commit(&params, &coeffs, &blind, Secrecy::Secret, budget),
            Ok(expected)
        );
        let lagrange = msm_naive::<Ep>(&coeffs, params.g_lagrange()) + params.w() * blind;
        assert_eq!(
            commit_lagrange(&params, &coeffs, &blind, Secrecy::Public, budget),
            Ok(lagrange)
        );
        assert_eq!(
            tables.commit_lagrange(&params, &coeffs, &blind, Secrecy::Public, budget),
            Ok(lagrange)
        );
        // Prefix commitments skip the tables and use g[..m].
        let prefix = commit(&params, &coeffs[..5], &blind, Secrecy::Public, budget).expect("msm");
        assert_eq!(
            prefix,
            msm_naive::<Ep>(&coeffs[..5], &params.g()[..5]) + params.w() * blind
        );
        assert_eq!(
            tables.commit(&params, &coeffs[..5], &blind, Secrecy::Public, budget),
            Ok(prefix)
        );
        let too_long = vec![Fq::ONE; 17];
        assert!(commit(&params, &too_long, &blind, Secrecy::Public, budget).is_err());
        // An all-zero column commits to W, never the identity.
        let zero = commit_lagrange(
            &params,
            &[Fq::ZERO; 16],
            &default_blind(),
            Secrecy::Public,
            budget,
        )
        .expect("msm");
        assert_eq!(zero.to_affine(), params.w());
        assert_eq!(CommitmentTables::<Ep>::default().present(), (false, false));
        assert_eq!(
            CommitmentTables::<Ep>::build(&params, MemoryBudget::new(0)).present(),
            (false, false)
        );
    }

    #[test]
    fn msm_combinations_and_fallback() {
        let params = params();
        let g = params.g();
        let mut msm = Msm::<Ep>::from_point(g[0]);
        msm.push(Fq::from(2), g[1]);
        let mut other = Msm::<Ep>::new();
        other.push(-Fq::ONE, g[0]);
        msm.add_msm(&other);
        msm.scale(&Fq::from(3));
        assert_eq!(msm.len(), 3);
        assert!(!msm.is_empty());
        assert_eq!(msm.terms().count(), 3);
        let expected = g[1].to_curve() * Fq::from(6);
        assert_eq!(msm.evaluate(MemoryBudget::DEFAULT), expected);
        assert_eq!(msm.to_affine(MemoryBudget::DEFAULT), expected.to_affine());
        // A zero budget narrows the window to one bit with the same result.
        let scalars: Vec<Fq> = (1..=16_u64).map(Fq::from).collect();
        assert_eq!(
            msm_complete::<Ep>(&scalars, g, MemoryBudget::new(0)),
            msm_naive::<Ep>(&scalars, g)
        );
        let mut zero = Msm::<Ep>::from_point(g[2]);
        zero.push(-Fq::ONE, g[2]);
        assert!(zero.is_identity(MemoryBudget::DEFAULT));
        assert!(Msm::<Ep>::default().is_identity(MemoryBudget::DEFAULT));
    }
}
