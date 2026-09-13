//! Auxiliary-role ordering, ordinary arithmetic oracles and guarded scratch failure tests.
//!
//! The fixture binds metadata and computes values; it does not authenticate an actual key.
//! Only oracle construction materializes auxiliary banks. The source retains short instance
//! prefixes and computes fixed values, without domain-sized fixed/instance allocations.

use super::*;
use crate::poly::stored_advice::StoredPolynomialRoleV1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Fixed,
    Instance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Validate(usize),
    Value(Role, usize, usize),
}

#[derive(Clone, Copy)]
enum Fault {
    Getter {
        role: Role,
        index: usize,
        panic: bool,
    },
    Validate(usize),
}

struct Auxiliary<F: StoredAssignmentFieldV1> {
    domain: StoredPolynomialLayoutV1,
    advice: Vec<StoredPolynomialLayoutV1>,
    challenge_phases: Vec<u8>,
    fixed_columns: usize,
    instance_columns: usize,
    prefixes: Vec<Vec<F>>,
    events: Vec<Event>,
    validations: usize,
    getters: [usize; 2],
    fault: Option<Fault>,
    state: Rc<State>,
}

impl<F: StoredAssignmentFieldV1> Auxiliary<F> {
    fn new(expected: StoredExpressionContextV1<'_>, state: &Rc<State>) -> Self {
        Self {
            domain: expected.domain,
            advice: expected.advice.to_vec(),
            challenge_phases: expected.challenge_phases.to_vec(),
            fixed_columns: expected.fixed_columns,
            instance_columns: expected.instance_columns,
            prefixes: (0..expected.instance_columns)
                .map(|column| {
                    [
                        F::from(17 + column as u64),
                        F::from(31 + column as u64),
                        F::from(47 + column as u64),
                    ]
                    .into_iter()
                    .take(expected.domain.scalar_count().min(3))
                    .collect()
                })
                .collect(),
            events: Vec::new(),
            validations: 0,
            getters: [0; 2],
            fault: None,
            state: Rc::clone(state),
        }
    }

    fn fixed_formula(column: usize, row: usize) -> F {
        F::from(101 + column as u64 * 19 + row as u64 * 7)
    }

    fn get(&mut self, role: Role, column: usize, row: usize) -> Result<F, StoredExpressionErrorV1> {
        assert!(
            !self.state.active.get(),
            "auxiliary read inside advice plaintext callback"
        );
        let (slot, columns) = match role {
            Role::Fixed => (0, self.fixed_columns),
            Role::Instance => (1, self.instance_columns),
        };
        if column >= columns || row >= self.domain.scalar_count() {
            return Err(StoredExpressionErrorV1::Context);
        }
        let index = self.getters[slot];
        self.getters[slot] += 1;
        self.events.push(Event::Value(role, column, row));
        if let Some(Fault::Getter {
            role: at_role,
            index: at,
            panic,
        }) = self.fault
        {
            if at_role == role && at == index {
                assert!(!panic, "injected auxiliary getter unwind");
                return Err(StoredExpressionErrorV1::Store(
                    StoredPolynomialErrorV1::Authentication,
                ));
            }
        }
        match role {
            Role::Fixed => Ok(Self::fixed_formula(column, row)),
            Role::Instance => self
                .prefixes
                .get(column)
                .map(|prefix| prefix.get(row).copied().unwrap_or(F::ZERO))
                .ok_or(StoredExpressionErrorV1::Context),
        }
    }

    fn value_events(&self) -> Vec<(Role, usize, usize)> {
        self.events
            .iter()
            .filter_map(|event| match event {
                Event::Value(role, column, row) => Some((*role, *column, *row)),
                Event::Validate(_) => None,
            })
            .collect()
    }
}

