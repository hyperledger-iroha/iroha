//! Key-derived lookup tiles versus ordinary evaluation, with complete protocol-owner cleanup.
//!
//! Fixtures retain plaintext copies for an independent dense arithmetic oracle. These tests
//! create real keys and stored prefixes, not complete stored lookup arguments or proofs.

use super::*;
use crate::plonk::{
    Expression,
    evaluation::evaluate,
    stored::{StoredExpressionErrorV1, StoredRowTileV1},
};
use crate::poly::stored_advice::{StoredLookupSideV1, StoredPolynomialRoleV1};

struct LookupProducer<C: CurveAffine>(Producer<C>);

impl<C: CurveAffine> Circuit<C::Scalar> for LookupProducer<C> {
    type Config = Config;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = CircuitParams;

    fn without_witnesses(&self) -> Self {
        Self(self.0.without_witnesses())
    }

    #[cfg(feature = "circuit-params")]
    fn params(&self) -> Self::Params {
        self.0.params()
    }

    fn configure(meta: &mut ConstraintSystem<C::Scalar>) -> Config {
        let config = <Producer<C> as Circuit<C::Scalar>>::configure(meta);
        meta.lookup_any("original keyed mixed lookup", |meta| {
            let a = meta.query_advice(config.advice[0], Rotation(1));
            let b = meta.query_advice(config.advice[1], Rotation(-1));
            let fixed = meta.query_fixed(config.fixed, Rotation(-1));
            let instance = meta.query_instance(config.instances[2], Rotation(1));
            let empty = meta.query_instance(config.instances[1], Rotation::cur());
            let challenge = meta.query_challenge(config.challenge);
            vec![
                (
                    (a + fixed.clone() * instance.clone()) * challenge.clone(),
                    fixed + instance,
                ),
                (
                    b - empty,
                    challenge + Expression::Constant(C::Scalar::from(9)),
                ),
            ]
        });
        meta.lookup_any("second retained lookup", |meta| {
            vec![(
                meta.query_instance(config.instances[0], Rotation(-1)),
                meta.query_fixed(config.fixed, Rotation::cur())
                    + Expression::Constant(C::Scalar::from(31)),
            )]
        });
        config
    }

    fn synthesize_for_measurement(
        &self,
        config: Config,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.0.run(config, layouter, true)
    }

    fn synthesize(&self, config: Config, layouter: impl Layouter<C::Scalar>) -> Result<(), Error> {
        self.0.run(config, layouter, false)
    }
}

fn lookup_key<C>(params: &ParamsIPA<C>, compressed: bool) -> ProvingKey<C>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
{
    let shared = Shared::<C>::new();
    let producer = LookupProducer(Producer::new(&shared, 3));
    let vk = keygen_vk_custom(params, &producer, compressed).unwrap();
    keygen_pk(params, vk, &producer).unwrap()
}

