//! Single-phase failure latching and original V1 floor-planner regression oracles.
//!
//! The parent recording store is deliberately plaintext test infrastructure. These tests do
//! not qualify the encrypted store, an admitted production circuit, a complete proof, or RSS.

use super::super::synthesis::{StoredSinglePhaseAssignmentV1, StoredSynthesisErrorV1};
use super::*;
use crate::{
    circuit::{Layouter, Value, floor_planner::V1},
    plonk::{
        Advice, Assignment, Challenge, Circuit, Column, Error, Fixed, FloorPlanner, Instance,
        Selector,
    },
};

#[derive(Clone, Debug)]
struct Config {
    advice: [Column<Advice>; 2],
    fixed: Column<Fixed>,
    instance: Column<Instance>,
    selector: Selector,
    challenge: Challenge,
}

fn configure<F: Field>(meta: &mut ConstraintSystem<F>) -> Config {
    let advice = [meta.advice_column(), meta.advice_column()];
    let fixed = meta.fixed_column();
    let instance = meta.instance_column();
    meta.enable_equality(advice[0]);
    meta.enable_equality(advice[1]);
    meta.enable_equality(instance);
    Config {
        advice,
        fixed,
        instance,
        selector: meta.selector(),
        challenge: meta.challenge_usable_after(FirstPhase),
    }
}

fn bridge<'params, 'instances, C>(
    params: &'params ParamsIPA<C>,
    backend: &Rc<Backend>,
    instances: &'instances [&'instances [C::Scalar]],
) -> (
    StoredSinglePhaseAssignmentV1<'params, 'instances, C, Writer>,
    Config,
)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
{
    let mut meta = ConstraintSystem::default();
    let config = configure(&mut meta);
    let domain = EvaluationDomain::new(4, params.k());
    let plan = admit_stored_phase_plan_v1(params, &domain, &meta).unwrap();
    let columns = writers(&plan, 0, 11, backend);
    (
        StoredSinglePhaseAssignmentV1::new(plan, columns, instances).unwrap(),
        config,
    )
}

fn assignments_match_direct_phase<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    for k in [4, 9] {
        let params = ParamsIPA::<C>::new(k);
        let backend = Rc::new(Backend::default());
        let baseline_backend = Rc::new(Backend::default());
        let values = [C::Scalar::from(17)];
        let instances = [&values[..]];
        let (mut adapter, config) = bridge(&params, &backend, &instances);
        let mut meta = ConstraintSystem::default();
        configure(&mut meta);
        let domain = EvaluationDomain::new(4, k);
        let plan = admit_stored_phase_plan_v1(&params, &domain, &meta).unwrap();
        let usable = plan.usable_rows;
        let raw = writers(&plan, 0, 11, &baseline_backend);
        let mut baseline = StoredPhaseAssignmentsV1::begin(plan, raw).unwrap();
        assert_eq!(
            adapter
                .query_instance(config.instance, 0)
                .unwrap()
                .assign()
                .unwrap(),
            values[0]
        );
        assert!(adapter.get_challenge(config.challenge).assign().is_err());
        for column in 0..2 {
            for (row, value) in phase_inputs(column, usable, C::Scalar::from(3)) {
                adapter.assign_advice_discarding_value(
                    config.advice[column],
                    row,
                    Value::known(value),
                );
                baseline
                    .assign_discarding_value(column, row, value)
                    .unwrap();
            }
        }
        // Existing proving semantics take these values/constraints from the actual proving key.
        adapter
            .enable_selector(|| "valid", &config.selector, 0)
            .unwrap();
        adapter.assign_fixed(
            config.fixed,
            (1 << k) - 1,
            Assigned::Trivial(C::Scalar::ONE),
        );
        adapter.copy(config.advice[0].into(), 0, config.instance.into(), 0);
        adapter
            .fill_from_row(config.fixed, 1 << k, Value::unknown())
            .unwrap();
        let mut rng = ChaCha20Rng::from_seed([47; 32]);
        let mut expected_rng = rng.clone();
        let prepared = adapter
            .into_assignments(Ok(()))
            .unwrap()
            .finish(&mut rng)
            .unwrap();
        let expected = baseline.finish(&mut expected_rng).unwrap();
        assert_eq!(prepared.commitments, expected.commitments);
        assert_eq!(
            backend.record.borrow().sealed,
            baseline_backend.record.borrow().sealed
        );
        for (actual, expected) in prepared.columns.iter().zip(&expected.columns) {
            assert_eq!(actual.layout, expected.layout);
            assert_eq!(actual.blind.0.0, expected.blind.0.0);
        }
        let mut transcript = Blake2bWrite::<Vec<u8>, C, Challenge255<C>>::init(Vec::new());
        let mut expected_transcript = Blake2bWrite::<Vec<u8>, C, Challenge255<C>>::init(Vec::new());
        let committed = prepared.absorb(&mut transcript).unwrap();
        let expected_committed = expected.absorb(&mut expected_transcript).unwrap();
        assert_eq!(committed.challenge(0), expected_committed.challenge(0));
        assert_eq!(transcript.finalize(), expected_transcript.finalize());
        let mut next = [0; 64];
        let mut expected_next = [0; 64];
        rng.fill_bytes(&mut next);
        expected_rng.fill_bytes(&mut expected_next);
        assert_eq!(next, expected_next);
        drop(committed);
        assert_eq!(backend.record.borrow().snapshot_drops, 2);
    }
}

