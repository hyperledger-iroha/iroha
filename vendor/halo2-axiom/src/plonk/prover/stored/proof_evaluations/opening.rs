//! Stored-source IPA multiopening through the guarded P frontier.
//!
//! This consumes the exact scalar-stage owner and uses the ordinary nominal challenges,
//! source/point grouping, polynomial/blind folds, Kate recurrence and transcript sequence.
//! Three reusable columns plus the existing H-copy tile and flat metadata form this stage's
//! checked scratch payload, including their explicit retained headers. General stack temporaries,
//! inherited keys, backend/MSM allocations and process RSS are outside this stage-only bound.
//! The consuming inner IPA completes an internal closed proof owner. TODO: connect authenticated
//! Core constructors before exposing proof completion as a production entry point.

use super::{
    Fields, ProofEvaluationsPendingStoredIpaProverV1, StoredLookupErrorV1, StoredOpeningSourceV1,
    add, clear, mul, reserve,
};
use crate::{
    arithmetic::CurveAffine,
    plonk::prover::stored::lookup_permuted::SecretLookupBlindV1,
    poly::{
        Coeff, EvaluationDomain, Polynomial,
        commitment::{Blind, ParamsProver},
        ipa::multiopen::{ChallengeX1, ChallengeX2, ChallengeX3, ChallengeX4},
        stored_advice::{
            STORED_SCALARS_PER_CHUNK_V1, StoredPolynomialProviderV1,
            assignment::StoredAssignmentFieldV1,
        },
    },
    transcript::{EncodedChallenge, TranscriptWrite},
};
#[cfg(test)]
use ff::PrimeField;
use ff::{Field, WithSmallOrderMulGroup};
use group::Curve;
#[cfg(test)]
use group::GroupEncoding;
use rand_core::RngCore;

#[path = "opening/planner.rs"]
mod planner;
use planner::Planner;

#[path = "opening/inner_ipa.rs"]
pub(in crate::plonk::prover::stored) mod inner_ipa;

struct Column<F: StoredAssignmentFieldV1>(Polynomial<F, Coeff>);
impl<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>> Column<F> {
    fn new(domain: &EvaluationDomain<F>, n: usize) -> Result<Self, StoredLookupErrorV1> {
        let mut values = reserve(n)?;
        values.resize(n, F::ZERO);
        Ok(Self(domain.coeff_from_vec(values)))
    }
}
impl<F: StoredAssignmentFieldV1> Drop for Column<F> {
    fn drop(&mut self) {
        clear(&mut self.0.values);
    }
}
struct Workspace<F: StoredAssignmentFieldV1> {
    source: Column<F>,
    q: Column<F>,
    p: Column<F>,
    q_blind: SecretLookupBlindV1<F>,
    p_blind: SecretLookupBlindV1<F>,
    scalar: SecretLookupBlindV1<F>,
}
impl<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>> Workspace<F> {
    fn new(domain: &EvaluationDomain<F>, n: usize) -> Result<Self, StoredLookupErrorV1> {
        Ok(Self {
            source: Column::new(domain, n)?,
            q: Column::new(domain, n)?,
            p: Column::new(domain, n)?,
            q_blind: SecretLookupBlindV1(Blind(F::ZERO)),
            p_blind: SecretLookupBlindV1(Blind(F::ZERO)),
            scalar: SecretLookupBlindV1(Blind(F::ZERO)),
        })
    }
    fn actual_payload(&self, n: usize) -> Result<usize, StoredLookupErrorV1> {
        field_payload::<F>(
            self.source.0.values.capacity(),
            self.q.0.values.capacity(),
            self.p.0.values.capacity(),
            n,
        )
    }
}
fn field_payload<F: StoredAssignmentFieldV1>(
    source: usize,
    q: usize,
    p: usize,
    n: usize,
) -> Result<usize, StoredLookupErrorV1> {
    // H reconstruction overlaps these three columns with one additional bounded decode tile.
    let fields = add(add(add(source, q)?, p)?, n.min(STORED_SCALARS_PER_CHUNK_V1))?;
    add(
        mul(fields, std::mem::size_of::<F>())?,
        add(
            std::mem::size_of::<Workspace<F>>(),
            std::mem::size_of::<Fields<F>>(),
        )?,
    )
}

/// Closed P frontier retaining every original proof owner and the original typed x3.
#[allow(dead_code)]
pub(crate) struct PreparedStoredIpaOpeningV1<
    'params,
    'instances,
    C,
    P,
    R,
    T,
    E,
    const Q: bool,
    const M: u64,