impl<F: StoredAssignmentFieldV1> StoredAuxiliarySourceV1<F> for Auxiliary<F> {
    fn validate(
        &mut self,
        expected: StoredExpressionContextV1<'_>,
    ) -> Result<(), StoredExpressionErrorV1> {
        assert!(
            !self.state.active.get(),
            "auxiliary validation inside advice plaintext callback"
        );
        self.validations += 1;
        self.events.push(Event::Validate(self.validations));
        if matches!(self.fault, Some(Fault::Validate(at)) if at == self.validations) {
            return Err(StoredExpressionErrorV1::Store(
                StoredPolynomialErrorV1::Poisoned,
            ));
        }
        if self.domain != expected.domain
            || self.domain.field() != F::STORED_FIELD
            || (self.instance_columns != 0
                && self.domain.basis() != StoredPolynomialBasisV1::Lagrange)
            || self.advice != expected.advice
            || self.fixed_columns != expected.fixed_columns
            || self.instance_columns != expected.instance_columns
            || self.challenge_phases != expected.challenge_phases
            || self.prefixes.len() != self.instance_columns
            || self
                .prefixes
                .iter()
                .any(|prefix| prefix.len() > self.domain.scalar_count())
        {
            return Err(StoredExpressionErrorV1::Context);
        }
        Ok(())
    }

    fn fixed_value(&mut self, column: usize, row: usize) -> Result<F, StoredExpressionErrorV1> {
        self.get(Role::Fixed, column, row)
    }

    fn instance_value(&mut self, column: usize, row: usize) -> Result<F, StoredExpressionErrorV1> {
        self.get(Role::Instance, column, row)
    }
}

fn fixed<F>(column: usize, rotation: i32) -> Expression<F> {
    Expression::Fixed(FixedQuery {
        index: Some(899),
        column_index: column,
        rotation: Rotation(rotation),
    })
}

fn instance<F>(column: usize, rotation: i32) -> Expression<F> {
    Expression::Instance(InstanceQuery {
        index: Some(799),
        column_index: column,
        rotation: Rotation(rotation),
    })
}

fn bindings(layouts: &[StoredPolynomialLayoutV1]) -> StoredExpressionContextV1<'_> {
    StoredExpressionContextV1 {
        domain: layouts[0],
        advice: layouts,
        fixed_columns: 1,
        instance_columns: 1,
        challenge_phases: &[0, 1],
    }
}

fn execute<F: StoredAssignmentFieldV1, R>(
    plan: &StoredExpressionPlanV1<'_, F>,
    tile: StoredRowTileV1,
    snapshots: &mut [Snapshot],
    auxiliary: &mut Auxiliary<F>,
    challenges: &[F],
    consume: impl FnOnce(&[F]) -> Result<R, StoredExpressionErrorV1>,
) -> Result<R, StoredExpressionErrorV1> {
    let mut inputs: Vec<_> = snapshots
        .iter_mut()
        .zip(plan.context.advice)
        .map(|(snapshot, expected)| StoredAdviceInputV1 {
            expected: *expected,
            snapshot,
        })
        .collect();
    with_stored_expression_sources_v1(
        plan,
        tile,
        &mut StoredAdviceSliceReaderV1::new(&mut inputs),
        auxiliary,
        challenges,
        consume,
    )
}

fn mixed_oracle<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>() {
    for k in [4, 9] {
        let n = 1_usize << k;
        let domain = EvaluationDomain::<F>::new(3, k);
        let layouts = [layout::<F>(k, 0, 0), layout::<F>(k, 1, 1)];
        let admitted = bindings(&layouts);
        let state = Rc::new(State::default());
        let advice_bank: Vec<_> = (0..2)
            .map(|column| {
                domain.lagrange_from_vec(
                    (0..n)
                        .map(|row| F::from(3 + column * 11 + row as u64 * 13))
                        .collect(),
                )
            })
            .collect();
        let mut snapshots: Vec<_> = layouts
            .iter()
            .zip(&advice_bank)
            .map(|(binding, values)| snapshot(*binding, values, &state))
            .collect();
        let template = Auxiliary::<F>::new(admitted, &state);
        let fixed_bank = [domain.lagrange_from_vec(
            (0..n)
                .map(|row| Auxiliary::<F>::fixed_formula(0, row))
                .collect(),
        )];
        let instance_bank = [domain.lagrange_from_vec(
            (0..n)
                .map(|row| template.prefixes[0].get(row).copied().unwrap_or(F::ZERO))
                .collect(),
        )];
        let challenges = [F::from(123), F::from(991)];
        for expression in mixed_expressions::<F>() {
            let expected = super::super::super::evaluate(
                &expression,
                n,
                1,
                &fixed_bank,
                &advice_bank,
                &instance_bank,
                &challenges,
            );
            let plan = prepare_stored_expression_v1(&expression, admitted, 8 * TILE * 32).unwrap();
            for start in [0, n.saturating_sub(TILE)] {
                let tile = StoredRowTileV1 {
                    start,
                    len: TILE.min(n),
                };
                let mut source = Auxiliary::<F>::new(admitted, &state);
                CLEAR_OBSERVATION.with(|record| record.set((0, true)));
                let actual = execute(
                    &plan,
                    tile,
                    &mut snapshots,
                    &mut source,
                    &challenges,
                    |values| Ok(values.to_vec()),
                )
                .unwrap();
                assert_eq!(actual, expected[start..start + tile.len]);
                let binaries = plan
                    .instructions
                    .iter()
                    .filter(|instruction| {
                        matches!(
                            instruction,
                            Instruction::Sum { .. } | Instruction::Product { .. }
                        )
                    })
                    .count();
                CLEAR_OBSERVATION.with(|record| {
                    assert_eq!(record.get(), ((plan.slots + binaries) * TILE, true))
                });
                assert_eq!(source.events.first(), Some(&Event::Validate(1)));
                assert_eq!(
                    source.events.last(),
                    Some(&Event::Validate(source.validations))
                );
                assert!(!state.active.get());
            }
        }
    }
}

