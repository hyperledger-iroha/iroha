//! Concrete generalized Bulletproof arithmetic-circuit verifier.
//!
//! This is a direct concrete port of the two protocols used by the pinned
//! `generalized-bulletproofs` implementation. Independent equations are
//! checked separately instead of probabilistically batched, which removes
//! verifier randomness from consensus without permitting cross-equation
//! cancellation.

use std::ops::{Add, Index, IndexMut, Mul, Sub};

use rand_core_06::{CryptoRng, RngCore};
use zeroize::Zeroize;

use super::{
    FcmpNativeErrorV1,
    proof_math::{
        BatchVerifier, ProofGeneratorView, ProofPoint, ProofScalar, ProofSuite, ProverTranscript,
        VerifierTranscript, multiexp,
    },
};

const MAX_PROVER_SCALAR_ATTEMPTS_V1: usize = 128;

fn random_scalar<F: ProofScalar>(
    rng: &mut (impl RngCore + CryptoRng),
) -> Result<F, FcmpNativeErrorV1> {
    for _ in 0..MAX_PROVER_SCALAR_ATTEMPTS_V1 {
        if let Some(scalar) = F::random(rng)? {
            return Ok(scalar);
        }
    }
    Err(FcmpNativeErrorV1::ProverRandomnessExhausted)
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ScalarVector<F: ProofScalar>(pub(super) Vec<F>);

impl<F: ProofScalar> Zeroize for ScalarVector<F> {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl<F: ProofScalar> Drop for ScalarVector<F> {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl<F: ProofScalar> Index<usize> for ScalarVector<F> {
    type Output = F;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<F: ProofScalar> IndexMut<usize> for ScalarVector<F> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl<F: ProofScalar> Add<F> for ScalarVector<F> {
    type Output = Self;
    fn add(mut self, scalar: F) -> Self {
        for value in &mut self.0 {
            *value += scalar;
        }
        self
    }
}

impl<F: ProofScalar> Sub<F> for ScalarVector<F> {
    type Output = Self;
    fn sub(mut self, scalar: F) -> Self {
        for value in &mut self.0 {
            *value -= scalar;
        }
        self
    }
}

impl<F: ProofScalar> Mul<F> for ScalarVector<F> {
    type Output = Self;
    fn mul(mut self, scalar: F) -> Self {
        for value in &mut self.0 {
            *value *= scalar;
        }
        self
    }
}

impl<F: ProofScalar> Add<&Self> for ScalarVector<F> {
    type Output = Self;
    fn add(mut self, other: &Self) -> Self {
        assert_eq!(self.len(), other.len());
        for (value, other) in self.0.iter_mut().zip(&other.0) {
            *value += *other;
        }
        self
    }
}

impl<F: ProofScalar> Sub<&Self> for ScalarVector<F> {
    type Output = Self;
    fn sub(mut self, other: &Self) -> Self {
        assert_eq!(self.len(), other.len());
        for (value, other) in self.0.iter_mut().zip(&other.0) {
            *value -= *other;
        }
        self
    }
}

impl<F: ProofScalar> Mul<&Self> for ScalarVector<F> {
    type Output = Self;
    fn mul(mut self, other: &Self) -> Self {
        assert_eq!(self.len(), other.len());
        for (value, other) in self.0.iter_mut().zip(&other.0) {
            *value *= *other;
        }
        self
    }
}

impl<F: ProofScalar> ScalarVector<F> {
    pub(super) fn zero(len: usize) -> Self {
        Self(vec![F::ZERO; len])
    }

    pub(super) fn powers(value: F, len: usize) -> Self {
        assert!(len != 0);
        let mut result = Vec::with_capacity(len);
        result.push(F::ONE);
        if len > 1 {
            result.push(value);
        }
        for index in 2..len {
            result.push(result[index - 1] * value);
        }
        Self(result)
    }

    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    pub(super) fn inner_product(&self, vector: impl Iterator<Item = F>) -> F {
        let mut count = 0;
        let mut result = F::ZERO;
        for (left, right) in self.0.iter().zip(vector) {
            result += *left * right;
            count += 1;
        }
        assert_eq!(count, self.len());
        result
    }

    fn split(mut self) -> Result<(Self, Self), FcmpNativeErrorV1> {
        if self.len() <= 1 || self.len() % 2 != 0 {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let right = self.0.split_off(self.len() / 2);
        Ok((self, Self(right)))
    }
}

/// Opening of one Pedersen vector commitment used by the FCMP circuit.
#[derive(Clone)]
pub(super) struct VectorCommitmentOpening<F: ProofScalar> {
    pub(super) values: ScalarVector<F>,
    pub(super) mask: F,
}

impl<F: ProofScalar> Zeroize for VectorCommitmentOpening<F> {
    fn zeroize(&mut self) {
        self.values.zeroize();
        self.mask.zeroize();
    }
}

impl<F: ProofScalar> Drop for VectorCommitmentOpening<F> {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl<F: ProofScalar> VectorCommitmentOpening<F> {
    pub(super) fn new(values: Vec<F>, mask: F) -> Self {
        Self {
            values: ScalarVector(values),
            mask,
        }
    }
}

/// Witness for a concrete generalized-Bulletproof arithmetic circuit.
#[derive(Clone)]
pub(super) struct ArithmeticCircuitWitness<S: ProofSuite> {
    a_l: ScalarVector<S::Scalar>,
    a_r: ScalarVector<S::Scalar>,
    a_o: ScalarVector<S::Scalar>,
    vector_commitments: Vec<VectorCommitmentOpening<S::Scalar>>,
}

impl<S: ProofSuite> ArithmeticCircuitWitness<S> {
    pub(super) fn new(
        a_l: Vec<S::Scalar>,
        a_r: Vec<S::Scalar>,
        vector_commitments: Vec<VectorCommitmentOpening<S::Scalar>>,
    ) -> Result<Self, FcmpNativeErrorV1> {
        let a_l = ScalarVector(a_l);
        let a_r = ScalarVector(a_r);
        if a_l.len() != a_r.len() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        let a_o = a_l.clone() * &a_r;
        Ok(Self {
            a_l,
            a_r,
            a_o,
            vector_commitments,
        })
    }
}

/// One constrainable circuit variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub(super) enum Variable {
    aL(usize),
    aR(usize),
    aO(usize),
    CG {
        commitment: usize,
        index: usize,
    },
    #[cfg_attr(not(test), allow(dead_code))]
    V(usize),
}

/// Sparse generalized-Bulletproof linear combination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LinComb<F: ProofScalar> {
    pub(super) highest_a_index: Option<usize>,
    pub(super) highest_c_index: Option<usize>,
    pub(super) highest_v_index: Option<usize>,
    pub(super) wl: Vec<(usize, F)>,
    pub(super) wr: Vec<(usize, F)>,
    pub(super) wo: Vec<(usize, F)>,
    pub(super) wcg: Vec<Vec<(usize, F)>>,
    pub(super) wv: Vec<(usize, F)>,
    pub(super) c: F,
}

impl<F: ProofScalar> From<Variable> for LinComb<F> {
    fn from(variable: Variable) -> Self {
        Self::empty().term(F::ONE, variable)
    }
}

impl<F: ProofScalar> Add<&Self> for LinComb<F> {
    type Output = Self;
    fn add(mut self, other: &Self) -> Self {
        self.highest_a_index = self.highest_a_index.max(other.highest_a_index);
        self.highest_c_index = self.highest_c_index.max(other.highest_c_index);
        self.highest_v_index = self.highest_v_index.max(other.highest_v_index);
        self.wl.extend(&other.wl);
        self.wr.extend(&other.wr);
        self.wo.extend(&other.wo);
        self.wcg
            .resize_with(self.wcg.len().max(other.wcg.len()), Vec::new);
        for (ours, theirs) in self.wcg.iter_mut().zip(&other.wcg) {
            ours.extend(theirs);
        }
        self.wv.extend(&other.wv);
        self.c += other.c;
        self
    }
}

impl<F: ProofScalar> Sub<&Self> for LinComb<F> {
    type Output = Self;
    fn sub(mut self, other: &Self) -> Self {
        self.highest_a_index = self.highest_a_index.max(other.highest_a_index);
        self.highest_c_index = self.highest_c_index.max(other.highest_c_index);
        self.highest_v_index = self.highest_v_index.max(other.highest_v_index);
        self.wl
            .extend(other.wl.iter().map(|(index, weight)| (*index, -*weight)));
        self.wr
            .extend(other.wr.iter().map(|(index, weight)| (*index, -*weight)));
        self.wo
            .extend(other.wo.iter().map(|(index, weight)| (*index, -*weight)));
        self.wcg
            .resize_with(self.wcg.len().max(other.wcg.len()), Vec::new);
        for (ours, theirs) in self.wcg.iter_mut().zip(&other.wcg) {
            ours.extend(theirs.iter().map(|(index, weight)| (*index, -*weight)));
        }
        self.wv
            .extend(other.wv.iter().map(|(index, weight)| (*index, -*weight)));
        self.c -= other.c;
        self
    }
}

impl<F: ProofScalar> Mul<F> for LinComb<F> {
    type Output = Self;
    fn mul(mut self, scalar: F) -> Self {
        for (_, weight) in &mut self.wl {
            *weight *= scalar;
        }
        for (_, weight) in &mut self.wr {
            *weight *= scalar;
        }
        for (_, weight) in &mut self.wo {
            *weight *= scalar;
        }
        for commitment in &mut self.wcg {
            for (_, weight) in commitment {
                *weight *= scalar;
            }
        }
        for (_, weight) in &mut self.wv {
            *weight *= scalar;
        }
        self.c *= scalar;
        self
    }
}

impl<F: ProofScalar> LinComb<F> {
    pub(super) fn empty() -> Self {
        Self {
            highest_a_index: None,
            highest_c_index: None,
            highest_v_index: None,
            wl: Vec::new(),
            wr: Vec::new(),
            wo: Vec::new(),
            wcg: Vec::new(),
            wv: Vec::new(),
            c: F::ZERO,
        }
    }

    pub(super) fn term(mut self, scalar: F, variable: Variable) -> Self {
        match variable {
            Variable::aL(index) => {
                self.highest_a_index = self.highest_a_index.max(Some(index));
                self.wl.push((index, scalar));
            }
            Variable::aR(index) => {
                self.highest_a_index = self.highest_a_index.max(Some(index));
                self.wr.push((index, scalar));
            }
            Variable::aO(index) => {
                self.highest_a_index = self.highest_a_index.max(Some(index));
                self.wo.push((index, scalar));
            }
            Variable::CG { commitment, index } => {
                self.highest_c_index = self.highest_c_index.max(Some(commitment));
                self.highest_a_index = self.highest_a_index.max(Some(index));
                if self.wcg.len() <= commitment {
                    self.wcg.resize_with(commitment + 1, Vec::new);
                }
                self.wcg[commitment].push((index, scalar));
            }
            Variable::V(index) => {
                self.highest_v_index = self.highest_v_index.max(Some(index));
                self.wv.push((index, scalar));
            }
        }
        self
    }

    pub(super) fn constant(mut self, scalar: F) -> Self {
        self.c += scalar;
        self
    }
}

fn accumulate<F: ProofScalar>(accumulator: &mut ScalarVector<F>, values: &[(usize, F)], weight: F) {
    for (index, coefficient) in values {
        accumulator[*index] += *coefficient * weight;
    }
}

#[derive(Clone, Debug)]
pub(super) struct ArithmeticCircuitStatement<'a, S: ProofSuite> {
    generators: ProofGeneratorView<'a, S>,
    constraints: Vec<LinComb<S::Scalar>>,
    vector_commitments: Vec<S::Point>,
    scalar_commitments: Vec<S::Point>,
}

impl<'a, S: ProofSuite> ArithmeticCircuitStatement<'a, S> {
    pub(super) fn new(
        generators: ProofGeneratorView<'a, S>,
        constraints: Vec<LinComb<S::Scalar>>,
        vector_commitments: Vec<S::Point>,
        scalar_commitments: Vec<S::Point>,
    ) -> Result<Self, FcmpNativeErrorV1> {
        for constraint in &constraints {
            if Some(generators.g_bold.len()) <= constraint.highest_a_index
                || Some(vector_commitments.len()) <= constraint.highest_c_index
                || Some(scalar_commitments.len()) <= constraint.highest_v_index
            {
                return Err(FcmpNativeErrorV1::ArithmeticInvariant);
            }
        }
        Ok(Self {
            generators,
            constraints,
            vector_commitments,
            scalar_commitments,
        })
    }

    fn yz_challenges(
        &self,
        y: S::Scalar,
        z_one: S::Scalar,
    ) -> Result<(ScalarVector<S::Scalar>, ScalarVector<S::Scalar>), FcmpNativeErrorV1> {
        let y_inverse = y.invert().ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let y_inverse = ScalarVector::powers(y_inverse, self.generators.g_bold.len());
        let mut z = Vec::with_capacity(self.constraints.len());
        if !self.constraints.is_empty() {
            z.push(z_one);
            for index in 1..self.constraints.len() {
                z.push(z[index - 1] * z_one);
            }
        }
        Ok((y_inverse, ScalarVector(z)))
    }

    pub(super) fn prove(
        self,
        rng: &mut (impl RngCore + CryptoRng),
        transcript: &mut ProverTranscript,
        mut witness: ArithmeticCircuitWitness<S>,
    ) -> Result<(), FcmpNativeErrorV1> {
        let n = self.generators.g_bold.len();
        let commitment_count = self.vector_commitments.len();
        if witness.a_l.len() > n
            || witness.a_l.len() != witness.a_r.len()
            || witness.vector_commitments.len() != commitment_count
            || !self.scalar_commitments.is_empty()
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        while witness.a_l.len() < n {
            witness.a_l.0.push(S::Scalar::ZERO);
            witness.a_r.0.push(S::Scalar::ZERO);
            witness.a_o.0.push(S::Scalar::ZERO);
        }
        for opening in &mut witness.vector_commitments {
            if opening.values.len() > n {
                return Err(FcmpNativeErrorV1::ArithmeticInvariant);
            }
            opening.values.0.resize(n, S::Scalar::ZERO);
        }

        // Validate every opening and every circuit constraint before emitting
        // any proof bytes. A malformed native witness is an API error, never a
        // source of a knowingly-invalid proof.
        for (commitment, opening) in self
            .vector_commitments
            .iter()
            .zip(&witness.vector_commitments)
        {
            let mut terms = opening
                .values
                .0
                .iter()
                .copied()
                .zip(self.generators.g_bold.iter().copied())
                .collect::<Vec<_>>();
            terms.push((opening.mask, self.generators.h));
            if multiexp::<S>(&terms) != *commitment {
                return Err(FcmpNativeErrorV1::ArithmeticInvariant);
            }
        }
        for constraint in &self.constraints {
            let mut evaluation = constraint.c;
            for (index, weight) in &constraint.wl {
                evaluation += witness.a_l[*index] * *weight;
            }
            for (index, weight) in &constraint.wr {
                evaluation += witness.a_r[*index] * *weight;
            }
            for (index, weight) in &constraint.wo {
                evaluation += witness.a_o[*index] * *weight;
            }
            for (commitment, weights) in constraint.wcg.iter().enumerate() {
                for (index, weight) in weights {
                    evaluation += witness.vector_commitments[commitment].values[*index] * *weight;
                }
            }
            if !constraint.wv.is_empty() || !evaluation.is_zero() {
                return Err(FcmpNativeErrorV1::ArithmeticInvariant);
            }
        }

        let alpha = random_scalar::<S::Scalar>(rng)?;
        let beta = random_scalar::<S::Scalar>(rng)?;
        let rho = random_scalar::<S::Scalar>(rng)?;
        let ai = {
            let mut terms = Vec::with_capacity((2 * n) + 1);
            terms.extend(
                witness
                    .a_l
                    .0
                    .iter()
                    .copied()
                    .zip(self.generators.g_bold.iter().copied()),
            );
            terms.extend(
                witness
                    .a_r
                    .0
                    .iter()
                    .copied()
                    .zip(self.generators.h_bold.iter().copied()),
            );
            terms.push((alpha, self.generators.h));
            multiexp::<S>(&terms)
        };
        let ao = {
            let mut terms = witness
                .a_o
                .0
                .iter()
                .copied()
                .zip(self.generators.g_bold.iter().copied())
                .collect::<Vec<_>>();
            terms.push((beta, self.generators.h));
            multiexp::<S>(&terms)
        };
        let s_l = ScalarVector(
            (0..n)
                .map(|_| random_scalar::<S::Scalar>(rng))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let s_r = ScalarVector(
            (0..n)
                .map(|_| random_scalar::<S::Scalar>(rng))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let s_point = {
            let mut terms = Vec::with_capacity((2 * n) + 1);
            terms.extend(
                s_l.0
                    .iter()
                    .copied()
                    .zip(self.generators.g_bold.iter().copied()),
            );
            terms.extend(
                s_r.0
                    .iter()
                    .copied()
                    .zip(self.generators.h_bold.iter().copied()),
            );
            terms.push((rho, self.generators.h));
            multiexp::<S>(&terms)
        };
        if ai.is_identity() || ao.is_identity() || s_point.is_identity() {
            return Err(FcmpNativeErrorV1::CircuitProverCommitmentIdentity);
        }
        transcript.push_point(ai);
        transcript.push_point(ao);
        transcript.push_point(s_point);
        let y = transcript.challenge::<S>()?;
        let z_one = transcript.challenge::<S>()?;
        let (y_inverse, z) = self.yz_challenges(y, z_one)?;
        let y_powers = ScalarVector::powers(y, n);

        let ni = 2 + (2 * (commitment_count / 2));
        let ilr = ni / 2;
        let io = ni;
        let is = ni + 1;
        let jlr = ni / 2;
        let jo = 0;
        let js = ni + 1;
        let mut l = vec![ScalarVector(Vec::new()); is + 1];
        let mut r = vec![ScalarVector(Vec::new()); is + 1];

        let mut l_weights = ScalarVector::zero(n);
        let mut r_weights = ScalarVector::zero(n);
        let mut o_weights = ScalarVector::zero(n);
        for (constraint, z) in self.constraints.iter().zip(&z.0) {
            accumulate(&mut l_weights, &constraint.wl, *z);
            accumulate(&mut r_weights, &constraint.wr, *z);
            accumulate(&mut o_weights, &constraint.wo, *z);
        }
        l[ilr] = (r_weights * &y_inverse) + &witness.a_l;
        l[io] = witness.a_o.clone();
        l[is] = s_l;
        r[jlr] = l_weights + &(witness.a_r.clone() * &y_powers);
        r[jo] = o_weights - &y_powers;
        r[js] = s_r * &y_powers;
        for coefficient in &mut l {
            if coefficient.0.is_empty() {
                *coefficient = ScalarVector::zero(n);
            } else if coefficient.len() != n {
                return Err(FcmpNativeErrorV1::ArithmeticInvariant);
            }
        }
        for coefficient in &mut r {
            if coefficient.0.is_empty() {
                *coefficient = ScalarVector::zero(n);
            } else if coefficient.len() != n {
                return Err(FcmpNativeErrorV1::ArithmeticInvariant);
            }
        }

        let mut cg_weights = Vec::with_capacity(commitment_count);
        for commitment in 0..commitment_count {
            let mut weights = ScalarVector::zero(n);
            for (constraint, z) in self.constraints.iter().zip(&z.0) {
                if let Some(values) = constraint.wcg.get(commitment) {
                    accumulate(&mut weights, values, *z);
                }
            }
            cg_weights.push(weights);
        }
        for (mut index, (opening, weights)) in witness
            .vector_commitments
            .iter()
            .zip(cg_weights)
            .enumerate()
        {
            if index >= ilr {
                index += 1;
            }
            let reverse = ni - index;
            l[index] = opening.values.clone();
            r[reverse] = weights;
        }

        let t_poly_len = 1 + (2 * (l.len() - 1));
        let mut t = ScalarVector::zero(t_poly_len);
        for (left_index, left) in l.iter().enumerate() {
            for (right_index, right) in r.iter().enumerate() {
                t[left_index + right_index] += left.inner_product(right.0.iter().copied());
            }
        }
        let tau_before = (0..ni)
            .map(|_| random_scalar::<S::Scalar>(rng))
            .collect::<Result<Vec<_>, _>>()?;
        let tau_after = (0..t_poly_len - ni - 1)
            .map(|_| random_scalar::<S::Scalar>(rng))
            .collect::<Result<Vec<_>, _>>()?;
        for (coefficient, mask) in t.0[..ni].iter().zip(&tau_before) {
            let commitment = multiexp::<S>(&[
                (*coefficient, self.generators.g),
                (*mask, self.generators.h),
            ]);
            if commitment.is_identity() {
                return Err(FcmpNativeErrorV1::CircuitProverCommitmentIdentity);
            }
            transcript.push_point(commitment);
        }
        for (coefficient, mask) in t.0[ni + 1..].iter().zip(&tau_after) {
            let commitment = multiexp::<S>(&[
                (*coefficient, self.generators.g),
                (*mask, self.generators.h),
            ]);
            if commitment.is_identity() {
                return Err(FcmpNativeErrorV1::CircuitProverCommitmentIdentity);
            }
            transcript.push_point(commitment);
        }
        let x = ScalarVector::powers(transcript.challenge::<S>()?, t_poly_len);
        let evaluate = |polynomial: &[ScalarVector<S::Scalar>]| {
            let mut result = ScalarVector::zero(n);
            for (index, coefficient) in polynomial.iter().enumerate() {
                result = result + &(coefficient.clone() * x[index]);
            }
            result
        };
        let l_eval = evaluate(&l);
        let r_eval = evaluate(&r);
        let t_caret = l_eval.inner_product(r_eval.0.iter().copied());

        // FCMP does not use scalar commitments, so the omitted t[ni] mask is
        // zero. Vector-commitment masks instead contribute to `u` below.
        let mut tau_x_poly = tau_before;
        tau_x_poly.push(S::Scalar::ZERO);
        tau_x_poly.extend(tau_after);
        let tau_x = tau_x_poly
            .into_iter()
            .enumerate()
            .fold(S::Scalar::ZERO, |sum, (index, coefficient)| {
                sum + (coefficient * x[index])
            });
        let mut u = (alpha * x[ilr]) + (beta * x[io]) + (rho * x[is]);
        for (mut index, opening) in witness.vector_commitments.iter().enumerate() {
            if index >= ilr {
                index += 1;
            }
            u += x[index] * opening.mask;
        }

        let mut p_terms = Vec::with_capacity(1 + (2 * n));
        for (index, (left, right)) in l_eval.0.iter().zip(&r_eval.0).enumerate() {
            p_terms.push((*left, self.generators.g_bold[index]));
            p_terms.push((y_inverse[index] * *right, self.generators.h_bold[index]));
        }
        transcript.push_scalar(tau_x);
        transcript.push_scalar(u);
        transcript.push_scalar(t_caret);
        let ip_x = transcript.challenge::<S>()?;
        p_terms.push((ip_x * t_caret, self.generators.g));
        let p = multiexp::<S>(&p_terms);
        prove_inner_product::<S>(
            self.generators,
            y_inverse,
            ip_x,
            p,
            l_eval,
            r_eval,
            transcript,
        )
    }

    pub(super) fn verify(
        self,
        transcript: &mut VerifierTranscript<'_>,
    ) -> Result<(), FcmpNativeErrorV1> {
        let n = self.generators.g_bold.len();
        let commitment_count = self.vector_commitments.len();
        let ni = 2 + (2 * (commitment_count / 2));
        let ilr = ni / 2;
        let io = ni;
        let is = ni + 1;
        let jlr = ni / 2;
        let l_r_poly_len = ni + 2;
        let t_poly_len = (2 * l_r_poly_len) - 1;

        let ai = transcript.read_point::<S::Point>()?;
        let ao = transcript.read_point::<S::Point>()?;
        let s_point = transcript.read_point::<S::Point>()?;
        let y = transcript.challenge::<S>()?;
        let z_one = transcript.challenge::<S>()?;
        let (y_inverse, z) = self.yz_challenges(y, z_one)?;

        let mut l_weights = ScalarVector::zero(n);
        let mut r_weights = ScalarVector::zero(n);
        let mut o_weights = ScalarVector::zero(n);
        for (constraint, z) in self.constraints.iter().zip(&z.0) {
            accumulate(&mut l_weights, &constraint.wl, *z);
            accumulate(&mut r_weights, &constraint.wr, *z);
            accumulate(&mut o_weights, &constraint.wo, *z);
        }
        let r_weights = r_weights * &y_inverse;
        let delta = r_weights.inner_product(l_weights.0.iter().copied());

        let mut t_before = Vec::with_capacity(ni);
        for _ in 0..ni {
            t_before.push(transcript.read_point::<S::Point>()?);
        }
        let mut t_after = Vec::with_capacity(t_poly_len - ni - 1);
        for _ in 0..(t_poly_len - ni - 1) {
            t_after.push(transcript.read_point::<S::Point>()?);
        }
        let x = ScalarVector::powers(transcript.challenge::<S>()?, t_poly_len);
        let tau_x = transcript.read_scalar::<S::Scalar>()?;
        let u = transcript.read_scalar::<S::Scalar>()?;
        let t_caret = transcript.read_scalar::<S::Scalar>()?;

        // Check the polynomial commitment equation independently.
        let mut polynomial = BatchVerifier::<S>::new();
        polynomial.g += t_caret;
        polynomial.h += tau_x;
        let mut v_weights = ScalarVector::zero(self.scalar_commitments.len());
        for (constraint, z) in self.constraints.iter().zip(&z.0) {
            accumulate(&mut v_weights, &constraint.wv, -*z);
        }
        v_weights = v_weights * x[ni];
        polynomial.g -= x[ni]
            * (delta - z.inner_product(self.constraints.iter().map(|constraint| constraint.c)));
        polynomial.additional.extend(
            v_weights
                .0
                .iter()
                .copied()
                .zip(self.scalar_commitments.iter().copied())
                .map(|(weight, point)| (-weight, point)),
        );
        polynomial.additional.extend(
            t_before
                .into_iter()
                .enumerate()
                .map(|(index, point)| (-x[index], point)),
        );
        polynomial.additional.extend(
            t_after
                .into_iter()
                .enumerate()
                .map(|(index, point)| (-x[ni + 1 + index], point)),
        );
        if !polynomial.verify() {
            return Err(FcmpNativeErrorV1::CircuitEquation);
        }

        // Build P and verify the inner-product equation independently.
        let mut ipa = BatchVerifier::<S>::new();
        ipa.ensure_len(n);
        ipa.additional.push((x[ilr], ai));
        ipa.additional.push((x[io], ao));
        let log_n = n.trailing_zeros() as usize;
        if !n.is_power_of_two() || log_n >= ipa.h_sum.len() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        ipa.h_sum[log_n] -= S::Scalar::ONE;
        ipa.additional.push((x[is], s_point));

        let mut h_bold_scalars = l_weights * x[jlr];
        for (index, weight) in (r_weights * x[jlr]).0.iter().copied().enumerate() {
            ipa.g_bold[index] += weight;
        }
        h_bold_scalars = h_bold_scalars + &(o_weights * S::Scalar::ONE);

        let mut cg_weights = Vec::with_capacity(self.vector_commitments.len());
        for commitment in 0..self.vector_commitments.len() {
            let mut weights = ScalarVector::zero(n);
            for (constraint, z) in self.constraints.iter().zip(&z.0) {
                if let Some(values) = constraint.wcg.get(commitment) {
                    accumulate(&mut weights, values, *z);
                }
            }
            cg_weights.push(weights);
        }
        for (mut index, (commitment, weights)) in self
            .vector_commitments
            .iter()
            .copied()
            .zip(cg_weights)
            .enumerate()
        {
            if index >= ni / 2 {
                index += 1;
            }
            let reverse = ni - index;
            ipa.additional.push((x[index], commitment));
            h_bold_scalars = h_bold_scalars + &(weights * x[reverse]);
        }
        h_bold_scalars = h_bold_scalars * &y_inverse;
        for (index, scalar) in h_bold_scalars.0.iter().copied().enumerate() {
            ipa.h_bold[index] += scalar;
        }
        ipa.h -= u;

        let ip_x = transcript.challenge::<S>()?;
        ipa.g += ip_x * t_caret;
        verify_inner_product::<S>(self.generators, y_inverse, ip_x, &mut ipa, transcript)?;
        if !ipa.verify() {
            return Err(FcmpNativeErrorV1::CircuitEquation);
        }
        Ok(())
    }
}

fn prove_inner_product<S: ProofSuite>(
    generators: ProofGeneratorView<'_, S>,
    h_bold_weights: ScalarVector<S::Scalar>,
    u_scalar: S::Scalar,
    mut p: S::Point,
    mut a: ScalarVector<S::Scalar>,
    mut b: ScalarVector<S::Scalar>,
    transcript: &mut ProverTranscript,
) -> Result<(), FcmpNativeErrorV1> {
    let n = generators.g_bold.len();
    if n == 0
        || !n.is_power_of_two()
        || generators.h_bold.len() != n
        || h_bold_weights.len() != n
        || a.len() != n
        || b.len() != n
    {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }

    let mut g_bold = generators.g_bold.to_vec();
    let mut h_bold = generators
        .h_bold
        .iter()
        .copied()
        .zip(h_bold_weights.0.iter().copied())
        .map(|(point, weight)| point.scale(weight))
        .collect::<Vec<_>>();
    let u = generators.g.scale(u_scalar);

    // The inner-product protocol itself does not otherwise use `P` while
    // proving. Check the opening unconditionally so release builds cannot
    // serialize a proof for an inconsistent caller-supplied statement.
    let mut opening_terms = Vec::with_capacity((2 * n) + 1);
    opening_terms.extend(a.0.iter().copied().zip(g_bold.iter().copied()));
    opening_terms.extend(b.0.iter().copied().zip(h_bold.iter().copied()));
    opening_terms.push((a.inner_product(b.0.iter().copied()), u));
    if multiexp::<S>(&opening_terms) != p {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }

    while g_bold.len() > 1 {
        let (a_left, a_right) = a.split()?;
        let (b_left, b_right) = b.split()?;
        let half = g_bold.len() / 2;
        let g_right = g_bold.split_off(half);
        let g_left = g_bold;
        let h_right = h_bold.split_off(half);
        let h_left = h_bold;
        if a_left.len() != half
            || a_right.len() != half
            || b_left.len() != half
            || b_right.len() != half
            || h_left.len() != half
            || h_right.len() != half
        {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }

        let c_left = a_left.inner_product(b_right.0.iter().copied());
        let c_right = a_right.inner_product(b_left.0.iter().copied());
        let left = {
            let mut terms = Vec::with_capacity((2 * half) + 1);
            terms.extend(a_left.0.iter().copied().zip(g_right.iter().copied()));
            terms.extend(b_right.0.iter().copied().zip(h_left.iter().copied()));
            terms.push((c_left, u));
            multiexp::<S>(&terms)
        };
        let right = {
            let mut terms = Vec::with_capacity((2 * half) + 1);
            terms.extend(a_right.0.iter().copied().zip(g_left.iter().copied()));
            terms.extend(b_left.0.iter().copied().zip(h_right.iter().copied()));
            terms.push((c_right, u));
            multiexp::<S>(&terms)
        };
        if left.is_identity() || right.is_identity() {
            return Err(FcmpNativeErrorV1::InnerProductRoundIdentity);
        }

        transcript.push_point(left);
        transcript.push_point(right);
        let challenge = transcript.challenge::<S>()?;
        let inverse = challenge
            .invert()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;

        g_bold = g_left
            .into_iter()
            .zip(g_right)
            .map(|(left, right)| multiexp::<S>(&[(inverse, left), (challenge, right)]))
            .collect();
        h_bold = h_left
            .into_iter()
            .zip(h_right)
            .map(|(left, right)| multiexp::<S>(&[(challenge, left), (inverse, right)]))
            .collect();
        p = left.scale(challenge.square()) + p + right.scale(inverse.square());
        a = (a_left * challenge) + &(a_right * inverse);
        b = (b_left * inverse) + &(b_right * challenge);
    }

    if g_bold.len() != 1 || h_bold.len() != 1 || a.len() != 1 || b.len() != 1 {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    let folded = multiexp::<S>(&[(a[0], g_bold[0]), (b[0], h_bold[0]), (a[0] * b[0], u)]);
    if folded != p {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    transcript.push_scalar(a[0]);
    transcript.push_scalar(b[0]);
    Ok(())
}

fn challenge_products<F: ProofScalar>(challenges: &[(F, F)]) -> Vec<F> {
    let mut products = vec![F::ONE; 1 << challenges.len()];
    if !challenges.is_empty() {
        products[0] = challenges[0].1;
        products[1] = challenges[0].0;
        for (column, challenge) in challenges.iter().enumerate().skip(1) {
            let mut slots = (1 << (column + 1)) - 1;
            while slots > 0 {
                products[slots] = products[slots / 2] * challenge.0;
                products[slots - 1] = products[slots / 2] * challenge.1;
                slots = slots.saturating_sub(2);
            }
        }
    }
    products
}

fn verify_inner_product<S: ProofSuite>(
    generators: ProofGeneratorView<'_, S>,
    h_bold_weights: ScalarVector<S::Scalar>,
    u: S::Scalar,
    verifier: &mut BatchVerifier<S>,
    transcript: &mut VerifierTranscript<'_>,
) -> Result<(), FcmpNativeErrorV1> {
    if generators.h_bold.len() != h_bold_weights.len() {
        return Err(FcmpNativeErrorV1::ArithmeticInvariant);
    }
    verifier.ensure_len(generators.g_bold.len());
    let mut lr_len = 0;
    while (1 << lr_len) < generators.g_bold.len() {
        lr_len += 1;
    }
    let mut challenges = Vec::with_capacity(lr_len);
    for _ in 0..lr_len {
        let left = transcript.read_point::<S::Point>()?;
        let right = transcript.read_point::<S::Point>()?;
        let x = transcript.challenge::<S>()?;
        let inverse = x.invert().ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        verifier.additional.push((x.square(), left));
        verifier.additional.push((inverse.square(), right));
        challenges.push((x, inverse));
    }
    let products = challenge_products(&challenges);
    let a = transcript.read_scalar::<S::Scalar>()?;
    let b = transcript.read_scalar::<S::Scalar>()?;
    for index in 0..generators.g_bold.len() {
        verifier.g_bold[index] -= products[index] * a;
        verifier.h_bold[index] -= products[products.len() - 1 - index] * b * h_bold_weights[index];
    }
    verifier.g -= a * b * u;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rand_08::{SeedableRng as _, rngs::StdRng};

    use super::*;
    use crate::privacy_engines::fcmp_plus_plus::{
        FailingRngV1,
        field::{Field25519, SelenePoint},
        proof_math::{SeleneSuite, selene_bp_generators},
    };

    #[derive(Default)]
    struct NonCanonicalRng {
        calls: usize,
    }

    impl RngCore for NonCanonicalRng {
        fn next_u32(&mut self) -> u32 {
            u32::MAX
        }

        fn next_u64(&mut self) -> u64 {
            u64::MAX
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            destination.fill(0xff);
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
            self.calls += 1;
            destination.fill(0xff);
            Ok(())
        }
    }

    impl CryptoRng for NonCanonicalRng {}

    fn circuit_constraints() -> Vec<LinComb<Field25519>> {
        vec![
            LinComb::empty()
                .term(Field25519::ONE, Variable::aO(0))
                .constant(-Field25519::from_u64(12)),
            LinComb::empty()
                .term(Field25519::ONE, Variable::aO(1))
                .constant(-Field25519::from_u64(30)),
            LinComb::empty()
                .term(Field25519::ONE, Variable::aL(0))
                .term(
                    Field25519::ONE,
                    Variable::CG {
                        commitment: 0,
                        index: 0,
                    },
                )
                .constant(-Field25519::from_u64(10)),
            LinComb::empty()
                .term(Field25519::ONE, Variable::aR(1))
                .term(
                    Field25519::ONE,
                    Variable::CG {
                        commitment: 1,
                        index: 3,
                    },
                )
                .constant(-Field25519::from_u64(20)),
        ]
    }

    fn commitment(
        generators: ProofGeneratorView<'_, SeleneSuite>,
        opening: &VectorCommitmentOpening<Field25519>,
    ) -> SelenePoint {
        let mut terms = opening
            .values
            .0
            .iter()
            .copied()
            .zip(generators.g_bold.iter().copied())
            .collect::<Vec<_>>();
        terms.push((opening.mask, generators.h));
        multiexp::<SeleneSuite>(&terms)
    }

    fn verify_test_circuit(context: [u8; 32], proof: &[u8]) -> Result<(), FcmpNativeErrorV1> {
        let generators = selene_bp_generators().reduce(4)?;
        let mut transcript = VerifierTranscript::new(context, proof);
        let (vector_commitments, scalar_commitments) =
            transcript.read_commitments::<SeleneSuite>(2, 0)?;
        ArithmeticCircuitStatement::new(
            generators,
            circuit_constraints(),
            vector_commitments,
            scalar_commitments,
        )?
        .verify(&mut transcript)?;
        if transcript.consumed() != proof.len() {
            return Err(FcmpNativeErrorV1::TranscriptConsumption);
        }
        Ok(())
    }

    #[test]
    fn native_arithmetic_circuit_prover_round_trips_and_tampering_fails_closed() {
        let context = [0x42_u8; 32];
        let generators = selene_bp_generators().reduce(4).expect("generators");
        let openings = vec![
            VectorCommitmentOpening::new(
                vec![
                    Field25519::from_u64(7),
                    Field25519::from_u64(8),
                    Field25519::from_u64(9),
                    Field25519::from_u64(10),
                ],
                Field25519::from_u64(13),
            ),
            VectorCommitmentOpening::new(
                vec![
                    Field25519::from_u64(11),
                    Field25519::from_u64(12),
                    Field25519::from_u64(13),
                    Field25519::from_u64(14),
                ],
                Field25519::from_u64(17),
            ),
        ];
        let commitments = openings
            .iter()
            .map(|opening| commitment(generators, opening))
            .collect::<Vec<_>>();
        let witness = ArithmeticCircuitWitness::<SeleneSuite>::new(
            vec![Field25519::from_u64(3), Field25519::from_u64(5)],
            vec![Field25519::from_u64(4), Field25519::from_u64(6)],
            openings.clone(),
        )
        .expect("witness");
        let mut transcript = ProverTranscript::new(context);
        transcript.write_commitments::<SeleneSuite>(commitments.clone(), Vec::new());
        let statement = ArithmeticCircuitStatement::new(
            generators,
            circuit_constraints(),
            commitments.clone(),
            Vec::new(),
        )
        .expect("statement");
        let mut rng = StdRng::seed_from_u64(0xfca5_0001);
        statement
            .prove(&mut rng, &mut transcript, witness)
            .expect("proof");
        let proof = transcript.complete();
        assert_eq!(proof.len() % 32, 0);
        verify_test_circuit(context, &proof).expect("native proof verifies");

        // Every serialized point/scalar phase is bound either by the
        // transcript or a checked proof equation.
        for element in 0..(proof.len() / 32) {
            let mut mutated = proof.clone();
            mutated[element * 32] ^= 1;
            assert!(
                verify_test_circuit(context, &mutated).is_err(),
                "mutated proof element {element} was accepted"
            );
        }
        assert!(verify_test_circuit([0x43; 32], &proof).is_err());
        let mut extended = proof.clone();
        extended.extend_from_slice(&[0_u8; 32]);
        assert!(verify_test_circuit(context, &extended).is_err());

        // Bad multiplication values and bad Pedersen openings are rejected
        // before an arithmetic proof can be emitted.
        let invalid_gate_witness = ArithmeticCircuitWitness::<SeleneSuite>::new(
            vec![Field25519::from_u64(4), Field25519::from_u64(5)],
            vec![Field25519::from_u64(4), Field25519::from_u64(6)],
            openings.clone(),
        )
        .expect("shape-valid witness");
        let mut bad_gate_transcript = ProverTranscript::new(context);
        bad_gate_transcript.write_commitments::<SeleneSuite>(commitments.clone(), Vec::new());
        assert!(
            ArithmeticCircuitStatement::new(
                generators,
                circuit_constraints(),
                commitments.clone(),
                Vec::new(),
            )
            .expect("statement")
            .prove(&mut rng, &mut bad_gate_transcript, invalid_gate_witness)
            .is_err()
        );

        let mut bad_openings = openings;
        bad_openings[0].values[0] += Field25519::ONE;
        let invalid_opening_witness = ArithmeticCircuitWitness::<SeleneSuite>::new(
            vec![Field25519::from_u64(3), Field25519::from_u64(5)],
            vec![Field25519::from_u64(4), Field25519::from_u64(6)],
            bad_openings,
        )
        .expect("shape-valid witness");
        let mut bad_opening_transcript = ProverTranscript::new(context);
        bad_opening_transcript.write_commitments::<SeleneSuite>(commitments.clone(), Vec::new());
        assert!(
            ArithmeticCircuitStatement::new(
                generators,
                circuit_constraints(),
                commitments,
                Vec::new(),
            )
            .expect("statement")
            .prove(
                &mut rng,
                &mut bad_opening_transcript,
                invalid_opening_witness
            )
            .is_err()
        );
    }

    #[test]
    fn generalized_bulletproof_randomness_rejects_noncanonical_rng_at_fixed_bound() {
        let mut rng = NonCanonicalRng::default();
        assert_eq!(
            random_scalar::<Field25519>(&mut rng),
            Err(FcmpNativeErrorV1::ProverRandomnessExhausted)
        );
        assert_eq!(rng.calls, MAX_PROVER_SCALAR_ATTEMPTS_V1);
        assert_eq!(MAX_PROVER_SCALAR_ATTEMPTS_V1, 128);
        assert_eq!(
            random_scalar::<Field25519>(&mut FailingRngV1),
            Err(FcmpNativeErrorV1::RandomnessUnavailable)
        );
    }
}
