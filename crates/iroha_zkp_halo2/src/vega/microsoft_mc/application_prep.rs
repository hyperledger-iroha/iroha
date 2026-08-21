//! Governed Figure 9 application-witness commitment preparation.
//!
//! This stage starts only after all eight SHA-step witnesses and the core
//! witness satisfy their governed PK equations.  It exact-matches the derived
//! Hyrax key, establishes one proof-scoped RNG, and commits the padded split
//! sections without allocating their zero suffixes.  It does not construct the
//! semantic 47-round verifier witness and therefore grants no prover authority.

use super::super::{
    VegaT256ScalarV1 as Scalar,
    commitment::{Commitment, CommitmentKey},
    engine::VegaRandomSourceV1,
    figure9::Figure9McMaterial,
};
use super::{
    Figure9ApplicationPrepError,
    prover_key::McProverKeyWire,
    rng::{Figure9RandomError, Figure9StdRng},
    split_adapter::{Figure9SecretScalars, Figure9SplitWitnessAdapter, ValidatedFigure9Witnesses},
    verifier_key::McVerifierKeyWire,
    verify,
    wire::{McCommitment, SplitInstanceWire},
};

const APPLICATION_COMMITMENT_WIDTH: usize = 2_048;
const FIGURE9_STEP_COUNT: usize = 8;
const FIGURE9_APPLICATION_BLINDING_SCALARS: usize = 2_560;

/// Private rest values and segment blindings retained for the downstream fold.
pub(super) struct PreparedPrivateSections {
    pub(super) rest_values: Figure9SecretScalars,
    pub(super) precommitted_blindings: Figure9SecretScalars,
    pub(super) rest_blindings: Figure9SecretScalars,
}

impl PreparedPrivateSections {
    fn clear_secret(&mut self) {
        self.rest_values.clear_secret();
        self.precommitted_blindings.clear_secret();
        self.rest_blindings.clear_secret();
    }
}

/// Exact application state immediately before the semantic Microsoft fold.
///
/// The owner is intentionally move-only.  It borrows the single shared native
/// assignment, owns only eight unpadded rest vectors, and keeps the shared
/// commitment outside every stored split instance so the canonical wire can
/// encode those 256 points exactly once.
pub(super) struct GovernedFigure9ApplicationPrep<'a> {
    pub(super) application_key: CommitmentKey,
    pub(super) shared_witness: &'a [Scalar],
    pub(super) shared_blindings: Figure9SecretScalars,
    pub(super) shared_commitment: McCommitment,
    pub(super) step_private: [PreparedPrivateSections; FIGURE9_STEP_COUNT],
    pub(super) core_private: PreparedPrivateSections,
    pub(super) step_instances: Vec<SplitInstanceWire>,
    pub(super) core_instance: SplitInstanceWire,
    pub(super) rng: Option<Figure9StdRng>,
}

impl Drop for GovernedFigure9ApplicationPrep<'_> {
    fn drop(&mut self) {
        #[cfg(test)]
        let had_secrets = !self.shared_blindings.is_empty()
            || self.step_private.iter().any(|private| {
                !private.rest_values.is_empty()
                    || !private.precommitted_blindings.is_empty()
                    || !private.rest_blindings.is_empty()
            })
            || !self.core_private.precommitted_blindings.is_empty()
            || !self.core_private.rest_blindings.is_empty();

        self.shared_blindings.clear_secret();
        for private in &mut self.step_private {
            private.clear_secret();
        }
        self.core_private.clear_secret();

        #[cfg(test)]
        if had_secrets
            && self.shared_blindings.iter().all(|value| value.is_zero())
            && self.step_private.iter().all(|private| {
                private.rest_values.iter().all(|value| value.is_zero())
                    && private
                        .precommitted_blindings
                        .iter()
                        .all(|value| value.is_zero())
                    && private.rest_blindings.iter().all(|value| value.is_zero())
            })
            && self
                .core_private
                .precommitted_blindings
                .iter()
                .all(|value| value.is_zero())
            && self
                .core_private
                .rest_blindings
                .iter()
                .all(|value| value.is_zero())
        {
            let _ = FIGURE9_APPLICATION_PREP_ZEROIZED_DROPS.try_with(|drops| {
                drops.set(drops.get().saturating_add(1));
            });
        }
    }
}

