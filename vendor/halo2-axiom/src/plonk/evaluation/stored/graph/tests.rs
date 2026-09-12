//! Retained-graph oracle, bounded reads and failure cleanup for both Pasta directions.

use super::*;
use crate::{
    plonk::evaluation::CalculationInfo,
    poly::{
        EvaluationDomain,
        stored_advice::{StoredAdviceErrorV1, StoredAdviceLayoutV1, StoredPolynomialBasisV1},
    },
};
use ff::{Field, PrimeField};
use halo2curves::pasta::{EpAffine, EqAffine};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[derive(Default)]
struct Recording {
    calls: RefCell<Vec<(u32, u64)>>,
    busy: Cell<bool>,
    fail: Cell<Option<(u32, u64)>>,
    unwind: Cell<Option<(u32, u64)>>,
    corrupt: Cell<Option<(u32, u64)>>,
}
struct Window(Rc<Recording>);
impl Drop for Window {
    fn drop(&mut self) {
        self.0.busy.set(false);
    }
}
struct Snapshot {
    layout: StoredAdviceLayoutV1,
    values: Vec<[u8; 32]>,
    recording: Rc<Recording>,
    poisoned: bool,
}
impl StoredAdviceSnapshotV1 for Snapshot {
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
        if expected != self.layout {
            return Err(StoredAdviceErrorV1::Context);
        }
        let len = expected.chunk_scalar_count(chunk)?;
        assert!(!self.recording.busy.replace(true), "no nested reads");
        let _window = Window(Rc::clone(&self.recording));
        self.poisoned = true;
        let location = (expected.column(), chunk);
        self.recording.calls.borrow_mut().push(location);
        assert_ne!(
            self.recording.unwind.get(),
            Some(location),
            "injected read panic"
        );
        if self.recording.fail.get() == Some(location) {
            return Err(StoredAdviceErrorV1::Storage);
        }
        let start = chunk as usize * TILE;
        let mut encoded = self.values[start..start + len].to_vec();
        if self.recording.corrupt.get() == Some(location) {
            encoded[0] = [255; 32];
        }
        let result = consume(&encoded)?;
        self.poisoned = false;
        Ok(result)
    }
    fn with_column<R>(
        &mut self,
        _: StoredAdviceLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1> {
        panic!("full column reads forbidden")
    }
}

fn layout<F: StoredAssignmentFieldV1>(
    k: u32,
    column: u32,
    basis: StoredPolynomialBasisV1,
) -> StoredAdviceLayoutV1 {
    StoredAdviceLayoutV1::new(
        [91; 32],
        10 + u64::from(column),
        F::STORED_FIELD,
        basis,
        k,
        column,
        0,
    )
    .unwrap()
}

fn graph<C: CurveAffine>() -> GraphEvaluator<C> {
    let mut graph = GraphEvaluator::<C>::default();
    graph.rotations = vec![0, 1, -1];
    let a = graph.add_calculation(Calculation::Store(ValueSource::Advice(0, 0)));
    let b = graph.add_calculation(Calculation::Add(a, ValueSource::Advice(1, 1)));
    let c = graph.add_calculation(Calculation::Sub(b, ValueSource::Fixed(0, 2)));
    let d = graph.add_calculation(Calculation::Mul(c, ValueSource::Instance(0, 0)));
    let e = graph.add_calculation(Calculation::Square(d));
    let f = graph.add_calculation(Calculation::Double(e));
    let g = graph.add_calculation(Calculation::Negate(f));
    let h = graph.add_calculation(Calculation::Add(
        ValueSource::Challenge(0),
        ValueSource::Beta(),
    ));
    let i = graph.add_calculation(Calculation::Mul(ValueSource::Gamma(), ValueSource::Theta()));
    graph.add_calculation(Calculation::Horner(
        ValueSource::PreviousValue(),
        vec![
            a,
            b,
            c,
            d,
            e,
            f,
            g,
            h,
            i,
            ValueSource::Advice(0, 0),
            ValueSource::Constant(1),
        ],
        ValueSource::Y(),
    ));
    graph.finish_building();
    graph
}

