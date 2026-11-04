//! Radix-2 number-theoretic transforms over Goldilocks subgroups and their cosets.
//!
//! A [`Domain`] is the multiplicative subgroup `H = <w>` of size `2^log_n`. Its transforms map
//! natural-order evaluations on `shift * H` to natural-order coefficients and back. Low-degree
//! extensions evaluate a polynomial on `blowup` cosets of `H` with one size-`n` transform each,
//! which keeps every transform cache-resident.

use crate::field::{Fp, Fp4, batch_inverse_ext};

/// Twiddle tables for one power-of-two subgroup.
#[derive(Clone, Debug)]
pub struct Domain {
    log_n: u32,
    generator: Fp,
    /// `w^i` for `i < n/2`.
    twiddles: Vec<Fp>,
    /// Stage tables concatenated: stage with half-size `h` holds `w_{2h}^j`, `j < h`, at offset `h - 1`.
    stage_twiddles: Vec<Fp>,
    /// The same for `w^-1`.
    stage_inverse_twiddles: Vec<Fp>,
    n_inverse: Fp,
}

fn bit_reverse_permute<T: Copy>(values: &mut [T]) {
    let n = values.len();
    if n <= 2 {
        return;
    }
    let shift = usize::BITS - n.trailing_zeros();
    for i in 0..n {
        let j = i.reverse_bits() >> shift;
        if i < j {
            values.swap(i, j);
        }
    }
}

fn stage_tables(generator: Fp, log_n: u32) -> Vec<Fp> {
    let n = 1usize << log_n;
    let mut out = Vec::with_capacity(n);
    let mut half = 1usize;
    while half < n {
        // w_{2h} = generator^(n / 2h)
        let step = generator.pow((n / (2 * half)) as u64);
        let mut w = Fp::ONE;
        for _ in 0..half {
            out.push(w);
            w *= step;
        }
        half *= 2;
    }
    out
}

fn dit(values: &mut [Fp], stages: &[Fp]) {
    let n = values.len();
    bit_reverse_permute(values);
    for pair in values.chunks_exact_mut(2) {
        let (u, v) = (pair[0], pair[1]);
        pair[0] = u + v;
        pair[1] = u - v;
    }
    let mut half = 2;
    while half < n {
        let tw = &stages[half - 1..2 * half - 1];
        for block in values.chunks_exact_mut(2 * half) {
            let (lo, hi) = block.split_at_mut(half);
            for ((l, h), w) in lo.iter_mut().zip(hi.iter_mut()).zip(tw) {
                let t = *h * *w;
                let u = *l;
                *l = u + t;
                *h = u - t;
            }
        }
        half *= 2;
    }
}

/// Powers `c^i` of each coset shift of a low-degree extension, shared by every column.
#[derive(Clone, Debug)]
pub struct CosetPowers {
    /// `powers[j][i] = (shift * v^j)^i`.
    pub powers: Vec<Vec<Fp>>,
}

impl CosetPowers {
    /// Precomputes the `blowup` coset shifts `shift * v^j` of the extension of `domain`.
    pub fn new(domain: &Domain, blowup: usize, shift: Fp) -> Self {
        let n = domain.size();
        let v = Fp::two_adic_generator(domain.log_n() + blowup.trailing_zeros());
        let mut coset_shift = shift;
        let mut powers = Vec::with_capacity(blowup);
        for _ in 0..blowup {
            let mut row = Vec::with_capacity(n);
            let mut p = Fp::ONE;
            for _ in 0..n {
                row.push(p);
                p *= coset_shift;
            }
            powers.push(row);
            coset_shift *= v;
        }
        Self { powers }
    }
}

impl Domain {
    /// Builds the subgroup of size `2^log_n`.
    ///
    /// # Panics
    /// Panics if `log_n` is zero or exceeds the field's two-adicity.
    pub fn new(log_n: u32) -> Self {
        assert!((1..=32).contains(&log_n), "domain size must be 2^1..2^32");
        let n = 1usize << log_n;
        let generator = Fp::two_adic_generator(log_n);
        let generator_inverse = generator.inverse().expect("root of unity is nonzero");
        let mut twiddles = Vec::with_capacity(n / 2);
        let mut inverse_twiddles = Vec::with_capacity(n / 2);
        let (mut w, mut w_inv) = (Fp::ONE, Fp::ONE);
        for _ in 0..n / 2 {
            twiddles.push(w);
            inverse_twiddles.push(w_inv);
            w *= generator;
            w_inv *= generator_inverse;
        }
        let n_inverse = Fp::new(n as u64).inverse().expect("n is nonzero");
        let _ = inverse_twiddles;
        Self {
            log_n,
            generator,
            twiddles,
            stage_twiddles: stage_tables(generator, log_n),
            stage_inverse_twiddles: stage_tables(generator_inverse, log_n),
            n_inverse,
        }
    }