#[test]
fn both_pasta_auxiliary_sources_match_all_ordinary_expression_variants_and_clear_scratch() {
    mixed_oracle::<Fp>();
    mixed_oracle::<Fq>();
}

fn role_rotations<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>() {
    for k in [0, 1, 4, 9] {
        let n = 1_usize << k;
        let layouts = [layout::<F>(k, 0, 0)];
        let admitted = bindings(&layouts);
        let state = Rc::new(State::default());
        let mut snapshots = [snapshot(layouts[0], &vec![F::ONE; n], &state)];
        for rotation in [-257, -1, 0, 1, 257, i32::MIN, i32::MAX] {
            // The repeated fixed leaf must execute twice, in tree order, without a cache.
            let expression =
                (fixed::<F>(0, rotation) + instance::<F>(0, -1)) * fixed::<F>(0, rotation);
            let plan = prepare_stored_expression_v1(&expression, admitted, 3 * TILE * 32).unwrap();
            for start in [0, n.saturating_sub(TILE)] {
                let tile = StoredRowTileV1 {
                    start,
                    len: TILE.min(n),
                };
                let mut source = Auxiliary::<F>::new(admitted, &state);
                let prefix = source.prefixes[0].clone();
                let wrap = |row: usize, rot: i32| {
                    (row as i64 + i64::from(rot)).rem_euclid(n as i64) as usize
                };
                let expected: Vec<_> = (start..start + tile.len)
                    .map(|row| {
                        let f = Auxiliary::<F>::fixed_formula(0, wrap(row, rotation));
                        let i = prefix.get(wrap(row, -1)).copied().unwrap_or(F::ZERO);
                        (f + i) * f
                    })
                    .collect();
                let actual = execute(
                    &plan,
                    tile,
                    &mut snapshots,
                    &mut source,
                    &[F::ONE, F::ONE],
                    |values| Ok(values.to_vec()),
                )
                .unwrap();
                assert_eq!(actual, expected);
                let expected_events: Vec<_> = [
                    (Role::Fixed, rotation),
                    (Role::Instance, -1),
                    (Role::Fixed, rotation),
                ]
                .into_iter()
                .flat_map(|(role, rot)| {
                    (start..start + tile.len).map(move |row| (role, 0, wrap(row, rot)))
                })
                .collect();
                assert_eq!(source.value_events(), expected_events);
                assert_eq!(source.validations, 8); // Preflight + before/after each leaf + final.
                assert!(state.record.borrow().reads.is_empty());
            }
        }
    }
}

#[test]
fn both_pasta_fixed_and_instance_getters_preserve_role_leaf_order_and_extreme_rotations() {
    role_rotations::<Fp>();
    role_rotations::<Fq>();
}

