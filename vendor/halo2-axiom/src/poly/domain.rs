//! Contains utilities for performing polynomial arithmetic over an evaluation
//! domain that is of a suitable size for the application.

use crate::{
    arithmetic::{best_fft, parallelize},
    fft::recursive::FFTData,
    plonk::Assigned,
};

use super::{Coeff, ExtendedLagrangeCoeff, LagrangeCoeff, Polynomial, Rotation};

use group::ff::{BatchInvert, Field, WithSmallOrderMulGroup};

use std::{collections::HashMap, marker::PhantomData};

/// This structure contains precomputed constants and other details needed for
/// performing operations on an evaluation domain of size $2^k$ and an extended
/// domain of size $2^{k} * j$ with $j \neq 0$.
#[derive(Clone, Debug)]
pub struct EvaluationDomain<F: Field> {
    n: u64,
    k: u32,
    extended_k: u32,
    omega: F,
    omega_inv: F,
    extended_omega: F,
    extended_omega_inv: F,
    g_coset: F,
    g_coset_inv: F,
    quotient_poly_degree: u64,
    ifft_divisor: F,
    extended_ifft_divisor: F,
    t_evaluations: Vec<F>,
    barycentric_weight: F,

    // Recursive stuff
    fft_data: HashMap<usize, FFTData<F>>,
}

impl<F: WithSmallOrderMulGroup<3>> EvaluationDomain<F> {
    /// This constructs a new evaluation domain object based on the provided
    /// values $j, k$.
    pub fn new(j: u32, k: u32) -> Self {
        // quotient_poly_degree * params.n - 1 is the degree of the quotient polynomial
        let quotient_poly_degree = (j - 1) as u64;

        // n = 2^k
        let n = 1u64 << k;

        // We need to work within an extended domain, not params.k but params.k + i
        // for some integer i such that 2^(params.k + i) is sufficiently large to
        // describe the quotient polynomial.
        let mut extended_k = k;
        while (1 << extended_k) < (n * quotient_poly_degree) {
            extended_k += 1;
        }
        #[cfg(feature = "profile")]
        println!("k: {}, extended_k: {}", k, extended_k);

        // ensure extended_k <= S
        assert!(extended_k <= F::S);

        let mut extended_omega = F::ROOT_OF_UNITY;

        // Get extended_omega, the 2^{extended_k}'th root of unity
        // The loop computes extended_omega = omega^{2 ^ (S - extended_k)}
        // Notice that extended_omega ^ {2 ^ extended_k} = omega ^ {2^S} = 1.
        for _ in extended_k..F::S {
            extended_omega = extended_omega.square();
        }
        let extended_omega = extended_omega;

        // Get omega, the 2^{k}'th root of unity (i.e. n'th root of unity)
        // The loop computes omega = extended_omega ^ {2 ^ (extended_k - k)}
        //           = (omega^{2 ^ (S - extended_k)})  ^ {2 ^ (extended_k - k)}
        //           = omega ^ {2 ^ (S - k)}.
        // Notice that omega ^ {2^k} = omega ^ {2^S} = 1.
        let mut omegas = Vec::with_capacity((extended_k - k + 1) as usize);
        let mut omega = extended_omega;
        omegas.push(omega);
        for _ in k..extended_k {
            omega = omega.square();
            omegas.push(omega);
        }
        let omega = omega;
        omegas.reverse();
        let mut omegas_inv = omegas.clone(); // Inversion computed later

        // We use zeta here because we know it generates a coset, and it's available
        // already.
        // The coset evaluation domain is:
        // zeta {1, extended_omega, extended_omega^2, ..., extended_omega^{(2^extended_k) - 1}}
        let g_coset = F::ZETA;
        let g_coset_inv = g_coset.square();

        let mut t_evaluations = Vec::with_capacity(1 << (extended_k - k));
        {
            // Compute the evaluations of t(X) = X^n - 1 in the coset evaluation domain.
            // We don't have to compute all of them, because it will repeat.
            let orig = F::ZETA.pow_vartime([n]);
            let step = extended_omega.pow_vartime([n]);
            let mut cur = orig;
            loop {
                t_evaluations.push(cur);
                cur *= &step;
                if cur == orig {
                    break;
                }
            }
            assert_eq!(t_evaluations.len(), 1 << (extended_k - k));

            // Subtract 1 from each to give us t_evaluations[i] = t(zeta * extended_omega^i)
            for coeff in &mut t_evaluations {
                *coeff -= &F::ONE;
            }

            // Invert, because we're dividing by this polynomial.
            // We invert in a batch, below.
        }

        let mut ifft_divisor = F::from(1 << k); // Inversion computed later
        let mut extended_ifft_divisor = F::from(1 << extended_k); // Inversion computed later

        // The barycentric weight of 1 over the evaluation domain
        // 1 / \prod_{i != 0} (1 - omega^i)
        let mut barycentric_weight = F::from(n); // Inversion computed later

        // Compute batch inversion
        t_evaluations
            .iter_mut()
            .chain(Some(&mut ifft_divisor))
            .chain(Some(&mut extended_ifft_divisor))
            .chain(Some(&mut barycentric_weight))
            .chain(&mut omegas_inv)
            .batch_invert();

        let omega_inv = omegas_inv[0];
        let extended_omega_inv = *omegas_inv.last().unwrap();
        let mut fft_data = HashMap::new();
        for (i, (omega, omega_inv)) in omegas.into_iter().zip(omegas_inv).enumerate() {
            let intermediate_k = k as usize + i;
            let len = 1usize << intermediate_k;
            fft_data.insert(len, FFTData::<F>::new(len, omega, omega_inv));
        }

        EvaluationDomain {
            n,
            k,
            extended_k,
            omega,
            omega_inv,
            extended_omega,
            extended_omega_inv,
            g_coset,
            g_coset_inv,
            quotient_poly_degree,
            ifft_divisor,
            extended_ifft_divisor,
            t_evaluations,
            barycentric_weight,
            fft_data,
        }
    }

