//! Bounded public prime-power lifting polynomials and upper-digit programs.
//!
//! For an odd prime `p`, precision `e`, and each balanced digit `a`, interpolate
//! `H(a) = a - a^p (mod p^e)` with degree less than `p`. The polynomial
//! `F(X) = X^p + H(X)` fixes those representatives and has derivative zero modulo
//! `p`: all coefficients of `H` are divisible by `p`. Taylor expansion therefore
//! sends agreement modulo `p^k` to agreement modulo `p^(k+1)`, for `k >= 1`.
//! The complete upper-digit tableau uses these lifts, subtraction and exact
//! division by `p`; a lift alone would return only the lowest digit.
//!
//! This is the public arithmetic preparation used in the Halevi--Shoup lifting
//! construction (Bootstrapping for HElib, section 5.3), with the upper-digit
//! tableau described in Geelen--Vercauteren, Bootstrapping for BGV and BFV
//! Revisited, section 6.1. Polynomial coefficients and program instructions are
//! public. All modular polynomial arithmetic reuses [`FixedModulus`]. There is
//! no ciphertext execution, decryption fallback or BGV profile admission here.
//!
//! Word contexts require `p^e < 2^63`; for `p = 257`, precisions one through seven
//! fit. An unsupported precision is refused, never truncated. Existing BGV
//! plaintext admission is unchanged. TODO: integrate a proved plaintext-
//! precision transition, authenticated encrypted-secret relation, exact noise
//! transfers and the complete coefficient/slot maps before ciphertext refresh.

use core::mem::size_of;

use zeroize::{Zeroize, Zeroizing};

use crate::{constant_time::FixedModulus, modular::is_prime_u64};

/// Refusal while validating or preparing a public prime-power program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PrimePowerError {
    /// The base is not an odd prime or the precision is zero.
    #[error("invalid prime-power context")]
    InvalidContext,
    /// The exact requested prime power does not fit the canonical word kernel.
    #[error("prime-power precision exceeds the word kernel")]
    UnsupportedPrecision,
    /// A public payload or work count cannot be represented.
    #[error("prime-power geometry overflow")]
    GeometryOverflow,
    /// At least one explicit public compiler ceiling is insufficient.
    #[error("prime-power compiler budget exceeded")]
    Budget,
    /// A fallible public allocation was refused.
    #[error("prime-power allocation refused")]
    Allocation,
    /// The requested retained precision is not strictly between zero and `e`.
    #[error("invalid retained-digit count")]
    RetainedDigits,
    /// A public construction invariant failed.
    #[error("prime-power construction invariant failed")]
    Invariant,
}

/// Validated arithmetic context for the exact public modulus `prime^precision`.
///
/// This value is not a ciphertext profile or a parameter qualification token.
/// It owns no allocation and admits no implicit smaller precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrimePowerContext {
    prime: u64,
    precision: u32,
    arithmetic: FixedModulus,
}

impl PrimePowerContext {
    /// Validate an odd prime and its exact power below `2^63`.
    ///
    /// # Errors
    /// Refuses a non-prime/even base, zero precision, or an over-word power.
    pub fn new(prime: u64, precision: u32) -> Result<Self, PrimePowerError> {
        if prime < 3 || prime.is_multiple_of(2) || precision == 0 {
            return Err(PrimePowerError::InvalidContext);
        }
        // 3^40 exceeds the kernel limit, so even hostile precision inputs take
        // at most 39 checked public multiplications before primality testing.
        if precision > 39 {
            return Err(PrimePowerError::UnsupportedPrecision);
        }
        let modulus = checked_power(prime, precision)?;
        if !is_prime_u64(prime) {
            return Err(PrimePowerError::InvalidContext);
        }
        let arithmetic =
            FixedModulus::derive(modulus).ok_or(PrimePowerError::UnsupportedPrecision)?;
        Ok(Self {
            prime,
            precision,
            arithmetic,
        })
    }

    /// The odd prime base.
    #[must_use]
    pub const fn prime(self) -> u64 {
        self.prime
    }

    /// The exact base-prime precision.
    #[must_use]
    pub const fn precision(self) -> u32 {
        self.precision
    }

    /// The exact modulus, always below `2^63`.
    #[must_use]
    pub const fn modulus(self) -> u64 {
        self.arithmetic.modulus
    }

    fn at_precision(self, precision: u32) -> Result<Self, PrimePowerError> {
        if precision == 0 || precision > self.precision {
            return Err(PrimePowerError::Invariant);
        }
        let modulus = checked_power(self.prime, precision)?;
        Ok(Self {
            prime: self.prime,
            precision,
            arithmetic: FixedModulus::derive(modulus).ok_or(PrimePowerError::Invariant)?,
        })
    }
}

fn checked_power(prime: u64, precision: u32) -> Result<u64, PrimePowerError> {
    let mut value = 1_u64;
    for _ in 0..precision {
        value = value
            .checked_mul(prime)
            .filter(|&next| next < 1_u64 << 63)
            .ok_or(PrimePowerError::UnsupportedPrecision)?;
    }
    Ok(value)
}

fn checked_bytes(words: usize) -> Result<usize, PrimePowerError> {
    words
        .checked_mul(size_of::<u64>())
        .ok_or(PrimePowerError::GeometryOverflow)
}

fn power_operations(exponent: u64) -> u64 {
    u64::from(u64::BITS - exponent.leading_zeros() + exponent.count_ones())
}

/// Explicit ceilings for one public lifting-polynomial preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiftingPolynomialBudget {
    /// Retained coefficient element bytes; allocator headers are excluded.
    pub coefficient_bytes: usize,
    /// Additional product/quotient element bytes during preparation.
    pub scratch_bytes: usize,
    /// Canonical add/subtract/multiply calls, including public power operations.
    pub arithmetic_operations: u64,
}