> where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    inner: ProofEvaluationsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
    p: Column<C::Scalar>,
    blind: SecretLookupBlindV1<C::Scalar>,
    x3: ChallengeX3<C>,
}

/// Logical minimum for this stage only, including the H tile and flat grouping allocations.
pub(in crate::plonk::prover::stored) fn scratch_bytes<C, P, R, T, E, const Q: bool, const M: u64>(
    owner: &ProofEvaluationsPendingStoredIpaProverV1<'_, '_, C, P, R, T, E, Q, M>,
) -> Result<usize, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    let s = owner.validate()?;
    add(
        field_payload::<C::Scalar>(s.n, s.n, s.n, s.n)?,
        Planner::<C::Scalar>::minimum_payload(owner.plan.len())?,
    )
}

/// Caller-owned derived secret, cleared on every failed consuming blind selection.
struct BlindDestination<'a, F: StoredAssignmentFieldV1> {
    value: &'a mut Blind<F>,
    keep: bool,
}
impl<F: StoredAssignmentFieldV1> Drop for BlindDestination<'_, F> {
    fn drop(&mut self) {
        if !self.keep {
            clear(std::slice::from_mut(&mut self.value.0));
        }
    }
}
fn fold<F: StoredAssignmentFieldV1>(
    output: &mut [F],
    challenge: F,
    addend: &[F],
) -> Result<(), StoredLookupErrorV1> {
    if output.len() != addend.len() {
        return Err(StoredLookupErrorV1::Context);
    }
    // Match the ordinary multiply-all then add-all schedule, including ZERO/ONE special cases.
    if challenge == F::ZERO {
        clear(output);
    } else if challenge != F::ONE {
        for coefficient in output.iter_mut() {
            *coefficient *= challenge;
        }
    }
    for (coefficient, added) in output.iter_mut().zip(addend) {
        *coefficient += added;
    }
    Ok(())
}
fn divide<F: StoredAssignmentFieldV1>(
    values: &mut [F],
    live: &mut usize,
    point: F,
) -> Result<(), StoredLookupErrorV1> {
    if *live == 0 || *live > values.len() {
        return Err(StoredLookupErrorV1::Context);
    }
    let b = -point;
    let mut original = values[*live - 1];
    let mut tmp = F::ZERO;
    for index in (0..*live - 1).rev() {
        let next_original = values[index];
        let mut lead = original;
        lead -= tmp;
        values[index] = lead;
        tmp = lead;
        tmp *= b;
        original = next_original;
    }
    // Keep every physical slot initialized and erase the removed value before logical shortening.
    clear(&mut values[*live - 1..*live]);
    *live -= 1;
    Ok(())
}
fn evaluate<F: StoredAssignmentFieldV1>(values: &[F], point: F, output: &mut F) {
    *output = F::ZERO;
    for coefficient in values.iter().rev() {
        *output = *output * point + coefficient;
    }
}

impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    ProofEvaluationsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    fn fold_query_blind(
        mut self,
        representative_query: usize,
        challenge: C::Scalar,
        accumulator: &mut Blind<C::Scalar>,
    ) -> Result<Self, StoredLookupErrorV1> {
        let mut output = BlindDestination {
            value: accumulator,
            keep: false,
        };
        self.validate()?;
        let source = self
            .plan
            .get(representative_query)
            .ok_or(StoredLookupErrorV1::Context)?
            .source;
        if let StoredOpeningSourceV1::Advice(column) = source {
            self.inner.inner.inner.advice = self.inner.inner.inner.advice.fold_opening_blind(
                u32::try_from(column).map_err(|_| StoredLookupErrorV1::Context)?,
                challenge,
                output.value,
            )?;
        } else {
            output.value.0 *= challenge;
            let inner = &self.inner.inner.inner;
            use StoredOpeningSourceV1 as S;
            output.value.0 += match source {
                S::Instance(_) | S::Fixed(_) | S::Permutation(_) => Blind::<C::Scalar>::default().0,
                S::CopyProduct(i) => {
                    inner
                        .permutations
                        .get(i)
                        .ok_or(StoredLookupErrorV1::Context)?
                        .blind
                        .0
                        .0
                }
                S::LookupProduct(i) => {
                    inner
                        .lookups
                        .get(i)
                        .ok_or(StoredLookupErrorV1::Context)?
                        .product
                        .blind
                        .0
                        .0
                }
                S::LookupInput(i) => {
                    inner
                        .lookups
                        .get(i)
                        .ok_or(StoredLookupErrorV1::Context)?
                        .input
                        .blind
                        .0
                        .0
                }
                S::LookupTable(i) => {
                    inner
                        .lookups
                        .get(i)
                        .ok_or(StoredLookupErrorV1::Context)?
                        .table
                        .blind
                        .0
                        .0
                }
                S::Quotient => self.h_blind.0.0,
                S::Random => inner.random.blind.0.0,
                S::Advice(_) => return Err(StoredLookupErrorV1::Context),
            };
        }
        self.validate()?;
        output.keep = true;
        Ok(self)
    }

    fn reconstruct_q(
        mut self,
        plan: &Planner<C::Scalar>,
        set: usize,
        x1: C::Scalar,
        work: &mut Workspace<C::Scalar>,
        pass: u8,
    ) -> Result<Self, StoredLookupErrorV1> {
        self.validate()?;
        if set >= plan.set_count {
            return Err(StoredLookupErrorV1::Context);
        }
        clear(&mut work.q.0.values);
        clear(std::slice::from_mut(&mut work.q_blind.0.0));
        let mut initialized = false;
        for source in plan.sources[..plan.source_count]
            .iter()
            .filter(|source| source.set == set)
        {
            if self
                .plan
                .get(source.representative)
                .is_none_or(|query| query.source != source.identity)
            {
                return Err(StoredLookupErrorV1::Context);
            }
            self.validate()?;
            self =
                self.copy_opening_coefficients(source.representative, &mut work.source.0.values)?;
            self.validate()?;
            if initialized {
                fold(&mut work.q.0.values, x1, &work.source.0.values)?;
            } else {
                work.q.0.values.copy_from_slice(&work.source.0.values);
                initialized = true;
            }
            clear(&mut work.source.0.values);
            self = self.fold_query_blind(source.representative, x1, &mut work.q_blind.0)?;
            self.validate()?;
        }
        if !initialized {
            return Err(StoredLookupErrorV1::Context);
        }
        #[cfg(test)]
        observe(OpeningObservationV1::Reconstructed {
            pass,
            set,
            coefficients: encoded(&work.q.0.values),
            blind: work.q_blind.0.0.to_repr(),
        });
        #[cfg(not(test))]
        let _ = pass;
        Ok(self)
    }
}

impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    ProofEvaluationsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    R: RngCore,
    E: EncodedChallenge<C>,
    T: TranscriptWrite<C, E>,
{
    /// Consume through exact ordinary outer multiopening, retaining guarded P and typed x3.
    ///
    /// Grouping evaluations occur for every original query after x1/x2 and collision checking.
    /// No provider handle, ordinal or write is created. A result is not yet an inner IPA proof.
    pub(crate) fn prepare_ipa_opening(
        mut self,
        scratch_limit_bytes: usize,
    ) -> Result<
        PreparedStoredIpaOpeningV1<'params, 'instances, C, P, R, T, E, Q, M>,
        StoredLookupErrorV1,
    > {
        let s = self.validate()?;
        if scratch_bytes(&self)? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let mut plan = Planner::new(self.plan.len())?;
        let mut work = Workspace::new(&self.inner.inner.inner.pk.vk.domain, s.n)?;
        if add(plan.actual_payload()?, work.actual_payload(s.n)?)? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        self.validate()?;
        let x1: ChallengeX1<C> = self.inner.inner.inner.transcript.squeeze_challenge_scalar();
        self.validate()?;
        #[cfg(test)]
        observe(OpeningObservationV1::Challenge {
            number: 1,
            value: x1.to_repr(),
        });
        let x2: ChallengeX2<C> = self.inner.inner.inner.transcript.squeeze_challenge_scalar();
        self.validate()?;
        #[cfg(test)]
        observe(OpeningObservationV1::Challenge {
            number: 2,
            value: x2.to_repr(),
        });
        plan.populate(&self.plan)?;
        self.validate()?;
        // Preserve the otherwise-unused original get_eval calls in original query order.
        for query in 0..self.plan.len() {
            let point = self.plan[query].point;
            let slot = plan.evaluation_slot(query)?;
            self.validate()?;
            self = self.copy_opening_coefficients(query, &mut work.source.0.values)?;
            self.validate()?;
            evaluate(&work.source.0.values, point, &mut plan.evaluations.0[slot]);
            clear(&mut work.source.0.values);
            #[cfg(test)]
            observe(OpeningObservationV1::InitialEvaluation {
                query,
                value: plan.evaluations.0[slot].to_repr(),
            });
        }
        #[cfg(test)]
        observe(grouping(&plan));
        for set in 0..plan.set_count {
            self = self.reconstruct_q(&plan, set, *x1, &mut work, 0)?;
            let points = plan.point_ids(plan.sets[set]);
            if points.len() > s.n {
                return Err(StoredLookupErrorV1::Context);
            }
            let mut live = s.n;
            for point in points {
                divide(&mut work.q.0.values, &mut live, plan.points.0[*point])?;
            }
            // divide keeps removed slots initialized and zero, so the n-slot column is padded.
            #[cfg(test)]
            observe(OpeningObservationV1::Divided {
                set,
                coefficients: encoded(&work.q.0.values),
            });
            if set == 0 {
                work.p.0.values.copy_from_slice(&work.q.0.values);
            } else {
                fold(&mut work.p.0.values, *x2, &work.q.0.values)?;
            }
        }
        self.validate()?;
        work.p_blind.0 = Blind(C::Scalar::random(&mut self.inner.inner.inner.rng));
        self.validate()?;
        let commitment = self
            .inner
            .inner
            .inner
            .params
            .commit(&work.p.0, work.p_blind.0)
            .to_affine();
        self.validate()?;
        #[cfg(test)]
        observe(OpeningObservationV1::Quotient {
            coefficients: encoded(&work.p.0.values),
            blind: work.p_blind.0.0.to_repr(),
            commitment: commitment.to_bytes().as_ref().to_vec(),
        });
        self.inner
            .inner
            .inner
            .transcript
            .write_point(commitment)
            .map_err(|_| StoredLookupErrorV1::Transcript)?;
        self.validate()?;
        let x3: ChallengeX3<C> = self.inner.inner.inner.transcript.squeeze_challenge_scalar();
        self.validate()?;
        #[cfg(test)]
        observe(OpeningObservationV1::Challenge {
            number: 3,
            value: x3.to_repr(),
        });
        for set in 0..plan.set_count {
            self = self.reconstruct_q(&plan, set, *x1, &mut work, 1)?;
            evaluate(&work.q.0.values, *x3, &mut work.scalar.0.0);
            self.validate()?;
            self.inner
                .inner
                .inner
                .transcript
                .write_scalar(work.scalar.0.0)
                .map_err(|_| StoredLookupErrorV1::Transcript)?;
            self.validate()?;
            #[cfg(test)]
            observe(OpeningObservationV1::U {
                set,
                value: work.scalar.0.0.to_repr(),
            });
            clear(std::slice::from_mut(&mut work.scalar.0.0));
            clear(std::slice::from_mut(&mut work.q_blind.0.0));
        }
        self.validate()?;
        let x4: ChallengeX4<C> = self.inner.inner.inner.transcript.squeeze_challenge_scalar();
        self.validate()?;
        #[cfg(test)]
        observe(OpeningObservationV1::Challenge {
            number: 4,
            value: x4.to_repr(),
        });
        for set in 0..plan.set_count {
            self = self.reconstruct_q(&plan, set, *x1, &mut work, 2)?;
            fold(&mut work.p.0.values, *x4, &work.q.0.values)?;
            work.p_blind.0 = Blind(work.p_blind.0.0 * *x4 + work.q_blind.0.0);
            self.validate()?;
        }
        #[cfg(test)]
        observe(OpeningObservationV1::Prepared {
            coefficients: encoded(&work.p.0.values),
            blind: work.p_blind.0.0.to_repr(),
            x3: x3.to_repr(),
        });
        let Workspace {
            source,
            q,
            p,
            q_blind,
            p_blind,
            scalar,
        } = work;
        drop(source);
        self.validate()?;
        drop(q);
        self.validate()?;
        drop(q_blind);
        drop(scalar);
        drop(plan);
        self.validate()?;
        Ok(PreparedStoredIpaOpeningV1 {
            inner: self,
            p,
            blind: p_blind,
            x3,
        })
    }
}

#[cfg(test)]
impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    PreparedStoredIpaOpeningV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    pub(in crate::plonk::prover::stored) fn observed_inner(
        &self,
    ) -> &ProofEvaluationsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M> {
        &self.inner
    }
    pub(in crate::plonk::prover::stored) fn observed_p(&self) -> &Polynomial<C::Scalar, Coeff> {
        &self.p.0
    }
    pub(in crate::plonk::prover::stored) fn observed_p_blind(&self) -> Blind<C::Scalar> {
        self.blind.0
    }
    pub(in crate::plonk::prover::stored) fn observed_x3(&self) -> ChallengeX3<C> {
        self.x3
    }
}