    /// Obtains a polynomial in Lagrange form when given a vector of Lagrange
    /// coefficients of size `n`; panics if the provided vector is the wrong
    /// length.
    pub fn lagrange_from_vec(&self, values: Vec<F>) -> Polynomial<F, LagrangeCoeff> {
        assert_eq!(values.len(), self.n as usize);

        Polynomial {
            values,
            _marker: PhantomData,
        }
    }

    pub fn lagrange_assigned_from_vec(
        &self,
        values: Vec<Assigned<F>>,
    ) -> Polynomial<Assigned<F>, LagrangeCoeff> {
        assert_eq!(values.len(), self.n as usize);

        Polynomial {
            values,
            _marker: PhantomData,
        }
    }

    /// Obtains a polynomial in coefficient form when given a vector of
    /// coefficients of size `n`; panics if the provided vector is the wrong
    /// length.
    pub fn coeff_from_vec(&self, values: Vec<F>) -> Polynomial<F, Coeff> {
        assert_eq!(values.len(), self.n as usize);

        Polynomial {
            values,
            _marker: PhantomData,
        }
    }

    /// Obtains a polynomial in ExtendedLagrange form when given a vector of
    /// Lagrange polynomials with total size `extended_n`; panics if the
    /// provided vector is the wrong length.
    pub fn lagrange_vec_to_extended(
        &self,
        values: Vec<Polynomial<F, LagrangeCoeff>>,
    ) -> Polynomial<F, ExtendedLagrangeCoeff> {
        assert_eq!(values.len(), self.extended_len() >> self.k);
        assert_eq!(values[0].len(), self.n as usize);

        // transpose the values in parallel
        let mut transposed = vec![vec![F::ZERO; values.len()]; self.n as usize];
        values.into_iter().enumerate().for_each(|(i, p)| {
            parallelize(&mut transposed, |transposed, start| {
                for (transposed, p) in transposed.iter_mut().zip(p.values[start..].iter()) {
                    transposed[i] = *p;
                }
            });
        });

        Polynomial {
            values: transposed.into_iter().flatten().collect(),
            _marker: PhantomData,
        }
    }

    /// Obtains a polynomial in ExtendedLagrange form when given a vector of
    /// Lagrange polynomials with total size `extended_n`; panics if the
    /// provided vector is the wrong length.
    pub fn extended_from_lagrange_vec(
        &self,
        values: Vec<Polynomial<F, LagrangeCoeff>>,
    ) -> Polynomial<F, ExtendedLagrangeCoeff> {
        assert_eq!(values.len(), self.extended_len() >> self.k);
        assert_eq!(values[0].len(), self.n as usize);

        // transpose the values in parallel
        let mut transposed = vec![vec![F::ZERO; values.len()]; self.n as usize];
        values.into_iter().enumerate().for_each(|(i, p)| {
            parallelize(&mut transposed, |transposed, start| {
                for (transposed, p) in transposed.iter_mut().zip(p.values[start..].iter()) {
                    transposed[i] = *p;
                }
            });
        });

        Polynomial {
            values: transposed.into_iter().flatten().collect(),
            _marker: PhantomData,
        }
    }

    /// Returns an empty (zero) polynomial in the coefficient basis
    pub fn empty_coeff(&self) -> Polynomial<F, Coeff> {
        Polynomial {
            values: vec![F::ZERO; self.n as usize],
            _marker: PhantomData,
        }
    }