#[test]
fn eq_bridge_matches_direct_phase_points_blinds_transcript_and_rng() {
    assignments_match_direct_phase::<EqAffine>();
}

#[test]
fn ep_bridge_matches_direct_phase_points_blinds_transcript_and_rng() {
    assignments_match_direct_phase::<EpAffine>();
}

#[test]
fn constructor_refuses_instance_shape_and_multiple_phases_before_writes() {
    let params = ParamsIPA::<EqAffine>::new(4);
    let domain = EvaluationDomain::new(4, 4);
    for case in 0..4 {
        let mut meta = ConstraintSystem::default();
        configure(&mut meta);
        if case == 3 {
            meta.advice_column_in(SecondPhase);
        }
        let plan = admit_stored_phase_plan_v1(&params, &domain, &meta).unwrap();
        let backend = Rc::new(Backend::default());
        let columns = writers(&plan, 0, 11, &backend);
        let count = columns.len();
        let too_long = vec![Fp::ONE; 1 << 4];
        let mut instances: Vec<&[Fp]> = match case {
            0 => vec![],
            1 => vec![&[], &[]],
            2 => vec![&too_long],
            _ => vec![&[]],
        };
        let expected = if case == 3 {
            StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::Admission)
        } else {
            StoredSynthesisErrorV1::Instances
        };
        assert!(
            matches!(StoredSinglePhaseAssignmentV1::new(plan, columns, &instances), Err(error) if error == expected)
        );
        assert_eq!(backend.record.borrow().writer_drops, count);
        assert!(backend.record.borrow().sealed.is_empty());
        instances.clear();
    }
}

#[test]
fn ignored_reference_unknown_and_phase_requests_destroy_every_writer() {
    let params = ParamsIPA::<EqAffine>::new(4);
    for case in 0..3 {
        let backend = Rc::new(Backend::default());
        let instances = [&[][..]];
        let (mut adapter, config) = bridge(&params, &backend, &instances);
        let expected = match case {
            0 => {
                assert!(
                    adapter
                        .assign_advice(
                            config.advice[0],
                            0,
                            Value::known(Assigned::Trivial(Fp::ONE))
                        )
                        .assign()
                        .is_err()
                );
                StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::ReferenceReturn)
            }
            1 => {
                adapter.assign_advice_discarding_value(config.advice[0], 0, Value::unknown());
                StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::UnknownValue)
            }
            _ => {
                adapter.next_phase();
                StoredSynthesisErrorV1::PhaseAdvance
            }
        };
        assert_eq!(backend.record.borrow().writer_drops, 2);
        adapter.assign_advice_discarding_value(
            config.advice[1],
            0,
            Value::known(Assigned::Trivial(Fp::ONE)),
        );
        assert!(adapter.query_instance(config.instance, 0).is_err());
        assert!(matches!(adapter.into_assignments(Ok(())), Err(error) if error == expected));
        assert!(backend.record.borrow().sealed.is_empty());
    }
}

