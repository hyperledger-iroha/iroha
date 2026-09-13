//! Actual completed-phase expression/graph composition and whole-owner failure regressions.
//!
//! These reuse the seeded IPA phase fixtures and their observed real blind destructor. Test
//! oracles deliberately materialize plaintext banks. They do not qualify an encrypted spool,
//! complete quotient/proof ownership, physical hardware, full-process RSS or performance.

use super::*;
use crate::{
    plonk::{
        Expression,
        stored::{
            StoredExpressionContextV1, StoredExpressionErrorV1, StoredRowTileV1,
            graph::{TestEvaluator, prepare_stored_graph_v1},
            prepare_stored_expression_v1,
        },
    },
    poly::Rotation,
};

const CHALLENGE_PHASES: &[u8] = &[1, 0, 2, 0];

fn model<F: StoredAssignmentFieldV1>() -> (ConstraintSystem<F>, Vec<Expression<F>>) {
    let mut meta = ConstraintSystem::default();
    let a0 = meta.advice_column();
    let a1 = meta.advice_column_in(SecondPhase);
    let c0 = meta.challenge_usable_after(SecondPhase);
    let a2 = meta.advice_column();
    let c1 = meta.challenge_usable_after(FirstPhase);
    let a3 = meta.advice_column_in(ThirdPhase);
    let c2 = meta.challenge_usable_after(ThirdPhase);
    let a4 = meta.advice_column_in(SecondPhase);
    let c3 = meta.challenge_usable_after(FirstPhase);
    let fixed = meta.fixed_column();
    let instance = meta.instance_column();
    let mut expressions = Vec::new();
    meta.create_gate("completed stored evaluation", |cells| {
        let left =
            cells.query_advice(a0, Rotation::next()) + cells.query_advice(a2, Rotation::prev());
        let right =
            cells.query_advice(a1, Rotation::cur()) * cells.query_fixed(fixed, Rotation::prev());
        let combined = (left * c1.expr() - right)
            + cells.query_instance(instance, Rotation::cur()) * c0.expr();
        let second = -(cells.query_advice(a3, Rotation::next())
            * cells.query_advice(a4, Rotation::prev()))
            * F::from(7)
            + c2.expr()
            + c3.expr();
        expressions = vec![combined, second];
        expressions.clone()
    });
    (meta, expressions)
}

fn complete_model<'params, C>(
    params: &'params ParamsIPA<C>,
    meta: &ConstraintSystem<C::Scalar>,
    backend: &Rc<Backend>,
) -> (
    CompleteStoredAdviceV1<'params, C, Snapshot>,
    CountingRng,
    CountingTranscript<C>,
)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let domain = EvaluationDomain::<C::Scalar>::new(4, params.k());
    let plan = admit_stored_phase_plan_v1(params, &domain, meta).unwrap();
    let phases = plan.phases.len();
    let mut phase_writers: Vec<_> = (0..phases)
        .map(|phase| writers(&plan, phase, 11 + phase as u64 * 13, backend))
        .collect();
    let mut owner =
        StoredPhaseAssignmentsV1::<C, Writer>::begin(plan, std::mem::take(&mut phase_writers[0]))
            .unwrap();
    let mut rng = CountingRng::new(backend);
    let mut transcript = CountingTranscript::<C>::new();
    for phase in 0..phases {
        let active = owner.active.as_ref().unwrap();
        let usable = active.session.plan.usable_rows;
        let columns = active.session.plan.phases[phase].columns.clone();
        let prior = active
            .session
            .challenges
            .get(1)
            .copied()
            .flatten()
            .unwrap_or(C::Scalar::ZERO);
        for column in columns {
            for (row, value) in phase_inputs(column, usable, prior) {
                owner.assign_discarding_value(column, row, value).unwrap();
            }
        }
        let committed = owner
            .finish(&mut rng)
            .unwrap()
            .absorb(&mut transcript)
            .unwrap();
        if phase + 1 == phases {
            return (committed.into_complete().unwrap(), rng, transcript);
        }
        owner = committed
            .begin_next(std::mem::take(&mut phase_writers[phase + 1]))
            .unwrap();
    }
    unreachable!("admitted plans always contain phase zero")
}

