//! Number-theoretic transforms over the Pasta fields.
//!
//! [`FftDomain`] evaluates and interpolates polynomials on the multiplicative
//! subgroup of order `n = 2^k`, and on its cosets. It uses the generator
//! halo2 uses: `omega = ROOT_OF_UNITY^(2^(32 - k))`.
//!
//! # Algorithm
//!
//! Decimation in frequency (natural-order input, bit-reversed intermediate
//! order), iterative, with two radix-2 stages fused per pass ("radix-4"
//! memory traffic: each pass reads and writes the array once for two stages).
//! A trailing radix-2 stage runs when `k` is odd. The final pass restores
//! natural order. In a prime field a radix-4 butterfly needs the same number of
//! multiplications as two radix-2 stages, so the gain is memory traffic only.
//!
//! The twiddle factors of every stage are cached contiguously in the domain
//! ([`FftDomain::twiddle_bytes`] reports their size). The coset shift is fused
//! into the first pass of [`FftDomain::coset_fft`]; the inverse transforms fuse
//! the `1/n` scaling (and the inverse shift) into the final reordering pass.
//!
//! # Determinism
//!
//! The transforms are exact field arithmetic, so the output is a pure function
//! of the input regardless of the Rayon pool size or chunking.
#![allow(clippy::many_single_char_names)]

use rayon::prelude::*;

use crate::field::PastaField;

/// Below this many elements per pass the transforms run on the calling
/// thread; above it they split work across the caller's Rayon pool.
const PARALLEL_MIN: usize = 1 << 12;

/// FFT domain construction failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FftError {
    /// `k` exceeds the 2-adicity of the field (32) or the platform limit.
    UnsupportedSize {
        /// The requested `log2(n)`.
        k: u64,
    },
    /// The input length is not `n`.
    WrongLength {
        /// The domain size `n`.
        expected: usize,
        /// The input length.
        actual: usize,
    },
    /// A coset shift of zero (not a coset; the inverse transform would need
    /// `0^-1`).
    ZeroShift,
}

impl core::fmt::Display for FftError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedSize { k } => write!(f, "unsupported FFT size 2^{k}"),
            Self::WrongLength { expected, actual } => {
                write!(f, "FFT input has {actual} elements, expected {expected}")
            }
            Self::ZeroShift => write!(f, "FFT coset shift is zero"),
        }
    }
}

impl std::error::Error for FftError {}

/// A multiplicative subgroup of order `2^k` with cached twiddle factors.
#[derive(Clone, Debug)]
pub struct FftDomain<F: PastaField> {
    k: u32,
    n: usize,
    omega: F,
    omega_inv: F,
    n_inv: F,
    /// `forward[s]` holds `w_{2h}^j` for `j < h`, `h = 2^s`.
    forward: Vec<Vec<F>>,
    /// The same for `omega_inv`.
    inverse: Vec<Vec<F>>,
}

impl<F: PastaField> FftDomain<F> {
    /// Builds the domain of size `2^k` and its twiddle caches.
    ///
    /// # Errors
    ///
    /// [`FftError::UnsupportedSize`] when `k > 32` or `2^k` does not fit
    /// `usize`.
    pub fn new(k: u32) -> Result<Self, FftError> {
        if k > F::S || k >= usize::BITS {
            return Err(FftError::UnsupportedSize { k: u64::from(k) });
        }
        let n = 1usize << k;
        let mut omega = F::ROOT_OF_UNITY;
        let mut omega_inv = F::ROOT_OF_UNITY_INV;
        for _ in k..F::S {
            omega = omega.square();
            omega_inv = omega_inv.square();
        }
        // n^-1 = (1/2)^k.
        let n_inv = F::TWO_INV.pow_vartime([u64::from(k)]);
        let forward = stage_twiddles(omega, k);
        let inverse = stage_twiddles(omega_inv, k);
        Ok(Self {
            k,
            n,
            omega,
            omega_inv,
            n_inv,
            forward,
            inverse,
        })
    }