#[cfg(test)]
std::thread_local! {
    static FIGURE9_APPLICATION_PREP_ZEROIZED_DROPS: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
}

#[cfg(test)]
fn application_prep_zeroized_drop_count() -> usize {
    FIGURE9_APPLICATION_PREP_ZEROIZED_DROPS
        .try_with(core::cell::Cell::get)
        .unwrap_or(0)
}

/// Validate, blind, commit, and transcript-check the nine application inputs.
///
/// The external random source is not reachable until the adapter has checked
/// all nine governed assignments and the derived commitment key has matched the
/// installed PK/VK pair exactly.
pub(super) fn prepare<'a, R: VegaRandomSourceV1>(
    material: &'a Figure9McMaterial,
    step_public_values: &[Vec<Scalar>],
    core_public_values: &'a [Scalar],
    prover_key: &McProverKeyWire,
    verifier_key: &McVerifierKeyWire,
    worker_count: usize,
    random: &mut R,
) -> Result<GovernedFigure9ApplicationPrep<'a>, Figure9ApplicationPrepError> {
    let adapter = Figure9SplitWitnessAdapter::new(material, step_public_values, core_public_values)
        .map_err(Figure9ApplicationPrepError::Split)?;
    let validated = adapter
        .validated_governed_witnesses(
            step_public_values,
            &prover_key.step_shape,
            &prover_key.core_shape,
        )
        .map_err(Figure9ApplicationPrepError::Split)?;

    // These checks deliberately remain after all relation checks and before
    // the sole external seed draw.
    prover_key
        .validate_against(verifier_key)
        .map_err(|_| Figure9ApplicationPrepError::InvalidGovernedKey)?;
    let application_key = verify::derive_and_match_key(&prover_key.application_key)
        .map_err(|_| Figure9ApplicationPrepError::InvalidGovernedKey)?;
    let application_key = application_key
        .with_worker_count(worker_count)
        .map_err(|_| Figure9ApplicationPrepError::InvalidGovernedKey)?;
    if application_key.columns() != APPLICATION_COMMITMENT_WIDTH {
        return Err(Figure9ApplicationPrepError::InvalidGovernedKey);
    }

    let rng = Figure9StdRng::from_external(random).map_err(map_random_error)?;
    GovernedFigure9ApplicationPrep::from_validated(validated, application_key, verifier_key, rng)
}

