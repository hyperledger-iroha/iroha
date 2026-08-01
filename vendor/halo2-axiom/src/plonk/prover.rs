#[cfg(feature = "profile")]
use ark_std::{end_timer, start_timer};
use ff::{BatchInvert, Field, WithSmallOrderMulGroup};
use group::Curve;
use rand_core::RngCore;

use std::marker::PhantomData;
use std::ops::RangeTo;

#[cfg(feature = "multicore")]
use crate::multicore::IndexedParallelIterator;
use crate::multicore::{IntoParallelIterator, ParallelIterator};
use std::{collections::HashMap, iter};

use super::{
    ChallengeBeta, ChallengeGamma, ChallengeTheta, ChallengeX, ChallengeY, Error, ProvingKey,
    VerifyingKey,
    circuit::{
        Advice, Any, Assignment, Challenge, Circuit, Column, ConstraintSystem, Fixed, FloorPlanner,
        Instance, Selector,
        sealed::{self},
    },
    lookup, permutation, vanishing,
};

use crate::{
    arithmetic::{CurveAffine, eval_polynomial},
    circuit::Value,
    helpers::release_allocator_slack,
    plonk::Assigned,
    poly::{
        Basis, Coeff, EvaluationDomain, LagrangeCoeff, Polynomial, ProverQuery,
        commitment::{Blind, CommitmentScheme, Params, Prover},
    },
};
use crate::{
    poly::{batch_invert_assigned, batch_invert_assigned_consuming},
    transcript::{EncodedChallenge, TranscriptWrite},
};

/// This creates a proof for the provided `circuit` when given the public
/// parameters `params` and the proving key [`ProvingKey`] that was
/// generated previously for the same circuit. The provided `instances`
/// are zero-padded internally.
pub fn create_proof<
    'params,
    'a,
    Scheme: CommitmentScheme,
    P: Prover<'params, Scheme>,
    E: EncodedChallenge<Scheme::Curve>,
    R: RngCore + 'a,
    T: TranscriptWrite<Scheme::Curve, E>,
    ConcreteCircuit: Circuit<Scheme::Scalar>,