    /// Returns an empty (zero) polynomial in the Lagrange coefficient basis
    pub fn empty_lagrange(&self) -> Polynomial<F, LagrangeCoeff> {
        Polynomial {
            values: vec![F::ZERO; self.n as usize],
            _marker: PhantomData,
        }
    }

    /// Returns an empty (zero) polynomial in the Lagrange coefficient basis, with
    /// deferred inversions.
    pub(crate) fn empty_lagrange_assigned(&self) -> Polynomial<Assigned<F>, LagrangeCoeff> {
        Polynomial {
            values: vec![F::ZERO.into(); self.n as usize],
            _marker: PhantomData,
        }
    }

    /// Returns a constant polynomial in the Lagrange coefficient basis
    pub fn constant_lagrange(&self, scalar: F) -> Polynomial<F, LagrangeCoeff> {
        Polynomial {
            values: vec![scalar; self.n as usize],
            _marker: PhantomData,
        }
    }

    /// Returns an empty (zero) polynomial in the extended Lagrange coefficient
    /// basis
    pub fn empty_extended(&self) -> Polynomial<F, ExtendedLagrangeCoeff> {
        Polynomial {
            values: vec![F::ZERO; self.extended_len()],
            _marker: PhantomData,
        }
    }

    /// Returns a constant polynomial in the extended Lagrange coefficient
    /// basis
    pub fn constant_extended(&self, scalar: F) -> Polynomial<F, ExtendedLagrangeCoeff> {
        Polynomial {
            values: vec![scalar; self.extended_len()],
            _marker: PhantomData,
        }
    }

    /// This takes us from an n-length vector into the coefficient form.
    ///
    /// This function will panic if the provided vector is not the correct
    /// length.
    pub fn lagrange_to_coeff(&self, mut a: Polynomial<F, LagrangeCoeff>) -> Polynomial<F, Coeff> {
        assert_eq!(a.values.len(), 1 << self.k);

        // Perform inverse FFT to obtain the polynomial in coefficient form
        self.ifft(&mut a.values, self.omega_inv, self.k, self.ifft_divisor);

        Polynomial {
            values: a.values,
            _marker: PhantomData,
        }
    }

    /// Constructs the first Lagrange basis polynomial `l_0(X)` directly in coefficient form.
    ///
    /// For an evaluation domain of size `n`, `l_0(X)` simplifies to
    /// `(X^n - 1) / (n * (X - 1))`, which has constant coefficients `1 / n`.
    /// Generating the coefficients explicitly avoids an inverse FFT when only
    /// the zeroth basis element is required.
    pub fn lagrange_basis_0_coeff(&self) -> Polynomial<F, Coeff> {
        let mut coeff = self.empty_coeff();
        let inv_n = F::from(self.n)
            .invert()
            .expect("domain size is non-zero and not divisible by the field characteristic");
        parallelize(&mut coeff.values, |chunk, _| {
            for value in chunk.iter_mut() {
                *value = inv_n;
            }
        });
        coeff
    }

    /// This takes us from an n-length coefficient vector into a coset of the extended
    /// evaluation domain, rotating by `rotation` if desired.
    pub fn coeff_to_extended(
        &self,
        p: &Polynomial<F, Coeff>,
    ) -> Polynomial<F, ExtendedLagrangeCoeff> {
        assert_eq!(p.values.len(), 1 << self.k);

        let mut a = Vec::with_capacity(self.extended_len());
        a.extend(&p.values);

        self.distribute_powers_zeta(&mut a, true);
        a.resize(self.extended_len(), F::ZERO);
        self.fft_inner(&mut a, self.extended_omega, self.extended_k, false);

        Polynomial {
            values: a,
            _marker: PhantomData,
        }
    }

    /// This takes us from an n-length coefficient vector into parts of the
    /// extended evaluation domain. For example, for a polynomial with size n,
    /// and an extended domain of size mn, we can compute all parts
    /// independently, which are
    ///     `FFT(f(zeta * X), n)`
    ///     `FFT(f(zeta * extended_omega * X), n)`
    ///     ...
    ///     `FFT(f(zeta * extended_omega^{m-1} * X), n)`
    pub fn coeff_to_extended_parts(
        &self,
        a: &Polynomial<F, Coeff>,
    ) -> Vec<Polynomial<F, LagrangeCoeff>> {
        assert_eq!(a.values.len(), 1 << self.k);

        let num_parts = self.extended_len() >> self.k;
        let mut extended_omega_factor = F::ONE;
        (0..num_parts)
            .map(|_| {
                let part = self.coeff_to_extended_part(a.clone(), extended_omega_factor);
                extended_omega_factor *= self.extended_omega;
                part
            })
            .collect()
    }