impl<'a> GovernedFigure9ApplicationPrep<'a> {
    fn from_validated(
        validated: ValidatedFigure9Witnesses<'a>,
        application_key: CommitmentKey,
        verifier_key: &McVerifierKeyWire,
        mut rng: Figure9StdRng,
    ) -> Result<Self, Figure9ApplicationPrepError> {
        let ValidatedFigure9Witnesses {
            shared_witness,
            step_public_values,
            step_rest_values,
            core_public_values,
        } = validated;

        let shared_blindings = sample_blindings(
            &mut rng,
            verifier_key.step_shape.shared / APPLICATION_COMMITMENT_WIDTH,
        );
        let shared_commitment = commit_segment(
            &application_key,
            shared_witness,
            verifier_key.step_shape.shared,
            shared_blindings.as_slice(),
        )?;

        let mut step_private = Vec::with_capacity(FIGURE9_STEP_COUNT);
        let mut step_instances = Vec::with_capacity(FIGURE9_STEP_COUNT);
        for (public_values, rest_values) in step_public_values
            .into_iter()
            .zip(step_rest_values.into_iter())
        {
            let precommitted_blindings = sample_blindings(
                &mut rng,
                verifier_key.step_shape.precommitted / APPLICATION_COMMITMENT_WIDTH,
            );
            let rest_blindings = sample_blindings(
                &mut rng,
                verifier_key.step_shape.rest / APPLICATION_COMMITMENT_WIDTH,
            );
            let precommitted = commit_segment(
                &application_key,
                &public_values,
                verifier_key.step_shape.precommitted,
                precommitted_blindings.as_slice(),
            )?;
            let rest = commit_segment(
                &application_key,
                rest_values.as_slice(),
                verifier_key.step_shape.rest,
                rest_blindings.as_slice(),
            )?;
            step_instances.push(SplitInstanceWire {
                shared: None,
                precommitted: Some(precommitted),
                rest,
                public_values,
                challenges: Vec::new(),
            });
            step_private.push(PreparedPrivateSections {
                rest_values,
                precommitted_blindings,
                rest_blindings,
            });
        }
        let step_private: [PreparedPrivateSections; FIGURE9_STEP_COUNT] =
            step_private
                .try_into()
                .map_err(|_| Figure9ApplicationPrepError::InvalidGovernedKey)?;

        let core_precommitted_blindings = sample_blindings(
            &mut rng,
            verifier_key.core_shape.precommitted / APPLICATION_COMMITMENT_WIDTH,
        );
        let core_rest_blindings = sample_blindings(
            &mut rng,
            verifier_key.core_shape.rest / APPLICATION_COMMITMENT_WIDTH,
        );
        let core_precommitted = commit_segment(
            &application_key,
            &core_public_values,
            verifier_key.core_shape.precommitted,
            core_precommitted_blindings.as_slice(),
        )?;
        // The core's rest section is padding only; no zero vector is allocated.
        let core_rest = commit_segment(
            &application_key,
            &[],
            verifier_key.core_shape.rest,
            core_rest_blindings.as_slice(),
        )?;
        let core_instance = SplitInstanceWire {
            shared: None,
            precommitted: Some(core_precommitted),
            rest: core_rest,
            public_values: core_public_values,
            challenges: Vec::new(),
        };
        let core_private = PreparedPrivateSections {
            rest_values: Figure9SecretScalars::with_capacity(0),
            precommitted_blindings: core_precommitted_blindings,
            rest_blindings: core_rest_blindings,
        };

        verify::validate_application_instance_transcripts(
            Some(&shared_commitment),
            &step_instances,
            &core_instance,
            verifier_key,
        )
        .map_err(|_| Figure9ApplicationPrepError::Transcript)?;

        let prepared = Self {
            application_key,
            shared_witness,
            shared_blindings,
            shared_commitment,
            step_private,
            core_private,
            step_instances,
            core_instance,
            rng: Some(rng),
        };
        prepared.validate_exact_counts(verifier_key)?;
        Ok(prepared)
    }