>(
    params: &'params Scheme::ParamsProver,
    pk: &ProvingKey<Scheme::Curve>,
    circuits: &[ConcreteCircuit],
    instances: &[&[&'a [Scheme::Scalar]]],
    mut rng: R,
    mut transcript: &'a mut T,
) -> Result<(), Error>
where
    Scheme::Scalar: WithSmallOrderMulGroup<3>,
    <Scheme as CommitmentScheme>::ParamsProver: Sync,
{
    if circuits.len() != instances.len() {
        return Err(Error::InvalidInstances);
    }

    for instance in instances.iter() {
        if instance.len() != pk.vk.cs.num_instance_columns {
            return Err(Error::InvalidInstances);
        }
    }

    // Hash verification key into transcript
    pk.vk.hash_into(transcript)?;

    let domain = &pk.vk.domain;
    let mut meta = ConstraintSystem::default();
    #[cfg(feature = "circuit-params")]
    let config = ConcreteCircuit::configure_with_params(&mut meta, circuits[0].params());
    #[cfg(not(feature = "circuit-params"))]
    let config = ConcreteCircuit::configure(&mut meta);

    // Selector optimizations cannot be applied here; use the ConstraintSystem
    // from the verification key.
    let meta = &pk.vk.cs;

    struct InstanceSingle<C: CurveAffine> {
        pub instance_values: Vec<Polynomial<C::Scalar, LagrangeCoeff>>,
        pub instance_polys: Vec<Polynomial<C::Scalar, Coeff>>,
    }

    let instance: Vec<InstanceSingle<Scheme::Curve>> = instances
        .iter()
        .map(|instance| -> InstanceSingle<Scheme::Curve> {
            let instance_values = instance
                .iter()
                .map(|values| {
                    let mut poly = domain.empty_lagrange();
                    assert_eq!(poly.len(), params.n() as usize);
                    if values.len() > (poly.len() - (meta.blinding_factors() + 1)) {
                        panic!("Error::InstanceTooLarge");
                    }
                    for (poly, value) in poly.iter_mut().zip(values.iter()) {
                        *poly = *value;
                    }
                    poly
                })
                .collect::<Vec<_>>();

            let instance_polys: Vec<_> = instance_values
                .iter()
                .map(|poly| {
                    let lagrange_vec = domain.lagrange_from_vec(poly.to_vec());
                    domain.lagrange_to_coeff(lagrange_vec)
                })
                .collect();

            InstanceSingle {
                instance_values,
                instance_polys,
            }
        })
        .collect();

    #[derive(Clone)]
    struct AdviceSingle<C: CurveAffine, B: Basis> {
        pub advice_polys: Vec<Polynomial<C::Scalar, B>>,
        pub advice_blinds: Vec<Blind<C::Scalar>>,
    }

    struct WitnessCollection<'params, 'a, 'b, Scheme, P, C, E, R, T>
    where
        Scheme: CommitmentScheme<Curve = C>,
        P: Prover<'params, Scheme>,
        C: CurveAffine,
        E: EncodedChallenge<C>,
        R: RngCore + 'a,
        T: TranscriptWrite<C, E>,
    {
        params: &'params Scheme::ParamsProver,
        current_phase: sealed::Phase,
        last_phase: sealed::Phase,
        phases_complete: bool,
        advice: Vec<Polynomial<Assigned<C::Scalar>, LagrangeCoeff>>,
        challenges: &'b mut HashMap<usize, C::Scalar>,
        instances: &'b [&'a [C::Scalar]],
        usable_rows: RangeTo<usize>,
        advice_single: AdviceSingle<C, LagrangeCoeff>,
        instance_single: &'b InstanceSingle<C>,
        rng: &'b mut R,
        transcript: &'b mut &'a mut T,
        column_indices: [Vec<usize>; 3],
        challenge_indices: [Vec<usize>; 3],
        unusable_rows_start: usize,
        _marker: PhantomData<(P, E)>,
    }

    impl<'params, 'a, 'b, F, Scheme, P, C, E, R, T> Assignment<F>
        for WitnessCollection<'params, 'a, 'b, Scheme, P, C, E, R, T>
    where
        F: Field,
        Scheme: CommitmentScheme<Curve = C>,
        P: Prover<'params, Scheme>,
        C: CurveAffine<ScalarExt = F>,
        E: EncodedChallenge<C>,
        R: RngCore,
        T: TranscriptWrite<C, E>,
        <Scheme as CommitmentScheme>::ParamsProver: Sync,
    {
        fn enter_region<NR, N>(&mut self, _: N)
        where
            NR: Into<String>,
            N: FnOnce() -> NR,
        {
            // Do nothing; we don't care about regions in this context.
        }

        fn exit_region(&mut self) {
            // Do nothing; we don't care about regions in this context.
        }

        fn enable_selector<A, AR>(&mut self, _: A, _: &Selector, _: usize) -> Result<(), Error>
        where
            A: FnOnce() -> AR,
            AR: Into<String>,
        {
            // We only care about advice columns here

            Ok(())
        }

        fn annotate_column<A, AR>(&mut self, _annotation: A, _column: Column<Any>)
        where
            A: FnOnce() -> AR,
            AR: Into<String>,
        {
            // Do nothing
        }

        fn query_instance(&self, column: Column<Instance>, row: usize) -> Result<Value<F>, Error> {
            if !self.usable_rows.contains(&row) {
                return Err(Error::not_enough_rows_available(self.params.k()));
            }

            self.instances
                .get(column.index())
                .and_then(|column| column.get(row))
                .map(|v| Value::known(*v))
                .ok_or(Error::BoundsFailure)
        }

        fn assign_advice<'v>(
            //<V, VR, A, AR>(
            &mut self,
            //_: A,
            column: Column<Advice>,
            row: usize,
            to: Value<Assigned<F>>,
        ) -> Value<&'v Assigned<F>> {
            // debug_assert_eq!(self.current_phase, column.column_type().phase);

            debug_assert!(
                self.usable_rows.contains(&row),
                "{:?}",
                Error::not_enough_rows_available(self.params.k())
            );

            let advice_get_mut = self
                .advice
                .get_mut(column.index())
                .expect("Not enough advice columns")
                .get_mut(row)
                .expect("Not enough rows");
            // We can get another 3-4% decrease in witness gen time by using the following unsafe code, but this skips all array bound checks so we should use it only if the performance gain is really necessary:
            /*
            let advice_get_mut = unsafe {
                self.advice
                    .get_unchecked_mut(column.index())
                    .get_unchecked_mut(row)
            };
            */
            *advice_get_mut = to
                .assign()
                .expect("No Value::unknown() in advice column allowed during create_proof");
            let immutable_raw_ptr = advice_get_mut as *const Assigned<F>;
            Value::known(unsafe { &*immutable_raw_ptr })
        }

        fn assign_fixed(&mut self, _: Column<Fixed>, _: usize, _: Assigned<F>) {
            // We only care about advice columns here
        }

        fn copy(&mut self, _: Column<Any>, _: usize, _: Column<Any>, _: usize) {
            // We only care about advice columns here
        }

        fn fill_from_row(
            &mut self,
            _: Column<Fixed>,
            _: usize,
            _: Value<Assigned<F>>,
        ) -> Result<(), Error> {
            Ok(())
        }

        fn get_challenge(&self, challenge: Challenge) -> Value<F> {
            self.challenges
                .get(&challenge.index())
                .cloned()
                .map(Value::known)
                .unwrap_or_else(Value::unknown)
        }

        fn push_namespace<NR, N>(&mut self, _: N)
        where
            NR: Into<String>,
            N: FnOnce() -> NR,
        {
            // Do nothing; we don't care about namespaces in this context.
        }

        fn pop_namespace(&mut self, _: Option<String>) {
            // Do nothing; we don't care about namespaces in this context.
        }

        fn next_phase(&mut self) {
            assert!(
                !self.phases_complete,
                "all configured advice phases are already committed"
            );
            let phase = self.current_phase.to_u8() as usize;
            #[cfg(feature = "profile")]
            let start1 = start_timer!(|| format!("Phase {phase} inversion and MSM commitment"));
            if phase == 0 {
                // Absorb instances into transcript.
                // Do this here and not earlier in case we want to be able to mutate
                // the instances during synthesize in FirstPhase in the future
                if !P::QUERY_INSTANCE {
                    for values in self.instances.iter() {
                        for value in values.iter() {
                            self.transcript
                                .common_scalar(*value)
                                .expect("Absorbing instance value to transcript failed");
                        }
                    }
                } else {
                    let instance_commitments_projective: Vec<_> =
                        (&self.instance_single.instance_values)
                            .into_par_iter()
                            .map(|poly| self.params.commit_lagrange(poly, Blind::default()))
                            .collect();
                    let mut instance_commitments =
                        vec![C::identity(); instance_commitments_projective.len()];
                    C::CurveExt::batch_normalize(
                        &instance_commitments_projective,
                        &mut instance_commitments,
                    );
                    let instance_commitments = instance_commitments;
                    drop(instance_commitments_projective);

                    for commitment in &instance_commitments {
                        self.transcript
                            .common_point(*commitment)
                            .expect("Absorbing instance commitment to transcript failed");
                    }
                }
            }
            // Commit the advice columns in the current phase
            let mut advice_values = batch_invert_assigned(
                self.column_indices
                    .get(phase)
                    .expect("The API only supports 3 phases right now")
                    .iter()
                    .map(|column_index| &self.advice[*column_index][..])
                    .collect(),
            );
            // Add blinding factors to advice columns
            for advice_values in &mut advice_values {
                for cell in &mut advice_values[self.unusable_rows_start..] {
                    *cell = F::random(&mut self.rng);
                }
            }
            // Compute commitments to advice column polynomials
            let blinds: Vec<_> = advice_values
                .iter()
                .map(|_| Blind(F::random(&mut self.rng)))
                .collect();
            let advice_commitments_projective: Vec<_> = (&advice_values)
                .into_par_iter()
                .zip((&blinds).into_par_iter())
                .map(|(poly, blind)| {
                    let commitment = self.params.commit_lagrange(poly, *blind);
                    release_allocator_slack();
                    commitment
                })
                .collect();
            let mut advice_commitments = vec![C::identity(); advice_commitments_projective.len()];
            C::CurveExt::batch_normalize(&advice_commitments_projective, &mut advice_commitments);
            let advice_commitments = advice_commitments;
            drop(advice_commitments_projective);

            for commitment in &advice_commitments {
                self.transcript
                    .write_point(*commitment)
                    .expect("Absorbing advice commitment to transcript failed");
            }
            for ((column_index, advice_poly), blind) in self.column_indices[phase]
                .iter()
                .zip(advice_values)
                .zip(blinds)
            {
                self.advice_single.advice_polys[*column_index] = advice_poly;
                self.advice_single.advice_blinds[*column_index] = blind;
            }
            for challenge_index in self.challenge_indices[phase].iter() {
                let existing = self.challenges.insert(
                    *challenge_index,
                    *self.transcript.squeeze_challenge_scalar::<()>(),
                );
                assert!(existing.is_none());
            }
            if self.current_phase == self.last_phase {
                // `Phase` represents one of the three public advice phases,
                // not a one-past-the-end cursor. Keep the final phase selected
                // after committing it and track completion independently.
                self.phases_complete = true;
            } else {
                self.current_phase = self.current_phase.next();
            }
            #[cfg(feature = "profile")]
            end_timer!(start1);
        }
    }

    let mut column_indices = [(); 3].map(|_| vec![]);
    for (index, phase) in meta.advice_column_phase.iter().enumerate() {
        column_indices[phase.to_u8() as usize].push(index);
    }
    let mut challenge_indices = [(); 3].map(|_| vec![]);
    for (index, phase) in meta.challenge_phase.iter().enumerate() {
        challenge_indices[phase.to_u8() as usize].push(index);
    }

    #[cfg(feature = "profile")]
    let phase1_time = start_timer!(|| "Phase 1: Witness assignment and MSM commitments");
    let (advice, challenges) = {
        let mut advice = Vec::with_capacity(instances.len());
        let mut challenges = HashMap::<usize, Scheme::Scalar>::with_capacity(meta.num_challenges);

        let unusable_rows_start = params.n() as usize - (meta.blinding_factors() + 1);
        let phases = pk.vk.cs.phases().collect::<Vec<_>>();
        let num_phases = phases.len();
        // WARNING: this will currently not work if `circuits` has more than 1 circuit
        // because the original API squeezes the challenges for a phase after running all circuits
        // once in that phase.
        if num_phases > 1 {
            assert_eq!(
                circuits.len(),
                1,
                "New challenge API doesn't work with multiple circuits yet"
            );
        }
        for ((circuit, instances), instance_single) in
            circuits.iter().zip(instances).zip(instance.iter())
        {
            let mut witness: WitnessCollection<Scheme, P, _, E, _, _> = WitnessCollection {
                params,
                current_phase: phases[0],
                last_phase: *phases.last().expect("at least the first advice phase"),
                phases_complete: false,
                advice: vec![domain.empty_lagrange_assigned(); meta.num_advice_columns],
                instances,
                challenges: &mut challenges,
                // The prover will not be allowed to assign values to advice
                // cells that exist within inactive rows, which include some
                // number of blinding factors and an extra row for use in the
                // permutation argument.
                usable_rows: ..unusable_rows_start,
                advice_single: AdviceSingle::<Scheme::Curve, LagrangeCoeff> {
                    advice_polys: vec![domain.empty_lagrange(); meta.num_advice_columns],
                    advice_blinds: vec![Blind::default(); meta.num_advice_columns],
                },
                instance_single,
                rng: &mut rng,
                transcript: &mut transcript,
                column_indices: column_indices.clone(),
                challenge_indices: challenge_indices.clone(),
                unusable_rows_start,
                _marker: PhantomData,
            };

            // while loop is for compatibility with circuits that do not use the new `next_phase` API to manage phases
            // If the circuit uses the new API, then the while loop will only execute once
            while !witness.phases_complete {
                #[cfg(feature = "profile")]
                let syn_time = start_timer!(|| format!(
                    "Synthesize time starting from phase {} (synthesize may cross multiple phases)",
                    witness.current_phase.to_u8()
                ));
                // Synthesize the circuit to obtain the witness and other information.
                ConcreteCircuit::FloorPlanner::synthesize(
                    &mut witness,
                    circuit,
                    config.clone(),
                    meta.constants.clone(),
                )
                .unwrap();
                #[cfg(feature = "profile")]
                end_timer!(syn_time);
                if !witness.phases_complete {
                    witness.next_phase();
                }
            }
            advice.push(witness.advice_single);
        }

        assert_eq!(challenges.len(), meta.num_challenges);
        let challenges = (0..meta.num_challenges)
            .map(|index| challenges.remove(&index).unwrap())
            .collect::<Vec<_>>();

        (advice, challenges)
    };
    #[cfg(feature = "profile")]
    end_timer!(phase1_time);

    #[cfg(feature = "profile")]
    let phase2_time = start_timer!(|| "Phase 2: Lookup commit permuted");
    // Sample theta challenge for keeping lookup columns linearly independent
    let theta: ChallengeTheta<_> = transcript.squeeze_challenge_scalar();

    let lookups: Vec<Vec<lookup::prover::Permuted<Scheme::Curve>>> = instance
        .iter()
        .zip(advice.iter())
        .map(|(instance, advice)| -> Vec<_> {
            // Construct and commit to permuted values for each lookup
            pk.vk
                .cs
                .lookups
                .iter()
                .map(|lookup| {
                    lookup
                        .commit_permuted(
                            pk,
                            params,
                            domain,
                            theta,
                            &advice.advice_polys,
                            &pk.fixed_values,
                            &instance.instance_values,
                            &challenges,
                            &mut rng,
                            transcript,
                        )
                        .unwrap()
                })
                .collect()
        })
        .collect();
    #[cfg(feature = "profile")]
    end_timer!(phase2_time);

    #[cfg(feature = "profile")]
    let phase3a_time = start_timer!(|| "Phase 3a: Commit to permutations");

    // Sample beta challenge
    let beta: ChallengeBeta<_> = transcript.squeeze_challenge_scalar();

    // Sample gamma challenge
    let gamma: ChallengeGamma<_> = transcript.squeeze_challenge_scalar();

    // Commit to permutations.
    let permutations: Vec<permutation::prover::Committed<Scheme::Curve>> = instance
        .iter()
        .zip(advice.iter())
        .map(|(instance, advice)| {
            pk.vk
                .cs
                .permutation
                .commit(
                    params,
                    pk,
                    &pk.permutation,
                    &advice.advice_polys,
                    &pk.fixed_values,
                    &instance.instance_values,
                    beta,
                    gamma,
                    &mut rng,
                    transcript,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    #[cfg(feature = "profile")]
    end_timer!(phase3a_time);

    #[cfg(feature = "profile")]
    let phase3b_time = start_timer!(|| "Phase 3b: Lookup commit product");
    let lookups: Vec<Vec<lookup::prover::Committed<Scheme::Curve>>> = lookups
        .into_iter()
        .map(|lookups| -> Vec<_> {
            // Construct and commit to products for each lookup
            lookups
                .into_iter()
                .map(|lookup| {
                    lookup
                        .commit_product(pk, params, beta, gamma, &mut rng, transcript)
                        .unwrap()
                })
                .collect()
        })
        .collect();
    #[cfg(feature = "profile")]
    end_timer!(phase3b_time);

    #[cfg(feature = "profile")]
    let vanishing_time = start_timer!(|| "Commit to vanishing argument's random poly");
    // Commit to the vanishing argument's random polynomial for blinding h(x_3)
    let vanishing = vanishing::Argument::commit(params, domain, &mut rng, transcript).unwrap();

    // Obtain challenge for keeping all separate gates linearly independent
    let y: ChallengeY<_> = transcript.squeeze_challenge_scalar();

    #[cfg(feature = "profile")]
    end_timer!(vanishing_time);
    #[cfg(feature = "profile")]
    let fft_time = start_timer!(|| "Calculate advice polys (fft)");

    // Calculate the advice polys
    let advice: Vec<AdviceSingle<Scheme::Curve, Coeff>> = advice
        .into_iter()
        .map(
            |AdviceSingle {
                 advice_polys,
                 advice_blinds,
             }| {
                AdviceSingle {
                    advice_polys: advice_polys
                        .into_iter()
                        .map(|poly| domain.lagrange_to_coeff(poly))
                        .collect::<Vec<_>>(),
                    advice_blinds,
                }
            },
        )
        .collect();
    #[cfg(feature = "profile")]
    end_timer!(fft_time);

    #[cfg(feature = "profile")]
    let phase4_time = start_timer!(|| "Phase 4: Evaluate h(X)");
    // Evaluate the h(X) polynomial
    let h_poly = pk.ev.evaluate_h(
        pk,
        &advice
            .iter()
            .map(|a| a.advice_polys.as_slice())
            .collect::<Vec<_>>(),
        &instance
            .iter()
            .map(|i| i.instance_polys.as_slice())
            .collect::<Vec<_>>(),
        &challenges,
        *y,
        *beta,
        *gamma,
        *theta,
        &lookups,
        &permutations,
        false,
    );
    #[cfg(feature = "profile")]
    end_timer!(phase4_time);

    #[cfg(feature = "profile")]
    let timer = start_timer!(|| "Commit to vanishing argument's h(X) commitments");
    // Construct the vanishing argument's h(X) commitments
    let vanishing = vanishing.construct(params, domain, h_poly, &mut rng, transcript)?;
    #[cfg(feature = "profile")]
    end_timer!(timer);
    #[cfg(feature = "profile")]
    let eval_time = start_timer!(|| "Commit to vanishing argument's h(X) commitments");

    let x: ChallengeX<_> = transcript.squeeze_challenge_scalar();
    let xn = x.pow([params.n()]);

    if P::QUERY_INSTANCE {
        // Compute and hash instance evals for each circuit instance
        for instance in instance.iter() {
            // Evaluate polynomials at omega^i x
            let instance_evals: Vec<_> = meta
                .instance_queries
                .iter()
                .map(|&(column, at)| {
                    eval_polynomial(
                        &instance.instance_polys[column.index()],
                        domain.rotate_omega(*x, at),
                    )
                })
                .collect();

            // Hash each instance column evaluation
            for eval in instance_evals.iter() {
                transcript.write_scalar(*eval)?;
            }
        }
    }

    // Compute and hash advice evals for each circuit instance
    for advice in advice.iter() {
        // Evaluate polynomials at omega^i x
        let advice_evals: Vec<_> = meta
            .advice_queries
            .iter()
            .map(|&(column, at)| {
                eval_polynomial(
                    &advice.advice_polys[column.index()],
                    domain.rotate_omega(*x, at),
                )
            })
            .collect();

        // Hash each advice column evaluation
        for eval in advice_evals.iter() {
            transcript.write_scalar(*eval)?;
        }
    }

    // Compute and hash fixed evals (shared across all circuit instances)
    let fixed_evals: Vec<_> = meta
        .fixed_queries
        .iter()
        .map(|&(column, at)| {
            eval_polynomial(&pk.fixed_polys[column.index()], domain.rotate_omega(*x, at))
        })
        .collect();

    // Hash each fixed column evaluation
    for eval in fixed_evals.iter() {
        transcript.write_scalar(*eval)?;
    }

    let vanishing = vanishing.evaluate(x, xn, domain, transcript)?;

    // Evaluate common permutation data
    pk.permutation.evaluate(x, transcript)?;

    // Evaluate the permutations, if any, at omega^i x.
    let permutations: Vec<permutation::prover::Evaluated<Scheme::Curve>> = permutations
        .into_iter()
        .map(|permutation| permutation.construct().evaluate(pk, x, transcript).unwrap())
        .collect();

    // Evaluate the lookups, if any, at omega^i x.
    let lookups: Vec<Vec<lookup::prover::Evaluated<Scheme::Curve>>> = lookups
        .into_iter()
        .map(|lookups| -> Vec<_> {
            lookups
                .into_iter()
                .map(|p| p.evaluate(pk, x, transcript).unwrap())
                .collect()
        })
        .collect();
    #[cfg(feature = "profile")]
    end_timer!(eval_time);

    let instances = instance
        .iter()
        .zip(advice.iter())
        .zip(permutations.iter())
        .zip(lookups.iter())
        .flat_map(|(((instance, advice), permutation), lookups)| {
            iter::empty()
                .chain(
                    P::QUERY_INSTANCE
                        .then_some(pk.vk.cs.instance_queries.iter().map(move |&(column, at)| {
                            ProverQuery {
                                point: domain.rotate_omega(*x, at),
                                poly: &instance.instance_polys[column.index()],
                                blind: Blind::default(),
                            }
                        }))
                        .into_iter()
                        .flatten(),
                )
                .chain(
                    pk.vk
                        .cs
                        .advice_queries
                        .iter()
                        .map(move |&(column, at)| ProverQuery {
                            point: domain.rotate_omega(*x, at),
                            poly: &advice.advice_polys[column.index()],
                            blind: advice.advice_blinds[column.index()],
                        }),
                )
                .chain(permutation.open(pk, x))
                .chain(lookups.iter().flat_map(move |p| p.open(pk, x)))
        })
        .chain(
            pk.vk
                .cs
                .fixed_queries
                .iter()
                .map(|&(column, at)| ProverQuery {
                    point: domain.rotate_omega(*x, at),
                    poly: &pk.fixed_polys[column.index()],
                    blind: Blind::default(),
                }),
        )
        .chain(pk.permutation.open(x))
        // We query the h(X) polynomial at x
        .chain(vanishing.open(x));

    #[cfg(feature = "profile")]
    let multiopen_time = start_timer!(|| "Phase 5: multiopen");
    let prover = P::new(params);
    #[allow(clippy::let_and_return)]
    let multiopen_res = prover
        .create_proof(&mut rng, transcript, instances)
        .map_err(|_| Error::ConstraintSystemFailure);
    #[cfg(feature = "profile")]
    end_timer!(multiopen_time);
    multiopen_res
}

enum OwnedAdviceStorage<F: Field> {
    Empty,
    Assigned(Vec<Assigned<F>>),
    Evaluated {
        values: Vec<F>,
        rational_denominators: Vec<(usize, F)>,
        last_row: Option<usize>,
    },
}

struct OwnedAdviceColumn<F: Field> {
    storage: OwnedAdviceStorage<F>,
    len: usize,
    reference_exposed: bool,
}

impl<F: Field> OwnedAdviceColumn<F> {
    fn new(len: usize) -> Self {
        Self {
            storage: OwnedAdviceStorage::Empty,
            len,
            reference_exposed: false,
        }
    }

    fn promote_to_assigned(&mut self) {
        if matches!(self.storage, OwnedAdviceStorage::Assigned(_)) {
            return;
        }
        let values = match std::mem::replace(&mut self.storage, OwnedAdviceStorage::Empty) {
            OwnedAdviceStorage::Empty => vec![Assigned::Zero; self.len],
            OwnedAdviceStorage::Assigned(values) => values,
            OwnedAdviceStorage::Evaluated {
                values,
                rational_denominators,
                ..
            } => {
                let mut values = values
                    .into_iter()
                    .map(Assigned::Trivial)
                    .collect::<Vec<_>>();
                for (row, denominator) in rational_denominators {
                    values[row] = Assigned::Rational(values[row].numerator(), denominator);
                }
                values
            }
        };
        self.storage = OwnedAdviceStorage::Assigned(values);
    }

    fn get_mut_returning_reference(&mut self, row: usize) -> Option<&mut Assigned<F>> {
        if row >= self.len {
            return None;
        }
        self.promote_to_assigned();
        // `Assignment::assign_advice` returns a reference with a lifetime that
        // may extend across `Layouter::next_phase`. Keep this allocation alive
        // after its phase commitment so those existing references remain valid.
        self.reference_exposed = true;
        match &mut self.storage {
            OwnedAdviceStorage::Assigned(values) => values.get_mut(row),
            OwnedAdviceStorage::Empty | OwnedAdviceStorage::Evaluated { .. } => unreachable!(),
        }
    }

    fn assign_discarding_value(&mut self, row: usize, value: Assigned<F>) -> Option<()> {
        if row >= self.len {
            return None;
        }

        let requires_promotion = matches!(
            &self.storage,
            OwnedAdviceStorage::Evaluated {
                last_row: Some(last_row),
                ..
            } if row <= *last_row
        );
        if requires_promotion {
            self.promote_to_assigned();
            match &mut self.storage {
                OwnedAdviceStorage::Assigned(values) => values[row] = value,
                OwnedAdviceStorage::Empty | OwnedAdviceStorage::Evaluated { .. } => unreachable!(),
            }
            return Some(());
        }

        if matches!(self.storage, OwnedAdviceStorage::Empty) {
            self.storage = OwnedAdviceStorage::Evaluated {
                values: vec![F::ZERO; self.len],
                rational_denominators: Vec::new(),
                last_row: None,
            };
        }
        match &mut self.storage {
            OwnedAdviceStorage::Assigned(values) => values[row] = value,
            OwnedAdviceStorage::Evaluated {
                values,
                rational_denominators,
                last_row,
            } => {
                values[row] = match value {
                    Assigned::Zero => F::ZERO,
                    Assigned::Trivial(value) => value,
                    Assigned::Rational(numerator, denominator) => {
                        rational_denominators.push((row, denominator));
                        numerator
                    }
                };
                *last_row = Some(row);
            }
            OwnedAdviceStorage::Empty => unreachable!(),
        }
        Some(())
    }

    fn take_polynomial(&mut self, domain: &EvaluationDomain<F>) -> Polynomial<F, LagrangeCoeff>
    where
        F: WithSmallOrderMulGroup<3>,
    {
        if self.reference_exposed {
            return match &self.storage {
                OwnedAdviceStorage::Assigned(values) => {
                    batch_invert_assigned(vec![values.as_slice()])
                        .pop()
                        .expect("one assigned advice column produces one polynomial")
                }
                OwnedAdviceStorage::Empty | OwnedAdviceStorage::Evaluated { .. } => {
                    unreachable!("an exposed advice reference requires assigned storage")
                }
            };
        }
        match std::mem::replace(&mut self.storage, OwnedAdviceStorage::Empty) {
            OwnedAdviceStorage::Empty => domain.empty_lagrange(),
            OwnedAdviceStorage::Assigned(values) => batch_invert_assigned_consuming(vec![values])
                .pop()
                .expect("one assigned advice column produces one polynomial"),
            OwnedAdviceStorage::Evaluated {
                mut values,
                mut rational_denominators,
                ..
            } => {
                rational_denominators
                    .iter_mut()
                    .map(|(_, denominator)| denominator)
                    .batch_invert();
                for (row, inverse) in rational_denominators {
                    values[row] *= inverse;
                }
                domain.lagrange_from_vec(values)
            }
        }
    }
}

/// Creates a proof for one owned circuit while releasing generation-only
/// storage at its last use.
///
/// This is the memory-bounded counterpart to [`create_proof`]. It accepts one
/// owned circuit and one owned proving key, drops the circuit immediately after
/// witness synthesis, then drops the proving key's Lagrange-only preprocessing
/// after lookup and permutation commitments. On success it returns the
/// proving key's owned verifier key so callers can verify the new proof without
/// reparsing or retaining a duplicate verifier domain.
pub fn create_proof_consuming<
    'params,
    'a,
    Scheme: CommitmentScheme,
    P: Prover<'params, Scheme>,
    E: EncodedChallenge<Scheme::Curve>,
    R: RngCore + 'a,
    T: TranscriptWrite<Scheme::Curve, E>,
    ConcreteCircuit: Circuit<Scheme::Scalar>,
>(
    params: &'params Scheme::ParamsProver,
    mut pk: ProvingKey<Scheme::Curve>,
    circuit: ConcreteCircuit,
    instances: &[&[&'a [Scheme::Scalar]]],
    mut rng: R,
    mut transcript: &'a mut T,
) -> Result<VerifyingKey<Scheme::Curve>, Error>
where
    Scheme::Scalar: WithSmallOrderMulGroup<3>,
    <Scheme as CommitmentScheme>::ParamsProver: Sync,
{
    if instances.len() != 1 {
        return Err(Error::InvalidInstances);
    }
    let mut circuit = Some(circuit);

    for instance in instances.iter() {
        if instance.len() != pk.vk.cs.num_instance_columns {
            return Err(Error::InvalidInstances);
        }
    }

    // Hash verification key into transcript
    pk.vk.hash_into(transcript)?;

    let domain = &pk.vk.domain;
    let mut configured_meta = ConstraintSystem::default();
    #[cfg(feature = "circuit-params")]
    let config = ConcreteCircuit::configure_with_params(
        &mut configured_meta,
        circuit
            .as_ref()
            .expect("owned circuit is present before synthesis")
            .params(),
    );
    #[cfg(not(feature = "circuit-params"))]
    let config = ConcreteCircuit::configure(&mut configured_meta);

    // Configuration is repeated only to recover the circuit-specific config
    // value. The proving key owns the authoritative optimized constraint
    // system, so do not retain this duplicate expression graph through proof
    // construction.
    drop(configured_meta);

    // Selector optimizations cannot be applied here; use the ConstraintSystem
    // from the verification key.
    let meta = &pk.vk.cs;

    struct InstanceSingle<C: CurveAffine> {
        pub instance_values: Vec<Polynomial<C::Scalar, LagrangeCoeff>>,
        pub instance_polys: Vec<Polynomial<C::Scalar, Coeff>>,
    }

    let instance: Vec<InstanceSingle<Scheme::Curve>> = instances
        .iter()
        .map(|instance| -> InstanceSingle<Scheme::Curve> {
            let instance_values = instance
                .iter()
                .map(|values| {
                    let mut poly = domain.empty_lagrange();
                    assert_eq!(poly.len(), params.n() as usize);
                    if values.len() > (poly.len() - (meta.blinding_factors() + 1)) {
                        panic!("Error::InstanceTooLarge");
                    }
                    for (poly, value) in poly.iter_mut().zip(values.iter()) {
                        *poly = *value;
                    }
                    poly
                })
                .collect::<Vec<_>>();

            InstanceSingle {
                instance_values,
                // The coefficient form is not needed until after the lookup
                // and copy-permutation commitments have finished using the
                // Lagrange form. Populate it by consuming that allocation at
                // the phase boundary below.
                instance_polys: Vec::new(),
            }
        })
        .collect();

    #[derive(Clone)]
    struct AdviceSingle<C: CurveAffine, B: Basis> {
        pub advice_polys: Vec<Polynomial<C::Scalar, B>>,
        pub advice_blinds: Vec<Blind<C::Scalar>>,
    }

    struct PendingAdviceSingle<C: CurveAffine> {
        pub advice_polys: Vec<Option<Polynomial<C::Scalar, LagrangeCoeff>>>,
        pub advice_blinds: Vec<Blind<C::Scalar>>,
    }

    struct WitnessCollection<'params, 'a, 'b, Scheme, P, C, E, R, T>
    where
        Scheme: CommitmentScheme<Curve = C>,
        P: Prover<'params, Scheme>,
        C: CurveAffine,
        E: EncodedChallenge<C>,
        R: RngCore + 'a,
        T: TranscriptWrite<C, E>,
    {
        params: &'params Scheme::ParamsProver,
        domain: &'b EvaluationDomain<C::Scalar>,
        current_phase: sealed::Phase,
        last_phase: sealed::Phase,
        phases_complete: bool,
        advice: Vec<OwnedAdviceColumn<C::Scalar>>,
        challenges: &'b mut HashMap<usize, C::Scalar>,
        instances: &'b [&'a [C::Scalar]],
        usable_rows: RangeTo<usize>,
        advice_single: PendingAdviceSingle<C>,
        instance_single: &'b InstanceSingle<C>,
        rng: &'b mut R,
        transcript: &'b mut &'a mut T,
        column_indices: [Vec<usize>; 3],
        challenge_indices: [Vec<usize>; 3],
        unusable_rows_start: usize,
        _marker: PhantomData<(P, E)>,
    }

    impl<'params, 'a, 'b, F, Scheme, P, C, E, R, T> Assignment<F>
        for WitnessCollection<'params, 'a, 'b, Scheme, P, C, E, R, T>
    where
        F: WithSmallOrderMulGroup<3>,
        Scheme: CommitmentScheme<Curve = C>,
        P: Prover<'params, Scheme>,
        C: CurveAffine<ScalarExt = F>,
        E: EncodedChallenge<C>,
        R: RngCore,
        T: TranscriptWrite<C, E>,
        <Scheme as CommitmentScheme>::ParamsProver: Sync,
    {
        fn enter_region<NR, N>(&mut self, _: N)
        where
            NR: Into<String>,
            N: FnOnce() -> NR,
        {
            // Do nothing; we don't care about regions in this context.
        }

        fn exit_region(&mut self) {
            // Do nothing; we don't care about regions in this context.
        }

        fn enable_selector<A, AR>(&mut self, _: A, _: &Selector, _: usize) -> Result<(), Error>
        where
            A: FnOnce() -> AR,
            AR: Into<String>,
        {
            // We only care about advice columns here

            Ok(())
        }

        fn annotate_column<A, AR>(&mut self, _annotation: A, _column: Column<Any>)
        where
            A: FnOnce() -> AR,
            AR: Into<String>,
        {
            // Do nothing
        }

        fn query_instance(&self, column: Column<Instance>, row: usize) -> Result<Value<F>, Error> {
            if !self.usable_rows.contains(&row) {
                return Err(Error::not_enough_rows_available(self.params.k()));
            }

            self.instances
                .get(column.index())
                .and_then(|column| column.get(row))
                .map(|v| Value::known(*v))
                .ok_or(Error::BoundsFailure)
        }

        fn assign_advice<'v>(
            //<V, VR, A, AR>(
            &mut self,
            //_: A,
            column: Column<Advice>,
            row: usize,
            to: Value<Assigned<F>>,
        ) -> Value<&'v Assigned<F>> {
            // debug_assert_eq!(self.current_phase, column.column_type().phase);

            debug_assert!(
                self.usable_rows.contains(&row),
                "{:?}",
                Error::not_enough_rows_available(self.params.k())
            );

            let advice_get_mut = self
                .advice
                .get_mut(column.index())
                .expect("Not enough advice columns")
                .get_mut_returning_reference(row)
                .expect("Not enough rows");
            // We can get another 3-4% decrease in witness gen time by using the following unsafe code, but this skips all array bound checks so we should use it only if the performance gain is really necessary:
            /*
            let advice_get_mut = unsafe {
                self.advice
                    .get_unchecked_mut(column.index())
                    .get_unchecked_mut(row)
            };
            */
            *advice_get_mut = to
                .assign()
                .expect("No Value::unknown() in advice column allowed during create_proof");
            let immutable_raw_ptr = advice_get_mut as *const Assigned<F>;
            Value::known(unsafe { &*immutable_raw_ptr })
        }

        fn assign_advice_discarding_value(
            &mut self,
            column: Column<Advice>,
            row: usize,
            to: Value<Assigned<F>>,
        ) {
            debug_assert!(
                self.usable_rows.contains(&row),
                "{:?}",
                Error::not_enough_rows_available(self.params.k())
            );
            let value = to
                .assign()
                .expect("No Value::unknown() in advice column allowed during create_proof");
            self.advice
                .get_mut(column.index())
                .expect("Not enough advice columns")
                .assign_discarding_value(row, value)
                .expect("Not enough rows");
        }

        fn assign_fixed(&mut self, _: Column<Fixed>, _: usize, _: Assigned<F>) {
            // We only care about advice columns here
        }

        fn copy(&mut self, _: Column<Any>, _: usize, _: Column<Any>, _: usize) {
            // We only care about advice columns here
        }

        fn fill_from_row(
            &mut self,
            _: Column<Fixed>,
            _: usize,
            _: Value<Assigned<F>>,
        ) -> Result<(), Error> {
            Ok(())
        }

        fn get_challenge(&self, challenge: Challenge) -> Value<F> {
            self.challenges
                .get(&challenge.index())
                .cloned()
                .map(Value::known)
                .unwrap_or_else(Value::unknown)
        }

        fn push_namespace<NR, N>(&mut self, _: N)
        where
            NR: Into<String>,
            N: FnOnce() -> NR,
        {
            // Do nothing; we don't care about namespaces in this context.
        }

        fn pop_namespace(&mut self, _: Option<String>) {
            // Do nothing; we don't care about namespaces in this context.
        }

        fn next_phase(&mut self) {
            assert!(
                !self.phases_complete,
                "all configured advice phases are already committed"
            );
            let phase = self.current_phase.to_u8() as usize;
            #[cfg(feature = "profile")]
            let start1 = start_timer!(|| format!("Phase {phase} inversion and MSM commitment"));
            if phase == 0 {
                // Absorb instances into transcript.
                // Do this here and not earlier in case we want to be able to mutate
                // the instances during synthesize in FirstPhase in the future
                if !P::QUERY_INSTANCE {
                    for values in self.instances.iter() {
                        for value in values.iter() {
                            self.transcript
                                .common_scalar(*value)
                                .expect("Absorbing instance value to transcript failed");
                        }
                    }
                } else {
                    let instance_commitments_projective: Vec<_> =
                        (&self.instance_single.instance_values)
                            .into_par_iter()
                            .map(|poly| self.params.commit_lagrange(poly, Blind::default()))
                            .collect();
                    let mut instance_commitments =
                        vec![C::identity(); instance_commitments_projective.len()];
                    C::CurveExt::batch_normalize(
                        &instance_commitments_projective,
                        &mut instance_commitments,
                    );
                    let instance_commitments = instance_commitments;
                    drop(instance_commitments_projective);

                    for commitment in &instance_commitments {
                        self.transcript
                            .common_point(*commitment)
                            .expect("Absorbing instance commitment to transcript failed");
                    }
                }
            }
            // Commit the advice columns in the current phase
            let phase_column_indices = self
                .column_indices
                .get(phase)
                .expect("The API only supports 3 phases right now");
            let mut advice_values = Vec::with_capacity(phase_column_indices.len());
            for &column_index in phase_column_indices {
                advice_values.push(self.advice[column_index].take_polynomial(self.domain));
            }
            // Add blinding factors to advice columns
            for advice_values in &mut advice_values {
                for cell in &mut advice_values[self.unusable_rows_start..] {
                    *cell = F::random(&mut self.rng);
                }
            }
            // Compute commitments to advice column polynomials
            let blinds: Vec<_> = advice_values
                .iter()
                .map(|_| Blind(F::random(&mut self.rng)))
                .collect();
            let advice_commitments_projective: Vec<_> = (&advice_values)
                .into_par_iter()
                .zip((&blinds).into_par_iter())
                .map(|(poly, blind)| self.params.commit_lagrange(poly, *blind))
                .collect();
            let mut advice_commitments = vec![C::identity(); advice_commitments_projective.len()];
            C::CurveExt::batch_normalize(&advice_commitments_projective, &mut advice_commitments);
            let advice_commitments = advice_commitments;
            drop(advice_commitments_projective);

            for commitment in &advice_commitments {
                self.transcript
                    .write_point(*commitment)
                    .expect("Absorbing advice commitment to transcript failed");
            }
            for ((column_index, advice_poly), blind) in self.column_indices[phase]
                .iter()
                .zip(advice_values)
                .zip(blinds)
            {
                self.advice_single.advice_polys[*column_index] = Some(advice_poly);
                self.advice_single.advice_blinds[*column_index] = blind;
            }
            for challenge_index in self.challenge_indices[phase].iter() {
                let existing = self.challenges.insert(
                    *challenge_index,
                    *self.transcript.squeeze_challenge_scalar::<()>(),
                );
                assert!(existing.is_none());
            }
            if self.current_phase == self.last_phase {
                // `Phase` has no one-past-the-end value. Match the borrowed
                // prover by tracking final-phase completion separately.
                self.phases_complete = true;
            } else {
                self.current_phase = self.current_phase.next();
            }
            #[cfg(feature = "profile")]
            end_timer!(start1);
        }
    }

    let mut column_indices = [(); 3].map(|_| vec![]);
    for (index, phase) in meta.advice_column_phase.iter().enumerate() {
        column_indices[phase.to_u8() as usize].push(index);
    }
    let mut challenge_indices = [(); 3].map(|_| vec![]);
    for (index, phase) in meta.challenge_phase.iter().enumerate() {
        challenge_indices[phase.to_u8() as usize].push(index);
    }

    #[cfg(feature = "profile")]
    let phase1_time = start_timer!(|| "Phase 1: Witness assignment and MSM commitments");
    let (advice, challenges) = {
        let mut advice: Vec<AdviceSingle<Scheme::Curve, LagrangeCoeff>> =
            Vec::with_capacity(instances.len());
        let mut challenges = HashMap::<usize, Scheme::Scalar>::with_capacity(meta.num_challenges);

        let unusable_rows_start = params.n() as usize - (meta.blinding_factors() + 1);
        let phases = pk.vk.cs.phases().collect::<Vec<_>>();
        // This entry point owns exactly one circuit, so keep each borrow local
        // to synthesis and release the circuit before the final phase
        // commitment allocates blinding and MSM scratch.
        let mut witness: WitnessCollection<Scheme, P, _, E, _, _> = WitnessCollection {
            params,
            domain,
            current_phase: phases[0],
            last_phase: *phases.last().expect("at least the first advice phase"),
            phases_complete: false,
            advice: (0..meta.num_advice_columns)
                .map(|_| OwnedAdviceColumn::new(params.n() as usize))
                .collect(),
            instances: instances[0],
            challenges: &mut challenges,
            // The prover will not be allowed to assign values to advice
            // cells that exist within inactive rows, which include some
            // number of blinding factors and an extra row for use in the
            // permutation argument.
            usable_rows: ..unusable_rows_start,
            advice_single: PendingAdviceSingle::<Scheme::Curve> {
                advice_polys: vec![None; meta.num_advice_columns],
                advice_blinds: vec![Blind::default(); meta.num_advice_columns],
            },
            instance_single: &instance[0],
            rng: &mut rng,
            transcript: &mut transcript,
            column_indices: column_indices.clone(),
            challenge_indices: challenge_indices.clone(),
            unusable_rows_start,
            _marker: PhantomData,
        };

        // The loop is for compatibility with circuits that do not use the new
        // `next_phase` API to manage phases. If the circuit uses the new API,
        // this loop only executes once.
        while !witness.phases_complete {
            #[cfg(feature = "profile")]
            let syn_time = start_timer!(|| format!(
                "Synthesize time starting from phase {} (synthesize may cross multiple phases)",
                witness.current_phase.to_u8()
            ));
            // Keep this borrow scoped to synthesis so the owned circuit can be
            // released before the final call to `next_phase` consumes RNG and
            // builds the advice commitments.
            ConcreteCircuit::FloorPlanner::synthesize(
                &mut witness,
                circuit
                    .as_ref()
                    .expect("owned circuit is present during synthesis"),
                config.clone(),
                meta.constants.clone(),
            )
            .unwrap();
            #[cfg(feature = "profile")]
            end_timer!(syn_time);

            let needs_phase_commit = !witness.phases_complete;
            if !needs_phase_commit || witness.current_phase == witness.last_phase {
                drop(circuit.take());
                release_allocator_slack();
            }
            if needs_phase_commit {
                witness.next_phase();
            }
        }
        let PendingAdviceSingle {
            advice_polys,
            advice_blinds,
        } = witness.advice_single;
        advice.push(AdviceSingle {
            advice_polys: advice_polys
                .into_iter()
                .map(|poly| poly.expect("every advice column is committed exactly once"))
                .collect(),
            advice_blinds,
        });

        assert_eq!(challenges.len(), meta.num_challenges);
        let challenges = (0..meta.num_challenges)
            .map(|index| challenges.remove(&index).unwrap())
            .collect::<Vec<_>>();

        (advice, challenges)
    };
    // Defensive for phase-management implementations that return after
    // advancing past the final phase without a post-synthesis commitment.
    drop(circuit.take());
    release_allocator_slack();
    #[cfg(feature = "profile")]
    end_timer!(phase1_time);

    #[cfg(feature = "profile")]
    let phase2_time = start_timer!(|| "Phase 2: Lookup commit permuted");
    // Sample theta challenge for keeping lookup columns linearly independent
    let theta: ChallengeTheta<_> = transcript.squeeze_challenge_scalar();

    let lookups: Vec<Vec<lookup::prover::Permuted<Scheme::Curve>>> = instance
        .iter()
        .zip(advice.iter())
        .map(|(instance, advice)| -> Vec<_> {
            // Construct and commit to permuted values for each lookup
            pk.vk
                .cs
                .lookups
                .iter()
                .map(|lookup| {
                    lookup
                        .commit_permuted(
                            &pk,
                            params,
                            domain,
                            theta,
                            &advice.advice_polys,
                            &pk.fixed_values,
                            &instance.instance_values,
                            &challenges,
                            &mut rng,
                            transcript,
                        )
                        .unwrap()
                })
                .collect()
        })
        .collect();
    #[cfg(feature = "profile")]
    end_timer!(phase2_time);

    #[cfg(feature = "profile")]
    let phase3a_time = start_timer!(|| "Phase 3a: Commit to permutations");

    // Sample beta challenge
    let beta: ChallengeBeta<_> = transcript.squeeze_challenge_scalar();

    // Sample gamma challenge
    let gamma: ChallengeGamma<_> = transcript.squeeze_challenge_scalar();

    // Commit to permutations.
    let permutations: Vec<permutation::prover::Committed<Scheme::Curve>> = instance
        .iter()
        .zip(advice.iter())
        .map(|(instance, advice)| {
            pk.vk
                .cs
                .permutation
                .commit(
                    params,
                    &pk,
                    &pk.permutation,
                    &advice.advice_polys,
                    &pk.fixed_values,
                    &instance.instance_values,
                    beta,
                    gamma,
                    &mut rng,
                    transcript,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    #[cfg(feature = "profile")]
    end_timer!(phase3a_time);

    // Lookup permutation and copy-permutation commitments are the final users
    // of the key's Lagrange-basis preprocessing. The quotient and multi-open
    // suffix only needs coefficient forms, so release these domain-sized
    // vectors before entering it.
    drop(std::mem::take(&mut pk.fixed_values));
    pk.permutation.drop_lagrange_polynomials();
    release_allocator_slack();
    let instance = instance
        .into_iter()
        .map(|mut instance| {
            instance.instance_polys = std::mem::take(&mut instance.instance_values)
                .into_iter()
                .map(|poly| domain.lagrange_to_coeff(poly))
                .collect();
            instance
        })
        .collect::<Vec<_>>();

    #[cfg(feature = "profile")]
    let phase3b_time = start_timer!(|| "Phase 3b: Lookup commit product");
    let lookups: Vec<Vec<lookup::prover::Committed<Scheme::Curve>>> = lookups
        .into_iter()
        .map(|lookups| -> Vec<_> {
            // Construct and commit to products for each lookup
            lookups
                .into_iter()
                .map(|lookup| {
                    lookup
                        .commit_product(&pk, params, beta, gamma, &mut rng, transcript)
                        .unwrap()
                })
                .collect()
        })
        .collect();
    #[cfg(feature = "profile")]
    end_timer!(phase3b_time);

    #[cfg(feature = "profile")]
    let vanishing_time = start_timer!(|| "Commit to vanishing argument's random poly");
    // Commit to the vanishing argument's random polynomial for blinding h(x_3)
    let vanishing = vanishing::Argument::commit(params, domain, &mut rng, transcript).unwrap();

    // Obtain challenge for keeping all separate gates linearly independent
    let y: ChallengeY<_> = transcript.squeeze_challenge_scalar();

    #[cfg(feature = "profile")]
    end_timer!(vanishing_time);
    #[cfg(feature = "profile")]
    let fft_time = start_timer!(|| "Calculate advice polys (fft)");

    // Calculate the advice polys
    let advice: Vec<AdviceSingle<Scheme::Curve, Coeff>> = advice
        .into_iter()
        .map(
            |AdviceSingle {
                 advice_polys,
                 advice_blinds,
             }| {
                AdviceSingle {
                    advice_polys: advice_polys
                        .into_iter()
                        .map(|poly| domain.lagrange_to_coeff(poly))
                        .collect::<Vec<_>>(),
                    advice_blinds,
                }
            },
        )
        .collect();
    #[cfg(feature = "profile")]
    end_timer!(fft_time);

    #[cfg(feature = "profile")]
    let phase4_time = start_timer!(|| "Phase 4: Evaluate h(X)");
    // Evaluate the h(X) polynomial
    let h_poly = pk.ev.evaluate_h(
        &pk,
        &advice
            .iter()
            .map(|a| a.advice_polys.as_slice())
            .collect::<Vec<_>>(),
        &instance
            .iter()
            .map(|i| i.instance_polys.as_slice())
            .collect::<Vec<_>>(),
        &challenges,
        *y,
        *beta,
        *gamma,
        *theta,
        &lookups,
        &permutations,
        true,
    );
    // The quotient is the final user of this proving-only preprocessing. Keep
    // the verifier key, queried fixed/sigma polynomials, and committed witness
    // polynomials intact for evaluation and multi-open, but release the three
    // unqueried selector-mask coefficient polynomials and the evaluator graph
    // now.
    drop(std::mem::take(&mut pk.l0.values));
    drop(std::mem::take(&mut pk.l_last.values));
    drop(std::mem::take(&mut pk.l_active_row.values));
    drop(std::mem::take(&mut pk.ev));
    drop(challenges);
    release_allocator_slack();
    #[cfg(feature = "profile")]
    end_timer!(phase4_time);

    #[cfg(feature = "profile")]
    let timer = start_timer!(|| "Commit to vanishing argument's h(X) commitments");
    // Construct the vanishing argument's h(X) commitments
    let vanishing = vanishing.construct(params, domain, h_poly, &mut rng, transcript)?;
    #[cfg(feature = "profile")]
    end_timer!(timer);
    #[cfg(feature = "profile")]
    let eval_time = start_timer!(|| "Commit to vanishing argument's h(X) commitments");

    let x: ChallengeX<_> = transcript.squeeze_challenge_scalar();
    let xn = x.pow([params.n()]);

    if P::QUERY_INSTANCE {
        // Compute and hash instance evals for each circuit instance
        for instance in instance.iter() {
            // Evaluate polynomials at omega^i x
            let instance_evals: Vec<_> = meta
                .instance_queries
                .iter()
                .map(|&(column, at)| {
                    eval_polynomial(
                        &instance.instance_polys[column.index()],
                        domain.rotate_omega(*x, at),
                    )
                })
                .collect();

            // Hash each instance column evaluation
            for eval in instance_evals.iter() {
                transcript.write_scalar(*eval)?;
            }
        }
    }

    // Compute and hash advice evals for each circuit instance
    for advice in advice.iter() {
        // Evaluate polynomials at omega^i x
        let advice_evals: Vec<_> = meta
            .advice_queries
            .iter()
            .map(|&(column, at)| {
                eval_polynomial(
                    &advice.advice_polys[column.index()],
                    domain.rotate_omega(*x, at),
                )
            })
            .collect();

        // Hash each advice column evaluation
        for eval in advice_evals.iter() {
            transcript.write_scalar(*eval)?;
        }
    }

    // Compute and hash fixed evals (shared across all circuit instances)
    let fixed_evals: Vec<_> = meta
        .fixed_queries
        .iter()
        .map(|&(column, at)| {
            eval_polynomial(&pk.fixed_polys[column.index()], domain.rotate_omega(*x, at))
        })
        .collect();

    // Hash each fixed column evaluation
    for eval in fixed_evals.iter() {
        transcript.write_scalar(*eval)?;
    }

    let vanishing = vanishing.evaluate(x, xn, domain, transcript)?;

    // Evaluate common permutation data
    pk.permutation.evaluate(x, transcript)?;

    // Evaluate the permutations, if any, at omega^i x.
    let permutations: Vec<permutation::prover::Evaluated<Scheme::Curve>> = permutations
        .into_iter()
        .map(|permutation| {
            permutation
                .construct()
                .evaluate(&pk, x, transcript)
                .unwrap()
        })
        .collect();

    // Evaluate the lookups, if any, at omega^i x.
    let lookups: Vec<Vec<lookup::prover::Evaluated<Scheme::Curve>>> = lookups
        .into_iter()
        .map(|lookups| -> Vec<_> {
            lookups
                .into_iter()
                .map(|p| p.evaluate(&pk, x, transcript).unwrap())
                .collect()
        })
        .collect();
    #[cfg(feature = "profile")]
    end_timer!(eval_time);

    // Capture a copyable reference in the nested `move` closures below. Using
    // the owned binding directly would move the proving key into the iterator
    // even though every query only borrows it.
    let pk_ref = &pk;
    let instances = instance
        .iter()
        .zip(advice.iter())
        .zip(permutations.iter())
        .zip(lookups.iter())
        .flat_map(|(((instance, advice), permutation), lookups)| {
            iter::empty()
                .chain(
                    P::QUERY_INSTANCE
                        .then_some(pk_ref.vk.cs.instance_queries.iter().map(
                            move |&(column, at)| ProverQuery {
                                point: domain.rotate_omega(*x, at),
                                poly: &instance.instance_polys[column.index()],
                                blind: Blind::default(),
                            },
                        ))
                        .into_iter()
                        .flatten(),
                )
                .chain(
                    pk_ref
                        .vk
                        .cs
                        .advice_queries
                        .iter()
                        .map(move |&(column, at)| ProverQuery {
                            point: domain.rotate_omega(*x, at),
                            poly: &advice.advice_polys[column.index()],
                            blind: advice.advice_blinds[column.index()],
                        }),
                )
                .chain(permutation.open(pk_ref, x))
                .chain(lookups.iter().flat_map(move |p| p.open(pk_ref, x)))
        })
        .chain(
            pk_ref
                .vk
                .cs
                .fixed_queries
                .iter()
                .map(|&(column, at)| ProverQuery {
                    point: domain.rotate_omega(*x, at),
                    poly: &pk_ref.fixed_polys[column.index()],
                    blind: Blind::default(),
                }),
        )
        .chain(pk_ref.permutation.open(x))
        // We query the h(X) polynomial at x
        .chain(vanishing.open(x));

    #[cfg(feature = "profile")]
    let multiopen_time = start_timer!(|| "Phase 5: multiopen");
    let prover = P::new(params);
    #[allow(clippy::let_and_return)]
    let multiopen_res = prover
        .create_proof(&mut rng, transcript, instances)
        .map_err(|_| Error::ConstraintSystemFailure);
    #[cfg(feature = "profile")]
    end_timer!(multiopen_time);
    multiopen_res?;
    release_allocator_slack();
    let ProvingKey { vk, .. } = pk;
    Ok(vk)
}

#[test]
fn discarded_owned_advice_is_compact_and_matches_assigned_evaluation() {
    use halo2curves::bn256::Fr;

    let domain = EvaluationDomain::<Fr>::new(1, 2);
    let mut column = OwnedAdviceColumn::new(4);
    column.assign_discarding_value(0, Assigned::Zero).unwrap();
    column
        .assign_discarding_value(1, Assigned::Trivial(Fr::from(3)))
        .unwrap();
    column
        .assign_discarding_value(2, Assigned::Rational(Fr::from(4), Fr::from(2)))
        .unwrap();
    column
        .assign_discarding_value(3, Assigned::Rational(Fr::from(7), Fr::ZERO))
        .unwrap();
    assert!(matches!(
        column.storage,
        OwnedAdviceStorage::Evaluated { .. }
    ));
    assert_eq!(
        column
            .take_polynomial(&domain)
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![Fr::ZERO, Fr::from(3), Fr::from(2), Fr::ZERO]
    );

    // A non-monotone write takes the compatibility path, where the sparse
    // rational metadata is reconstructed before the overwrite.
    let mut mixed = OwnedAdviceColumn::new(4);
    mixed
        .assign_discarding_value(2, Assigned::Rational(Fr::from(9), Fr::from(3)))
        .unwrap();
    mixed
        .assign_discarding_value(1, Assigned::Trivial(Fr::from(5)))
        .unwrap();
    assert!(matches!(mixed.storage, OwnedAdviceStorage::Assigned(_)));
    assert_eq!(
        mixed
            .take_polynomial(&domain)
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![Fr::ZERO, Fr::from(5), Fr::from(3), Fr::ZERO]
    );

    // Ordinary assignment exposes a reference that may be used after a phase
    // transition. Its backing allocation must therefore survive commitment.
    let mut referenced = OwnedAdviceColumn::new(4);
    *referenced.get_mut_returning_reference(0).unwrap() =
        Assigned::Rational(Fr::from(10), Fr::from(2));
    let address_before = match &referenced.storage {
        OwnedAdviceStorage::Assigned(values) => values.as_ptr(),
        OwnedAdviceStorage::Empty | OwnedAdviceStorage::Evaluated { .. } => unreachable!(),
    };
    assert_eq!(referenced.take_polynomial(&domain)[0], Fr::from(5));
    let address_after = match &referenced.storage {
        OwnedAdviceStorage::Assigned(values) => values.as_ptr(),
        OwnedAdviceStorage::Empty | OwnedAdviceStorage::Evaluated { .. } => unreachable!(),
    };
    assert_eq!(address_before, address_after);

    // Promoting a compact discard-only column because a later assignment
    // exposes a reference must preserve every previously evaluated value and
    // keep the promoted allocation alive through polynomial extraction.
    let mut promoted = OwnedAdviceColumn::new(4);
    promoted
        .assign_discarding_value(0, Assigned::Rational(Fr::from(10), Fr::from(2)))
        .unwrap();
    promoted
        .assign_discarding_value(1, Assigned::Trivial(Fr::from(6)))
        .unwrap();
    *promoted.get_mut_returning_reference(2).unwrap() =
        Assigned::Rational(Fr::from(12), Fr::from(3));
    let address_before = match &promoted.storage {
        OwnedAdviceStorage::Assigned(values) => values.as_ptr(),
        OwnedAdviceStorage::Empty | OwnedAdviceStorage::Evaluated { .. } => unreachable!(),
    };
    assert_eq!(
        promoted
            .take_polynomial(&domain)
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![Fr::from(5), Fr::from(6), Fr::from(4), Fr::ZERO]
    );
    let address_after = match &promoted.storage {
        OwnedAdviceStorage::Assigned(values) => values.as_ptr(),
        OwnedAdviceStorage::Empty | OwnedAdviceStorage::Evaluated { .. } => unreachable!(),
    };
    assert_eq!(address_before, address_after);
}

#[test]
fn reconstructed_proving_key_masks_have_canonical_domain_evaluations() {
    use halo2curves::pasta::Fp;

    use crate::{
        arithmetic::eval_polynomial,
        poly::{EvaluationDomain, Rotation},
    };

    let domain = EvaluationDomain::<Fp>::new(4, 4);
    let blinding_factors = 3;
    let last_active_row = domain.get_n() as usize - blinding_factors - 1;
    let (l0, l_last, l_active_row) =
        super::keygen::create_proving_key_masks(&domain, blinding_factors);

    for row in 0..domain.get_n() as usize {
        let point = domain.rotate_omega(Fp::ONE, Rotation(row as i32));
        assert_eq!(
            eval_polynomial(&l0, point),
            if row == 0 { Fp::ONE } else { Fp::ZERO }
        );
        assert_eq!(
            eval_polynomial(&l_last, point),
            if row == last_active_row {
                Fp::ONE
            } else {
                Fp::ZERO
            }
        );
        assert_eq!(
            eval_polynomial(&l_active_row, point),
            if row < last_active_row {
                Fp::ONE
            } else {
                Fp::ZERO
            }
        );
    }
}

#[test]
fn test_create_proof() {
    use crate::{
        circuit::SimpleFloorPlanner,
        plonk::{keygen_pk, keygen_vk},
        poly::kzg::{
            commitment::{KZGCommitmentScheme, ParamsKZG},
            multiopen::ProverSHPLONK,
        },
        transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
    };
    use halo2curves::bn256::Bn256;
    use rand_core::OsRng;

    #[derive(Clone, Copy)]
    struct MyCircuit;

    impl<F: Field> Circuit<F> for MyCircuit {
        type Config = ();
        type FloorPlanner = SimpleFloorPlanner;
        #[cfg(feature = "circuit-params")]
        type Params = ();

        fn without_witnesses(&self) -> Self {
            *self
        }

        fn configure(_meta: &mut ConstraintSystem<F>) -> Self::Config {}

        fn synthesize(
            &self,
            _config: Self::Config,
            _layouter: impl crate::circuit::Layouter<F>,
        ) -> Result<(), Error> {
            Ok(())
        }
    }

    let params: ParamsKZG<Bn256> = ParamsKZG::setup(3, OsRng);
    let vk = keygen_vk(&params, &MyCircuit).expect("keygen_vk should not fail");
    let pk = keygen_pk(&params, vk, &MyCircuit).expect("keygen_pk should not fail");
    let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);

    // Create proof with wrong number of instances
    let proof = create_proof::<KZGCommitmentScheme<_>, ProverSHPLONK<_>, _, _, _, _>(
        &params,
        &pk,
        &[MyCircuit, MyCircuit],
        &[],
        OsRng,
        &mut transcript,
    );
    assert!(matches!(proof.unwrap_err(), Error::InvalidInstances));

    // Create proof with correct number of instances
    create_proof::<KZGCommitmentScheme<_>, ProverSHPLONK<_>, _, _, _, _>(
        &params,
        &pk,
        &[MyCircuit, MyCircuit],
        &[&[], &[]],
        OsRng,
        &mut transcript,
    )
    .expect("proof generation should not fail");
}

#[test]
fn consuming_proof_matches_borrowed_and_releases_owned_inputs() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use crate::{
        SerdeFormat,
        circuit::SimpleFloorPlanner,
        plonk::{keygen_pk, keygen_vk},
        poly::kzg::{
            commitment::{KZGCommitmentScheme, ParamsKZG},
            multiopen::ProverSHPLONK,
        },
        transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
    };
    use halo2curves::bn256::{Bn256, Fr};
    use rand_chacha::ChaCha20Rng;
    use rand_core::{CryptoRng, Error as RngError, OsRng, SeedableRng};

    #[derive(Clone)]
    struct DropCircuit {
        dropped: Option<Arc<AtomicBool>>,
    }

    #[derive(Clone, Copy)]
    struct DropConfig {
        columns: [Column<Advice>; 4],
        discarded_column: Column<Advice>,
    }

    impl Drop for DropCircuit {
        fn drop(&mut self) {
            if let Some(dropped) = &self.dropped {
                dropped.store(true, Ordering::SeqCst);
            }
        }
    }

    impl<F: Field> Circuit<F> for DropCircuit {
        type Config = DropConfig;
        type FloorPlanner = SimpleFloorPlanner;
        #[cfg(feature = "circuit-params")]
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self { dropped: None }
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            // Degree four and four equality columns exercise two permutation
            // sets of two columns, matching the compact Kagemusha chunk width.
            meta.set_minimum_degree(4);
            let columns = [(); 4].map(|_| meta.advice_column());
            for column in columns {
                meta.enable_equality(column);
            }
            let discarded_column = meta.advice_column();
            DropConfig {
                columns,
                discarded_column,
            }
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl crate::circuit::Layouter<F>,
        ) -> Result<(), Error> {
            layouter.assign_region(
                || "copy-permutation region",
                |mut region| {
                    let denominator = F::ONE.double();
                    let left = region.assign_advice(
                        config.columns[0],
                        0,
                        Value::known(Assigned::Rational(denominator.double(), denominator)),
                    );
                    for column in config.columns.into_iter().skip(1) {
                        left.copy_advice(&mut region, column, 0);
                    }
                    region.assign_advice_discarding_value(
                        config.discarded_column,
                        0,
                        Value::known(Assigned::Rational(denominator.double(), denominator)),
                    );
                    region.assign_advice_discarding_value(
                        config.discarded_column,
                        1,
                        Value::known(Assigned::Rational(F::ONE, F::ZERO)),
                    );
                    Ok(())
                },
            )
        }
    }

    struct DropCheckingRng {
        inner: ChaCha20Rng,
        circuit_dropped: Arc<AtomicBool>,
    }

    impl DropCheckingRng {
        fn assert_circuit_dropped(&self) {
            assert!(
                self.circuit_dropped.load(Ordering::SeqCst),
                "owned circuit must be dropped before post-synthesis randomness"
            );
        }
    }

    impl RngCore for DropCheckingRng {
        fn next_u32(&mut self) -> u32 {
            self.assert_circuit_dropped();
            self.inner.next_u32()
        }

        fn next_u64(&mut self) -> u64 {
            self.assert_circuit_dropped();
            self.inner.next_u64()
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            self.assert_circuit_dropped();
            self.inner.fill_bytes(dest);
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RngError> {
            self.assert_circuit_dropped();
            self.inner.try_fill_bytes(dest)
        }
    }

    impl CryptoRng for DropCheckingRng {}

    let keygen_circuit = DropCircuit { dropped: None };
    let params: ParamsKZG<Bn256> = ParamsKZG::setup(3, OsRng);
    let vk = keygen_vk(&params, &keygen_circuit).expect("keygen_vk should not fail");
    let pk = keygen_pk(&params, vk, &keygen_circuit).expect("keygen_pk should not fail");
    let expected_vk_bytes = pk.get_vk().to_bytes(SerdeFormat::Processed);
    let error_pk = pk.clone();
    let borrowed_pk = pk.clone();
    let no_columns: &[&[Fr]] = &[];
    let proof_instances: [&[&[Fr]]; 1] = [no_columns];
    let seed = [7_u8; 32];

    let mut borrowed_transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
    create_proof::<KZGCommitmentScheme<_>, ProverSHPLONK<_>, _, _, _, _>(
        &params,
        &borrowed_pk,
        &[DropCircuit { dropped: None }],
        &proof_instances,
        ChaCha20Rng::from_seed(seed),
        &mut borrowed_transcript,
    )
    .expect("borrowed proof generation should not fail");
    let borrowed_proof = borrowed_transcript.finalize();

    let circuit_dropped = Arc::new(AtomicBool::new(false));
    let mut consuming_transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
    let returned_vk =
        create_proof_consuming::<KZGCommitmentScheme<_>, ProverSHPLONK<_>, _, _, _, _>(
            &params,
            pk,
            DropCircuit {
                dropped: Some(Arc::clone(&circuit_dropped)),
            },
            &proof_instances,
            DropCheckingRng {
                inner: ChaCha20Rng::from_seed(seed),
                circuit_dropped: Arc::clone(&circuit_dropped),
            },
            &mut consuming_transcript,
        )
        .expect("consuming proof generation should not fail");
    let consuming_proof = consuming_transcript.finalize();

    assert!(circuit_dropped.load(Ordering::SeqCst));
    assert_eq!(borrowed_proof, consuming_proof);
    assert_eq!(
        expected_vk_bytes,
        returned_vk.to_bytes(SerdeFormat::Processed)
    );

    let rejected_circuit_dropped = Arc::new(AtomicBool::new(false));
    let mut rejected_transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
    let rejected = create_proof_consuming::<KZGCommitmentScheme<_>, ProverSHPLONK<_>, _, _, _, _>(
        &params,
        error_pk,
        DropCircuit {
            dropped: Some(Arc::clone(&rejected_circuit_dropped)),
        },
        &[],
        ChaCha20Rng::from_seed(seed),
        &mut rejected_transcript,
    );
    assert!(matches!(rejected, Err(Error::InvalidInstances)));
    assert!(rejected_circuit_dropped.load(Ordering::SeqCst));
}

#[test]
fn consuming_ipa_multiphase_proof_matches_borrowed() {
    use crate::{
        circuit::SimpleFloorPlanner,
        plonk::{FirstPhase, SecondPhase, keygen_pk, keygen_vk},
        poly::{
            Rotation,
            commitment::ParamsProver,
            ipa::{
                commitment::{IPACommitmentScheme, ParamsIPA},
                multiopen::ProverIPA,
            },
        },
        transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
    };
    use halo2curves::pasta::{EqAffine, Fp};
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    #[derive(Clone)]
    struct MultiPhaseCircuit {
        first_value: Assigned<Fp>,
    }

    #[derive(Clone, Copy)]
    struct MultiPhaseConfig {
        first: Column<Advice>,
        second: Column<Advice>,
        challenge: Challenge,
        selector: Selector,
    }

    impl Circuit<Fp> for MultiPhaseCircuit {
        type Config = MultiPhaseConfig;
        type FloorPlanner = SimpleFloorPlanner;
        #[cfg(feature = "circuit-params")]
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self {
                first_value: Assigned::Zero,
            }
        }

        fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
            let first = meta.advice_column_in(FirstPhase);
            let challenge = meta.challenge_usable_after(FirstPhase);
            let second = meta.advice_column_in(SecondPhase);
            let selector = meta.selector();
            meta.create_gate("second phase uses first-phase challenge", |meta| {
                let selector = meta.query_selector(selector);
                let first = meta.query_advice(first, Rotation::cur());
                let second = meta.query_advice(second, Rotation::cur());
                let challenge = meta.query_challenge(challenge);
                vec![selector * (second - first * challenge)]
            });
            MultiPhaseConfig {
                first,
                second,
                challenge,
                selector,
            }
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl crate::circuit::Layouter<Fp>,
        ) -> Result<(), Error> {
            let first = layouter.assign_region(
                || "first phase",
                |mut region| {
                    config.selector.enable(&mut region, 0)?;
                    Ok(region.assign_advice(config.first, 0, Value::known(self.first_value)))
                },
            )?;

            layouter.next_phase();
            let challenge = layouter.get_challenge(config.challenge);
            let second_value = first.value().map(|value| **value) * challenge;
            layouter.assign_region(
                || "challenge-dependent second phase",
                |mut region| {
                    region.assign_advice_discarding_value(config.second, 0, second_value);
                    Ok(())
                },
            )
        }
    }

    let circuit = MultiPhaseCircuit {
        first_value: Assigned::Rational(Fp::from(10), Fp::from(2)),
    };
    let params = ParamsIPA::<EqAffine>::new(4);
    let vk = keygen_vk(&params, &circuit).expect("keygen_vk should not fail");
    let pk = keygen_pk(&params, vk, &circuit).expect("keygen_pk should not fail");
    let borrowed_pk = pk.clone();
    let no_columns: &[&[Fp]] = &[];
    let proof_instances: [&[&[Fp]]; 1] = [no_columns];
    let seed = [29_u8; 32];

    let mut borrowed_transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
    create_proof::<IPACommitmentScheme<_>, ProverIPA<_>, _, _, _, _>(
        &params,
        &borrowed_pk,
        &[circuit.clone()],
        &proof_instances,
        ChaCha20Rng::from_seed(seed),
        &mut borrowed_transcript,
    )
    .expect("borrowed IPA proof generation should not fail");
    let borrowed_proof = borrowed_transcript.finalize();

    let mut consuming_transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
    create_proof_consuming::<IPACommitmentScheme<_>, ProverIPA<_>, _, _, _, _>(
        &params,
        pk,
        circuit,
        &proof_instances,
        ChaCha20Rng::from_seed(seed),
        &mut consuming_transcript,
    )
    .expect("consuming IPA proof generation should not fail");
    let consuming_proof = consuming_transcript.finalize();

    assert_eq!(borrowed_proof, consuming_proof);
}

