//! Fixed RNS arithmetic for the Jindo application rings.
//!
//! Both rings are `Z_q[X]/(X^1024 + 1)`.  The two inner 46-bit primes and two
//! outer 36-bit primes are pinned, prime, pairwise distinct, and congruent to
//! one modulo 2048. Their pinned primitive 2048th roots make negacyclic NTT
//! multiplication deterministic across every target without native-width
//! overflow.

use super::JINDO_RING_DEGREE_V1;
use zeroize::Zeroize;

/// One pinned NTT prime and primitive `2d`-th root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JindoPrimeModulusV1 {
    modulus: u64,
    psi: u64,
}

impl JindoPrimeModulusV1 {
    pub(crate) const fn new(modulus: u64, psi: u64) -> Self {
        Self { modulus, psi }
    }

    pub(crate) const fn modulus(self) -> u64 {
        self.modulus
    }
}

/// Inner-commitment modulus `q` from the current N=256, batch=4 reference profile.
pub(crate) const JINDO_INNER_MODULI_V1: [JindoPrimeModulusV1; 2] = [
    JindoPrimeModulusV1::new(70_368_744_067_073, 22_701_904_919_461),
    JindoPrimeModulusV1::new(70_368_744_183_809, 12_022_014_596_385),
];

/// Outer-commitment modulus `q_o` from the same closed profile.
pub(crate) const JINDO_OUTER_MODULI_V1: [JindoPrimeModulusV1; 2] = [
    JindoPrimeModulusV1::new(48_591_984_641, 25_236_428_417),
    JindoPrimeModulusV1::new(48_592_009_217, 4_690_178_537),
];

/// One application-ring element in a fixed two-prime RNS basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct JindoRnsPolynomialV1 {
    residues: [[u64; JINDO_RING_DEGREE_V1]; 2],
}

impl Zeroize for JindoRnsPolynomialV1 {
    fn zeroize(&mut self) {
        self.residues.zeroize();
    }
}

impl Default for JindoRnsPolynomialV1 {
    fn default() -> Self {
        Self::zero()
    }
}

impl JindoRnsPolynomialV1 {
    /// Additive identity.
    pub(crate) const fn zero() -> Self {
        Self {
            residues: [[0; JINDO_RING_DEGREE_V1]; 2],
        }
    }

    /// Build from two canonical residue rows.
    pub(crate) fn from_residues(
        residues: [[u64; JINDO_RING_DEGREE_V1]; 2],
        moduli: [JindoPrimeModulusV1; 2],
    ) -> Option<Self> {
        for (row, prime) in residues.iter().zip(moduli) {
            if row.iter().any(|coefficient| *coefficient >= prime.modulus) {
                return None;
            }
        }
        Some(Self { residues })
    }

    /// Build from balanced coefficients whose magnitudes fit below both RNS
    /// products.
    pub(crate) fn from_balanced_coefficients(
        coefficients: [i128; JINDO_RING_DEGREE_V1],
        moduli: [JindoPrimeModulusV1; 2],
    ) -> Self {
        let mut residues = [[0_u64; JINDO_RING_DEGREE_V1]; 2];
        for (row, prime) in residues.iter_mut().zip(moduli) {
            for (out, coefficient) in row.iter_mut().zip(coefficients) {
                let modulus = i128::from(prime.modulus);
                let reduced = coefficient % modulus;
                *out = if reduced < 0 {
                    (reduced + modulus) as u64
                } else {
                    reduced as u64
                };
            }
        }
        Self { residues }
    }

    /// Borrow both exact residue rows.
    pub(crate) const fn residues(&self) -> &[[u64; JINDO_RING_DEGREE_V1]; 2] {
        &self.residues
    }

    /// Add two ring elements.
    pub(crate) fn add_assign(&mut self, rhs: &Self, moduli: [JindoPrimeModulusV1; 2]) {
        for ((left_row, right_row), prime) in self
            .residues
            .iter_mut()
            .zip(rhs.residues.iter())
            .zip(moduli)
        {
            for (left, right) in left_row.iter_mut().zip(right_row) {
                *left = add_mod(*left, *right, prime.modulus);
            }
        }
    }