fn raw_prefix<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>() {
    let k = 9;
    let n = 1_usize << k;
    let domain = EvaluationDomain::<F>::new(3, k);
    let layouts = [layout::<F>(k, 0, 0)];
    let admitted = bindings(&layouts);
    let state = Rc::new(State::default());
    let mut snapshots = [snapshot(layouts[0], &vec![F::ZERO; n], &state)];
    for prefix_len in [0, 1, 3] {
        for rotation in [-1, 1, 257] {
            let expression = instance::<F>(0, rotation);
            let plan = prepare_stored_expression_v1(&expression, admitted, TILE * 32).unwrap();
            let mut source = Auxiliary::<F>::new(admitted, &state);
            source.prefixes[0].truncate(prefix_len);
            assert_eq!(
                source.prefixes.iter().map(Vec::len).sum::<usize>(),
                prefix_len
            );
            let dense = [domain.lagrange_from_vec(
                (0..n)
                    .map(|row| source.prefixes[0].get(row).copied().unwrap_or(F::ZERO))
                    .collect(),
            )];
            let expected = super::super::super::evaluate(&expression, n, 1, &[], &[], &dense, &[]);
            for start in [0, 256] {
                let result = execute(
                    &plan,
                    StoredRowTileV1 { start, len: TILE },
                    &mut snapshots,
                    &mut source,
                    &[F::ONE, F::ONE],
                    |values| Ok(values.to_vec()),
                )
                .unwrap();
                assert_eq!(result, expected[start..start + TILE]);
            }
            assert_eq!(
                source.prefixes.iter().map(Vec::len).sum::<usize>(),
                prefix_len
            );
            assert_eq!(
                source.instance_value(0, n),
                Err(StoredExpressionErrorV1::Context)
            );
            assert_eq!(
                source.instance_value(1, 0),
                Err(StoredExpressionErrorV1::Context)
            );
            assert!(state.record.borrow().reads.is_empty());
        }
    }
}

#[test]
fn both_pasta_short_raw_instance_prefixes_supply_virtual_padding_without_a_source_bank() {
    raw_prefix::<Fp>();
    raw_prefix::<Fq>();
}