#[test]
fn ignored_backward_duplicate_bounds_and_wrong_typed_phase_cannot_recover() {
    let params = ParamsIPA::<EqAffine>::new(4);
    for case in 0..5 {
        let backend = Rc::new(Backend::default());
        let instances = [&[][..]];
        let (mut adapter, config) = bridge(&params, &backend, &instances);
        adapter.assign_advice_discarding_value(
            config.advice[0],
            3,
            Value::known(Assigned::Trivial(Fp::ONE)),
        );
        let (column, row, expected) = match case {
            0 => (
                config.advice[0],
                3,
                StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::Assignment(
                    StoredAssignmentErrorV1::NonMonotonic,
                )),
            ),
            1 => (
                config.advice[0],
                2,
                StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::Assignment(
                    StoredAssignmentErrorV1::NonMonotonic,
                )),
            ),
            2 => (
                config.advice[0],
                16,
                StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::Assignment(
                    StoredAssignmentErrorV1::Row,
                )),
            ),
            3 => (
                Column::new(2, Advice::default()),
                0,
                StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::Admission),
            ),
            _ => (
                Column::new(0, Advice::new(SecondPhase)),
                4,
                StoredSynthesisErrorV1::Coordinate,
            ),
        };
        adapter.assign_advice_discarding_value(
            column,
            row,
            Value::known(Assigned::Trivial(Fp::ONE)),
        );
        assert_eq!(backend.record.borrow().writer_drops, 2);
        assert!(matches!(adapter.into_assignments(Ok(())), Err(error) if error == expected));
    }
}

#[test]
fn ignored_instance_query_failure_invalidates_the_mutable_phase_through_shared_access() {
    let params = ParamsIPA::<EqAffine>::new(4);
    for case in 0..3 {
        let backend = Rc::new(Backend::default());
        let values = [Fp::from(19)];
        let instances = [&values[..]];
        let (mut adapter, config) = bridge(&params, &backend, &instances);
        let (column, row) = match case {
            0 => (config.instance, 1),
            1 => (Column::new(1, Instance), 0),
            _ => (config.instance, 16),
        };
        let failure = adapter.query_instance(column, row);
        if case == 2 {
            assert!(matches!(
                failure,
                Err(Error::NotEnoughRowsAvailable { current_k: 4 })
            ));
        } else {
            assert!(matches!(failure, Err(Error::BoundsFailure)));
        }
        assert_eq!(backend.record.borrow().writer_drops, 2);
        adapter.assign_advice_discarding_value(
            config.advice[0],
            0,
            Value::known(Assigned::Trivial(Fp::ONE)),
        );
        assert!(matches!(
            adapter.into_assignments(Ok(())),
            Err(StoredSynthesisErrorV1::Instances)
        ));
    }
}

#[test]
fn invalid_challenge_queries_cannot_be_ignored() {
    let params = ParamsIPA::<EqAffine>::new(4);
    for later_phase in [false, true] {
        let backend = Rc::new(Backend::default());
        let instances = [&[][..]];
        let (adapter, _) = bridge(&params, &backend, &instances);
        let mut other = ConstraintSystem::<Fp>::default();
        other.advice_column();
        let challenge = if later_phase {
            other.advice_column_in(SecondPhase);
            other.challenge_usable_after(SecondPhase)
        } else {
            other.challenge_usable_after(FirstPhase);
            other.challenge_usable_after(FirstPhase)
        };
        assert!(adapter.get_challenge(challenge).assign().is_err());
        assert_eq!(backend.record.borrow().writer_drops, 2);
        assert!(matches!(
            adapter.into_assignments(Ok(())),
            Err(StoredSynthesisErrorV1::Coordinate)
        ));
    }
}

