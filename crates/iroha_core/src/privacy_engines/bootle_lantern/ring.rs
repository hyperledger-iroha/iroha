//! Canonical negacyclic rings used by the fixed Bootle/Lantern profile.
//!
//! Both rings are represented in coefficient form in
//! `R_m = Z_m[X] / (X^64 + 1)`. Constructors reject rather than reduce
//! externally supplied residues. Arithmetic methods always return canonical
//! residues and use explicit modular operations, so debug and release builds
//! have identical overflow behavior.

use thiserror::Error;
use zeroize::Zeroize;

use super::params::{APPLICATION_MODULUS_V1, APPLICATION_RING_DEGREE_V1, PROOF_MODULUS_V1};

/// One canonical polynomial in `Z_12289[X] / (X^64 + 1)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationPolynomialV1 {
    coefficients: [u16; APPLICATION_RING_DEGREE_V1],
}

impl ApplicationPolynomialV1 {
    /// Additive identity.
    pub const ZERO: Self = Self {
        coefficients: [0; APPLICATION_RING_DEGREE_V1],
    };

    /// Construct from canonical application-ring residues.
    ///
    /// # Errors
    ///
    /// Returns an error at the first coefficient at least `12_289`.
    pub fn new(coefficients: [u16; APPLICATION_RING_DEGREE_V1]) -> Result<Self, RingErrorV1> {
        if let Some((index, coefficient)) = coefficients
            .iter()
            .copied()
            .enumerate()
            .find(|(_, coefficient)| *coefficient >= APPLICATION_MODULUS_V1)
        {
            return Err(RingErrorV1::NonCanonicalApplicationCoefficient {
                index: u8::try_from(index).expect("ring index fits u8"),
                coefficient: u64::from(coefficient),
            });
        }
        Ok(Self { coefficients })
    }

    /// Construct a constant polynomial from a canonical residue.
    ///
    /// # Errors
    ///
    /// Returns an error when `constant >= 12_289`.
    pub fn constant(constant: u16) -> Result<Self, RingErrorV1> {
        if constant >= APPLICATION_MODULUS_V1 {
            return Err(RingErrorV1::NonCanonicalApplicationCoefficient {
                index: 0,
                coefficient: u64::from(constant),
            });
        }
        let mut coefficients = [0; APPLICATION_RING_DEGREE_V1];
        coefficients[0] = constant;
        Ok(Self { coefficients })
    }

    /// Construct from the unique centered integer lift of each coefficient.
    #[must_use]
    pub fn from_centered_coefficients(coefficients: [i64; APPLICATION_RING_DEGREE_V1]) -> Self {
        let modulus = i64::from(APPLICATION_MODULUS_V1);
        let mut output = [0_u16; APPLICATION_RING_DEGREE_V1];
        for (output, coefficient) in output.iter_mut().zip(coefficients) {
            let residue = coefficient.rem_euclid(modulus);
            *output = u16::try_from(residue).expect("reduced application residue fits u16");
        }
        Self {
            coefficients: output,
        }
    }

    /// Encode one direct 64-bit attribute as little-endian binary
    /// coefficients.
    #[must_use]
    pub fn from_direct_attribute(attribute: [u8; 8]) -> Self {
        let mut coefficients = [0_u16; APPLICATION_RING_DEGREE_V1];
        for (index, coefficient) in coefficients.iter_mut().enumerate() {
            *coefficient = u16::from((attribute[index / 8] >> (index % 8)) & 1);
        }
        Self { coefficients }
    }

    /// Decode a binary polynomial back to its direct 64-bit attribute.
    ///
    /// # Errors
    ///
    /// Rejects the first coefficient other than zero or one.
    pub fn to_direct_attribute(self) -> Result<[u8; 8], RingErrorV1> {
        let mut attribute = [0_u8; 8];
        for (index, coefficient) in self.coefficients.iter().copied().enumerate() {
            if coefficient > 1 {
                return Err(RingErrorV1::NonBinaryCoefficient {
                    index: u8::try_from(index).expect("ring index fits u8"),
                    coefficient: u64::from(coefficient),
                });
            }
            attribute[index / 8] |=
                u8::try_from(coefficient).expect("binary coefficient fits u8") << (index % 8);
        }
        Ok(attribute)
    }

