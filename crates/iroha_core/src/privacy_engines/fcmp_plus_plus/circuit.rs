//! FCMP++ arithmetic-circuit and embedded-curve gadgets.
//!
//! This is a concrete, allocation-bounded port of the circuit abstraction and
//! EC discrete-log gadgets used by the pinned FCMP++ implementation.  The
//! verifier builds the exact same constraints as the prover: hidden points are proved on their
//! embedded curve, divisor evaluations bind every discrete logarithm, public rerandomizations are
//! checked with incomplete addition, and every tree layer is checked with set membership.
#[cfg(test)]
use super::proof_math::FcmpProofRandomSource;
use super::{
    FcmpNativeErrorV1,
    bulletproof::{
        ArithmeticCircuitStatement, ArithmeticCircuitWitness, LinComb, Variable,
        VectorCommitmentOpening,
    },
    divisor::NormalizedDivisor,
    proof_math::{
        ProofGeneratorView, ProofScalar, ProofSuite, ProverTranscript, SecretMultiexpBuilder,
        SecretPoint, VerifierTranscript,
    },
};
use zeroize::Zeroize;
const COMMITMENT_WORD_LEN: usize = 128;
const MAX_EMBEDDED_POINT_ATTEMPTS_V1: usize = 128;
const MAX_DLOG_CHALLENGE_ATTEMPTS_V1: usize = 128;
pub(super) trait CircuitTranscript {
    fn circuit_challenge<S: ProofSuite>(&mut self) -> Result<S::Scalar, FcmpNativeErrorV1>;
    fn circuit_challenge_bytes(&mut self) -> [u8; 64];
}
impl CircuitTranscript for ProverTranscript {
    fn circuit_challenge<S: ProofSuite>(&mut self) -> Result<S::Scalar, FcmpNativeErrorV1> {
        self.challenge::<S>()
    }
    fn circuit_challenge_bytes(&mut self) -> [u8; 64] {
        self.challenge_bytes()
    }
}
impl CircuitTranscript for VerifierTranscript<'_> {
    fn circuit_challenge<S: ProofSuite>(&mut self) -> Result<S::Scalar, FcmpNativeErrorV1> {
        self.challenge::<S>()
    }
    fn circuit_challenge_bytes(&mut self) -> [u8; 64] {
        self.challenge_bytes()
    }
}
/// Static coefficient dimensions for one embedded-curve discrete-log gadget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DlogParameters {
    pub(super) scalar_bits: usize,
    pub(super) x_coefficients: usize,
    pub(super) x_coefficients_minus_one: usize,
    pub(super) yx_coefficients: usize,
}
impl DlogParameters {
    const fn new(scalar_bits: usize) -> Self {
        let x_coefficients = (scalar_bits + 1) / 2;
        Self {
            scalar_bits,
            x_coefficients,
            x_coefficients_minus_one: x_coefficients - 1,
            yx_coefficients: x_coefficients - 2,
        }
    }
    fn validate(self) -> Result<(), FcmpNativeErrorV1> {
        if !(3..=255).contains(&self.scalar_bits)
            || self.x_coefficients != (self.scalar_bits + 1) / 2
            || self.x_coefficients_minus_one + 1 != self.x_coefficients
            || self.yx_coefficients + 2 != self.x_coefficients
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        Ok(())
    }
}
/// Ed25519's scalar modulus occupies 253 bits.
pub(super) const ED25519_DLOG_PARAMETERS: DlogParameters = DlogParameters::new(253);
/// Selene and Helios scalar moduli each occupy 255 bits.
pub(super) const CYCLE_DLOG_PARAMETERS: DlogParameters = DlogParameters::new(255);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CurveSpec<F: ProofScalar> {
    pub(super) a: F,
    pub(super) b: F,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OnCurve {
    x: Variable,
    y: Variable,
}
impl OnCurve {
    pub(super) const fn x(self) -> Variable {
        self.x
    }
    pub(super) const fn y(self) -> Variable {
        self.y
    }
}
/// The variable layout of a normalized divisor. The coefficient of `x` is
/// fixed to one and therefore deliberately has no variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Divisor {
    pub(super) y: Variable,
    pub(super) yx: Vec<Variable>,
    pub(super) x_from_power_of_2: Vec<Variable>,
    pub(super) zero: Variable,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PointWithDlog {
    pub(super) point: (Variable, Variable),
    pub(super) dlog: Vec<Variable>,
    pub(super) divisor: Divisor,
}
/// Deterministic variable allocator matching the upstream 128-element word
/// packing. Branches always receive a dedicated vector commitment.
#[derive(Clone, Debug)]
pub(super) struct VectorCommitmentTape {
    commitment_len: usize,
    current_j_offset: usize,
    commitments: usize,
    branch_lengths: Vec<usize>,
}
impl VectorCommitmentTape {
    pub(super) fn new(commitment_len: usize) -> Result<Self, FcmpNativeErrorV1> {
        if commitment_len == 0
            || !commitment_len.is_power_of_two()
            || commitment_len % COMMITMENT_WORD_LEN != 0
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        Ok(Self {
            commitment_len,
            current_j_offset: 0,
            commitments: 0,
            branch_lengths: Vec::new(),
        })
    }
    pub(super) const fn commitment_count(&self) -> usize {
        self.commitments
    }
    fn append_word(&mut self) -> Result<Vec<Variable>, FcmpNativeErrorV1> {
        if self.current_j_offset == 0 {
            self.commitments = self
                .commitments
                .checked_add(1)
                .ok_or(FcmpNativeErrorV1::TreeFull)?;
        }
        let commitment = self
            .commitments
            .checked_sub(1)
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let end = self
            .current_j_offset
            .checked_add(COMMITMENT_WORD_LEN)
            .ok_or(FcmpNativeErrorV1::TreeFull)?;
        if end > self.commitment_len {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let variables = (self.current_j_offset..end)
            .map(|index| Variable::CG { commitment, index })
            .collect();
        self.current_j_offset = if end == self.commitment_len { 0 } else { end };
        Ok(variables)
    }
    pub(super) fn append_branch(
        &mut self,
        branch_len: usize,
    ) -> Result<Vec<Variable>, FcmpNativeErrorV1> {
        if self.current_j_offset != 0
            || self.branch_lengths.len() != self.commitments
            || branch_len == 0
            || branch_len > self.commitment_len
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let commitment = self.commitments;
        self.commitments = self
            .commitments
            .checked_add(1)
            .ok_or(FcmpNativeErrorV1::TreeFull)?;
        self.branch_lengths.push(branch_len);
        Ok((0..branch_len)
            .map(|index| Variable::CG { commitment, index })
            .collect())
    }
    pub(super) fn append_dlog(
        &mut self,
        parameters: DlogParameters,
    ) -> Result<(Vec<Variable>, Vec<Variable>, Variable), FcmpNativeErrorV1> {
        parameters.validate()?;
        let mut variables = self.append_word()?;
        variables.extend(self.append_word()?);
        if variables.len() != 256 {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let extra = variables
            .pop()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let padding = variables[parameters.scalar_bits..255].to_vec();
        let dlog = variables[..parameters.scalar_bits].to_vec();
        Ok((dlog, padding, extra))
    }
    pub(super) fn append_divisor(
        &mut self,
        parameters: DlogParameters,
    ) -> Result<(Divisor, Variable), FcmpNativeErrorV1> {
        parameters.validate()?;
        let mut variables = self.append_word()?;
        variables.extend(self.append_word()?);
        if variables.len() != 256 {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let extra = variables
            .pop()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let mut cursor = 1;
        let yx_end = cursor + parameters.yx_coefficients;
        let yx = variables[cursor..yx_end].to_vec();
        cursor = yx_end;
        let x_end = cursor + parameters.x_coefficients_minus_one;
        let x_from_power_of_2 = variables[cursor..x_end].to_vec();
        cursor = x_end;
        let zero = *variables
            .get(cursor)
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        Ok((
            Divisor {
                y: variables[0],
                yx,
                x_from_power_of_2,
                zero,
            },
            extra,
        ))
    }
    pub(super) fn append_claimed_point(
        &mut self,
        parameters: DlogParameters,
    ) -> Result<(PointWithDlog, Vec<Variable>), FcmpNativeErrorV1> {
        let (dlog, padding, x) = self.append_dlog(parameters)?;
        let (divisor, y) = self.append_divisor(parameters)?;
        Ok((
            PointWithDlog {
                point: (x, y),
                dlog,
                divisor,
            },
            padding,
        ))
    }
}
/// Prover-side value tape sharing the verifier's exact variable allocator.
pub(super) struct ProverVectorCommitmentTape<F: ProofScalar> {
    layout: VectorCommitmentTape,
    values: Vec<ZeroizingScalarVec<F>>,
    values_logical_capacity: usize,
    values_allocation_capacity: usize,
}
struct ZeroizingScalarVec<F: ProofScalar> {
    values: Vec<F>,
    logical_capacity: usize,
    allocation_capacity: usize,
}
/// Owns one secret-derived discrete-log coefficient until its field-scalar owner has been
/// constructed. This type is intentionally neither `Copy` nor `Clone`.
struct SecretDlogCoefficientV1(u64);
#[cfg(test)]
std::thread_local! {
    static SECRET_DLOG_COEFFICIENT_DROPS_V1: core::cell::Cell<usize> =
        const { core::cell::Cell::new(0) };
}
impl SecretDlogCoefficientV1 {
    fn copy_from_borrowed(value: &u64) -> Self {
        Self(*value)
    }
    fn into_scalar_owner_v1<F: ProofScalar>(self) -> SecretScalarGuard<F> {
        let scalar = SecretScalarGuard::new(F::from_u64(self.0));
        drop(self);
        scalar
    }
}
impl Drop for SecretDlogCoefficientV1 {
    fn drop(&mut self) {
        self.0.zeroize();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.0);
        #[cfg(test)]
        let _ = SECRET_DLOG_COEFFICIENT_DROPS_V1.try_with(|drops| {
            drops.set(drops.get().saturating_add(1));
        });
    }
}
/// Erases one callee-owned `Copy` scalar parameter on every exit path.
struct BorrowedProofScalarSlot<'a, F: ProofScalar>(&'a mut F);
impl<F: ProofScalar> BorrowedProofScalarSlot<'_, F> {
    fn get(&self) -> F {
        *self.0
    }
}
impl<F: ProofScalar> Drop for BorrowedProofScalarSlot<'_, F> {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}
impl<F: ProofScalar> ZeroizingScalarVec<F> {
    fn new(logical_capacity: usize) -> Result<Self, FcmpNativeErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(logical_capacity)
            .map_err(|_| FcmpNativeErrorV1::TreeFull)?;
        let allocation_capacity = values.capacity();
        if allocation_capacity < logical_capacity {
            return Err(FcmpNativeErrorV1::TreeFull);
        }
        Ok(Self {
            values,
            logical_capacity,
            allocation_capacity,
        })
    }
    fn len(&self) -> usize {
        self.values.len()
    }
    fn as_slice(&self) -> &[F] {
        &self.values
    }
    fn push_owned(&mut self, mut value: SecretScalarGuard<F>) -> Result<(), FcmpNativeErrorV1> {
        let allocation_capacity = self.values.capacity();
        let allocation_ptr = self.values.as_ptr();
        if self.values.len() >= self.logical_capacity || self.values.len() >= allocation_capacity {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        debug_assert_eq!(allocation_capacity, self.allocation_capacity);
        debug_assert_eq!(allocation_ptr, self.values.as_ptr());
        self.values.push(F::ZERO);
        let destination = self.values.len() - 1;
        core::mem::swap(&mut self.values[destination], &mut value.0);
        drop(value);
        debug_assert_eq!(self.values.capacity(), allocation_capacity);
        debug_assert_eq!(self.values.as_ptr(), allocation_ptr);
        Ok(())
    }
    fn push_borrowed(&mut self, value: &F) -> Result<(), FcmpNativeErrorV1> {
        let allocation_capacity = self.values.capacity();
        let allocation_ptr = self.values.as_ptr();
        if self.values.len() >= self.logical_capacity || self.values.len() >= allocation_capacity {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        debug_assert_eq!(allocation_capacity, self.allocation_capacity);
        debug_assert_eq!(self.values.as_ptr(), allocation_ptr);
        let value = SecretScalarGuard::copy_from_borrowed(value);
        self.push_owned(value)?;
        debug_assert_eq!(self.values.capacity(), allocation_capacity);
        debug_assert_eq!(self.values.as_ptr(), allocation_ptr);
        Ok(())
    }
    fn extend_borrowed_v1(&mut self, values: &[F]) -> Result<(), FcmpNativeErrorV1> {
        let start_len = self.values.len();
        let allocation_capacity = self.values.capacity();
        let allocation_ptr = self.values.as_ptr();
        let end = start_len
            .checked_add(values.len())
            .ok_or(FcmpNativeErrorV1::TreeFull)?;
        if end > self.logical_capacity || end > allocation_capacity {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        debug_assert_eq!(allocation_capacity, self.allocation_capacity);
        debug_assert_eq!(self.values.as_ptr(), allocation_ptr);
        for value in values {
            self.push_borrowed(value)?;
        }
        debug_assert_eq!(self.values.len(), end);
        debug_assert_eq!(self.values.capacity(), allocation_capacity);
        debug_assert_eq!(self.values.as_ptr(), allocation_ptr);
        Ok(())
    }
    fn take(&mut self) -> Vec<F> {
        self.logical_capacity = 0;
        self.allocation_capacity = 0;
        core::mem::take(&mut self.values)
    }
}
impl<F: ProofScalar> Drop for ZeroizingScalarVec<F> {
    fn drop(&mut self) {
        for scalar in &mut self.values {
            scalar.clear_secret();
        }
    }
}
impl<F: ProofScalar> ProverVectorCommitmentTape<F> {
    pub(super) fn new(commitment_len: usize) -> Result<Self, FcmpNativeErrorV1> {
        let layout = VectorCommitmentTape::new(commitment_len)?;
        // Every commitment is non-empty, and the public row bound is also the
        // maximum commitment count accepted by the FCMP statement. Reserve
        // that complete outer allocation before any opening scalar is copied
        // into an inner buffer.
        let mut values = Vec::new();
        values
            .try_reserve_exact(commitment_len)
            .map_err(|_| FcmpNativeErrorV1::TreeFull)?;
        let values_allocation_capacity = values.capacity();
        if values_allocation_capacity < commitment_len {
            return Err(FcmpNativeErrorV1::TreeFull);
        }
        Ok(Self {
            layout,
            values,
            values_logical_capacity: commitment_len,
            values_allocation_capacity,
        })
    }
    pub(super) const fn commitment_count(&self) -> usize {
        self.layout.commitment_count()
    }
    fn has_commitment_capacity(&self) -> bool {
        self.values.len() < self.values_logical_capacity
    }
    fn push_commitment(
        &mut self,
        commitment: ZeroizingScalarVec<F>,
    ) -> Result<(), FcmpNativeErrorV1> {
        if !self.has_commitment_capacity() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        debug_assert_eq!(self.values.capacity(), self.values_allocation_capacity);
        self.values.push(commitment);
        debug_assert_eq!(self.values.capacity(), self.values_allocation_capacity);
        Ok(())
    }
    fn append_word(&mut self, values: &[F]) -> Result<Vec<Variable>, FcmpNativeErrorV1> {
        if values.len() != COMMITMENT_WORD_LEN {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let starts_commitment = self.layout.current_j_offset == 0;
        if starts_commitment && !self.has_commitment_capacity() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let mut destination = starts_commitment
            .then(|| ZeroizingScalarVec::new(self.layout.commitment_len))
            .transpose()?;
        let variables = self.layout.append_word()?;
        if starts_commitment {
            let mut destination = destination
                .take()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            destination.extend_borrowed_v1(values)?;
            self.push_commitment(destination)?;
        } else {
            self.values
                .last_mut()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                .extend_borrowed_v1(values)?;
        }
        if self.values.len() != self.layout.commitments {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        Ok(variables)
    }
    pub(super) fn append_branch(
        &mut self,
        branch: &[F],
    ) -> Result<Vec<Variable>, FcmpNativeErrorV1> {
        if !self.has_commitment_capacity() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let mut destination = ZeroizingScalarVec::new(self.layout.commitment_len)?;
        let variables = self.layout.append_branch(branch.len())?;
        destination.extend_borrowed_v1(branch)?;
        self.push_commitment(destination)?;
        if self.values.len() != self.layout.commitments {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        Ok(variables)
    }
    pub(super) fn append_dlog(
        &mut self,
        parameters: DlogParameters,
        dlog: &[u64],
        padding: &[F],
        extra: &F,
    ) -> Result<(Vec<Variable>, Vec<Variable>, Variable), FcmpNativeErrorV1> {
        parameters.validate()?;
        if dlog.len() != parameters.scalar_bits || padding.len() > 255 - parameters.scalar_bits {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let mut witness = ZeroizingScalarVec::new(2 * COMMITMENT_WORD_LEN)?;
        for coefficient in dlog {
            let coefficient = SecretDlogCoefficientV1::copy_from_borrowed(coefficient);
            let scalar = coefficient.into_scalar_owner_v1::<F>();
            witness.push_owned(scalar)?;
        }
        witness.extend_borrowed_v1(padding)?;
        while witness.len() < (2 * COMMITMENT_WORD_LEN) - 1 {
            witness.push_borrowed(&F::ZERO)?;
        }
        witness.push_borrowed(extra)?;
        let mut variables = self.append_word(&witness.as_slice()[..COMMITMENT_WORD_LEN])?;
        variables.extend(self.append_word(&witness.as_slice()[COMMITMENT_WORD_LEN..])?);
        let extra = variables
            .pop()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let padding = variables[parameters.scalar_bits..255].to_vec();
        let dlog = variables[..parameters.scalar_bits].to_vec();
        Ok((dlog, padding, extra))
    }
    pub(super) fn append_divisor(
        &mut self,
        parameters: DlogParameters,
        divisor: &NormalizedDivisor<F>,
        extra: &F,
    ) -> Result<(Divisor, Variable), FcmpNativeErrorV1> {
        parameters.validate()?;
        if divisor.yx.len() > parameters.yx_coefficients
            || divisor.x.is_empty()
            || divisor.x.len() > parameters.x_coefficients
            || divisor.x[0] != F::ONE
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let mut witness = ZeroizingScalarVec::new(2 * COMMITMENT_WORD_LEN)?;
        witness.push_borrowed(&divisor.y)?;
        for index in 0..parameters.yx_coefficients {
            witness.push_borrowed(divisor.yx.get(index).unwrap_or(&F::ZERO))?;
        }
        for index in 1..parameters.x_coefficients {
            witness.push_borrowed(divisor.x.get(index).unwrap_or(&F::ZERO))?;
        }
        witness.push_borrowed(&divisor.zero)?;
        if witness.len() > 255 {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        while witness.len() < (2 * COMMITMENT_WORD_LEN) - 1 {
            witness.push_borrowed(&F::ZERO)?;
        }
        witness.push_borrowed(extra)?;
        let mut variables = self.append_word(&witness.as_slice()[..COMMITMENT_WORD_LEN])?;
        variables.extend(self.append_word(&witness.as_slice()[COMMITMENT_WORD_LEN..])?);
        let extra = variables
            .pop()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let mut cursor = 1;
        let yx_end = cursor + parameters.yx_coefficients;
        let yx = variables[cursor..yx_end].to_vec();
        cursor = yx_end;
        let x_end = cursor + parameters.x_coefficients_minus_one;
        let x_from_power_of_2 = variables[cursor..x_end].to_vec();
        cursor = x_end;
        let zero = *variables
            .get(cursor)
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        Ok((
            Divisor {
                y: variables[0],
                yx,
                x_from_power_of_2,
                zero,
            },
            extra,
        ))
    }
    pub(super) fn append_claimed_point(
        &mut self,
        parameters: DlogParameters,
        dlog: &[u64],
        divisor: &NormalizedDivisor<F>,
        point: &(F, F),
        padding: &[F],
    ) -> Result<(PointWithDlog, Vec<Variable>), FcmpNativeErrorV1> {
        let (dlog, padding, x) = self.append_dlog(parameters, dlog, padding, &point.0)?;
        let (divisor, y) = self.append_divisor(parameters, divisor, &point.1)?;
        Ok((
            PointWithDlog {
                point: (x, y),
                dlog,
                divisor,
            },
            padding,
        ))
    }
    pub(super) fn commitments_and_openings<S: ProofSuite<Scalar = F>>(
        mut self,
        generators: ProofGeneratorView<'_, S>,
        masks: &[F],
    ) -> Result<(Vec<SecretPoint<S::Point>>, Vec<VectorCommitmentOpening<F>>), FcmpNativeErrorV1>
    {
        if self.values.len() != self.layout.commitments || masks.len() != self.values.len() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let commitment_count = self.values.len();
        // Reserve every public result slot before masks or opening scalars are
        // copied into their final owners. This keeps allocation failure
        // recoverable, so dropping `self` still clears every accepted opening.
        let mut commitments = Vec::new();
        commitments
            .try_reserve_exact(commitment_count)
            .map_err(|_| FcmpNativeErrorV1::TreeFull)?;
        let commitments_allocation_capacity = commitments.capacity();
        if commitments_allocation_capacity < commitment_count {
            return Err(FcmpNativeErrorV1::TreeFull);
        }
        let mut openings = Vec::new();
        openings
            .try_reserve_exact(commitment_count)
            .map_err(|_| FcmpNativeErrorV1::TreeFull)?;
        let openings_allocation_capacity = openings.capacity();
        if openings_allocation_capacity < commitment_count {
            return Err(FcmpNativeErrorV1::TreeFull);
        }
        let mut owned_masks = ZeroizingScalarVec::new(masks.len())?;
        owned_masks.extend_borrowed_v1(masks)?;
        for (values, mask) in self.values.iter().zip(owned_masks.as_slice()) {
            if values.as_slice().is_empty() || values.len() > generators.g_bold.len() {
                return Err(FcmpNativeErrorV1::ArithmeticInvariant);
            }
            let term_count = values
                .len()
                .checked_add(1)
                .ok_or(FcmpNativeErrorV1::TreeFull)?;
            let mut terms = SecretMultiexpBuilder::<S>::new(term_count)?;
            for (scalar, point) in values.as_slice().iter().zip(generators.g_bold) {
                terms.push(scalar, point)?;
            }
            terms.push(mask, &generators.h)?;
            let commitment = terms.evaluate()?;
            if commitment.is_identity() {
                return Err(FcmpNativeErrorV1::CircuitProverCommitmentIdentity);
            }
            commitments.push(commitment);
            debug_assert_eq!(commitments.capacity(), commitments_allocation_capacity);
        }
        for (values, mask) in self.values.iter_mut().zip(owned_masks.values.iter_mut()) {
            openings.push(VectorCommitmentOpening::take_mask_from_slot(
                values.take(),
                mask,
            ));
            debug_assert_eq!(openings.capacity(), openings_allocation_capacity);
        }
        Ok((commitments, openings))
    }
}
/// Exact verifier-side arithmetic circuit. Its multiplication count is
/// retained separately from constraints because unconstrained witness gates
/// are still part of the generalized Bulletproof vectors.
pub(super) struct Circuit<S: ProofSuite> {
    muls: usize,
    constraints: Vec<LinComb<S::Scalar>>,
    prover: Option<CircuitProverData<S>>,
}
/// One named secret scalar slot cleared whenever control leaves its scope.
///
/// `ProofScalar` is `Copy`, so this cannot erase compiler temporaries. It does
/// ensure that the intentional circuit-evaluation accumulator and its returned
/// owner are cleared on success, error, and unwind paths.
struct SecretScalarGuard<F: ProofScalar>(F);
impl<F: ProofScalar> SecretScalarGuard<F> {
    fn new(mut value: F) -> Self {
        let incoming = BorrowedProofScalarSlot(&mut value);
        let owned = Self(incoming.get());
        drop(incoming);
        owned
    }
    /// Copies a borrowed secret directly into this drop-zeroizing owner.
    fn copy_from_borrowed(value: &F) -> Self {
        Self(*value)
    }
    fn expose_copy(&self) -> F {
        self.0
    }
    fn expose_ref(&self) -> &F {
        &self.0
    }
    fn expose_mut(&mut self) -> &mut F {
        &mut self.0
    }
}
impl<F: ProofScalar> Drop for SecretScalarGuard<F> {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}
struct CircuitProverWitnesses<F: ProofScalar> {
    a_l: ZeroizingScalarVec<F>,
    a_r: ZeroizingScalarVec<F>,
}
impl<F: ProofScalar> CircuitProverWitnesses<F> {
    fn new(max_muls: usize) -> Result<Self, FcmpNativeErrorV1> {
        // Both complete public-size allocations happen before either vector
        // can receive a private witness scalar.
        let a_l = ZeroizingScalarVec::new(max_muls)?;
        let a_r = ZeroizingScalarVec::new(max_muls)?;
        Ok(Self { a_l, a_r })
    }
    fn len(&self) -> Result<usize, FcmpNativeErrorV1> {
        if self.a_l.len() != self.a_r.len() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        Ok(self.a_l.len())
    }
    fn row_capacity(&self) -> Result<usize, FcmpNativeErrorV1> {
        if self.a_l.logical_capacity != self.a_r.logical_capacity
            || self.a_l.allocation_capacity < self.a_l.logical_capacity
            || self.a_r.allocation_capacity < self.a_r.logical_capacity
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        Ok(self.a_l.logical_capacity)
    }
    fn push_gate(&mut self, left: &F, right: &F) -> Result<(), FcmpNativeErrorV1> {
        // Validate both preallocated vectors before making the only
        // intentional witness copies. The scalar-vector insertion boundary
        // immediately guards and clears each callee-owned copy.
        let row_capacity = self.row_capacity()?;
        if self.len()? >= row_capacity {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        self.a_l.push_borrowed(left)?;
        self.a_r.push_borrowed(right)?;
        Ok(())
    }
    fn take(&mut self) -> (Vec<F>, Vec<F>) {
        (self.a_l.take(), self.a_r.take())
    }
}
struct CircuitProverData<S: ProofSuite> {
    witnesses: CircuitProverWitnesses<S::Scalar>,
    vector_commitments: Vec<VectorCommitmentOpening<S::Scalar>>,
}
fn eval_prover_lincomb<F: ProofScalar>(
    lincomb: &LinComb<F>,
    witnesses: &CircuitProverWitnesses<F>,
    vector_commitments: &[VectorCommitmentOpening<F>],
) -> Result<SecretScalarGuard<F>, FcmpNativeErrorV1> {
    let mut result = SecretScalarGuard::new(lincomb.c);
    for (index, weight) in &lincomb.wl {
        *result.expose_mut() += *witnesses
            .a_l
            .as_slice()
            .get(*index)
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
            * *weight;
    }
    for (index, weight) in &lincomb.wr {
        *result.expose_mut() += *witnesses
            .a_r
            .as_slice()
            .get(*index)
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
            * *weight;
    }
    for (index, weight) in &lincomb.wo {
        *result.expose_mut() += *witnesses
            .a_l
            .as_slice()
            .get(*index)
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
            * *witnesses
                .a_r
                .as_slice()
                .get(*index)
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
            * *weight;
    }
    for (commitment, weights) in lincomb.wcg.iter().enumerate() {
        let values = &vector_commitments
            .get(commitment)
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
            .values;
        for (index, weight) in weights {
            *result.expose_mut() += *values
                .0
                .get(*index)
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                * *weight;
        }
    }
    Ok(result)
}
impl<S: ProofSuite> Circuit<S> {
    pub(super) fn prove(
        vector_commitments: Vec<VectorCommitmentOpening<S::Scalar>>,
        max_muls: usize,
    ) -> Result<Self, FcmpNativeErrorV1> {
        let witnesses = CircuitProverWitnesses::new(max_muls)?;
        Ok(Self {
            muls: 0,
            constraints: Vec::new(),
            prover: Some(CircuitProverData {
                witnesses,
                vector_commitments,
            }),
        })
    }
    pub(super) fn verify() -> Self {
        Self {
            muls: 0,
            constraints: Vec::new(),
            prover: None,
        }
    }
    pub(super) const fn muls(&self) -> usize {
        self.muls
    }
    pub(super) fn constrain_equal_to_zero(&mut self, lincomb: LinComb<S::Scalar>) {
        self.constraints.push(lincomb);
    }
    pub(super) fn equality(&mut self, left: LinComb<S::Scalar>, right: &LinComb<S::Scalar>) {
        self.constrain_equal_to_zero(left - right);
    }
    fn eval(
        &self,
        lincomb: &LinComb<S::Scalar>,
    ) -> Result<Option<SecretScalarGuard<S::Scalar>>, FcmpNativeErrorV1> {
        let Some(prover) = &self.prover else {
            return Ok(None);
        };
        Ok(Some(eval_prover_lincomb(
            lincomb,
            &prover.witnesses,
            &prover.vector_commitments,
        )?))
    }
    pub(super) fn mul_with_witness(
        &mut self,
        left: Option<LinComb<S::Scalar>>,
        right: Option<LinComb<S::Scalar>>,
        witness: Option<(&S::Scalar, &S::Scalar)>,
    ) -> Result<(Variable, Variable, Variable), FcmpNativeErrorV1> {
        if self.prover.is_some() != witness.is_some() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let index = self.muls;
        let next_muls = self
            .muls
            .checked_add(1)
            .ok_or(FcmpNativeErrorV1::TreeFull)?;
        let l = Variable::aL(index);
        let r = Variable::aR(index);
        let o = Variable::aO(index);
        if let Some((left, right)) = witness {
            let prover = self
                .prover
                .as_mut()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            prover.witnesses.push_gate(left, right)?;
        }
        self.muls = next_muls;
        if let Some(left) = left {
            self.constrain_equal_to_zero(left.term(-S::Scalar::ONE, l));
        }
        if let Some(right) = right {
            self.constrain_equal_to_zero(right.term(-S::Scalar::ONE, r));
        }
        Ok((l, r, o))
    }
    pub(super) fn mul(
        &mut self,
        left: Option<LinComb<S::Scalar>>,
        right: Option<LinComb<S::Scalar>>,
    ) -> Result<(Variable, Variable, Variable), FcmpNativeErrorV1> {
        match (&left, &right) {
            (Some(left), Some(right)) => match (self.eval(left)?, self.eval(right)?) {
                (Some(left_witness), Some(right_witness)) => self.mul_with_witness(
                    Some(left.clone()),
                    Some(right.clone()),
                    Some((left_witness.expose_ref(), right_witness.expose_ref())),
                ),
                (None, None) => {
                    self.mul_with_witness(Some(left.clone()), Some(right.clone()), None)
                }
                _ => return Err(FcmpNativeErrorV1::ArithmeticInvariant),
            },
            _ if self.prover.is_none() => self.mul_with_witness(left, right, None),
            _ => return Err(FcmpNativeErrorV1::ArithmeticInvariant),
        }
    }
    pub(super) fn inverse(
        &mut self,
        value: Option<LinComb<S::Scalar>>,
    ) -> Result<(Variable, Variable), FcmpNativeErrorV1> {
        let evaluated = match value.as_ref() {
            Some(value) => self.eval(value)?,
            None if self.prover.is_none() => None,
            None => return Err(FcmpNativeErrorV1::ArithmeticInvariant),
        };
        let (l, r, o) = match evaluated {
            Some(value_witness) => {
                let inverse_witness = SecretScalarGuard::new(
                    value_witness
                        .expose_copy()
                        .invert()
                        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
                );
                self.mul_with_witness(
                    value,
                    None,
                    Some((value_witness.expose_ref(), inverse_witness.expose_ref())),
                )?
            }
            None => self.mul_with_witness(value, None, None)?,
        };
        self.constrain_equal_to_zero(LinComb::from(o).constant(-S::Scalar::ONE));
        Ok((l, r))
    }
    pub(super) fn inequality(
        &mut self,
        left: LinComb<S::Scalar>,
        right: &LinComb<S::Scalar>,
    ) -> Result<(), FcmpNativeErrorV1> {
        self.inverse(Some(left - right))?;
        Ok(())
    }
    pub(super) fn on_curve(
        &mut self,
        curve: &CurveSpec<S::Scalar>,
        (x, y): (Variable, Variable),
    ) -> Result<OnCurve, FcmpNativeErrorV1> {
        let (_, _, x2) = self.mul(Some(LinComb::from(x)), Some(LinComb::from(x)))?;
        let (_, _, x3) = self.mul(Some(LinComb::from(x2)), Some(LinComb::from(x)))?;
        let expected_y2 = LinComb::from(x3).term(curve.a, x).constant(curve.b);
        let (_, _, y2) = self.mul(Some(LinComb::from(y)), Some(LinComb::from(y)))?;
        self.equality(LinComb::from(y2), &expected_y2);
        Ok(OnCurve { x, y })
    }
    pub(super) fn incomplete_add_fixed(
        &mut self,
        fixed: (S::Scalar, S::Scalar),
        addend: OnCurve,
        sum: OnCurve,
    ) -> Result<OnCurve, FcmpNativeErrorV1> {
        self.inequality(LinComb::from(addend.x), &LinComb::empty().constant(fixed.0))?;
        let (x0, y0) = fixed;
        let (x1, y1) = (addend.x, addend.y);
        let (x2, y2) = (sum.x, sum.y);
        let x1_minus_x0 = LinComb::from(x1).constant(-x0);
        let (slope, _, product) = match (
            self.eval(&LinComb::from(y1).constant(-y0))?,
            self.eval(&x1_minus_x0)?,
        ) {
            (Some(y_difference), Some(x_difference)) => {
                let slope_witness = SecretScalarGuard::new(
                    y_difference.expose_copy()
                        * x_difference
                            .expose_copy()
                            .invert()
                            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
                );
                self.mul_with_witness(
                    None,
                    Some(x1_minus_x0),
                    Some((slope_witness.expose_ref(), x_difference.expose_ref())),
                )?
            }
            (None, None) => self.mul_with_witness(None, Some(x1_minus_x0), None)?,
            _ => return Err(FcmpNativeErrorV1::ArithmeticInvariant),
        };
        self.equality(LinComb::from(product), &LinComb::from(y1).constant(-y0));
        let x2_minus_x0 = LinComb::from(x2).constant(-x0);
        let (_, _, product) = self.mul(Some(LinComb::from(slope)), Some(x2_minus_x0))?;
        self.equality(
            LinComb::from(product),
            &LinComb::empty().term(-S::Scalar::ONE, y2).constant(-y0),
        );
        let (_, _, slope_squared) =
            self.mul(Some(LinComb::from(slope)), Some(LinComb::from(slope)))?;
        self.equality(
            LinComb::from(slope_squared),
            &LinComb::from(x1).term(S::Scalar::ONE, x2).constant(x0),
        );
        Ok(sum)
    }
    pub(super) fn member_of_list(
        &mut self,
        member: LinComb<S::Scalar>,
        list: Vec<LinComb<S::Scalar>>,
    ) -> Result<(), FcmpNativeErrorV1> {
        let mut list = list.into_iter();
        let first = list.next().ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let mut carry = first - &member;
        for item in list {
            let next = item - &member;
            let (_, _, output) = self.mul(Some(carry), Some(next))?;
            carry = LinComb::from(output);
        }
        self.constrain_equal_to_zero(carry);
        Ok(())
    }
    pub(super) fn tuple_member_of_list<T: CircuitTranscript>(
        &mut self,
        transcript: &mut T,
        member: Vec<Variable>,
        list: Vec<Vec<Variable>>,
    ) -> Result<(), FcmpNativeErrorV1> {
        if member.is_empty() || list.is_empty() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        for variable in member.iter().chain(list.iter().flatten()) {
            if !matches!(variable, Variable::CG { .. }) {
                return Err(FcmpNativeErrorV1::ArithmeticInvariant);
            }
        }
        let challenges = (0..member.len())
            .map(|_| transcript.circuit_challenge::<S>())
            .collect::<Result<Vec<_>, _>>()?;
        let aggregate = |variables: Vec<Variable>| {
            let mut result = LinComb::empty();
            for (index, variable) in variables.into_iter().enumerate() {
                result = result + &(LinComb::from(variable) * challenges[index]);
            }
            result
        };
        let member = aggregate(member);
        let mut aggregated_list = Vec::with_capacity(list.len());
        for tuple in list {
            if tuple.len() != challenges.len() {
                return Err(FcmpNativeErrorV1::ArithmeticInvariant);
            }
            aggregated_list.push(aggregate(tuple));
        }
        self.member_of_list(member, aggregated_list)
    }
    pub(super) fn statement<'a>(
        self,
        generators: ProofGeneratorView<'a, S>,
        vector_commitments: Vec<S::Point>,
    ) -> Result<ArithmeticCircuitStatement<'a, S>, FcmpNativeErrorV1> {
        if self.muls > generators.g_bold.len() || self.prover.is_some() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        Ok(ArithmeticCircuitStatement::new(
            generators,
            self.constraints,
            vector_commitments,
            Vec::new(),
        )?)
    }
    pub(super) fn proving_statement<'a>(
        self,
        generators: ProofGeneratorView<'a, S>,
        vector_commitments: Vec<S::Point>,
    ) -> Result<
        (
            ArithmeticCircuitStatement<'a, S>,
            ArithmeticCircuitWitness<S>,
        ),
        FcmpNativeErrorV1,
    > {
        if self.muls > generators.g_bold.len() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let mut prover = self.prover.ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        if prover.witnesses.row_capacity()? != generators.g_bold.len()
            || prover.witnesses.len()? != self.muls
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let statement = ArithmeticCircuitStatement::new(
            generators,
            self.constraints,
            vector_commitments,
            Vec::new(),
        )?;
        let (a_l, a_r) = prover.witnesses.take();
        let witness = ArithmeticCircuitWitness::new(
            a_l,
            a_r,
            core::mem::take(&mut prover.vector_commitments),
        )?;
        Ok((statement, witness))
    }
}
/// Affine table `[G, 2G, 4G, ...]` used by the divisor interpolation gadget.
#[derive(Clone, Debug)]
pub(super) struct GeneratorTable<F: ProofScalar> {
    points: Vec<(F, F)>,
}
impl<F: ProofScalar> GeneratorTable<F> {
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.points.len()
    }
    pub(super) fn new(
        curve: &CurveSpec<F>,
        generator: (F, F),
        parameters: DlogParameters,
    ) -> Result<Self, FcmpNativeErrorV1> {
        parameters.validate()?;
        if (generator.1.square())
            != ((generator.0.square() * generator.0) + (curve.a * generator.0) + curve.b)
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        fn double<F: ProofScalar>(a: F, (x1, y1): (F, F)) -> Result<(F, F), FcmpNativeErrorV1> {
            // mdbl-2007-bl, normalized from the X/Y/Z representation used by
            // the pinned EC gadget.
            let xx = x1 * x1;
            let w = a + (xx + xx.double());
            let y1y1 = y1 * y1;
            let r = y1y1.double();
            let sss = (y1 * r).double().double();
            let rr = r * r;
            let b = ((x1 + r) * (x1 + r)) - xx - rr;
            let h = (w * w) - b.double();
            let x3 = h.double() * y1;
            let y3 = (w * (b - h)) - rr.double();
            let inverse = sss.invert().ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            Ok((x3 * inverse, y3 * inverse))
        }
        let mut points = Vec::with_capacity(parameters.scalar_bits);
        points.push(generator);
        while points.len() < parameters.scalar_bits {
            let previous = points
                .last()
                .copied()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
            points.push(double(curve.a, previous)?);
        }
        Ok(Self { points })
    }
}
#[derive(Clone, Debug)]
struct ChallengePoint<F: ProofScalar> {
    y: F,
    yx: Vec<F>,
    x: Vec<F>,
    p_0_n_0: F,
    x_p_0_n_0: Vec<F>,
    p_1_n: F,
    p_1_d: F,
}
impl<F: ProofScalar> ChallengePoint<F> {
    fn new(
        curve: &CurveSpec<F>,
        parameters: DlogParameters,
        slope: F,
        x: F,
        y: F,
        inverse_two_y: F,
    ) -> Result<Self, FcmpNativeErrorV1> {
        parameters.validate()?;
        let mut x_powers = Vec::with_capacity(parameters.x_coefficients);
        let mut power = x;
        for _ in 0..parameters.x_coefficients {
            x_powers.push(power);
            power *= x;
        }
        let yx = x_powers
            .iter()
            .take(parameters.yx_coefficients)
            .map(|power| y * *power)
            .collect::<Vec<_>>();
        let three_x_squared_plus_a = (x.square() * F::from_u64(3)) + curve.a;
        let two_y = y.double();
        let p_0_n_0 = three_x_squared_plus_a * inverse_two_y;
        let x_p_0_n_0 = x_powers
            .iter()
            .take(parameters.yx_coefficients)
            .map(|power| p_0_n_0 * *power)
            .collect();
        Ok(Self {
            y,
            yx,
            x: x_powers,
            p_0_n_0,
            x_p_0_n_0,
            p_1_n: two_y,
            p_1_d: (-slope * two_y) + three_x_squared_plus_a,
        })
    }
}
#[derive(Clone, Debug)]
pub(super) struct DiscreteLogChallenge<F: ProofScalar> {
    c0: ChallengePoint<F>,
    c1: ChallengePoint<F>,
    c2: ChallengePoint<F>,
    slope: F,
    intercept: F,
}
#[derive(Clone, Debug)]
pub(super) struct ChallengedGenerator<F: ProofScalar>(Vec<F>);
fn batch_invert<F: ProofScalar>(values: &mut [F]) -> Result<(), FcmpNativeErrorV1> {
    if values.iter().any(|value| value.is_zero()) {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    let mut prefixes = Vec::with_capacity(values.len());
    let mut product = F::ONE;
    for value in values.iter().copied() {
        prefixes.push(product);
        product *= value;
    }
    let mut inverse = product
        .invert()
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    for index in (0..values.len()).rev() {
        let original = values[index];
        values[index] = inverse * prefixes[index];
        inverse *= original;
    }
    Ok(())
}
fn sample_embedded_curve_point<S: ProofSuite, T: CircuitTranscript>(
    transcript: &mut T,
    curve: &CurveSpec<S::Scalar>,
    odd_y: bool,
) -> Result<(S::Scalar, S::Scalar), FcmpNativeErrorV1> {
    for _ in 0..MAX_EMBEDDED_POINT_ATTEMPTS_V1 {
        let x = transcript.circuit_challenge::<S>()?;
        let rhs = (x.square() * x) + (curve.a * x) + curve.b;
        if let Some(mut y) = rhs.sqrt() {
            if y.is_odd() != odd_y {
                y = -y;
            }
            return Ok((x, y));
        }
    }
    Err(FcmpNativeErrorV1::DlogChallengeExhausted)
}
fn incomplete_add<F: ProofScalar>(
    first: (F, F),
    second: (F, F),
) -> Result<(F, F), FcmpNativeErrorV1> {
    if first.0 == second.0 {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    let u = second.1 - first.1;
    let uu = u * u;
    let v = second.0 - first.0;
    let vv = v * v;
    let vvv = v * vv;
    let r = vv * first.0;
    let a = uu - vvv - r.double();
    let x3 = v * a;
    let y3 = (u * (r - a)) - (vvv * first.1);
    let inverse = vvv.invert().ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    Ok((x3 * inverse, y3 * inverse))
}
fn discrete_log_challenge_once<S: ProofSuite, T: CircuitTranscript>(
    transcript: &mut T,
    curve: &CurveSpec<S::Scalar>,
    parameters: DlogParameters,
    generators: &[&GeneratorTable<S::Scalar>],
) -> Result<
    Option<(
        DiscreteLogChallenge<S::Scalar>,
        Vec<ChallengedGenerator<S::Scalar>>,
    )>,
    FcmpNativeErrorV1,
> {
    parameters.validate()?;
    if generators.is_empty()
        || generators
            .iter()
            .any(|table| table.points.len() != parameters.scalar_bits)
    {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    let signs = transcript.circuit_challenge_bytes();
    let c0 = sample_embedded_curve_point::<S, T>(transcript, curve, signs[0] & 1 == 1)?;
    let c1 = sample_embedded_curve_point::<S, T>(transcript, curve, (signs[0] >> 1) & 1 == 1)?;
    if c0.0 == c1.0 {
        return Ok(None);
    }
    let (c2_x, c2_y) = incomplete_add(c0, c1)?;
    let c2 = (c2_x, -c2_y);
    let slope = (c1.1 - c0.1)
        * (c1.0 - c0.0)
            .invert()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    let intercept = c0.1 - (slope * c0.0);
    let mut inversions = Vec::with_capacity(3 + generators.len() * parameters.scalar_bits);
    inversions.extend([c0.1.double(), c1.1.double(), c2.1.double()]);
    for generator in generators {
        inversions.extend(
            generator
                .points
                .iter()
                .map(|(x, y)| intercept - (*y - (slope * *x))),
        );
    }
    if inversions.iter().any(|value| value.is_zero()) {
        return Ok(None);
    }
    batch_invert(&mut inversions)?;
    let mut inversions = inversions.into_iter();
    let c0 = ChallengePoint::new(
        curve,
        parameters,
        slope,
        c0.0,
        c0.1,
        inversions
            .next()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
    )?;
    let c1 = ChallengePoint::new(
        curve,
        parameters,
        slope,
        c1.0,
        c1.1,
        inversions
            .next()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
    )?;
    let c2 = ChallengePoint::new(
        curve,
        parameters,
        slope,
        c2.0,
        c2.1,
        inversions
            .next()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
    )?;
    if c0.p_1_d.is_zero() || c1.p_1_d.is_zero() || c2.p_1_d.is_zero() {
        return Ok(None);
    }
    let mut challenged = Vec::with_capacity(generators.len());
    for _ in generators {
        let weights = inversions
            .by_ref()
            .take(parameters.scalar_bits)
            .collect::<Vec<_>>();
        if weights.len() != parameters.scalar_bits {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        challenged.push(ChallengedGenerator(weights));
    }
    if inversions.next().is_some() {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    Ok(Some((
        DiscreteLogChallenge {
            c0,
            c1,
            c2,
            slope,
            intercept,
        },
        challenged,
    )))
}
pub(super) fn discrete_log_challenge<S: ProofSuite, T: CircuitTranscript>(
    transcript: &mut T,
    curve: &CurveSpec<S::Scalar>,
    parameters: DlogParameters,
    generators: &[&GeneratorTable<S::Scalar>],
) -> Result<
    (
        DiscreteLogChallenge<S::Scalar>,
        Vec<ChallengedGenerator<S::Scalar>>,
    ),
    FcmpNativeErrorV1,
> {
    for _ in 0..MAX_DLOG_CHALLENGE_ATTEMPTS_V1 {
        if let Some(challenge) =
            discrete_log_challenge_once::<S, T>(transcript, curve, parameters, generators)?
        {
            return Ok(challenge);
        }
    }
    Err(FcmpNativeErrorV1::DlogChallengeExhausted)
}
fn divisor_challenge_eval<S: ProofSuite>(
    circuit: &mut Circuit<S>,
    divisor: &Divisor,
    challenge: &ChallengePoint<S::Scalar>,
) -> Result<Variable, FcmpNativeErrorV1> {
    if divisor.yx.len() != challenge.yx.len()
        || divisor.x_from_power_of_2.len() + 1 != challenge.x.len()
        || divisor.yx.len() != challenge.x_p_0_n_0.len()
    {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    let mut p_0_n_1 = LinComb::empty().term(challenge.p_0_n_0, divisor.y);
    for (variable, weight) in divisor.yx.iter().zip(&challenge.x_p_0_n_0) {
        p_0_n_1 = p_0_n_1.term(*weight, *variable);
    }
    let mut p_0_n_2 = LinComb::empty().constant(S::Scalar::ONE);
    let first_yx = *divisor
        .yx
        .first()
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    p_0_n_2 = p_0_n_2.term(challenge.y, first_yx);
    for (index, variable) in divisor.yx.iter().enumerate().skip(1) {
        let original_power =
            S::Scalar::from_u64(u64::try_from(index + 1).map_err(|_| FcmpNativeErrorV1::TreeFull)?);
        p_0_n_2 = p_0_n_2.term(original_power * challenge.yx[index - 1], *variable);
    }
    for (index, variable) in divisor.x_from_power_of_2.iter().enumerate() {
        let original_power =
            S::Scalar::from_u64(u64::try_from(index + 2).map_err(|_| FcmpNativeErrorV1::TreeFull)?);
        p_0_n_2 = p_0_n_2.term(original_power * challenge.x[index], *variable);
    }
    let p_0_n = p_0_n_1 + &p_0_n_2;
    let mut p_0_d = LinComb::empty().term(challenge.y, divisor.y);
    for (variable, weight) in divisor.yx.iter().zip(&challenge.yx) {
        p_0_d = p_0_d.term(*weight, *variable);
    }
    for (index, variable) in divisor.x_from_power_of_2.iter().enumerate() {
        p_0_d = p_0_d.term(challenge.x[index + 1], *variable);
    }
    p_0_d = p_0_d
        .term(S::Scalar::ONE, divisor.zero)
        .constant(challenge.x[0]);
    let p_n = p_0_n * challenge.p_1_n;
    let p_d = p_0_d * challenge.p_1_d;
    let (_, quotient, numerator_claim) = match (circuit.eval(&p_d)?, circuit.eval(&p_n)?) {
        (Some(denominator), Some(numerator)) => {
            let quotient_witness = SecretScalarGuard::new(
                numerator.expose_copy()
                    * denominator
                        .expose_copy()
                        .invert()
                        .ok_or(FcmpNativeErrorV1::DlogWitnessPole)?,
            );
            circuit.mul_with_witness(
                Some(p_d),
                None,
                Some((denominator.expose_ref(), quotient_witness.expose_ref())),
            )?
        }
        (None, None) => circuit.mul_with_witness(Some(p_d), None, None)?,
        _ => return Err(FcmpNativeErrorV1::ArithmeticInvariant),
    };
    circuit.equality(p_n, &LinComb::from(numerator_claim));
    Ok(quotient)
}
fn reject_hidden_dlog_pole<F: ProofScalar>(
    denominator: Option<F>,
) -> Result<(), FcmpNativeErrorV1> {
    if denominator == Some(F::ZERO) {
        return Err(FcmpNativeErrorV1::DlogWitnessPole);
    }
    Ok(())
}
pub(super) fn discrete_log<S: ProofSuite>(
    circuit: &mut Circuit<S>,
    curve: &CurveSpec<S::Scalar>,
    parameters: DlogParameters,
    point: PointWithDlog,
    challenge: &DiscreteLogChallenge<S::Scalar>,
    challenged_generator: &ChallengedGenerator<S::Scalar>,
) -> Result<OnCurve, FcmpNativeErrorV1> {
    parameters.validate()?;
    if point.dlog.len() != parameters.scalar_bits
        || point.divisor.yx.len() != parameters.yx_coefficients
        || point.divisor.x_from_power_of_2.len() != parameters.x_coefficients_minus_one
        || challenged_generator.0.len() != parameters.scalar_bits
    {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    for variable in [
        point.point.0,
        point.point.1,
        point.divisor.y,
        point.divisor.zero,
    ]
    .iter()
    .chain(&point.divisor.yx)
    .chain(&point.divisor.x_from_power_of_2)
    .chain(&point.dlog)
    {
        if !matches!(variable, Variable::CG { .. }) {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
    }
    let point_on_curve = circuit.on_curve(curve, point.point)?;
    let lhs = LinComb::from(divisor_challenge_eval(
        circuit,
        &point.divisor,
        &challenge.c0,
    )?) + &LinComb::from(divisor_challenge_eval(
        circuit,
        &point.divisor,
        &challenge.c1,
    )?) + &LinComb::from(divisor_challenge_eval(
        circuit,
        &point.divisor,
        &challenge.c2,
    )?);
    let mut rhs = LinComb::empty();
    for (coefficient, weight) in point.dlog.iter().zip(&challenged_generator.0) {
        rhs = rhs.term(*weight, *coefficient);
    }
    let output_interpolation = LinComb::empty()
        .constant(challenge.intercept)
        .term(S::Scalar::ONE, point_on_curve.y)
        .term(challenge.slope, point_on_curve.x);
    // This denominator depends on the hidden point and the transcript
    // challenge.  It is an honest-abort pole, not a malformed witness; the
    // complete prover must rebuild fresh commitments and retry.
    let denominator = circuit.eval(&output_interpolation)?;
    reject_hidden_dlog_pole(denominator.as_ref().map(SecretScalarGuard::expose_copy))?;
    let (_, inverse) = circuit.inverse(Some(output_interpolation))?;
    rhs = rhs.term(S::Scalar::ONE, inverse);
    circuit.equality(lhs, &rhs);
    Ok(point_on_curve)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn first_layer<S: ProofSuite, T: CircuitTranscript>(
    circuit: &mut Circuit<S>,
    transcript: &mut T,
    curve: &CurveSpec<S::Scalar>,
    parameters: DlogParameters,
    t_table: &GeneratorTable<S::Scalar>,
    u_table: &GeneratorTable<S::Scalar>,
    v_table: &GeneratorTable<S::Scalar>,
    g_table: &GeneratorTable<S::Scalar>,
    output_key_tilde: (S::Scalar, S::Scalar),
    output_blind: PointWithDlog,
    output_key: (Variable, Variable),
    linking_generator_tilde: (S::Scalar, S::Scalar),
    input_blind_u: PointWithDlog,
    linking_generator: (Variable, Variable),
    rerandomization_commitment: (S::Scalar, S::Scalar),
    input_blind_v: PointWithDlog,
    input_blind_blind: PointWithDlog,
    pseudo_out: (S::Scalar, S::Scalar),
    commitment_blind: PointWithDlog,
    amount_commitment: (Variable, Variable),
    branch: Vec<Vec<Variable>>,
) -> Result<(), FcmpNativeErrorV1> {
    let (challenge, challenged) = discrete_log_challenge::<S, T>(
        transcript,
        curve,
        parameters,
        &[t_table, u_table, v_table, g_table],
    )?;
    let [challenged_t, challenged_u, challenged_v, challenged_g]: [ChallengedGenerator<S::Scalar>;
        4] = challenged
        .try_into()
        .map_err(|_| FcmpNativeErrorV1::ArithmeticInvariant)?;
    let output_key = circuit.on_curve(curve, output_key)?;
    let output_blind = discrete_log(
        circuit,
        curve,
        parameters,
        output_blind,
        &challenge,
        &challenged_t,
    )?;
    circuit.incomplete_add_fixed(output_key_tilde, output_blind, output_key)?;
    if input_blind_u.dlog != input_blind_v.dlog {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    let linking_generator = circuit.on_curve(curve, linking_generator)?;
    let input_blind_u = discrete_log(
        circuit,
        curve,
        parameters,
        input_blind_u,
        &challenge,
        &challenged_u,
    )?;
    circuit.incomplete_add_fixed(linking_generator_tilde, input_blind_u, linking_generator)?;
    let input_blind_v = discrete_log(
        circuit,
        curve,
        parameters,
        input_blind_v,
        &challenge,
        &challenged_v,
    )?;
    let input_blind_blind = discrete_log(
        circuit,
        curve,
        parameters,
        input_blind_blind,
        &challenge,
        &challenged_t,
    )?;
    circuit.incomplete_add_fixed(rerandomization_commitment, input_blind_v, input_blind_blind)?;
    let amount_commitment = circuit.on_curve(curve, amount_commitment)?;
    let commitment_blind = discrete_log(
        circuit,
        curve,
        parameters,
        commitment_blind,
        &challenge,
        &challenged_g,
    )?;
    circuit.incomplete_add_fixed(pseudo_out, commitment_blind, amount_commitment)?;
    circuit.tuple_member_of_list(
        transcript,
        vec![
            output_key.x(),
            output_key.y(),
            linking_generator.x(),
            linking_generator.y(),
            amount_commitment.x(),
            amount_commitment.y(),
        ],
        branch,
    )
}
pub(super) fn additional_layer_discrete_log_challenge<S: ProofSuite, T: CircuitTranscript>(
    transcript: &mut T,
    curve: &CurveSpec<S::Scalar>,
    parameters: DlogParameters,
    h_table: &GeneratorTable<S::Scalar>,
) -> Result<
    (
        DiscreteLogChallenge<S::Scalar>,
        ChallengedGenerator<S::Scalar>,
    ),
    FcmpNativeErrorV1,
> {
    let (challenge, generators) =
        discrete_log_challenge::<S, T>(transcript, curve, parameters, &[h_table])?;
    let [generator]: [ChallengedGenerator<S::Scalar>; 1] = generators
        .try_into()
        .map_err(|_| FcmpNativeErrorV1::ArithmeticInvariant)?;
    Ok((challenge, generator))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn additional_layer<S: ProofSuite>(
    circuit: &mut Circuit<S>,
    curve: &CurveSpec<S::Scalar>,
    parameters: DlogParameters,
    challenged_h: &(
        DiscreteLogChallenge<S::Scalar>,
        ChallengedGenerator<S::Scalar>,
    ),
    blinded_hash: (S::Scalar, S::Scalar),
    blind: PointWithDlog,
    hash: (Variable, Variable),
    branch: Vec<Variable>,
) -> Result<(), FcmpNativeErrorV1> {
    let blind = discrete_log(
        circuit,
        curve,
        parameters,
        blind,
        &challenged_h.0,
        &challenged_h.1,
    )?;
    let hash = circuit.on_curve(curve, hash)?;
    circuit.incomplete_add_fixed(blinded_hash, blind, hash)?;
    circuit.member_of_list(
        LinComb::from(hash.x()),
        branch.into_iter().map(LinComb::from).collect(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::fcmp_plus_plus::{
        FCMP_LAYER_ONE_LEN_V1, FCMP_LAYER_TWO_LEN_V1,
        field::Field25519,
        proof_math::{HeliosSuite, SeleneSuite, selene_bp_generators},
    };
    use core::{
        cell::Cell,
        ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
    };
    use rand_08::{SeedableRng as _, rngs::StdRng};
    use std::panic::{self, AssertUnwindSafe};
    std::thread_local! {
        static TRACKING_CLEAR_CALLS: Cell<usize> = const { Cell::new(0) };
        static TRACKING_PANIC_ON_ADD_ASSIGN: Cell<bool> = const { Cell::new(false) };
        static TRACKING_PANIC_ON_FROM_U64: Cell<bool> = const { Cell::new(false) };
        static TRACKING_PANIC_ON_CLEAR_CALL: Cell<usize> = const { Cell::new(0) };
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TrackingScalar(u64);
    impl Add for TrackingScalar {
        type Output = Self;
        fn add(self, rhs: Self) -> Self::Output {
            Self(self.0.wrapping_add(rhs.0))
        }
    }
    impl Sub for TrackingScalar {
        type Output = Self;
        fn sub(self, rhs: Self) -> Self::Output {
            Self(self.0.wrapping_sub(rhs.0))
        }
    }
    impl Mul for TrackingScalar {
        type Output = Self;
        fn mul(self, rhs: Self) -> Self::Output {
            Self(self.0.wrapping_mul(rhs.0))
        }
    }
    impl Neg for TrackingScalar {
        type Output = Self;
        fn neg(self) -> Self::Output {
            Self(self.0.wrapping_neg())
        }
    }
    impl AddAssign for TrackingScalar {
        fn add_assign(&mut self, rhs: Self) {
            if TRACKING_PANIC_ON_ADD_ASSIGN.with(|flag| flag.replace(false)) {
                panic!("tracking scalar add-assign panic");
            }
            *self = *self + rhs;
        }
    }
    impl SubAssign for TrackingScalar {
        fn sub_assign(&mut self, rhs: Self) {
            *self = *self - rhs;
        }
    }
    impl MulAssign for TrackingScalar {
        fn mul_assign(&mut self, rhs: Self) {
            *self = *self * rhs;
        }
    }
    impl ProofScalar for TrackingScalar {
        const ZERO: Self = Self(0);
        const ONE: Self = Self(1);
        const SCALAR_BITS: usize = 64;
        fn from_u64(value: u64) -> Self {
            if TRACKING_PANIC_ON_FROM_U64.with(|flag| flag.replace(false)) {
                panic!("tracking scalar from-u64 panic");
            }
            Self(value)
        }
        fn decode(bytes: [u8; 32]) -> Option<Self> {
            Some(Self(u64::from_le_bytes(bytes[..8].try_into().ok()?)))
        }
        fn encode(self) -> [u8; 32] {
            let mut bytes = [0_u8; 32];
            bytes[..8].copy_from_slice(&self.0.to_le_bytes());
            bytes
        }
        fn reduce_wide(bytes: [u8; 64]) -> Self {
            Self(u64::from_le_bytes(
                bytes[..8]
                    .try_into()
                    .expect("tracking scalar prefix has eight bytes"),
            ))
        }
        fn invert(self) -> Option<Self> {
            (!self.is_zero()).then_some(Self::ONE)
        }
        fn sqrt(self) -> Option<Self> {
            Some(self)
        }
        fn square(self) -> Self {
            self * self
        }
        fn double(self) -> Self {
            self + self
        }
        fn is_zero(self) -> bool {
            self == Self::ZERO
        }
        fn is_odd(self) -> bool {
            self.0 & 1 == 1
        }
        fn clear_secret(&mut self) {
            self.0 = 0;
            TRACKING_CLEAR_CALLS.with(|calls| calls.set(calls.get() + 1));
            TRACKING_PANIC_ON_CLEAR_CALL.with(|call| match call.get() {
                0 => {}
                1 => {
                    call.set(0);
                    panic!("tracking scalar clear panic");
                }
                remaining => call.set(remaining - 1),
            });
        }
    }
    fn tracking_values(len: usize, offset: u64) -> ZeroizingScalarVec<TrackingScalar> {
        let mut values = ZeroizingScalarVec::new(len).expect("tracking allocation");
        for index in 0..len {
            values
                .push_borrowed(&TrackingScalar(
                    offset.wrapping_add(u64::try_from(index).expect("small tracking index")),
                ))
                .expect("tracking value fits exact allocation");
        }
        values
    }
    fn reset_tracking_clears() {
        TRACKING_CLEAR_CALLS.with(|calls| calls.set(0));
    }
    fn tracking_clears() -> usize {
        TRACKING_CLEAR_CALLS.with(Cell::get)
    }
    fn set_tracking_add_assign_panic(enabled: bool) {
        TRACKING_PANIC_ON_ADD_ASSIGN.with(|flag| flag.set(enabled));
    }
    fn tracking_add_assign_will_panic() -> bool {
        TRACKING_PANIC_ON_ADD_ASSIGN.with(Cell::get)
    }
    fn set_tracking_clear_panic(call: usize) {
        TRACKING_PANIC_ON_CLEAR_CALL.with(|pending| pending.set(call));
    }
    fn set_tracking_from_u64_panic(enabled: bool) {
        TRACKING_PANIC_ON_FROM_U64.with(|flag| flag.set(enabled));
    }
    fn tracking_from_u64_will_panic() -> bool {
        TRACKING_PANIC_ON_FROM_U64.with(Cell::get)
    }
    fn reset_secret_dlog_coefficient_drops_v1() {
        SECRET_DLOG_COEFFICIENT_DROPS_V1.with(|drops| drops.set(0));
    }
    fn secret_dlog_coefficient_drops_v1() -> usize {
        SECRET_DLOG_COEFFICIENT_DROPS_V1.with(Cell::get)
    }
    #[test]
    fn tape_layout_matches_first_release_word_packing() {
        let mut tape = VectorCommitmentTape::new(256).expect("tape");
        let branch = tape
            .append_branch(6 * FCMP_LAYER_ONE_LEN_V1)
            .expect("branch");
        assert_eq!(branch.len(), 228);
        assert_eq!(tape.commitment_count(), 1);
        let (ed_point, padding) = tape
            .append_claimed_point(ED25519_DLOG_PARAMETERS)
            .expect("Ed point");
        assert_eq!(ed_point.dlog.len(), 253);
        assert_eq!(ed_point.divisor.yx.len(), 125);
        assert_eq!(ed_point.divisor.x_from_power_of_2.len(), 126);
        assert_eq!(padding.len(), 2);
        assert_eq!(tape.commitment_count(), 3);
        let (cycle_point, padding) = tape
            .append_claimed_point(CYCLE_DLOG_PARAMETERS)
            .expect("cycle point");
        assert_eq!(cycle_point.dlog.len(), 255);
        assert!(padding.is_empty());
        assert_eq!(cycle_point.divisor.yx.len(), 126);
        assert_eq!(cycle_point.divisor.x_from_power_of_2.len(), 127);
        assert_eq!(tape.commitment_count(), 5);
    }
    #[test]
    fn row_formulas_are_reproduced_by_gadget_shapes() {
        assert_eq!(MAX_EMBEDDED_POINT_ATTEMPTS_V1, 128);
        assert_eq!(MAX_DLOG_CHALLENGE_ATTEMPTS_V1, 128);
        // These constants are a compact regression on the circuit structure:
        // first-layer tuple opening is 97 rows, a non-leaf Selene layer is 52,
        // and a Helios layer is 32.
        assert_eq!(5 * 7 + 3 * 3 + 4 * 4 + (FCMP_LAYER_ONE_LEN_V1 - 1), 97);
        assert_eq!(1 + 7 + 3 + 4 + (FCMP_LAYER_ONE_LEN_V1 - 1), 52);
        assert_eq!(1 + 7 + 3 + 4 + (FCMP_LAYER_TWO_LEN_V1 - 1), 32);
        // Keep both generic instantiations type-checked.
        let _: Circuit<SeleneSuite> = Circuit::verify();
        let _: Circuit<HeliosSuite> = Circuit::verify();
    }
    #[test]
    fn hidden_dlog_denominator_poles_are_retryable_only_for_provers() {
        assert_eq!(
            reject_hidden_dlog_pole(Some(Field25519::ZERO)),
            Err(FcmpNativeErrorV1::DlogWitnessPole)
        );
        assert!(reject_hidden_dlog_pole(Some(Field25519::ONE)).is_ok());
        assert!(reject_hidden_dlog_pole::<Field25519>(None).is_ok());
    }
    #[test]
    fn prover_tape_and_circuit_emit_a_verifiable_native_witness() {
        let context = [0x81_u8; 32];
        let generators = selene_bp_generators().reduce(128).expect("generators");
        let mut tape = ProverVectorCommitmentTape::new(128).expect("tape");
        let first_values = [Field25519::from_u64(3), Field25519::from_u64(4)];
        let first = tape.append_branch(&first_values).expect("first commitment");
        let second_values = [Field25519::from_u64(9)];
        let second = tape
            .append_branch(&second_values)
            .expect("second commitment");
        let masks = vec![Field25519::from_u64(5), Field25519::from_u64(7)];
        let (secret_commitments, openings) = tape
            .commitments_and_openings::<SeleneSuite>(generators, &masks)
            .expect("commitments");
        assert_eq!(masks, [Field25519::from_u64(5), Field25519::from_u64(7)]);
        let mut circuit = Circuit::<SeleneSuite>::prove(openings, 128).expect("prover circuit");
        let (_, _, product) = circuit
            .mul(Some(LinComb::from(first[0])), Some(LinComb::from(first[1])))
            .expect("multiplication");
        circuit.constrain_equal_to_zero(LinComb::from(product).constant(-Field25519::from_u64(12)));
        circuit
            .constrain_equal_to_zero(LinComb::from(second[0]).constant(-Field25519::from_u64(9)));
        let mut transcript = ProverTranscript::new(context);
        let commitments = transcript
            .write_secret_commitments::<SeleneSuite>(secret_commitments)
            .expect("borrowed secret commitment publication");
        let (statement, witness) = circuit
            .proving_statement(generators, commitments)
            .expect("proving statement");
        let mut rng = StdRng::seed_from_u64(0xc1_0017);
        statement
            .prove(
                &mut FcmpProofRandomSource::new(&mut rng),
                &mut transcript,
                witness,
            )
            .expect("proof");
        let proof = transcript.complete();
        let mut verifier_transcript = VerifierTranscript::new(context, &proof);
        let (commitments, scalar_commitments) = verifier_transcript
            .read_commitments::<SeleneSuite>(2, 0)
            .expect("commitments");
        let mut verifier_tape = VectorCommitmentTape::new(128).expect("verifier tape");
        let first = verifier_tape.append_branch(2).expect("first branch");
        let second = verifier_tape.append_branch(1).expect("second branch");
        let mut verifier = Circuit::<SeleneSuite>::verify();
        let (_, _, product) = verifier
            .mul(Some(LinComb::from(first[0])), Some(LinComb::from(first[1])))
            .expect("multiplication");
        verifier
            .constrain_equal_to_zero(LinComb::from(product).constant(-Field25519::from_u64(12)));
        verifier
            .constrain_equal_to_zero(LinComb::from(second[0]).constant(-Field25519::from_u64(9)));
        verifier
            .statement(generators, commitments)
            .expect("statement")
            .verify(&mut verifier_transcript)
            .expect("verification");
        assert!(scalar_commitments.is_empty());
        assert_eq!(verifier_transcript.consumed(), proof.len());

        let generators = selene_bp_generators().reduce(128).expect("generators");
        let mut identity_tape = ProverVectorCommitmentTape::new(128).expect("identity tape");
        identity_tape
            .append_branch(&[Field25519::ZERO])
            .expect("zero branch");
        let identity_masks = [Field25519::ZERO];
        assert!(matches!(
            identity_tape.commitments_and_openings::<SeleneSuite>(generators, &identity_masks),
            Err(FcmpNativeErrorV1::CircuitProverCommitmentIdentity)
        ));
        assert_eq!(identity_masks, [Field25519::ZERO]);
    }
    #[test]
    fn scalar_vector_owned_and_borrowed_pushes_preserve_capacity_and_clear_every_exit() {
        reset_tracking_clears();
        let mut values = ZeroizingScalarVec::<TrackingScalar>::new(1).expect("bounded vector");
        let allocation_capacity = values.values.capacity();
        let allocation_ptr = values.values.as_ptr();
        let first = SecretScalarGuard::copy_from_borrowed(&TrackingScalar(41));
        values.push_owned(first).expect("first owned value");
        assert_eq!(tracking_clears(), 1);
        assert_eq!(values.as_slice(), &[TrackingScalar(41)]);
        assert_eq!(values.values.capacity(), allocation_capacity);
        assert_eq!(values.values.as_ptr(), allocation_ptr);
        assert_eq!(
            values.push_borrowed(&TrackingScalar(43)),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(tracking_clears(), 1);
        let rejected = SecretScalarGuard::copy_from_borrowed(&TrackingScalar(47));
        assert_eq!(
            values.push_owned(rejected),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(tracking_clears(), 2);
        assert_eq!(values.values.capacity(), allocation_capacity);
        assert_eq!(values.values.as_ptr(), allocation_ptr);
        drop(values);
        assert_eq!(tracking_clears(), 3);

        reset_tracking_clears();
        let unwind = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut values = ZeroizingScalarVec::<TrackingScalar>::new(1).expect("unwind vector");
            let value = SecretScalarGuard::copy_from_borrowed(&TrackingScalar(53));
            set_tracking_clear_panic(1);
            let _ = values.push_owned(value);
        }));
        assert!(unwind.is_err());
        assert_eq!(tracking_clears(), 2);
        set_tracking_clear_panic(0);

        let source = include_str!("circuit.rs");
        let owner = source
            .split_once("impl<F: ProofScalar> ZeroizingScalarVec<F> {")
            .expect("zeroizing scalar vector")
            .1
            .split_once("impl<F: ProofScalar> Drop for ZeroizingScalarVec<F>")
            .expect("zeroizing scalar vector boundary")
            .0;
        let push_owned = owner
            .split_once("fn push_owned(")
            .expect("owned scalar push")
            .1
            .split_once("fn push_borrowed(")
            .expect("borrowed scalar push boundary")
            .0;
        for step in [
            "let allocation_capacity = self.values.capacity()",
            "let allocation_ptr = self.values.as_ptr()",
            "if self.values.len() >= self.logical_capacity",
            "self.values.push(F::ZERO)",
            "core::mem::swap(&mut self.values[destination], &mut value.0)",
            "drop(value)",
            "debug_assert_eq!(self.values.capacity(), allocation_capacity)",
            "debug_assert_eq!(self.values.as_ptr(), allocation_ptr)",
        ] {
            assert!(push_owned.contains(step), "missing owned-push step {step}");
        }
        assert!(!push_owned.contains("expose_copy"));
        let push_borrowed = owner
            .split_once("fn push_borrowed(")
            .expect("borrowed scalar push")
            .1
            .split_once("fn extend_borrowed_v1(")
            .expect("borrowed scalar push boundary")
            .0;
        let capacity = push_borrowed
            .find("if self.values.len() >= self.logical_capacity")
            .expect("borrowed capacity preflight");
        let guard = push_borrowed
            .find("let value = SecretScalarGuard::copy_from_borrowed(value)")
            .expect("direct borrowed-copy owner");
        let delegate = push_borrowed
            .find("self.push_owned(value)?")
            .expect("owned push delegation");
        assert!(capacity < guard && guard < delegate);
        assert!(!push_borrowed.contains("expose_copy"));
        assert!(!push_borrowed.contains("self.values.push("));
        assert!(!owner.contains("fn extend_from_slice("));
    }
    #[test]
    fn scalar_vector_borrowed_extension_preflights_and_clears_every_exit() {
        reset_tracking_clears();
        let source = [TrackingScalar(59), TrackingScalar(61)];
        let mut values = ZeroizingScalarVec::<TrackingScalar>::new(source.len())
            .expect("borrowed-extension vector");
        let allocation_capacity = values.values.capacity();
        let allocation_ptr = values.values.as_ptr();
        values
            .extend_borrowed_v1(&source)
            .expect("borrowed extension");
        assert_eq!(source, [TrackingScalar(59), TrackingScalar(61)]);
        assert_eq!(values.as_slice(), &source);
        assert_eq!(values.values.capacity(), allocation_capacity);
        assert_eq!(values.values.as_ptr(), allocation_ptr);
        assert_eq!(tracking_clears(), 2);
        drop(values);
        assert_eq!(tracking_clears(), 4);

        reset_tracking_clears();
        let rejected_source = [TrackingScalar(67), TrackingScalar(71)];
        let mut rejected =
            ZeroizingScalarVec::<TrackingScalar>::new(1).expect("preflight-rejection vector");
        let rejected_capacity = rejected.values.capacity();
        let rejected_ptr = rejected.values.as_ptr();
        assert_eq!(
            rejected.extend_borrowed_v1(&rejected_source),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(rejected_source, [TrackingScalar(67), TrackingScalar(71)]);
        assert!(rejected.as_slice().is_empty());
        assert_eq!(rejected.values.capacity(), rejected_capacity);
        assert_eq!(rejected.values.as_ptr(), rejected_ptr);
        assert_eq!(tracking_clears(), 0);
        drop(rejected);
        assert_eq!(tracking_clears(), 0);

        reset_tracking_clears();
        let unwind_source = [TrackingScalar(73), TrackingScalar(79)];
        let unwind = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut values = ZeroizingScalarVec::<TrackingScalar>::new(unwind_source.len())
                .expect("borrowed-extension unwind vector");
            set_tracking_clear_panic(1);
            let _ = values.extend_borrowed_v1(&unwind_source);
        }));
        assert!(unwind.is_err());
        assert_eq!(unwind_source, [TrackingScalar(73), TrackingScalar(79)]);
        assert_eq!(tracking_clears(), 2);
        set_tracking_clear_panic(0);

        let production = include_str!("circuit.rs")
            .split_once("#[cfg(test)]\nmod tests")
            .expect("production circuit boundary")
            .0;
        let scalar_vector = production
            .split_once("impl<F: ProofScalar> ZeroizingScalarVec<F> {")
            .expect("zeroizing scalar vector")
            .1
            .split_once("impl<F: ProofScalar> Drop for ZeroizingScalarVec<F>")
            .expect("zeroizing scalar vector boundary")
            .0;
        let borrowed_extension = scalar_vector
            .split_once("fn extend_borrowed_v1(")
            .expect("borrowed scalar extension")
            .1
            .split_once("fn take(")
            .expect("borrowed scalar extension boundary")
            .0;
        let mut cursor = 0;
        for step in [
            "let start_len = self.values.len()",
            "let allocation_capacity = self.values.capacity()",
            "let allocation_ptr = self.values.as_ptr()",
            "let end = start_len",
            ".checked_add(values.len())",
            "if end > self.logical_capacity || end > allocation_capacity",
            "for value in values",
            "self.push_borrowed(value)?",
            "debug_assert_eq!(self.values.len(), end)",
            "debug_assert_eq!(self.values.capacity(), allocation_capacity)",
            "debug_assert_eq!(self.values.as_ptr(), allocation_ptr)",
        ] {
            let offset = borrowed_extension[cursor..]
                .find(step)
                .unwrap_or_else(|| panic!("missing borrowed-extension step {step}"));
            cursor += offset + step.len();
        }
        assert_eq!(
            borrowed_extension
                .matches("self.push_borrowed(value)?")
                .count(),
            1
        );
        for forbidden in [
            "self.values.extend_from_slice(",
            "self.values.push(",
            "SecretScalarGuard",
            "*value",
            ".copied(",
            ".cloned(",
            ".clone(",
            "expose",
            "fn get",
            ".get(",
            "callback",
            "FnOnce",
            "FnMut",
            "Deref",
            "Clone",
        ] {
            assert!(
                !borrowed_extension.contains(forbidden),
                "retained borrowed-extension path {forbidden}"
            );
        }
    }
    #[test]
    fn dlog_coefficient_ingestion_owns_exact_counts_errors_and_unwind() {
        let ed_coefficients = zeroize::Zeroizing::new(
            (0..ED25519_DLOG_PARAMETERS.scalar_bits)
                .map(|index| u64::try_from(index % 7).expect("small Ed coefficient"))
                .collect::<Vec<_>>(),
        );
        let ed_padding_source = [TrackingScalar(61), TrackingScalar(67)];
        reset_secret_dlog_coefficient_drops_v1();
        let mut ed_tape =
            ProverVectorCommitmentTape::<TrackingScalar>::new(2 * COMMITMENT_WORD_LEN)
                .expect("Ed dlog tape");
        let (ed_dlog, ed_padding, _) = ed_tape
            .append_dlog(
                ED25519_DLOG_PARAMETERS,
                &ed_coefficients,
                &ed_padding_source,
                &TrackingScalar(73),
            )
            .expect("owned Ed dlog ingestion");
        assert_eq!(ed_dlog.len(), 253);
        assert_eq!(ed_padding.len(), 2);
        assert_eq!(secret_dlog_coefficient_drops_v1(), 253);
        let ed_values = ed_tape.values[0].as_slice();
        assert_eq!(ed_values.len(), 256);
        assert!(
            ed_values[..253]
                .iter()
                .zip(ed_coefficients.iter())
                .all(|(scalar, coefficient)| scalar.0 == *coefficient)
        );
        assert_eq!(ed_padding_source, [TrackingScalar(61), TrackingScalar(67)]);
        assert_eq!(ed_values[253], TrackingScalar(61));
        assert_eq!(ed_values[254], TrackingScalar(67));
        assert_eq!(ed_values[255], TrackingScalar(73));

        let cycle_coefficients = zeroize::Zeroizing::new(
            (0..CYCLE_DLOG_PARAMETERS.scalar_bits)
                .map(|index| u64::try_from(index % 11).expect("small cycle coefficient"))
                .collect::<Vec<_>>(),
        );
        reset_secret_dlog_coefficient_drops_v1();
        let mut cycle_tape =
            ProverVectorCommitmentTape::<TrackingScalar>::new(2 * COMMITMENT_WORD_LEN)
                .expect("cycle dlog tape");
        let (cycle_dlog, cycle_padding, _) = cycle_tape
            .append_dlog(
                CYCLE_DLOG_PARAMETERS,
                &cycle_coefficients,
                &[],
                &TrackingScalar(79),
            )
            .expect("owned cycle dlog ingestion");
        assert_eq!(cycle_dlog.len(), 255);
        assert!(cycle_padding.is_empty());
        assert_eq!(secret_dlog_coefficient_drops_v1(), 255);
        let cycle_values = cycle_tape.values[0].as_slice();
        assert_eq!(cycle_values.len(), 256);
        assert!(
            cycle_values[..255]
                .iter()
                .zip(cycle_coefficients.iter())
                .all(|(scalar, coefficient)| scalar.0 == *coefficient)
        );
        assert_eq!(cycle_values[255], TrackingScalar(79));

        reset_secret_dlog_coefficient_drops_v1();
        reset_tracking_clears();
        let rejected_cycle_padding = [TrackingScalar(83)];
        let rejected_cycle_extra = TrackingScalar(89);
        let mut rejected_cycle_tape =
            ProverVectorCommitmentTape::<TrackingScalar>::new(2 * COMMITMENT_WORD_LEN)
                .expect("rejected cycle-padding tape");
        assert!(matches!(
            rejected_cycle_tape.append_dlog(
                CYCLE_DLOG_PARAMETERS,
                &cycle_coefficients,
                &rejected_cycle_padding,
                &rejected_cycle_extra,
            ),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        ));
        assert_eq!(rejected_cycle_padding, [TrackingScalar(83)]);
        assert_eq!(rejected_cycle_extra, TrackingScalar(89));
        assert_eq!(secret_dlog_coefficient_drops_v1(), 0);
        assert_eq!(tracking_clears(), 0);
        assert!(rejected_cycle_tape.values.is_empty());

        reset_secret_dlog_coefficient_drops_v1();
        let mut rejected_tape =
            ProverVectorCommitmentTape::<TrackingScalar>::new(2 * COMMITMENT_WORD_LEN)
                .expect("rejected dlog tape");
        assert!(matches!(
            rejected_tape.append_dlog(
                ED25519_DLOG_PARAMETERS,
                &ed_coefficients[..252],
                &[],
                &TrackingScalar::ZERO,
            ),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        ));
        assert_eq!(secret_dlog_coefficient_drops_v1(), 0);
        assert!(rejected_tape.values.is_empty());

        reset_secret_dlog_coefficient_drops_v1();
        let unwind = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut tape =
                ProverVectorCommitmentTape::<TrackingScalar>::new(2 * COMMITMENT_WORD_LEN)
                    .expect("dlog unwind tape");
            set_tracking_from_u64_panic(true);
            let _ = tape.append_dlog(
                ED25519_DLOG_PARAMETERS,
                &ed_coefficients,
                &[],
                &TrackingScalar::ZERO,
            );
        }));
        assert!(unwind.is_err());
        assert!(!tracking_from_u64_will_panic());
        assert_eq!(secret_dlog_coefficient_drops_v1(), 1);

        let production = include_str!("circuit.rs")
            .split_once("#[cfg(test)]\nmod tests")
            .expect("production circuit boundary")
            .0;
        let coefficient_declaration = production
            .split_once("/// Owns one secret-derived discrete-log coefficient")
            .expect("dlog coefficient declaration")
            .1
            .split_once("impl SecretDlogCoefficientV1")
            .expect("dlog coefficient declaration boundary")
            .0;
        assert!(!coefficient_declaration.contains("#[derive"));
        let coefficient_owner = production
            .split_once("struct SecretDlogCoefficientV1")
            .expect("dlog coefficient owner")
            .1
            .split_once("/// Erases one callee-owned `Copy` scalar parameter")
            .expect("dlog coefficient owner boundary")
            .0;
        for step in [
            "fn copy_from_borrowed(value: &u64) -> Self",
            "fn into_scalar_owner_v1<F: ProofScalar>(self) -> SecretScalarGuard<F>",
            "SecretScalarGuard::new(F::from_u64(self.0))",
            "drop(self)",
            "self.0.zeroize()",
            "compiler_fence(core::sync::atomic::Ordering::SeqCst)",
            "core::hint::black_box(&mut self.0)",
        ] {
            assert!(
                coefficient_owner.contains(step),
                "missing coefficient-owner step {step}"
            );
        }
        for forbidden in [
            "derive(",
            "-> u64",
            "fn expose",
            "fn get",
            "callback",
            "FnOnce",
            "FnMut",
            "Deref",
            "Clone",
            ".clone(",
            ".copied(",
        ] {
            assert!(
                !coefficient_owner.contains(forbidden),
                "retained coefficient-owner API {forbidden}"
            );
        }
        let append_dlog = production
            .split_once("dlog: &[u64],")
            .expect("prover dlog ingestion")
            .1
            .split_once("pub(super) fn append_divisor(")
            .expect("prover dlog ingestion boundary")
            .0;
        let validation = append_dlog
            .find("parameters.validate()?")
            .expect("dlog parameter validation");
        let length = append_dlog
            .find("if dlog.len() != parameters.scalar_bits")
            .expect("dlog length validation");
        let allocation = append_dlog
            .find("let mut witness = ZeroizingScalarVec::new")
            .expect("dlog witness allocation");
        let borrow = append_dlog
            .find("for coefficient in dlog")
            .expect("borrowed dlog iteration");
        let owner = append_dlog
            .find("SecretDlogCoefficientV1::copy_from_borrowed(coefficient)")
            .expect("coefficient owner construction");
        let scalar = append_dlog
            .find("coefficient.into_scalar_owner_v1::<F>()")
            .expect("scalar owner conversion");
        let insertion = append_dlog
            .find("witness.push_owned(scalar)?")
            .expect("owned scalar insertion");
        let padding = append_dlog
            .find("witness.extend_borrowed_v1(padding)?")
            .expect("borrowed padding ingestion");
        assert!(validation < length && length < allocation && allocation < borrow);
        assert!(borrow < owner && owner < scalar && scalar < insertion && insertion < padding);
        for forbidden in [
            "dlog.iter().copied()",
            "F::from_u64(value)",
            "witness.push_borrowed(&F::from_u64",
            "witness.extend_from_slice(padding)",
            "coefficient.expose",
            "coefficient.clone",
            "callback",
            "FnOnce",
            "FnMut",
            "Deref",
            "Clone",
        ] {
            assert!(
                !append_dlog.contains(forbidden),
                "retained dlog path {forbidden}"
            );
        }
    }
    #[test]
    fn prover_tape_never_reallocates_scalar_buffers_and_clears_error_paths() {
        reset_tracking_clears();
        let source = tracking_values(2 * COMMITMENT_WORD_LEN, 1);
        assert_eq!(tracking_clears(), 2 * COMMITMENT_WORD_LEN);
        reset_tracking_clears();
        let mut tape = ProverVectorCommitmentTape::<TrackingScalar>::new(256).expect("tape");
        let outer_pointer = tape.values.as_ptr();
        let outer_allocation_capacity = tape.values_allocation_capacity;
        assert_eq!(tape.values_logical_capacity, 256);
        assert!(outer_allocation_capacity >= tape.values_logical_capacity);
        tape.append_word(&source.as_slice()[..COMMITMENT_WORD_LEN])
            .expect("first word");
        assert_eq!(tracking_clears(), COMMITMENT_WORD_LEN);
        assert_eq!(source.as_slice()[0], TrackingScalar(1));
        assert_eq!(source.as_slice()[COMMITMENT_WORD_LEN], TrackingScalar(129));
        assert_eq!(tape.values.as_ptr(), outer_pointer);
        assert_eq!(tape.values.capacity(), outer_allocation_capacity);
        let pointer = tape.values[0].as_slice().as_ptr();
        let allocation_capacity = tape.values[0].allocation_capacity;
        assert_eq!(tape.values[0].logical_capacity, 256);
        tape.append_word(&source.as_slice()[COMMITMENT_WORD_LEN..])
            .expect("second word");
        assert_eq!(tracking_clears(), 2 * COMMITMENT_WORD_LEN);
        assert_eq!(source.as_slice()[0], TrackingScalar(1));
        assert_eq!(
            source.as_slice()[2 * COMMITMENT_WORD_LEN - 1],
            TrackingScalar(256)
        );
        assert_eq!(tape.values[0].as_slice().as_ptr(), pointer);
        assert_eq!(tape.values[0].allocation_capacity, allocation_capacity);
        assert_eq!(tape.values[0].len(), 256);
        drop(tape);
        assert_eq!(tracking_clears(), 512);
        drop(source);
        assert_eq!(tracking_clears(), 768);
        reset_tracking_clears();
        let source = tracking_values(COMMITMENT_WORD_LEN, 7);
        let rejected = tracking_values(2, 99);
        assert_eq!(tracking_clears(), COMMITMENT_WORD_LEN + 2);
        reset_tracking_clears();
        let mut tape = ProverVectorCommitmentTape::<TrackingScalar>::new(256).expect("tape");
        tape.append_word(source.as_slice())
            .expect("partial commitment");
        assert_eq!(tracking_clears(), COMMITMENT_WORD_LEN);
        assert_eq!(
            tape.append_branch(rejected.as_slice()),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(tracking_clears(), COMMITMENT_WORD_LEN);
        drop(tape);
        drop(source);
        drop(rejected);
        assert_eq!(tracking_clears(), 386);
        reset_tracking_clears();
        let branch = [TrackingScalar(41)];
        let mut tape = ProverVectorCommitmentTape::<TrackingScalar>::new(128).expect("tape");
        let outer_pointer = tape.values.as_ptr();
        let outer_allocation_capacity = tape.values_allocation_capacity;
        for _ in 0..128 {
            tape.append_branch(&branch)
                .expect("branch within row bound");
            assert_eq!(tape.values.as_ptr(), outer_pointer);
            assert_eq!(tape.values.capacity(), outer_allocation_capacity);
        }
        assert_eq!(branch, [TrackingScalar(41)]);
        assert_eq!(
            tape.append_branch(&branch),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(tracking_clears(), 128);
        drop(tape);
        assert_eq!(tracking_clears(), 256);
        reset_tracking_clears();
        let unwind = panic::catch_unwind(AssertUnwindSafe(|| {
            let branch = [TrackingScalar(43), TrackingScalar(47)];
            let mut tape = ProverVectorCommitmentTape::<TrackingScalar>::new(128).expect("tape");
            tape.append_branch(&branch).expect("branch before unwind");
            panic!("exercise tape cleanup during unwind");
        }));
        assert!(unwind.is_err());
        assert_eq!(tracking_clears(), 4);
    }
    #[test]
    fn append_word_rejects_before_copy_and_clears_both_unwind_destinations() {
        reset_tracking_clears();
        let wrong_length = [TrackingScalar(101); COMMITMENT_WORD_LEN - 1];
        let mut rejected =
            ProverVectorCommitmentTape::<TrackingScalar>::new(256).expect("word-rejection tape");
        let rejected_ptr = rejected.values.as_ptr();
        let rejected_capacity = rejected.values.capacity();
        assert_eq!(
            rejected.append_word(&wrong_length),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(wrong_length, [TrackingScalar(101); COMMITMENT_WORD_LEN - 1]);
        assert_eq!(tracking_clears(), 0);
        assert!(rejected.values.is_empty());
        assert_eq!(rejected.values.as_ptr(), rejected_ptr);
        assert_eq!(rejected.values.capacity(), rejected_capacity);
        assert_eq!(rejected.layout.current_j_offset, 0);
        assert_eq!(rejected.layout.commitments, 0);
        drop(rejected);
        assert_eq!(tracking_clears(), 0);

        let inconsistent_source = [TrackingScalar(103); COMMITMENT_WORD_LEN];
        let mut inconsistent =
            ProverVectorCommitmentTape::<TrackingScalar>::new(256).expect("inconsistent word tape");
        inconsistent.layout.current_j_offset = COMMITMENT_WORD_LEN;
        inconsistent.layout.commitments = 1;
        let inconsistent_ptr = inconsistent.values.as_ptr();
        let inconsistent_capacity = inconsistent.values.capacity();
        assert_eq!(
            inconsistent.append_word(&inconsistent_source),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(
            inconsistent_source,
            [TrackingScalar(103); COMMITMENT_WORD_LEN]
        );
        assert_eq!(tracking_clears(), 0);
        assert!(inconsistent.values.is_empty());
        assert_eq!(inconsistent.values.as_ptr(), inconsistent_ptr);
        assert_eq!(inconsistent.values.capacity(), inconsistent_capacity);
        drop(inconsistent);
        assert_eq!(tracking_clears(), 0);

        let new_destination_source = [TrackingScalar(107); COMMITMENT_WORD_LEN];
        let unwind = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut tape = ProverVectorCommitmentTape::<TrackingScalar>::new(256)
                .expect("new-destination unwind tape");
            set_tracking_clear_panic(1);
            let _ = tape.append_word(&new_destination_source);
        }));
        assert!(unwind.is_err());
        assert_eq!(
            new_destination_source,
            [TrackingScalar(107); COMMITMENT_WORD_LEN]
        );
        assert_eq!(tracking_clears(), 2);
        set_tracking_clear_panic(0);

        reset_tracking_clears();
        let first_word = [TrackingScalar(109); COMMITMENT_WORD_LEN];
        let continuation_source = [TrackingScalar(113); COMMITMENT_WORD_LEN];
        let unwind = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut tape = ProverVectorCommitmentTape::<TrackingScalar>::new(256)
                .expect("continuation unwind tape");
            tape.append_word(&first_word).expect("first retained word");
            reset_tracking_clears();
            set_tracking_clear_panic(1);
            let _ = tape.append_word(&continuation_source);
        }));
        assert!(unwind.is_err());
        assert_eq!(first_word, [TrackingScalar(109); COMMITMENT_WORD_LEN]);
        assert_eq!(
            continuation_source,
            [TrackingScalar(113); COMMITMENT_WORD_LEN]
        );
        assert_eq!(tracking_clears(), 130);
        set_tracking_clear_panic(0);

        let production = include_str!("circuit.rs")
            .split_once("#[cfg(test)]\nmod tests")
            .expect("production circuit boundary")
            .0;
        let append_word = production
            .split_once("fn append_word(&mut self, values: &[F])")
            .expect("prover append-word ingestion")
            .1
            .split_once("pub(super) fn append_branch(")
            .expect("prover append-word boundary")
            .0;
        let length = append_word
            .find("if values.len() != COMMITMENT_WORD_LEN")
            .expect("word length validation");
        let capacity = append_word
            .find("if starts_commitment && !self.has_commitment_capacity()")
            .expect("word commitment-capacity preflight");
        let allocation = append_word
            .find("let mut destination = starts_commitment")
            .expect("word destination allocation");
        let layout = append_word
            .find("let variables = self.layout.append_word()?")
            .expect("word layout allocation");
        let new_destination = append_word
            .find("destination.extend_borrowed_v1(values)?")
            .expect("new-destination borrowed ingestion");
        let retained_destination = append_word
            .rfind(".extend_borrowed_v1(values)?")
            .expect("retained-destination borrowed ingestion");
        assert!(
            length < capacity
                && capacity < allocation
                && allocation < layout
                && layout < new_destination
                && new_destination < retained_destination
        );
        assert_eq!(
            append_word.matches("extend_borrowed_v1(values)?").count(),
            2
        );
        for forbidden in [
            "extend_from_slice(values)",
            "values.iter().copied",
            "values.iter().cloned",
            ".copied(",
            ".cloned(",
            ".clone(",
            "self.values.push(",
            "SecretScalarGuard",
            "expose",
            "fn get",
            ".get(",
            "callback",
            "FnOnce",
            "FnMut",
            "Deref",
            "Clone",
        ] {
            assert!(
                !append_word.contains(forbidden),
                "retained append-word path {forbidden}"
            );
        }
        let prover_tape = production
            .split_once("impl<F: ProofScalar> ProverVectorCommitmentTape<F> {")
            .expect("prover tape implementation")
            .1
            .split_once("/// Exact verifier-side arithmetic circuit")
            .expect("prover tape implementation boundary")
            .0;
        let append_branch = prover_tape
            .split_once("pub(super) fn append_branch(\n        &mut self,\n        branch: &[F],")
            .expect("prover append-branch ingestion")
            .1
            .split_once("pub(super) fn append_dlog(")
            .expect("prover append-branch boundary")
            .0;
        let capacity = append_branch
            .find("if !self.has_commitment_capacity()")
            .expect("branch commitment-capacity preflight");
        let allocation = append_branch
            .find("let mut destination = ZeroizingScalarVec::new")
            .expect("branch destination allocation");
        let layout = append_branch
            .find("let variables = self.layout.append_branch(branch.len())?")
            .expect("branch layout allocation");
        let ingestion = append_branch
            .find("destination.extend_borrowed_v1(branch)?")
            .expect("branch borrowed ingestion");
        let retention = append_branch
            .find("self.push_commitment(destination)?")
            .expect("branch destination retention");
        assert!(capacity < allocation && allocation < layout && layout < ingestion);
        assert!(ingestion < retention);
        assert_eq!(
            append_branch
                .matches("destination.extend_borrowed_v1(branch)?")
                .count(),
            1
        );
        for forbidden in [
            "extend_from_slice(branch)",
            "branch.iter().copied",
            "branch.iter().cloned",
            ".copied(",
            ".cloned(",
            ".clone(",
            "SecretScalarGuard",
            "expose",
            "fn get",
            ".get(",
            "callback",
            "FnOnce",
            "FnMut",
            "Deref",
            "Clone",
        ] {
            assert!(
                !append_branch.contains(forbidden),
                "retained append-branch path {forbidden}"
            );
        }
        let commitment_producer = prover_tape
            .split_once("pub(super) fn commitments_and_openings<S: ProofSuite<Scalar = F>>")
            .expect("commitment producer")
            .1;
        let shape = commitment_producer
            .find("if self.values.len() != self.layout.commitments")
            .expect("commitment shape preflight");
        let commitment_allocation = commitment_producer
            .find("let mut commitments = Vec::new()")
            .expect("commitment result allocation");
        let opening_allocation = commitment_producer
            .find("let mut openings = Vec::new()")
            .expect("opening result allocation");
        let mask_allocation = commitment_producer
            .find("ZeroizingScalarVec::new(masks.len())?")
            .expect("mask owner allocation");
        let ingestion = commitment_producer
            .find("owned_masks.extend_borrowed_v1(masks)?")
            .expect("borrowed mask ingestion");
        let msm = commitment_producer
            .find("for (values, mask) in self.values.iter().zip(owned_masks.as_slice())")
            .expect("commitment MSM order");
        let openings = commitment_producer
            .find("for (values, mask) in self.values.iter_mut().zip(owned_masks.values.iter_mut())")
            .expect("opening construction order");
        assert!(shape < commitment_allocation && commitment_allocation < opening_allocation);
        assert!(opening_allocation < mask_allocation && mask_allocation < ingestion);
        assert!(ingestion < msm && msm < openings);
        assert_eq!(
            commitment_producer
                .matches("owned_masks.extend_borrowed_v1(masks)?")
                .count(),
            1
        );
        for forbidden in [
            "owned_masks.extend_from_slice(masks)",
            "masks.to_vec()",
            "Vec::from(masks)",
            "copy_from_slice(masks)",
            "masks.iter().copied",
            "masks.iter().cloned",
            "owned_masks.values.push",
            "masks[index]",
            "owned_masks.as_slice()[index]",
            "VectorCommitmentOpening::new(",
        ] {
            assert!(
                !commitment_producer.contains(forbidden),
                "retained commitment mask path {forbidden}"
            );
        }
        assert_eq!(
            commitment_producer
                .matches("VectorCommitmentOpening::take_mask_from_slot(")
                .count(),
            1
        );
    }
    #[test]
    fn circuit_witness_bounds_and_eval_guard_clear_every_exit_path() {
        reset_tracking_clears();
        let source = include_str!("circuit.rs");
        let scalar_guard = source
            .split_once("impl<F: ProofScalar> SecretScalarGuard<F> {")
            .expect("secret scalar guard")
            .1
            .split_once("impl<F: ProofScalar> Drop for SecretScalarGuard<F>")
            .expect("secret scalar guard boundary")
            .0;
        assert!(scalar_guard.contains("fn new(mut value: F) -> Self"));
        assert!(scalar_guard.contains("fn copy_from_borrowed(value: &F) -> Self"));
        assert!(scalar_guard.contains("Self(*value)"));
        assert!(!scalar_guard.contains("let mut value_copy = *value;"));
        assert_eq!(scalar_guard.matches("BorrowedProofScalarSlot").count(), 1);
        let mul = source
            .split_once("pub(super) fn mul(")
            .expect("multiplication helper")
            .1
            .split_once("\n    pub(super) fn inverse(")
            .expect("inverse follows multiplication")
            .0;
        assert!(!mul.contains("let witness = match"));
        assert!(mul.contains("Some((left_witness.expose_ref(), right_witness.expose_ref()))"));
        let inverse = source
            .split_once("pub(super) fn inverse(")
            .expect("inverse helper")
            .1
            .split_once("\n    pub(super) fn inequality(")
            .expect("inequality follows inverse")
            .0;
        assert!(!inverse.contains("let value = value.expose_copy()"));
        assert!(!inverse.contains("let witness ="));
        assert!(inverse.contains("let inverse_witness = SecretScalarGuard::new"));
        assert!(inverse.contains("value_witness.expose_ref()"));
        assert!(inverse.contains("inverse_witness.expose_ref()"));
        let incomplete_add = source
            .split_once("pub(super) fn incomplete_add_fixed(")
            .expect("incomplete-add helper")
            .1
            .split_once("\n    pub(super) fn member_of_list(")
            .expect("membership helper follows incomplete add")
            .0;
        assert!(!incomplete_add.contains("let y_difference = y_difference.expose_copy()"));
        assert!(!incomplete_add.contains("let x_difference = x_difference.expose_copy()"));
        assert!(!incomplete_add.contains("let slope_witness = match"));
        assert!(incomplete_add.contains("let slope_witness = SecretScalarGuard::new"));
        assert!(incomplete_add.contains("slope_witness.expose_ref()"));
        assert!(incomplete_add.contains("x_difference.expose_ref()"));
        let divisor_eval = source
            .split_once("fn divisor_challenge_eval<")
            .expect("divisor challenge evaluator")
            .1
            .split_once("\nfn reject_hidden_dlog_pole<")
            .expect("pole check follows divisor evaluator")
            .0;
        assert!(!divisor_eval.contains("let denominator = denominator.expose_copy()"));
        assert!(!divisor_eval.contains("let numerator = numerator.expose_copy()"));
        assert!(!divisor_eval.contains("let quotient_witness = match"));
        assert!(divisor_eval.contains("let quotient_witness = SecretScalarGuard::new"));
        assert!(divisor_eval.contains("denominator.expose_ref()"));
        assert!(divisor_eval.contains("quotient_witness.expose_ref()"));
        let witness_owner = source
            .split_once("impl<F: ProofScalar> CircuitProverWitnesses<F> {")
            .expect("circuit witness owner")
            .1
            .split_once("struct CircuitProverData")
            .expect("circuit witness owner boundary")
            .0;
        let witness_api = source
            .split_once("pub(super) fn mul_with_witness(")
            .expect("borrowed circuit witness API")
            .1
            .split_once("pub(super) fn mul(")
            .expect("borrowed circuit witness API boundary")
            .0;
        assert!(witness_api.contains("witness: Option<(&S::Scalar, &S::Scalar)>"));
        assert!(!witness_api.contains("Option<(S::Scalar, S::Scalar)>"));
        let capacity_check = witness_owner
            .find("let row_capacity = self.row_capacity()?;")
            .expect("capacity preflight");
        let first_check = witness_owner
            .find("if self.len()? >= row_capacity")
            .expect("length preflight");
        let left_transfer = witness_owner
            .find("self.a_l.push_borrowed(left)?;")
            .expect("left insertion transfer");
        let right_transfer = witness_owner
            .find("self.a_r.push_borrowed(right)?;")
            .expect("right insertion transfer");
        assert!(capacity_check < first_check);
        assert!(first_check < left_transfer && left_transfer < right_transfer);
        assert!(!witness_owner.contains(".push(*left)"));
        assert!(!witness_owner.contains(".push(*right)"));
        let mut witnesses =
            CircuitProverWitnesses::<TrackingScalar>::new(2).expect("bounded witness allocation");
        let left_pointer = witnesses.a_l.as_slice().as_ptr();
        let right_pointer = witnesses.a_r.as_slice().as_ptr();
        let left_allocation_capacity = witnesses.a_l.allocation_capacity;
        let right_allocation_capacity = witnesses.a_r.allocation_capacity;
        let first = (TrackingScalar(2), TrackingScalar(3));
        witnesses.push_gate(&first.0, &first.1).expect("first gate");
        let second = (TrackingScalar(5), TrackingScalar(7));
        witnesses
            .push_gate(&second.0, &second.1)
            .expect("second gate");
        assert_eq!(witnesses.a_l.as_slice().as_ptr(), left_pointer);
        assert_eq!(witnesses.a_r.as_slice().as_ptr(), right_pointer);
        assert_eq!(witnesses.a_l.allocation_capacity, left_allocation_capacity);
        assert_eq!(witnesses.a_r.allocation_capacity, right_allocation_capacity);
        assert_eq!(witnesses.len(), Ok(2));
        assert_eq!(tracking_clears(), 4);
        let overflow = (TrackingScalar(11), TrackingScalar(13));
        assert_eq!(
            witnesses.push_gate(&overflow.0, &overflow.1),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(tracking_clears(), 4);
        reset_tracking_clears();
        drop(witnesses);
        assert_eq!(tracking_clears(), 4);
        reset_tracking_clears();
        let mut zero =
            CircuitProverWitnesses::<TrackingScalar>::new(0).expect("zero public gate bound");
        let zero_gate = (TrackingScalar(17), TrackingScalar(19));
        assert_eq!(
            zero.push_gate(&zero_gate.0, &zero_gate.1),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(zero.len(), Ok(0));
        assert_eq!(tracking_clears(), 0);
        let mut mismatched =
            CircuitProverWitnesses::<TrackingScalar>::new(1).expect("matched witness allocation");
        mismatched.a_r.logical_capacity = 0;
        let rejected = (TrackingScalar(31), TrackingScalar(37));
        assert_eq!(
            mismatched.push_gate(&rejected.0, &rejected.1),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(mismatched.a_l.len(), 0);
        assert_eq!(mismatched.a_r.len(), 0);
        assert_eq!(tracking_clears(), 0);
        let mut invalid_second_vector =
            CircuitProverWitnesses::<TrackingScalar>::new(1).expect("second-vector allocation");
        invalid_second_vector.a_r.allocation_capacity = 0;
        let rejected = (TrackingScalar(41), TrackingScalar(43));
        assert_eq!(
            invalid_second_vector.push_gate(&rejected.0, &rejected.1),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(invalid_second_vector.a_l.len(), 0);
        assert_eq!(invalid_second_vector.a_r.len(), 0);
        assert_eq!(tracking_clears(), 0);
        reset_tracking_clears();
        let unwind = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut partially_inserted = CircuitProverWitnesses::<TrackingScalar>::new(1)
                .expect("partial second-vector unwind allocation");
            let witness = (TrackingScalar(47), TrackingScalar(53));
            // The first owner clear fails after retaining the left witness and
            // before the right insertion, so unwind must erase the partial row.
            set_tracking_clear_panic(1);
            let _ = partially_inserted.push_gate(&witness.0, &witness.1);
        }));
        assert!(unwind.is_err());
        assert_eq!(tracking_clears(), 2);
        let mut witnesses =
            CircuitProverWitnesses::<TrackingScalar>::new(1).expect("single witness allocation");
        let single = (TrackingScalar(2), TrackingScalar(3));
        witnesses
            .push_gate(&single.0, &single.1)
            .expect("single witness");
        reset_tracking_clears();
        let valid = LinComb::empty()
            .constant(TrackingScalar(5))
            .term(TrackingScalar(3), Variable::aL(0));
        let result = eval_prover_lincomb(&valid, &witnesses, &[]).expect("valid evaluation");
        assert_eq!(result.expose_copy(), TrackingScalar(11));
        assert_eq!(tracking_clears(), 1);
        drop(result);
        assert_eq!(tracking_clears(), 2);
        reset_tracking_clears();
        let invalid = LinComb::empty().term(TrackingScalar::ONE, Variable::aL(1));
        assert!(matches!(
            eval_prover_lincomb(&invalid, &witnesses, &[]),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        ));
        assert_eq!(tracking_clears(), 2);
        reset_tracking_clears();
        set_tracking_add_assign_panic(true);
        let unwind = panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = eval_prover_lincomb(&valid, &witnesses, &[]);
        }));
        assert!(unwind.is_err());
        assert!(!tracking_add_assign_will_panic());
        assert_eq!(tracking_clears(), 2);
        reset_tracking_clears();
        drop(witnesses);
        assert_eq!(tracking_clears(), 2);
        reset_tracking_clears();
        let unwind = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut witnesses = CircuitProverWitnesses::<TrackingScalar>::new(1)
                .expect("single witness allocation");
            let witness = (TrackingScalar(23), TrackingScalar(29));
            witnesses
                .push_gate(&witness.0, &witness.1)
                .expect("witness before unwind");
            panic!("exercise witness cleanup during unwind");
        }));
        assert!(unwind.is_err());
        // Both borrowed-copy owners clear before the retained slots unwind.
        assert_eq!(tracking_clears(), 4);
    }
    #[test]
    fn circuit_enforces_the_public_prover_gate_bound_without_partial_growth() {
        let mut zero = Circuit::<SeleneSuite>::prove(Vec::new(), 0).expect("zero-gate circuit");
        let zero_witness = (Field25519::from_u64(2), Field25519::from_u64(3));
        assert_eq!(
            zero.mul_with_witness(None, None, Some((&zero_witness.0, &zero_witness.1))),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(zero.muls(), 0);
        let mut one = Circuit::<SeleneSuite>::prove(Vec::new(), 1).expect("one-gate circuit");
        let first_witness = (Field25519::from_u64(2), Field25519::from_u64(3));
        one.mul_with_witness(None, None, Some((&first_witness.0, &first_witness.1)))
            .expect("first gate");
        assert_eq!(one.muls(), 1);
        let overflow_witness = (Field25519::from_u64(5), Field25519::from_u64(7));
        assert_eq!(
            one.mul_with_witness(None, None, Some((&overflow_witness.0, &overflow_witness.1))),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
        assert_eq!(one.muls(), 1);
        let witnesses = &one.prover.as_ref().expect("prover data").witnesses;
        assert_eq!(witnesses.len(), Ok(1));
        assert_eq!(witnesses.row_capacity(), Ok(1));
        let generators = selene_bp_generators().reduce(128).expect("generators");
        let mismatched = Circuit::<SeleneSuite>::prove(Vec::new(), 1)
            .expect("one-row allocation before generator binding");
        assert!(matches!(
            mismatched.proving_statement(generators, Vec::new()),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        ));
    }
}
