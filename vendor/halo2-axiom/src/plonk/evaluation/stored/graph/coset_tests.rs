//! Independent ordinary graph parity, role/rotation provenance and reusable scratch cleanup.
//!
//! This source adapter models closed tile calls, not encryption or complete-proof authority.

use super::*;
use crate::{
    arithmetic::CurveAffine,
    plonk::evaluation::{Calculation, GraphEvaluator, ValueSource},
    poly::{
        EvaluationDomain, LagrangeCoeff, Polynomial,
        stored_advice::{
            StoredPolynomialBasisV1, StoredPolynomialLayoutV1, StoredPolynomialRoleV1,
            assignment::StoredAssignmentFieldV1,
        },
    },
};
use ff::{Field, WithSmallOrderMulGroup};
use halo2curves::pasta::{EpAffine, EqAffine};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Advice,
    Fixed,
    Instance,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Query {
    kind: Kind,
    column: usize,
    rotation: i32,
    start: usize,
    len: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceFault {
    Error(Kind),
    Panic(Kind),
    ContextAfter(Kind),
    PartAfter(Kind),
    ShapeAfter(Kind),
    ValidateError(usize),
    ValidatePanic(usize),
}
#[derive(Default)]
struct Calls {
    queries: RefCell<Vec<Query>>,
    validations: Cell<usize>,
    busy: Cell<bool>,
    fault: Cell<Option<SourceFault>>,
}
struct Window(Rc<Calls>);
impl Drop for Window {
    fn drop(&mut self) {
        self.0.busy.set(false);
    }
}
struct Source<F> {
    original: StoredPolynomialLayoutV1,
    extension_log: u32,
    part: u32,
    advice_phases: Vec<u8>,
    challenge_phases: Vec<u8>,
    advice: Vec<Polynomial<F, LagrangeCoeff>>,
    fixed: Vec<Polynomial<F, LagrangeCoeff>>,
    instance: Vec<Polynomial<F, LagrangeCoeff>>,
    calls: Rc<Calls>,
}
impl<F: StoredAssignmentFieldV1> Source<F> {
    fn copy(
        &mut self,
        kind: Kind,
        column: usize,
        rotation: i32,
        tile: StoredRowTileV1,
        destination: &mut [F],
    ) -> Result<(), StoredExpressionErrorV1> {
        assert!(!self.calls.busy.replace(true), "queries never overlap");
        let _window = Window(Rc::clone(&self.calls));
        assert_eq!(destination.len(), tile.len);
        assert!(destination.iter().all(|v| *v == F::ZERO));
        self.calls.queries.borrow_mut().push(Query {
            kind,
            column,
            rotation,
            start: tile.start,
            len: tile.len,
        });
        let columns = match kind {
            Kind::Advice => &self.advice,
            Kind::Fixed => &self.fixed,
            Kind::Instance => &self.instance,
        };
        let values = columns
            .get(column)
            .ok_or(StoredExpressionErrorV1::Context)?;
        let n = self.original.scalar_count();
        for (offset, value) in destination.iter_mut().enumerate() {
            let row = (tile.start as i64 + offset as i64 + i64::from(rotation)).rem_euclid(n as i64)
                as usize;
            *value = values[row];
        }
        match self.calls.fault.get() {
            Some(SourceFault::Error(target)) if target == kind => {
                return Err(StoredExpressionErrorV1::Consumer);
            }
            Some(SourceFault::Panic(target)) if target == kind => {
                panic!("injected source failure after filling private tile");
            }
            Some(SourceFault::ContextAfter(target)) if target == kind => {
                self.original = StoredPolynomialLayoutV1::new(
                    [92; 32],
                    self.original.ordinal(),
                    self.original.field(),
                    self.original.basis(),
                    self.original.k(),
                    self.original.role(),
                )
                .unwrap();
            }
            Some(SourceFault::PartAfter(target)) if target == kind => {
                self.part ^= 1;
            }
            Some(SourceFault::ShapeAfter(target)) if target == kind => {
                self.advice_phases[2] = 1;
            }
            _ => (),
        }
        Ok(())
    }
}
impl<F: StoredAssignmentFieldV1> StoredCosetGraphSourceV1<F> for Source<F> {
    fn validate_context(
        &mut self,
        expected: &StoredCosetGraphContextV1<'_>,
        part: u32,
    ) -> Result<(), StoredExpressionErrorV1> {
        let call = self.calls.validations.get() + 1;
        self.calls.validations.set(call);
        assert!(
            !self.calls.busy.get(),
            "source validation is outside windows"
        );
        if self.calls.fault.get() == Some(SourceFault::ValidatePanic(call)) {
            panic!("injected source validation unwind");
        }
        if self.calls.fault.get() == Some(SourceFault::ValidateError(call))
            || self.original != expected.original
            || self.extension_log != expected.extension_log
            || self.part != part
            || self.advice_phases != expected.advice_phases
            || self.challenge_phases != expected.challenge_phases
            || self.advice.len() != expected.advice_phases.len()
            || self.fixed.len() != expected.fixed_columns
            || self.instance.len() != expected.instance_columns
            || self
                .advice
                .iter()
                .chain(&self.fixed)
                .chain(&self.instance)
                .any(|column| column.len() != self.original.scalar_count())
        {
            return Err(StoredExpressionErrorV1::Context);
        }
        Ok(())
    }
    fn copy_advice_query_into(
        &mut self,
        column: usize,
        rotation: i32,
        tile: StoredRowTileV1,
        destination: &mut [F],
    ) -> Result<(), StoredExpressionErrorV1> {
        self.copy(Kind::Advice, column, rotation, tile, destination)
    }
    fn copy_fixed_query_into(
        &mut self,
        column: usize,
        rotation: i32,
        tile: StoredRowTileV1,
        destination: &mut [F],
    ) -> Result<(), StoredExpressionErrorV1> {
        self.copy(Kind::Fixed, column, rotation, tile, destination)
    }
    fn copy_instance_query_into(
        &mut self,
        column: usize,
        rotation: i32,
        tile: StoredRowTileV1,
        destination: &mut [F],
    ) -> Result<(), StoredExpressionErrorV1> {
        self.copy(Kind::Instance, column, rotation, tile, destination)
    }
}
fn original<F: StoredAssignmentFieldV1>(k: u32) -> StoredPolynomialLayoutV1 {
    StoredPolynomialLayoutV1::new(
        [91; 32],
        100,
        F::STORED_FIELD,
        StoredPolynomialBasisV1::Coefficient,
        k,
        StoredPolynomialRoleV1::VanishingRandom,
    )
    .unwrap()
}
fn context(original: StoredPolynomialLayoutV1) -> StoredCosetGraphContextV1<'static> {
    StoredCosetGraphContextV1 {
        original,
        extension_log: 2,
        advice_phases: &[0, 1, 2],
        fixed_columns: 2,
        instance_columns: 2,
        challenge_phases: &[0, 1, 2],
    }
}
fn make_source<F>(k: u32, part: u32) -> Source<F>
where
    F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
{
    let domain = EvaluationDomain::<F>::new(4, k);
    let n = 1_usize << k;
    let factor = domain.get_extended_omega().pow_vartime([u64::from(part)]);
    let columns = |count: usize, kind: Kind| {
        (0..count)
            .map(|column| {
                let values = (0..n)
                    .map(|row| match kind {
                        Kind::Advice => F::from((row * 7 + column * 19 + 3) as u64),
                        Kind::Fixed => F::from((row * 3 + column * 17 + 5) as u64),
                        Kind::Instance if row < 3 => F::from((row * row + column * 11 + 1) as u64),
                        Kind::Instance => F::ZERO,
                    })
                    .collect();
                domain.coeff_to_extended_part(
                    domain.lagrange_to_coeff(domain.lagrange_from_vec(values)),
                    factor,
                )
            })
            .collect()
    };
    Source {
        original: original::<F>(k),
        extension_log: 2,
        part,
        advice_phases: vec![0, 1, 2],
        challenge_phases: vec![0, 1, 2],
        advice: columns(3, Kind::Advice),
        fixed: columns(2, Kind::Fixed),
        instance: columns(2, Kind::Instance),
        calls: Rc::new(Calls::default()),
    }
}
fn all_operations<C: CurveAffine>() -> GraphEvaluator<C> {
    let mut graph = GraphEvaluator::<C>::default();
    graph.rotations = vec![0, 1, -1, 257];
    let a = graph.add_calculation(Calculation::Store(ValueSource::Advice(0, 0)));
    let b = graph.add_calculation(Calculation::Add(a, ValueSource::Advice(1, 1)));
    let c = graph.add_calculation(Calculation::Sub(b, ValueSource::Fixed(0, 2)));
    let d = graph.add_calculation(Calculation::Mul(c, ValueSource::Instance(0, 3)));
    let e = graph.add_calculation(Calculation::Square(d));
    let f = graph.add_calculation(Calculation::Double(e));
    let g = graph.add_calculation(Calculation::Negate(f));
    let h = graph.add_calculation(Calculation::Add(
        ValueSource::Challenge(0),
        ValueSource::Beta(),
    ));
    let i = graph.add_calculation(Calculation::Mul(
        ValueSource::Challenge(1),
        ValueSource::Gamma(),
    ));
    let j = graph.add_calculation(Calculation::Sub(
        ValueSource::Challenge(2),
        ValueSource::Theta(),
    ));
    let empty = graph.add_calculation(Calculation::Horner(
        ValueSource::PreviousValue(),
        vec![],
        ValueSource::Beta(),
    ));
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
            j,
            empty,
            ValueSource::Advice(0, 0),
            ValueSource::Fixed(0, 2),
            ValueSource::Instance(0, 3),
            ValueSource::Constant(1),
        ],
        ValueSource::Y(),
    ));
    graph.finish_building();
    graph
}
fn reset_clears() {
    crate::plonk::evaluation::stored::CLEAR_OBSERVATION.with(|v| v.set((0, true)));
}
fn clears() -> (usize, bool) {
    crate::plonk::evaluation::stored::CLEAR_OBSERVATION.with(Cell::get)
}
fn parity<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
{
    for k in [4, 8, 9] {
        let graph = all_operations::<C>();
        let graph_before = format!("{graph:?}");
        let plan =
            prepare_stored_coset_graph_v1(&graph, context(original::<C::Scalar>(k)), 1 << 20)
                .unwrap();
        assert_eq!(plan.maximum_private_query_requests(), 3);
        assert_eq!(plan.field_count(), 4 * 256 + graph.num_intermediates + 256);
        assert_eq!(plan.scratch_bytes(), plan.field_count() * 32);
        assert!(
            plan.planning_temporary_bytes()
                >= graph.num_intermediates * std::mem::size_of::<bool>()
        );
        let mut workspace =
            StoredCosetGraphWorkspaceV1::<C::Scalar>::new(plan.field_count(), 1 << 20).unwrap();
        let capacity = workspace.scratch_bytes();
        assert!(capacity >= plan.scratch_bytes());
        for part in 0..4 {
            let mut source = make_source::<C::Scalar>(k, part);
            assert!(
                source.instance[0]
                    .iter()
                    .skip(3)
                    .any(|v| *v != C::Scalar::ZERO)
            );
            let n = 1_usize << k;
            for start in (0..n).step_by(256) {
                let tile = StoredRowTileV1 {
                    start,
                    len: 256.min(n - start),
                };
                let previous = (0..tile.len)
                    .map(|row| C::Scalar::from((start + row + 29) as u64))
                    .collect::<Vec<_>>();
                let [beta, gamma, theta, y] = [2, 3, 5, 7].map(C::Scalar::from);
                let challenges = [11, 13, 17].map(C::Scalar::from);
                let mut ordinary = graph.instance();
                let expected = previous
                    .iter()
                    .enumerate()
                    .map(|(offset, previous)| {
                        graph.evaluate(
                            &mut ordinary,
                            &source.fixed,
                            &source.advice,
                            &source.instance,
                            &challenges,
                            &beta,
                            &gamma,
                            &theta,
                            &y,
                            previous,
                            start + offset,
                            1,
                            n as i32,
                        )
                    })
                    .collect::<Vec<_>>();
                let calls = Rc::clone(&source.calls);
                calls.queries.borrow_mut().clear();
                reset_clears();
                with_stored_coset_graph_chunk_v1(
                    &plan,
                    part,
                    tile,
                    &mut workspace,
                    &mut source,
                    &challenges,
                    beta,
                    gamma,
                    theta,
                    y,
                    &previous,
                    |actual| {
                        assert_eq!(actual, expected);
                        assert!(!calls.busy.get());
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(workspace.scratch_bytes(), capacity);
                assert_eq!(
                    calls.queries.borrow().as_slice(),
                    &[
                        Query {
                            kind: Kind::Advice,
                            column: 0,
                            rotation: 0,
                            start,
                            len: tile.len
                        },
                        Query {
                            kind: Kind::Advice,
                            column: 1,
                            rotation: 1,
                            start,
                            len: tile.len
                        },
                        Query {
                            kind: Kind::Fixed,
                            column: 0,
                            rotation: -1,
                            start,
                            len: tile.len
                        },
                        Query {
                            kind: Kind::Instance,
                            column: 0,
                            rotation: 257,
                            start,
                            len: tile.len
                        },
                    ],
                );
                let (count, zero) = clears();
                assert!(zero && count >= plan.field_count());
                assert_eq!(format!("{graph:?}"), graph_before);
            }
        }
    }
}

#[test]
fn both_pasta_coset_graph_matches_actual_retained_graph_all_operations_queries_and_previous_values()
{
    parity::<EpAffine>();
    parity::<EqAffine>();
}

fn faults<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
{
    let graph = all_operations::<C>();
    let plan =
        prepare_stored_coset_graph_v1(&graph, context(original::<C::Scalar>(9)), 1 << 20).unwrap();
    let cases = [Kind::Advice, Kind::Fixed, Kind::Instance]
        .into_iter()
        .flat_map(|kind| {
            [
                SourceFault::Error(kind),
                SourceFault::Panic(kind),
                SourceFault::ContextAfter(kind),
                SourceFault::PartAfter(kind),
                SourceFault::ShapeAfter(kind),
            ]
        })
        .chain([
            SourceFault::ValidateError(1),
            SourceFault::ValidateError(2),
            SourceFault::ValidateError(3),
            SourceFault::ValidatePanic(1),
            SourceFault::ValidatePanic(2),
            SourceFault::ValidatePanic(3),
        ])
        .collect::<Vec<_>>();
    for fault in cases {
        let mut source = make_source::<C::Scalar>(9, 1);
        source.calls.fault.set(Some(fault));
        let calls = Rc::clone(&source.calls);
        let mut workspace =
            StoredCosetGraphWorkspaceV1::<C::Scalar>::new(plan.field_count(), 1 << 20).unwrap();
        let capacity = workspace.scratch_bytes();
        reset_clears();
        let consumed = Cell::new(false);
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_stored_coset_graph_chunk_v1(
                &plan,
                1,
                StoredRowTileV1 {
                    start: 256,
                    len: 256,
                },
                &mut workspace,
                &mut source,
                &[C::Scalar::ONE; 3],
                C::Scalar::ONE,
                C::Scalar::ONE,
                C::Scalar::ONE,
                C::Scalar::ONE,
                &[C::Scalar::ONE; 256],
                |_| {
                    consumed.set(true);
                    Ok(())
                },
            )
        }));
        if matches!(fault, SourceFault::Panic(_) | SourceFault::ValidatePanic(_)) {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(!consumed.get());
        assert!(!calls.busy.get());
        assert!(clears().1);
        if !calls.queries.borrow().is_empty() {
            assert!(clears().0 >= plan.field_count());
        }
        // Raw helper failures consume no source authority. Reuse only the independently owned
        // clean workspace with a fresh source, proving no row of stale intermediates survives.
        let mut clean = make_source::<C::Scalar>(9, 1);
        with_stored_coset_graph_chunk_v1(
            &plan,
            1,
            StoredRowTileV1 { start: 0, len: 256 },
            &mut workspace,
            &mut clean,
            &[C::Scalar::ONE; 3],
            C::Scalar::ZERO,
            C::Scalar::ZERO,
            C::Scalar::ZERO,
            C::Scalar::ZERO,
            &[C::Scalar::ZERO; 256],
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(workspace.scratch_bytes(), capacity);
    }
    for unwind in [false, true] {
        let mut source = make_source::<C::Scalar>(9, 1);
        let calls = Rc::clone(&source.calls);
        let mut workspace =
            StoredCosetGraphWorkspaceV1::<C::Scalar>::new(plan.field_count(), 1 << 20).unwrap();
        reset_clears();
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_stored_coset_graph_chunk_v1(
                &plan,
                1,
                StoredRowTileV1 { start: 0, len: 256 },
                &mut workspace,
                &mut source,
                &[C::Scalar::ONE; 3],
                C::Scalar::ONE,
                C::Scalar::ONE,
                C::Scalar::ONE,
                C::Scalar::ONE,
                &[C::Scalar::ONE; 256],
                |_| -> Result<(), StoredExpressionErrorV1> {
                    assert!(!calls.busy.get());
                    assert!(!unwind, "injected graph consumer unwind");
                    Err(StoredExpressionErrorV1::Consumer)
                },
            )
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), Err(StoredExpressionErrorV1::Consumer));
        }
        assert!(clears().1 && clears().0 >= plan.field_count());
    }
    for unwind in [false, true] {
        let mut source = make_source::<C::Scalar>(9, 1);
        let calls = Rc::clone(&source.calls);
        let mut workspace =
            StoredCosetGraphWorkspaceV1::<C::Scalar>::new(plan.field_count(), 1 << 20).unwrap();
        let consumed = Cell::new(false);
        reset_clears();
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_stored_coset_graph_chunk_v1(
                &plan,
                1,
                StoredRowTileV1 { start: 0, len: 256 },
                &mut workspace,
                &mut source,
                &[C::Scalar::ONE; 3],
                C::Scalar::ONE,
                C::Scalar::ONE,
                C::Scalar::ONE,
                C::Scalar::ONE,
                &[C::Scalar::ONE; 256],
                |_| {
                    consumed.set(true);
                    let next = calls.validations.get() + 1;
                    calls.fault.set(Some(if unwind {
                        SourceFault::ValidatePanic(next)
                    } else {
                        SourceFault::ValidateError(next)
                    }));
                    Ok(())
                },
            )
        }));
        assert!(
            consumed.get(),
            "failure must occur after a successful consumer"
        );
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), Err(StoredExpressionErrorV1::Context));
        }
        assert!(!calls.busy.get());
        assert!(workspace.fields.iter().all(|v| *v == C::Scalar::ZERO));
        assert!(clears().1 && clears().0 >= plan.field_count());
    }
}