    /// Borrow canonical residues.
    #[must_use]
    pub const fn coefficients(&self) -> &[u16; APPLICATION_RING_DEGREE_V1] {
        &self.coefficients
    }

    /// Return whether this polynomial is the additive identity.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.coefficients
            .iter()
            .all(|coefficient| *coefficient == 0)
    }

    /// Add in the application ring.
    #[must_use]
    pub fn add(self, rhs: Self) -> Self {
        let mut output = [0_u16; APPLICATION_RING_DEGREE_V1];
        for ((output, lhs), rhs) in output
            .iter_mut()
            .zip(self.coefficients)
            .zip(rhs.coefficients)
        {
            *output = add_mod_u16(lhs, rhs, APPLICATION_MODULUS_V1);
        }
        Self {
            coefficients: output,
        }
    }

    /// Subtract in the application ring.
    #[must_use]
    pub fn sub(self, rhs: Self) -> Self {
        let mut output = [0_u16; APPLICATION_RING_DEGREE_V1];
        for ((output, lhs), rhs) in output
            .iter_mut()
            .zip(self.coefficients)
            .zip(rhs.coefficients)
        {
            *output = sub_mod_u16(lhs, rhs, APPLICATION_MODULUS_V1);
        }
        Self {
            coefficients: output,
        }
    }

    /// Negate in the application ring.
    #[must_use]
    pub fn negate(self) -> Self {
        Self::ZERO.sub(self)
    }

    /// Multiply modulo `X^64 + 1`.
    #[must_use]
    pub fn multiply(self, rhs: Self) -> Self {
        let mut output = [0_u16; APPLICATION_RING_DEGREE_V1];
        for (lhs_index, lhs) in self.coefficients.iter().copied().enumerate() {
            for (rhs_index, rhs) in rhs.coefficients.iter().copied().enumerate() {
                let product = u16::try_from(
                    u32::from(lhs) * u32::from(rhs) % u32::from(APPLICATION_MODULUS_V1),
                )
                .expect("reduced application residue fits u16");
                let degree = lhs_index + rhs_index;
                if degree < APPLICATION_RING_DEGREE_V1 {
                    output[degree] = add_mod_u16(output[degree], product, APPLICATION_MODULUS_V1);
                } else {
                    output[degree - APPLICATION_RING_DEGREE_V1] = sub_mod_u16(
                        output[degree - APPLICATION_RING_DEGREE_V1],
                        product,
                        APPLICATION_MODULUS_V1,
                    );
                }
            }
        }
        Self {
            coefficients: output,
        }
    }

    /// Multiply by a signed integer in the application ring.
    #[must_use]
    pub fn scale_centered(self, scalar: i64) -> Self {
        let scalar = scalar.rem_euclid(i64::from(APPLICATION_MODULUS_V1));
        let scalar = u16::try_from(scalar).expect("reduced application scalar fits u16");
        let mut output = [0_u16; APPLICATION_RING_DEGREE_V1];
        for (output, coefficient) in output.iter_mut().zip(self.coefficients) {
            *output = u16::try_from(
                u32::from(coefficient) * u32::from(scalar) % u32::from(APPLICATION_MODULUS_V1),
            )
            .expect("reduced application residue fits u16");
        }
        Self {
            coefficients: output,
        }
    }

    /// Apply the involution `X -> X^-1` in the negacyclic ring.
    #[must_use]
    pub fn automorphism(self) -> Self {
        let mut output = [0_u16; APPLICATION_RING_DEGREE_V1];
        output[0] = self.coefficients[0];
        for index in 1..APPLICATION_RING_DEGREE_V1 {
            let coefficient = self.coefficients[APPLICATION_RING_DEGREE_V1 - index];
            output[index] = if coefficient == 0 {
                0
            } else {
                APPLICATION_MODULUS_V1 - coefficient
            };
        }
        Self {
            coefficients: output,
        }
    }

    /// Return a centered coefficient in `[-6144, 6144]`.
    #[must_use]
    pub fn centered_coefficient(&self, index: usize) -> i16 {
        let residue = self.coefficients[index];
        if residue <= APPLICATION_MODULUS_V1 / 2 {
            i16::try_from(residue).expect("application residue fits i16")
        } else {
            i16::try_from(residue).expect("application residue fits i16")
                - i16::try_from(APPLICATION_MODULUS_V1).expect("application modulus fits i16")
        }
    }

    /// Exact squared Euclidean norm of the centered lift.
    #[must_use]
    pub fn centered_squared_norm(&self) -> u64 {
        self.coefficients
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let coefficient = i64::from(self.centered_coefficient(index));
                u64::try_from(coefficient * coefficient).expect("square is non-negative")
            })
            .sum()
    }
}

