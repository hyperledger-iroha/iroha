//! Differential arithmetic, bounded I/O, binding and cleanup tests for stored expression tiles.
//!
//! The recording backend is an in-memory oracle, not storage authentication or process-RSS
//! evidence. Encrypted Core integration and complete seeded-proof equivalence remain separate.

use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use ff::{Field, WithSmallOrderMulGroup};
use halo2curves::pasta::{Fp, Fq};

use super::*;
use crate::{
    plonk::{
        AdviceQuery, ConstraintSystem, FirstPhase, FixedQuery, InstanceQuery, SecondPhase,
        circuit::sealed::SealedPhase,
    },
    poly::{EvaluationDomain, Rotation, stored_advice::StoredPastaFieldV1},
};

#[derive(Default)]
struct Record {
    reads: Vec<(u32, u64)>,
    fail: Option<u64>,
    panic: Option<u64>,
    short: Option<u64>,
}

#[derive(Default)]
struct State {
    active: Cell<bool>,
    record: RefCell<Record>,
}

struct Window(Rc<State>);

impl Drop for Window {
    fn drop(&mut self) {
        self.0.active.set(false);
    }
}

struct Snapshot {
    layout: StoredAdviceLayoutV1,
    values: Vec<[u8; 32]>,
    state: Rc<State>,
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
        if self.layout != expected {
            return Err(StoredAdviceErrorV1::Context);
        }
        let count = expected.chunk_scalar_count(chunk)?;
        assert!(
            !self.state.active.replace(true),
            "nested authenticated callback"
        );
        let _window = Window(Rc::clone(&self.state));
        self.poisoned = true;
        let (fail, panic, short) = {
            let mut record = self.state.record.borrow_mut();
            record.reads.push((self.layout.column(), chunk));
            (record.fail, record.panic, record.short)
        };
        if fail == Some(chunk) {
            return Err(StoredAdviceErrorV1::Authentication);
        }
        assert!(panic != Some(chunk), "injected storage unwind");
        let start = chunk as usize * TILE;
        let end = start + count - usize::from(short == Some(chunk));
        let result = consume(&self.values[start..end]);
        if result.is_ok() {
            self.poisoned = false;
        }
        result
    }

    fn with_column<R>(
        &mut self,
        _: StoredAdviceLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1> {
        panic!("expression evaluation must never materialize a column")
    }
}

fn layout<F: StoredAssignmentFieldV1>(k: u32, column: u32, phase: u8) -> StoredAdviceLayoutV1 {
    StoredAdviceLayoutV1::new(
        [37; 32],
        u64::from(column) + 11,
        F::STORED_FIELD,
        StoredPolynomialBasisV1::Lagrange,
        k,
        column,
        phase,
    )
    .unwrap()
}

fn context<'a>(layouts: &'a [StoredAdviceLayoutV1]) -> StoredExpressionContextV1<'a> {
    StoredExpressionContextV1 {
        domain: layouts[0],
        advice: layouts,
        fixed_columns: 0,
        instance_columns: 0,
        challenge_phases: &[],
    }
}

fn advice<F>(column: usize, rotation: i32, phase: u8) -> Expression<F> {
    Expression::Advice(AdviceQuery {
        // Deliberately unrelated to physical column; a query-cache index is not a column.
        index: Some(907),
        column_index: column,
        rotation: Rotation(rotation),
        phase: if phase == 0 {
            FirstPhase.to_sealed()
        } else {
            SecondPhase.to_sealed()
        },
    })
}

fn snapshot<F: StoredAssignmentFieldV1>(
    layout: StoredAdviceLayoutV1,
    values: &[F],
    state: &Rc<State>,
) -> Snapshot {
    Snapshot {
        layout,
        values: values.iter().map(|value| value.to_repr()).collect(),
        state: Rc::clone(state),
        poisoned: false,
    }
}