    /// `log2` of the domain size.
    pub fn k(&self) -> u32 {
        self.k
    }

    /// The domain size `n = 2^k`.
    pub fn n(&self) -> usize {
        self.n
    }

    /// The generator `omega` of the subgroup.
    pub fn omega(&self) -> F {
        self.omega
    }

    /// `omega^-1`.
    pub fn omega_inv(&self) -> F {
        self.omega_inv
    }

    /// `n^-1`.
    pub fn n_inv(&self) -> F {
        self.n_inv
    }

    /// Bytes held by the twiddle caches (both directions).
    pub fn twiddle_bytes(&self) -> usize {
        let count: usize = self
            .forward
            .iter()
            .chain(self.inverse.iter())
            .map(Vec::len)
            .sum();
        count.saturating_mul(core::mem::size_of::<F>())
    }

    fn check(&self, a: &[F]) -> Result<(), FftError> {
        if a.len() == self.n {
            Ok(())
        } else {
            Err(FftError::WrongLength {
                expected: self.n,
                actual: a.len(),
            })
        }
    }

    /// Evaluates the polynomial with coefficients `a` at `omega^i`, in place.
    ///
    /// # Errors
    ///
    /// [`FftError::WrongLength`] unless `a.len() == n`.
    pub fn fft(&self, a: &mut [F]) -> Result<(), FftError> {
        self.check(a)?;
        dif(a, &self.forward, None);
        bit_reverse_scale(a, self.k, None);
        Ok(())
    }

    /// Interpolates evaluations at `omega^i` back to coefficients, in place.
    ///
    /// # Errors
    ///
    /// [`FftError::WrongLength`] unless `a.len() == n`.
    pub fn ifft(&self, a: &mut [F]) -> Result<(), FftError> {
        self.check(a)?;
        dif(a, &self.inverse, None);
        bit_reverse_scale(a, self.k, Some((self.n_inv, F::ONE)));
        Ok(())
    }

    /// Evaluates the polynomial with coefficients `a` at `shift * omega^i`,
    /// in place. The multiplication of coefficient `i` by `shift^i` is fused
    /// into the first pass.
    ///
    /// # Errors
    ///
    /// [`FftError::WrongLength`] unless `a.len() == n`;
    /// [`FftError::ZeroShift`] for `shift = 0`, which is not a coset (checked
    /// before `a` is touched).
    pub fn coset_fft(&self, a: &mut [F], shift: F) -> Result<(), FftError> {
        self.check(a)?;
        check_shift(&shift)?;
        dif(a, &self.forward, Some(shift));
        bit_reverse_scale(a, self.k, None);
        Ok(())
    }

    /// Interpolates evaluations at `shift * omega^i` back to coefficients, in
    /// place.
    ///
    /// # Errors
    ///
    /// [`FftError::WrongLength`] unless `a.len() == n`;
    /// [`FftError::ZeroShift`] for `shift = 0` (checked before `a` is
    /// touched).
    pub fn coset_ifft(&self, a: &mut [F], shift: F) -> Result<(), FftError> {
        self.check(a)?;
        check_shift(&shift)?;
        // `shift` is nonzero, so the inverse exists.
        let shift_inv = shift.invert().unwrap_or(F::ZERO);
        dif(a, &self.inverse, None);
        bit_reverse_scale(a, self.k, Some((self.n_inv, shift_inv)));
        Ok(())
    }
}

/// Rejects a zero coset shift.
fn check_shift<F: PastaField>(shift: &F) -> Result<(), FftError> {
    if bool::from(shift.is_zero()) {
        Err(FftError::ZeroShift)
    } else {
        Ok(())
    }
}

/// `tw[s][j] = w^j` with `w` the primitive `2^(s+1)`-th root, for `j < 2^s`.
fn stage_twiddles<F: PastaField>(omega: F, k: u32) -> Vec<Vec<F>> {
    (0..k)
        .map(|s| {
            // w_{2h} = omega^(n / 2h) = omega^(2^(k - s - 1)).
            let w = omega.pow_vartime([1u64 << (k - s - 1)]);
            let h = 1usize << s;
            let mut v = Vec::with_capacity(h);
            let mut cur = F::ONE;
            for _ in 0..h {
                v.push(cur);
                cur *= w;
            }
            v
        })
        .collect()
}

