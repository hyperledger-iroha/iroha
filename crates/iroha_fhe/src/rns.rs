//! Residue number system arithmetic: chains, CRT reconstruction and basis extension.
//!
//! A chain is an ordered slice of pairwise-coprime word moduli. A polynomial is
//! limb-major: one residue vector per modulus, each with one residue per
//! coefficient. Reconstruction is exact mixed-radix (Garner) arithmetic in
//! `u128`; nothing is approximated and no floating point is used.
//!
//! Centered conventions, stated once and used everywhere in this crate:
//! a residue `x` modulo `M` represents `x` when `x <= floor(M / 2)` and
//! `x - M` otherwise, so the even-modulus midpoint `M / 2` is positive.
use crate::{
    constant_time::sub_mod_canonical_u64,
    modular::{
        add_mod_u64, gcd_u64, is_prime_u64, mod_inv_prime_u64, mod_pow_u64, mul_mod_u64,
        reduce_i128_to_u64_mod, reduce_u128_to_u64_mod, sub_mod_u64,
    },
    rounding::center_lift,
};
use thiserror::Error;
use zeroize::Zeroizing;

/// Largest limb count the stack-allocated reconstruction scratch supports.
pub const MAX_CRT_LIMBS: usize = 8;

/// Failure of an RNS kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RnsError {
    /// The modulus chain has no limb.
    #[error("modulus chain is empty")]
    EmptyChain,
    /// The product of the chain does not fit `u128`.
    #[error("modulus-chain product exceeds u128")]
    ProductOverflow,
    /// A modulus is zero.
    #[error("modulus chain contains a zero modulus")]
    ZeroModulus,
    /// The number of residues differs from the number of moduli.
    #[error("expected {expected} residues, found {found}")]
    ResidueCountMismatch {
        /// Number of moduli.
        expected: usize,
        /// Number of residues supplied.
        found: usize,
    },
    /// More limbs than [`MAX_CRT_LIMBS`].
    #[error("coefficient exceeds supported limb count {max_limbs}")]
    TooManyLimbs {
        /// The supported maximum.
        max_limbs: usize,
    },
    /// A limb has fewer residues than the polynomial degree.
    #[error("limb {limb_index} has fewer residues than the polynomial degree")]
    ShortLimb {
        /// Index of the short limb.
        limb_index: usize,
    },
    /// The reconstructed value does not fit `u128`.
    #[error("coefficient reconstruction exceeds u128")]
    ReconstructionOverflow,
    /// A CRT basis element has no inverse modulo its own limb.
    #[error("source limb {modulus} is not invertible")]
    LimbNotInvertible {
        /// The offending source modulus.
        modulus: u64,
    },
    /// A CRT term of the basis extension does not fit `u128`.
    #[error("basis-extension CRT term exceeds u128")]
    CrtTermOverflow,
    /// The quotient correction of the basis extension does not fit `u128`.
    #[error("basis-extension quotient exceeds u128")]
    QuotientOverflow,
    /// A residue to be centered is not below the source product.
    #[error("centered source residue exceeds the source-chain product")]
    CenteredSourceExceedsProduct,
    /// A centered value does not fit `i128`.
    #[error("centered reconstruction exceeds i128")]
    CenteredExceedsI128,
    /// A value to be centered is above the chain product.
    #[error("centered reconstruction exceeds the modulus-chain product")]
    CenteredExceedsProduct,
    /// A centered magnitude is above the bound the caller proved.
    #[error("centered reconstruction exceeds its magnitude bound")]
    CenteredExceedsBound,
}

/// Structural defect of an NTT-friendly modulus chain, in validation order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ModulusChainError {
    /// More limbs than the caller allows.
    #[error("modulus chain supports at most {max_limbs} limbs")]
    TooManyLimbs {
        /// The caller's maximum.
        max_limbs: usize,
    },
    /// A limb does not exceed the caller's exclusive lower bound.
    #[error("modulus limb {index} must exceed {bound}")]
    LimbNotAboveBound {
        /// Limb index.
        index: usize,
        /// The exclusive lower bound.
        bound: u64,
    },
    /// A limb is even.
    #[error("modulus limb {index} must be odd")]
    EvenLimb {
        /// Limb index.
        index: usize,
    },
    /// A limb does not exceed its predecessor.
    #[error("modulus limbs must be strictly increasing")]
    NotStrictlyIncreasing {
        /// Limb index.
        index: usize,
    },
    /// A limb is composite.
    #[error("modulus limb {index} must be prime")]
    CompositeLimb {
        /// Limb index.
        index: usize,
    },
    /// A limb is not congruent to one modulo the root order.
    #[error("modulus limb {index} must be 1 mod {root_order}")]
    LimbNotOneModOrder {
        /// Limb index.
        index: usize,
        /// Required root order.
        root_order: u64,
    },
    /// The caller's root source has no primitive root of the required order for a limb.
    #[error("modulus limb {index} has no supported primitive {root_order}-th root")]
    NoSupportedRoot {
        /// Limb index.
        index: usize,
        /// Required root order.
        root_order: u64,
    },
    /// A limb shares a factor with an earlier limb.
    #[error("modulus limbs must be pairwise coprime")]
    NotPairwiseCoprime {
        /// Limb index.
        index: usize,
    },
    /// The chain is empty.
    #[error("modulus chain must not be empty")]
    Empty,
    /// The product of the limbs does not fit `u128`.
    #[error("modulus-chain product exceeds u128")]
    ProductOverflow,
}