    /// This takes us from several n-length coefficient vectors each into parts
    /// of the extended evaluation domain. For example, for a polynomial with
    /// size n, and an extended domain of size mn, we can compute all parts
    /// independently, which are
    ///     `FFT(f(zeta * X), n)`
    ///     `FFT(f(zeta * extended_omega * X), n)`
    ///     ...
    ///     `FFT(f(zeta * extended_omega^{m-1} * X), n)`
    pub fn batched_coeff_to_extended_parts(
        &self,
        a: &[Polynomial<F, Coeff>],
    ) -> Vec<Vec<Polynomial<F, LagrangeCoeff>>> {
        assert_eq!(a[0].values.len(), 1 << self.k);

        let mut extended_omega_factor = F::ONE;
        let num_parts = self.extended_len() >> self.k;
        (0..num_parts)
            .map(|_| {
                let a_lagrange = a
                    .iter()
                    .map(|poly| self.coeff_to_extended_part(poly.clone(), extended_omega_factor))
                    .collect();
                extended_omega_factor *= self.extended_omega;
                a_lagrange
            })
            .collect()
    }

    /// This takes us from an n-length coefficient vector into a part of the
    /// extended evaluation domain. For example, for a polynomial with size n,
    /// and an extended domain of size mn, we can compute one of the m parts
    /// separately, which is
    ///     `FFT(f(zeta * extended_omega_factor * X), n)`
    /// where `extended_omega_factor` is `extended_omega^i` with `i` in `[0, m)`.
    pub fn coeff_to_extended_part(
        &self,
        mut a: Polynomial<F, Coeff>,
        extended_omega_factor: F,
    ) -> Polynomial<F, LagrangeCoeff> {
        assert_eq!(a.values.len(), 1 << self.k);

        self.distribute_powers(&mut a.values, self.g_coset * extended_omega_factor);
        let data = self.get_fft_data(a.len());
        best_fft(&mut a.values, self.omega, self.k, data, false);

        Polynomial {
            values: a.values,
            _marker: PhantomData,
        }
    }

    /// Rotate the extended domain polynomial over the original domain.
    pub fn rotate_extended(
        &self,
        poly: &Polynomial<F, ExtendedLagrangeCoeff>,
        rotation: Rotation,
    ) -> Polynomial<F, ExtendedLagrangeCoeff> {
        let new_rotation = ((1 << (self.extended_k - self.k)) * rotation.0.abs()) as usize;

        let mut poly = poly.clone();

        if rotation.0 >= 0 {
            poly.values.rotate_left(new_rotation);
        } else {
            poly.values.rotate_right(new_rotation);
        }

        poly
    }

    /// This takes us from the extended evaluation domain and gets us the
    /// quotient polynomial coefficients.
    ///
    /// This function will panic if the provided vector is not the correct
    /// length. The resulting coefficient vector retains the full extended
    /// domain size; callers that require an $n \cdot$ `quotient_poly_degree`
    /// prefix must truncate explicitly.
    pub fn extended_to_coeff(&self, mut a: Polynomial<F, ExtendedLagrangeCoeff>) -> Vec<F> {
        assert_eq!(a.values.len(), self.extended_len());

        // Inverse FFT
        self.ifft(
            &mut a.values,
            self.extended_omega_inv,
            self.extended_k,
            self.extended_ifft_divisor,
        );

        // Distribute powers to move from coset; opposite from the
        // transformation we performed earlier.
        self.distribute_powers_zeta(&mut a.values, false);

        // The evaluation domain might be slightly larger than necessary because
        // it always lies on a power-of-two boundary. We no longer truncate here
        // so downstream callers can decide how much of the quotient they need.
        a.values
    }

    /// This takes us from the a list of lagrange-based polynomials with
    /// different degrees and gets their extended lagrange-based summation.
    pub fn lagrange_vecs_to_extended(
        &self,
        mut a: Vec<Vec<Polynomial<F, LagrangeCoeff>>>,
    ) -> Polynomial<F, ExtendedLagrangeCoeff> {
        let mut result_poly = if a[a.len() - 1].len() == 1 << (self.extended_k - self.k) {
            self.lagrange_vec_to_extended(a.pop().unwrap())
        } else {
            self.empty_extended()
        };

        // Transform from each cluster of lagrange representations to coeff representations.
        let mut ifft_divisor = self.extended_ifft_divisor;
        let mut omega_inv = self.extended_omega_inv;
        {
            let mut i = a.last().unwrap().len() << self.k;
            while i < (1 << self.extended_k) {
                ifft_divisor = ifft_divisor + ifft_divisor;
                omega_inv = omega_inv * omega_inv;
                i <<= 1;
            }
        }

        let mut result = vec![F::ZERO; 1 << self.extended_k as usize];
        for (i, a_parts) in a.into_iter().enumerate().rev() {
            // transpose the values in parallel
            assert_eq!(1 << i, a_parts.len());
            let mut a_poly: Vec<F> = {
                let mut transposed = vec![vec![F::ZERO; a_parts.len()]; self.n as usize];
                a_parts.into_iter().enumerate().for_each(|(j, p)| {
                    parallelize(&mut transposed, |transposed, start| {
                        for (transposed, p) in transposed.iter_mut().zip(p.values[start..].iter()) {
                            transposed[j] = *p;
                        }
                    });
                });
                transposed.into_iter().flatten().collect()
            };

            self.ifft(&mut a_poly, omega_inv, self.k + i as u32, ifft_divisor);
            ifft_divisor = ifft_divisor + ifft_divisor;
            omega_inv = omega_inv * omega_inv;

            parallelize(&mut result[0..(self.n << i) as usize], |result, start| {
                for (other, current) in result.iter_mut().zip(a_poly[start..].iter()) {
                    *other += current;
                }
            });
        }
        let data = self.get_fft_data(result.len());
        best_fft(
            &mut result,
            self.extended_omega,
            self.extended_k,
            data,
            false,
        );
        parallelize(&mut result_poly.values, |values, start| {
            for (value, other) in values.iter_mut().zip(result[start..].iter()) {
                *value += other;
            }
        });
        result_poly
    }

