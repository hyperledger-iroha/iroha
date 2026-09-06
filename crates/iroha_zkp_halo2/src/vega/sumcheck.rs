//! Canonical compressed sum-check verifier used by Vega Spartan.
use super::{
    VegaT256ScalarV1 as Scalar, VegaTranscriptError, VegaTranscriptV1,
    algebra::{AlgebraError, decompress_univariate, evaluate_univariate, evaluation_table_size},
};
use thiserror::Error;
/// Failure while replaying a Vega sum-check proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum SumcheckError {
    #[error("Vega sum-check has the wrong number of rounds")]
    WrongRoundCount,
    #[error("Vega sum-check round has the wrong polynomial degree")]
    WrongDegree,
    #[error("Vega sum-check prover claim does not match its evaluation tables")]
    InvalidClaim,
    #[error("Vega sum-check could not reserve its evaluation table")]
    ResourceExhausted,
    #[error(transparent)]
    Algebra(#[from] AlgebraError),
    #[error(transparent)]
    Transcript(#[from] VegaTranscriptError),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompressedUnivariate {
    pub(super) coefficients_except_linear: Vec<Scalar>,
}
impl CompressedUnivariate {
    pub(super) fn new(
        coefficients_except_linear: Vec<Scalar>,
        degree: usize,
    ) -> Result<Self, SumcheckError> {
        if degree == 0 || coefficients_except_linear.len() != degree {
            return Err(SumcheckError::WrongDegree);
        }
        Ok(Self {
            coefficients_except_linear,
        })
    }
    pub(super) fn coefficients(&self) -> &[Scalar] {
        &self.coefficients_except_linear
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SumcheckProof {
    pub(super) rounds: Vec<CompressedUnivariate>,
}
/// Move-only owner for one prover table containing witness-derived scalars.
///
/// Binding erases the discarded half before shortening the vector, and drop erases the live half on
/// success, error, or unwind. The scalar type is `Copy`, so this is a best-effort erasure of the
/// owned heap allocation, not a claim that compiler-created register or stack copies are erased.
pub(super) struct SecretScalarTable {
    values: Vec<Scalar>,
}
/// One full multilinear table stored as independently allocated lower and upper halves. Consuming
/// the first sum-check round binds into `lower` and drops `upper`, so the allocator can release
/// half of the resident table instead of retaining a full-table `Vec` capacity after truncation.
pub(super) struct SplitSecretScalarTable {
    lower: SecretScalarTable,
    upper: SecretScalarTable,
}
impl SecretScalarTable {
    #[cfg(test)]
    pub(super) fn new(values: Vec<Scalar>) -> Self {
        Self { values }
    }
    pub(super) fn try_zeroed(len: usize) -> Result<Self, SumcheckError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(len)
            .map_err(|_| SumcheckError::ResourceExhausted)?;
        values.resize(len, Scalar::zero());
        Ok(Self { values })
    }
    pub(super) fn try_eq_evals(point: &[Scalar]) -> Result<Self, SumcheckError> {
        let size = table_size(point.len())?;
        let mut evaluations = Self::try_zeroed(size)?;
        evaluations.values[0] = Scalar::one();
        let mut populated = 1;
        for coordinate in point.iter().rev().copied() {
            for index in 0..populated {
                let selected = evaluations.values[index] * coordinate;
                evaluations.values[populated + index] = selected;
                evaluations.values[index] -= selected;
            }
            populated *= 2;
        }
        Ok(evaluations)
    }
    pub(super) fn len(&self) -> usize {
        self.values.len()
    }
    pub(super) fn as_slice(&self) -> &[Scalar] {
        &self.values
    }
    pub(super) fn as_mut_slice(&mut self) -> &mut [Scalar] {
        &mut self.values
    }
    fn bind_top(&mut self, challenge: Scalar) -> Result<(), SumcheckError> {
        if self.values.len() < 2 || !self.values.len().is_power_of_two() {
            return Err(SumcheckError::WrongRoundCount);
        }
        let half = self.values.len() / 2;
        let (lower, upper) = self.values.split_at_mut(half);
        for index in 0..half {
            let mut upper_value = upper[index];
            lower[index] += challenge * (upper_value - lower[index]);
            upper[index].clear_secret();
            upper_value.clear_secret();
        }
        self.values.truncate(half);
        Ok(())
    }
}
impl SplitSecretScalarTable {
    #[cfg(test)]
    pub(super) fn new(lower: Vec<Scalar>, upper: Vec<Scalar>) -> Self {
        Self {
            lower: SecretScalarTable::new(lower),
            upper: SecretScalarTable::new(upper),
        }
    }
    pub(super) fn try_zeroed(len: usize) -> Result<Self, SumcheckError> {
        if len < 2 || !len.is_power_of_two() {
            return Err(SumcheckError::WrongRoundCount);
        }
        let half = len / 2;
        Ok(Self {
            lower: SecretScalarTable::try_zeroed(half)?,
            upper: SecretScalarTable::try_zeroed(half)?,
        })
    }
    pub(super) fn len(&self) -> usize {
        self.lower.len() + self.upper.len()
    }
    pub(super) fn as_slices(&self) -> (&[Scalar], &[Scalar]) {
        (self.lower.as_slice(), self.upper.as_slice())
    }
    pub(super) fn as_mut_slices(&mut self) -> (&mut [Scalar], &mut [Scalar]) {
        (self.lower.as_mut_slice(), self.upper.as_mut_slice())
    }
    fn bind_first(mut self, challenge: Scalar) -> Result<SecretScalarTable, SumcheckError> {
        if self.lower.len() == 0 || self.lower.len() != self.upper.len() {
            return Err(SumcheckError::WrongRoundCount);
        }
        for index in 0..self.lower.len() {
            let mut lower_value = self.lower.as_slice()[index];
            let mut upper_value = self.upper.as_slice()[index];
            self.lower.as_mut_slice()[index] =
                lower_value + challenge * (upper_value - lower_value);
            self.upper.as_mut_slice()[index].clear_secret();
            lower_value.clear_secret();
            upper_value.clear_secret();
        }
        drop(self.upper);
        Ok(self.lower)
    }
}
impl Drop for SecretScalarTable {
    fn drop(&mut self) {
        #[cfg(test)]
        let had_values = !self.values.is_empty();
        let values = core::hint::black_box(&mut self.values);
        for value in values.iter_mut() {
            value.clear_secret();
        }
        #[cfg(test)]
        if had_values && values.iter().all(|value| value.is_zero()) {
            let _ = SECRET_SCALAR_TABLE_ZEROIZED_DROPS.try_with(|drops| {
                drops.set(drops.get().saturating_add(1));
            });
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *values);
    }
}
#[cfg(test)]
std::thread_local! {
    static SECRET_SCALAR_TABLE_ZEROIZED_DROPS: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
}
#[cfg(test)]
fn secret_scalar_table_zeroized_drop_count() -> usize {
    SECRET_SCALAR_TABLE_ZEROIZED_DROPS
        .try_with(core::cell::Cell::get)
        .unwrap_or(0)
}
/// Prove the cubic outer Spartan sum-check
/// `sum_x eq(tau,x) * (A(x) * B(x) - D(x))`.
///
/// The first round consumes independently allocated lower/upper product
/// halves and derives `eq(tau[1..], x)` in a depth-first scalar stream. Only
/// after all three upper halves are erased and freed is the half-sized bound
/// equality table allocated for the remaining rounds.
pub(super) fn prove_cubic_with_split_first_owned(
    initial_claim: Scalar,
    tau: &[Scalar],
    a: SplitSecretScalarTable,
    b: SplitSecretScalarTable,
    d: SplitSecretScalarTable,
    transcript: &mut VegaTranscriptV1,
) -> Result<(SumcheckProof, Vec<Scalar>, [Scalar; 3]), SumcheckError> {
    let round_count = tau.len();
    let expected = table_size(round_count)?;
    if round_count == 0
        || expected < 2
        || a.len() != expected
        || b.len() != expected
        || d.len() != expected
    {
        return Err(SumcheckError::WrongRoundCount);
    }
    let (a_lower, a_upper) = a.as_slices();
    let (b_lower, b_upper) = b.as_slices();
    let (d_lower, d_upper) = d.as_slices();
    let half = expected / 2;
    if a_lower.len() != half
        || a_upper.len() != half
        || b_lower.len() != half
        || b_upper.len() != half
        || d_lower.len() != half
        || d_upper.len() != half
    {
        return Err(SumcheckError::WrongRoundCount);
    }
    let tau_zero = tau[0];
    let eq_zero_scale = Scalar::one() - tau_zero;
    let eq_one_scale = tau_zero;
    let mut evaluations = [Scalar::zero(); 4];
    let mut visited = 0_usize;
    visit_eq_evaluations(&tau[1..], Scalar::one(), &mut |index, q| {
        let eq_zero = q * eq_zero_scale;
        let delta_eq = q * (eq_one_scale - eq_zero_scale);
        let a_zero = a_lower[index];
        let b_zero = b_lower[index];
        let d_zero = d_lower[index];
        let delta_a = a_upper[index] - a_zero;
        let delta_b = b_upper[index] - b_zero;
        let delta_d = d_upper[index] - d_zero;
        for (evaluation, point) in evaluations.iter_mut().zip([
            Scalar::zero(),
            Scalar::one(),
            Scalar::from_u64(2),
            Scalar::from_u64(3),
        ]) {
            *evaluation += (eq_zero + point * delta_eq)
                * ((a_zero + point * delta_a) * (b_zero + point * delta_b)
                    - d_zero
                    - point * delta_d);
        }
        visited += 1;
    });
    if visited != half || evaluations[0] + evaluations[1] != initial_claim {
        return Err(SumcheckError::InvalidClaim);
    }
    let coefficients = interpolate_cubic(evaluations)?;
    let compressed =
        CompressedUnivariate::new(vec![coefficients[0], coefficients[2], coefficients[3]], 3)?;
    transcript.absorb_univariate(b"p", compressed.coefficients())?;
    let first_challenge = transcript.squeeze(b"c")?;
    let claim = evaluate_univariate(&coefficients, first_challenge)?;
    let mut a = a.bind_first(first_challenge)?;
    let mut b = b.bind_first(first_challenge)?;
    let mut d = d.bind_first(first_challenge)?;
    let mut eq = SecretScalarTable::try_eq_evals(&tau[1..])?;
    let equality_scale =
        (Scalar::one() - first_challenge) * eq_zero_scale + first_challenge * eq_one_scale;
    for evaluation in eq.as_mut_slice() {
        *evaluation *= equality_scale;
    }
    let mut rounds = Vec::with_capacity(round_count);
    rounds.push(compressed);
    let mut challenges = Vec::with_capacity(round_count);
    challenges.push(first_challenge);
    prove_cubic_remaining_rounds_owned(
        claim,
        round_count - 1,
        &mut eq,
        &mut a,
        &mut b,
        &mut d,
        transcript,
        &mut rounds,
        &mut challenges,
    )?;
    if a.len() != 1 || b.len() != 1 || d.len() != 1 || eq.len() != 1 {
        return Err(SumcheckError::WrongRoundCount);
    }
    Ok((
        SumcheckProof::new(rounds),
        challenges,
        [a.as_slice()[0], b.as_slice()[0], d.as_slice()[0]],
    ))
}
fn visit_eq_evaluations(point: &[Scalar], weight: Scalar, visit: &mut impl FnMut(usize, Scalar)) {
    fn recurse(
        point: &[Scalar],
        depth: usize,
        weight: Scalar,
        index: &mut usize,
        visit: &mut impl FnMut(usize, Scalar),
    ) {
        if depth == point.len() {
            visit(*index, weight);
            *index += 1;
            return;
        }
        let coordinate = point[depth];
        recurse(
            point,
            depth + 1,
            weight * (Scalar::one() - coordinate),
            index,
            visit,
        );
        recurse(point, depth + 1, weight * coordinate, index, visit);
    }
    let mut index = 0;
    recurse(point, 0, weight, &mut index, visit);
}
#[allow(clippy::too_many_arguments)]
fn prove_cubic_remaining_rounds_owned(
    mut claim: Scalar,
    round_count: usize,
    eq: &mut SecretScalarTable,
    a: &mut SecretScalarTable,
    b: &mut SecretScalarTable,
    d: &mut SecretScalarTable,
    transcript: &mut VegaTranscriptV1,
    rounds: &mut Vec<CompressedUnivariate>,
    challenges: &mut Vec<Scalar>,
) -> Result<(), SumcheckError> {
    for _ in 0..round_count {
        let half = a.len() / 2;
        if half == 0 || b.len() != a.len() || d.len() != a.len() || eq.len() != a.len() {
            return Err(SumcheckError::WrongRoundCount);
        }
        let mut evaluation_zero = Scalar::zero();
        let mut evaluation_one = Scalar::zero();
        let mut evaluation_two = Scalar::zero();
        let mut evaluation_three = Scalar::zero();
        for index in 0..half {
            let eq_zero = eq.as_slice()[index];
            let a_zero = a.as_slice()[index];
            let b_zero = b.as_slice()[index];
            let d_zero = d.as_slice()[index];
            let delta_eq = eq.as_slice()[half + index] - eq_zero;
            let delta_a = a.as_slice()[half + index] - a_zero;
            let delta_b = b.as_slice()[half + index] - b_zero;
            let delta_d = d.as_slice()[half + index] - d_zero;
            evaluation_zero += eq_zero * (a_zero * b_zero - d_zero);
            evaluation_one +=
                (eq_zero + delta_eq) * ((a_zero + delta_a) * (b_zero + delta_b) - d_zero - delta_d);
            let two = Scalar::from_u64(2);
            evaluation_two += (eq_zero + two * delta_eq)
                * ((a_zero + two * delta_a) * (b_zero + two * delta_b) - d_zero - two * delta_d);
            let three = Scalar::from_u64(3);
            evaluation_three += (eq_zero + three * delta_eq)
                * ((a_zero + three * delta_a) * (b_zero + three * delta_b)
                    - d_zero
                    - three * delta_d);
        }
        if evaluation_zero + evaluation_one != claim {
            return Err(SumcheckError::InvalidClaim);
        }
        let coefficients = interpolate_cubic([
            evaluation_zero,
            evaluation_one,
            evaluation_two,
            evaluation_three,
        ])?;
        let compressed =
            CompressedUnivariate::new(vec![coefficients[0], coefficients[2], coefficients[3]], 3)?;
        transcript.absorb_univariate(b"p", compressed.coefficients())?;
        let challenge = transcript.squeeze(b"c")?;
        claim = evaluate_univariate(&coefficients, challenge)?;
        challenges.push(challenge);
        rounds.push(compressed);
        eq.bind_top(challenge)?;
        a.bind_top(challenge)?;
        b.bind_top(challenge)?;
        d.bind_top(challenge)?;
    }
    Ok(())
}
/// Prove the quadratic inner Spartan sum-check `sum_x A(x) * B(x)`.
pub(super) fn prove_quadratic_owned(
    initial_claim: Scalar,
    round_count: usize,
    mut a: SecretScalarTable,
    mut b: SecretScalarTable,
    transcript: &mut VegaTranscriptV1,
) -> Result<(SumcheckProof, Vec<Scalar>, [Scalar; 2]), SumcheckError> {
    let expected = table_size(round_count)?;
    if expected == 0 || a.len() != expected || b.len() != expected {
        return Err(SumcheckError::WrongRoundCount);
    }
    let mut claim = initial_claim;
    let mut rounds = Vec::with_capacity(round_count);
    let mut challenges = Vec::with_capacity(round_count);
    for _ in 0..round_count {
        let half = a.len() / 2;
        if half == 0 || b.len() != a.len() {
            return Err(SumcheckError::WrongRoundCount);
        }
        let mut evaluation_zero = Scalar::zero();
        let mut evaluation_one = Scalar::zero();
        let mut evaluation_two = Scalar::zero();
        for index in 0..half {
            let a_zero = a.as_slice()[index];
            let b_zero = b.as_slice()[index];
            let delta_a = a.as_slice()[half + index] - a_zero;
            let delta_b = b.as_slice()[half + index] - b_zero;
            evaluation_zero += a_zero * b_zero;
            evaluation_one += (a_zero + delta_a) * (b_zero + delta_b);
            let two = Scalar::from_u64(2);
            evaluation_two += (a_zero + two * delta_a) * (b_zero + two * delta_b);
        }
        if evaluation_zero + evaluation_one != claim {
            return Err(SumcheckError::InvalidClaim);
        }
        let coefficients =
            interpolate_quadratic([evaluation_zero, evaluation_one, evaluation_two])?;
        let compressed = CompressedUnivariate::new(vec![coefficients[0], coefficients[2]], 2)?;
        transcript.absorb_univariate(b"p", compressed.coefficients())?;
        let challenge = transcript.squeeze(b"c")?;
        claim = evaluate_univariate(&coefficients, challenge)?;
        challenges.push(challenge);
        rounds.push(compressed);
        a.bind_top(challenge)?;
        b.bind_top(challenge)?;
    }
    if a.len() != 1 || b.len() != 1 {
        return Err(SumcheckError::WrongRoundCount);
    }
    Ok((
        SumcheckProof::new(rounds),
        challenges,
        [a.as_slice()[0], b.as_slice()[0]],
    ))
}
fn table_size(round_count: usize) -> Result<usize, SumcheckError> {
    Ok(evaluation_table_size(round_count)?)
}
fn interpolate_quadratic(evaluations: [Scalar; 3]) -> Result<[Scalar; 3], SumcheckError> {
    let two_inverse = Scalar::from_u64(2)
        .inverse()
        .map_err(|_| SumcheckError::InvalidClaim)?;
    let constant = evaluations[0];
    let quadratic =
        (evaluations[0] - Scalar::from_u64(2) * evaluations[1] + evaluations[2]) * two_inverse;
    let linear = evaluations[1] - constant - quadratic;
    Ok([constant, linear, quadratic])
}
fn interpolate_cubic(evaluations: [Scalar; 4]) -> Result<[Scalar; 4], SumcheckError> {
    let two_inverse = Scalar::from_u64(2)
        .inverse()
        .map_err(|_| SumcheckError::InvalidClaim)?;
    let six_inverse = Scalar::from_u64(6)
        .inverse()
        .map_err(|_| SumcheckError::InvalidClaim)?;
    let constant = evaluations[0];
    let cubic = (evaluations[3] - Scalar::from_u64(3) * evaluations[2]
        + Scalar::from_u64(3) * evaluations[1]
        - evaluations[0])
        * six_inverse;
    let quadratic = (evaluations[2] - Scalar::from_u64(2) * evaluations[1] + evaluations[0])
        * two_inverse
        - Scalar::from_u64(3) * cubic;
    let linear = evaluations[1] - constant - quadratic - cubic;
    Ok([constant, linear, quadratic, cubic])
}
impl SumcheckProof {
    pub(super) fn new(rounds: Vec<CompressedUnivariate>) -> Self {
        Self { rounds }
    }
    pub(super) fn verify(
        &self,
        initial_claim: Scalar,
        round_count: usize,
        degree: usize,
        transcript: &mut VegaTranscriptV1,
    ) -> Result<(Scalar, Vec<Scalar>), SumcheckError> {
        if self.rounds.len() != round_count {
            return Err(SumcheckError::WrongRoundCount);
        }
        if degree == 0 {
            return Err(SumcheckError::WrongDegree);
        }
        let mut claim = initial_claim;
        let mut challenges = Vec::with_capacity(round_count);
        for round in &self.rounds {
            if round.coefficients().len() != degree {
                return Err(SumcheckError::WrongDegree);
            }
            let polynomial = decompress_univariate(round.coefficients(), claim)?;
            transcript.absorb_univariate(b"p", round.coefficients())?;
            let challenge = transcript.squeeze(b"c")?;
            claim = evaluate_univariate(&polynomial, challenge)?;
            challenges.push(challenge);
        }
        Ok((claim, challenges))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::algebra::{eq_evals, eq_evaluate};
    fn prove_cubic_with_split_inputs(
        initial_claim: Scalar,
        tau: &[Scalar],
        a: &[Scalar],
        b: &[Scalar],
        d: &[Scalar],
        transcript: &mut VegaTranscriptV1,
    ) -> Result<(SumcheckProof, Vec<Scalar>, [Scalar; 3]), SumcheckError> {
        let expected = table_size(tau.len())?;
        if a.len() != expected || b.len() != expected || d.len() != expected {
            return Err(SumcheckError::WrongRoundCount);
        }
        let half = expected / 2;
        prove_cubic_with_split_first_owned(
            initial_claim,
            tau,
            SplitSecretScalarTable::new(a[..half].to_vec(), a[half..].to_vec()),
            SplitSecretScalarTable::new(b[..half].to_vec(), b[half..].to_vec()),
            SplitSecretScalarTable::new(d[..half].to_vec(), d[half..].to_vec()),
            transcript,
        )
    }
    fn prove_quadratic(
        initial_claim: Scalar,
        round_count: usize,
        a: &[Scalar],
        b: &[Scalar],
        transcript: &mut VegaTranscriptV1,
    ) -> Result<(SumcheckProof, Vec<Scalar>, [Scalar; 2]), SumcheckError> {
        prove_quadratic_owned(
            initial_claim,
            round_count,
            SecretScalarTable::new(a.to_vec()),
            SecretScalarTable::new(b.to_vec()),
            transcript,
        )
    }
    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }
    fn multilinear_at(table: [Scalar; 4], x: Scalar, y: Scalar) -> Scalar {
        table[0] * (Scalar::one() - x) * (Scalar::one() - y)
            + table[1] * (Scalar::one() - x) * y
            + table[2] * x * (Scalar::one() - y)
            + table[3] * x * y
    }
    fn valid_two_round_proof() -> (SumcheckProof, Scalar) {
        let table = [s(2), s(3), s(5), s(7)];
        let claim = table
            .into_iter()
            .fold(Scalar::zero(), |sum, value| sum + value);
        let first_constant = table[0] + table[1];
        let mut prover_transcript = VegaTranscriptV1::new_neutron_nova();
        prover_transcript
            .absorb_univariate(b"p", &[first_constant])
            .expect("bounded");
        let first_challenge = prover_transcript.squeeze(b"c").expect("round");
        let second_constant = multilinear_at(table, first_challenge, Scalar::zero());
        (
            SumcheckProof::new(vec![
                CompressedUnivariate::new(vec![first_constant], 1).expect("linear"),
                CompressedUnivariate::new(vec![second_constant], 1).expect("linear"),
            ]),
            claim,
        )
    }
    fn bind_all(table: Vec<Scalar>, point: &[Scalar]) -> Scalar {
        let mut table = SecretScalarTable::new(table);
        for challenge in point {
            table.bind_top(*challenge).expect("power-of-two table");
        }
        assert_eq!(table.len(), 1);
        table.as_slice()[0]
    }
    #[test]
    fn cubic_and_quadratic_provers_replay_against_the_verifier() {
        let tau = [s(13), s(17)];
        let a = [s(2), s(3), s(5), s(7)];
        let b = [s(11), s(13), s(17), s(19)];
        let d = [s(23), s(29), s(31), s(37)];
        let eq = eq_evals(&tau).expect("small table");
        let cubic_claim = eq
            .iter()
            .copied()
            .zip(a.iter().copied())
            .zip(b.iter().copied().zip(d.iter().copied()))
            .fold(Scalar::zero(), |sum, ((eq, a), (b, d))| {
                sum + eq * (a * b - d)
            });
        let mut prover_transcript = VegaTranscriptV1::new_neutron_nova();
        let (cubic, prover_challenges, claims) =
            prove_cubic_with_split_inputs(cubic_claim, &tau, &a, &b, &d, &mut prover_transcript)
                .expect("valid cubic proof");
        let mut verifier_transcript = VegaTranscriptV1::new_neutron_nova();
        let (final_claim, verifier_challenges) = cubic
            .verify(cubic_claim, 2, 3, &mut verifier_transcript)
            .expect("valid cubic verification");
        assert_eq!(prover_challenges, verifier_challenges);
        assert_eq!(
            claims,
            [
                bind_all(a.to_vec(), &verifier_challenges),
                bind_all(b.to_vec(), &verifier_challenges),
                bind_all(d.to_vec(), &verifier_challenges),
            ]
        );
        assert_eq!(
            final_claim,
            eq_evaluate(&tau, &verifier_challenges).expect("same dimension")
                * (claims[0] * claims[1] - claims[2])
        );
        let quadratic_claim = a
            .iter()
            .copied()
            .zip(b.iter().copied())
            .fold(Scalar::zero(), |sum, (a, b)| sum + a * b);
        let mut prover_transcript = VegaTranscriptV1::new_neutron_nova();
        let (quadratic, prover_challenges, claims) =
            prove_quadratic(quadratic_claim, 2, &a, &b, &mut prover_transcript)
                .expect("valid quadratic proof");
        let mut verifier_transcript = VegaTranscriptV1::new_neutron_nova();
        let (final_claim, verifier_challenges) = quadratic
            .verify(quadratic_claim, 2, 2, &mut verifier_transcript)
            .expect("valid quadratic verification");
        assert_eq!(prover_challenges, verifier_challenges);
        assert_eq!(
            claims,
            [
                bind_all(a.to_vec(), &verifier_challenges),
                bind_all(b.to_vec(), &verifier_challenges),
            ]
        );
        assert_eq!(final_claim, claims[0] * claims[1]);
    }
    #[test]
    fn owned_provers_preserve_deterministic_transcript_schedule() {
        let tau = [s(13), s(17)];
        let a = [s(2), s(3), s(5), s(7)];
        let b = [s(11), s(13), s(17), s(19)];
        let d = [s(23), s(29), s(31), s(37)];
        let cubic_claim = eq_evals(&tau)
            .expect("small equality table")
            .into_iter()
            .zip(a)
            .zip(b.into_iter().zip(d))
            .fold(Scalar::zero(), |sum, ((eq, a), (b, d))| {
                sum + eq * (a * b - d)
            });
        let mut borrowed_transcript = VegaTranscriptV1::new_neutron_nova();
        let borrowed =
            prove_cubic_with_split_inputs(cubic_claim, &tau, &a, &b, &d, &mut borrowed_transcript)
                .expect("canonical split path");
        let mut split_transcript = VegaTranscriptV1::new_neutron_nova();
        let split = prove_cubic_with_split_first_owned(
            cubic_claim,
            &tau,
            SplitSecretScalarTable::new(a[..2].to_vec(), a[2..].to_vec()),
            SplitSecretScalarTable::new(b[..2].to_vec(), b[2..].to_vec()),
            SplitSecretScalarTable::new(d[..2].to_vec(), d[2..].to_vec()),
            &mut split_transcript,
        )
        .expect("split-first owned path");
        assert_eq!(split, borrowed);
        let borrowed_after = borrowed_transcript.squeeze(b"after").expect("bounded");
        assert_eq!(
            split_transcript.squeeze(b"after").expect("bounded"),
            borrowed_after
        );
        let quadratic_claim = a
            .iter()
            .copied()
            .zip(b.iter().copied())
            .fold(Scalar::zero(), |sum, (a, b)| sum + a * b);
        let mut borrowed_transcript = VegaTranscriptV1::new_neutron_nova();
        let borrowed = prove_quadratic(quadratic_claim, 2, &a, &b, &mut borrowed_transcript)
            .expect("reference path");
        let mut owned_transcript = VegaTranscriptV1::new_neutron_nova();
        let owned = prove_quadratic_owned(
            quadratic_claim,
            2,
            SecretScalarTable::new(a.to_vec()),
            SecretScalarTable::new(b.to_vec()),
            &mut owned_transcript,
        )
        .expect("owned path");
        assert_eq!(owned, borrowed);
        assert_eq!(
            owned_transcript.squeeze(b"after").expect("bounded"),
            borrowed_transcript.squeeze(b"after").expect("bounded")
        );
    }
    #[test]
    fn secret_table_owner_zeroizes_success_error_and_unwind() {
        assert!(matches!(
            SecretScalarTable::try_zeroed(usize::MAX),
            Err(SumcheckError::ResourceExhausted)
        ));
        let before_success = secret_scalar_table_zeroized_drop_count();
        let a = [s(1), s(2), s(3), s(4)];
        let b = [s(5), s(6), s(7), s(8)];
        let claim = a
            .iter()
            .copied()
            .zip(b.iter().copied())
            .fold(Scalar::zero(), |sum, (a, b)| sum + a * b);
        prove_quadratic_owned(
            claim,
            2,
            SecretScalarTable::new(a.to_vec()),
            SecretScalarTable::new(b.to_vec()),
            &mut VegaTranscriptV1::new_neutron_nova(),
        )
        .expect("valid owned proof");
        assert_eq!(
            secret_scalar_table_zeroized_drop_count(),
            before_success + 2
        );
        let before_error = secret_scalar_table_zeroized_drop_count();
        assert_eq!(
            prove_quadratic_owned(
                Scalar::zero(),
                2,
                SecretScalarTable::new(a.to_vec()),
                SecretScalarTable::new(b.to_vec()),
                &mut VegaTranscriptV1::new_neutron_nova(),
            ),
            Err(SumcheckError::InvalidClaim)
        );
        assert_eq!(secret_scalar_table_zeroized_drop_count(), before_error + 2);
        let before_unwind = secret_scalar_table_zeroized_drop_count();
        let unwind = std::panic::catch_unwind(|| {
            let _owned = SecretScalarTable::new(vec![s(9), s(10)]);
            panic!("injected table-owner unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(secret_scalar_table_zeroized_drop_count(), before_unwind + 1);
        let tau = [s(13), s(17)];
        let d = [s(9), s(10), s(11), s(12)];
        let cubic_claim = eq_evals(&tau)
            .expect("small equality table")
            .into_iter()
            .zip(a)
            .zip(b.into_iter().zip(d))
            .fold(Scalar::zero(), |sum, ((eq, a), (b, d))| {
                sum + eq * (a * b - d)
            });
        let before_split_success = secret_scalar_table_zeroized_drop_count();
        prove_cubic_with_split_first_owned(
            cubic_claim,
            &tau,
            SplitSecretScalarTable::new(a[..2].to_vec(), a[2..].to_vec()),
            SplitSecretScalarTable::new(b[..2].to_vec(), b[2..].to_vec()),
            SplitSecretScalarTable::new(d[..2].to_vec(), d[2..].to_vec()),
            &mut VegaTranscriptV1::new_neutron_nova(),
        )
        .expect("valid split-first proof");
        assert_eq!(
            secret_scalar_table_zeroized_drop_count(),
            before_split_success + 7
        );
        let before_split_error = secret_scalar_table_zeroized_drop_count();
        assert_eq!(
            prove_cubic_with_split_first_owned(
                Scalar::zero(),
                &tau,
                SplitSecretScalarTable::new(a[..2].to_vec(), a[2..].to_vec()),
                SplitSecretScalarTable::new(b[..2].to_vec(), b[2..].to_vec()),
                SplitSecretScalarTable::new(d[..2].to_vec(), d[2..].to_vec()),
                &mut VegaTranscriptV1::new_neutron_nova(),
            ),
            Err(SumcheckError::InvalidClaim)
        );
        assert_eq!(
            secret_scalar_table_zeroized_drop_count(),
            before_split_error + 6
        );
        let before_split_unwind = secret_scalar_table_zeroized_drop_count();
        let split_unwind = std::panic::catch_unwind(|| {
            let _owned = SplitSecretScalarTable::new(vec![s(13)], vec![s(17)]);
            panic!("injected split-table-owner unwind");
        });
        assert!(split_unwind.is_err());
        assert_eq!(
            secret_scalar_table_zeroized_drop_count(),
            before_split_unwind + 2
        );
    }
    #[test]
    fn owned_sumcheck_corridor_has_no_borrowed_table_clone() {
        let source = include_str!("sumcheck.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production sum-check source");
        let split_first = source
            .split("pub(super) fn prove_cubic_with_split_first_owned")
            .nth(1)
            .expect("split-first cubic prover")
            .split("/// Prove the quadratic inner Spartan sum-check")
            .next()
            .expect("split-first cubic boundary");
        let quadratic = source
            .split("pub(super) fn prove_quadratic_owned")
            .nth(1)
            .expect("owned quadratic prover")
            .split("fn table_size")
            .next()
            .expect("quadratic boundary");
        assert!(!split_first.contains(".to_vec()"));
        assert!(split_first.contains("visit_eq_evaluations(&tau[1..]"));
        assert!(split_first.contains("a.bind_first(first_challenge)"));
        assert!(split_first.contains("SecretScalarTable::try_eq_evals(&tau[1..])"));
        assert!(!quadratic.contains(".to_vec()"));
        assert!(!production.contains("fn prove_cubic_with_three_inputs_owned"));
        assert!(production.contains("upper[index].clear_secret()"));
        assert!(production.contains("impl Drop for SecretScalarTable"));
    }
    #[test]
    fn sumcheck_provers_reject_false_claims_shapes_and_work_overflow() {
        let table = [s(1), s(2), s(3), s(4)];
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert_eq!(
            prove_quadratic(s(1), 2, &table, &table, &mut transcript),
            Err(SumcheckError::InvalidClaim)
        );
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert_eq!(
            prove_quadratic(s(30), 2, &table[..3], &table[..3], &mut transcript),
            Err(SumcheckError::WrongRoundCount)
        );
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert!(prove_quadratic(s(0), 21, &[], &[], &mut transcript).is_err());
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        assert_eq!(
            prove_cubic_with_split_inputs(
                s(0),
                &[s(1), s(2)],
                &table,
                &table,
                &table[..3],
                &mut transcript
            ),
            Err(SumcheckError::WrongRoundCount)
        );
    }
    #[test]
    fn compressed_sumcheck_replays_a_real_multilinear_claim() {
        let (proof, claim) = valid_two_round_proof();
        let mut verifier_transcript = VegaTranscriptV1::new_neutron_nova();
        let (final_claim, challenges) = proof
            .verify(claim, 2, 1, &mut verifier_transcript)
            .expect("valid sum-check");
        let table = [s(2), s(3), s(5), s(7)];
        assert_eq!(
            final_claim,
            multilinear_at(table, challenges[0], challenges[1])
        );
    }
    #[test]
    fn sumcheck_rejects_wrong_round_degree_claim_and_transcript_mutations() {
        let (proof, claim) = valid_two_round_proof();
        assert_eq!(
            proof.verify(claim, 3, 1, &mut VegaTranscriptV1::new_neutron_nova()),
            Err(SumcheckError::WrongRoundCount)
        );
        assert_eq!(
            proof.verify(claim, 2, 2, &mut VegaTranscriptV1::new_neutron_nova()),
            Err(SumcheckError::WrongDegree)
        );
        let mut mutated = proof.clone();
        mutated.rounds[0].coefficients_except_linear[0] += Scalar::one();
        let mut transcript = VegaTranscriptV1::new_neutron_nova();
        let (mutated_final, mutated_challenges) = mutated
            .verify(claim, 2, 1, &mut transcript)
            .expect("structurally valid but algebraically false");
        let table = [s(2), s(3), s(5), s(7)];
        assert_ne!(
            mutated_final,
            multilinear_at(table, mutated_challenges[0], mutated_challenges[1])
        );
    }
}