/// Checked product of a non-empty chain.
///
/// # Errors
/// [`RnsError::EmptyChain`] or [`RnsError::ProductOverflow`].
pub fn checked_modulus_product(moduli: &[u64]) -> Result<u128, RnsError> {
    if moduli.is_empty() {
        return Err(RnsError::EmptyChain);
    }
    moduli.iter().try_fold(1_u128, |product, &modulus| {
        product
            .checked_mul(u128::from(modulus))
            .ok_or(RnsError::ProductOverflow)
    })
}

/// Bit length of the exact product of `moduli`, computed in `max_words` 64-bit words.
///
/// The empty product is one, with bit length one. Returns `None` when the
/// product needs more than `max_words` words.
#[must_use]
pub fn modulus_product_bit_len(moduli: &[u64], max_words: usize) -> Option<usize> {
    let mut words = vec![0_u64; max_words];
    *words.first_mut()? = 1;
    for &modulus in moduli {
        let mut carry = 0_u128;
        for word in &mut words {
            let product = u128::from(*word) * u128::from(modulus) + carry;
            *word = crate::modular::low_u64_from_u128(product);
            carry = product >> 64;
        }
        if carry != 0 {
            return None;
        }
    }
    Some(
        words
            .iter()
            .rposition(|word| *word != 0)
            .map_or(0, |index| {
                index * 64 + (64 - words[index].leading_zeros() as usize)
            }),
    )
}

/// Validate an ordered chain of NTT-friendly primes and return its product.
///
/// Per limb, in this order: it exceeds `exclusive_lower_bound`, is odd,
/// exceeds its predecessor, is prime, is congruent to one modulo `root_order`,
/// has a root of that order according to `root_for(modulus, root_order)`, and
/// is coprime to every earlier limb. The chain must then be non-empty with a
/// product that fits `u128`. The first violated rule is reported.
///
/// # Errors
/// The first [`ModulusChainError`] in the order above.
pub fn validate_ntt_modulus_chain(
    moduli: &[u64],
    max_limbs: usize,
    exclusive_lower_bound: u64,
    root_order: u64,
    root_for: impl Fn(u64, u64) -> Option<u64>,
) -> Result<u128, ModulusChainError> {
    if moduli.len() > max_limbs {
        return Err(ModulusChainError::TooManyLimbs { max_limbs });
    }
    let mut previous = 0_u64;
    for (index, &modulus) in moduli.iter().enumerate() {
        if modulus <= exclusive_lower_bound {
            return Err(ModulusChainError::LimbNotAboveBound {
                index,
                bound: exclusive_lower_bound,
            });
        }
        if modulus.is_multiple_of(2) {
            return Err(ModulusChainError::EvenLimb { index });
        }
        if index > 0 && modulus <= previous {
            return Err(ModulusChainError::NotStrictlyIncreasing { index });
        }
        if !is_prime_u64(modulus) {
            return Err(ModulusChainError::CompositeLimb { index });
        }
        if !(modulus - 1).is_multiple_of(root_order) {
            return Err(ModulusChainError::LimbNotOneModOrder { index, root_order });
        }
        if root_for(modulus, root_order).is_none() {
            return Err(ModulusChainError::NoSupportedRoot { index, root_order });
        }
        if moduli[..index]
            .iter()
            .any(|&prior| gcd_u64(prior, modulus) != 1)
        {
            return Err(ModulusChainError::NotPairwiseCoprime { index });
        }
        previous = modulus;
    }
    checked_modulus_product(moduli).map_err(|error| match error {
        RnsError::EmptyChain => ModulusChainError::Empty,
        _ => ModulusChainError::ProductOverflow,
    })
}

/// Residues of word coefficients in every limb: `coefficient mod modulus`.
///
/// A zero modulus yields zero residues.
#[must_use]
pub fn decompose(coefficients: &[u64], moduli: &[u64]) -> Vec<Vec<u64>> {
    moduli
        .iter()
        .map(|&modulus| {
            coefficients
                .iter()
                .map(|&coefficient| coefficient.checked_rem(modulus).unwrap_or(0))
                .collect()
        })
        .collect()
}