    /// Subtract two ring elements.
    pub(crate) fn sub_assign(&mut self, rhs: &Self, moduli: [JindoPrimeModulusV1; 2]) {
        for ((left_row, right_row), prime) in self
            .residues
            .iter_mut()
            .zip(rhs.residues.iter())
            .zip(moduli)
        {
            for (left, right) in left_row.iter_mut().zip(right_row) {
                *left = sub_mod(*left, *right, prime.modulus);
            }
        }
    }

    /// Multiply in `Z_q[X]/(X^1024 + 1)` using the pinned negacyclic NTT.
    pub(crate) fn mul(&self, rhs: &Self, moduli: [JindoPrimeModulusV1; 2]) -> Self {
        let mut residues = [[0_u64; JINDO_RING_DEGREE_V1]; 2];
        for (((out, left), right), prime) in residues
            .iter_mut()
            .zip(self.residues.iter())
            .zip(rhs.residues.iter())
            .zip(moduli)
        {
            *out = negacyclic_mul(*left, *right, prime);
        }
        Self { residues }
    }

    /// Multiply every coefficient by `2^exponent` in the selected ring.
    pub(crate) fn scale_power_of_two(
        &self,
        exponent: u32,
        moduli: [JindoPrimeModulusV1; 2],
    ) -> Self {
        let mut residues = self.residues;
        for ((output, input), prime) in residues.iter_mut().zip(self.residues.iter()).zip(moduli) {
            let scalar = pow_mod(2, u64::from(exponent), prime.modulus);
            for (output, input) in output.iter_mut().zip(input) {
                *output = mul_mod(*input, scalar, prime.modulus);
            }
        }
        Self { residues }
    }

    /// Return whether this ring element is the additive identity.
    pub(crate) fn is_zero(&self) -> bool {
        self.residues.iter().flatten().all(|residue| *residue == 0)
    }

    /// Return whether this element is a unit in every RNS ring factor.
    ///
    /// Each pinned prime splits `X^1024 + 1` completely. The twisted NTT is
    /// evaluation at its 1024 distinct roots, so an element is invertible if
    /// and only if every evaluation is non-zero in every CRT component.
    pub(crate) fn is_unit(&self, moduli: [JindoPrimeModulusV1; 2]) -> bool {
        self.residues
            .iter()
            .zip(moduli)
            .all(|(coefficients, prime)| {
                let modulus = prime.modulus;
                let mut evaluations = *coefficients;
                let mut twist = 1_u64;
                for value in &mut evaluations {
                    *value = mul_mod(*value, twist, modulus);
                    twist = mul_mod(twist, prime.psi, modulus);
                }
                cyclic_ntt(
                    &mut evaluations,
                    mul_mod(prime.psi, prime.psi, modulus),
                    modulus,
                );
                evaluations.iter().all(|value| *value != 0)
            })
    }

    /// Reconstruct one coefficient in `[0, q_0 q_1)`.
    pub(crate) fn reconstruct_coefficient(
        &self,
        coefficient_index: usize,
        moduli: [JindoPrimeModulusV1; 2],
    ) -> u128 {
        debug_assert!(coefficient_index < JINDO_RING_DEGREE_V1);
        crt_reconstruct(
            self.residues[0][coefficient_index],
            self.residues[1][coefficient_index],
            moduli,
        )
    }

    /// Return the absolute value of the balanced CRT representative.
    #[cfg(test)]
    pub(crate) fn balanced_abs_coefficient(
        &self,
        coefficient_index: usize,
        moduli: [JindoPrimeModulusV1; 2],
    ) -> u128 {
        let value = self.reconstruct_coefficient(coefficient_index, moduli);
        let product = u128::from(moduli[0].modulus) * u128::from(moduli[1].modulus);
        value.min(product - value)
    }

    /// Return the unique balanced representative in `[-q/2, q/2]`.
    pub(crate) fn balanced_coefficient(
        &self,
        coefficient_index: usize,
        moduli: [JindoPrimeModulusV1; 2],
    ) -> i128 {
        let value = self.reconstruct_coefficient(coefficient_index, moduli);
        let product = u128::from(moduli[0].modulus) * u128::from(moduli[1].modulus);
        if value > product / 2 {
            value as i128 - product as i128
        } else {
            value as i128
        }
    }
}