/// Exact public element payloads and arithmetic work for one preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiftingPolynomialGeometry {
    /// Retained `p + 1` coefficients, each eight bytes.
    pub coefficient_bytes: usize,
    /// Additional `2*p + 1` product/quotient words; no quadratic matrix.
    pub scratch_bytes: usize,
    /// Exact canonical add/subtract/multiply calls for this public context.
    ///
    /// Primality/context setup, integer indexing, allocation/clearing, and the
    /// `5*p` coefficient/unit/remainder/inverse/interpolation checks are separate.
    pub arithmetic_operations: u64,
}

/// Admitted preparation of a canonical degree-`p` lifting polynomial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiftingPolynomialPlan {
    context: PrimePowerContext,
    geometry: LiftingPolynomialGeometry,
}

impl LiftingPolynomialPlan {
    /// Compute exact checked payload/work geometry without allocating.
    ///
    /// # Errors
    /// Refuses an unrepresentable degree, byte count or arithmetic count.
    pub fn required(
        context: PrimePowerContext,
    ) -> Result<LiftingPolynomialGeometry, PrimePowerError> {
        let prime =
            usize::try_from(context.prime).map_err(|_| PrimePowerError::GeometryOverflow)?;
        let coefficients = prime
            .checked_add(1)
            .ok_or(PrimePowerError::GeometryOverflow)?;
        let scratch = prime
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or(PrimePowerError::GeometryOverflow)?;
        let phi = context.modulus() - context.modulus() / context.prime;
        let powers = power_operations(context.prime) + power_operations(phi - 1);
        let arithmetic_operations = context
            .prime
            .checked_mul(context.prime)
            .and_then(|value| value.checked_mul(9))
            .and_then(|value| {
                context
                    .prime
                    .checked_mul(8 + powers)
                    .and_then(|term| value.checked_add(term))
            })
            .ok_or(PrimePowerError::GeometryOverflow)?;
        Ok(LiftingPolynomialGeometry {
            coefficient_bytes: checked_bytes(coefficients)?,
            scratch_bytes: checked_bytes(scratch)?,
            arithmetic_operations,
        })
    }

    /// Admit all degree-sized buffers and public polynomial work before allocation.
    ///
    /// # Errors
    /// Returns geometry overflow or an insufficient explicit ceiling.
    pub fn new(
        context: PrimePowerContext,
        budget: LiftingPolynomialBudget,
    ) -> Result<Self, PrimePowerError> {
        let geometry = Self::required(context)?;
        if geometry.coefficient_bytes > budget.coefficient_bytes
            || geometry.scratch_bytes > budget.scratch_bytes
            || geometry.arithmetic_operations > budget.arithmetic_operations
        {
            return Err(PrimePowerError::Budget);
        }
        Ok(Self { context, geometry })
    }

    /// The admitted arithmetic context.
    #[must_use]
    pub const fn context(self) -> PrimePowerContext {
        self.context
    }

    /// Exact admitted coefficient/scratch payload and polynomial arithmetic work.
    #[must_use]
    pub const fn geometry(self) -> LiftingPolynomialGeometry {
        self.geometry
    }
}

struct Coefficients(Zeroizing<Vec<u64>>);

impl Coefficients {
    fn zeroed(length: usize) -> Result<Self, PrimePowerError> {
        allocation_checkpoint()?;
        let mut words = Zeroizing::new(Vec::new());
        words
            .try_reserve_exact(length)
            .map_err(|_| PrimePowerError::Allocation)?;
        words.resize(length, 0);
        Ok(Self(words))
    }
}

impl Drop for Coefficients {
    fn drop(&mut self) {
        self.0.as_mut_slice().zeroize();
        #[cfg(test)]
        tests::observe_cleared(&self.0);
        // The canonical Zeroizing<Vec> also clears any spare allocation before
        // release; the explicit slice clear above permits release-path tests.
    }
}

fn allocation_checkpoint() -> Result<(), PrimePowerError> {
    #[cfg(test)]
    tests::allocation_checkpoint()?;
    Ok(())
}

struct PreparationArithmetic {
    fixed: FixedModulus,
    operations: u64,
}

impl PreparationArithmetic {
    fn add(&mut self, lhs: u64, rhs: u64) -> u64 {
        self.operations += 1;
        self.fixed.add_canonical(lhs, rhs)
    }

    fn sub(&mut self, lhs: u64, rhs: u64) -> u64 {
        self.operations += 1;
        self.fixed.sub_canonical(lhs, rhs)
    }

    fn mul(&mut self, lhs: u64, rhs: u64) -> u64 {
        self.operations += 1;
        self.fixed.multiply(lhs, rhs)
    }

    fn power(&mut self, base: u64, exponent: u64) -> u64 {
        self.operations += power_operations(exponent);
        self.fixed.power(base, exponent)
    }

    fn at_public_node(&mut self, coefficients: &[u64], node: u64) -> u64 {
        let mut value = 0;
        for &coefficient in coefficients.iter().rev() {
            let product = self.mul(value, node);
            value = self.add(product, coefficient);
        }
        value
    }
}

fn public_node(index: usize, half: usize, modulus: u64) -> u64 {
    if index < half {
        modulus - (half - index) as u64
    } else {
        (index - half) as u64
    }
}

/// Immutable canonical public lifting coefficients, cleared on every release.
///
/// Coefficients are in increasing degree order. The owner is not cloneable;
/// borrowers can inspect the public polynomial without duplicating its storage.
pub struct LiftingPolynomial {
    context: PrimePowerContext,
    coefficients: Coefficients,
}