impl Zeroize for ApplicationPolynomialV1 {
    fn zeroize(&mut self) {
        self.coefficients.zeroize();
    }
}

/// One canonical polynomial in `Z_q[X] / (X^64 + 1)` for the internal
/// Lantern proof modulus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProofPolynomialV1 {
    coefficients: [u64; APPLICATION_RING_DEGREE_V1],
}

impl ProofPolynomialV1 {
    /// Additive identity.
    pub const ZERO: Self = Self {
        coefficients: [0; APPLICATION_RING_DEGREE_V1],
    };

    /// Construct from canonical internal proof-ring residues.
    ///
    /// # Errors
    ///
    /// Returns an error at the first coefficient at least `q`.
    pub fn new(coefficients: [u64; APPLICATION_RING_DEGREE_V1]) -> Result<Self, RingErrorV1> {
        if let Some((index, coefficient)) = coefficients
            .iter()
            .copied()
            .enumerate()
            .find(|(_, coefficient)| *coefficient >= PROOF_MODULUS_V1)
        {
            return Err(RingErrorV1::NonCanonicalProofCoefficient {
                index: u8::try_from(index).expect("ring index fits u8"),
                coefficient,
            });
        }
        Ok(Self { coefficients })
    }

    /// Construct a constant polynomial from a canonical residue.
    ///
    /// # Errors
    ///
    /// Returns an error when `constant >= q`.
    pub fn constant(constant: u64) -> Result<Self, RingErrorV1> {
        if constant >= PROOF_MODULUS_V1 {
            return Err(RingErrorV1::NonCanonicalProofCoefficient {
                index: 0,
                coefficient: constant,
            });
        }
        let mut coefficients = [0; APPLICATION_RING_DEGREE_V1];
        coefficients[0] = constant;
        Ok(Self { coefficients })
    }

    /// Construct a constant polynomial from any centered integer.
    #[must_use]
    pub fn constant_centered(constant: i64) -> Self {
        let mut coefficients = [0_u64; APPLICATION_RING_DEGREE_V1];
        coefficients[0] = canonicalize_i128(i128::from(constant), PROOF_MODULUS_V1);
        Self { coefficients }
    }

    /// Construct from arbitrary centered integer coefficients.
    #[must_use]
    pub fn from_centered_coefficients(coefficients: [i64; APPLICATION_RING_DEGREE_V1]) -> Self {
        let mut output = [0_u64; APPLICATION_RING_DEGREE_V1];
        for (output, coefficient) in output.iter_mut().zip(coefficients) {
            *output = canonicalize_i128(i128::from(coefficient), PROOF_MODULUS_V1);
        }
        Self {
            coefficients: output,
        }
    }