fn compare<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + ff::WithSmallOrderMulGroup<3>,
{
    for k in [0, 1, 8, 9, 16] {
        for basis in [
            StoredPolynomialBasisV1::Lagrange,
            StoredPolynomialBasisV1::CosetPart {
                extension_log: 2,
                part: 1,
            },
        ] {
            let size = 1_usize << k;
            let domain = EvaluationDomain::<C::Scalar>::new(4, k);
            let in_basis = |base: Polynomial<C::Scalar, LagrangeCoeff>| match basis {
                StoredPolynomialBasisV1::Lagrange => base,
                StoredPolynomialBasisV1::CosetPart {
                    extension_log,
                    part,
                } => {
                    assert_eq!(extension_log, domain.extended_k() - domain.k());
                    if k == 0 {
                        // A degree-zero polynomial is the same constant on every coset.
                        // The retained recursive FFT has no length-one stage, so construct
                        // this exact oracle input directly while still exercising k=0 tiles.
                        assert_eq!(base.len(), 1);
                        base
                    } else {
                        domain.coeff_to_extended_part(
                            domain.lagrange_to_coeff(base),
                            domain.get_extended_omega().pow_vartime([u64::from(part)]),
                        )
                    }
                }
                StoredPolynomialBasisV1::Coefficient => unreachable!("only evaluation bases"),
            };
            let advice: Vec<_> = (0..2)
                .map(|column| {
                    in_basis(
                        domain.lagrange_from_vec(
                            (0..size)
                                .map(|row| C::Scalar::from((row * 7 + column * 19 + 3) as u64))
                                .collect(),
                        ),
                    )
                })
                .collect();
            let fixed = vec![in_basis(
                domain.lagrange_from_vec(
                    (0..size)
                        .map(|row| C::Scalar::from((row * 3 + 5) as u64))
                        .collect(),
                ),
            )];
            let instance = vec![in_basis(
                domain.lagrange_from_vec(
                    (0..size)
                        .map(|row| C::Scalar::from((row * 11 + 1) as u64))
                        .collect(),
                ),
            )];
            let layouts = [
                layout::<C::Scalar>(k, 0, basis),
                layout::<C::Scalar>(k, 1, basis),
            ];
            let recording = Rc::new(Recording::default());
            let mut snapshots: Vec<_> = advice
                .iter()
                .zip(layouts)
                .map(|(poly, layout)| Snapshot {
                    layout,
                    values: poly.iter().map(PrimeField::to_repr).collect(),
                    recording: Rc::clone(&recording),
                    poisoned: false,
                })
                .collect();
            let graph = graph::<C>();
            let original_targets: Vec<_> =
                graph.calculations.iter().map(|info| info.target).collect();
            let context = StoredExpressionContextV1 {
                domain: layouts[0],
                advice: &layouts,
                fixed_columns: 1,
                instance_columns: 1,
                challenge_phases: &[0],
            };
            let plan = prepare_stored_graph_v1(&graph, context, 1024 * 1024).unwrap();
            assert_eq!(
                plan.advice_queries.len(),
                2,
                "repeated query uses one cache slot"
            );
            assert_eq!(
                plan.scratch_bytes(),
                (2 * TILE + graph.num_intermediates + TILE) * 32
            );
            let mut oracle = graph.instance();
            for start in [0, size.saturating_sub(TILE) / TILE * TILE] {
                let tile = StoredRowTileV1 {
                    start,
                    len: TILE.min(size - start),
                };
                let previous: Vec<_> = (0..tile.len)
                    .map(|row| C::Scalar::from((start + row + 23) as u64))
                    .collect();
                let [beta, gamma, theta, y, challenge] = [2, 3, 5, 7, 11].map(C::Scalar::from);
                let expected: Vec<_> = previous
                    .iter()
                    .enumerate()
                    .map(|(row, previous)| {
                        graph.evaluate(
                            &mut oracle,
                            &fixed,
                            &advice,
                            &instance,
                            &[challenge],
                            &beta,
                            &gamma,
                            &theta,
                            &y,
                            previous,
                            start + row,
                            1,
                            size as i32,
                        )
                    })
                    .collect();
                let before = recording.calls.borrow().len();
                let mut inputs: Vec<_> = snapshots
                    .iter_mut()
                    .zip(layouts)
                    .map(|(snapshot, expected)| StoredAdviceInputV1 { expected, snapshot })
                    .collect();
                with_stored_graph_chunk_v1(
                    &plan,
                    tile,
                    &mut inputs,
                    &fixed,
                    &instance,
                    &[challenge],
                    beta,
                    gamma,
                    theta,
                    y,
                    &previous,
                    |actual| {
                        assert_eq!(actual, expected);
                        assert!(!recording.busy.get());
                        Ok(())
                    },
                )
                .unwrap();
                assert!(recording.calls.borrow().len() - before <= plan.maximum_chunk_reads());
            }
            assert_eq!(
                original_targets,
                graph
                    .calculations
                    .iter()
                    .map(|info| info.target)
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn retained_graph_oracle_matches_both_pasta_fields_all_calculations_and_coset_parts() {
    compare::<EpAffine>();
    compare::<EqAffine>();
}

type F = <EqAffine as CurveAffine>::ScalarExt;
fn one_query(rotation: i32) -> GraphEvaluator<EqAffine> {
    let mut graph = GraphEvaluator::<EqAffine>::default();
    graph.rotations = vec![rotation];
    graph.add_calculation(Calculation::Store(ValueSource::Advice(0, 0)));
    graph.finish_building();
    graph
}

#[test]
fn graph_extreme_rotations_preserve_euclidean_indices_and_two_read_bound() {
    for rotation in [i32::MIN, i32::MAX, -513, -512, 512, 513] {
        let graph = one_query(rotation);
        let expected = layout::<F>(9, 0, StoredPolynomialBasisV1::Lagrange);
        let context = StoredExpressionContextV1 {
            domain: expected,
            advice: &[expected],
            fixed_columns: 0,
            instance_columns: 0,
            challenge_phases: &[],
        };
        let plan = prepare_stored_graph_v1(&graph, context, 65536).unwrap();
        let recording = Rc::new(Recording::default());
        let mut snapshot = Snapshot {
            layout: expected,
            values: (0..512).map(|row| F::from(row).to_repr()).collect(),
            recording: Rc::clone(&recording),
            poisoned: false,
        };
        with_stored_graph_chunk_v1(
            &plan,
            StoredRowTileV1 {
                start: 256,
                len: 256,
            },
            &mut [StoredAdviceInputV1 {
                expected,
                snapshot: &mut snapshot,
            }],
            &[],
            &[],
            &[],
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            &[F::ZERO; TILE],
            |actual| {
                for (row, value) in actual.iter().enumerate() {
                    assert_eq!(
                        *value,
                        F::from((256_i64 + row as i64 + i64::from(rotation)).rem_euclid(512) as u64)
                    );
                }
                Ok(())
            },
        )
        .unwrap();
        assert!(recording.calls.borrow().len() <= 2);
    }
}

#[test]
fn graph_admission_rejects_unfinished_undefined_and_out_of_range_sources() {
    let expected = layout::<F>(9, 0, StoredPolynomialBasisV1::Lagrange);
    let context = StoredExpressionContextV1 {
        domain: expected,
        advice: &[expected],
        fixed_columns: 0,
        instance_columns: 0,
        challenge_phases: &[],
    };
    let unfinished = GraphEvaluator::<EqAffine>::default();
    assert!(matches!(
        prepare_stored_graph_v1(&unfinished, context, 65536),
        Err(StoredExpressionErrorV1::Plan)
    ));
    for source in [
        ValueSource::Intermediate(0),
        ValueSource::Constant(99),
        ValueSource::Advice(1, 0),
        ValueSource::Advice(0, 1),
        ValueSource::Fixed(0, 0),
        ValueSource::Instance(0, 0),
        ValueSource::Challenge(0),
    ] {
        let mut graph = one_query(0);
        graph.calculations[0].calculation = Calculation::Store(source);
        assert!(prepare_stored_graph_v1(&graph, context, 65536).is_err());
    }
    let mut graph = one_query(0);
    graph.calculations[0].target = graph.num_intermediates;
    assert!(matches!(
        prepare_stored_graph_v1(&graph, context, 65536),
        Err(StoredExpressionErrorV1::Plan)
    ));
}

#[test]
fn graph_scratch_budget_retains_one_row_of_intermediates_and_checks_overflow() {
    let expected = layout::<F>(16, 0, StoredPolynomialBasisV1::Lagrange);
    let context = StoredExpressionContextV1 {
        domain: expected,
        advice: &[expected],
        fixed_columns: 0,
        instance_columns: 0,
        challenge_phases: &[],
    };
    let mut graph = one_query(0);
    graph.num_intermediates = 3000;
    let bytes = (TILE + 3000 + TILE) * 32;
    let plan = prepare_stored_graph_v1(&graph, context, bytes).unwrap();
    assert_eq!(plan.scratch_bytes(), bytes);
    assert!(matches!(
        prepare_stored_graph_v1(&graph, context, bytes - 1),
        Err(StoredExpressionErrorV1::ScratchLimit)
    ));
    assert_eq!(
        field_count(usize::MAX, 0),
        Err(StoredExpressionErrorV1::Plan)
    );
    assert_eq!(
        field_count(0, usize::MAX),
        Err(StoredExpressionErrorV1::Plan)
    );
    assert_eq!(
        scratch_bytes::<F>(usize::MAX),
        Err(StoredExpressionErrorV1::Plan)
    );
}

#[test]
fn graph_partial_reads_and_consumer_failures_clear_all_owned_fields() {
    for failure in 0..6 {
        let graph = one_query(1);
        let expected = layout::<F>(9, 0, StoredPolynomialBasisV1::Lagrange);
        let context = StoredExpressionContextV1 {
            domain: expected,
            advice: &[expected],
            fixed_columns: 0,
            instance_columns: 0,
            challenge_phases: &[],
        };
        let plan = prepare_stored_graph_v1(&graph, context, 65536).unwrap();
        let recording = Rc::new(Recording::default());
        if failure == 1 {
            recording.fail.set(Some((0, 1)));
        }
        if failure == 2 {
            recording.unwind.set(Some((0, 1)));
        }
        if failure == 3 {
            recording.corrupt.set(Some((0, 1)));
        }
        let mut snapshot = Snapshot {
            layout: expected,
            values: vec![F::from(17).to_repr(); 512],
            recording: Rc::clone(&recording),
            poisoned: false,
        };
        let consumed = Cell::new(false);
        super::super::CLEAR_OBSERVATION.with(|record| record.set((0, true)));
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_stored_graph_chunk_v1(
                &plan,
                StoredRowTileV1 {
                    start: 0,
                    len: TILE,
                },
                &mut [StoredAdviceInputV1 {
                    expected,
                    snapshot: &mut snapshot,
                }],
                &[],
                &[],
                &[],
                F::ZERO,
                F::ZERO,
                F::ZERO,
                F::ZERO,
                &[F::ZERO; TILE],
                |actual| {
                    consumed.set(true);
                    assert!(actual.iter().all(|value| *value == F::from(17)));
                    assert_ne!(failure, 5, "injected consumer panic");
                    if failure == 4 {
                        Err(StoredExpressionErrorV1::Consumer)
                    } else {
                        Ok(())
                    }
                },
            )
        }));
        assert_eq!(consumed.get(), matches!(failure, 0 | 4 | 5));
        assert_eq!(result.is_err(), matches!(failure, 2 | 5));
        if let Ok(result) = result {
            assert_eq!(result.is_ok(), failure == 0);
        }
        super::super::CLEAR_OBSERVATION
            .with(|record| assert_eq!(record.get(), (plan.field_count, true)));
        assert!(!recording.busy.get());
        assert_eq!(snapshot.poisoned, matches!(failure, 1 | 2 | 3));
    }
}

#[test]
fn graph_runtime_binding_and_tile_rejection_happen_before_reads() {
    let graph = one_query(0);
    let expected = layout::<F>(9, 0, StoredPolynomialBasisV1::Lagrange);
    let context = StoredExpressionContextV1 {
        domain: expected,
        advice: &[expected],
        fixed_columns: 0,
        instance_columns: 0,
        challenge_phases: &[],
    };
    let plan = prepare_stored_graph_v1(&graph, context, 65536).unwrap();
    let recording = Rc::new(Recording::default());
    let mut snapshot = Snapshot {
        layout: expected,
        values: vec![F::ONE.to_repr(); 512],
        recording: Rc::clone(&recording),
        poisoned: false,
    };
    for (tile, input_layout, previous_len) in [
        (
            StoredRowTileV1 {
                start: 1,
                len: TILE,
            },
            expected,
            TILE,
        ),
        (
            StoredRowTileV1 {
                start: 512,
                len: TILE,
            },
            expected,
            TILE,
        ),
        (
            StoredRowTileV1 {
                start: 0,
                len: TILE,
            },
            expected,
            TILE - 1,
        ),
        (
            StoredRowTileV1 {
                start: 0,
                len: TILE,
            },
            expected,
            TILE + 1,
        ),
        (
            StoredRowTileV1 {
                start: 0,
                len: TILE,
            },
            layout::<F>(9, 1, StoredPolynomialBasisV1::Lagrange),
            TILE,
        ),
    ] {
        let result = with_stored_graph_chunk_v1(
            &plan,
            tile,
            &mut [StoredAdviceInputV1 {
                expected: input_layout,
                snapshot: &mut snapshot,
            }],
            &[],
            &[],
            &[],
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            &vec![F::ZERO; previous_len],
            |_| -> Result<(), _> { panic!("invalid input exposed") },
        );
        assert!(result.is_err());
    }
    assert!(recording.calls.borrow().is_empty());
}

#[test]
fn empty_graph_yields_zero_without_advice_reads() {
    let mut graph = GraphEvaluator::<EqAffine>::default();
    graph.finish_building();
    let domain = layout::<F>(0, 0, StoredPolynomialBasisV1::Lagrange);
    let context = StoredExpressionContextV1 {
        domain,
        advice: &[],
        fixed_columns: 0,
        instance_columns: 0,
        challenge_phases: &[],
    };
    let plan = prepare_stored_graph_v1(&graph, context, TILE * 32).unwrap();
    assert_eq!(plan.maximum_chunk_reads(), 0);
    with_stored_graph_chunk_v1::<_, Snapshot, _>(
        &plan,
        StoredRowTileV1 { start: 0, len: 1 },
        &mut [],
        &[],
        &[],
        &[],
        F::ZERO,
        F::ZERO,
        F::ZERO,
        F::ZERO,
        &[F::ONE],
        |actual| {
            assert_eq!(actual, &[F::ZERO]);
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn graph_final_target_and_reused_slot_order_match_existing_evaluator() {
    let mut graph = one_query(0);
    graph.num_intermediates = 2;
    graph.calculations = vec![
        CalculationInfo {
            calculation: Calculation::Store(ValueSource::Advice(0, 0)),
            target: 1,
        },
        CalculationInfo {
            calculation: Calculation::Double(ValueSource::Intermediate(1)),
            target: 0,
        },
        CalculationInfo {
            calculation: Calculation::Sub(
                ValueSource::Intermediate(0),
                ValueSource::Intermediate(1),
            ),
            target: 1,
        },
        CalculationInfo {
            calculation: Calculation::Square(ValueSource::Intermediate(1)),
            target: 0,
        },
    ];
    let domain = EvaluationDomain::<F>::new(3, 0);
    let original_advice = vec![domain.lagrange_from_vec(vec![F::from(19)])];
    let mut data = graph.instance();
    let retained = graph.evaluate(
        &mut data,
        &[],
        &original_advice,
        &[],
        &[],
        &F::ZERO,
        &F::ZERO,
        &F::ZERO,
        &F::ZERO,
        &F::ZERO,
        0,
        1,
        1,
    );
    assert_eq!(retained, F::from(361));
    let expected = layout::<F>(0, 0, StoredPolynomialBasisV1::Lagrange);
    let context = StoredExpressionContextV1 {
        domain: expected,
        advice: &[expected],
        fixed_columns: 0,
        instance_columns: 0,
        challenge_phases: &[],
    };
    let plan = prepare_stored_graph_v1(&graph, context, 65536).unwrap();
    let recording = Rc::new(Recording::default());
    let mut snapshot = Snapshot {
        layout: expected,
        values: vec![F::from(19).to_repr()],
        recording,
        poisoned: false,
    };
    with_stored_graph_chunk_v1(
        &plan,
        StoredRowTileV1 { start: 0, len: 1 },
        &mut [StoredAdviceInputV1 {
            expected,
            snapshot: &mut snapshot,
        }],
        &[],
        &[],
        &[],
        F::ZERO,
        F::ZERO,
        F::ZERO,
        F::ZERO,
        &[F::ZERO],
        |actual| {
            assert_eq!(actual, &[F::from(361)]);
            assert_eq!(actual, &[retained]);
            Ok(())
        },
    )
    .unwrap();
}