fn keyed_oracle<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    for k in [4, 9] {
        for compressed in [false, true] {
            let params = ParamsIPA::<C>::new(k);
            let pk = lookup_key(&params, compressed);
            let rows = 1_usize << k;
            let last = if k == 9 { 300 } else { 3 };
            let values = [
                vec![C::Scalar::from(17)],
                vec![],
                (0..last).map(|i| C::Scalar::from(i as u64 + 23)).collect(),
            ];
            let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
            let shared = Shared::<C>::new();
            let mut pending = prepare_single_phase_stored_ipa_prefix_v1::<
                C,
                _,
                _,
                _,
                Challenge255<C>,
                _,
                true,
                6,
            >(
                &params,
                pk,
                LookupProducer(Producer::new(&shared, last)),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap();
            let vk = pending.pk.get_vk().to_bytes(crate::SerdeFormat::Processed);
            let fixed_allocation = pending.pk.fixed_values.as_ptr();
            let dense_advice = shared
                .log
                .lock()
                .unwrap()
                .sealed
                .iter()
                .map(|(_, values)| {
                    pending.pk.vk.domain.lagrange_from_vec(
                        values
                            .iter()
                            .map(|value| {
                                Option::<C::Scalar>::from(C::Scalar::from_repr(*value)).unwrap()
                            })
                            .collect(),
                    )
                })
                .collect::<Vec<_>>();
            let dense_instances = values
                .iter()
                .map(|values| {
                    let mut dense = pending.pk.vk.domain.empty_lagrange();
                    dense[0..values.len()].copy_from_slice(values);
                    dense
                })
                .collect::<Vec<_>>();
            let challenges = pending.advice.challenges().unwrap().collect::<Vec<_>>();
            let layouts = pending.advice.layouts().unwrap().collect::<Vec<_>>();
            let (events, draws, created) = {
                let log = shared.log.lock().unwrap();
                (log.events.clone(), log.rng_calls, log.created)
            };
            let mut expected_rng = shared.rng.lock().unwrap().clone();
            for lookup_index in 0..2 {
                for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
                    let count = if lookup_index == 0 { 2 } else { 1 };
                    for expression_index in 0..count {
                        let expected = {
                            let lookup = &pending.pk.vk.cs.lookups[lookup_index];
                            let expression = match side {
                                StoredLookupSideV1::Input => {
                                    &lookup.input_expressions[expression_index]
                                }
                                StoredLookupSideV1::Table => {
                                    &lookup.table_expressions[expression_index]
                                }
                            };
                            evaluate(
                                expression,
                                rows,
                                1,
                                &pending.pk.fixed_values,
                                &dense_advice,
                                &dense_instances,
                                &challenges,
                            )
                        };
                        for start in (0..rows).step_by(256) {
                            let tile = StoredRowTileV1 {
                                start,
                                len: 256.min(rows - start),
                            };
                            let (next, actual) = pending
                                .with_lookup_expression_tile(
                                    lookup_index,
                                    side,
                                    expression_index,
                                    tile,
                                    1 << 20,
                                    |values| Ok(values.to_vec()),
                                )
                                .unwrap();
                            pending = next;
                            assert_eq!(actual, expected[start..start + tile.len]);
                        }
                    }
                }
            }
            assert!(std::ptr::eq(pending.params, &params));
            assert_eq!(pending.pk.fixed_values.as_ptr(), fixed_allocation);
            assert_eq!(
                pending.pk.get_vk().to_bytes(crate::SerdeFormat::Processed),
                vk
            );
            assert!(std::ptr::eq(pending.instances.as_ptr(), instances.as_ptr()));
            assert_eq!(
                pending.advice.layouts().unwrap().collect::<Vec<_>>(),
                layouts
            );
            assert_eq!(
                pending.advice.challenges().unwrap().collect::<Vec<_>>(),
                challenges
            );
            let log = shared.log.lock().unwrap();
            assert_eq!(log.events, events);
            assert_eq!(log.rng_calls, draws);
            assert_eq!(log.created, created);
            assert_eq!(log.snapshot_drops, 0);
            assert_eq!(log.provider_drops, 0);
            assert_eq!(log.rng_drops, 0);
            assert_eq!(log.transcript_drops, 0);
            drop(log);
            let (mut actual, mut expected) = ([0; 64], [0; 64]);
            pending.rng.fill_bytes(&mut actual);
            expected_rng.fill_bytes(&mut expected);
            assert_eq!(actual, expected);
            drop(pending);
            assert_dropped(&shared);
        }
    }
}

#[test]
fn eq_keyed_lookup_tiles_match_dense_evaluation_and_preserve_every_original_owner() {
    keyed_oracle::<EqAffine>();
}

#[test]
fn ep_keyed_lookup_tiles_match_dense_evaluation_and_preserve_every_original_owner() {
    keyed_oracle::<EpAffine>();
}

fn invalid_keyed_tile<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let wrong_params = ParamsIPA::<C>::new(5);
    let pk = lookup_key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    // A full-domain prefix exceeds usable rows, although its length is still exactly n.
    let oversized = vec![C::Scalar::ONE; 16];
    let mut oversized_instances = instances.clone();
    oversized_instances[1] = &oversized;
    let short_domain = crate::poly::EvaluationDomain::<C::Scalar>::new(3, 3);
    for fault in 0..11 {
        let shared = Shared::<C>::new();
        let mut pending =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                pk.clone(),
                LookupProducer(Producer::new(&shared, 3)),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap();
        let mut lookup = 0;
        let mut expression = 0;
        let mut tile = StoredRowTileV1 { start: 0, len: 16 };
        let mut budget = 1 << 20;
        match fault {
            0 => lookup = usize::MAX,
            1 => expression = usize::MAX,
            2 => tile.start = 1,
            3 => tile.len = 0,
            4 => budget = 0,
            5 => {
                pending.pk.fixed_values.pop();
            }
            6 => pending.instances = &[],
            7 => pending.params = &wrong_params,
            8 => {
                let layout = pending.advice.layouts().unwrap().next().unwrap();
                assert!(
                    pending
                        .advice
                        .with_chunk(layout, 0, |_, _| Err::<(), _>(
                            StoredPolynomialErrorV1::Consumer
                        ))
                        .is_err()
                );
            }
            9 => pending.instances = &oversized_instances,
            10 => {
                // Keep the fixed-column count correct while changing one polynomial's n.
                pending.pk.fixed_values[0] =
                    short_domain.lagrange_from_vec(vec![C::Scalar::ONE; 8]);
            }
            _ => unreachable!(),
        }
        let (reads, events, draws) = {
            let log = shared.log.lock().unwrap();
            (log.reads, log.events.clone(), log.rng_calls)
        };
        let mut called = false;
        let result = pending.with_lookup_expression_tile(
            lookup,
            StoredLookupSideV1::Input,
            expression,
            tile,
            budget,
            |_| {
                called = true;
                Ok(())
            },
        );
        assert!(!called);
        assert!(result.is_err());
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.reads, reads);
        assert_eq!(log.events, events);
        assert_eq!(log.rng_calls, draws);
        assert_eq!(log.provider_drops, 1);
        assert_eq!(log.rng_drops, 1);
        assert_eq!(log.transcript_drops, 1);
    }
}