    /// `log2` of the size.
    pub const fn log_n(&self) -> u32 {
        self.log_n
    }

    /// The size.
    pub const fn size(&self) -> usize {
        1 << self.log_n
    }

    /// The generator `w`.
    pub const fn generator(&self) -> Fp {
        self.generator
    }

    /// `w^i`.
    pub fn element(&self, i: usize) -> Fp {
        let n = self.size();
        let i = i % n;
        if i < n / 2 {
            self.twiddles[i]
        } else {
            -self.twiddles[i - n / 2]
        }
    }

    /// Coefficients (natural order) to evaluations `f(w^k)` (natural order).
    ///
    /// # Panics
    /// Panics if the slice length differs from the domain size.
    pub fn forward(&self, values: &mut [Fp]) {
        assert_eq!(values.len(), self.size());
        dit(values, &self.stage_twiddles);
    }

    /// Evaluations `f(w^k)` to coefficients.
    ///
    /// # Panics
    /// Panics if the slice length differs from the domain size.
    pub fn inverse(&self, values: &mut [Fp]) {
        assert_eq!(values.len(), self.size());
        dit(values, &self.stage_inverse_twiddles);
        for v in values.iter_mut() {
            *v *= self.n_inverse;
        }
    }

    /// Coefficients to evaluations on the coset `shift * H`.
    pub fn forward_on_coset(&self, values: &mut [Fp], shift: Fp) {
        let mut power = Fp::ONE;
        for v in values.iter_mut() {
            *v *= power;
            power *= shift;
        }
        self.forward(values);
    }

    /// Evaluations on `shift * H` to coefficients.
    pub fn inverse_on_coset(&self, values: &mut [Fp], shift: Fp) {
        self.inverse(values);
        let shift_inverse = shift.inverse().expect("coset shift is nonzero");
        let mut power = Fp::ONE;
        for v in values.iter_mut() {
            *v *= power;
            power *= shift_inverse;
        }
    }

    /// Writes the low-degree extension of `coefficients` into `out` (coset-major: coset `j` occupies
    /// `out[j*n..(j+1)*n]`), using precomputed coset powers.
    pub fn low_degree_extend_into(
        &self,
        coefficients: &[Fp],
        powers: &CosetPowers,
        out: &mut [Fp],
    ) {
        let n = self.size();
        assert_eq!(coefficients.len(), n);
        assert_eq!(out.len(), n * powers.powers.len());
        for (chunk, coset) in out.chunks_exact_mut(n).zip(&powers.powers) {
            for ((o, c), p) in chunk.iter_mut().zip(coefficients).zip(coset) {
                *o = *c * *p;
            }
            self.forward(chunk);
        }
    }

    /// Evaluates coefficients on the `blowup` cosets `shift * v^j * H`, `v` a primitive
    /// `blowup*n`-th root. Coset `j` position `k` is the point `shift * v^(j + blowup*k)`, i.e. LDE
    /// index `j + blowup*k` of the domain `shift * <v>`.
    pub fn low_degree_extend(&self, coefficients: &[Fp], blowup: usize, shift: Fp) -> Vec<Vec<Fp>> {
        let n = self.size();
        assert_eq!(coefficients.len(), n);
        let log_blowup = blowup.trailing_zeros();
        let v = Fp::two_adic_generator(self.log_n + log_blowup);
        let mut coset_shift = shift;
        let mut out = Vec::with_capacity(blowup);
        for _ in 0..blowup {
            let mut buffer = coefficients.to_vec();
            self.forward_on_coset(&mut buffer, coset_shift);
            out.push(buffer);
            coset_shift *= v;
        }
        out
    }
}