    /// Embed one application-ring polynomial through its centered lift.
    #[must_use]
    pub fn from_application_centered(polynomial: ApplicationPolynomialV1) -> Self {
        let mut coefficients = [0_i64; APPLICATION_RING_DEGREE_V1];
        for (index, coefficient) in coefficients.iter_mut().enumerate() {
            *coefficient = i64::from(polynomial.centered_coefficient(index));
        }
        Self::from_centered_coefficients(coefficients)
    }

    /// Borrow canonical residues.
    #[must_use]
    pub const fn coefficients(&self) -> &[u64; APPLICATION_RING_DEGREE_V1] {
        &self.coefficients
    }

    /// Return whether this polynomial is the additive identity.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.coefficients
            .iter()
            .all(|coefficient| *coefficient == 0)
    }

    /// Add in the internal proof ring.
    #[must_use]
    pub fn add(self, rhs: Self) -> Self {
        let mut output = [0_u64; APPLICATION_RING_DEGREE_V1];
        for ((output, lhs), rhs) in output
            .iter_mut()
            .zip(self.coefficients)
            .zip(rhs.coefficients)
        {
            *output = add_mod_u64(lhs, rhs, PROOF_MODULUS_V1);
        }
        Self {
            coefficients: output,
        }
    }

    /// Subtract in the internal proof ring.
    #[must_use]
    pub fn sub(self, rhs: Self) -> Self {
        let mut output = [0_u64; APPLICATION_RING_DEGREE_V1];
        for ((output, lhs), rhs) in output
            .iter_mut()
            .zip(self.coefficients)
            .zip(rhs.coefficients)
        {
            *output = sub_mod_u64(lhs, rhs, PROOF_MODULUS_V1);
        }
        Self {
            coefficients: output,
        }
    }

    /// Negate in the internal proof ring.
    #[must_use]
    pub fn negate(self) -> Self {
        Self::ZERO.sub(self)
    }

    /// Multiply modulo `X^64 + 1`.
    #[must_use]
    pub fn multiply(self, rhs: Self) -> Self {
        let mut output = [0_u64; APPLICATION_RING_DEGREE_V1];
        for (lhs_index, lhs) in self.coefficients.iter().copied().enumerate() {
            for (rhs_index, rhs) in rhs.coefficients.iter().copied().enumerate() {
                let product =
                    u64::try_from(u128::from(lhs) * u128::from(rhs) % u128::from(PROOF_MODULUS_V1))
                        .expect("reduced proof residue fits u64");
                let degree = lhs_index + rhs_index;
                if degree < APPLICATION_RING_DEGREE_V1 {
                    output[degree] = add_mod_u64(output[degree], product, PROOF_MODULUS_V1);
                } else {
                    output[degree - APPLICATION_RING_DEGREE_V1] = sub_mod_u64(
                        output[degree - APPLICATION_RING_DEGREE_V1],
                        product,
                        PROOF_MODULUS_V1,
                    );
                }
            }
        }
        Self {
            coefficients: output,
        }
    }

    /// Multiply by a signed integer in the proof ring.
    #[must_use]
    pub fn scale_centered(self, scalar: i64) -> Self {
        let scalar = canonicalize_i128(i128::from(scalar), PROOF_MODULUS_V1);
        let mut output = [0_u64; APPLICATION_RING_DEGREE_V1];
        for (output, coefficient) in output.iter_mut().zip(self.coefficients) {
            *output = u64::try_from(
                u128::from(coefficient) * u128::from(scalar) % u128::from(PROOF_MODULUS_V1),
            )
            .expect("reduced proof residue fits u64");
        }
        Self {
            coefficients: output,
        }
    }

    /// Apply the involution `X -> X^-1` in the negacyclic ring.
    #[must_use]
    pub fn automorphism(self) -> Self {
        let mut output = [0_u64; APPLICATION_RING_DEGREE_V1];
        output[0] = self.coefficients[0];
        for index in 1..APPLICATION_RING_DEGREE_V1 {
            let coefficient = self.coefficients[APPLICATION_RING_DEGREE_V1 - index];
            output[index] = if coefficient == 0 {
                0
            } else {
                PROOF_MODULUS_V1 - coefficient
            };
        }
        Self {
            coefficients: output,
        }
    }

    /// Multiply by `X^power` modulo `X^64 + 1`.
    #[must_use]
    pub fn multiply_by_monomial(self, power: usize) -> Self {
        let reduced_power = power % (2 * APPLICATION_RING_DEGREE_V1);
        if reduced_power == 0 {
            return self;
        }
        let mut output = [0_u64; APPLICATION_RING_DEGREE_V1];
        for (index, coefficient) in self.coefficients.iter().copied().enumerate() {
            let degree = index + reduced_power;
            let wraps = degree / APPLICATION_RING_DEGREE_V1;
            let destination = degree % APPLICATION_RING_DEGREE_V1;
            output[destination] = if wraps % 2 == 0 || coefficient == 0 {
                coefficient
            } else {
                PROOF_MODULUS_V1 - coefficient
            };
        }
        Self {
            coefficients: output,
        }
    }

    /// Return a centered coefficient in `[-floor(q/2), floor(q/2)]`.
    #[must_use]
    pub fn centered_coefficient(&self, index: usize) -> i64 {
        let residue = self.coefficients[index];
        if residue <= PROOF_MODULUS_V1 / 2 {
            i64::try_from(residue).expect("proof residue fits i64")
        } else {
            i64::try_from(residue).expect("proof residue fits i64")
                - i64::try_from(PROOF_MODULUS_V1).expect("proof modulus fits i64")
        }
    }

    /// Exact squared Euclidean norm of the centered lift.
    #[must_use]
    pub fn centered_squared_norm(&self) -> u128 {
        self.coefficients
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let coefficient = i128::from(self.centered_coefficient(index));
                u128::try_from(coefficient * coefficient).expect("square is non-negative")
            })
            .sum()
    }
}