    /// This divides the polynomial (in the extended domain) by the vanishing
    /// polynomial of the $2^k$ size domain.
    pub fn divide_by_vanishing_poly(
        &self,
        mut a: Polynomial<F, ExtendedLagrangeCoeff>,
    ) -> Polynomial<F, ExtendedLagrangeCoeff> {
        assert_eq!(a.values.len(), self.extended_len());

        // Divide to obtain the quotient polynomial in the coset evaluation
        // domain.
        parallelize(&mut a.values, |h, mut index| {
            for h in h {
                *h *= &self.t_evaluations[index % self.t_evaluations.len()];
                index += 1;
            }
        });

        Polynomial {
            values: a.values,
            _marker: PhantomData,
        }
    }

    /// Given a slice of group elements `[a_0, a_1, a_2, ...]`, this returns
    /// `[a_0, [zeta]a_1, [zeta^2]a_2, a_3, [zeta]a_4, [zeta^2]a_5, a_6, ...]`,
    /// where zeta is a cube root of unity in the multiplicative subgroup with
    /// order (p - 1), i.e. zeta^3 = 1.
    ///
    /// `into_coset` should be set to `true` when moving into the coset,
    /// and `false` when moving out. This toggles the choice of `zeta`.
    fn distribute_powers_zeta(&self, a: &mut [F], into_coset: bool) {
        let coset_powers = if into_coset {
            [self.g_coset, self.g_coset_inv]
        } else {
            [self.g_coset_inv, self.g_coset]
        };
        parallelize(a, |a, mut index| {
            for a in a {
                // Distribute powers to move into/from coset
                let i = index % (coset_powers.len() + 1);
                if i != 0 {
                    *a *= &coset_powers[i - 1];
                }
                index += 1;
            }
        });
    }

    /// Given a slice of group elements `[a_0, a_1, a_2, ...]`, this returns
    /// `[a_0, [c]a_1, [c^2]a_2, [c^3]a_3, [c^4]a_4, ...]`,
    ///
    fn distribute_powers(&self, a: &mut [F], c: F) {
        parallelize(a, |a, index| {
            let mut c_power = c.pow_vartime([index as u64]);
            for a in a {
                *a *= c_power;
                c_power *= c;
            }
        });
    }

    fn ifft(&self, a: &mut Vec<F>, omega_inv: F, log_n: u32, divisor: F) {
        let fft_data = self.get_fft_data(a.len());
        crate::fft::parallel::fft(a, omega_inv, log_n, fft_data, true);
        // self.fft_inner(a, omega_inv, log_n, true);
        parallelize(a, |a, _| {
            for a in a {
                // Finish iFFT
                *a *= &divisor;
            }
        });
    }

    fn fft_inner(&self, a: &mut Vec<F>, omega: F, log_n: u32, inverse: bool) {
        let fft_data = self.get_fft_data(a.len());
        best_fft(a, omega, log_n, fft_data, inverse)
    }

    /// Get the size of the domain
    pub fn k(&self) -> u32 {
        self.k
    }

    /// Get the size of the extended domain
    pub fn extended_k(&self) -> u32 {
        self.extended_k
    }

    /// Get the size of the extended domain
    pub fn extended_len(&self) -> usize {
        1 << self.extended_k
    }

    /// Get $\omega$, the generator of the $2^k$ order multiplicative subgroup.
    pub fn get_omega(&self) -> F {
        self.omega
    }

    /// Get $\omega^{-1}$, the inverse of the generator of the $2^k$ order
    /// multiplicative subgroup.
    pub fn get_omega_inv(&self) -> F {
        self.omega_inv
    }