fn context(layouts: &[StoredPolynomialLayoutV1]) -> StoredExpressionContextV1<'_> {
    StoredExpressionContextV1 {
        domain: layouts[0],
        advice: layouts,
        fixed_columns: 1,
        instance_columns: 1,
        challenge_phases: CHALLENGE_PHASES,
    }
}

fn public_banks<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>(
    domain: &EvaluationDomain<F>,
) -> (
    Vec<Polynomial<F, LagrangeCoeff>>,
    Vec<Polynomial<F, LagrangeCoeff>>,
) {
    let size = 1_usize << domain.k();
    let bank = |factor, offset| {
        vec![
            domain.lagrange_from_vec(
                (0..size)
                    .map(|row| F::from((row * factor + offset) as u64))
                    .collect(),
            ),
        ]
    };
    (bank(7, 11), bank(13, 17))
}

fn composition<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    for k in [4, 9] {
        reset_blind_drops();
        let params = ParamsIPA::<C>::new(k);
        let (meta, expressions) = model::<C::Scalar>();
        let backend = Rc::new(Backend::default());
        let (mut complete, mut rng, transcript) = complete_model(&params, &meta, &backend);
        let layouts: Vec<_> = complete.layouts().unwrap().collect();
        let challenges: Vec<_> = complete.challenges().unwrap().collect();
        let session = complete.session.as_ref().unwrap();
        let allocation = session.columns.as_ptr();
        let snapshot_allocations: Vec<_> = session
            .columns
            .iter()
            .map(|column| column.snapshot.values.as_ptr())
            .collect();
        let domain = EvaluationDomain::<C::Scalar>::new(4, k);
        let size = 1_usize << k;
        let advice: Vec<_> = session
            .columns
            .iter()
            .map(|column| {
                domain.lagrange_from_vec(
                    column
                        .snapshot
                        .values
                        .iter()
                        .map(|bytes| {
                            Option::<C::Scalar>::from(C::Scalar::from_repr(*bytes)).unwrap()
                        })
                        .collect(),
                )
            })
            .collect();
        let (fixed, instance) = public_banks(&domain);
        let initial_draws = backend.record.borrow().rng_draws;
        let writes = transcript.writes;
        let squeezes = transcript.squeezes;
        let mut oracle_rng = rng.inner.clone();
        for expression in &expressions {
            let plan =
                prepare_stored_expression_v1(expression, context(&layouts), 1 << 20).unwrap();
            for start in (0..size).step_by(STORED_SCALARS_PER_CHUNK_V1).rev() {
                let tile = StoredRowTileV1 {
                    start,
                    len: STORED_SCALARS_PER_CHUNK_V1.min(size - start),
                };
                let expected: Vec<_> = (start..start + tile.len)
                    .map(|row| {
                        let at = |rotation: Rotation| {
                            (row as i64 + i64::from(rotation.0)).rem_euclid(size as i64) as usize
                        };
                        expression.evaluate(
                            &|value| value,
                            &|_| panic!("no selectors in admitted graph"),
                            &|query| fixed[query.column_index()][at(query.rotation())],
                            &|query| advice[query.column_index()][at(query.rotation())],
                            &|query| instance[query.column_index()][at(query.rotation())],
                            &|query| challenges[query.index()],
                            &|value| -value,
                            &|left, right| left + right,
                            &|left, right| left * right,
                            &|value, scalar| value * scalar,
                        )
                    })
                    .collect();
                let reads = backend.record.borrow().read_count;
                complete
                    .with_expression_tile(&plan, tile, &fixed, &instance, &challenges, |actual| {
                        assert!(!backend.busy.get());
                        assert_eq!(actual, expected.as_slice());
                        Ok(())
                    })
                    .unwrap();
                assert!(backend.record.borrow().read_count - reads <= plan.maximum_chunk_reads());
            }
        }
        let evaluator = TestEvaluator::<C>::new(&meta);
        let graph = &evaluator.custom_gates;
        let graph_plan = prepare_stored_graph_v1(graph, context(&layouts), 1 << 20).unwrap();
        let mut original = graph.instance();
        let [beta, gamma, theta, y] = [2, 3, 5, 7].map(C::Scalar::from);
        // Repeat complete traversal to prove that successful transactions restore the original
        // receipts, permitting later consumers without replay, replacement or bank conversion.
        for _ in 0..2 {
            for start in (0..size).step_by(STORED_SCALARS_PER_CHUNK_V1) {
                let tile = StoredRowTileV1 {
                    start,
                    len: STORED_SCALARS_PER_CHUNK_V1.min(size - start),
                };
                let previous: Vec<_> = (0..tile.len)
                    .map(|row| C::Scalar::from((start + row + 23) as u64))
                    .collect();
                let expected: Vec<_> = previous
                    .iter()
                    .enumerate()
                    .map(|(row, previous)| {
                        graph.evaluate(
                            &mut original,
                            &fixed,
                            &advice,
                            &instance,
                            &challenges,
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
                let reads = backend.record.borrow().read_count;
                complete
                    .with_graph_tile(
                        &graph_plan,
                        tile,
                        &fixed,
                        &instance,
                        &challenges,
                        beta,
                        gamma,
                        theta,
                        y,
                        &previous,
                        |actual| {
                            assert!(!backend.busy.get());
                            assert_eq!(actual, expected.as_slice());
                            Ok(())
                        },
                    )
                    .unwrap();
                assert!(
                    backend.record.borrow().read_count - reads <= graph_plan.maximum_chunk_reads()
                );
            }
        }
        assert_eq!(
            complete.session.as_ref().unwrap().columns.as_ptr(),
            allocation
        );
        assert_eq!(
            complete
                .session
                .as_ref()
                .unwrap()
                .columns
                .iter()
                .map(|column| column.snapshot.values.as_ptr())
                .collect::<Vec<_>>(),
            snapshot_allocations
        );
        assert!(std::ptr::eq(complete.params().unwrap(), &params));
        assert_eq!(
            complete.challenges().unwrap().collect::<Vec<_>>(),
            challenges
        );
        assert_eq!(backend.record.borrow().rng_draws, initial_draws);
        assert_eq!(transcript.writes, writes);
        assert_eq!(transcript.squeezes, squeezes);
        let mut next = [0; 64];
        let mut original_next = [0; 64];
        rng.fill_bytes(&mut next);
        oracle_rng.fill_bytes(&mut original_next);
        assert_eq!(next, original_next);
        BLIND_DROPS.with(|drops| assert_eq!(drops.get(), (0, 0)));
        drop(complete);
        assert_dropped(&backend, 5);
    }
}

#[test]
fn eq_completed_expression_and_graph_match_original_arithmetic_and_preserve_receipts() {
    composition::<EqAffine>();
}

#[test]
fn ep_completed_expression_and_graph_match_original_arithmetic_and_preserve_receipts() {
    composition::<EpAffine>();
}

fn failures(graph_mode: bool) {
    let params = ParamsIPA::<EqAffine>::new(9);
    let domain = EvaluationDomain::<Fp>::new(4, 9);
    let (meta, expressions) = model::<Fp>();
    let evaluator = TestEvaluator::<EqAffine>::new(&meta);
    for fault in 0..11 {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (mut complete, _, _) = complete_model(&params, &meta, &backend);
        let layouts: Vec<_> = complete.layouts().unwrap().collect();
        let challenges: Vec<_> = complete.challenges().unwrap().collect();
        let expression =
            prepare_stored_expression_v1(&expressions[0], context(&layouts), 1 << 20).unwrap();
        let graph =
            prepare_stored_graph_v1(&evaluator.custom_gates, context(&layouts), 1 << 20).unwrap();
        let (mut fixed, mut instance) = public_banks(&domain);
        let tile = StoredRowTileV1 {
            start: 0,
            len: if fault == 0 { 255 } else { 256 },
        };
        if fault == 1 {
            fixed.clear();
        }
        if fault == 9 {
            instance.clear();
        }
        if fault == 10 {
            complete.session.as_mut().unwrap().columns[4]
                .snapshot
                .layout
                .ordinal += 1;
        }
        {
            let mut record = backend.record.borrow_mut();
            let location = Some((0, 1));
            match fault {
                4 => record.fail_read = location,
                5 => record.panic_read = location,
                6 => record.corrupt_read = location,
                7 => record.short_read = location,
                8 => record.change_after_read = location,
                _ => (),
            }
        }
        let reads = backend.record.borrow().read_count;
        let called = Cell::new(false);
        let consumer = |_: &[Fp]| {
            called.set(true);
            assert!(!backend.busy.get());
            assert_ne!(fault, 3, "injected final tile consumer unwind");
            if fault == 2 {
                Err(StoredExpressionErrorV1::Consumer)
            } else {
                Ok(())
            }
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            if graph_mode {
                complete.with_graph_tile(
                    &graph,
                    tile,
                    &fixed,
                    &instance,
                    &challenges,
                    Fp::ONE,
                    Fp::ONE,
                    Fp::ONE,
                    Fp::ONE,
                    &[Fp::ZERO; 256],
                    consumer,
                )
            } else {
                complete.with_expression_tile(
                    &expression,
                    tile,
                    &fixed,
                    &instance,
                    &challenges,
                    consumer,
                )
            }
        }));
        if matches!(fault, 3 | 5) {
            assert!(
                result.is_err(),
                "outer catch must observe the original unwind"
            );
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(called.get(), matches!(fault, 2 | 3));
        if matches!(fault, 0 | 1 | 9 | 10) {
            assert_eq!(
                backend.record.borrow().read_count,
                reads,
                "preflight opens no plaintext"
            );
        } else {
            assert!(backend.record.borrow().read_count > reads);
        }
        assert_dropped(&backend, 5);
        assert_poisoned(&mut complete, layouts[0]);
        let reads_after = backend.record.borrow().read_count;
        // Retrying the actual concrete evaluator entry, not just metadata access, is refused.
        assert_eq!(
            complete.with_expression_tile(
                &expression,
                StoredRowTileV1 { start: 0, len: 256 },
                &fixed,
                &instance,
                &challenges,
                |_| panic!("poisoned owner exposed another result")
            ),
            Err::<(), _>(StoredPolynomialErrorV1::Poisoned.into())
        );
        assert_eq!(backend.record.borrow().read_count, reads_after);
    }
}

#[test]
fn completed_expression_preflight_consumer_storage_and_outer_caught_unwind_destroy_all_receipts() {
    failures(false);
}

#[test]
fn completed_graph_preflight_consumer_storage_and_outer_caught_unwind_destroy_all_receipts() {
    failures(true);
}

#[test]
fn completed_graph_previous_tile_shape_failure_destroys_owner_before_reads() {
    reset_blind_drops();
    let params = ParamsIPA::<EqAffine>::new(4);
    let (meta, _) = model::<Fp>();
    let backend = Rc::new(Backend::default());
    let (mut complete, _, _) = complete_model(&params, &meta, &backend);
    let layouts: Vec<_> = complete.layouts().unwrap().collect();
    let challenges: Vec<_> = complete.challenges().unwrap().collect();
    let evaluator = TestEvaluator::<EqAffine>::new(&meta);
    let plan =
        prepare_stored_graph_v1(&evaluator.custom_gates, context(&layouts), 1 << 20).unwrap();
    let (fixed, instance) = public_banks(&EvaluationDomain::new(4, 4));
    let reads = backend.record.borrow().read_count;
    assert_eq!(
        complete.with_graph_tile(
            &plan,
            StoredRowTileV1 { start: 0, len: 16 },
            &fixed,
            &instance,
            &challenges,
            Fp::ONE,
            Fp::ONE,
            Fp::ONE,
            Fp::ONE,
            &[Fp::ZERO; 15],
            |_| Ok(())
        ),
        Err(StoredExpressionErrorV1::Tile)
    );
    assert_eq!(backend.record.borrow().read_count, reads);
    assert_dropped(&backend, 5);
    assert_poisoned(&mut complete, layouts[0]);
}

#[test]
fn completed_context_challenges_schedule_coset_and_receipt_substitution_fail_before_reads() {
    let params = ParamsIPA::<EqAffine>::new(4);
    let (meta, _) = model::<Fp>();
    for fault in 0..9 {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (mut complete, _, _) = complete_model(&params, &meta, &backend);
        let original = complete.layouts().unwrap().next().unwrap();
        let mut layouts: Vec<_> = complete.layouts().unwrap().collect();
        let mut challenges: Vec<_> = complete.challenges().unwrap().collect();
        let mut phases = CHALLENGE_PHASES.to_vec();
        match fault {
            0 => challenges[0] += Fp::ONE,
            1 => phases[0] = 0,
            2 => {
                for layout in &mut layouts {
                    layout.k += 1;
                }
            }
            3 => {
                for layout in &mut layouts {
                    layout.basis = StoredPolynomialBasisV1::CosetPart {
                        extension_log: 1,
                        part: 0,
                    };
                }
            }
            4 => {
                for layout in &mut layouts {
                    layout.proof_context = [28; 32];
                }
            }
            5 => layouts[4].ordinal += 100,
            6 => set_phase(&mut layouts[4], 0),
            7 => {
                layouts.pop();
            }
            8 => complete.session.as_mut().unwrap().plan.field = StoredPastaFieldV1::Fq,
            _ => unreachable!(),
        }
        let admitted = StoredExpressionContextV1 {
            domain: layouts[0],
            advice: &layouts,
            fixed_columns: 1,
            instance_columns: 1,
            challenge_phases: &phases,
        };
        // A constant is deliberate: every original receipt/challenge is still bound even
        // when a particular expression does not query it. Each substituted plan is otherwise
        // valid under the raw helper's independently trusted-input contract.
        let plan =
            prepare_stored_expression_v1(&Expression::Constant(Fp::ONE), admitted, 8192).unwrap();
        let (fixed, instance) = public_banks(&EvaluationDomain::new(4, admitted.domain.k()));
        let reads = backend.record.borrow().read_count;
        assert_eq!(
            complete.with_expression_tile(
                &plan,
                StoredRowTileV1 {
                    start: 0,
                    len: admitted.domain.scalar_count()
                },
                &fixed,
                &instance,
                &challenges,
                |_| panic!("mismatched completed context")
            ),
            Err::<(), _>(StoredExpressionErrorV1::Context)
        );
        assert_eq!(backend.record.borrow().read_count, reads);
        assert_dropped(&backend, 5);
        assert_poisoned(&mut complete, original);
    }
    challenge_cardinality::<EqAffine>();
    challenge_cardinality::<EpAffine>();
}

// A constant keeps each malformed cardinality valid under the raw expression planner, so
// these failures must come from the complete owner's retained challenge schedule checks.
fn challenge_cardinality<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let (meta, _) = model::<C::Scalar>();
    for fault in 0..4 {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (mut complete, _, transcript) = complete_model(&params, &meta, &backend);
        let layouts: Vec<_> = complete.layouts().unwrap().collect();
        let mut challenges: Vec<_> = complete.challenges().unwrap().collect();
        let mut phases = CHALLENGE_PHASES.to_vec();
        let original_challenges = challenges.len();
        match fault {
            0 => {
                challenges.pop().unwrap();
                assert_eq!(challenges.len(), original_challenges - 1);
            }
            1 => {
                challenges.push(C::Scalar::ONE);
                assert_eq!(challenges.len(), original_challenges + 1);
            }
            2 => {
                phases.pop().unwrap();
                assert_eq!(phases.len(), original_challenges - 1);
            }
            3 => {
                phases.push(0);
                assert_eq!(phases.len(), original_challenges + 1);
            }
            _ => unreachable!(),
        }
        let admitted = StoredExpressionContextV1 {
            domain: layouts[0],
            advice: &layouts,
            fixed_columns: 1,
            instance_columns: 1,
            challenge_phases: &phases,
        };
        let plan =
            prepare_stored_expression_v1(&Expression::Constant(C::Scalar::ONE), admitted, 8192)
                .unwrap();
        let (fixed, instance) = public_banks(&EvaluationDomain::new(4, 4));
        let (reads, draws) = {
            let record = backend.record.borrow();
            (record.read_count, record.rng_draws)
        };
        let writes = transcript.writes;
        let squeezes = transcript.squeezes;
        let tile = StoredRowTileV1 { start: 0, len: 16 };
        assert_eq!(
            complete.with_expression_tile(&plan, tile, &fixed, &instance, &challenges, |_| {
                panic!("challenge cardinality mismatch exposed a completed result")
            }),
            Err::<(), _>(StoredExpressionErrorV1::Context),
        );
        assert_eq!(backend.record.borrow().read_count, reads);
        assert_eq!(backend.record.borrow().rng_draws, draws);
        assert_eq!(transcript.writes, writes);
        assert_eq!(transcript.squeezes, squeezes);
        assert_dropped(&backend, 5);
        assert_poisoned(&mut complete, layouts[0]);
        assert_eq!(
            complete.with_expression_tile(&plan, tile, &fixed, &instance, &challenges, |_| {
                panic!("poisoned cardinality owner exposed another result")
            }),
            Err::<(), _>(StoredPolynomialErrorV1::Poisoned.into()),
        );
        assert_eq!(backend.record.borrow().read_count, reads);
        assert_dropped(&backend, 5);
    }
}