fn run<F: StoredAssignmentFieldV1>(
    plan: &StoredExpressionPlanV1<'_, F>,
    snapshots: &mut [Snapshot],
    tile: StoredRowTileV1,
    fixed: &[Polynomial<F, LagrangeCoeff>],
    instance: &[Polynomial<F, LagrangeCoeff>],
    challenges: &[F],
) -> Result<Vec<F>, StoredExpressionErrorV1> {
    let mut inputs: Vec<_> = snapshots
        .iter_mut()
        .zip(plan.context.advice)
        .map(|(snapshot, expected)| StoredAdviceInputV1 {
            expected: *expected,
            snapshot,
        })
        .collect();
    with_stored_expression_chunk_v1(
        plan,
        tile,
        &mut inputs,
        fixed,
        instance,
        challenges,
        |values| Ok(values.to_vec()),
    )
}

fn mixed_expressions<F: StoredAssignmentFieldV1>() -> Vec<Expression<F>> {
    let mut cs = ConstraintSystem::<F>::default();
    cs.advice_column();
    cs.advice_column_in(SecondPhase);
    let challenge0 = cs.challenge_usable_after(FirstPhase);
    let challenge1 = cs.challenge_usable_after(SecondPhase);
    let a = advice(0, -1, 0);
    let b = advice(1, 257, 1);
    let fixed = Expression::Fixed(FixedQuery {
        index: Some(808),
        column_index: 0,
        rotation: Rotation(1),
    });
    let instance = Expression::Instance(InstanceQuery {
        index: Some(707),
        column_index: 0,
        rotation: Rotation(-257),
    });
    vec![
        Expression::Constant(F::ZERO),
        Expression::Constant(F::from(7)),
        a.clone(),
        b.clone(),
        fixed.clone(),
        instance.clone(),
        challenge0.expr(),
        challenge1.expr(),
        Expression::Negated(Box::new(a.clone())),
        Expression::Scaled(Box::new(b.clone()), F::from(3)),
        Expression::Sum(Box::new(a.clone()), Box::new(b.clone())),
        Expression::Product(Box::new(a.clone()), Box::new(b.clone())),
        // Balanced operands; duplicated advice leaves must each execute without a cache.
        Expression::Product(
            Box::new(Expression::Sum(Box::new(a.clone()), Box::new(fixed))),
            Box::new(Expression::Sum(Box::new(a.clone()), Box::new(instance))),
        ),
        Expression::Scaled(
            Box::new(Expression::Negated(Box::new(Expression::Sum(
                Box::new(challenge1.expr()),
                Box::new(Expression::Product(
                    Box::new(a),
                    Box::new(Expression::Sum(Box::new(b), Box::new(challenge0.expr()))),
                )),
            )))),
            F::ZERO,
        ),
    ]
}

fn differential<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>(coset: bool) {
    let k = 9;
    let domain = EvaluationDomain::<F>::new(5, k);
    let mut layouts = vec![layout::<F>(k, 0, 0), layout::<F>(k, 1, 1)];
    let mut banks: Vec<_> = (0..4)
        .map(|column| {
            domain.lagrange_from_vec(
                (0..1_usize << k)
                    .map(|row| F::from((row * 29 + column * 17 + 3) as u64))
                    .collect(),
            )
        })
        .collect();
    if coset {
        let basis = StoredPolynomialBasisV1::CosetPart {
            extension_log: domain.extended_k() - domain.k(),
            part: 3,
        };
        for (column, binding) in layouts.iter_mut().enumerate() {
            *binding = StoredAdviceLayoutV1::new(
                [37; 32],
                column as u64 + 11,
                F::STORED_FIELD,
                basis,
                k,
                column as u32,
                column as u8,
            )
            .unwrap();
        }
        let factor = domain.get_extended_omega().pow_vartime([3]);
        banks = banks
            .into_iter()
            .map(|column| domain.coeff_to_extended_part(domain.lagrange_to_coeff(column), factor))
            .collect();
    }
    let advice_bank = &banks[..2];
    let fixed = &banks[2..3];
    let instance = &banks[3..];
    let challenges = [F::from(123), F::from(991)];
    let mut admitted = context(&layouts);
    admitted.fixed_columns = 1;
    admitted.instance_columns = 1;
    admitted.challenge_phases = &[0, 1];
    let state = Rc::new(State::default());
    let mut snapshots: Vec<_> = layouts
        .iter()
        .zip(advice_bank)
        .map(|(layout, column)| snapshot(*layout, column, &state))
        .collect();
    for expression in mixed_expressions::<F>() {
        let expected = super::super::evaluate(
            &expression,
            1 << k,
            1,
            fixed,
            advice_bank,
            instance,
            &challenges,
        );
        let plan = prepare_stored_expression_v1(&expression, admitted, 8 * TILE * 32).unwrap();
        // Repeat and reorder reads to prove immutable access and domain-edge behavior.
        for start in [256, 0, 256] {
            state.record.borrow_mut().reads.clear();
            let actual = run(
                &plan,
                &mut snapshots,
                StoredRowTileV1 { start, len: TILE },
                fixed,
                instance,
                &challenges,
            )
            .unwrap();
            assert_eq!(actual, expected[start..start + TILE]);
            assert!(state.record.borrow().reads.len() <= plan.maximum_chunk_reads());
            assert!(!state.active.get());
        }
    }
}