    fn validate_exact_counts(
        &self,
        verifier_key: &McVerifierKeyWire,
    ) -> Result<(), Figure9ApplicationPrepError> {
        let step = &verifier_key.step_shape;
        let core = &verifier_key.core_shape;
        let total_blindings = self
            .shared_blindings
            .len()
            .checked_add(
                self.step_private
                    .iter()
                    .map(|private| {
                        private.precommitted_blindings.len() + private.rest_blindings.len()
                    })
                    .sum::<usize>(),
            )
            .and_then(|count| {
                count.checked_add(
                    self.core_private.precommitted_blindings.len()
                        + self.core_private.rest_blindings.len(),
                )
            })
            .ok_or(Figure9ApplicationPrepError::InvalidGovernedKey)?;
        if self.application_key.columns() != APPLICATION_COMMITMENT_WIDTH
            || self.shared_witness.len() != step.shared_unpadded
            || self.shared_commitment.points.len() != step.shared / APPLICATION_COMMITMENT_WIDTH
            || self.step_instances.len() != FIGURE9_STEP_COUNT
            || self.step_instances.len() != verifier_key.num_steps
            || self
                .step_instances
                .iter()
                .enumerate()
                .any(|(index, instance)| {
                    instance.shared.is_some()
                        || instance.challenges.len() != step.challenges
                        || instance.public_values != [Scalar::from_u64(index as u64)]
                        || instance
                            .precommitted
                            .as_ref()
                            .map(|value| value.points.len())
                            != Some(step.precommitted / APPLICATION_COMMITMENT_WIDTH)
                        || instance.rest.points.len() != step.rest / APPLICATION_COMMITMENT_WIDTH
                })
            || self
                .step_private
                .iter()
                .any(|private| private.rest_values.len() != step.rest_unpadded)
            || self.core_instance.shared.is_some()
            || self.core_instance.challenges.len() != core.challenges
            || self.core_instance.public_values.len() != core.public_values
            || self
                .core_instance
                .precommitted
                .as_ref()
                .map(|value| value.points.len())
                != Some(core.precommitted / APPLICATION_COMMITMENT_WIDTH)
            || self.core_instance.rest.points.len() != core.rest / APPLICATION_COMMITMENT_WIDTH
            || !self.core_private.rest_values.is_empty()
            || total_blindings != FIGURE9_APPLICATION_BLINDING_SCALARS
        {
            return Err(Figure9ApplicationPrepError::InvalidGovernedKey);
        }
        Ok(())
    }
}

fn sample_blindings(rng: &mut Figure9StdRng, count: usize) -> Figure9SecretScalars {
    let mut blindings = Figure9SecretScalars::with_capacity(count);
    for _ in 0..count {
        blindings.push(rng.scalar());
    }
    blindings
}

fn commit_segment(
    key: &CommitmentKey,
    values: &[Scalar],
    padded_len: usize,
    blindings: &[Scalar],
) -> Result<McCommitment, Figure9ApplicationPrepError> {
    let commitment = key
        .commit_padded_prefix(values, padded_len, blindings)
        .map_err(|_| Figure9ApplicationPrepError::Commitment)?;
    Ok(commitment_to_wire(commitment))
}

fn commitment_to_wire(commitment: Commitment) -> McCommitment {
    McCommitment {
        points: commitment.into_points(),
    }
}

fn map_random_error(error: Figure9RandomError) -> Figure9ApplicationPrepError {
    match error {
        Figure9RandomError::Source(error) => Figure9ApplicationPrepError::RandomSource(error),
        Figure9RandomError::DegenerateOrReused => Figure9ApplicationPrepError::DegenerateRandomness,
    }
}

#[cfg(test)]
mod tests {
    use super::super::verifier_key::{SparseMatrixWire, SplitShapeWire};
    use super::*;
    use crate::vega::{
        VegaT256PointV1 as Point, commitment::CommitmentError, transcript::VegaTranscriptV1,
    };

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    #[test]
    fn padded_segment_commitment_streams_zeros_and_binds_values_and_blindings() {
        let key = CommitmentKey::derive(b"ck", 4)
            .and_then(|key| key.with_worker_count(2))
            .expect("bounded commitment key");
        let values = [s(2), s(3), Scalar::zero(), s(5), Scalar::zero()];
        let blindings = [s(7), s(11), s(13)];
        let streamed = key
            .commit_padded_prefix(&values, 12, &blindings)
            .expect("streamed padded commitment");
        let mut explicit = values.to_vec();
        explicit.resize(12, Scalar::zero());
        assert_eq!(
            streamed,
            key.commit(&explicit, &blindings)
                .expect("materialized reference commitment")
        );

        let mut changed_values = values;
        changed_values[1] += Scalar::one();
        assert_ne!(
            streamed,
            key.commit_padded_prefix(&changed_values, 12, &blindings)
                .expect("mutated value commitment")
        );
        let mut changed_blindings = blindings;
        changed_blindings[2] += Scalar::one();
        assert_ne!(
            streamed,
            key.commit_padded_prefix(&values, 12, &changed_blindings)
                .expect("mutated blinding commitment")
        );
        assert_eq!(
            key.commit_padded_prefix(&values, 11, &blindings),
            Err(CommitmentError::InvalidDimension)
        );
        assert_eq!(
            key.commit_padded_prefix(&values, 12, &blindings[..2]),
            Err(CommitmentError::InvalidDimension)
        );
    }