/// Residues of the centered representatives of coefficients modulo `source_modulus`.
///
/// A coefficient above `floor(source_modulus / 2)` is first read as the negative
/// integer `coefficient - source_modulus`, so `q - a` decomposes as `-a`.
#[must_use]
pub fn decompose_centered(
    coefficients: &[u64],
    source_modulus: u64,
    moduli: &[u64],
) -> Vec<Vec<u64>> {
    moduli
        .iter()
        .map(|&modulus| {
            coefficients
                .iter()
                .map(|&coefficient| {
                    reduce_i128_to_u64_mod(center_lift(coefficient, source_modulus), modulus)
                })
                .collect()
        })
        .collect()
}

/// Mixed-radix (Garner) reconstruction of one coefficient in `[0, product)`.
///
/// The moduli must be pairwise-coprime primes; the mixed-radix digits live in
/// a clearing scratch buffer.
///
/// # Errors
/// [`RnsError::ResidueCountMismatch`], [`RnsError::TooManyLimbs`] or
/// [`RnsError::ReconstructionOverflow`].
pub fn reconstruct_coefficient(residues: &[u64], moduli: &[u64]) -> Result<u128, RnsError> {
    if residues.len() != moduli.len() {
        return Err(RnsError::ResidueCountMismatch {
            expected: moduli.len(),
            found: residues.len(),
        });
    }
    if residues.len() > MAX_CRT_LIMBS {
        return Err(RnsError::TooManyLimbs {
            max_limbs: MAX_CRT_LIMBS,
        });
    }
    let mut mixed = Zeroizing::new([0_u64; MAX_CRT_LIMBS]);
    for (index, (&residue, &modulus)) in residues.iter().zip(moduli).enumerate() {
        let mut coefficient = residue;
        for (&prior, &prior_modulus) in mixed[..index].iter().zip(moduli.iter()) {
            coefficient = mul_mod_u64(
                sub_mod_u64(coefficient, prior, modulus),
                mod_pow_u64(
                    prior_modulus.checked_rem(modulus).unwrap_or(0),
                    modulus.wrapping_sub(2),
                    modulus,
                ),
                modulus,
            );
        }
        mixed[index] = coefficient;
    }
    let mut value = 0_u128;
    let mut weight = 1_u128;
    for (index, &coefficient) in mixed[..residues.len()].iter().enumerate() {
        let term = u128::from(coefficient)
            .checked_mul(weight)
            .ok_or(RnsError::ReconstructionOverflow)?;
        value = value
            .checked_add(term)
            .ok_or(RnsError::ReconstructionOverflow)?;
        if index + 1 != residues.len() {
            weight = weight
                .checked_mul(u128::from(moduli[index]))
                .ok_or(RnsError::ReconstructionOverflow)?;
        }
    }
    Ok(value)
}

/// Reconstruct every coefficient of a limb-major polynomial.
///
/// The coefficients are built in a clearing buffer, so a rejected input leaves
/// no partial reconstruction behind.
///
/// # Errors
/// [`RnsError::ShortLimb`] when a limb has fewer than `degree` residues, or
/// any error of [`reconstruct_coefficient`].
pub fn reconstruct_polynomial(
    residues_by_limb: &[Vec<u64>],
    moduli: &[u64],
    degree: usize,
) -> Result<Vec<u128>, RnsError> {
    if residues_by_limb.len() != moduli.len() {
        return Err(RnsError::ResidueCountMismatch {
            expected: moduli.len(),
            found: residues_by_limb.len(),
        });
    }
    if moduli.len() > MAX_CRT_LIMBS {
        return Err(RnsError::TooManyLimbs {
            max_limbs: MAX_CRT_LIMBS,
        });
    }
    let mut coefficients = Zeroizing::new(Vec::with_capacity(degree));
    let mut residues = Zeroizing::new([0_u64; MAX_CRT_LIMBS]);
    for index in 0..degree {
        for (limb_index, limb) in residues_by_limb.iter().enumerate() {
            residues[limb_index] = *limb.get(index).ok_or(RnsError::ShortLimb { limb_index })?;
        }
        coefficients.push(reconstruct_coefficient(&residues[..moduli.len()], moduli)?);
    }
    Ok(std::mem::take(&mut *coefficients))
}

/// Reduce every coefficient into every target limb with `reduce`, limb-major.
///
/// Each limb is allocated at its final length and built in a clearing buffer,
/// so a rejected coefficient leaves no partial limb behind.
fn reduce_coefficients_into_limbs(
    coefficients: &[u128],
    target_moduli: &[u64],
    reduce: impl Fn(u128, u64) -> Result<u64, RnsError>,
) -> Result<Vec<Vec<u64>>, RnsError> {
    let mut limbs = Zeroizing::new(Vec::with_capacity(target_moduli.len()));
    for &modulus in target_moduli {
        let mut limb = Zeroizing::new(Vec::with_capacity(coefficients.len()));
        for &coefficient in coefficients {
            limb.push(reduce(coefficient, modulus)?);
        }
        limbs.push(std::mem::take(&mut *limb));
    }
    Ok(std::mem::take(&mut *limbs))
}