#[test]
fn fp_matches_every_ordinary_expression_variant() {
    differential::<Fp>(false);
}
#[test]
fn fq_matches_every_ordinary_expression_variant() {
    differential::<Fq>(false);
}
#[test]
fn fp_matches_exact_coset_part_evaluation() {
    differential::<Fp>(true);
}
#[test]
fn fq_matches_exact_coset_part_evaluation() {
    differential::<Fq>(true);
}

fn rotations<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>() {
    for k in [0, 1, 8, 9, 16] {
        let n = 1_usize << k;
        let domain = EvaluationDomain::<F>::new(2, k);
        let column =
            domain.lagrange_from_vec((0..n).map(|row| F::from((row + 1) as u64)).collect());
        let layouts = [layout::<F>(k, 0, 0)];
        let state = Rc::new(State::default());
        let mut snapshots = [snapshot(layouts[0], &column, &state)];
        let rotations = [
            0,
            1,
            -1,
            n as i32,
            -(n as i32),
            255,
            256,
            257,
            -255,
            -256,
            -257,
            30271,
            -30271,
            i32::MIN,
            i32::MAX,
        ];
        for rotation in rotations {
            let expression = advice::<F>(0, rotation, 0);
            let plan =
                prepare_stored_expression_v1(&expression, context(&layouts), TILE * 32).unwrap();
            assert_eq!(plan.scratch_bytes(), TILE * 32);
            let oracle = if (rotation as i64 + n as i64 - 1) <= i32::MAX as i64 {
                Some(super::super::evaluate(
                    &expression,
                    n,
                    1,
                    &[],
                    std::slice::from_ref(&column),
                    &[],
                    &[],
                ))
            } else {
                None
            };
            for start in [0, n.saturating_sub(TILE)] {
                let len = TILE.min(n - start);
                state.record.borrow_mut().reads.clear();
                let actual = run(
                    &plan,
                    &mut snapshots,
                    StoredRowTileV1 { start, len },
                    &[],
                    &[],
                    &[],
                )
                .unwrap();
                let expected: Vec<_> = (start..start + len)
                    .map(|row| {
                        column[((row as i64 + i64::from(rotation)).rem_euclid(n as i64)) as usize]
                    })
                    .collect();
                assert_eq!(actual, expected);
                if let Some(oracle) = &oracle {
                    assert_eq!(actual, oracle[start..start + len]);
                }
                let mut chunks = Vec::new();
                for row in start..start + len {
                    let chunk = ((row as i64 + i64::from(rotation)).rem_euclid(n as i64)) as u64
                        / TILE as u64;
                    if !chunks.contains(&(0, chunk)) {
                        chunks.push((0, chunk));
                    }
                }
                assert_eq!(state.record.borrow().reads, chunks);
                assert!(chunks.len() <= if n <= TILE { 1 } else { 2 });
            }
        }
    }
}

#[test]
fn fp_rotation_geometry_and_exact_authenticated_read_counts() {
    rotations::<Fp>();
}
#[test]
fn fq_rotation_geometry_and_exact_authenticated_read_counts() {
    rotations::<Fq>();
}