impl core::fmt::Debug for LiftingPolynomial {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LiftingPolynomial")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl LiftingPolynomial {
    /// Prepare the admitted degree-`p` polynomial using canonical word kernels.
    ///
    /// All inputs and branches in this compiler are public. Denominators are
    /// units modulo `p^e`; Euler's exponent `phi(p^e)-1` gives their inverses.
    /// Fermat inversion modulo the composite modulus would be incorrect.
    ///
    /// # Errors
    /// Returns allocation refusal or a violated public construction invariant.
    pub fn prepare(plan: &LiftingPolynomialPlan) -> Result<Self, PrimePowerError> {
        let context = plan.context;
        let prime =
            usize::try_from(context.prime).map_err(|_| PrimePowerError::GeometryOverflow)?;
        let mut coefficients = Coefficients::zeroed(prime + 1)?;
        let mut product = Coefficients::zeroed(prime + 1)?;
        let mut quotient = Coefficients::zeroed(prime)?;
        let mut arithmetic = PreparationArithmetic {
            fixed: context.arithmetic,
            operations: 0,
        };
        let modulus = context.modulus();
        let half = prime / 2;
        product.0[0] = 1;
        for degree in 0..prime {
            let node = public_node(degree, half, modulus);
            for power in (1..=degree + 1).rev() {
                let scaled = arithmetic.mul(node, product.0[power]);
                product.0[power] = arithmetic.sub(product.0[power - 1], scaled);
            }
            let constant = arithmetic.mul(node, product.0[0]);
            product.0[0] = arithmetic.sub(0, constant);
        }
        #[cfg(test)]
        tests::after_product()?;
        coefficients.0[prime] = 1;
        let phi = modulus - modulus / context.prime;
        for index in 0..prime {
            let node = public_node(index, half, modulus);
            quotient.0[prime - 1] = product.0[prime];
            for power in (1..prime).rev() {
                let scaled = arithmetic.mul(node, quotient.0[power]);
                quotient.0[power - 1] = arithmetic.add(product.0[power], scaled);
            }
            let remainder_product = arithmetic.mul(node, quotient.0[0]);
            if arithmetic.add(product.0[0], remainder_product) != 0 {
                return Err(PrimePowerError::Invariant);
            }
            let denominator = arithmetic.at_public_node(&quotient.0, node);
            if denominator.is_multiple_of(context.prime) {
                return Err(PrimePowerError::Invariant);
            }
            let inverse = arithmetic.power(denominator, phi - 1);
            if arithmetic.mul(denominator, inverse) != 1 {
                return Err(PrimePowerError::Invariant);
            }
            let node_power = arithmetic.power(node, context.prime);
            let correction = arithmetic.sub(node, node_power);
            let weight = arithmetic.mul(correction, inverse);
            for power in 0..prime {
                let term = arithmetic.mul(weight, quotient.0[power]);
                coefficients.0[power] = arithmetic.add(coefficients.0[power], term);
            }
        }
        if coefficients.0[..prime]
            .iter()
            .any(|coefficient| !coefficient.is_multiple_of(context.prime))
        {
            return Err(PrimePowerError::Invariant);
        }
        for index in 0..prime {
            let node = public_node(index, half, modulus);
            if arithmetic.at_public_node(&coefficients.0, node) != node {
                return Err(PrimePowerError::Invariant);
            }
        }
        if arithmetic.operations != plan.geometry.arithmetic_operations {
            return Err(PrimePowerError::Invariant);
        }
        Ok(Self {
            context,
            coefficients,
        })
    }

    /// The exact arithmetic context of these canonical coefficients.
    #[must_use]
    pub const fn context(&self) -> PrimePowerContext {
        self.context
    }

    /// Public canonical coefficients in increasing degree order.
    #[must_use]
    pub fn coefficients(&self) -> &[u64] {
        &self.coefficients.0
    }
}

/// One instruction of the complete public upper-digit tableau.
///
/// Register zero through `e-1` initially alias the canonical input at precision
/// `e`; the two additional registers are overwritten before every read. This
/// enum describes arithmetic, not an executable or authenticated ciphertext.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpperDigitInstruction {
    /// Evaluate one public lifting polynomial without changing precision.
    Evaluate {
        /// Index into the prepared polynomial pool.
        polynomial: usize,
        /// Source register, possibly equal to the destination.
        input: usize,
        /// Destination register.
        output: usize,
        /// Exact input/output precision.
        precision: u32,
    },
    /// Canonical subtraction modulo `p^precision`.
    Subtract {
        /// Minuend register.
        lhs: usize,
        /// Subtrahend register.
        rhs: usize,
        /// Destination register.
        output: usize,
        /// Exact precision of both inputs and the result.
        precision: u32,
    },
    /// Exact division of a divisible residue, reducing plaintext precision by one.
    ///
    /// This is not inversion of `p`, which is not a unit. A ciphertext backend
    /// still needs to establish divisibility and its own key/noise relation.
    DivideByPrime {
        /// Canonical, divisible source register.
        input: usize,
        /// Destination register at precision `precision - 1`.
        output: usize,
        /// Source precision, at least two.
        precision: u32,
    },
}

/// Explicit public ceilings for the complete prepared upper-digit program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpperDigitsBudget {
    /// All retained polynomial coefficient element bytes.
    pub coefficient_bytes: usize,
    /// Extra product/quotient element bytes while preparing one polynomial.
    pub preparation_scratch_bytes: usize,
    /// Instruction and polynomial-owner vector element bytes.
    pub program_bytes: usize,
    /// Sum of canonical polynomial-preparation add/subtract/multiply calls.
    pub preparation_operations: u64,
    /// Maximum live abstract scalar registers, including the two temporaries.
    /// This is a value count, not a ciphertext/backend memory allowance.
    pub live_values: usize,
}