    #[test]
    fn exact_key_match_rejects_generator_order_and_hiding_mutations() {
        let key = CommitmentKey::derive(b"ck", 4).expect("derived key");
        let mut wire = super::super::verifier_key::HyraxKeyWire {
            generators: key.generators().to_vec(),
            hiding_generator: key.hiding_generator(),
            columns: key.columns(),
        };
        verify::derive_and_match_key(&wire).expect("exact key matches");
        wire.generators.swap(0, 1);
        assert!(verify::derive_and_match_key(&wire).is_err());
        wire.generators.swap(0, 1);
        wire.hiding_generator = wire.generators[0];
        assert!(verify::derive_and_match_key(&wire).is_err());
    }

    #[test]
    fn split_transcript_rejects_commitment_order_and_challenge_mutations() {
        let empty_matrix = || SparseMatrixWire {
            data: Vec::new(),
            indices: Vec::new(),
            row_offsets: vec![0, 0],
            columns: 6_147,
        };
        let shape = SplitShapeWire {
            constraints: 1,
            constraints_unpadded: 0,
            shared_unpadded: 0,
            precommitted_unpadded: 0,
            rest_unpadded: 0,
            shared: APPLICATION_COMMITMENT_WIDTH,
            precommitted: APPLICATION_COMMITMENT_WIDTH,
            rest: APPLICATION_COMMITMENT_WIDTH,
            public_values: 1,
            challenges: 1,
            a: empty_matrix(),
            b: empty_matrix(),
            c: empty_matrix(),
        };
        let generator = Point::canonical_generator().expect("canonical T256 generator");
        let shared = McCommitment {
            points: vec![generator],
        };
        let precommitted = McCommitment {
            points: vec![generator.mul_scalar(s(2))],
        };
        let rest = McCommitment {
            points: vec![generator.mul_scalar(s(3))],
        };
        let public_values = vec![s(17)];
        let transcript = |values: &[Scalar]| {
            let mut transcript = VegaTranscriptV1::new_neutron_nova();
            transcript
                .absorb_scalars(b"public_values", values)
                .expect("bounded public transcript");
            transcript
        };
        let mut derive = transcript(&public_values);
        derive
            .absorb_commitment(b"comm_W_shared", &shared.to_local().expect("shared"))
            .expect("bounded shared transcript");
        derive
            .absorb_commitment(
                b"comm_W_precommitted",
                &precommitted.to_local().expect("precommitted"),
            )
            .expect("bounded precommit transcript");
        let challenge = derive.squeeze(b"challenge").expect("one challenge");
        let instance = SplitInstanceWire {
            shared: None,
            precommitted: Some(precommitted.clone()),
            rest: rest.clone(),
            public_values,
            challenges: vec![challenge],
        };
        verify::validate_split_instance_with_shared(
            &instance,
            Some(&shared),
            &shape,
            &mut transcript(&instance.public_values),
        )
        .expect("canonical split transcript");

        let mut wrong_challenge = instance.clone();
        wrong_challenge.challenges[0] += Scalar::one();
        assert!(
            verify::validate_split_instance_with_shared(
                &wrong_challenge,
                Some(&shared),
                &shape,
                &mut transcript(&wrong_challenge.public_values),
            )
            .is_err()
        );
        let mut wrong_order = instance;
        wrong_order.precommitted = Some(rest);
        wrong_order.rest = precommitted;
        assert!(
            verify::validate_split_instance_with_shared(
                &wrong_order,
                Some(&shared),
                &shape,
                &mut transcript(&wrong_order.public_values),
            )
            .is_err()
        );
    }