/// Runtime test observations; absent entirely from release builds and scratch accounting.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::plonk::prover::stored) enum OpeningObservationV1 {
    Challenge {
        number: u8,
        value: [u8; 32],
    },
    InitialEvaluation {
        query: usize,
        value: [u8; 32],
    },
    Grouping {
        queries: Vec<(usize, usize)>,
        sources: Vec<(
            StoredOpeningSourceV1,
            usize,
            usize,
            Vec<usize>,
            Vec<[u8; 32]>,
        )>,
        points: Vec<[u8; 32]>,
        sets: Vec<usize>,
    },
    Reconstructed {
        pass: u8,
        set: usize,
        coefficients: Vec<[u8; 32]>,
        blind: [u8; 32],
    },
    Divided {
        set: usize,
        coefficients: Vec<[u8; 32]>,
    },
    Quotient {
        coefficients: Vec<[u8; 32]>,
        blind: [u8; 32],
        commitment: Vec<u8>,
    },
    U {
        set: usize,
        value: [u8; 32],
    },
    Prepared {
        coefficients: Vec<[u8; 32]>,
        blind: [u8; 32],
        x3: [u8; 32],
    },
}
#[cfg(test)]
thread_local! { static OBSERVATIONS: std::cell::RefCell<Vec<OpeningObservationV1>> = const { std::cell::RefCell::new(Vec::new()) }; }
#[cfg(test)]
fn observe(value: OpeningObservationV1) {
    OBSERVATIONS.with(|values| values.borrow_mut().push(value));
}
#[cfg(test)]
pub(in crate::plonk::prover::stored) fn take_observations() -> Vec<OpeningObservationV1> {
    OBSERVATIONS.with(|values| std::mem::take(&mut *values.borrow_mut()))
}
#[cfg(test)]
fn encoded<F: StoredAssignmentFieldV1>(values: &[F]) -> Vec<[u8; 32]> {
    values.iter().map(PrimeField::to_repr).collect()
}
#[cfg(test)]
fn grouping<F: StoredAssignmentFieldV1>(plan: &Planner<F>) -> OpeningObservationV1 {
    OpeningObservationV1::Grouping {
        queries: plan
            .queries
            .iter()
            .map(|query| (query.source, query.point))
            .collect(),
        sources: plan.sources[..plan.source_count]
            .iter()
            .enumerate()
            .map(|(index, source)| {
                (
                    source.identity,
                    source.representative,
                    source.set,
                    plan.point_ids(index).to_vec(),
                    encoded(&plan.evaluations.0[source.start..source.start + source.len]),
                )
            })
            .collect(),
        points: encoded(&plan.points.0[..plan.point_count]),
        sets: plan.sets[..plan.set_count].to_vec(),
    }
}

#[cfg(test)]
impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    PreparedStoredIpaOpeningV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    R: RngCore,
    E: EncodedChallenge<C>,
    T: TranscriptWrite<C, E>,
{
    /// Test-only genuine ordinary IPA handoff. The ordinary inner buffers remain unqualified.
    pub(in crate::plonk::prover::stored) fn finish_ordinary_ipa_for_test(
        mut self,
    ) -> Result<
        ProofEvaluationsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
        StoredLookupErrorV1,
    > {
        self.inner.validate()?;
        let original = &mut self.inner.inner.inner.inner;
        crate::poly::ipa::commitment::create_proof(
            original.params,
            &mut original.rng,
            &mut original.transcript,
            &self.p.0,
            self.blind.0,
            *self.x3,
        )
        .map_err(|_| StoredLookupErrorV1::Transcript)?;
        self.inner.validate()?;
        let Self {
            inner, p, blind, ..
        } = self;
        drop(p);
        drop(blind);
        inner.validate()?;
        Ok(inner)
    }
}

/// Test only the metadata planner; this creates no proof owner and performs no source evaluation.
#[cfg(test)]
pub(in crate::plonk::prover::stored) fn project_plan_for_test<F: StoredAssignmentFieldV1>(
    queries: &[(StoredOpeningSourceV1, F)],
) -> Result<OpeningObservationV1, StoredLookupErrorV1> {
    let mut owned = reserve(queries.len())?;
    owned.extend(
        queries
            .iter()
            .map(|(source, point)| super::StoredOpeningQueryV1 {
                source: *source,
                point: *point,
            }),
    );
    let mut planner = Planner::new(queries.len())?;
    planner.populate(&owned)?;
    Ok(grouping(&planner))
}

#[cfg(test)]
#[path = "opening/arithmetic_tests.rs"]
mod arithmetic_tests;