#[test]
fn backend_error_or_caught_unwind_cannot_return_an_owner() {
    let params = ParamsIPA::<EqAffine>::new(9);
    for panic in [false, true] {
        let backend = Rc::new(Backend::default());
        if panic {
            backend.record.borrow_mut().panic_write = Some((0, 0));
        } else {
            backend.record.borrow_mut().fail_write = Some((0, 0));
        }
        let instances = [&[][..]];
        let (mut adapter, config) = bridge(&params, &backend, &instances);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            adapter.assign_advice_discarding_value(
                config.advice[0],
                256,
                Value::known(Assigned::Trivial(Fp::ONE)),
            );
        }));
        assert_eq!(outcome.is_err(), panic);
        assert_eq!(backend.record.borrow().writer_drops, 2);
        let expected = if panic {
            StoredPhaseErrorV1::Poisoned
        } else {
            StoredPhaseErrorV1::Assignment(StoredAssignmentErrorV1::Store(
                StoredAdviceErrorV1::Storage,
            ))
        };
        assert!(
            matches!(adapter.into_assignments(Ok(())), Err(StoredSynthesisErrorV1::Phase(error)) if error == expected)
        );
    }
}

#[test]
fn returned_synthesis_error_and_early_drop_release_all_writers() {
    let params = ParamsIPA::<EqAffine>::new(4);
    for early_drop in [false, true] {
        let backend = Rc::new(Backend::default());
        let instances = [&[][..]];
        let (mut adapter, config) = bridge(&params, &backend, &instances);
        adapter.assign_advice_discarding_value(
            config.advice[0],
            0,
            Value::known(Assigned::Trivial(Fp::from(9))),
        );
        if early_drop {
            drop(adapter);
        } else {
            assert!(matches!(
                adapter.into_assignments(Err(Error::Synthesis)),
                Err(StoredSynthesisErrorV1::Synthesis)
            ));
        }
        assert_eq!(backend.record.borrow().writer_drops, 2);
        assert!(backend.record.borrow().sealed.is_empty());
    }
}

// V1's optional thread-safe-region feature requires a Send+Sync backend. Retain the
// existing local failure-injection fixture above, and use this plain recording oracle
// for actual floor-planner calls under either feature mode.
use std::sync::{Arc, Mutex};