/// Exact checked public geometry of the complete upper-digit program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpperDigitsGeometry {
    /// Retained coefficient elements for all `e-r` polynomial precisions.
    pub coefficient_bytes: usize,
    /// Additional temporary polynomial-preparation element bytes.
    pub preparation_scratch_bytes: usize,
    /// Instruction and polynomial-owner vector element bytes.
    pub program_bytes: usize,
    /// Exact canonical operations in preparation, excluding named public setup.
    pub preparation_operations: u64,
    /// Number of polynomial evaluation instructions in the full tableau.
    pub polynomial_evaluations: usize,
    /// Number of instructions, exactly three per polynomial evaluation.
    pub instructions: usize,
    /// Live scalar registers (`e + 2`), separate from ciphertext memory.
    pub live_values: usize,
}

/// Admitted complete upper-digit program for a fixed public retained precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpperDigitsPlan {
    context: PrimePowerContext,
    retained_digits: u32,
    geometry: UpperDigitsGeometry,
}

impl UpperDigitsPlan {
    /// Additional owning headers while preparing one public lifting polynomial.
    ///
    /// The result includes the temporary complete program owner and its two
    /// product/quotient coefficient-buffer handles. Polynomial vector element
    /// headers and retained coefficients are already charged by `geometry`.
    /// Non-owning scalar/plan stack values, allocator headers and RSS are outside
    /// this typed owner scope; coefficient scratch remains a separate geometry field.
    #[must_use]
    pub const fn preparation_owner_bytes(&self) -> usize {
        size_of::<UpperDigitsProgram>() + 2 * size_of::<Coefficients>()
    }

    /// Compute complete checked coefficient/node/work/live-value geometry.
    ///
    /// Vector payloads include every polynomial-owner handle and instruction.
    /// Stack owner/plan values, allocator headers and future ciphertext/backend
    /// scratch are excluded. No degree-sized allocation happens here.
    ///
    /// # Errors
    /// Refuses invalid retained precision or any count/byte/work overflow.
    pub fn required(
        context: PrimePowerContext,
        retained_digits: u32,
    ) -> Result<UpperDigitsGeometry, PrimePowerError> {
        if retained_digits == 0 || retained_digits >= context.precision {
            return Err(PrimePowerError::RetainedDigits);
        }
        let count = usize::try_from(context.precision - retained_digits)
            .map_err(|_| PrimePowerError::GeometryOverflow)?;
        let precision =
            usize::try_from(context.precision).map_err(|_| PrimePowerError::GeometryOverflow)?;
        let polynomial_evaluations = precision
            .checked_mul(count)
            .and_then(|value| {
                count
                    .checked_mul(count + 1)
                    .and_then(|triangle| value.checked_sub(triangle / 2))
            })
            .ok_or(PrimePowerError::GeometryOverflow)?;
        let instructions = polynomial_evaluations
            .checked_mul(3)
            .ok_or(PrimePowerError::GeometryOverflow)?;
        let mut coefficient_bytes = 0_usize;
        let mut preparation_operations = 0_u64;
        let mut preparation_scratch_bytes = 0_usize;
        for offset in 0..count {
            let polynomial = LiftingPolynomialPlan::required(
                context.at_precision(context.precision - offset as u32)?,
            )?;
            coefficient_bytes = coefficient_bytes
                .checked_add(polynomial.coefficient_bytes)
                .ok_or(PrimePowerError::GeometryOverflow)?;
            preparation_operations = preparation_operations
                .checked_add(polynomial.arithmetic_operations)
                .ok_or(PrimePowerError::GeometryOverflow)?;
            preparation_scratch_bytes = preparation_scratch_bytes.max(polynomial.scratch_bytes);
        }
        let program_bytes = instructions
            .checked_mul(size_of::<UpperDigitInstruction>())
            .and_then(|bytes| {
                count
                    .checked_mul(size_of::<LiftingPolynomial>())
                    .and_then(|pool| bytes.checked_add(pool))
            })
            .ok_or(PrimePowerError::GeometryOverflow)?;
        Ok(UpperDigitsGeometry {
            coefficient_bytes,
            preparation_scratch_bytes,
            program_bytes,
            preparation_operations,
            polynomial_evaluations,
            instructions,
            live_values: precision
                .checked_add(2)
                .ok_or(PrimePowerError::GeometryOverflow)?,
        })
    }

    /// Admit the entire program before any coefficient/node allocation.
    ///
    /// # Errors
    /// Refuses invalid geometry or any insufficient public budget field.
    pub fn new(
        context: PrimePowerContext,
        retained_digits: u32,
        budget: UpperDigitsBudget,
    ) -> Result<Self, PrimePowerError> {
        let geometry = Self::required(context, retained_digits)?;
        if geometry.coefficient_bytes > budget.coefficient_bytes
            || geometry.preparation_scratch_bytes > budget.preparation_scratch_bytes
            || geometry.program_bytes > budget.program_bytes
            || geometry.preparation_operations > budget.preparation_operations
            || geometry.live_values > budget.live_values
        {
            return Err(PrimePowerError::Budget);
        }
        Ok(Self {
            context,
            retained_digits,
            geometry,
        })
    }

    /// Exact arithmetic input context.
    #[must_use]
    pub const fn context(self) -> PrimePowerContext {
        self.context
    }

    /// Number of upper base-prime digits retained by the program.
    #[must_use]
    pub const fn retained_digits(self) -> u32 {
        self.retained_digits
    }

    /// Exact admitted public geometry.
    #[must_use]
    pub const fn geometry(self) -> UpperDigitsGeometry {
        self.geometry
    }
}

/// Immutable public lifting/subtraction/exact-division program and its coefficients.
///
/// There is no production scalar interpreter or implicit ciphertext execution.
/// [`crate::bgv::EncryptedUpperDigitsPlan`] explicitly lowers this immutable
/// tableau to canonical BGV operations with complete public shape/resource
/// preflight. Its private matched divisions consume genuine coefficient-slot
/// provenance. TODO(C.8, R.9): complete the encrypted-secret bootstrap relation,
/// proof integration, circuit privacy and parameter/resource qualification.
pub struct UpperDigitsProgram {
    plan: UpperDigitsPlan,
    polynomials: Vec<LiftingPolynomial>,
    instructions: Vec<UpperDigitInstruction>,
}