fn add_mod(left: u64, right: u64, modulus: u64) -> u64 {
    let sum = left + right;
    if sum >= modulus { sum - modulus } else { sum }
}

fn sub_mod(left: u64, right: u64, modulus: u64) -> u64 {
    if left >= right {
        left - right
    } else {
        modulus - (right - left)
    }
}

fn mul_mod(left: u64, right: u64, modulus: u64) -> u64 {
    (u128::from(left) * u128::from(right) % u128::from(modulus)) as u64
}

fn pow_mod(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = mul_mod(result, base, modulus);
        }
        base = mul_mod(base, base, modulus);
        exponent >>= 1;
    }
    result
}

fn invert_mod(value: u64, modulus: u64) -> u64 {
    debug_assert_ne!(value, 0);
    pow_mod(value, modulus - 2, modulus)
}

fn cyclic_ntt(values: &mut [u64; JINDO_RING_DEGREE_V1], root: u64, modulus: u64) {
    let mut target = 0_usize;
    for source in 1..JINDO_RING_DEGREE_V1 {
        let mut bit = JINDO_RING_DEGREE_V1 >> 1;
        while target & bit != 0 {
            target ^= bit;
            bit >>= 1;
        }
        target ^= bit;
        if source < target {
            values.swap(source, target);
        }
    }

    let mut width = 2_usize;
    while width <= JINDO_RING_DEGREE_V1 {
        let twiddle_step = pow_mod(root, (JINDO_RING_DEGREE_V1 / width) as u64, modulus);
        for start in (0..JINDO_RING_DEGREE_V1).step_by(width) {
            let mut twiddle = 1_u64;
            for offset in 0..(width / 2) {
                let even = values[start + offset];
                let odd = mul_mod(values[start + offset + width / 2], twiddle, modulus);
                values[start + offset] = add_mod(even, odd, modulus);
                values[start + offset + width / 2] = sub_mod(even, odd, modulus);
                twiddle = mul_mod(twiddle, twiddle_step, modulus);
            }
        }
        width *= 2;
    }
}

fn negacyclic_mul(
    mut left: [u64; JINDO_RING_DEGREE_V1],
    mut right: [u64; JINDO_RING_DEGREE_V1],
    prime: JindoPrimeModulusV1,
) -> [u64; JINDO_RING_DEGREE_V1] {
    let modulus = prime.modulus;
    let inverse_psi = invert_mod(prime.psi, modulus);
    let omega = mul_mod(prime.psi, prime.psi, modulus);
    let inverse_omega = invert_mod(omega, modulus);

    let mut twist = 1_u64;
    for (left_coefficient, right_coefficient) in left.iter_mut().zip(right.iter_mut()) {
        *left_coefficient = mul_mod(*left_coefficient, twist, modulus);
        *right_coefficient = mul_mod(*right_coefficient, twist, modulus);
        twist = mul_mod(twist, prime.psi, modulus);
    }

    cyclic_ntt(&mut left, omega, modulus);
    cyclic_ntt(&mut right, omega, modulus);
    for (left_value, right_value) in left.iter_mut().zip(right) {
        *left_value = mul_mod(*left_value, right_value, modulus);
    }
    cyclic_ntt(&mut left, inverse_omega, modulus);

    let inverse_degree = invert_mod(JINDO_RING_DEGREE_V1 as u64, modulus);
    let mut inverse_twist = 1_u64;
    for value in &mut left {
        *value = mul_mod(
            mul_mod(*value, inverse_degree, modulus),
            inverse_twist,
            modulus,
        );
        inverse_twist = mul_mod(inverse_twist, inverse_psi, modulus);
    }
    left
}