    #[test]
    fn exact_application_counts_pin_shared_once_and_all_row_blindings() {
        assert_eq!(524_288 / APPLICATION_COMMITMENT_WIDTH, 256);
        assert_eq!(2_048 / APPLICATION_COMMITMENT_WIDTH, 1);
        assert_eq!(522_240 / APPLICATION_COMMITMENT_WIDTH, 255);
        assert_eq!(256 + (FIGURE9_STEP_COUNT + 1) * (1 + 255), 2_560);
        assert_eq!(FIGURE9_APPLICATION_BLINDING_SCALARS, 2_560);
    }

    fn drop_test_secrets(values: &[u64]) -> Figure9SecretScalars {
        let mut secrets = Figure9SecretScalars::with_capacity(values.len());
        for value in values {
            secrets.push(s(*value));
        }
        secrets
    }

    fn drop_test_private() -> PreparedPrivateSections {
        PreparedPrivateSections {
            rest_values: drop_test_secrets(&[3, 5]),
            precommitted_blindings: drop_test_secrets(&[7]),
            rest_blindings: drop_test_secrets(&[11, 13]),
        }
    }

    fn drop_test_prep() -> GovernedFigure9ApplicationPrep<'static> {
        let point = Point::canonical_generator().expect("canonical T256 generator");
        let commitment = McCommitment {
            points: vec![point],
        };
        GovernedFigure9ApplicationPrep {
            application_key: CommitmentKey::derive(b"figure9-prep-drop-test", 1)
                .expect("test commitment key"),
            shared_witness: &[],
            shared_blindings: drop_test_secrets(&[17, 19]),
            shared_commitment: commitment.clone(),
            step_private: core::array::from_fn(|_| drop_test_private()),
            core_private: drop_test_private(),
            step_instances: Vec::new(),
            core_instance: SplitInstanceWire {
                shared: None,
                precommitted: None,
                rest: commitment,
                public_values: Vec::new(),
                challenges: Vec::new(),
            },
            rng: Some(Figure9StdRng::from_seed([0x5a; 32])),
        }
    }

    #[test]
    fn application_prep_zeroizes_private_state_on_success_error_and_unwind() {
        fn error_path() -> Result<(), ()> {
            let _prepared = drop_test_prep();
            Err(())
        }

        let before_success = application_prep_zeroized_drop_count();
        drop(drop_test_prep());
        assert_eq!(application_prep_zeroized_drop_count(), before_success + 1);

        let before_error = application_prep_zeroized_drop_count();
        assert_eq!(error_path(), Err(()));
        assert_eq!(application_prep_zeroized_drop_count(), before_error + 1);

        let before_unwind = application_prep_zeroized_drop_count();
        let unwind = std::panic::catch_unwind(|| {
            let _prepared = drop_test_prep();
            panic!("injected Figure 9 application-prep unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(application_prep_zeroized_drop_count(), before_unwind + 1);
    }

    #[test]
    fn source_contract_keeps_prep_move_only_streamed_and_non_authorizing() {
        let source = include_str!("application_prep.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production prep source");
        let validation = production
            .find("validated_governed_witnesses")
            .expect("nine-witness validation");
        let key_match = production
            .find("derive_and_match_key")
            .expect("derived key match");
        let random = production
            .find("Figure9StdRng::from_external")
            .expect("sole external RNG boundary");
        assert!(validation < key_match && key_match < random);
        assert!(production.contains("commit_padded_prefix"));
        assert!(production.contains("shared: None"));
        assert!(production.contains("validate_application_instance_transcripts"));
        assert!(production.contains("impl Drop for GovernedFigure9ApplicationPrep<'_>"));
        assert!(production.contains(
            "fn sample_blindings(rng: &mut Figure9StdRng, count: usize) -> Figure9SecretScalars"
        ));
        assert!(
            !production
                .contains("#[derive(Clone)]\npub(super) struct GovernedFigure9ApplicationPrep")
        );
        assert!(!production.contains("VegaMcZkSNARK"));
        assert!(!production.contains("McProofWire"));
    }
}