#[test]
fn both_pasta_keyed_tile_preflights_destroy_the_whole_owner_without_witness_reads() {
    invalid_keyed_tile::<EqAffine>();
    invalid_keyed_tile::<EpAffine>();
}

fn failed_keyed_tile<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let pk = lookup_key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for fault in 0..4 {
        let shared = Shared::<C>::new();
        let pending =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                pk.clone(),
                LookupProducer(Producer::new(&shared, 300)),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap();
        let (events, draws) = {
            let mut log = shared.log.lock().unwrap();
            log.fault = match fault {
                0 => Some(Fault::Read(1)),
                1 => Some(Fault::PanicRead(1)),
                _ => None,
            };
            (log.events.clone(), log.rng_calls)
        };
        let mut called = false;
        let result = catch_unwind(AssertUnwindSafe(|| {
            pending
                .with_lookup_expression_tile(
                    0,
                    StoredLookupSideV1::Input,
                    0,
                    StoredRowTileV1 { start: 0, len: 256 },
                    1 << 20,
                    |_| {
                        called = true;
                        assert_ne!(fault, 3, "injected keyed consumer unwind");
                        Err::<(), _>(StoredExpressionErrorV1::Consumer)
                    },
                )
                .map(|_| ())
        }));
        assert_eq!(called, fault >= 2);
        if fault == 1 || fault == 3 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events);
        assert_eq!(log.rng_calls, draws);
        assert_eq!(log.provider_drops, 1);
        assert_eq!(log.rng_drops, 1);
        assert_eq!(log.transcript_drops, 1);
    }
}

#[test]
fn both_pasta_keyed_tile_backend_and_consumer_errors_or_unwinds_destroy_protocol_state() {
    failed_keyed_tile::<EqAffine>();
    failed_keyed_tile::<EpAffine>();
}

fn successful_consumer_snapshot_drift<C>(lookup_side: Option<StoredLookupSideV1>)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let pk = lookup_key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let pending =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
            &params,
            pk,
            LookupProducer(Producer::new(&shared, 3)),
            &instances,
            Provider::new(&shared),
            Rng(Arc::clone(&shared)),
            RecordingTranscript::new(&shared),
        )
        .unwrap();
    // Lookup 0/input 0 reads advice 0 only. The unused second receipt must also be checked
    // after the consumer returns, even though its ordinal and all values remain unchanged.
    let unused = pending.advice.layouts().unwrap().nth(1).unwrap();
    assert_eq!(unused.advice_coordinates().unwrap().0, 1);
    let replacement = StoredPolynomialLayoutV1::new(
        if lookup_side.is_some() {
            [23; 32]
        } else {
            [29; 32]
        },
        unused.ordinal(),
        unused.field(),
        unused.basis(),
        unused.k(),
        lookup_side.map_or(unused.role(), |side| {
            StoredPolynomialRoleV1::LookupCompressed { lookup: 1, side }
        }),
    )
    .unwrap();
    assert_eq!(
        replacement.same_proof_context(unused),
        lookup_side.is_some()
    );
    if lookup_side.is_some() {
        assert_ne!(replacement.role(), unused.role());
    }
    let (reads, events, draws) = {
        let log = shared.log.lock().unwrap();
        assert_eq!(log.sealed.len(), 2);
        assert_eq!(log.snapshot_drops, 0);
        (log.reads, log.events.clone(), log.rng_calls)
    };
    let returned_value = Arc::new(());
    let mut called = false;
    let result = pending.with_lookup_expression_tile(
        0,
        StoredLookupSideV1::Input,
        0,
        StoredRowTileV1 { start: 0, len: 16 },
        1 << 20,
        |tile| {
            called = true;
            assert_eq!(tile.len(), 16);
            shared.log.lock().unwrap().snapshot_layout_override =
                Some((unused.ordinal(), replacement));
            Ok(Arc::clone(&returned_value))
        },
    );
    assert!(called);
    assert!(matches!(
        result,
        Err(StoredExpressionErrorV1::Store(
            StoredPolynomialErrorV1::Context
        ))
    ));
    assert_eq!(Arc::strong_count(&returned_value), 1);
    assert_dropped(&shared);
    let log = shared.log.lock().unwrap();
    assert_eq!(log.reads, reads + 1);
    assert_eq!(log.events, events);
    assert_eq!(log.rng_calls, draws);
    assert_eq!(log.snapshot_drops, 2);
    assert_eq!(log.provider_drops, 1);
    assert_eq!(log.rng_drops, 1);
    assert_eq!(log.transcript_drops, 1);
}