/// Decimation-in-frequency transform; leaves the output in bit-reversed order.
///
/// With `shift`, coefficient `i` is multiplied by `shift^i` while the first
/// pass loads it.
fn dif<F: PastaField>(a: &mut [F], tw: &[Vec<F>], shift: Option<F>) {
    let n = a.len();
    if n <= 1 {
        // A single coefficient is its own evaluation; shift^0 = 1.
        return;
    }
    let k = tw.len();
    let mut s = k; // stages are applied for half sizes 2^(s-1) down to 1
    let mut first = true;
    while s >= 2 {
        // Fused stages with half sizes 2m (outer) and m (inner).
        let m = 1usize << (s - 2);
        let outer = &tw[s - 1]; // w_{4m}^j, j < 2m
        let inner = &tw[s - 2]; // w_{2m}^j, j < m
        let pass_shift = if first { shift } else { None };
        dif_pass4(a, m, outer, inner, pass_shift);
        first = false;
        s -= 2;
    }
    if s == 1 {
        // Final radix-2 stage with half size 1: twiddle 1.
        if let (true, Some(sh)) = (first, shift) {
            // k == 1: apply the shift to coefficient 1 before the stage.
            a[1] *= sh;
        }
        let body = |chunk: &mut [F]| {
            for pair in chunk.chunks_exact_mut(2) {
                let (x, y) = (pair[0], pair[1]);
                pair[0] = x + y;
                pair[1] = x - y;
            }
        };
        if n >= PARALLEL_MIN {
            a.par_chunks_mut(PARALLEL_MIN).for_each(body);
        } else {
            body(a);
        }
    }
}

/// One fused DIF pass over blocks of `4m`.
fn dif_pass4<F: PastaField>(a: &mut [F], m: usize, outer: &[F], inner: &[F], shift: Option<F>) {
    let n = a.len();
    let block = 4 * m;
    // The shift only occurs in the first pass, where the block is the whole
    // array, so element j + q*m sits at original index j + q*m.
    let quarter_shift = shift.map(|sh| {
        let sm = sh.pow_vartime([m as u64]);
        let s2m = sm.square();
        (sh, [F::ONE, sm, s2m, s2m * sm])
    });
    let butterfly = |q0: &mut [F], q1: &mut [F], q2: &mut [F], q3: &mut [F], j0: usize| {
        let mut sj = quarter_shift.map(|(sh, _)| sh.pow_vartime([j0 as u64]));
        for (off, (((x0, x1), x2), x3)) in q0
            .iter_mut()
            .zip(q1.iter_mut())
            .zip(q2.iter_mut())
            .zip(q3.iter_mut())
            .enumerate()
        {
            let j = j0 + off;
            let (mut a0, mut a1, mut a2, mut a3) = (*x0, *x1, *x2, *x3);
            if let (Some(s), Some((sh, c))) = (sj.as_mut(), quarter_shift.as_ref()) {
                a0 *= *s;
                a1 *= *s * c[1];
                a2 *= *s * c[2];
                a3 *= *s * c[3];
                *s *= sh;
            }
            // Outer stage (half 2m): pairs (j, j+2m) and (j+m, j+3m).
            let y0 = a0 + a2;
            let y1 = a1 + a3;
            let (y2, y3) = if j == 0 {
                (a0 - a2, (a1 - a3) * outer[m])
            } else {
                ((a0 - a2) * outer[j], (a1 - a3) * outer[j + m])
            };
            // Inner stage (half m): pairs (j, j+m) and (j+2m, j+3m).
            *x0 = y0 + y1;
            *x2 = y2 + y3;
            if j == 0 {
                *x1 = y0 - y1;
                *x3 = y2 - y3;
            } else {
                let w = inner[j];
                *x1 = (y0 - y1) * w;
                *x3 = (y2 - y3) * w;
            }
        }
    };
    let blocks = n / block;
    if n < PARALLEL_MIN || blocks >= rayon::current_num_threads() * 4 {
        let run = |blk: &mut [F]| {
            let (q0, rest) = blk.split_at_mut(m);
            let (q1, rest) = rest.split_at_mut(m);
            let (q2, q3) = rest.split_at_mut(m);
            butterfly(q0, q1, q2, q3, 0);
        };
        if n < PARALLEL_MIN {
            a.chunks_exact_mut(block).for_each(run);
        } else {
            a.par_chunks_exact_mut(block).for_each(run);
        }
    } else {
        // Few large blocks: split each block's j range across the pool.
        let chunk = (m / (rayon::current_num_threads() * 2))
            .max(256)
            .min(m)
            .max(1);
        for blk in a.chunks_exact_mut(block) {
            let (q0, rest) = blk.split_at_mut(m);
            let (q1, rest) = rest.split_at_mut(m);
            let (q2, q3) = rest.split_at_mut(m);
            q0.par_chunks_mut(chunk)
                .zip(q1.par_chunks_mut(chunk))
                .zip(q2.par_chunks_mut(chunk))
                .zip(q3.par_chunks_mut(chunk))
                .enumerate()
                .for_each(|(c, (((c0, c1), c2), c3))| butterfly(c0, c1, c2, c3, c * chunk));
        }
    }
}