    /// Get the generator of the extended domain's multiplicative subgroup.
    pub fn get_extended_omega(&self) -> F {
        self.extended_omega
    }

    /// Multiplies a value by some power of $\omega$, essentially rotating over
    /// the domain.
    pub fn rotate_omega(&self, value: F, rotation: Rotation) -> F {
        let mut point = value;
        if rotation.0 >= 0 {
            point *= &self.get_omega().pow_vartime([rotation.0 as u64]);
        } else {
            point *= &self
                .get_omega_inv()
                .pow_vartime([(rotation.0 as i64).unsigned_abs()]);
        }
        point
    }

    /// Computes evaluations (at the point `x`, where `xn = x^n`) of Lagrange
    /// basis polynomials `l_i(X)` defined such that `l_i(omega^i) = 1` and
    /// `l_i(omega^j) = 0` for all `j != i` at each provided rotation `i`.
    ///
    /// # Implementation
    ///
    /// The polynomial
    ///     $$\prod_{j=0,j \neq i}^{n - 1} (X - \omega^j)$$
    /// has a root at all points in the domain except $\omega^i$, where it evaluates to
    ///     $$\prod_{j=0,j \neq i}^{n - 1} (\omega^i - \omega^j)$$
    /// and so we divide that polynomial by this value to obtain $l_i(X)$. Since
    ///     $$\prod_{j=0,j \neq i}^{n - 1} (X - \omega^j)
    ///       = \frac{X^n - 1}{X - \omega^i}$$
    /// then $l_i(x)$ for some $x$ is evaluated as
    ///     $$\left(\frac{x^n - 1}{x - \omega^i}\right)
    ///       \cdot \left(\frac{1}{\prod_{j=0,j \neq i}^{n - 1} (\omega^i - \omega^j)}\right).$$
    /// We refer to
    ///     $$1 \over \prod_{j=0,j \neq i}^{n - 1} (\omega^i - \omega^j)$$
    /// as the barycentric weight of $\omega^i$.
    ///
    /// We know that for $i = 0$
    ///     $$\frac{1}{\prod_{j=0,j \neq i}^{n - 1} (\omega^i - \omega^j)} = \frac{1}{n}.$$
    ///
    /// If we multiply $(1 / n)$ by $\omega^i$ then we obtain
    ///     $$\frac{1}{\prod_{j=0,j \neq 0}^{n - 1} (\omega^i - \omega^j)}
    ///       = \frac{1}{\prod_{j=0,j \neq i}^{n - 1} (\omega^i - \omega^j)}$$
    /// which is the barycentric weight of $\omega^i$.
    pub fn l_i_range<I: IntoIterator<Item = i32> + Clone>(
        &self,
        x: F,
        xn: F,
        rotations: I,
    ) -> Vec<F> {
        let mut results;
        {
            let rotations = rotations.clone().into_iter();
            results = Vec::with_capacity(rotations.size_hint().1.unwrap_or(0));
            for rotation in rotations {
                let rotation = Rotation(rotation);
                let result = x - self.rotate_omega(F::ONE, rotation);
                results.push(result);
            }
            results.iter_mut().batch_invert();
        }

        let common = (xn - F::ONE) * self.barycentric_weight;
        for (rotation, result) in rotations.into_iter().zip(results.iter_mut()) {
            let rotation = Rotation(rotation);
            *result = self.rotate_omega(*result * common, rotation);
        }

        results
    }

    /// Gets the quotient polynomial's degree (as a multiple of n)
    pub fn get_quotient_poly_degree(&self) -> usize {
        self.quotient_poly_degree as usize
    }

    /// Obtain a pinned version of this evaluation domain; a structure with the
    /// minimal parameters needed to determine the rest of the evaluation
    /// domain.
    pub fn pinned(&self) -> PinnedEvaluationDomain<'_, F> {
        PinnedEvaluationDomain {
            k: &self.k,
            extended_k: &self.extended_k,
            omega: &self.omega,
        }
    }

    /// Get the private field `n`
    pub fn get_n(&self) -> u64 {
        self.n
    }

    /// Get the private `fft_data`
    pub fn get_fft_data(&self, l: usize) -> &FFTData<F> {
        self.fft_data
            .get(&l)
            .expect("log_2(l) must be in k..=extended_k")
    }
}

/// Represents the minimal parameters that determine an `EvaluationDomain`.
#[allow(dead_code)]
#[derive(Debug)]
pub struct PinnedEvaluationDomain<'a, F: Field> {
    k: &'a u32,
    extended_k: &'a u32,
    omega: &'a F,
}