#[test]
fn instruction_order_retains_tree_without_reassociation_or_zero_shortcuts() {
    let layouts = [layout::<Fp>(9, 0, 0)];
    let leaf = || advice::<Fp>(0, 0, 0);
    let expression = Expression::Scaled(
        Box::new(Expression::Sum(
            Box::new(Expression::Negated(Box::new(leaf()))),
            Box::new(Expression::Product(Box::new(leaf()), Box::new(leaf()))),
        )),
        Fp::ZERO,
    );
    let plan = prepare_stored_expression_v1(&expression, context(&layouts), 3 * TILE * 32).unwrap();
    assert!(matches!(
        plan.instructions.as_slice(),
        [
            Instruction::Advice { target: 0, .. },
            Instruction::Negated { target: 0 },
            Instruction::Advice { target: 1, .. },
            Instruction::Advice { target: 2, .. },
            Instruction::Product { left: 1, right: 2 },
            Instruction::Sum { left: 0, right: 1 },
            Instruction::Scaled { target: 0, .. },
        ]
    ));
    assert_eq!(plan.scratch_bytes(), 3 * TILE * 32);
    let state = Rc::new(State::default());
    let mut snapshots = [snapshot(layouts[0], &vec![Fp::ONE; 512], &state)];
    CLEAR_OBSERVATION.with(|record| record.set((0, true)));
    assert_eq!(
        run(
            &plan,
            &mut snapshots,
            StoredRowTileV1 {
                start: 0,
                len: TILE
            },
            &[],
            &[],
            &[]
        )
        .unwrap(),
        vec![Fp::ZERO; TILE]
    );
    assert_eq!(state.record.borrow().reads, vec![(0, 0); 3]);
    // Two right slots are wiped at release and all three allocated slots are wiped at drop.
    CLEAR_OBSERVATION.with(|record| assert_eq!(record.get(), (5 * TILE, true)));
}

#[test]
fn scratch_bound_rejects_skewed_trees_and_checked_overflow_before_reads() {
    let layouts = [layout::<Fp>(16, 0, 0)];
    let mut expression = advice::<Fp>(0, 0, 0);
    for _ in 0..32 {
        expression = Expression::Sum(Box::new(advice(0, 0, 0)), Box::new(expression));
    }
    assert_eq!(
        prepare_stored_expression_v1(&expression, context(&layouts), 32 * TILE * 32).unwrap_err(),
        StoredExpressionErrorV1::ScratchLimit
    );
    let plan =
        prepare_stored_expression_v1(&expression, context(&layouts), 33 * TILE * 32).unwrap();
    assert_eq!(plan.slots, 33);
    assert_eq!(plan.maximum_chunk_reads(), 66);
    assert_eq!(
        checked_scratch_bytes::<Fp>(usize::MAX),
        Err(StoredExpressionErrorV1::Plan)
    );
    assert_eq!(
        checked_scratch_bytes::<Fq>(usize::MAX / TILE + 1),
        Err(StoredExpressionErrorV1::Plan)
    );
}

#[test]
fn released_slots_are_reused_with_domain_independent_witness_payload() {
    for k in [9, 16] {
        let layouts = [layout::<Fp>(k, 0, 0)];
        let sum = || {
            Expression::Sum(
                Box::new(advice::<Fp>(0, 0, 0)),
                Box::new(advice::<Fp>(0, 0, 0)),
            )
        };
        let expression = Expression::Sum(Box::new(sum()), Box::new(sum()));
        let plan =
            prepare_stored_expression_v1(&expression, context(&layouts), 3 * TILE * 32).unwrap();
        assert_eq!(plan.scratch_bytes(), 24_576);
        let state = Rc::new(State::default());
        let mut snapshots = [snapshot(layouts[0], &vec![Fp::ONE; 1 << k], &state)];
        CLEAR_OBSERVATION.with(|record| record.set((0, true)));
        assert_eq!(
            run(
                &plan,
                &mut snapshots,
                StoredRowTileV1 {
                    start: 0,
                    len: TILE
                },
                &[],
                &[],
                &[]
            )
            .unwrap(),
            vec![Fp::from(4); TILE]
        );
        assert_eq!(state.record.borrow().reads, vec![(0, 0); 4]);
        // Slot 1 is cleared after the left sum, then reused by the right subtree.
        // Three binary releases plus all three initialized slots are observed wiped.
        CLEAR_OBSERVATION.with(|record| assert_eq!(record.get(), (6 * TILE, true)));
    }
}