/// Reduce exact wide coefficients into every target limb.
///
/// # Errors
/// [`RnsError::ZeroModulus`] when a target modulus is zero.
pub fn reduce_into_limbs(
    coefficients: &[u128],
    target_moduli: &[u64],
) -> Result<Vec<Vec<u64>>, RnsError> {
    reduce_coefficients_into_limbs(coefficients, target_moduli, |coefficient, modulus| {
        reduce_u128_to_u64_mod(coefficient, modulus).ok_or(RnsError::ZeroModulus)
    })
}

/// Reduce centered wide coefficients modulo `source_product` into every target limb.
///
/// Each coefficient is read with the centered convention of this module
/// relative to `source_product`, then reduced as a signed integer.
///
/// # Errors
/// [`RnsError::CenteredSourceExceedsProduct`] or [`RnsError::ZeroModulus`].
pub fn reduce_centered_into_limbs(
    coefficients: &[u128],
    source_product: u128,
    target_moduli: &[u64],
) -> Result<Vec<Vec<u64>>, RnsError> {
    reduce_coefficients_into_limbs(coefficients, target_moduli, |coefficient, modulus| {
        reduce_centered_source_residue_to_u64_mod(coefficient, source_product, modulus)
    })
}

/// Exact basis extension of a limb-major polynomial into target limbs.
///
/// Each coefficient is the canonical representative modulo the source product
/// `Q = q_0 * ... * q_k`. With `Q_i = Q / q_i` and CRT digits
/// `d_i = x_i * Q_i^-1 mod q_i`, the integer `sum_i d_i * Q_i` equals
/// `x + v * Q` for an exactly computed quotient `v`, and each target residue is
/// `sum_i d_i * Q_i - v * Q` reduced into the target limb. The quotient is
/// counted by exact `u128` comparisons, not estimated, so the result is the
/// exact residue of the canonical representative. The target chain need not
/// cover the source product. The CRT digits and the target limbs under
/// construction live in clearing buffers.
///
/// # Errors
/// Any error of [`checked_modulus_product`], [`RnsError::LimbNotInvertible`],
/// [`RnsError::ShortLimb`], [`RnsError::ResidueCountMismatch`],
/// [`RnsError::CrtTermOverflow`], [`RnsError::QuotientOverflow`] or
/// [`RnsError::ZeroModulus`].
pub fn basis_extend_target_limbs(
    residues_by_limb: &[Vec<u64>],
    source_moduli: &[u64],
    target_moduli: &[u64],
    degree: usize,
) -> Result<Vec<Vec<u64>>, RnsError> {
    if residues_by_limb.len() != source_moduli.len() {
        return Err(RnsError::ResidueCountMismatch {
            expected: source_moduli.len(),
            found: residues_by_limb.len(),
        });
    }
    let source_product = checked_modulus_product(source_moduli)?;
    let source_limb_data = source_moduli
        .iter()
        .map(|&source_modulus| {
            let source_basis = source_product
                .checked_div(u128::from(source_modulus))
                .ok_or(RnsError::ZeroModulus)?;
            let source_basis_mod_source = reduce_u128_to_u64_mod(source_basis, source_modulus)
                .ok_or(RnsError::ZeroModulus)?;
            let inverse = mod_inv_prime_u64(source_basis_mod_source, source_modulus).ok_or(
                RnsError::LimbNotInvertible {
                    modulus: source_modulus,
                },
            )?;
            Ok((source_modulus, source_basis, inverse))
        })
        .collect::<Result<Vec<_>, RnsError>>()?;
    // Each limb is allocated at its final length: a growing buffer would leave copies behind.
    let mut output = Zeroizing::new(
        target_moduli
            .iter()
            .map(|_| Vec::with_capacity(degree))
            .collect::<Vec<Vec<u64>>>(),
    );
    let mut crt_digits = Zeroizing::new(Vec::with_capacity(source_limb_data.len()));
    for coefficient_index in 0..degree {
        let mut quotient = 0_u128;
        let mut remainder = 0_u128;
        crt_digits.clear();
        for (limb_index, &(source_modulus, source_basis, inverse)) in
            source_limb_data.iter().enumerate()
        {
            let residue = *residues_by_limb[limb_index]
                .get(coefficient_index)
                .ok_or(RnsError::ShortLimb { limb_index })?;
            let crt_digit = mul_mod_u64(residue, inverse, source_modulus);
            let term = source_basis
                .checked_mul(u128::from(crt_digit))
                .ok_or(RnsError::CrtTermOverflow)?;
            if remainder >= source_product - term {
                remainder -= source_product - term;
                quotient = quotient.checked_add(1).ok_or(RnsError::QuotientOverflow)?;
            } else {
                remainder += term;
            }
            crt_digits.push((crt_digit, source_basis));
        }
        for (target_limb, &target_modulus) in output.iter_mut().zip(target_moduli) {
            let reduce = |value: u128| {
                reduce_u128_to_u64_mod(value, target_modulus).ok_or(RnsError::ZeroModulus)
            };
            let mut target_residue = 0_u64;
            for &(crt_digit, source_basis) in crt_digits.iter() {
                target_residue = add_mod_u64(
                    target_residue,
                    mul_mod_u64(
                        reduce(u128::from(crt_digit))?,
                        reduce(source_basis)?,
                        target_modulus,
                    ),
                    target_modulus,
                );
            }
            let correction =
                mul_mod_u64(reduce(quotient)?, reduce(source_product)?, target_modulus);
            target_limb.push(sub_mod_u64(target_residue, correction, target_modulus));
        }
    }
    Ok(std::mem::take(&mut *output))
}