#[test]
fn test_rotate() {
    use rand_core::OsRng;

    use crate::arithmetic::eval_polynomial;
    use halo2curves::pasta::pallas::Scalar;

    let domain = EvaluationDomain::<Scalar>::new(1, 3);
    let rng = OsRng;

    let mut poly = domain.empty_lagrange();
    assert_eq!(poly.len(), 8);
    for value in poly.iter_mut() {
        *value = Scalar::random(rng);
    }

    let poly_rotated_cur = poly.rotate(Rotation::cur());
    let poly_rotated_next = poly.rotate(Rotation::next());
    let poly_rotated_prev = poly.rotate(Rotation::prev());

    let poly = domain.lagrange_to_coeff(poly);
    let poly_rotated_cur = domain.lagrange_to_coeff(poly_rotated_cur);
    let poly_rotated_next = domain.lagrange_to_coeff(poly_rotated_next);
    let poly_rotated_prev = domain.lagrange_to_coeff(poly_rotated_prev);

    let x = Scalar::random(rng);

    assert_eq!(
        eval_polynomial(&poly[..], x),
        eval_polynomial(&poly_rotated_cur[..], x)
    );
    assert_eq!(
        eval_polynomial(&poly[..], x * domain.omega),
        eval_polynomial(&poly_rotated_next[..], x)
    );
    assert_eq!(
        eval_polynomial(&poly[..], x * domain.omega_inv),
        eval_polynomial(&poly_rotated_prev[..], x)
    );
}

#[test]
fn extended_to_coeff_retains_extended_length() {
    use core::convert::TryFrom;
    use halo2curves::pasta::pallas::Scalar;

    // With (j = 4, k = 3) we have n = 8 and quotient degree = 3, so the
    // quotient span is 24 while the extended domain rounds up to 32.
    let domain = EvaluationDomain::<Scalar>::new(4, 3);
    let coeffs = domain.extended_to_coeff(domain.empty_extended());

    assert_eq!(coeffs.len(), domain.extended_len());

    let quotient_len = usize::try_from(domain.get_n()).expect("domain size fits usize")
        * domain.get_quotient_poly_degree();
    assert!(
        quotient_len < coeffs.len(),
        "expected extended domain to exceed quotient length"
    );
}

#[test]
fn test_l_i() {
    use rand_core::OsRng;

    use crate::arithmetic::{eval_polynomial, lagrange_interpolate};
    use halo2curves::pasta::pallas::Scalar;
    let domain = EvaluationDomain::<Scalar>::new(1, 3);

    let mut l = vec![];
    let mut points = vec![];
    for i in 0..8 {
        points.push(domain.omega.pow([i]));
    }
    for i in 0..8 {
        let mut l_i = vec![Scalar::zero(); 8];
        l_i[i] = Scalar::ONE;
        let l_i = lagrange_interpolate(&points[..], &l_i[..]);
        l.push(l_i);
    }

    let x = Scalar::random(OsRng);
    let xn = x.pow([8]);

    let evaluations = domain.l_i_range(x, xn, -7..=7);
    for i in 0..8 {
        assert_eq!(eval_polynomial(&l[i][..], x), evaluations[7 + i]);
        assert_eq!(eval_polynomial(&l[(8 - i) % 8][..], x), evaluations[7 - i]);
    }
}

#[test]
fn lagrange_basis_0_matches_coeff_fft() {
    use halo2curves::pasta::pallas::Scalar;

    let domain = EvaluationDomain::<Scalar>::new(1, 4);

    let mut lagrange = domain.empty_lagrange();
    lagrange[0] = Scalar::ONE;
    let expected = domain.lagrange_to_coeff(lagrange);

    let optimized = domain.lagrange_basis_0_coeff();

    assert_eq!(optimized.values, expected.values);
}

#[test]
fn test_coeff_to_extended_part() {
    use halo2curves::pasta::pallas::Scalar;
    use rand_core::OsRng;

    let domain = EvaluationDomain::<Scalar>::new(1, 3);
    let rng = OsRng;
    let mut poly = domain.empty_coeff();
    assert_eq!(poly.len(), 8);
    for value in poly.iter_mut() {
        *value = Scalar::random(rng);
    }

    let want = domain.coeff_to_extended(&poly);
    let got = {
        let parts = domain.coeff_to_extended_parts(&poly);
        domain.lagrange_vec_to_extended(parts)
    };
    assert_eq!(want.values, got.values);
}