fn invalid_bindings<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>() {
    let layouts = [layout::<F>(9, 0, 0)];
    let admitted = bindings(&layouts);
    let expression = advice::<F>(0, 0, 0) + fixed::<F>(0, 0) + instance::<F>(0, 0);
    let plan = prepare_stored_expression_v1(&expression, admitted, 3 * TILE * 32).unwrap();
    for fault in 0..11 {
        let state = Rc::new(State::default());
        let mut snapshots = [snapshot(layouts[0], &vec![F::ONE; 512], &state)];
        let mut source = Auxiliary::<F>::new(admitted, &state);
        let old = source.domain;
        let make = |proof, field, basis, k| {
            StoredPolynomialLayoutV1::new(proof, old.ordinal(), field, basis, k, old.role())
                .unwrap()
        };
        source.domain = match fault {
            0 => make(
                [37; 32],
                if F::STORED_FIELD == StoredPastaFieldV1::Fp {
                    StoredPastaFieldV1::Fq
                } else {
                    StoredPastaFieldV1::Fp
                },
                old.basis(),
                old.k(),
            ),
            1 => make([37; 32], old.field(), old.basis(), 8),
            2 => make(
                [37; 32],
                old.field(),
                StoredPolynomialBasisV1::Coefficient,
                9,
            ),
            3 => make(
                [37; 32],
                old.field(),
                StoredPolynomialBasisV1::CosetPart {
                    extension_log: 2,
                    part: 1,
                },
                9,
            ),
            4 => make([38; 32], old.field(), old.basis(), old.k()),
            _ => old,
        };
        match fault {
            5 => source.fixed_columns = 0,
            6 => source.instance_columns = 0,
            7 => source.prefixes.clear(),
            8 => {
                source.advice.pop();
            }
            9 => source.challenge_phases.swap(0, 1),
            10 => source.prefixes[0].resize(513, F::ZERO),
            _ => (),
        }
        let mut consumed = false;
        CLEAR_OBSERVATION.with(|record| record.set((0, true)));
        assert_eq!(
            execute(
                &plan,
                StoredRowTileV1 {
                    start: 0,
                    len: TILE
                },
                &mut snapshots,
                &mut source,
                &[F::ONE, F::ONE],
                |_| {
                    consumed = true;
                    Ok(())
                }
            ),
            Err(StoredExpressionErrorV1::Context)
        );
        assert!(!consumed);
        assert_eq!(source.getters, [0; 2]);
        assert!(state.record.borrow().reads.is_empty());
        CLEAR_OBSERVATION.with(|record| assert_eq!(record.get(), (0, true)));
    }
    let state = Rc::new(State::default());
    let mut source = Auxiliary::<F>::new(admitted, &state);
    for (column, row) in [(1, 0), (0, 512), (usize::MAX, usize::MAX)] {
        assert_eq!(
            source.fixed_value(column, row),
            Err(StoredExpressionErrorV1::Context)
        );
        assert_eq!(
            source.instance_value(column, row),
            Err(StoredExpressionErrorV1::Context)
        );
    }
    assert_eq!(source.getters, [0; 2]);
    assert!(source.events.is_empty());
    for expression in [fixed::<F>(1, 0), instance::<F>(1, 0)] {
        assert_eq!(
            prepare_stored_expression_v1(&expression, admitted, usize::MAX).unwrap_err(),
            StoredExpressionErrorV1::Context
        );
    }
    // Exact coset part is part of the binding, even for matching extension and domain degree.
    let domain = StoredPolynomialLayoutV1::new(
        [37; 32],
        11,
        F::STORED_FIELD,
        StoredPolynomialBasisV1::CosetPart {
            extension_log: 2,
            part: 0,
        },
        9,
        StoredPolynomialRoleV1::Advice {
            column: 0,
            phase: 0,
        },
    )
    .unwrap();
    let cosets = [domain];
    let mut expected = bindings(&cosets);
    expected.instance_columns = 0;
    let mut source = Auxiliary::<F>::new(expected, &state);
    assert!(source.validate(expected).is_ok());
    source.domain = StoredPolynomialLayoutV1::new(
        [37; 32],
        11,
        F::STORED_FIELD,
        StoredPolynomialBasisV1::CosetPart {
            extension_log: 2,
            part: 1,
        },
        9,
        StoredPolynomialRoleV1::Advice {
            column: 0,
            phase: 0,
        },
    )
    .unwrap();
    assert_eq!(
        source.validate(expected),
        Err(StoredExpressionErrorV1::Context)
    );
    assert_eq!(source.getters, [0; 2]);

    // Equal coset labels still cannot turn raw-prefix zero padding into coset values.
    let expected = bindings(&cosets);
    let mut source = Auxiliary::<F>::new(expected, &state);
    assert_eq!(
        source.validate(expected),
        Err(StoredExpressionErrorV1::Context)
    );
    assert_eq!(source.getters, [0; 2]);

    // The preserved dense adapter rejects missing coordinates independently of the plan.
    use super::super::auxiliary::DenseAuxiliarySourceV1;
    let domain = EvaluationDomain::<F>::new(3, 9);
    let fixed_bank = [domain.lagrange_from_vec(vec![F::from(7); 512])];
    let instance_bank = [domain.lagrange_from_vec(vec![F::from(11); 512])];
    let mut dense = DenseAuxiliarySourceV1::new(admitted.domain, &fixed_bank, &instance_bank);
    assert!(dense.validate(admitted).is_ok());
    assert_eq!(dense.fixed_value(0, 511), Ok(F::from(7)));
    assert_eq!(dense.instance_value(0, 511), Ok(F::from(11)));
    for (column, row) in [(1, 0), (0, 512), (usize::MAX, usize::MAX)] {
        assert_eq!(
            dense.fixed_value(column, row),
            Err(StoredExpressionErrorV1::Context)
        );
        assert_eq!(
            dense.instance_value(column, row),
            Err(StoredExpressionErrorV1::Context)
        );
    }
    let short_domain = EvaluationDomain::<F>::new(3, 8);
    let short = [short_domain.lagrange_from_vec(vec![F::ONE; 256])];
    assert_eq!(
        DenseAuxiliarySourceV1::new(admitted.domain, &short, &instance_bank).validate(admitted),
        Err(StoredExpressionErrorV1::Context)
    );
    assert_eq!(
        DenseAuxiliarySourceV1::new(admitted.domain, &fixed_bank, &short).validate(admitted),
        Err(StoredExpressionErrorV1::Context)
    );
}

#[test]
fn both_pasta_auxiliary_binding_and_invalid_coordinates_fail_before_plaintext_or_scalar_reads() {
    invalid_bindings::<Fp>();
    invalid_bindings::<Fq>();
}

fn failure_expression<F: StoredAssignmentFieldV1>() -> Expression<F> {
    (advice::<F>(0, -1, 0) + fixed::<F>(0, 0)) + (instance::<F>(0, 0) + advice::<F>(0, 0, 0))
}

