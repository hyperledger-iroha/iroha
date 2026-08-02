//! FCMP++ arithmetic-circuit and embedded-curve gadgets.
//!
//! This is a concrete, allocation-bounded port of the circuit abstraction and
//! EC discrete-log gadgets used by the pinned FCMP++ implementation.  The
//! verifier builds the exact same constraints as the prover: hidden points
//! are proved on their embedded curve, divisor evaluations bind every
//! discrete logarithm, public rerandomizations are checked with incomplete
//! addition, and every tree layer is checked with set membership.

use zeroize::Zeroize;

use super::{
    FcmpNativeErrorV1,
    bulletproof::{
        ArithmeticCircuitStatement, ArithmeticCircuitWitness, LinComb, Variable,
        VectorCommitmentOpening,
    },
    divisor::NormalizedDivisor,
    proof_math::{
        FcmpProofRandomSource, ProofGeneratorView, ProofPoint, ProofScalar, ProofSuite,
        ProverTranscript, VerifierTranscript, multiexp,
    },
};

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
#[derive(Clone)]
pub(super) struct ProverVectorCommitmentTape<F: ProofScalar> {
    layout: VectorCommitmentTape,
    values: Vec<Vec<F>>,
}

impl<F: ProofScalar> Drop for ProverVectorCommitmentTape<F> {
    fn drop(&mut self) {
        self.values.zeroize();
    }
}

impl<F: ProofScalar> ProverVectorCommitmentTape<F> {
    pub(super) fn new(commitment_len: usize) -> Result<Self, FcmpNativeErrorV1> {
        Ok(Self {
            layout: VectorCommitmentTape::new(commitment_len)?,
            values: Vec::new(),
        })
    }

    pub(super) const fn commitment_count(&self) -> usize {
        self.layout.commitment_count()
    }