#[test]
fn three_phase_proof_commits_the_final_phase_exactly_once() {
    use crate::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        plonk::{
            Advice, Challenge, Column, ConstraintSystem, Expression, FirstPhase, SecondPhase,
            Selector, ThirdPhase, keygen_pk, keygen_vk, verifier::verify_proof,
        },
        poly::kzg::{
            commitment::{KZGCommitmentScheme, ParamsKZG},
            multiopen::{ProverSHPLONK, VerifierSHPLONK},
            strategy::SingleStrategy,
        },
        transcript::{
            Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
        },
    };
    use halo2curves::bn256::{Bn256, Fr};
    use rand_core::OsRng;

    #[derive(Clone, Debug)]
    struct ThreePhaseConfig {
        selector: Selector,
        first: Column<Advice>,
        second: Column<Advice>,
        third: Column<Advice>,
        after_first: Challenge,
        after_second: Challenge,
    }

    #[derive(Clone, Copy, Debug)]
    struct ThreePhaseCircuit {
        commit_final_in_synthesize: bool,
        advance_after_completion: bool,
    }

    impl Circuit<Fr> for ThreePhaseCircuit {
        type Config = ThreePhaseConfig;
        type FloorPlanner = SimpleFloorPlanner;
        #[cfg(feature = "circuit-params")]
        type Params = ();

        fn without_witnesses(&self) -> Self {
            *self
        }

        fn configure(meta: &mut ConstraintSystem<Fr>) -> Self::Config {
            let selector = meta.selector();
            let first = meta.advice_column_in(FirstPhase);
            let after_first = meta.challenge_usable_after(FirstPhase);
            let second = meta.advice_column_in(SecondPhase);
            let after_second = meta.challenge_usable_after(SecondPhase);
            let third = meta.advice_column_in(ThirdPhase);

            meta.create_gate("three phase witness", |_| {
                vec![
                    selector.expr() * (first.cur() - Expression::Constant(Fr::ONE)),
                    selector.expr() * (second.cur() - after_first.expr()),
                    selector.expr() * (third.cur() - after_second.expr()),
                ]
            });

            ThreePhaseConfig {
                selector,
                first,
                second,
                third,
                after_first,
                after_second,
            }
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<Fr>,
        ) -> Result<(), Error> {
            layouter.assign_region(
                || "three phase witness",
                |mut region| {
                    config.selector.enable(&mut region, 0)?;
                    region.assign_advice(config.first, 0, Value::known(Fr::ONE));

                    region.next_phase();
                    let after_first = region.get_challenge(config.after_first);
                    region.assign_advice(config.second, 0, after_first);

                    region.next_phase();
                    let after_second = region.get_challenge(config.after_second);
                    region.assign_advice(config.third, 0, after_second);

                    if self.commit_final_in_synthesize {
                        region.next_phase();
                    }
                    if self.advance_after_completion {
                        region.next_phase();
                    }
                    Ok(())
                },
            )
        }
    }

    let params = ParamsKZG::<Bn256>::setup(5, OsRng);
    let key_circuit = ThreePhaseCircuit {
        commit_final_in_synthesize: false,
        advance_after_completion: false,
    };
    let vk = keygen_vk(&params, &key_circuit).expect("three-phase VK generation must succeed");
    let pk = keygen_pk(&params, vk, &key_circuit).expect("three-phase PK generation must succeed");

    for commit_final_in_synthesize in [false, true] {
        let circuit = ThreePhaseCircuit {
            commit_final_in_synthesize,
            advance_after_completion: false,
        };
        let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
        create_proof::<KZGCommitmentScheme<Bn256>, ProverSHPLONK<Bn256>, _, _, _, _>(
            &params,
            &pk,
            &[circuit],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("all three advice phases must be committed");
        let proof = transcript.finalize();

        let strategy = SingleStrategy::new(&params);
        let mut transcript = Blake2bRead::<_, _, Challenge255<_>>::init(&proof[..]);
        verify_proof::<KZGCommitmentScheme<Bn256>, VerifierSHPLONK<Bn256>, _, _, _>(
            &params,
            pk.get_vk(),
            strategy,
            &[&[]],
            &mut transcript,
        )
        .expect("the three-phase proof must verify");
    }

    let overflow = ThreePhaseCircuit {
        commit_final_in_synthesize: true,
        advance_after_completion: true,
    };
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
        let _ = create_proof::<KZGCommitmentScheme<Bn256>, ProverSHPLONK<Bn256>, _, _, _, _>(
            &params,
            &pk,
            &[overflow],
            &[&[]],
            OsRng,
            &mut transcript,
        );
    }));
    assert!(
        rejected.is_err(),
        "a fourth phase transition must be rejected before duplicate commitment"
    );
}