#[test]
fn both_pasta_successful_keyed_consumer_receipt_drift_rejects_and_destroys_all_owners() {
    successful_consumer_snapshot_drift::<EqAffine>(None);
    successful_consumer_snapshot_drift::<EpAffine>(None);
}

struct EmptyLookupProducer<C: CurveAffine>(Producer<C>);

#[derive(Clone, Copy)]
struct EmptyConfig {
    fixed: Column<Fixed>,
    instance: Column<Instance>,
}

impl<C: CurveAffine> EmptyLookupProducer<C> {
    fn run(
        &self,
        config: EmptyConfig,
        mut layouter: impl Layouter<C::Scalar>,
        measurement: bool,
    ) -> Result<(), Error> {
        if measurement {
            self.0.shared.log.lock().unwrap().measurement_passes += 1;
        } else {
            self.0.shared.log.lock().unwrap().synthesis_passes += 1;
        }
        layouter.assign_region(
            || "zero-advice fixed input",
            |mut region| {
                region.assign_fixed(config.fixed, 0, C::Scalar::from(7));
                let _ = region.instance_value(config.instance, 0)?;
                Ok(())
            },
        )
    }
}

impl<C: CurveAffine> Circuit<C::Scalar> for EmptyLookupProducer<C> {
    type Config = EmptyConfig;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self(self.0.without_witnesses())
    }

    fn configure(meta: &mut ConstraintSystem<C::Scalar>) -> EmptyConfig {
        let fixed = meta.fixed_column();
        let instance = meta.instance_column();
        meta.lookup_any("zero-advice original lookup", |meta| {
            vec![(
                meta.query_fixed(fixed, Rotation(-1)) + meta.query_instance(instance, Rotation(1)),
                meta.query_instance(instance, Rotation::cur()),
            )]
        });
        EmptyConfig { fixed, instance }
    }

    fn synthesize_for_measurement(
        &self,
        config: EmptyConfig,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.run(config, layouter, true)
    }

    fn synthesize(
        &self,
        config: EmptyConfig,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.run(config, layouter, false)
    }
}

fn empty_keyed_tile<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let key_shared = Shared::<C>::new();
    let producer = EmptyLookupProducer(Producer::new(&key_shared, 0));
    let vk = keygen_vk_custom(&params, &producer, true).unwrap();
    let pk = keygen_pk(&params, vk, &producer).unwrap();
    let values = vec![C::Scalar::from(17)];
    let instances: [&[C::Scalar]; 1] = [&values];
    let shared = Shared::<C>::new();
    let pending =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
            &params,
            pk,
            EmptyLookupProducer(Producer::new(&shared, 0)),
            &instances,
            Provider::new(&shared),
            Rng(Arc::clone(&shared)),
            RecordingTranscript::new(&shared),
        )
        .unwrap();
    assert_eq!(pending.advice.proof_context().unwrap(), None);
    let events = shared.log.lock().unwrap().events.clone();
    let mut dense = pending.pk.vk.domain.empty_lagrange();
    dense[0] = C::Scalar::from(17);
    let expected = evaluate(
        &pending.pk.vk.cs.lookups[0].input_expressions[0],
        16,
        1,
        &pending.pk.fixed_values,
        &[],
        &[dense],
        &[],
    );
    let (pending, actual) = pending
        .with_lookup_expression_tile(
            0,
            StoredLookupSideV1::Input,
            0,
            StoredRowTileV1 { start: 0, len: 16 },
            1 << 20,
            |values| Ok(values.to_vec()),
        )
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(pending.advice.proof_context().unwrap(), None);
    assert_eq!(pending.advice.layouts().unwrap().len(), 0);
    let log = shared.log.lock().unwrap();
    assert_eq!(log.events, events);
    assert_eq!(log.rng_calls, 0);
    assert_eq!(log.created, 0);
    assert_eq!(log.reads, 0);
    assert_eq!(log.snapshot_drops, 0);
    drop(log);
    drop(pending);
    assert_dropped(&shared);
}

#[test]
fn both_pasta_zero_advice_lookup_tiles_keep_absent_store_context_and_zero_padding() {
    empty_keyed_tile::<EqAffine>();
    empty_keyed_tile::<EpAffine>();
}

#[test]
fn both_pasta_successful_keyed_consumer_lookup_role_drift_destroys_all_owners() {
    for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
        successful_consumer_snapshot_drift::<EqAffine>(Some(side));
        successful_consumer_snapshot_drift::<EpAffine>(Some(side));
    }
}

#[path = "lookup_tests.rs"]
mod compression;
