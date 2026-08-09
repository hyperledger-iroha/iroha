//! Native scalar decompositions and elliptic-curve divisors for FCMP++.
//!
//! FCMP proves an embedded-curve discrete logarithm by committing to a
//! normalized polynomial divisor.  This module implements the construction
//! directly over the two concrete proof fields.  Polynomials are kept sparse
//! and are reduced by `y² = x³ + a·x + b` after every multiplication; this is
//! substantially smaller than a general multivariate-polynomial engine while
//! producing the same canonical normalized divisor.

use std::collections::{BTreeMap, BTreeSet};

use curve25519_dalek::edwards::EdwardsPoint;
use zeroize::Zeroize;

use super::{
    FcmpNativeErrorV1,
    field::{Field25519, HeliosPoint, SelenePoint, edwards_to_wei25519},
    proof_math::ProofScalar,
};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct NormalizedDivisor<F: ProofScalar> {
    pub(super) y: F,
    pub(super) yx: Vec<F>,
    /// Coefficients of `x¹, x², ...`; the first coefficient is always one.
    pub(super) x: Vec<F>,
    pub(super) zero: F,
}

impl<F: ProofScalar> Zeroize for NormalizedDivisor<F> {
    fn zeroize(&mut self) {
        self.y.clear_secret();
        for coefficient in &mut self.yx {
            coefficient.clear_secret();
        }
        for coefficient in &mut self.x {
            coefficient.clear_secret();
        }
        self.yx.clear();
        self.x.clear();
        self.zero.clear_secret();
    }
}

