//! Positive whole-PLONK tests for a satisfiable stored-prefix/IPA continuation fixture.
//!
//! The relation combines selected square constraints, a fixed-table lookup and copy constraints
//! to three public instance columns. The test-only IPA adapter retains the genuine protocol
//! owner; this is not authenticated Core integration, guarded inner-IPA or hardware evidence.

use super::*;
use crate::{
    plonk::{VerifyingKey, verify_proof},
    poly::{VerificationStrategy, ipa::strategy::SingleStrategy},
};

#[derive(Clone, Copy)]
struct SquareConfig {
    a: Column<Advice>,
    square: Column<Advice>,
    table: Column<Fixed>,
    selected: Selector,
    instances: [Column<Instance>; 3],
}
struct SquareCircuit<C: CurveAffine>(Producer<C>);
impl<C: CurveAffine> SquareCircuit<C> {
    fn run(
        &self,
        config: SquareConfig,
        mut layouter: impl Layouter<C::Scalar>,
        measurement: bool,
    ) -> Result<(), Error> {
        if measurement {
            self.0.shared.log.lock().unwrap().measurement_passes += 1;
        } else {
            self.0.shared.log.lock().unwrap().synthesis_passes += 1;
        }
        let copied = layouter.assign_region(
            || "satisfiable square and lookup rows",
            |mut region| {
                let mut copied = Vec::new();
                for row in 0..4 {
                    config.selected.enable(&mut region, row)?;
                    let value = C::Scalar::from((row + 2) as u64);
                    let a =
                        region.assign_advice_discarding_value(config.a, row, Value::known(value));
                    let square = region.assign_advice_discarding_value(
                        config.square,
                        row,
                        Value::known(value.square()),
                    );
                    region.assign_fixed(config.table, row, value);
                    if row == 0 {
                        copied.extend([a, square]);
                    } else if row == 1 {
                        copied.push(a);
                    }
                }
                Ok(copied)
            },
        )?;
        for (cell, instance) in copied.into_iter().zip(config.instances) {
            layouter.constrain_instance(cell, instance, 0);
        }
        Ok(())
    }
}
impl<C: CurveAffine> Circuit<C::Scalar> for SquareCircuit<C> {
    type Config = SquareConfig;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self(Producer::new(&self.0.shared, 3))
    }
    fn configure(meta: &mut ConstraintSystem<C::Scalar>) -> Self::Config {
        let config = SquareConfig {
            a: meta.advice_column(),
            square: meta.advice_column(),
            table: meta.fixed_column(),
            selected: meta.complex_selector(),
            instances: [
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
            ],
        };
        meta.set_minimum_degree(6);
        meta.enable_equality(config.a);
        meta.enable_equality(config.square);
        for instance in config.instances {
            meta.enable_equality(instance);
        }
        meta.create_gate("selected square is correct", |meta| {
            let selected = meta.query_selector(config.selected);
            let a = meta.query_advice(config.a, Rotation::cur());
            let square = meta.query_advice(config.square, Rotation::cur());
            vec![selected * (a.clone() * a - square)]
        });
        meta.lookup_any("selected square input belongs to fixed table", |meta| {
            let selected = meta.query_selector(config.selected);
            let a = meta.query_advice(config.a, Rotation::cur());
            let table = meta.query_fixed(config.table, Rotation::cur());
            vec![(selected * a, table)]
        });
        config
    }
    fn synthesize_for_measurement(
        &self,
        config: Self::Config,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.run(config, layouter, true)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.run(config, layouter, false)
    }
}

fn verify_square<C: CurveAffine, const Q: bool, const M: u64>(
    params: &ParamsIPA<C>,
    vk: &VerifyingKey<C>,
    proof: &[u8],
    values: &[Vec<C::Scalar>],
) -> bool
where
    C::Scalar: WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let mut transcript = Blake2bRead::<_, C, Challenge255<C>>::init(proof);
    verify_proof::<IPACommitmentScheme<C>, VerifierIPA<'_, C, Q, M>, _, _, _>(
        params,
        vk,
        SingleStrategy::<C, Q, M>::new(params),
        &[&instances],
        &mut transcript,
    )
    .is_ok()
}