fn empty_domain<C>(role: StoredPolynomialRoleV1)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for fault in 0..4 {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (mut complete, _, transcript) =
            complete_model(&params, &ConstraintSystem::default(), &backend);
        let k = if fault == 1 { 5 } else { 4 };
        let basis = if fault == 2 {
            StoredPolynomialBasisV1::CosetPart {
                extension_log: 1,
                part: 0,
            }
        } else {
            StoredPolynomialBasisV1::Lagrange
        };
        let label =
            StoredPolynomialLayoutV1::new([81; 32], 0, C::Scalar::STORED_FIELD, basis, k, role)
                .unwrap();
        let context = StoredExpressionContextV1 {
            domain: label,
            advice: &[],
            fixed_columns: 0,
            instance_columns: usize::from(fault == 3),
            challenge_phases: &[],
        };
        let plan =
            prepare_stored_expression_v1(&Expression::Constant(C::Scalar::from(19)), context, 8192)
                .unwrap();
        let domain = EvaluationDomain::<C::Scalar>::new(3, k);
        let instance = if fault == 3 {
            vec![domain.empty_lagrange()]
        } else {
            vec![]
        };
        let result = complete.with_expression_tile(
            &plan,
            StoredRowTileV1 {
                start: 0,
                len: 1_usize << k,
            },
            &[],
            &instance,
            &[],
            |actual| {
                assert_eq!(actual, &[C::Scalar::from(19); 16]);
                Ok(())
            },
        );
        if fault == 0 {
            assert_eq!(result, Ok(()));
            assert_eq!(complete.proof_context().unwrap(), None);
            assert_eq!(complete.layouts().unwrap().count(), 0);
            assert!(std::ptr::eq(complete.params().unwrap(), &params));
        } else {
            assert_eq!(result, Err(StoredExpressionErrorV1::Context));
            assert!(matches!(
                complete.params(),
                Err(StoredPhaseErrorV1::Poisoned)
            ));
        }
        assert_eq!(backend.record.borrow().read_count, 0);
        assert_eq!(backend.record.borrow().rng_draws, 0);
        assert_eq!(transcript.writes, 0);
        assert_eq!(transcript.squeezes, 0);
        drop(complete);
        assert_dropped(&backend, 0);
    }
}

#[test]
fn both_fields_empty_advice_still_binds_retained_domain_basis_and_instance_dimensions() {
    empty_domain::<EqAffine>(StoredPolynomialRoleV1::Advice {
        column: 0,
        phase: 0,
    });
    empty_domain::<EpAffine>(StoredPolynomialRoleV1::Advice {
        column: 0,
        phase: 0,
    });
}

#[test]
fn both_fields_lookup_labelled_empty_advice_geometry_keeps_domain_admission() {
    for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
        let role = StoredPolynomialRoleV1::LookupCompressed { lookup: 3, side };
        empty_domain::<EqAffine>(role);
        empty_domain::<EpAffine>(role);
    }
}