struct SyncWriter {
    layout: StoredAdviceLayoutV1,
    record: Arc<Mutex<Recording>>,
    values: Vec<[u8; 32]>,
}
struct SyncSnapshot {
    layout: StoredAdviceLayoutV1,
    record: Arc<Mutex<Recording>>,
    values: Vec<[u8; 32]>,
    poisoned: bool,
}
impl Drop for SyncWriter {
    fn drop(&mut self) {
        self.record.lock().unwrap().writer_drops += 1;
    }
}
impl Drop for SyncSnapshot {
    fn drop(&mut self) {
        self.record.lock().unwrap().snapshot_drops += 1;
    }
}
impl StoredAdviceWriterV1 for SyncWriter {
    type Snapshot = SyncSnapshot;
    fn layout(&self) -> StoredAdviceLayoutV1 {
        self.layout
    }
    fn write_chunk(&mut self, chunk: u64, values: &[[u8; 32]]) -> Result<(), StoredAdviceErrorV1> {
        assert_eq!(
            chunk as usize * STORED_SCALARS_PER_CHUNK_V1,
            self.values.len()
        );
        assert_eq!(values.len(), self.layout.chunk_scalar_count(chunk)?);
        self.values.extend_from_slice(values);
        Ok(())
    }
    fn seal(mut self) -> Result<Self::Snapshot, StoredAdviceErrorV1> {
        assert_eq!(self.values.len(), self.layout.scalar_count());
        self.record
            .lock()
            .unwrap()
            .sealed
            .push((self.layout, self.values.clone()));
        Ok(SyncSnapshot {
            layout: self.layout,
            record: Arc::clone(&self.record),
            values: std::mem::take(&mut self.values),
            poisoned: false,
        })
    }
}
impl StoredAdviceSnapshotV1 for SyncSnapshot {
    fn layout(&self) -> StoredAdviceLayoutV1 {
        self.layout
    }
    fn with_chunk<R>(
        &mut self,
        expected: StoredAdviceLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1> {
        if self.poisoned {
            return Err(StoredAdviceErrorV1::Poisoned);
        }
        self.poisoned = true;
        if expected != self.layout {
            return Err(StoredAdviceErrorV1::Context);
        }
        let count = expected.chunk_scalar_count(chunk)?;
        let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
        let result = consume(&self.values[start..start + count])?;
        self.poisoned = false;
        Ok(result)
    }
    fn with_column<R>(
        &mut self,
        _: StoredAdviceLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1> {
        panic!("single-phase synthesis and commitment must only read bounded chunks")
    }
}
fn sync_writers<C: CurveAffine>(
    plan: &StoredPhasePlanV1<'_, C>,
    record: &Arc<Mutex<Recording>>,
) -> Vec<SyncWriter> {
    plan.phases[0]
        .columns
        .iter()
        .enumerate()
        .map(|(ordinal, index)| SyncWriter {
            layout: StoredAdviceLayoutV1::new(
                [9; 32],
                11 + ordinal as u64,
                plan.field,
                StoredPolynomialBasisV1::Lagrange,
                plan.k,
                *index as u32,
                0,
            )
            .unwrap(),
            record: Arc::clone(record),
            values: Vec::new(),
        })
        .collect()
}
fn v1_bridge<'params, 'instances>(
    params: &'params ParamsIPA<EqAffine>,
    backend: &Arc<Mutex<Recording>>,
    instances: &'instances [&'instances [Fp]],
) -> (
    StoredSinglePhaseAssignmentV1<'params, 'instances, EqAffine, SyncWriter>,
    Config,
) {
    let mut meta = ConstraintSystem::default();
    let config = configure(&mut meta);
    let domain = EvaluationDomain::new(4, params.k());
    let plan = admit_stored_phase_plan_v1(params, &domain, &meta).unwrap();
    let columns = sync_writers(&plan, backend);
    (
        StoredSinglePhaseAssignmentV1::new(plan, columns, instances).unwrap(),
        config,
    )
}

#[derive(Clone)]
struct RegionCircuit<F> {
    value: F,
    smaller_first: bool,
}

impl<F: Field> Circuit<F> for RegionCircuit<F> {
    type Config = Config;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            value: F::ZERO,
            smaller_first: self.smaller_first,
        }
    }
    fn configure(meta: &mut ConstraintSystem<F>) -> Config {
        configure(meta)
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        for length in if self.smaller_first { [2, 4] } else { [4, 2] } {
            layouter.assign_region(
                || "original V1 placement",
                |mut region| {
                    config.selector.enable(&mut region, 0)?;
                    region.assign_fixed(config.fixed, 0, self.value);
                    for row in 0..length {
                        region.assign_advice_discarding_value(
                            config.advice[0],
                            row,
                            Value::known(self.value + F::from(row as u64)),
                        );
                    }
                    Ok(())
                },
            )?;
        }
        Ok(())
    }
}