#[test]
fn both_pasta_coset_graph_source_and_consumer_errors_unwinds_clear_reusable_workspace() {
    faults::<EpAffine>();
    faults::<EqAffine>();
}

fn query_graph<C: CurveAffine>(value: ValueSource, rotations: Vec<i32>) -> GraphEvaluator<C> {
    let mut graph = GraphEvaluator::<C>::default();
    graph.rotations = rotations;
    graph.add_calculation(Calculation::Store(value));
    graph.finish_building();
    graph
}
fn admission<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
{
    let graph = all_operations::<C>();
    let anchor = original::<C::Scalar>(9);
    let shape = context(anchor);
    let plan = prepare_stored_coset_graph_v1(&graph, shape, 1 << 20).unwrap();
    assert_eq!(plan.context().original, anchor);
    assert_eq!(plan.context().advice_phases, &[0, 1, 2]);
    assert!(plan.metadata_bytes() >= 4 * std::mem::size_of::<super::Query>());
    assert!(matches!(
        prepare_stored_coset_graph_v1(&graph, shape, plan.scratch_bytes() - 1),
        Err(StoredExpressionErrorV1::ScratchLimit)
    ));
    assert!(prepare_stored_coset_graph_v1(&graph, shape, plan.scratch_bytes()).is_ok());
    assert!(matches!(
        StoredCosetGraphWorkspaceV1::<C::Scalar>::new(plan.field_count(), plan.scratch_bytes() - 1),
        Err(StoredExpressionErrorV1::ScratchLimit)
    ));
    assert!(StoredCosetGraphWorkspaceV1::<C::Scalar>::new(usize::MAX, usize::MAX).is_err());
    let mut overflow = graph.clone();
    overflow.num_intermediates = usize::MAX;
    assert!(prepare_stored_coset_graph_v1(&overflow, shape, usize::MAX).is_err());
    assert!(matches!(
        prepare_stored_coset_graph_v1(&GraphEvaluator::<C>::default(), shape, 1 << 20),
        Err(StoredExpressionErrorV1::Plan)
    ));
    for invalid in [
        ValueSource::Intermediate(0),
        ValueSource::Constant(99),
        ValueSource::Advice(3, 0),
        ValueSource::Advice(0, 1),
        ValueSource::Fixed(2, 0),
        ValueSource::Fixed(0, 1),
        ValueSource::Instance(2, 0),
        ValueSource::Instance(0, 1),
        ValueSource::Challenge(3),
    ] {
        let mut invalid_graph = query_graph::<C>(ValueSource::Advice(0, 0), vec![0]);
        invalid_graph.calculations[0].calculation = Calculation::Store(invalid);
        assert!(prepare_stored_coset_graph_v1(&invalid_graph, shape, 1 << 20).is_err());
    }
    let mut target = graph.clone();
    target.calculations[0].target = target.num_intermediates;
    assert!(prepare_stored_coset_graph_v1(&target, shape, 1 << 20).is_err());
    for case in 0..8 {
        let mut invalid = shape;
        match case {
            0 => invalid.extension_log = 0,
            1 => invalid.extension_log = 11,
            2 => invalid.advice_phases = &[0, 3, 2],
            3 => invalid.challenge_phases = &[0, 3, 2],
            4..=7 => {
                let mut basis = anchor.basis();
                let mut role = anchor.role();
                let mut field = anchor.field();
                let mut k = anchor.k();
                match case {
                    4 => basis = StoredPolynomialBasisV1::Lagrange,
                    5 => role = StoredPolynomialRoleV1::Instance { column: 0 },
                    6 => {
                        field = match field {
                            crate::poly::stored_advice::StoredPastaFieldV1::Fp => {
                                crate::poly::stored_advice::StoredPastaFieldV1::Fq
                            }
                            crate::poly::stored_advice::StoredPastaFieldV1::Fq => {
                                crate::poly::stored_advice::StoredPastaFieldV1::Fp
                            }
                        }
                    }
                    7 => k = 19,
                    _ => unreachable!(),
                }
                invalid.original =
                    StoredPolynomialLayoutV1::new([91; 32], 100, field, basis, k, role).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(prepare_stored_coset_graph_v1(&graph, invalid, 1 << 20).is_err());
    }
    for case in 0..8 {
        let mut source = make_source::<C::Scalar>(9, 1);
        let calls = Rc::clone(&source.calls);
        let mut workspace =
            StoredCosetGraphWorkspaceV1::<C::Scalar>::new(plan.field_count() + 7, 1 << 20).unwrap();
        let pointer = workspace.fields.as_ptr();
        let capacity = workspace.fields.capacity();
        // Structural preflight must clear even a larger scratch allocation left with caller
        // test sentinels. This does not confer source or owner reuse after a failed operation.
        workspace.fields.fill(C::Scalar::from(43));
        let mut part = 1;
        let mut tile = StoredRowTileV1 {
            start: 256,
            len: 256,
        };
        let mut previous = vec![C::Scalar::ONE; 256];
        let mut challenges = vec![C::Scalar::ONE; 3];
        match case {
            0 => part = 4,
            1 => tile.start = 512,
            2 => tile.start = 1,
            3 => tile.len = 0,
            4 => tile.len = 255,
            5 => previous.pop().map(|_| ()).unwrap(),
            6 => challenges.pop().map(|_| ()).unwrap(),
            7 => workspace.fields.truncate(plan.field_count() - 1),
            _ => unreachable!(),
        }
        reset_clears();
        let consumed = Cell::new(false);
        let result = with_stored_coset_graph_chunk_v1(
            &plan,
            part,
            tile,
            &mut workspace,
            &mut source,
            &challenges,
            C::Scalar::ONE,
            C::Scalar::ONE,
            C::Scalar::ONE,
            C::Scalar::ONE,
            &previous,
            |_| {
                consumed.set(true);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(!consumed.get());
        assert_eq!(calls.validations.get(), 0);
        assert!(calls.queries.borrow().is_empty());
        assert!(workspace.fields.iter().all(|v| *v == C::Scalar::ZERO));
        assert_eq!(workspace.fields.as_ptr(), pointer);
        assert_eq!(workspace.fields.capacity(), capacity);
        assert!(clears().1);
    }
}

#[test]
fn both_pasta_coset_graph_admission_capacity_overflow_and_preflight_refusals_do_no_source_reads() {
    admission::<EpAffine>();
    admission::<EqAffine>();
}

fn rotation_and_empty<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
{
    for rotation in [i32::MIN, i32::MAX, -513, -512, -257, 257, 512, 513] {
        for kind in [Kind::Advice, Kind::Fixed, Kind::Instance] {
            let value = match kind {
                Kind::Advice => ValueSource::Advice(0, 0),
                Kind::Fixed => ValueSource::Fixed(0, 0),
                Kind::Instance => ValueSource::Instance(0, 0),
            };
            let graph = query_graph::<C>(value, vec![rotation]);
            let plan =
                prepare_stored_coset_graph_v1(&graph, context(original::<C::Scalar>(9)), 1 << 20)
                    .unwrap();
            assert_eq!(
                plan.maximum_private_query_requests(),
                usize::from(kind != Kind::Fixed)
            );
            let mut source = make_source::<C::Scalar>(9, 3);
            let columns = match kind {
                Kind::Advice => &source.advice,
                Kind::Fixed => &source.fixed,
                Kind::Instance => &source.instance,
            };
            let expected = (0..256)
                .map(|offset| {
                    let index = (256_i64 + offset + i64::from(rotation)).rem_euclid(512) as usize;
                    columns[0][index]
                })
                .collect::<Vec<_>>();
            let mut workspace =
                StoredCosetGraphWorkspaceV1::<C::Scalar>::new(plan.field_count(), 1 << 20).unwrap();
            with_stored_coset_graph_chunk_v1(
                &plan,
                3,
                StoredRowTileV1 {
                    start: 256,
                    len: 256,
                },
                &mut workspace,
                &mut source,
                &[C::Scalar::ONE; 3],
                C::Scalar::ONE,
                C::Scalar::ONE,
                C::Scalar::ONE,
                C::Scalar::ONE,
                &[C::Scalar::from(99); 256],
                |actual| {
                    assert_eq!(actual, expected);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(source.calls.queries.borrow().len(), 1);
        }
    }
    let mut duplicate = GraphEvaluator::<C>::default();
    duplicate.rotations = vec![1, 1];
    duplicate.add_calculation(Calculation::Horner(
        ValueSource::Constant(0),
        vec![
            ValueSource::Advice(0, 0),
            ValueSource::Advice(0, 1),
            ValueSource::Advice(0, 0),
            ValueSource::Fixed(0, 0),
            ValueSource::Instance(0, 0),
        ],
        ValueSource::Theta(),
    ));
    duplicate.finish_building();
    let plan =
        prepare_stored_coset_graph_v1(&duplicate, context(original::<C::Scalar>(4)), 1 << 20)
            .unwrap();
    assert_eq!(plan.maximum_private_query_requests(), 3);
    assert_eq!(plan.queries.len(), 4);
    let mut source = make_source::<C::Scalar>(4, 0);
    let mut workspace =
        StoredCosetGraphWorkspaceV1::<C::Scalar>::new(plan.field_count() + 256, 1 << 20).unwrap();
    let pointer = workspace.fields.as_ptr();
    with_stored_coset_graph_chunk_v1(
        &plan,
        0,
        StoredRowTileV1 { start: 0, len: 16 },
        &mut workspace,
        &mut source,
        &[C::Scalar::ONE; 3],
        C::Scalar::ONE,
        C::Scalar::ONE,
        C::Scalar::from(7),
        C::Scalar::ONE,
        &[C::Scalar::ONE; 16],
        |_| Ok(()),
    )
    .unwrap();
    let requests = source.calls.queries.borrow();
    assert_eq!(requests.len(), 4);
    assert_eq!(
        requests[0], requests[1],
        "rotation indices, not values, distinguish cache requests"
    );
    drop(requests);
    for empty in [false, true] {
        let mut graph = GraphEvaluator::<C>::default();
        if !empty {
            graph.add_calculation(Calculation::Horner(
                ValueSource::PreviousValue(),
                vec![],
                ValueSource::Y(),
            ));
        }
        graph.finish_building();
        let mut shape = context(original::<C::Scalar>(4));
        shape.advice_phases = &[];
        shape.fixed_columns = 0;
        shape.instance_columns = 0;
        shape.challenge_phases = &[];
        let plan = prepare_stored_coset_graph_v1(&graph, shape, 1 << 20).unwrap();
        assert_eq!(plan.maximum_private_query_requests(), 0);
        assert!(plan.queries.is_empty());
        if empty {
            assert_eq!(plan.planning_temporary_bytes(), 0);
        } else {
            assert!(
                plan.planning_temporary_bytes()
                    >= graph.num_intermediates * std::mem::size_of::<bool>()
            );
        }
        let mut source = make_source::<C::Scalar>(4, 0);
        source.advice.clear();
        source.fixed.clear();
        source.instance.clear();
        source.advice_phases.clear();
        source.challenge_phases.clear();
        reset_clears();
        with_stored_coset_graph_chunk_v1(
            &plan,
            0,
            StoredRowTileV1 { start: 0, len: 16 },
            &mut workspace,
            &mut source,
            &[],
            C::Scalar::ONE,
            C::Scalar::ONE,
            C::Scalar::ONE,
            C::Scalar::ONE,
            &[C::Scalar::from(13); 16],
            |actual| {
                assert!(actual.iter().all(|v| *v
                    == if empty {
                        C::Scalar::ZERO
                    } else {
                        C::Scalar::from(13)
                    }));
                Ok(())
            },
        )
        .unwrap();
        assert!(source.calls.queries.borrow().is_empty());
        assert_eq!(workspace.fields.as_ptr(), pointer);
        assert!(workspace.fields.iter().all(|v| *v == C::Scalar::ZERO));
        assert!(clears().1 && clears().0 >= workspace.fields.len());
    }
}

#[test]
fn both_pasta_coset_graph_preserves_extreme_rotations_distinct_query_indices_and_empty_banks() {
    rotation_and_empty::<EpAffine>();
    rotation_and_empty::<EqAffine>();
}