fn crt_reconstruct(residue_zero: u64, residue_one: u64, moduli: [JindoPrimeModulusV1; 2]) -> u128 {
    let q0 = moduli[0].modulus;
    let q1 = moduli[1].modulus;
    let residue_zero_mod_q1 = residue_zero % q1;
    let difference = sub_mod(residue_one, residue_zero_mod_q1, q1);
    let q0_inverse_mod_q1 = invert_mod(q0 % q1, q1);
    let correction = mul_mod(difference, q0_inverse_mod_q1, q1);
    u128::from(residue_zero) + u128::from(q0) * u128::from(correction)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_prime_64(value: u64) -> bool {
        if value < 2 {
            return false;
        }
        for small in [2_u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
            if value % small == 0 {
                return value == small;
            }
        }
        let mut odd = value - 1;
        let powers = odd.trailing_zeros();
        odd >>= powers;
        for witness in [2_u64, 325, 9_375, 28_178, 450_775, 9_780_504, 1_795_265_022] {
            if witness % value == 0 {
                continue;
            }
            let mut candidate = pow_mod(witness % value, odd, value);
            if candidate == 1 || candidate == value - 1 {
                continue;
            }
            let mut accepted = false;
            for _ in 1..powers {
                candidate = mul_mod(candidate, candidate, value);
                if candidate == value - 1 {
                    accepted = true;
                    break;
                }
            }
            if !accepted {
                return false;
            }
        }
        true
    }

    fn naive_negacyclic(
        left: [u64; JINDO_RING_DEGREE_V1],
        right: [u64; JINDO_RING_DEGREE_V1],
        modulus: u64,
    ) -> [u64; JINDO_RING_DEGREE_V1] {
        let mut out = [0_u64; JINDO_RING_DEGREE_V1];
        for (left_index, left_value) in left.into_iter().enumerate() {
            for (right_index, right_value) in right.into_iter().enumerate() {
                let product = mul_mod(left_value, right_value, modulus);
                let index = left_index + right_index;
                if index < JINDO_RING_DEGREE_V1 {
                    out[index] = add_mod(out[index], product, modulus);
                } else {
                    out[index - JINDO_RING_DEGREE_V1] =
                        sub_mod(out[index - JINDO_RING_DEGREE_V1], product, modulus);
                }
            }
        }
        out
    }

    #[test]
    fn pinned_moduli_are_prime_distinct_and_ntt_friendly() {
        let all = [
            JINDO_INNER_MODULI_V1[0],
            JINDO_INNER_MODULI_V1[1],
            JINDO_OUTER_MODULI_V1[0],
            JINDO_OUTER_MODULI_V1[1],
        ];
        for (index, prime) in all.into_iter().enumerate() {
            assert!(is_prime_64(prime.modulus), "modulus {index}");
            assert_eq!((prime.modulus - 1) % 2048, 0);
            assert_eq!(pow_mod(prime.psi, 2048, prime.modulus), 1);
            assert_eq!(pow_mod(prime.psi, 1024, prime.modulus), prime.modulus - 1);
        }
        for left in 0..all.len() {
            for right in (left + 1)..all.len() {
                assert_ne!(all[left].modulus, all[right].modulus);
            }
        }
    }

    #[test]
    fn ntt_multiplication_matches_naive_negacyclic_convolution() {
        for prime in [
            JINDO_INNER_MODULI_V1[0],
            JINDO_INNER_MODULI_V1[1],
            JINDO_OUTER_MODULI_V1[0],
            JINDO_OUTER_MODULI_V1[1],
        ] {
            let mut left = [0_u64; JINDO_RING_DEGREE_V1];
            let mut right = [0_u64; JINDO_RING_DEGREE_V1];
            for index in 0..JINDO_RING_DEGREE_V1 {
                left[index] = ((index as u64).wrapping_mul(0x1_0000_01b3) + 17) % prime.modulus;
                right[index] = ((index as u64).wrapping_mul(0x9e37_79b9) + 29) % prime.modulus;
            }
            assert_eq!(
                negacyclic_mul(left, right, prime),
                naive_negacyclic(left, right, prime.modulus),
                "modulus {}",
                prime.modulus
            );
        }
    }

    #[test]
    fn negacyclic_wrap_has_the_required_negative_sign() {
        for prime in [JINDO_INNER_MODULI_V1[0], JINDO_OUTER_MODULI_V1[0]] {
            let mut left = [0_u64; JINDO_RING_DEGREE_V1];
            let mut right = [0_u64; JINDO_RING_DEGREE_V1];
            left[JINDO_RING_DEGREE_V1 - 1] = 7;
            right[1] = 11;
            let product = negacyclic_mul(left, right, prime);
            assert_eq!(product[0], prime.modulus - 77);
            assert!(product[1..].iter().all(|value| *value == 0));
        }
    }

    #[test]
    fn crt_roundtrips_boundary_and_balanced_representatives() {
        for moduli in [JINDO_INNER_MODULI_V1, JINDO_OUTER_MODULI_V1] {
            let product = u128::from(moduli[0].modulus) * u128::from(moduli[1].modulus);
            for value in [
                0_u128,
                1,
                u128::from(moduli[0].modulus) - 1,
                u128::from(moduli[0].modulus),
                product / 2,
                product - 2,
                product - 1,
            ] {
                let residue_zero = (value % u128::from(moduli[0].modulus)) as u64;
                let residue_one = (value % u128::from(moduli[1].modulus)) as u64;
                assert_eq!(
                    crt_reconstruct(residue_zero, residue_one, moduli),
                    value,
                    "value {value}"
                );
            }

            let mut residues = [[0_u64; JINDO_RING_DEGREE_V1]; 2];
            residues[0][0] = moduli[0].modulus - 1;
            residues[1][0] = moduli[1].modulus - 1;
            let polynomial =
                JindoRnsPolynomialV1::from_residues(residues, moduli).expect("canonical");
            assert_eq!(polynomial.balanced_abs_coefficient(0, moduli), 1);
        }
    }

    #[test]
    fn residue_decoder_rejects_noncanonical_coefficients() {
        let mut residues = [[0_u64; JINDO_RING_DEGREE_V1]; 2];
        residues[1][173] = JINDO_INNER_MODULI_V1[1].modulus;
        assert!(JindoRnsPolynomialV1::from_residues(residues, JINDO_INNER_MODULI_V1).is_none());
    }

    #[test]
    fn ring_add_sub_mul_obey_basic_identities() {
        let moduli = JINDO_INNER_MODULI_V1;
        let mut left_rows = [[0_u64; JINDO_RING_DEGREE_V1]; 2];
        let mut right_rows = [[0_u64; JINDO_RING_DEGREE_V1]; 2];
        for (row_index, prime) in moduli.into_iter().enumerate() {
            for coefficient in 0..JINDO_RING_DEGREE_V1 {
                left_rows[row_index][coefficient] = (coefficient as u64 * 31 + 5) % prime.modulus;
                right_rows[row_index][coefficient] = (coefficient as u64 * 47 + 9) % prime.modulus;
            }
        }
        let left = JindoRnsPolynomialV1::from_residues(left_rows, moduli).expect("left");
        let right = JindoRnsPolynomialV1::from_residues(right_rows, moduli).expect("right");
        let mut sum = left.clone();
        sum.add_assign(&right, moduli);
        sum.sub_assign(&right, moduli);
        assert_eq!(sum, left);
        assert_eq!(
            left.mul(&JindoRnsPolynomialV1::zero(), moduli),
            JindoRnsPolynomialV1::zero()
        );
        assert_eq!(left.mul(&right, moduli), right.mul(&left, moduli));
    }

    #[test]
    fn unit_test_uses_every_negacyclic_root_and_rns_factor() {
        let mut monomial = [0_i128; JINDO_RING_DEGREE_V1];
        monomial[JINDO_RING_DEGREE_V1 - 1] = -1;
        assert!(
            JindoRnsPolynomialV1::from_balanced_coefficients(monomial, JINDO_OUTER_MODULI_V1)
                .is_unit(JINDO_OUTER_MODULI_V1)
        );

        let mut residues = [[0_u64; JINDO_RING_DEGREE_V1]; 2];
        residues[0][0] = sub_mod(
            0,
            JINDO_OUTER_MODULI_V1[0].psi,
            JINDO_OUTER_MODULI_V1[0].modulus,
        );
        residues[0][1] = 1;
        residues[1][0] = 1;
        let vanishes_at_first_root =
            JindoRnsPolynomialV1::from_residues(residues, JINDO_OUTER_MODULI_V1)
                .expect("canonical residues");
        assert!(!vanishes_at_first_root.is_unit(JINDO_OUTER_MODULI_V1));
    }
}