#[test]
fn original_v1_absolute_rows_are_accepted_without_replanning_or_replay() {
    let params = ParamsIPA::<EqAffine>::new(4);
    let backend = Arc::new(Mutex::new(Recording::default()));
    let instances = [&[][..]];
    let (mut adapter, config) = v1_bridge(&params, &backend, &instances);
    let circuit = RegionCircuit {
        value: Fp::from(23),
        smaller_first: false,
    };
    let outcome = V1::synthesize(&mut adapter, &circuit, config, vec![]);
    drop(circuit);
    let mut rng = ChaCha20Rng::from_seed([11; 32]);
    drop(
        adapter
            .into_assignments(outcome)
            .unwrap()
            .finish(&mut rng)
            .unwrap(),
    );
    let record = backend.lock().unwrap();
    let rows = &record.sealed[0].1;
    let expected: Vec<_> = [0, 1, 2, 3, 0, 1]
        .into_iter()
        .map(|row| (Fp::from(23) + Fp::from(row)).to_repr())
        .collect();
    assert_eq!(&rows[..6], &expected);
    assert_eq!(record.snapshot_drops, 2);
}

#[test]
fn original_v1_backward_absolute_region_placement_is_refused() {
    let params = ParamsIPA::<EqAffine>::new(4);
    let backend = Arc::new(Mutex::new(Recording::default()));
    let instances = [&[][..]];
    let (mut adapter, config) = v1_bridge(&params, &backend, &instances);
    let circuit = RegionCircuit {
        value: Fp::from(23),
        smaller_first: true,
    };
    // V1 places the four-row region first physically, while preserving synthesis call order.
    let outcome = V1::synthesize(&mut adapter, &circuit, config, vec![]);
    drop(circuit);
    assert!(
        outcome.is_ok(),
        "the producer can ignore void assignment errors"
    );
    assert!(matches!(
        adapter.into_assignments(outcome),
        Err(StoredSynthesisErrorV1::Phase(
            StoredPhaseErrorV1::Assignment(StoredAssignmentErrorV1::NonMonotonic)
        ))
    ));
    assert_eq!(backend.lock().unwrap().writer_drops, 2);
    assert!(backend.lock().unwrap().sealed.is_empty());
}

fn optimized_key_selectors<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    use crate::plonk::{keygen_pk, keygen_vk_custom};
    let params = ParamsIPA::<C>::new(4);
    for compress in [false, true] {
        let circuit = RegionCircuit {
            value: C::Scalar::from(23),
            smaller_first: false,
        };
        let vk = keygen_vk_custom(&params, &circuit, compress).unwrap();
        let pk = keygen_pk(&params, vk, &circuit).unwrap();
        let mut fresh = ConstraintSystem::default();
        let config = configure(&mut fresh);
        assert_eq!(fresh.num_selectors(), 1);
        assert_eq!(pk.get_vk().cs().num_selectors(), usize::from(compress));
        let plan = admit_stored_phase_plan_v1(&params, pk.get_vk().get_domain(), pk.get_vk().cs())
            .unwrap();
        let backend = Arc::new(Mutex::new(Recording::default()));
        let columns = sync_writers(&plan, &backend);
        let instances = [&[][..]];
        let mut adapter = StoredSinglePhaseAssignmentV1::new(plan, columns, &instances).unwrap();
        let result = V1::synthesize(&mut adapter, &circuit, config, vec![]);
        drop(circuit);
        let mut rng = ChaCha20Rng::from_seed([71; 32]);
        drop(
            adapter
                .into_assignments(result)
                .unwrap()
                .finish(&mut rng)
                .unwrap(),
        );
        assert_eq!(backend.lock().unwrap().snapshot_drops, 2);
    }
}

#[test]
fn eq_original_selectors_survive_direct_and_compressed_proving_key_conversion() {
    optimized_key_selectors::<EqAffine>();
}

#[test]
fn ep_original_selectors_survive_direct_and_compressed_proving_key_conversion() {
    optimized_key_selectors::<EpAffine>();
}