/// Barycentric weights `w^k / (z - w^k)` scaled by `(z^n - 1)/n`, so that
/// `f(z) = sum_k f(w^k) * weight_k` for every polynomial of degree `< n`.
///
/// # Panics
/// Panics if `z` lies in the subgroup.
pub fn barycentric_weights(domain: &Domain, z: Fp4) -> Vec<Fp4> {
    let n = domain.size();
    let mut denominators: Vec<Fp4> = (0..n)
        .map(|k| z - Fp4::from_base(domain.element(k)))
        .collect();
    assert!(
        denominators.iter().all(|d| !d.is_zero()),
        "evaluation point lies in the trace domain"
    );
    batch_inverse_ext(&mut denominators);
    let scale = (z.pow(n as u64) - Fp4::ONE).mul_base(Fp::new(n as u64).inverse().expect("n != 0"));
    denominators
        .iter()
        .enumerate()
        .map(|(k, inv)| (*inv * scale).mul_base(domain.element(k)))
        .collect()
}

/// `sum_k values[k] * weights[k]` for base-field values.
pub fn evaluate_with_weights(values: &[Fp], weights: &[Fp4]) -> Fp4 {
    let mut acc = [Fp::ZERO; 4];
    for (v, w) in values.iter().zip(weights) {
        acc[0] += w.0[0] * *v;
        acc[1] += w.0[1] * *v;
        acc[2] += w.0[2] * *v;
        acc[3] += w.0[3] * *v;
    }
    Fp4(acc)
}

/// Evaluates a polynomial given by base-field coefficients at an extension point (Horner).
pub fn evaluate_coefficients(coefficients: &[Fp], z: Fp4) -> Fp4 {
    let mut acc = Fp4::ZERO;
    for c in coefficients.iter().rev() {
        acc = (acc * z).add_base(*c);
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive_eval(coefficients: &[Fp], x: Fp) -> Fp {
        coefficients
            .iter()
            .rev()
            .fold(Fp::ZERO, |acc, c| acc * x + *c)
    }

    #[test]
    fn forward_matches_naive_evaluation_and_inverts() {
        for log_n in [1u32, 2, 3, 6, 10] {
            let domain = Domain::new(log_n);
            let coefficients: Vec<Fp> = (0..domain.size() as u64)
                .map(|i| Fp::new(i * i + 3))
                .collect();
            let mut values = coefficients.clone();
            domain.forward(&mut values);
            for (k, v) in values.iter().enumerate() {
                assert_eq!(*v, naive_eval(&coefficients, domain.element(k)));
            }
            domain.inverse(&mut values);
            assert_eq!(values, coefficients);
            let shift = Fp::new(7);
            let mut coset = coefficients.clone();
            domain.forward_on_coset(&mut coset, shift);
            for (k, v) in coset.iter().enumerate() {
                assert_eq!(*v, naive_eval(&coefficients, shift * domain.element(k)));
            }
            domain.inverse_on_coset(&mut coset, shift);
            assert_eq!(coset, coefficients);
        }
    }

    #[test]
    fn lde_cosets_cover_the_large_domain() {
        let domain = Domain::new(4);
        let coefficients: Vec<Fp> = (0..16u64).map(|i| Fp::new(5 * i + 1)).collect();
        let shift = Fp::new(7);
        let cosets = domain.low_degree_extend(&coefficients, 4, shift);
        let big = Fp::two_adic_generator(6);
        for (j, coset) in cosets.iter().enumerate() {
            for (k, v) in coset.iter().enumerate() {
                let x = shift * big.pow((j + 4 * k) as u64);
                assert_eq!(*v, naive_eval(&coefficients, x));
            }
        }
    }

    #[test]
    fn barycentric_matches_horner() {
        let domain = Domain::new(5);
        let coefficients: Vec<Fp> = (0..32u64).map(|i| Fp::new(i * 977 + 13)).collect();
        let mut values = coefficients.clone();
        domain.forward(&mut values);
        let z = Fp4([Fp::new(3), Fp::new(1), Fp::new(4), Fp::new(1)]);
        let weights = barycentric_weights(&domain, z);
        assert_eq!(
            evaluate_with_weights(&values, &weights),
            evaluate_coefficients(&coefficients, z)
        );
    }
}