fn getter_failures<F: StoredAssignmentFieldV1>() {
    let layouts = [layout::<F>(9, 0, 0)];
    let admitted = bindings(&layouts);
    let plan =
        prepare_stored_expression_v1(&failure_expression::<F>(), admitted, 3 * TILE * 32).unwrap();
    assert_eq!(plan.slots, 3);
    for role in [Role::Fixed, Role::Instance] {
        for panic in [false, true] {
            let state = Rc::new(State::default());
            let mut snapshots = [snapshot(layouts[0], &vec![F::from(13); 512], &state)];
            let mut source = Auxiliary::<F>::new(admitted, &state);
            source.fault = Some(Fault::Getter {
                role,
                index: 5,
                panic,
            });
            let mut consumed = false;
            CLEAR_OBSERVATION.with(|record| record.set((0, true)));
            let result = catch_unwind(AssertUnwindSafe(|| {
                execute(
                    &plan,
                    StoredRowTileV1 {
                        start: 0,
                        len: TILE,
                    },
                    &mut snapshots,
                    &mut source,
                    &[F::ONE, F::ONE],
                    |_| {
                        consumed = true;
                        Ok(())
                    },
                )
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert_eq!(
                    result.unwrap(),
                    Err(StoredExpressionErrorV1::Store(
                        StoredPolynomialErrorV1::Authentication
                    ))
                );
            }
            assert!(!consumed);
            assert_eq!(state.record.borrow().reads, [(0, 1), (0, 0)]);
            assert!(!state.active.get());
            assert_eq!(
                source.getters,
                if role == Role::Fixed {
                    [6, 0]
                } else {
                    [TILE, 6]
                }
            );
            let cleared = if role == Role::Fixed { 3 } else { 4 };
            CLEAR_OBSERVATION.with(|record| assert_eq!(record.get(), (cleared * TILE, true)));
        }
    }
}

#[test]
fn both_pasta_mid_leaf_auxiliary_errors_and_unwinds_clear_partial_fields_and_stop_later_leaves() {
    getter_failures::<Fp>();
    getter_failures::<Fq>();
}

fn validation_failures<F: StoredAssignmentFieldV1>() {
    let layouts = [layout::<F>(4, 0, 0)];
    let admitted = bindings(&layouts);
    let plan =
        prepare_stored_expression_v1(&failure_expression::<F>(), admitted, 3 * TILE * 32).unwrap();
    // Preflight, before fixed, after fixed, before instance, after instance, final.
    for at in [1, 2, 3, 4, 5, 6] {
        let state = Rc::new(State::default());
        let mut snapshots = [snapshot(layouts[0], &vec![F::from(13); 16], &state)];
        let mut source = Auxiliary::<F>::new(admitted, &state);
        source.fault = Some(Fault::Validate(at));
        let mut consumed = false;
        CLEAR_OBSERVATION.with(|record| record.set((0, true)));
        assert_eq!(
            execute(
                &plan,
                StoredRowTileV1 { start: 0, len: 16 },
                &mut snapshots,
                &mut source,
                &[F::ONE, F::ONE],
                |_| {
                    consumed = true;
                    Ok(())
                }
            ),
            Err(StoredExpressionErrorV1::Store(
                StoredPolynomialErrorV1::Poisoned
            ))
        );
        assert!(!consumed);
        assert_eq!(source.validations, at);
        assert_eq!(
            source.getters,
            match at {
                1 | 2 => [0, 0],
                3 | 4 => [16, 0],
                _ => [16, 16],
            }
        );
        assert_eq!(
            state.record.borrow().reads.len(),
            match at {
                1 => 0,
                6 => 2,
                _ => 1,
            }
        );
        let cleared_slots = match at {
            1 => 0,
            2 | 3 => 3,
            4 | 5 => 4,
            6 => 6,
            _ => unreachable!(),
        };
        CLEAR_OBSERVATION.with(|record| assert_eq!(record.get(), (cleared_slots * TILE, true)));
        assert!(!state.active.get());
    }
}

#[test]
fn both_pasta_auxiliary_revalidation_after_leaves_and_before_consumer_clears_all_live_slots() {
    validation_failures::<Fp>();
    validation_failures::<Fq>();
}