impl<F: ProofScalar> Drop for NormalizedDivisor<F> {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl<F: ProofScalar> NormalizedDivisor<F> {
    #[cfg(test)]
    pub(super) fn eval(&self, x: F, y: F) -> F {
        let mut result = self.zero + (self.y * y);
        let mut x_power = x;
        for coefficient in &self.yx {
            result += *coefficient * y * x_power;
            x_power *= x;
        }
        x_power = x;
        for coefficient in &self.x {
            result += *coefficient * x_power;
            x_power *= x;
        }
        result
    }
}

/// Decompose a canonical nonzero scalar into exactly `scalar_bits`
/// coefficients whose weighted sum represents the scalar and whose ordinary
/// sum is also `scalar_bits`.
///
/// This is the FCMP divisor decomposition, not a binary decomposition:
/// coefficients may exceed one.  The fixed coefficient sum makes the number
/// of divisor points independent of the scalar.
pub(super) fn scalar_decomposition<F: ProofScalar>(
    scalar: F,
    scalar_bits: usize,
) -> Result<Vec<u64>, FcmpNativeErrorV1> {
    let decomposition =
        scalar_decomposition_encoded(scalar.encode(), (-F::ONE).encode(), scalar_bits)?;

    // Validate the two defining equations before using a secret-derived
    // decomposition to construct a large divisor.
    let mut represented = F::ZERO;
    let mut power = F::ONE;
    for coefficient in &decomposition {
        represented += F::from_u64(*coefficient) * power;
        power = power.double();
    }
    if represented != scalar
        || decomposition.iter().copied().sum::<u64>()
            != u64::try_from(scalar_bits).map_err(|_| FcmpNativeErrorV1::TreeFull)?
    {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    Ok(decomposition)
}

pub(super) fn ed25519_scalar_decomposition(
    scalar: curve25519_dalek::scalar::Scalar,
) -> Result<Vec<u64>, FcmpNativeErrorV1> {
    use curve25519_dalek::scalar::Scalar;

    let decomposition =
        scalar_decomposition_encoded(scalar.to_bytes(), (-Scalar::ONE).to_bytes(), 253)?;
    let mut represented = Scalar::ZERO;
    let mut power = Scalar::ONE;
    for coefficient in &decomposition {
        represented += Scalar::from(*coefficient) * power;
        power += power;
    }
    if represented != scalar || decomposition.iter().copied().sum::<u64>() != 253 {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    Ok(decomposition)
}

fn scalar_decomposition_encoded(
    scalar: [u8; 32],
    minus_one: [u8; 32],
    scalar_bits: usize,
) -> Result<Vec<u64>, FcmpNativeErrorV1> {
    if !(3..=255).contains(&scalar_bits) || scalar.iter().all(|byte| *byte == 0) {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    if scalar[scalar_bits / 8] >> (scalar_bits % 8) != 0
        || scalar[(scalar_bits / 8 + 1)..]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(FcmpNativeErrorV1::ScalarEncoding);
    }

    let bit = |bytes: &[u8; 32], index: usize| u64::from((bytes[index / 8] >> (index % 8)) & 1);
    let mut decomposition = (0..scalar_bits)
        .map(|index| bit(&scalar, index))
        .collect::<Vec<_>>();

    // The coefficient-rebalancing algorithm requires an integer larger than
    // the bit count.  For tiny scalars, add the field modulus as a
    // non-carried coefficient vector (`bits(-1) + 1`), preserving the value
    // modulo the scalar field.
    let mut low_bytes = [0_u8; 8];
    low_bytes.copy_from_slice(&scalar[..8]);
    let is_tiny = scalar[8..].iter().all(|byte| *byte == 0)
        && u64::from_le_bytes(low_bytes)
            < u64::try_from(scalar_bits).map_err(|_| FcmpNativeErrorV1::TreeFull)?;
    if is_tiny {
        for index in 0..scalar_bits {
            decomposition[index] += bit(&minus_one, index);
        }
        decomposition[0] += 1;
    }

    let target = u64::try_from(scalar_bits).map_err(|_| FcmpNativeErrorV1::TreeFull)?;
    let mut sum = decomposition.iter().copied().sum::<u64>();

    // First lower an excessive coefficient sum without changing the
    // represented integer: `2·2^i -> 1·2^(i+1)`.
    let mut log2_bits = 0;
    while (1_usize << log2_bits) < scalar_bits {
        log2_bits += 1;
    }
    for _ in 0..log2_bits {
        let mut done = sum == target;
        for index in 0..(scalar_bits - 1) {
            let act = !done && decomposition[index] > 1;
            if act {
                decomposition[index] -= 2;
                decomposition[index + 1] += 1;
                sum -= 1;
                done = true;
            }
        }
    }

    // Then raise a deficient coefficient sum by replacing the highest
    // nonzero `2^i` with two `2^(i-1)` terms.
    for _ in 0..scalar_bits {
        let mut done = sum == target;
        for index in (1..scalar_bits).rev() {
            let act = !done && decomposition[index] != 0;
            if act {
                decomposition[index] -= 1;
                decomposition[index - 1] += 2;
                sum += 1;
                done = true;
            }
        }
    }
    if sum != target {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    Ok(decomposition)
}

pub(super) trait DivisorPoint<F: ProofScalar>: Copy + Eq {
    fn identity() -> Self;
    fn is_identity(self) -> bool;
    fn add(self, other: Self) -> Self;
    fn negate(self) -> Self;
    fn double(self) -> Self;
    fn coordinates(self) -> Result<(F, F), FcmpNativeErrorV1>;
}

impl DivisorPoint<Field25519> for EdwardsPoint {
    fn identity() -> Self {
        <EdwardsPoint as curve25519_dalek::traits::Identity>::identity()
    }

    fn is_identity(self) -> bool {
        self == <EdwardsPoint as curve25519_dalek::traits::Identity>::identity()
    }

    fn add(self, other: Self) -> Self {
        self + other
    }

    fn negate(self) -> Self {
        -self
    }

    fn double(self) -> Self {
        self + self
    }

    fn coordinates(self) -> Result<(Field25519, Field25519), FcmpNativeErrorV1> {
        edwards_to_wei25519(self.compress().to_bytes())
    }
}

impl DivisorPoint<Field25519> for HeliosPoint {
    fn identity() -> Self {
        HeliosPoint::identity()
    }

    fn is_identity(self) -> bool {
        HeliosPoint::is_identity(self)
    }

    fn add(self, other: Self) -> Self {
        HeliosPoint::add(self, other)
    }

    fn negate(self) -> Self {
        HeliosPoint::negate(self)
    }

    fn double(self) -> Self {
        HeliosPoint::double(self)
    }

    fn coordinates(self) -> Result<(Field25519, Field25519), FcmpNativeErrorV1> {
        HeliosPoint::coordinates(self).ok_or(FcmpNativeErrorV1::ArithmeticInvariant)
    }
}

impl DivisorPoint<super::field::HelioseleneField> for SelenePoint {
    fn identity() -> Self {
        SelenePoint::identity()
    }

    fn is_identity(self) -> bool {
        SelenePoint::is_identity(self)
    }

    fn add(self, other: Self) -> Self {
        SelenePoint::add(self, other)
    }

    fn negate(self) -> Self {
        SelenePoint::negate(self)
    }

    fn double(self) -> Self {
        SelenePoint::double(self)
    }

    fn coordinates(
        self,
    ) -> Result<
        (
            super::field::HelioseleneField,
            super::field::HelioseleneField,
        ),
        FcmpNativeErrorV1,
    > {
        SelenePoint::coordinates(self).ok_or(FcmpNativeErrorV1::ArithmeticInvariant)
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ReducedPolynomial<F: ProofScalar> {
    // `(power of y, power of x) -> coefficient`; all stored y powers are
    // zero or one after reduction.
    terms: BTreeMap<(usize, usize), F>,
}

impl<F: ProofScalar> Drop for ReducedPolynomial<F> {
    fn drop(&mut self) {
        for coefficient in self.terms.values_mut() {
            coefficient.clear_secret();
        }
        self.terms.clear();
    }
}

impl<F: ProofScalar> ReducedPolynomial<F> {
    fn zero() -> Self {
        Self {
            terms: BTreeMap::new(),
        }
    }

    fn one() -> Self {
        Self::monomial((0, 0), F::ONE)
    }

    fn monomial(index: (usize, usize), coefficient: F) -> Self {
        let mut result = Self::zero();
        result.add_term(index, coefficient);
        result
    }

    fn add_term(&mut self, index: (usize, usize), coefficient: F) {
        if coefficient.is_zero() {
            return;
        }
        let value = self.terms.entry(index).or_insert(F::ZERO);
        *value += coefficient;
        if value.is_zero() {
            self.terms.remove(&index);
        }
    }

    fn add(mut self, other: &Self) -> Self {
        for (index, coefficient) in &other.terms {
            self.add_term(*index, *coefficient);
        }
        self
    }

    fn scale(mut self, scalar: F) -> Self {
        if scalar.is_zero() {
            return Self::zero();
        }
        for coefficient in self.terms.values_mut() {
            *coefficient *= scalar;
        }
        self
    }

    fn mul_mod(mut self, other: &Self, curve_a: F, curve_b: F) -> Result<Self, FcmpNativeErrorV1> {
        let mut result = Self::zero();
        for ((left_y, left_x), left) in core::mem::take(&mut self.terms) {
            for ((right_y, right_x), right) in &other.terms {
                let y_power = left_y + *right_y;
                let x_power = left_x
                    .checked_add(*right_x)
                    .ok_or(FcmpNativeErrorV1::TreeFull)?;
                let coefficient = left * *right;
                match y_power {
                    0 | 1 => result.add_term((y_power, x_power), coefficient),
                    2 => {
                        // y² x^k = x^(k+3) + a x^(k+1) + b x^k.
                        result.add_term(
                            (
                                0,
                                x_power.checked_add(3).ok_or(FcmpNativeErrorV1::TreeFull)?,
                            ),
                            coefficient,
                        );
                        result.add_term(
                            (
                                0,
                                x_power.checked_add(1).ok_or(FcmpNativeErrorV1::TreeFull)?,
                            ),
                            coefficient * curve_a,
                        );
                        result.add_term((0, x_power), coefficient * curve_b);
                    }
                    _ => return Err(FcmpNativeErrorV1::ArithmeticInvariant),
                }
            }
        }
        Ok(result)
    }

    /// Divide by an x-only polynomial. Each y coefficient is an independent
    /// univariate polynomial in x.
    fn div_rem_x(self, denominator: &Self) -> Result<(Self, Self), FcmpNativeErrorV1> {
        if denominator.terms.is_empty()
            || denominator.terms.keys().any(|(y_power, _)| *y_power != 0)
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let (&(_, denominator_degree), &denominator_lead) = denominator
            .terms
            .last_key_value()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let denominator_inverse = denominator_lead
            .invert()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let y_powers = self
            .terms
            .keys()
            .map(|(y_power, _)| *y_power)
            .collect::<BTreeSet<_>>();
        let mut quotient = Self::zero();
        let mut remainder = Self::zero();

        for y_power in y_powers {
            let mut row = self
                .terms
                .iter()
                .filter(|((row, _), _)| *row == y_power)
                .map(|((_, degree), coefficient)| (*degree, *coefficient))
                .collect::<BTreeMap<_, _>>();
            loop {
                let Some((&degree, &coefficient)) = row.last_key_value() else {
                    break;
                };
                if degree < denominator_degree {
                    break;
                }
                let shift = degree - denominator_degree;
                let q = coefficient * denominator_inverse;
                quotient.add_term((y_power, shift), q);
                for ((_, denominator_power), denominator_coefficient) in &denominator.terms {
                    let target = shift
                        .checked_add(*denominator_power)
                        .ok_or(FcmpNativeErrorV1::TreeFull)?;
                    let value = row.entry(target).or_insert(F::ZERO);
                    *value -= q * *denominator_coefficient;
                    if value.is_zero() {
                        row.remove(&target);
                    }
                }
            }
            for (degree, coefficient) in row {
                remainder.add_term((y_power, degree), coefficient);
            }
        }
        Ok((quotient, remainder))
    }

    fn normalized(self) -> Result<NormalizedDivisor<F>, FcmpNativeErrorV1> {
        if self.terms.keys().any(|(y_power, _)| *y_power > 1) {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let x_one = *self
            .terms
            .get(&(0, 1))
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let inverse = x_one
            .invert()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let normalized = self.scale(inverse);
        let y = normalized.terms.get(&(1, 0)).copied().unwrap_or(F::ZERO);
        let max_yx = normalized
            .terms
            .keys()
            .filter_map(|(y_power, x_power)| (*y_power == 1).then_some(*x_power))
            .max()
            .unwrap_or(0);
        let yx = (1..=max_yx)
            .map(|power| {
                normalized
                    .terms
                    .get(&(1, power))
                    .copied()
                    .unwrap_or(F::ZERO)
            })
            .collect();
        let max_x = normalized
            .terms
            .keys()
            .filter_map(|(y_power, x_power)| (*y_power == 0).then_some(*x_power))
            .max()
            .unwrap_or(0);
        let x = (1..=max_x)
            .map(|power| {
                normalized
                    .terms
                    .get(&(0, power))
                    .copied()
                    .unwrap_or(F::ZERO)
            })
            .collect::<Vec<_>>();
        if x.first().copied() != Some(F::ONE) {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        Ok(NormalizedDivisor {
            y,
            yx,
            x,
            zero: normalized.terms.get(&(0, 0)).copied().unwrap_or(F::ZERO),
        })
    }
}

fn line<F: ProofScalar, P: DivisorPoint<F>>(
    first: P,
    second: P,
) -> Result<ReducedPolynomial<F>, FcmpNativeErrorV1> {
    if first.is_identity() && second.is_identity() {
        return Ok(ReducedPolynomial::one());
    }
    if first.is_identity() || second.is_identity() || first == second.negate() {
        let point = if first.is_identity() { second } else { first };
        let (x, _) = point.coordinates()?;
        return Ok(ReducedPolynomial::monomial((0, 1), F::ONE)
            .add(&ReducedPolynomial::monomial((0, 0), -x)));
    }

    // For equal points, the line through P and -2P is tangent at P.
    let second = if first == second {
        first.double().negate()
    } else {
        second
    };
    let (first_x, first_y) = first.coordinates()?;
    let (second_x, second_y) = second.coordinates()?;
    let slope = (second_y - first_y)
        * (second_x - first_x)
            .invert()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    let intercept = second_y - (slope * second_x);
    Ok(ReducedPolynomial::monomial((1, 0), F::ONE)
        .add(&ReducedPolynomial::monomial((0, 1), -slope))
        .add(&ReducedPolynomial::monomial((0, 0), -intercept)))
}

fn new_divisor<F: ProofScalar, P: DivisorPoint<F>>(
    curve_a: F,
    curve_b: F,
    points: &[P],
) -> Result<ReducedPolynomial<F>, FcmpNativeErrorV1> {
    if points.len() < 2 || points.iter().any(|point| point.is_identity()) {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    let sum = points
        .iter()
        .copied()
        .fold(P::identity(), |sum, point| sum.add(point));
    if !sum.is_identity() {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }

    let mut divisors = Vec::with_capacity(points.len().div_ceil(2));
    let mut index = 0;
    while index < points.len() {
        let first = points[index];
        let second = points.get(index + 1).copied();
        divisors.push((
            2_usize,
            first.add(second.unwrap_or(P::identity())),
            line(first, second.unwrap_or(first.negate()))?,
        ));
        index += 2;
    }

    while divisors.len() > 1 {
        let mut next = Vec::with_capacity(divisors.len().div_ceil(2));
        if divisors.len() % 2 == 1 {
            next.push(
                divisors
                    .pop()
                    .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
            );
        }
        while let Some((left_points, left_sum, left_divisor)) = divisors.pop() {
            let (right_points, right_sum, right_divisor) = divisors
                .pop()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            let numerator = left_divisor
                .mul_mod(&right_divisor, curve_a, curve_b)?
                .mul_mod(&line(left_sum, right_sum)?, curve_a, curve_b)?;
            let denominator = line(left_sum, left_sum.negate())?.mul_mod(
                &line(right_sum, right_sum.negate())?,
                curve_a,
                curve_b,
            )?;
            let (quotient, remainder) = numerator.div_rem_x(&denominator)?;
            if !remainder.terms.is_empty() {
                return Err(FcmpNativeErrorV1::ArithmeticInvariant);
            }
            next.push((
                left_points
                    .checked_add(right_points)
                    .ok_or(FcmpNativeErrorV1::TreeFull)?,
                left_sum.add(right_sum),
                quotient,
            ));
        }
        divisors = next;
    }
    divisors
        .pop()
        .map(|(_, _, divisor)| divisor)
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)
}

pub(super) fn scalar_mul_divisor<F: ProofScalar, P: DivisorPoint<F>>(
    curve_a: F,
    curve_b: F,
    generator: P,
    decomposition: &[u64],
    result: P,
) -> Result<NormalizedDivisor<F>, FcmpNativeErrorV1> {
    if decomposition.len() < 3
        || generator.is_identity()
        || result.is_identity()
        || decomposition.iter().copied().sum::<u64>()
            != u64::try_from(decomposition.len()).map_err(|_| FcmpNativeErrorV1::TreeFull)?
    {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    let mut points = Vec::with_capacity(decomposition.len() + 1);
    points.push(result.negate());
    let mut power = generator;
    for coefficient in decomposition {
        for _ in 0..*coefficient {
            points.push(power);
        }
        power = power.double();
    }
    if points.len() != decomposition.len() + 1 {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    new_divisor(curve_a, curve_b, &points)?.normalized()
}

#[cfg(test)]
mod tests {
    use curve25519_dalek::{constants::ED25519_BASEPOINT_POINT, scalar::Scalar};

    use super::*;
    use crate::privacy_engines::fcmp_plus_plus::{
        field::{HelioseleneField, field25519_from_u64, helios_hash_initializer},
        proof_math::ProofPoint as _,
    };

    #[test]
    fn scalar_decompositions_preserve_value_and_fixed_weight() {
        for scalar in [
            Scalar::ONE,
            Scalar::from(2_u64),
            Scalar::from(252_u64),
            Scalar::from(253_u64),
            Scalar::from(0xdead_beef_u64),
            -Scalar::ONE,
        ] {
            let decomposition = ed25519_scalar_decomposition(scalar).expect("decomposition");
            assert_eq!(decomposition.len(), 253);
            assert_eq!(decomposition.iter().sum::<u64>(), 253);
        }

        for scalar in [
            HelioseleneField::ONE,
            HelioseleneField::from_u64(254),
            HelioseleneField::from_u64(255),
            HelioseleneField::from_u64(0x1234_5678),
            -HelioseleneField::ONE,
        ] {
            let decomposition = scalar_decomposition(scalar, 255).expect("decomposition");
            assert_eq!(decomposition.len(), 255);
            assert_eq!(decomposition.iter().sum::<u64>(), 255);
        }
        assert!(ed25519_scalar_decomposition(Scalar::ZERO).is_err());
        assert!(scalar_decomposition(HelioseleneField::ZERO, 255).is_err());
    }

    #[test]
    fn native_sparse_divisor_vanishes_at_scalar_mul_endpoints() {
        // A real 253-point FCMP divisor over Wei25519.
        let scalar = Scalar::from(17_u64);
        let decomposition = ed25519_scalar_decomposition(scalar).expect("decomposition");
        let result = ED25519_BASEPOINT_POINT * scalar;
        let curve_a = Field25519::new(&p256::elliptic_curve::bigint::U256::from_be_hex(
            "2aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa984914a144",
        ));
        let curve_b = Field25519::new(&p256::elliptic_curve::bigint::U256::from_be_hex(
            "7b425ed097b425ed097b425ed097b425ed097b425ed097b4260b5e9c7710c864",
        ));
        let divisor = scalar_mul_divisor(
            curve_a,
            curve_b,
            ED25519_BASEPOINT_POINT,
            &decomposition,
            result,
        )
        .expect("divisor");
        assert_eq!(divisor.x.first().copied(), Some(Field25519::ONE));
        assert!(divisor.yx.len() <= 125);
        assert!(divisor.x.len() <= 127);
        let mut represented_power = ED25519_BASEPOINT_POINT;
        let represented_power = decomposition
            .iter()
            .find_map(|coefficient| {
                let point = represented_power;
                represented_power = represented_power + represented_power;
                (*coefficient != 0).then_some(point)
            })
            .expect("fixed-weight decomposition has a point");
        for point in [represented_power, -result] {
            let (x, y) = point.coordinates().expect("coordinates");
            assert!(divisor.eval(x, y).is_zero());
        }

        // Keep the cycle-curve implementation and point abstraction covered.
        let cycle_scalar = HelioseleneField::from_u64(9);
        let cycle_decomposition =
            scalar_decomposition(cycle_scalar, 255).expect("cycle decomposition");
        let cycle_generator = helios_hash_initializer();
        let cycle_result = cycle_generator.scale(cycle_scalar);
        let cycle_b = Field25519::new(&p256::elliptic_curve::bigint::U256::from_be_hex(
            "22e8c739b0ea70b8be94a76b3ebb7b3b043f6f384113bf3522b49ee1edd73ad4",
        ));
        let cycle_divisor = scalar_mul_divisor(
            -field25519_from_u64(3),
            cycle_b,
            cycle_generator,
            &cycle_decomposition,
            cycle_result,
        )
        .expect("cycle divisor");
        assert_eq!(cycle_divisor.x.first().copied(), Some(Field25519::ONE));
        let (cycle_x, cycle_y) = cycle_result
            .negate()
            .coordinates()
            .expect("cycle coordinates");
        assert!(cycle_divisor.eval(cycle_x, cycle_y).is_zero());
    }
}