    fn append_word(&mut self, values: Vec<F>) -> Result<Vec<Variable>, FcmpNativeErrorV1> {
        if values.len() != COMMITMENT_WORD_LEN {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let starts_commitment = self.layout.current_j_offset == 0;
        let variables = self.layout.append_word()?;
        if starts_commitment {
            self.values.push(values);
        } else {
            self.values
                .last_mut()
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                .extend(values);
        }
        if self.values.len() != self.layout.commitments {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        Ok(variables)
    }

    pub(super) fn append_branch(
        &mut self,
        branch: Vec<F>,
    ) -> Result<Vec<Variable>, FcmpNativeErrorV1> {
        let variables = self.layout.append_branch(branch.len())?;
        self.values.push(branch);
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
        extra: F,
    ) -> Result<(Vec<Variable>, Vec<Variable>, Variable), FcmpNativeErrorV1> {
        parameters.validate()?;
        if dlog.len() != parameters.scalar_bits || padding.len() > 255 - parameters.scalar_bits {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let mut witness = dlog.iter().copied().map(F::from_u64).collect::<Vec<_>>();
        witness.extend_from_slice(padding);
        witness.resize(255, F::ZERO);
        witness.push(extra);
        let second = witness.split_off(COMMITMENT_WORD_LEN);
        let mut variables = self.append_word(witness)?;
        variables.extend(self.append_word(second)?);
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
        extra: F,
    ) -> Result<(Divisor, Variable), FcmpNativeErrorV1> {
        parameters.validate()?;
        if divisor.yx.len() > parameters.yx_coefficients
            || divisor.x.is_empty()
            || divisor.x.len() > parameters.x_coefficients
            || divisor.x[0] != F::ONE
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let mut witness = Vec::with_capacity(256);
        witness.push(divisor.y);
        for index in 0..parameters.yx_coefficients {
            witness.push(divisor.yx.get(index).copied().unwrap_or(F::ZERO));
        }
        for index in 1..parameters.x_coefficients {
            witness.push(divisor.x.get(index).copied().unwrap_or(F::ZERO));
        }
        witness.push(divisor.zero);
        if witness.len() > 255 {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        witness.resize(255, F::ZERO);
        witness.push(extra);
        let second = witness.split_off(COMMITMENT_WORD_LEN);
        let mut variables = self.append_word(witness)?;
        variables.extend(self.append_word(second)?);
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
        point: (F, F),
        padding: &[F],
    ) -> Result<(PointWithDlog, Vec<Variable>), FcmpNativeErrorV1> {
        let (dlog, padding, x) = self.append_dlog(parameters, dlog, padding, point.0)?;
        let (divisor, y) = self.append_divisor(parameters, divisor, point.1)?;
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
        masks: Vec<F>,
    ) -> Result<(Vec<S::Point>, Vec<VectorCommitmentOpening<F>>), FcmpNativeErrorV1> {
        if self.values.len() != self.layout.commitments || masks.len() != self.values.len() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let mut commitments = Vec::with_capacity(self.values.len());
        let mut openings = Vec::with_capacity(self.values.len());
        for (values, mask) in core::mem::take(&mut self.values).into_iter().zip(masks) {
            if values.is_empty() || values.len() > generators.g_bold.len() {
                return Err(FcmpNativeErrorV1::ArithmeticInvariant);
            }
            let mut terms = values
                .iter()
                .copied()
                .zip(generators.g_bold.iter().copied())
                .collect::<Vec<_>>();
            terms.push((mask, generators.h));
            let commitment = multiexp::<S>(&terms);
            if commitment.is_identity() {
                return Err(FcmpNativeErrorV1::CircuitProverCommitmentIdentity);
            }
            commitments.push(commitment);
            openings.push(VectorCommitmentOpening::new(values, mask));
        }
        Ok((commitments, openings))
    }
}

/// Exact verifier-side arithmetic circuit. Its multiplication count is
/// retained separately from constraints because unconstrained witness gates
/// are still part of the generalized Bulletproof vectors.
#[derive(Clone)]
pub(super) struct Circuit<S: ProofSuite> {
    muls: usize,
    constraints: Vec<LinComb<S::Scalar>>,
    prover: Option<CircuitProverData<S>>,
}

#[derive(Clone)]
struct CircuitProverData<S: ProofSuite> {
    a_l: Vec<S::Scalar>,
    a_r: Vec<S::Scalar>,
    vector_commitments: Vec<VectorCommitmentOpening<S::Scalar>>,
}

impl<S: ProofSuite> Drop for CircuitProverData<S> {
    fn drop(&mut self) {
        self.a_l.zeroize();
        self.a_r.zeroize();
        self.vector_commitments.zeroize();
    }
}

impl<S: ProofSuite> Circuit<S> {
    pub(super) fn prove(vector_commitments: Vec<VectorCommitmentOpening<S::Scalar>>) -> Self {
        Self {
            muls: 0,
            constraints: Vec::new(),
            prover: Some(CircuitProverData {
                a_l: Vec::new(),
                a_r: Vec::new(),
                vector_commitments,
            }),
        }
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

    fn eval(&self, lincomb: &LinComb<S::Scalar>) -> Result<Option<S::Scalar>, FcmpNativeErrorV1> {
        let Some(prover) = &self.prover else {
            return Ok(None);
        };
        let mut result = lincomb.c;
        for (index, weight) in &lincomb.wl {
            result += *prover
                .a_l
                .get(*index)
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                * *weight;
        }
        for (index, weight) in &lincomb.wr {
            result += *prover
                .a_r
                .get(*index)
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                * *weight;
        }
        for (index, weight) in &lincomb.wo {
            result += *prover
                .a_l
                .get(*index)
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                * *prover
                    .a_r
                    .get(*index)
                    .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                * *weight;
        }
        for (commitment, weights) in lincomb.wcg.iter().enumerate() {
            let values = &prover
                .vector_commitments
                .get(commitment)
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                .values;
            for (index, weight) in weights {
                result += *values
                    .0
                    .get(*index)
                    .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?
                    * *weight;
            }
        }
        if !lincomb.wv.is_empty() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        Ok(Some(result))
    }

    pub(super) fn mul_with_witness(
        &mut self,
        left: Option<LinComb<S::Scalar>>,
        right: Option<LinComb<S::Scalar>>,
        witness: Option<(S::Scalar, S::Scalar)>,
    ) -> Result<(Variable, Variable, Variable), FcmpNativeErrorV1> {
        if self.prover.is_some() != witness.is_some() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let index = self.muls;
        self.muls = self
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
            prover.a_l.push(left);
            prover.a_r.push(right);
        }
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
        let witness = match (&left, &right) {
            (Some(left), Some(right)) => match (self.eval(left)?, self.eval(right)?) {
                (Some(left), Some(right)) => Some((left, right)),
                (None, None) => None,
                _ => return Err(FcmpNativeErrorV1::ArithmeticInvariant),
            },
            _ if self.prover.is_none() => None,
            _ => return Err(FcmpNativeErrorV1::ArithmeticInvariant),
        };
        self.mul_with_witness(left, right, witness)
    }

    pub(super) fn inverse(
        &mut self,
        value: Option<LinComb<S::Scalar>>,
    ) -> Result<(Variable, Variable), FcmpNativeErrorV1> {
        let witness = match value.as_ref() {
            Some(value) => self
                .eval(value)?
                .map(|value| {
                    value
                        .invert()
                        .map(|inverse| (value, inverse))
                        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)
                })
                .transpose()?,
            None if self.prover.is_none() => None,
            None => return Err(FcmpNativeErrorV1::ArithmeticInvariant),
        };
        let (l, r, o) = self.mul_with_witness(value, None, witness)?;
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
        let slope_witness = match (
            self.eval(&LinComb::from(y1).constant(-y0))?,
            self.eval(&x1_minus_x0)?,
        ) {
            (Some(y_difference), Some(x_difference)) => Some((
                y_difference
                    * x_difference
                        .invert()
                        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
                x_difference,
            )),
            (None, None) => None,
            _ => return Err(FcmpNativeErrorV1::ArithmeticInvariant),
        };
        let (slope, _, product) = self.mul_with_witness(None, Some(x1_minus_x0), slope_witness)?;
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
        ArithmeticCircuitStatement::new(
            generators,
            self.constraints,
            vector_commitments,
            Vec::new(),
        )
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
        if prover.a_l.len() != self.muls || prover.a_r.len() != self.muls {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let statement = ArithmeticCircuitStatement::new(
            generators,
            self.constraints,
            vector_commitments,
            Vec::new(),
        )?;
        let witness = ArithmeticCircuitWitness::new(
            core::mem::take(&mut prover.a_l),
            core::mem::take(&mut prover.a_r),
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
    #[cfg_attr(not(test), allow(dead_code))]
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
    let quotient_witness = match (circuit.eval(&p_d)?, circuit.eval(&p_n)?) {
        (Some(denominator), Some(numerator)) => Some((
            denominator,
            numerator
                * denominator
                    .invert()
                    .ok_or(FcmpNativeErrorV1::DlogWitnessPole)?,
        )),
        (None, None) => None,
        _ => return Err(FcmpNativeErrorV1::ArithmeticInvariant),
    };
    let (_, quotient, numerator_claim) =
        circuit.mul_with_witness(Some(p_d), None, quotient_witness)?;
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
        if !matches!(variable, Variable::CG { .. } | Variable::V(_)) {
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
    reject_hidden_dlog_pole(circuit.eval(&output_interpolation)?)?;
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
    use rand_08::{SeedableRng as _, rngs::StdRng};

    use super::*;
    use crate::privacy_engines::fcmp_plus_plus::{
        FCMP_LAYER_ONE_LEN_V1, FCMP_LAYER_TWO_LEN_V1,
        field::Field25519,
        proof_math::{HeliosSuite, SeleneSuite, selene_bp_generators},
    };

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
        let first = tape
            .append_branch(vec![Field25519::from_u64(3), Field25519::from_u64(4)])
            .expect("first commitment");
        let second = tape
            .append_branch(vec![Field25519::from_u64(9)])
            .expect("second commitment");
        let masks = vec![Field25519::from_u64(5), Field25519::from_u64(7)];
        let (commitments, openings) = tape
            .commitments_and_openings::<SeleneSuite>(generators, masks)
            .expect("commitments");

        let mut circuit = Circuit::<SeleneSuite>::prove(openings);
        let (_, _, product) = circuit
            .mul(Some(LinComb::from(first[0])), Some(LinComb::from(first[1])))
            .expect("multiplication");
        circuit.constrain_equal_to_zero(LinComb::from(product).constant(-Field25519::from_u64(12)));
        circuit
            .constrain_equal_to_zero(LinComb::from(second[0]).constant(-Field25519::from_u64(9)));
        let (statement, witness) = circuit
            .proving_statement(generators, commitments.clone())
            .expect("proving statement");
        let mut transcript = ProverTranscript::new(context);
        transcript.write_commitments::<SeleneSuite>(commitments, Vec::new());
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
    }
}