/// Reduce the centered representative of `value` modulo `source_product` into `target_modulus`.
///
/// # Errors
/// [`RnsError::CenteredSourceExceedsProduct`] when `value >= source_product`,
/// or [`RnsError::ZeroModulus`].
pub fn reduce_centered_source_residue_to_u64_mod(
    value: u128,
    source_product: u128,
    target_modulus: u64,
) -> Result<u64, RnsError> {
    if value >= source_product {
        return Err(RnsError::CenteredSourceExceedsProduct);
    }
    let reduce =
        |value: u128| reduce_u128_to_u64_mod(value, target_modulus).ok_or(RnsError::ZeroModulus);
    if value <= source_product / 2 {
        return reduce(value);
    }
    Ok(sub_mod_canonical_u64(
        0,
        reduce(source_product - value)?,
        target_modulus,
    ))
}

/// Reduce a bounded centered value modulo the chain product into `modulus`.
///
/// `value` is accepted as non-negative when `value <= centered_abs_bound` and
/// as the negative integer `value - rns_product` when
/// `rns_product - value <= centered_abs_bound`; anything else is rejected, so
/// a chain too narrow for the bound cannot alias silently.
///
/// # Errors
/// [`RnsError::CenteredExceedsProduct`], [`RnsError::CenteredExceedsBound`] or
/// [`RnsError::ZeroModulus`].
pub fn reduce_centered_value_to_u64_mod(
    value: u128,
    rns_product: u128,
    centered_abs_bound: u128,
    modulus: u64,
) -> Result<u64, RnsError> {
    let reduce = |value: u128| reduce_u128_to_u64_mod(value, modulus).ok_or(RnsError::ZeroModulus);
    if value <= centered_abs_bound {
        return reduce(value);
    }
    let negative_magnitude = rns_product
        .checked_sub(value)
        .ok_or(RnsError::CenteredExceedsProduct)?;
    if negative_magnitude > centered_abs_bound {
        return Err(RnsError::CenteredExceedsBound);
    }
    Ok(sub_mod_canonical_u64(
        0,
        reduce(negative_magnitude)?,
        modulus,
    ))
}

/// Signed value of a bounded centered residue modulo the chain product.
///
/// Acceptance follows [`reduce_centered_value_to_u64_mod`].
///
/// # Errors
/// [`RnsError::CenteredExceedsI128`], [`RnsError::CenteredExceedsProduct`] or
/// [`RnsError::CenteredExceedsBound`].
pub fn reduce_centered_value_to_i128(
    value: u128,
    rns_product: u128,
    centered_abs_bound: u128,
) -> Result<i128, RnsError> {
    if value <= centered_abs_bound {
        return i128::try_from(value).map_err(|_| RnsError::CenteredExceedsI128);
    }
    let negative_magnitude = rns_product
        .checked_sub(value)
        .ok_or(RnsError::CenteredExceedsProduct)?;
    if negative_magnitude > centered_abs_bound {
        return Err(RnsError::CenteredExceedsBound);
    }
    i128::try_from(negative_magnitude)
        .map_err(|_| RnsError::CenteredExceedsI128)?
        .checked_neg()
        .ok_or(RnsError::CenteredExceedsI128)
}