#[test]
fn context_rejects_duplicate_missing_cross_proof_field_basis_k_and_phase_bindings() {
    let original = [layout::<Fp>(9, 0, 0), layout::<Fp>(9, 1, 1)];
    let expression = advice::<Fp>(1, 0, 1);
    let make = |proof, ordinal, field, basis, k, column, phase| {
        StoredAdviceLayoutV1::new(proof, ordinal, field, basis, k, column, phase).unwrap()
    };
    let variants = [
        original[0], // duplicate physical coordinate and ordinal
        make(
            [38; 32],
            12,
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            9,
            1,
            1,
        ),
        make(
            [37; 32],
            11,
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            9,
            1,
            1,
        ),
        make(
            [37; 32],
            12,
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Lagrange,
            9,
            1,
            1,
        ),
        make(
            [37; 32],
            12,
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            9,
            1,
            1,
        ),
        make(
            [37; 32],
            12,
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::CosetPart {
                extension_log: 2,
                part: 1,
            },
            9,
            1,
            1,
        ),
        make(
            [37; 32],
            12,
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            8,
            1,
            1,
        ),
        make(
            [37; 32],
            12,
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            9,
            2,
            1,
        ),
        make(
            [37; 32],
            12,
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            9,
            1,
            0,
        ),
    ];
    for wrong in variants {
        let bindings = [original[0], wrong];
        assert_eq!(
            prepare_stored_expression_v1(&expression, context(&bindings), usize::MAX).unwrap_err(),
            StoredExpressionErrorV1::Context
        );
    }
    assert_eq!(
        prepare_stored_expression_v1(&expression, context(&original[..1]), usize::MAX).unwrap_err(),
        StoredExpressionErrorV1::Context
    );
    let wrong_domain = [make(
        [37; 32],
        11,
        StoredPastaFieldV1::Fp,
        StoredPolynomialBasisV1::Coefficient,
        9,
        0,
        0,
    )];
    assert_eq!(
        prepare_stored_expression_v1(
            &Expression::Constant(Fp::ONE),
            context(&wrong_domain),
            usize::MAX
        )
        .unwrap_err(),
        StoredExpressionErrorV1::Context
    );
    assert_eq!(
        prepare_stored_expression_v1(
            &Expression::Constant(Fq::ONE),
            context(&original),
            usize::MAX
        )
        .unwrap_err(),
        StoredExpressionErrorV1::Context
    );
    let first = make(
        [37; 32],
        11,
        StoredPastaFieldV1::Fp,
        StoredPolynomialBasisV1::CosetPart {
            extension_log: 2,
            part: 0,
        },
        9,
        0,
        0,
    );
    let second = make(
        [37; 32],
        12,
        StoredPastaFieldV1::Fp,
        StoredPolynomialBasisV1::CosetPart {
            extension_log: 2,
            part: 1,
        },
        9,
        1,
        1,
    );
    assert_eq!(
        prepare_stored_expression_v1(&expression, context(&[first, second]), usize::MAX)
            .unwrap_err(),
        StoredExpressionErrorV1::Context
    );
    assert!(first.same_proof_context(second));
    assert!(!first.same_proof_context(make(
        [38; 32],
        11,
        StoredPastaFieldV1::Fp,
        StoredPolynomialBasisV1::Lagrange,
        9,
        0,
        0
    )));
}