/// Bit-reverses the order of `a` (length `2^k`) and, with `scale = (c, s)`,
/// multiplies the element at natural index `i` by `c * s^i`.
fn bit_reverse_scale<F: PastaField>(a: &mut [F], k: u32, scale: Option<(F, F)>) {
    let n = a.len();
    if k > 0 {
        let shift = usize::BITS - k;
        for i in 0..n {
            let r = i.reverse_bits() >> shift;
            if i < r {
                a.swap(i, r);
            }
        }
    }
    if let Some((c, s)) = scale {
        let unit_shift = s == F::ONE;
        let body = |(ci, chunk): (usize, &mut [F])| {
            if unit_shift {
                // Plain inverse transform: one multiplication per element.
                for x in chunk {
                    *x *= c;
                }
                return;
            }
            let start = ci * PARALLEL_MIN;
            let mut f = c * s.pow_vartime([start as u64]);
            for x in chunk {
                *x *= f;
                f *= s;
            }
        };
        if n >= PARALLEL_MIN {
            a.par_chunks_mut(PARALLEL_MIN).enumerate().for_each(body);
        } else {
            a.chunks_mut(PARALLEL_MIN).enumerate().for_each(body);
        }
    }
}

/// Reference `O(n^2)` evaluation of `a` at `shift * omega^i` (tests and
/// differential checks only).
pub fn naive_coset_dft<F: PastaField>(a: &[F], omega: F, shift: F) -> Vec<F> {
    let n = a.len();
    let mut out = Vec::with_capacity(n);
    let mut x = shift;
    for _ in 0..n {
        // Horner evaluation at x.
        let v = a.iter().rev().fold(F::ZERO, |acc, c| acc * x + c);
        out.push(v);
        x *= omega;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Fp, Fq};
    use ff::Field;
    use rand_chacha::ChaCha20Rng;
    use rand_core_06::SeedableRng;

    fn random_vec<F: PastaField>(n: usize, seed: u64) -> Vec<F> {
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        (0..n).map(|_| F::random(&mut rng)).collect()
    }

    #[test]
    fn small_sizes_match_naive() {
        for k in 0..=9u32 {
            let d = FftDomain::<Fp>::new(k).unwrap();
            let a = random_vec::<Fp>(1 << k, u64::from(k));
            let mut b = a.clone();
            d.fft(&mut b).unwrap();
            assert_eq!(b, naive_coset_dft(&a, d.omega(), Fp::ONE), "k = {k}");
            d.ifft(&mut b).unwrap();
            assert_eq!(b, a);
            let shift = <Fp as ff::WithSmallOrderMulGroup<3>>::ZETA;
            let mut c = a.clone();
            d.coset_fft(&mut c, shift).unwrap();
            assert_eq!(c, naive_coset_dft(&a, d.omega(), shift), "coset k = {k}");
            d.coset_ifft(&mut c, shift).unwrap();
            assert_eq!(c, a);
        }
    }

    #[test]
    fn domain_parameters() {
        let d = FftDomain::<Fq>::new(10).unwrap();
        assert_eq!(d.n(), 1024);
        assert_eq!(d.k(), 10);
        assert_eq!(d.omega().pow_vartime([1024u64]), Fq::ONE);
        assert_ne!(d.omega().pow_vartime([512u64]), Fq::ONE);
        assert_eq!(d.omega() * d.omega_inv(), Fq::ONE);
        assert_eq!(d.n_inv() * Fq::from(1024u64), Fq::ONE);
        assert!(d.twiddle_bytes() > 0);
        assert_eq!(
            FftDomain::<Fq>::new(33).unwrap_err(),
            FftError::UnsupportedSize { k: 33 }
        );
        let mut short = vec![Fq::ZERO; 3];
        assert_eq!(
            d.fft(&mut short),
            Err(FftError::WrongLength {
                expected: 1024,
                actual: 3
            })
        );
        assert_eq!(
            FftError::UnsupportedSize { k: 40 }.to_string(),
            "unsupported FFT size 2^40"
        );
    }

    #[test]
    fn zero_coset_shift_is_rejected_without_touching_the_input() {
        let d = FftDomain::<Fp>::new(3).unwrap();
        let a = random_vec::<Fp>(8, 21);
        let mut b = a.clone();
        assert_eq!(d.coset_ifft(&mut b, Fp::ZERO), Err(FftError::ZeroShift));
        assert_eq!(b, a);
        assert_eq!(d.coset_fft(&mut b, Fp::ZERO), Err(FftError::ZeroShift));
        assert_eq!(b, a);
        assert_eq!(FftError::ZeroShift.to_string(), "FFT coset shift is zero");
        assert_eq!(check_shift(&Fp::ONE), Ok(()));
        // Length errors take precedence, as before.
        assert!(matches!(
            d.coset_ifft(&mut b[..3], Fp::ZERO),
            Err(FftError::WrongLength { .. })
        ));
    }

    #[test]
    fn stage_twiddles_are_powers() {
        let w = FftDomain::<Fp>::new(4).unwrap().omega();
        let tw = stage_twiddles(w, 4);
        assert_eq!(tw.len(), 4);
        assert_eq!(tw[3][1], w);
        assert_eq!(tw[0], vec![Fp::ONE]);
    }

    #[test]
    fn bit_reverse_scale_permutes() {
        let mut a: Vec<Fp> = (0..8u64).map(Fp::from).collect();
        bit_reverse_scale(&mut a, 3, None);
        let expected: Vec<Fp> = [0u64, 4, 2, 6, 1, 5, 3, 7]
            .into_iter()
            .map(Fp::from)
            .collect();
        assert_eq!(a, expected);
        let mut b = vec![Fp::ONE; 4];
        bit_reverse_scale(&mut b, 2, Some((Fp::from(3u64), Fp::from(2u64))));
        assert_eq!(
            b,
            vec![
                Fp::from(3u64),
                Fp::from(6u64),
                Fp::from(12u64),
                Fp::from(24u64)
            ]
        );
    }

    #[test]
    fn dif_pass_and_naive_dft_agree_on_impulse() {
        let d = FftDomain::<Fp>::new(3).unwrap();
        let mut a = vec![Fp::ZERO; 8];
        a[1] = Fp::ONE;
        d.fft(&mut a).unwrap();
        let mut x = Fp::ONE;
        for v in a {
            assert_eq!(v, x);
            x *= d.omega();
        }
        let mut b = vec![Fp::ONE; 8];
        dif(&mut b, &stage_twiddles(d.omega(), 3), None);
        assert_eq!(b[0], Fp::from(8u64));
    }
}