fn square_proof<C, const Q: bool, const M: u64>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    let params = ParamsIPA::<C>::new(4);
    let key_shared = Shared::<C>::new();
    let key_circuit = SquareCircuit(Producer::new(&key_shared, 3));
    let vk = keygen_vk_custom(&params, &key_circuit, true).unwrap();
    let pk = keygen_pk(&params, vk.clone(), &key_circuit).unwrap();
    assert!(!pk.vk.cs.lookups.is_empty());
    assert!(!pk.vk.cs.permutation.columns.is_empty());
    assert_eq!(pk.vk.cs.num_advice_columns, 2);
    let values = [2, 4, 3].map(|value| vec![C::Scalar::from(value)]).to_vec();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let ordinary = Shared::<C>::new();
    let mut dense_transcript = RecordingTranscript::new(&ordinary);
    let dense_vk = create_proof_consuming::<
        IPACommitmentScheme<C>,
        ProverIPA<'_, C, Q, M>,
        Challenge255<C>,
        _,
        _,
        _,
    >(
        &params,
        pk.clone(),
        SquareCircuit(Producer::new(&ordinary, 3)),
        &[&instances],
        Rng(Arc::clone(&ordinary)),
        &mut dense_transcript,
    )
    .unwrap();
    let dense_bytes = dense_transcript.inner.clone().finalize();
    assert!(verify_square::<C, Q, M>(
        &params,
        &dense_vk,
        &dense_bytes,
        &values
    ));
    let shared = Shared::<C>::new();
    let storage = Controls::new();
    let control = OpeningControls::new(&storage);
    let coefficient =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Scripted<C>, _, Q, M>(
            &params,
            pk,
            SquareCircuit(Producer::new(&shared, 3)),
            &instances,
            InverseProvider {
                inner: Provider::new(&shared),
                controls: Arc::clone(&storage),
            },
            OpeningRng {
                inner: Rng(Arc::clone(&shared)),
                controls: Arc::clone(&control),
            },
            OpeningTranscript {
                inner: RecordingTranscript::new(&shared),
                controls: Arc::clone(&control),
            },
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap()
        .compress_lookups(1 << 26)
        .unwrap()
        .sort_lookup_values(1 << 26)
        .unwrap()
        .prepare_lookup_membership(1 << 26)
        .unwrap()
        .commit_permuted_lookups(1 << 26)
        .unwrap()
        .commit_products(1 << 26)
        .unwrap()
        .commit_vanishing_and_stage_coefficients(1 << 26)
        .unwrap()
        .evaluate_quotient_numerator(1 << 26)
        .unwrap()
        .stage_quotient_coefficients(1 << 26)
        .unwrap();
    let input = coefficient
        .commit_quotient(1 << 26)
        .unwrap()
        .evaluate_and_plan(1 << 26)
        .unwrap();
    opening::take_observations();
    evaluations::take_clear_observations();
    let finished = input
        .prepare_ipa_opening(1 << 26)
        .unwrap()
        .finish_ordinary_ipa_for_test()
        .unwrap();
    let bytes = finished
        .observed_inner()
        .inner
        .inner
        .transcript
        .inner
        .inner
        .clone()
        .finalize();
    assert_eq!(bytes, dense_bytes);
    assert_eq!(
        shared.log.lock().unwrap().events,
        ordinary.log.lock().unwrap().events
    );
    let mut actual_next = [0; 64];
    let mut expected_next = [0; 64];
    shared
        .rng
        .lock()
        .unwrap()
        .clone()
        .fill_bytes(&mut actual_next);
    ordinary
        .rng
        .lock()
        .unwrap()
        .clone()
        .fill_bytes(&mut expected_next);
    assert_eq!(actual_next, expected_next);
    assert!(verify_square::<C, Q, M>(&params, &vk, &bytes, &values));
    for column in 0..values.len() {
        let mut altered = values.clone();
        altered[column][0] += C::Scalar::ONE;
        assert!(!verify_square::<C, Q, M>(&params, &vk, &bytes, &altered));
    }
    for offset in [0, bytes.len() / 2, bytes.len() - 1] {
        let mut altered = bytes.clone();
        altered[offset] ^= 1;
        assert!(!verify_square::<C, Q, M>(&params, &vk, &altered, &values));
    }
    assert!(!verify_square::<C, Q, M>(
        &params,
        &vk,
        &bytes[..bytes.len() - 1],
        &values
    ));
    let (cleared, zero) = evaluations::take_clear_observations();
    assert!(cleared > params.n() as usize && zero);
    drop(finished);
    assert!(storage.bank.lock().unwrap().live.is_empty());
    assert_dropped(&shared);
    opening::take_observations();
}

#[test]
fn both_pasta_satisfiable_stored_square_lookup_copy_proofs_match_dense_and_verify_all_instance_modes()
 {
    square_proof::<EqAffine, false, 0>();
    square_proof::<EqAffine, true, 0>();
    square_proof::<EqAffine, true, 6>();
    square_proof::<EpAffine, false, 0>();
    square_proof::<EpAffine, true, 0>();
    square_proof::<EpAffine, true, 6>();
}

#[path = "inner_ipa_tests.rs"]
mod inner_ipa;