#[test]
fn unadmitted_selector_column_and_challenge_fail_during_planning() {
    let layouts = [layout::<Fp>(9, 0, 0)];
    let mut cs = ConstraintSystem::<Fp>::default();
    cs.advice_column();
    let selector = cs.selector();
    let challenge = cs.challenge_usable_after(FirstPhase);
    let cases: [(Expression<Fp>, StoredExpressionErrorV1); 4] = [
        (selector.expr(), StoredExpressionErrorV1::Plan),
        (
            Expression::Fixed(FixedQuery {
                index: None,
                column_index: 0,
                rotation: Rotation(0),
            }),
            StoredExpressionErrorV1::Context,
        ),
        (
            Expression::Instance(InstanceQuery {
                index: None,
                column_index: 0,
                rotation: Rotation(0),
            }),
            StoredExpressionErrorV1::Context,
        ),
        (challenge.expr(), StoredExpressionErrorV1::Context),
    ];
    for (expression, error) in cases {
        assert_eq!(
            prepare_stored_expression_v1(&expression, context(&layouts), usize::MAX).unwrap_err(),
            error
        );
    }
    let mut admitted = context(&layouts);
    admitted.challenge_phases = &[1];
    assert_eq!(
        prepare_stored_expression_v1(&challenge.expr::<Fp>(), admitted, usize::MAX).unwrap_err(),
        StoredExpressionErrorV1::Context
    );
    admitted.challenge_phases = &[3];
    assert_eq!(
        prepare_stored_expression_v1(&Expression::Constant(Fp::ONE), admitted, usize::MAX)
            .unwrap_err(),
        StoredExpressionErrorV1::Context
    );
}

#[test]
fn invalid_tile_binding_and_bank_dimensions_expose_no_plaintext_or_result() {
    let layouts = [layout::<Fp>(9, 0, 0)];
    let state = Rc::new(State::default());
    let mut source = snapshot(layouts[0], &vec![Fp::ONE; 512], &state);
    let expression = advice::<Fp>(0, 1, 0);
    let plan = prepare_stored_expression_v1(&expression, context(&layouts), TILE * 32).unwrap();
    for tile in [
        StoredRowTileV1 {
            start: 1,
            len: TILE,
        },
        StoredRowTileV1 { start: 0, len: 0 },
        StoredRowTileV1 { start: 0, len: 255 },
        StoredRowTileV1 {
            start: 512,
            len: TILE,
        },
        StoredRowTileV1 {
            start: usize::MAX,
            len: TILE,
        },
    ] {
        assert_eq!(
            with_stored_expression_chunk_v1(
                &plan,
                tile,
                &mut [StoredAdviceInputV1 {
                    expected: layouts[0],
                    snapshot: &mut source
                }],
                &[],
                &[],
                &[],
                |_| -> Result<(), _> { panic!("invalid tile exposed output") }
            ),
            Err(StoredExpressionErrorV1::Tile)
        );
    }
    let tile = StoredRowTileV1 {
        start: 0,
        len: TILE,
    };
    assert_eq!(
        read_advice(
            &mut StoredAdviceInputV1 {
                expected: layouts[0],
                snapshot: &mut source
            },
            tile,
            1,
            &mut [Fp::ZERO; 1],
        ),
        Err(StoredExpressionErrorV1::Tile),
    );
    let wrong = StoredAdviceLayoutV1::new(
        [37; 32],
        91,
        StoredPastaFieldV1::Fp,
        StoredPolynomialBasisV1::Lagrange,
        9,
        0,
        0,
    )
    .unwrap();
    assert_eq!(
        with_stored_expression_chunk_v1(
            &plan,
            tile,
            &mut [StoredAdviceInputV1 {
                expected: wrong,
                snapshot: &mut source
            }],
            &[],
            &[],
            &[],
            |_| -> Result<(), _> { panic!("wrong expected identity exposed output") }
        ),
        Err(StoredExpressionErrorV1::Context)
    );
    source.layout = wrong;
    assert_eq!(
        run(
            &plan,
            std::slice::from_mut(&mut source),
            tile,
            &[],
            &[],
            &[]
        ),
        Err(StoredExpressionErrorV1::Context)
    );
    source.layout = layouts[0];
    assert_eq!(
        run(&plan, &mut [], tile, &[], &[], &[]),
        Err(StoredExpressionErrorV1::Context)
    );
    assert_eq!(
        run(
            &plan,
            std::slice::from_mut(&mut source),
            tile,
            &[],
            &[],
            &[Fp::ONE]
        ),
        Err(StoredExpressionErrorV1::Context)
    );
    let domain = EvaluationDomain::<Fp>::new(2, 8);
    let short = [domain.lagrange_from_vec(vec![Fp::ONE; 256])];
    let mut admitted = context(&layouts);
    admitted.fixed_columns = 1;
    admitted.instance_columns = 1;
    let plan = prepare_stored_expression_v1(&expression, admitted, TILE * 32).unwrap();
    assert_eq!(
        run(
            &plan,
            std::slice::from_mut(&mut source),
            tile,
            &short,
            &short,
            &[]
        ),
        Err(StoredExpressionErrorV1::Context)
    );
    assert!(state.record.borrow().reads.is_empty());
    assert!(!source.poisoned);
}