/// Limb-wise sum of two limb-major polynomials.
#[must_use]
pub fn add_limbs(lhs: &[Vec<u64>], rhs: &[Vec<u64>], moduli: &[u64]) -> Vec<Vec<u64>> {
    lhs.iter()
        .zip(rhs)
        .zip(moduli)
        .map(|((lhs_limb, rhs_limb), &modulus)| {
            crate::polynomial::add_mod(lhs_limb, rhs_limb, modulus)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTERED: [u64; 8] = [
        30_593, 30_977, 31_489, 31_873, 32_257, 33_409, 35_201, 35_969,
    ];

    fn splitmix64(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = *state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }
    fn residues_of(value: u128, moduli: &[u64]) -> Vec<u64> {
        moduli
            .iter()
            .map(|&modulus| u64::try_from(value % u128::from(modulus)).unwrap())
            .collect()
    }
    fn any_root(modulus: u64, order: u64) -> Option<u64> {
        crate::modular::primitive_root_of_order_with_candidate_limit(modulus, order, 4_096)
    }

    #[test]
    fn product_is_checked() {
        assert_eq!(checked_modulus_product(&[]), Err(RnsError::EmptyChain));
        assert_eq!(checked_modulus_product(&[7, 11]), Ok(77));
        assert_eq!(
            checked_modulus_product(&[u64::MAX, u64::MAX, 2]),
            Err(RnsError::ProductOverflow)
        );
        assert_eq!(
            checked_modulus_product(&[u64::MAX, u64::MAX]),
            Ok(u128::from(u64::MAX) * u128::from(u64::MAX))
        );
    }

    #[test]
    fn wide_product_bit_length_is_exact_and_bounded() {
        assert_eq!(modulus_product_bit_len(&[], 2), Some(1));
        assert_eq!(modulus_product_bit_len(&[], 0), None);
        assert_eq!(modulus_product_bit_len(&[0, 5], 2), Some(0));
        assert_eq!(
            modulus_product_bit_len(&[2_013_265_921, 1_811_939_329], 4),
            Some(62)
        );
        assert_eq!(modulus_product_bit_len(&[u64::MAX], 1), Some(64));
        assert_eq!(modulus_product_bit_len(&[u64::MAX, 2], 1), None);
        assert_eq!(modulus_product_bit_len(&[u64::MAX, 2], 2), Some(65));
        // (2^64 - 1)^3 = 2^192 - 3 * 2^128 + ..., which needs 192 bits.
        assert_eq!(modulus_product_bit_len(&[u64::MAX; 3], 3), Some(192));
        assert_eq!(modulus_product_bit_len(&[u64::MAX; 3], 2), None);
        let product = checked_modulus_product(&REGISTERED).unwrap();
        assert_eq!(
            modulus_product_bit_len(&REGISTERED, 2),
            Some((u128::BITS - product.leading_zeros()) as usize)
        );
    }

    #[test]
    fn chain_validation_reports_the_first_defect_in_order() {
        let validate = |moduli: &[u64], bound: u64| {
            validate_ntt_modulus_chain(moduli, 8, bound, 128, any_root)
        };
        assert_eq!(
            validate(&REGISTERED, 257),
            Ok(checked_modulus_product(&REGISTERED).unwrap())
        );
        assert_eq!(validate(&[], 257), Err(ModulusChainError::Empty));
        assert_eq!(
            validate_ntt_modulus_chain(&[30_593; 9], 8, 257, 128, any_root),
            Err(ModulusChainError::TooManyLimbs { max_limbs: 8 })
        );
        assert_eq!(
            validate(&[257, 30_593], 257),
            Err(ModulusChainError::LimbNotAboveBound {
                index: 0,
                bound: 257
            })
        );
        assert_eq!(
            validate(&[0], 0),
            Err(ModulusChainError::LimbNotAboveBound { index: 0, bound: 0 })
        );
        assert_eq!(
            validate(&[30_593, 30_978], 257),
            Err(ModulusChainError::EvenLimb { index: 1 })
        );
        assert_eq!(
            validate(&[30_593, 30_593], 257),
            Err(ModulusChainError::NotStrictlyIncreasing { index: 1 })
        );
        assert_eq!(
            validate(&[385, 30_593], 257),
            Err(ModulusChainError::CompositeLimb { index: 0 })
        );
        assert_eq!(
            validate(&[263, 30_593], 257),
            Err(ModulusChainError::LimbNotOneModOrder {
                index: 0,
                root_order: 128
            })
        );
        assert_eq!(
            validate_ntt_modulus_chain(&[30_593], 8, 257, 128, |_, _| None),
            Err(ModulusChainError::NoSupportedRoot {
                index: 0,
                root_order: 128
            })
        );
        // The three largest 64-bit primes overflow u128.
        let wide = [
            18_446_744_073_709_551_521_u64,
            18_446_744_073_709_551_533,
            18_446_744_073_709_551_557,
        ];
        assert!(wide.iter().all(|&modulus| is_prime_u64(modulus)));
        assert_eq!(
            validate_ntt_modulus_chain(&wide, 8, 257, 2, |_, _| Some(1)),
            Err(ModulusChainError::ProductOverflow)
        );
        // Pairwise coprimality is implied by the earlier rules for increasing primes.
        assert_eq!(
            validate_ntt_modulus_chain(&[3, 5, 7], 8, 2, 2, |_, _| Some(1)),
            Ok(105)
        );
    }

    #[test]
    fn decomposition_and_reconstruction_round_trip_every_boundary() {
        let product = checked_modulus_product(&REGISTERED).unwrap();
        let mut state = 0xC0_u64;
        let mut values = vec![
            0_u128,
            1,
            product - 1,
            product / 2,
            product / 2 + 1,
            u128::from(u64::MAX),
        ];
        for _ in 0..256 {
            let wide =
                (u128::from(splitmix64(&mut state)) << 64) | u128::from(splitmix64(&mut state));
            values.push(wide % product);
        }
        for value in values {
            let residues = residues_of(value, &REGISTERED);
            assert_eq!(reconstruct_coefficient(&residues, &REGISTERED), Ok(value));
        }
        let coefficients = [0_u64, 1, 269_484_031, 134_742_016, u64::MAX];
        let limbs = decompose(&coefficients, &REGISTERED);
        assert_eq!(limbs.len(), REGISTERED.len());
        let reconstructed =
            reconstruct_polynomial(&limbs, &REGISTERED, coefficients.len()).unwrap();
        for (coefficient, wide) in coefficients.iter().zip(reconstructed) {
            assert_eq!(
                u128::from(*coefficient),
                wide,
                "every word is below the registered product"
            );
        }
        assert_eq!(decompose(&[5], &[0]), vec![vec![0]]);
    }

    #[test]
    fn reconstruction_rejects_bad_shapes_and_overflow() {
        assert_eq!(
            reconstruct_coefficient(&[1], &[3, 5]),
            Err(RnsError::ResidueCountMismatch {
                expected: 2,
                found: 1
            })
        );
        assert_eq!(
            reconstruct_coefficient(&[0; 9], &[3; 9]),
            Err(RnsError::TooManyLimbs {
                max_limbs: MAX_CRT_LIMBS
            })
        );
        assert_eq!(
            reconstruct_coefficient(&[u64::MAX - 1, 0, 1], &[u64::MAX, u64::MAX - 2, 3]),
            Err(RnsError::ReconstructionOverflow)
        );
        assert_eq!(reconstruct_coefficient(&[], &[]), Ok(0));
        assert_eq!(
            reconstruct_polynomial(&[vec![1, 2], vec![1]], &[3, 5], 2),
            Err(RnsError::ShortLimb { limb_index: 1 })
        );
        assert_eq!(
            reconstruct_polynomial(&[vec![1, 2]], &[3, 5], 2),
            Err(RnsError::ResidueCountMismatch {
                expected: 2,
                found: 1
            })
        );
        assert_eq!(
            reconstruct_polynomial(&vec![vec![0]; 9], &[3; 9], 1),
            Err(RnsError::TooManyLimbs {
                max_limbs: MAX_CRT_LIMBS
            })
        );
    }

    #[test]
    fn centered_decomposition_reads_the_upper_half_as_negative() {
        let source = 269_484_032_u64; // even: the midpoint stays positive
        let moduli = [30_593_u64, 30_977];
        let coefficients = [0, 1, source / 2, source / 2 + 1, source - 1];
        let limbs = decompose_centered(&coefficients, source, &moduli);
        let signed = [
            0_i128,
            1,
            i128::from(source / 2),
            -i128::from(source / 2 - 1),
            -1,
        ];
        for (limb, &modulus) in limbs.iter().zip(&moduli) {
            for (residue, value) in limb.iter().zip(signed) {
                assert_eq!(
                    *residue,
                    u64::try_from(value.rem_euclid(i128::from(modulus))).unwrap()
                );
            }
        }
    }

    #[test]
    fn target_limb_basis_extension_is_the_exact_residue_of_the_canonical_representative() {
        let source = &REGISTERED[..3];
        let target = [35_201_u64, 35_969, 2_013_265_921, 3];
        let product = checked_modulus_product(source).unwrap();
        let mut state = 0xC1_u64;
        let mut values = vec![0_u128, 1, product - 1, product / 2, product / 2 + 1];
        values.extend((0..200).map(|_| u128::from(splitmix64(&mut state)) % product));
        let degree = values.len();
        let limbs: Vec<Vec<u64>> = source
            .iter()
            .map(|&modulus| {
                values
                    .iter()
                    .map(|value| u64::try_from(value % u128::from(modulus)).unwrap())
                    .collect()
            })
            .collect();
        let extended = basis_extend_target_limbs(&limbs, source, &target, degree).expect("extend");
        for (limb, &modulus) in extended.iter().zip(&target) {
            for (residue, value) in limb.iter().zip(&values) {
                assert_eq!(u128::from(*residue), value % u128::from(modulus));
            }
        }
        // The reconstructing path agrees with the quotient-corrected path.
        let coefficients = reconstruct_polynomial(&limbs, source, degree).unwrap();
        assert_eq!(coefficients, values);
        assert_eq!(reduce_into_limbs(&coefficients, &target).unwrap(), extended);
    }

    #[test]
    fn basis_extension_rejects_malformed_chains_and_shapes() {
        assert_eq!(
            basis_extend_target_limbs(&[vec![1]], &[], &[7], 1),
            Err(RnsError::ResidueCountMismatch {
                expected: 0,
                found: 1
            })
        );
        assert_eq!(
            basis_extend_target_limbs(&[], &[], &[7], 1),
            Err(RnsError::EmptyChain)
        );
        assert_eq!(
            basis_extend_target_limbs(&[vec![1], vec![1]], &[3, 3], &[7], 1),
            Err(RnsError::LimbNotInvertible { modulus: 3 })
        );
        assert_eq!(
            basis_extend_target_limbs(&[vec![1], vec![1]], &[2, 5], &[7], 1),
            Err(RnsError::LimbNotInvertible { modulus: 2 })
        );
        assert_eq!(
            basis_extend_target_limbs(&[vec![1], vec![]], &[5, 7], &[11], 1),
            Err(RnsError::ShortLimb { limb_index: 1 })
        );
        assert_eq!(
            basis_extend_target_limbs(&[vec![1], vec![1]], &[5, 7], &[0], 1),
            Err(RnsError::ZeroModulus)
        );
        assert_eq!(reduce_into_limbs(&[5], &[0]), Err(RnsError::ZeroModulus));
        assert_eq!(
            basis_extend_target_limbs(&[vec![1], vec![1]], &[0, 7], &[11], 1),
            Err(RnsError::ZeroModulus)
        );
    }

    #[test]
    fn centered_reductions_cover_the_half_boundary_and_their_error_cases() {
        // Odd product 35: floor(35/2) = 17 is the largest positive representative.
        assert_eq!(reduce_centered_source_residue_to_u64_mod(17, 35, 11), Ok(6));
        assert_eq!(
            reduce_centered_source_residue_to_u64_mod(18, 35, 11),
            Ok(5),
            "-17 mod 11"
        );
        assert_eq!(
            reduce_centered_source_residue_to_u64_mod(34, 35, 11),
            Ok(10),
            "-1 mod 11"
        );
        // Even product 36: the midpoint 18 is positive, 19 is -17.
        assert_eq!(reduce_centered_source_residue_to_u64_mod(18, 36, 11), Ok(7));
        assert_eq!(reduce_centered_source_residue_to_u64_mod(19, 36, 11), Ok(5));
        assert_eq!(reduce_centered_source_residue_to_u64_mod(13, 35, 12), Ok(1));
        assert_eq!(
            reduce_centered_source_residue_to_u64_mod(23, 35, 12),
            Ok(0),
            "-12 mod 12"
        );
        assert_eq!(
            reduce_centered_source_residue_to_u64_mod(35, 35, 11),
            Err(RnsError::CenteredSourceExceedsProduct)
        );
        assert_eq!(
            reduce_centered_source_residue_to_u64_mod(1, 35, 0),
            Err(RnsError::ZeroModulus)
        );
        assert_eq!(
            reduce_centered_into_limbs(&[17, 18, 34], 35, &[11, 12]),
            Ok(vec![vec![6, 5, 10], vec![5, 7, 11]])
        );

        assert_eq!(reduce_centered_value_to_u64_mod(4, 35, 4, 11), Ok(4));
        assert_eq!(
            reduce_centered_value_to_u64_mod(31, 35, 4, 11),
            Ok(7),
            "-4 mod 11"
        );
        assert_eq!(
            reduce_centered_value_to_u64_mod(24, 35, 11, 11),
            Ok(0),
            "-11 mod 11"
        );
        assert_eq!(
            reduce_centered_value_to_u64_mod(5, 35, 4, 11),
            Err(RnsError::CenteredExceedsBound)
        );
        assert_eq!(
            reduce_centered_value_to_u64_mod(30, 35, 4, 11),
            Err(RnsError::CenteredExceedsBound)
        );
        assert_eq!(
            reduce_centered_value_to_u64_mod(36, 35, 4, 11),
            Err(RnsError::CenteredExceedsProduct)
        );
        assert_eq!(
            reduce_centered_value_to_u64_mod(4, 35, 4, 0),
            Err(RnsError::ZeroModulus)
        );

        assert_eq!(reduce_centered_value_to_i128(4, 35, 4), Ok(4));
        assert_eq!(reduce_centered_value_to_i128(31, 35, 4), Ok(-4));
        assert_eq!(
            reduce_centered_value_to_i128(5, 35, 4),
            Err(RnsError::CenteredExceedsBound)
        );
        assert_eq!(
            reduce_centered_value_to_i128(36, 35, 4),
            Err(RnsError::CenteredExceedsProduct)
        );
        assert_eq!(
            reduce_centered_value_to_i128(u128::MAX, u128::MAX, u128::MAX),
            Err(RnsError::CenteredExceedsI128)
        );
        assert_eq!(
            reduce_centered_value_to_i128(1, u128::MAX, 0),
            Err(RnsError::CenteredExceedsBound)
        );
        // A negative magnitude accepted by the bound always fits i128.
        assert_eq!(
            reduce_centered_value_to_i128((1_u128 << 127) + 2, u128::MAX, (1_u128 << 127) + 1),
            Ok(-((1_i128 << 126) + ((1_i128 << 126) - 3)))
        );
    }

    #[test]
    fn limbwise_sum_reduces_each_limb() {
        let moduli = [5_u64, 7];
        assert_eq!(
            add_limbs(
                &[vec![4, 1], vec![6, 2]],
                &[vec![4, 4], vec![6, 5]],
                &moduli
            ),
            vec![vec![3, 0], vec![5, 0]]
        );
    }
}