#[test]
fn bench_coeff_to_extended_parts() {
    use halo2curves::pasta::pallas::Scalar;
    use rand_core::OsRng;
    use std::time::Instant;

    let k = 20;
    let domain = EvaluationDomain::<Scalar>::new(3, k);
    let rng = OsRng;
    let mut poly1 = domain.empty_coeff();
    assert_eq!(poly1.len(), 1 << k);

    for value in poly1.iter_mut() {
        *value = Scalar::random(rng);
    }

    let poly2 = poly1.clone();

    let coeff_to_extended_timer = Instant::now();
    let _ = domain.coeff_to_extended(&poly1);
    println!(
        "domain.coeff_to_extended time: {}s",
        coeff_to_extended_timer.elapsed().as_secs_f64()
    );

    let coeff_to_extended_parts_timer = Instant::now();
    let _ = domain.coeff_to_extended_parts(&poly2);
    println!(
        "domain.coeff_to_extended_parts time: {}s",
        coeff_to_extended_parts_timer.elapsed().as_secs_f64()
    );
}

#[test]
fn test_lagrange_vecs_to_extended() {
    use halo2curves::pasta::pallas::Scalar;
    use rand_core::OsRng;

    let rng = OsRng;
    let domain = EvaluationDomain::<Scalar>::new(8, 10);
    let mut poly_vec = vec![];
    let mut poly_lagrange_vecs = vec![];
    let mut want = domain.empty_extended();
    let mut omega = domain.extended_omega;
    for i in (0..(domain.extended_k - domain.k + 1)).rev() {
        let mut poly = vec![Scalar::zero(); (1 << i) * domain.n as usize];
        for value in poly.iter_mut() {
            *value = Scalar::random(rng);
        }
        // poly under coeff representation.
        poly_vec.push(poly.clone());
        // poly under lagrange vector representation.
        let mut poly2 = poly.clone();
        let data = domain.get_fft_data(poly2.len());
        best_fft(&mut poly2, omega, i + domain.k, data, false);
        let transposed_poly: Vec<Polynomial<Scalar, LagrangeCoeff>> = (0..(1 << i))
            .map(|j| {
                let mut p = domain.empty_lagrange();
                for k in 0..domain.n {
                    p[k as usize] = poly2[j + (k as usize) * (1 << i)];
                }
                p
            })
            .collect();
        poly_lagrange_vecs.push(transposed_poly);
        // poly under extended representation.
        poly.resize(domain.extended_len(), Scalar::zero());
        let data = domain.get_fft_data(poly.len());
        best_fft(
            &mut poly,
            domain.extended_omega,
            domain.extended_k,
            data,
            false,
        );
        let poly = {
            let mut p = domain.empty_extended();
            p.values = poly;
            p
        };
        want = want + &poly;
        omega = omega * omega;
    }
    poly_lagrange_vecs.reverse();
    let got = domain.lagrange_vecs_to_extended(poly_lagrange_vecs);
    assert_eq!(want.values, got.values);
}

#[test]
fn bench_lagrange_vecs_to_extended() {
    use halo2curves::pasta::pallas::Scalar;
    use rand_core::OsRng;
    use std::time::Instant;

    let rng = OsRng;
    let domain = EvaluationDomain::<Scalar>::new(8, 10);
    let mut poly_vec = vec![];
    let mut poly_lagrange_vecs = vec![];
    let mut poly_extended_vecs = vec![];
    let mut omega = domain.extended_omega;

    for i in (0..(domain.extended_k - domain.k + 1)).rev() {
        let mut poly = vec![Scalar::zero(); (1 << i) * domain.n as usize];
        for value in poly.iter_mut() {
            *value = Scalar::random(rng);
        }
        // poly under coeff representation.
        poly_vec.push(poly.clone());
        // poly under lagrange vector representation.
        let mut poly2 = poly.clone();
        let data = domain.get_fft_data(poly2.len());
        best_fft(&mut poly2, omega, i + domain.k, data, false);
        let transposed_poly: Vec<Polynomial<Scalar, LagrangeCoeff>> = (0..(1 << i))
            .map(|j| {
                let mut p = domain.empty_lagrange();
                for k in 0..domain.n {
                    p[k as usize] = poly2[j + (k as usize) * (1 << i)];
                }
                p
            })
            .collect();
        poly_lagrange_vecs.push(transposed_poly);
        // poly under extended representation.
        poly.resize(domain.extended_len(), Scalar::zero());
        let data = domain.get_fft_data(poly.len());
        best_fft(
            &mut poly,
            domain.extended_omega,
            domain.extended_k,
            data,
            false,
        );
        let poly = {
            let mut p = domain.empty_extended();
            p.values = poly;
            p
        };
        poly_extended_vecs.push(poly);
        omega = omega * omega;
    }

    let want_timer = Instant::now();
    let _ = poly_extended_vecs
        .iter()
        .fold(domain.empty_extended(), |acc, p| acc + p);
    println!("want time: {}s", want_timer.elapsed().as_secs_f64());
    poly_lagrange_vecs.reverse();
    let got_timer = Instant::now();
    let _ = domain.lagrange_vecs_to_extended(poly_lagrange_vecs);
    println!("got time: {}s", got_timer.elapsed().as_secs_f64());
}