fn read_failure<F: StoredAssignmentFieldV1>(kind: u8) {
    let layouts = [layout::<F>(9, 0, 0)];
    let state = Rc::new(State::default());
    let mut source = snapshot(layouts[0], &vec![F::ONE; 512], &state);
    match kind {
        0 => state.record.borrow_mut().fail = Some(1),
        1 => source.values[256] = [255; 32],
        2 => state.record.borrow_mut().short = Some(1),
        3 => state.record.borrow_mut().panic = Some(1),
        _ => unreachable!(),
    }
    let plan =
        prepare_stored_expression_v1(&advice::<F>(0, 1, 0), context(&layouts), TILE * 32).unwrap();
    let called = Cell::new(false);
    CLEAR_OBSERVATION.with(|record| record.set((0, true)));
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_stored_expression_chunk_v1(
            &plan,
            StoredRowTileV1 {
                start: 0,
                len: TILE,
            },
            &mut [StoredAdviceInputV1 {
                expected: layouts[0],
                snapshot: &mut source,
            }],
            &[],
            &[],
            &[],
            |_| {
                called.set(true);
                Ok(())
            },
        )
    }));
    if kind == 3 {
        assert!(result.is_err());
    } else {
        let expected = if kind == 0 {
            StoredAdviceErrorV1::Authentication
        } else {
            StoredAdviceErrorV1::Encoding
        };
        assert_eq!(
            result.unwrap(),
            Err(StoredExpressionErrorV1::Store(expected))
        );
    }
    assert!(!called.get());
    assert_eq!(state.record.borrow().reads, vec![(0, 0), (0, 1)]);
    assert!(!state.active.get());
    assert!(source.poisoned);
    CLEAR_OBSERVATION.with(|record| assert_eq!(record.get(), (TILE, true)));
}

#[test]
fn second_chunk_authentication_failure_clears_partial_result() {
    read_failure::<Fp>(0);
    read_failure::<Fq>(0);
}
#[test]
fn second_chunk_noncanonical_scalar_clears_partial_result() {
    read_failure::<Fp>(1);
    read_failure::<Fq>(1);
}
#[test]
fn second_chunk_short_encoding_clears_partial_result() {
    read_failure::<Fp>(2);
    read_failure::<Fq>(2);
}
#[test]
fn second_chunk_unwind_clears_partial_result_and_releases_window() {
    read_failure::<Fp>(3);
    read_failure::<Fq>(3);
}

#[test]
fn consumer_error_and_unwind_clear_complete_result_after_window_is_closed() {
    let layouts = [layout::<Fq>(1, 0, 0)];
    let state = Rc::new(State::default());
    let mut source = snapshot(layouts[0], &[Fq::from(2), Fq::from(3)], &state);
    let plan = prepare_stored_expression_v1(&advice::<Fq>(0, -1, 0), context(&layouts), TILE * 32)
        .unwrap();
    for panic in [false, true] {
        CLEAR_OBSERVATION.with(|record| record.set((0, true)));
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_stored_expression_chunk_v1(
                &plan,
                StoredRowTileV1 { start: 0, len: 2 },
                &mut [StoredAdviceInputV1 {
                    expected: layouts[0],
                    snapshot: &mut source,
                }],
                &[],
                &[],
                &[],
                |values| -> Result<(), _> {
                    assert!(!state.active.get());
                    assert_eq!(values, &[Fq::from(3), Fq::from(2)]);
                    assert!(!panic, "injected consumer unwind");
                    Err(StoredExpressionErrorV1::Consumer)
                },
            )
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), Err(StoredExpressionErrorV1::Consumer));
        }
        assert!(!state.active.get());
        assert!(!source.poisoned); // Consumer is outside the backend callback; caller aborts proof.
        CLEAR_OBSERVATION.with(|record| assert_eq!(record.get(), (TILE, true)));
    }
}