impl core::fmt::Debug for UpperDigitsProgram {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("UpperDigitsProgram")
            .field("plan", &self.plan)
            .finish_non_exhaustive()
    }
}

impl UpperDigitsProgram {
    /// Prepare every required polynomial and the complete public tableau.
    ///
    /// # Errors
    /// Returns allocation refusal or a public polynomial invariant failure.
    pub fn prepare(plan: &UpperDigitsPlan) -> Result<Self, PrimePowerError> {
        let count = (plan.context.precision - plan.retained_digits) as usize;
        let precision = plan.context.precision as usize;
        let mut polynomials = Vec::new();
        let mut instructions = Vec::new();
        allocation_checkpoint()?;
        polynomials
            .try_reserve_exact(count)
            .map_err(|_| PrimePowerError::Allocation)?;
        allocation_checkpoint()?;
        instructions
            .try_reserve_exact(plan.geometry.instructions)
            .map_err(|_| PrimePowerError::Allocation)?;
        for offset in 0..count {
            let context = plan
                .context
                .at_precision(plan.context.precision - offset as u32)?;
            let geometry = LiftingPolynomialPlan::required(context)?;
            let polynomial_plan = LiftingPolynomialPlan::new(
                context,
                LiftingPolynomialBudget {
                    coefficient_bytes: geometry.coefficient_bytes,
                    scratch_bytes: geometry.scratch_bytes,
                    arithmetic_operations: geometry.arithmetic_operations,
                },
            )?;
            polynomials.push(LiftingPolynomial::prepare(&polynomial_plan)?);
            let current_precision = context.precision;
            let mut input = offset;
            for destination in offset + 1..precision {
                instructions.push(UpperDigitInstruction::Evaluate {
                    polynomial: offset,
                    input,
                    output: precision,
                    precision: current_precision,
                });
                instructions.push(UpperDigitInstruction::Subtract {
                    lhs: destination,
                    rhs: precision,
                    output: precision + 1,
                    precision: current_precision,
                });
                instructions.push(UpperDigitInstruction::DivideByPrime {
                    input: precision + 1,
                    output: destination,
                    precision: current_precision,
                });
                input = precision;
            }
        }
        if instructions.len() != plan.geometry.instructions {
            return Err(PrimePowerError::Invariant);
        }
        Ok(Self {
            plan: *plan,
            polynomials,
            instructions,
        })
    }

    /// The complete admitted public plan.
    #[must_use]
    pub const fn plan(&self) -> UpperDigitsPlan {
        self.plan
    }

    /// Public polynomials in descending precision order `e, e-1, ..., r+1`.
    #[must_use]
    pub fn polynomials(&self) -> &[LiftingPolynomial] {
        &self.polynomials
    }

    /// Complete public instruction order, with no omitted zero/dead steps.
    #[must_use]
    pub fn instructions(&self) -> &[UpperDigitInstruction] {
        &self.instructions
    }

    /// The register that contains the result at exactly the retained precision.
    #[must_use]
    pub fn output_register(&self) -> usize {
        self.plan.context.precision as usize - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };

    thread_local! {
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
        static FAIL_ALLOCATION: Cell<usize> = const { Cell::new(usize::MAX) };
        static PRODUCT_ACTION: Cell<u8> = const { Cell::new(0) };
        static CLEARED_WORDS: Cell<usize> = const { Cell::new(0) };
        static NONZERO_RELEASE: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn allocation_checkpoint() -> Result<(), PrimePowerError> {
        let index = ALLOCATIONS.get();
        ALLOCATIONS.set(index + 1);
        if index == FAIL_ALLOCATION.get() {
            Err(PrimePowerError::Allocation)
        } else {
            Ok(())
        }
    }

    pub(super) fn after_product() -> Result<(), PrimePowerError> {
        match PRODUCT_ACTION.replace(0) {
            1 => Err(PrimePowerError::Invariant),
            2 => panic!("injected after populated public polynomial product"),
            _ => Ok(()),
        }
    }

    pub(super) fn observe_cleared(words: &[u64]) {
        CLEARED_WORDS.set(CLEARED_WORDS.get() + words.len());
        NONZERO_RELEASE.set(NONZERO_RELEASE.get() || words.iter().any(|&word| word != 0));
    }

    fn reset_observation() {
        ALLOCATIONS.set(0);
        FAIL_ALLOCATION.set(usize::MAX);
        PRODUCT_ACTION.set(0);
        CLEARED_WORDS.set(0);
        NONZERO_RELEASE.set(false);
    }

    fn polynomial_plan(context: PrimePowerContext) -> LiftingPolynomialPlan {
        let geometry = LiftingPolynomialPlan::required(context).unwrap();
        LiftingPolynomialPlan::new(
            context,
            LiftingPolynomialBudget {
                coefficient_bytes: geometry.coefficient_bytes,
                scratch_bytes: geometry.scratch_bytes,
                arithmetic_operations: geometry.arithmetic_operations,
            },
        )
        .unwrap()
    }

    fn program_plan(context: PrimePowerContext, retained: u32) -> UpperDigitsPlan {
        let geometry = UpperDigitsPlan::required(context, retained).unwrap();
        UpperDigitsPlan::new(
            context,
            retained,
            UpperDigitsBudget {
                coefficient_bytes: geometry.coefficient_bytes,
                preparation_scratch_bytes: geometry.preparation_scratch_bytes,
                program_bytes: geometry.program_bytes,
                preparation_operations: geometry.preparation_operations,
                live_values: geometry.live_values,
            },
        )
        .unwrap()
    }

    // Independent ordinary-integer test arithmetic. No production FixedModulus
    // kernel is used to generate the Newton-basis reference coefficients.
    fn oracle_mul(lhs: u64, rhs: u64, modulus: u64) -> u64 {
        (u128::from(lhs) * u128::from(rhs) % u128::from(modulus)) as u64
    }

    fn oracle_pow(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
        let mut result = 1;
        while exponent != 0 {
            if exponent & 1 == 1 {
                result = oracle_mul(result, base, modulus);
            }
            base = oracle_mul(base, base, modulus);
            exponent >>= 1;
        }
        result
    }

    fn oracle_inverse(value: u64, modulus: u64) -> u64 {
        let (mut old_r, mut r) = (i128::from(modulus), i128::from(value));
        let (mut old_t, mut t) = (0_i128, 1_i128);
        while r != 0 {
            let quotient = old_r / r;
            (old_r, r) = (r, old_r - quotient * r);
            (old_t, t) = (t, old_t - quotient * t);
        }
        assert_eq!(old_r, 1);
        old_t.rem_euclid(i128::from(modulus)) as u64
    }

    fn oracle_evaluate(coefficients: &[u64], input: u64, modulus: u64) -> u64 {
        coefficients
            .iter()
            .rev()
            .fold(0, |accumulator, &coefficient| {
                ((u128::from(accumulator) * u128::from(input) + u128::from(coefficient))
                    % u128::from(modulus)) as u64
            })
    }

    fn newton_coefficients(context: PrimePowerContext) -> Vec<u64> {
        let prime = context.prime();
        let modulus = context.modulus();
        let origin = -(i128::from(prime) / 2);
        let mut differences: Vec<_> = (0..prime)
            .map(|index| {
                let node = (origin + i128::from(index)).rem_euclid(i128::from(modulus)) as u64;
                (node + modulus - oracle_pow(node, prime, modulus)) % modulus
            })
            .collect();
        let mut result = vec![0; prime as usize + 1];
        result[prime as usize] = 1;
        let mut basis = vec![1];
        let mut factorial = 1;
        for order in 0..prime as usize {
            let multiplier =
                oracle_mul(differences[0], oracle_inverse(factorial, modulus), modulus);
            for (power, &coefficient) in basis.iter().enumerate() {
                result[power] =
                    (result[power] + oracle_mul(multiplier, coefficient, modulus)) % modulus;
            }
            differences = differences
                .windows(2)
                .map(|pair| (pair[1] + modulus - pair[0]) % modulus)
                .collect();
            let node = (origin + order as i128).rem_euclid(i128::from(modulus)) as u64;
            let mut following = vec![0; basis.len() + 1];
            for (power, &coefficient) in basis.iter().enumerate() {
                following[power] =
                    (following[power] + modulus - oracle_mul(node, coefficient, modulus)) % modulus;
                following[power + 1] = (following[power + 1] + coefficient) % modulus;
            }
            basis = following;
            factorial = oracle_mul(factorial, order as u64 + 1, modulus);
        }
        result
    }

    fn execute_scalar(program: &UpperDigitsProgram, input: u64) -> u64 {
        let plan = program.plan();
        assert!(input < plan.context().modulus());
        let mut values = Zeroizing::new(vec![0_u64; plan.geometry().live_values]);
        let mut precisions = vec![0; plan.geometry().live_values];
        for register in 0..plan.context().precision() as usize {
            values[register] = input;
            precisions[register] = plan.context().precision();
        }
        for &instruction in program.instructions() {
            match instruction {
                UpperDigitInstruction::Evaluate {
                    polynomial,
                    input,
                    output,
                    precision,
                } => {
                    assert_eq!(precisions[input], precision);
                    let polynomial = &program.polynomials()[polynomial];
                    assert_eq!(polynomial.context().precision(), precision);
                    values[output] = oracle_evaluate(
                        polynomial.coefficients(),
                        values[input],
                        polynomial.context().modulus(),
                    );
                    precisions[output] = precision;
                }
                UpperDigitInstruction::Subtract {
                    lhs,
                    rhs,
                    output,
                    precision,
                } => {
                    assert_eq!(precisions[lhs], precision);
                    assert_eq!(precisions[rhs], precision);
                    let modulus = plan.context().prime().pow(precision);
                    values[output] = (values[lhs] + modulus - values[rhs]) % modulus;
                    precisions[output] = precision;
                }
                UpperDigitInstruction::DivideByPrime {
                    input,
                    output,
                    precision,
                } => {
                    assert_eq!(precisions[input], precision);
                    assert!(values[input].is_multiple_of(plan.context().prime()));
                    values[output] = values[input] / plan.context().prime();
                    precisions[output] = precision - 1;
                    assert!(values[output] < plan.context().prime().pow(precision - 1));
                }
            }
        }
        assert_eq!(
            precisions[program.output_register()],
            plan.retained_digits()
        );
        values[program.output_register()]
    }

    fn integer_upper(mut value: i128, prime: u64, precision: u32, retained: u32) -> u64 {
        let p = i128::from(prime);
        for _ in retained..precision {
            let mut low = value.rem_euclid(p);
            if low > p / 2 {
                low -= p;
            }
            value = (value - low) / p;
        }
        value.rem_euclid(p.pow(retained)) as u64
    }

    fn boundary_values(context: PrimePowerContext) -> Vec<u64> {
        let mut values = vec![
            0,
            1,
            context.prime() / 2,
            context.prime() / 2 + 1,
            context.prime() - 1,
        ];
        let modulus = i128::from(context.modulus());
        for precision in 1..=context.precision() {
            let power = i128::from(context.prime()).pow(precision);
            for sign in [-1, 1] {
                for offset in -1..=1 {
                    values.push((sign * power + offset).rem_euclid(modulus) as u64);
                }
            }
        }
        let mut sample = 0x7a12_93e4_7651_03d9_u64;
        for _ in 0..32 {
            sample = sample
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            values.push(sample % context.modulus());
        }
        values.sort_unstable();
        values.dedup();
        values
    }

    #[test]
    fn contexts_refuse_invalid_and_over_word_precision_without_truncation() {
        for (prime, precision) in [(0, 1), (2, 1), (9, 3), (257, 0)] {
            assert_eq!(
                PrimePowerContext::new(prime, precision),
                Err(PrimePowerError::InvalidContext)
            );
        }
        for (prime, precision) in [(257, 8), (3, 40), (257, u32::MAX), (u64::MAX, 1)] {
            assert_eq!(
                PrimePowerContext::new(prime, precision),
                Err(PrimePowerError::UnsupportedPrecision)
            );
        }
        assert_eq!(
            PrimePowerContext::new(257, 7).unwrap().modulus(),
            74_051_159_531_521_793
        );
        let context = PrimePowerContext::new(3, 39).unwrap();
        assert_eq!(context.at_precision(1).unwrap().modulus(), 3);
        assert_eq!(context.at_precision(0), Err(PrimePowerError::Invariant));
        assert_eq!(context.at_precision(40), Err(PrimePowerError::Invariant));
        assert_eq!(
            UpperDigitsPlan::required(context, 0),
            Err(PrimePowerError::RetainedDigits)
        );
        assert_eq!(
            UpperDigitsPlan::required(context, 39),
            Err(PrimePowerError::RetainedDigits)
        );
        let huge_degree = PrimePowerContext::new(2_305_843_009_213_693_951, 1).unwrap();
        assert_eq!(
            LiftingPolynomialPlan::required(huge_degree),
            Err(PrimePowerError::GeometryOverflow)
        );
    }

    #[test]
    fn lifting_matches_independent_newton_and_root_vectors_through_precision_seven() {
        // Selected coefficients from the independently run root Newton oracle;
        // every coefficient also compares against the Newton implementation.
        let root_vectors = [
            [0, 0, 0, 0, 0],
            [29_041, 11_822, 31_611, 30_840, 19_275],
            [11_785_763, 7_079_065, 14_166_097, 7_032_034, 16_267_329],
            [
                3_593_424_886,
                3_113_429_584,
                608_276_852,
                2_400_449_647,
                4_361_763_137,
            ],
            [
                278_429_060_149,
                827_620_335_373,
                153_294_740_887,
                744_020_417_817,
                1_121_154_185_793,
            ],
            [
                64_184_257_964_398,
                38_946_886_699_311,
                233_353_512_496_743,
                223_853_844_136_160,
                288_136_806_808_385,
            ],
            [
                24_555_812_896_794_563,
                48_445_930_549_328_343,
                53_826_799_710_407_457,
                10_884_915_722_215_173,
                74_051_159_530_814_529,
            ],
        ];
        for precision in 1..=7 {
            let context = PrimePowerContext::new(257, precision).unwrap();
            let polynomial = LiftingPolynomial::prepare(&polynomial_plan(context)).unwrap();
            assert_eq!(polynomial.coefficients(), newton_coefficients(context));
            let actual = [1, 3, 65, 127, 255].map(|index| polynomial.coefficients()[index]);
            assert_eq!(actual, root_vectors[precision as usize - 1]);
            for starting_precision in 1..precision {
                let modulus = 257_u64.pow(starting_precision + 1);
                for digit in -128_i128..=128 {
                    let input = (digit + i128::from(257_u64.pow(starting_precision)) * 113)
                        .rem_euclid(i128::from(context.modulus()))
                        as u64;
                    assert_eq!(
                        oracle_evaluate(polynomial.coefficients(), input, context.modulus())
                            % modulus,
                        digit.rem_euclid(i128::from(modulus)) as u64
                    );
                }
            }
            assert!(format!("{polynomial:?}").contains("LiftingPolynomial"));
        }
        for (prime, precision) in [(3, 4), (3, 39), (5, 4), (17, 3)] {
            let context = PrimePowerContext::new(prime, precision).unwrap();
            let polynomial = LiftingPolynomial::prepare(&polynomial_plan(context)).unwrap();
            assert_eq!(polynomial.coefficients(), newton_coefficients(context));
        }
    }

    #[test]
    fn complete_upper_program_matches_integer_oracle_and_precision_flow() {
        for (prime, precision) in [(3, 4), (5, 4), (17, 3)]
            .into_iter()
            .chain((2..=7).map(|precision| (257, precision)))
        {
            let context = PrimePowerContext::new(prime, precision).unwrap();
            let values = if context.modulus() <= 4_913 {
                (0..context.modulus()).collect()
            } else {
                boundary_values(context)
            };
            for retained in 1..precision {
                let plan = program_plan(context, retained);
                let program = UpperDigitsProgram::prepare(&plan).unwrap();
                let removed = (precision - retained) as usize;
                assert_eq!(program.polynomials().len(), removed);
                assert_eq!(
                    plan.geometry().polynomial_evaluations,
                    precision as usize * removed - removed * (removed + 1) / 2
                );
                assert_eq!(
                    program.instructions().len(),
                    3 * plan.geometry().polynomial_evaluations
                );
                assert_eq!(program.plan(), plan);
                for &input in &values {
                    assert_eq!(
                        execute_scalar(&program, input),
                        integer_upper(i128::from(input), prime, precision, retained),
                        "p={prime}, e={precision}, r={retained}, x={input}"
                    );
                }
                assert!(format!("{program:?}").contains("UpperDigitsProgram"));
            }
        }
        // The universal word limit also admits 3^39. Exercise both extreme
        // retained precisions at every public power boundary.
        let widest = PrimePowerContext::new(3, 39).unwrap();
        for retained in [1, 38] {
            let program = UpperDigitsProgram::prepare(&program_plan(widest, retained)).unwrap();
            for input in boundary_values(widest) {
                assert_eq!(
                    execute_scalar(&program, input),
                    integer_upper(i128::from(input), 3, 39, retained)
                );
            }
        }
        let program =
            UpperDigitsProgram::prepare(&program_plan(PrimePowerContext::new(257, 2).unwrap(), 1))
                .unwrap();
        assert_eq!(execute_scalar(&program, 0), 0);
        assert_eq!(execute_scalar(&program, 257), 1);
    }

    #[test]
    #[ignore = "exhaustive 66,049-residue degree-257 scalar diagnostic"]
    fn exhaustive_f257_squared_upper_digit_extraction() {
        let context = PrimePowerContext::new(257, 2).unwrap();
        let program = UpperDigitsProgram::prepare(&program_plan(context, 1)).unwrap();
        for input in 0..context.modulus() {
            assert_eq!(
                execute_scalar(&program, input),
                integer_upper(i128::from(input), 257, 2, 1)
            );
        }
    }

    #[test]
    fn exact_and_one_under_budgets_refuse_before_any_allocation() {
        let context = PrimePowerContext::new(257, 7).unwrap();
        let geometry = LiftingPolynomialPlan::required(context).unwrap();
        assert_eq!(geometry.coefficient_bytes, 8 * 258);
        assert_eq!(geometry.scratch_bytes, 8 * 515);
        let budget = LiftingPolynomialBudget {
            coefficient_bytes: geometry.coefficient_bytes,
            scratch_bytes: geometry.scratch_bytes,
            arithmetic_operations: geometry.arithmetic_operations,
        };
        for field in 0..3 {
            let mut insufficient = budget;
            match field {
                0 => insufficient.coefficient_bytes -= 1,
                1 => insufficient.scratch_bytes -= 1,
                _ => insufficient.arithmetic_operations -= 1,
            }
            reset_observation();
            assert_eq!(
                LiftingPolynomialPlan::new(context, insufficient),
                Err(PrimePowerError::Budget)
            );
            assert_eq!(ALLOCATIONS.get(), 0);
        }
        let plan = LiftingPolynomialPlan::new(context, budget).unwrap();
        assert_eq!(plan.context(), context);
        assert_eq!(plan.geometry(), geometry);
        drop(LiftingPolynomial::prepare(&plan).unwrap());

        let geometry = UpperDigitsPlan::required(context, 1).unwrap();
        let budget = UpperDigitsBudget {
            coefficient_bytes: geometry.coefficient_bytes,
            preparation_scratch_bytes: geometry.preparation_scratch_bytes,
            program_bytes: geometry.program_bytes,
            preparation_operations: geometry.preparation_operations,
            live_values: geometry.live_values,
        };
        for field in 0..5 {
            let mut insufficient = budget;
            match field {
                0 => insufficient.coefficient_bytes -= 1,
                1 => insufficient.preparation_scratch_bytes -= 1,
                2 => insufficient.program_bytes -= 1,
                3 => insufficient.preparation_operations -= 1,
                _ => insufficient.live_values -= 1,
            }
            reset_observation();
            assert_eq!(
                UpperDigitsPlan::new(context, 1, insufficient),
                Err(PrimePowerError::Budget)
            );
            assert_eq!(ALLOCATIONS.get(), 0);
        }
        let plan = UpperDigitsPlan::new(context, 1, budget).unwrap();
        assert_eq!(geometry.coefficient_bytes, 6 * 8 * 258);
        assert_eq!(geometry.instructions, 63);
        assert_eq!(geometry.live_values, 9);
        drop(UpperDigitsProgram::prepare(&plan).unwrap());
    }

    #[test]
    fn clearing_owners_cover_every_allocation_refusal_error_success_and_unwind() {
        let plan = polynomial_plan(PrimePowerContext::new(17, 3).unwrap());
        for fail in 0..3 {
            reset_observation();
            FAIL_ALLOCATION.set(fail);
            assert!(matches!(
                LiftingPolynomial::prepare(&plan),
                Err(PrimePowerError::Allocation)
            ));
            assert_eq!(CLEARED_WORDS.get(), [0, 18, 36][fail]);
            assert!(!NONZERO_RELEASE.get());
        }
        for action in [0, 1, 2] {
            reset_observation();
            PRODUCT_ACTION.set(action);
            let result = catch_unwind(AssertUnwindSafe(|| LiftingPolynomial::prepare(&plan)));
            match action {
                0 => {
                    let polynomial = result.unwrap().unwrap();
                    assert_eq!(CLEARED_WORDS.get(), 35);
                    drop(polynomial);
                }
                1 => assert!(matches!(result.unwrap(), Err(PrimePowerError::Invariant))),
                _ => assert!(result.is_err()),
            }
            assert_eq!(CLEARED_WORDS.get(), 53);
            assert!(!NONZERO_RELEASE.get());
        }
        let plan = program_plan(PrimePowerContext::new(17, 3).unwrap(), 1);
        for fail in 0..8 {
            reset_observation();
            FAIL_ALLOCATION.set(fail);
            assert!(matches!(
                UpperDigitsProgram::prepare(&plan),
                Err(PrimePowerError::Allocation)
            ));
            assert!(!NONZERO_RELEASE.get());
            assert_eq!(CLEARED_WORDS.get(), [0, 0, 0, 18, 36, 53, 71, 89][fail]);
        }
        reset_observation();
        drop(UpperDigitsProgram::prepare(&plan).unwrap());
        assert_eq!(CLEARED_WORDS.get(), 106);
        assert!(!NONZERO_RELEASE.get());
    }
}