impl Zeroize for ProofPolynomialV1 {
    fn zeroize(&mut self) {
        self.coefficients.zeroize();
    }
}

fn canonicalize_i128(value: i128, modulus: u64) -> u64 {
    let modulus = i128::from(modulus);
    let mut residue = value % modulus;
    if residue < 0 {
        residue += modulus;
    }
    u64::try_from(residue).expect("canonical residue fits u64")
}

fn add_mod_u16(lhs: u16, rhs: u16, modulus: u16) -> u16 {
    let sum = u32::from(lhs) + u32::from(rhs);
    u16::try_from(sum % u32::from(modulus)).expect("reduced sum fits u16")
}

fn sub_mod_u16(lhs: u16, rhs: u16, modulus: u16) -> u16 {
    if lhs >= rhs {
        lhs - rhs
    } else {
        modulus - (rhs - lhs)
    }
}

fn add_mod_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    let sum = u128::from(lhs) + u128::from(rhs);
    u64::try_from(sum % u128::from(modulus)).expect("reduced sum fits u64")
}

fn sub_mod_u64(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    if lhs >= rhs {
        lhs - rhs
    } else {
        modulus - (rhs - lhs)
    }
}

/// Canonical ring failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RingErrorV1 {
    /// An application-ring residue is not in `[0, 12_289)`.
    #[error("application polynomial coefficient {index} has non-canonical residue {coefficient}")]
    NonCanonicalApplicationCoefficient {
        /// Coefficient index.
        index: u8,
        /// Rejected residue.
        coefficient: u64,
    },
    /// An internal proof-ring residue is not in `[0, q)`.
    #[error("proof polynomial coefficient {index} has non-canonical residue {coefficient}")]
    NonCanonicalProofCoefficient {
        /// Coefficient index.
        index: u8,
        /// Rejected residue.
        coefficient: u64,
    },
    /// A direct attribute polynomial was not binary.
    #[error("direct attribute coefficient {index} is {coefficient}, not zero or one")]
    NonBinaryCoefficient {
        /// Coefficient index.
        index: u8,
        /// Rejected residue.
        coefficient: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn application_monomial(index: usize, coefficient: u16) -> ApplicationPolynomialV1 {
        let mut coefficients = [0_u16; APPLICATION_RING_DEGREE_V1];
        coefficients[index] = coefficient;
        ApplicationPolynomialV1::new(coefficients).expect("canonical monomial")
    }

    fn proof_monomial(index: usize, coefficient: u64) -> ProofPolynomialV1 {
        let mut coefficients = [0_u64; APPLICATION_RING_DEGREE_V1];
        coefficients[index] = coefficient;
        ProofPolynomialV1::new(coefficients).expect("canonical monomial")
    }

    #[test]
    fn constructors_reject_modulus_and_larger_residues_at_exact_index() {
        for coefficient in [APPLICATION_MODULUS_V1, u16::MAX] {
            let mut coefficients = [0_u16; APPLICATION_RING_DEGREE_V1];
            coefficients[63] = coefficient;
            assert_eq!(
                ApplicationPolynomialV1::new(coefficients),
                Err(RingErrorV1::NonCanonicalApplicationCoefficient {
                    index: 63,
                    coefficient: u64::from(coefficient)
                })
            );
        }
        for coefficient in [PROOF_MODULUS_V1, PROOF_MODULUS_V1 + 1, u64::MAX] {
            let mut coefficients = [0_u64; APPLICATION_RING_DEGREE_V1];
            coefficients[41] = coefficient;
            assert_eq!(
                ProofPolynomialV1::new(coefficients),
                Err(RingErrorV1::NonCanonicalProofCoefficient {
                    index: 41,
                    coefficient
                })
            );
        }
    }

    #[test]
    fn direct_attributes_use_little_endian_bits_and_reject_nonbinary_values() {
        for attribute in [
            [0_u8; 8],
            [u8::MAX; 8],
            [0x81, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81],
        ] {
            let polynomial = ApplicationPolynomialV1::from_direct_attribute(attribute);
            assert_eq!(
                polynomial.to_direct_attribute().expect("binary polynomial"),
                attribute
            );
        }
        let polynomial = application_monomial(37, 2);
        assert_eq!(
            polynomial.to_direct_attribute(),
            Err(RingErrorV1::NonBinaryCoefficient {
                index: 37,
                coefficient: 2
            })
        );
    }

    #[test]
    fn application_negacyclic_wrap_is_subtractive_and_canonical() {
        let x = application_monomial(1, 1);
        let x63 = application_monomial(63, 1);
        let product = x.multiply(x63);
        assert_eq!(
            product,
            ApplicationPolynomialV1::constant(APPLICATION_MODULUS_V1 - 1)
                .expect("canonical minus one")
        );
        assert_eq!(product.centered_coefficient(0), -1);
        assert_eq!(product.centered_squared_norm(), 1);
    }

    #[test]
    fn proof_negacyclic_wrap_is_subtractive_and_canonical() {
        let x = proof_monomial(1, 1);
        let x63 = proof_monomial(63, 1);
        let product = x.multiply(x63);
        assert_eq!(
            product,
            ProofPolynomialV1::constant(PROOF_MODULUS_V1 - 1).expect("canonical minus one")
        );
        assert_eq!(product.centered_coefficient(0), -1);
        assert_eq!(product.centered_squared_norm(), 1);
    }

    #[test]
    fn ring_laws_hold_on_adversarial_boundary_polynomials() {
        let mut application_boundary = [0_u16; APPLICATION_RING_DEGREE_V1];
        let mut proof_boundary = [0_u64; APPLICATION_RING_DEGREE_V1];
        for index in 0..APPLICATION_RING_DEGREE_V1 {
            application_boundary[index] = if index % 2 == 0 {
                APPLICATION_MODULUS_V1 - 1
            } else {
                1
            };
            proof_boundary[index] = if index % 2 == 0 {
                PROOF_MODULUS_V1 - 1
            } else {
                1
            };
        }
        let a = ApplicationPolynomialV1::new(application_boundary).expect("canonical");
        let q = ProofPolynomialV1::new(proof_boundary).expect("canonical");
        let application_one = ApplicationPolynomialV1::constant(1).expect("one");
        let proof_one = ProofPolynomialV1::constant(1).expect("one");

        assert_eq!(a.add(a.negate()), ApplicationPolynomialV1::ZERO);
        assert_eq!(a.multiply(application_one), a);
        assert_eq!(a.sub(a), ApplicationPolynomialV1::ZERO);
        assert_eq!(q.add(q.negate()), ProofPolynomialV1::ZERO);
        assert_eq!(q.multiply(proof_one), q);
        assert_eq!(q.sub(q), ProofPolynomialV1::ZERO);
        assert!(
            a.coefficients()
                .iter()
                .all(|coefficient| *coefficient < APPLICATION_MODULUS_V1)
        );
        assert!(
            q.coefficients()
                .iter()
                .all(|coefficient| *coefficient < PROOF_MODULUS_V1)
        );
    }

    #[test]
    fn distributivity_holds_for_wrapping_terms_in_both_rings() {
        let a = application_monomial(63, APPLICATION_MODULUS_V1 - 1);
        let b = application_monomial(1, APPLICATION_MODULUS_V1 - 1);
        let c = application_monomial(32, 7);
        assert_eq!(a.multiply(b.add(c)), a.multiply(b).add(a.multiply(c)));

        let a = proof_monomial(63, PROOF_MODULUS_V1 - 1);
        let b = proof_monomial(1, PROOF_MODULUS_V1 - 1);
        let c = proof_monomial(32, 7);
        assert_eq!(a.multiply(b.add(c)), a.multiply(b).add(a.multiply(c)));
    }

    #[test]
    fn centered_lifts_scaling_automorphism_and_monomials_are_exact() {
        let application = ApplicationPolynomialV1::from_centered_coefficients(
            core::array::from_fn(|index| {
                if index == 0 {
                    -1
                } else if index == 63 {
                    7
                } else {
                    0
                }
            }),
        );
        assert_eq!(application.centered_coefficient(0), -1);
        assert_eq!(application.centered_coefficient(63), 7);
        assert_eq!(application.automorphism().automorphism(), application);
        assert_eq!(
            application.scale_centered(-3),
            application
                .multiply(ApplicationPolynomialV1::constant(
                    APPLICATION_MODULUS_V1 - 3
                )
                .expect("canonical"))
        );

        let proof = ProofPolynomialV1::from_application_centered(application);
        assert_eq!(proof.centered_coefficient(0), -1);
        assert_eq!(proof.centered_coefficient(63), 7);
        assert_eq!(proof.automorphism().automorphism(), proof);
        assert_eq!(
            proof.scale_centered(-3),
            proof
                .multiply(ProofPolynomialV1::constant(PROOF_MODULUS_V1 - 3).expect("canonical"))
        );
        let x = proof_monomial(1, 1);
        assert_eq!(
            x.multiply_by_monomial(63),
            ProofPolynomialV1::constant(PROOF_MODULUS_V1 - 1).expect("minus one")
        );
        assert_eq!(x.multiply_by_monomial(128), x);
    }

    #[test]
    fn zeroize_erases_every_residue() {
        let mut application =
            ApplicationPolynomialV1::from_centered_coefficients([7; APPLICATION_RING_DEGREE_V1]);
        let mut proof =
            ProofPolynomialV1::from_centered_coefficients([-9; APPLICATION_RING_DEGREE_V1]);
        application.zeroize();
        proof.zeroize();
        assert!(application.is_zero());
        assert!(proof.is_zero());
    }
}
