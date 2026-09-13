//! Actual phase/coefficient ownership tests for the concrete lookup bridge.
//!
//! The intentionally plaintext backend measures identity checks and whole-owner destruction.
//! It is neither an authenticated-spool implementation nor evidence of full-proof resource use.

use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use ff::{Field, FromUniformBytes, PrimeField};
use halo2curves::pasta::{EpAffine, EqAffine};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

use super::*;
use crate::{
    plonk::{
        ConstraintSystem, Expression, FirstPhase,
        stored::{StoredExpressionContextV1, prepare_stored_expression_v1},
    },
    poly::{
        Rotation,
        commitment::ParamsProver,
        stored_advice::{
            STORED_SCALARS_PER_CHUNK_V1, StoredLookupSideV1, StoredPastaFieldV1,
            phase::{StoredPhaseAssignmentsV1, admit_stored_phase_plan_v1},
        },
    },
    transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
};

#[derive(Default)]
struct Backend {
    live_snapshots: Cell<usize>,
    live_writers: Cell<usize>,
    reads: Cell<usize>,
    fail_read: Cell<bool>,
    panic_read: Cell<bool>,
    identities: RefCell<Vec<Rc<Cell<StoredPolynomialLayoutV1>>>>,
}

struct Snapshot {
    layout: Rc<Cell<StoredPolynomialLayoutV1>>,
    backend: Rc<Backend>,
    values: Vec<[u8; 32]>,
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        self.backend
            .live_snapshots
            .set(self.backend.live_snapshots.get() - 1);
    }
}

impl StoredPolynomialSnapshotV1 for Snapshot {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        self.layout.get()
    }

    fn with_chunk<R>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredPolynomialErrorV1>,
    ) -> Result<R, StoredPolynomialErrorV1> {
        if expected != self.layout.get() {
            return Err(StoredPolynomialErrorV1::Context);
        }
        let count = expected.chunk_scalar_count(chunk)?;
        self.backend.reads.set(self.backend.reads.get() + 1);
        assert!(
            !self.backend.panic_read.get(),
            "injected lookup read unwind"
        );
        if self.backend.fail_read.get() {
            return Err(StoredPolynomialErrorV1::Authentication);
        }
        let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
        consume(&self.values[start..start + count])
    }

    fn with_column<R>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredPolynomialErrorV1>,
    ) -> Result<R, StoredPolynomialErrorV1> {
        if expected != self.layout.get() {
            return Err(StoredPolynomialErrorV1::Context);
        }
        consume(&self.values)
    }
}

struct Writer {
    layout: StoredPolynomialLayoutV1,
    backend: Rc<Backend>,
    values: Vec<[u8; 32]>,
    next_chunk: u64,
    observations: Cell<usize>,
    drift: bool,
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.backend
            .live_writers
            .set(self.backend.live_writers.get() - 1);
    }
}

impl StoredPolynomialWriterV1 for Writer {
    type Snapshot = Snapshot;

    fn layout(&self) -> StoredPolynomialLayoutV1 {
        self.observations.set(self.observations.get() + 1);
        let mut layout = self.layout;
        if self.drift && self.observations.get() >= 2 {
            layout.ordinal += 1;
        }
        layout
    }

    fn write_chunk(
        &mut self,
        chunk: u64,
        values: &[[u8; 32]],
    ) -> Result<(), StoredPolynomialErrorV1> {
        assert_eq!(chunk, self.next_chunk);
        assert_eq!(values.len(), self.layout.chunk_scalar_count(chunk)?);
        assert!(
            values
                .iter()
                .all(|value| self.layout.field().is_canonical(value))
        );
        self.values.extend_from_slice(values);
        self.next_chunk += 1;
        Ok(())
    }

    fn seal(mut self) -> Result<Self::Snapshot, StoredPolynomialErrorV1> {
        assert_eq!(self.next_chunk as usize, self.layout.chunk_count());
        let layout = Rc::new(Cell::new(self.layout));
        self.backend
            .identities
            .borrow_mut()
            .push(Rc::clone(&layout));
        self.backend
            .live_snapshots
            .set(self.backend.live_snapshots.get() + 1);
        Ok(Snapshot {
            layout,
            backend: Rc::clone(&self.backend),
            values: std::mem::take(&mut self.values),
        })
    }
}

struct Provider {
    backend: Rc<Backend>,
    context: [u8; 32],
    next: u64,
    fault: u8,
    created: usize,
}

impl StoredPolynomialProviderV1 for Provider {
    type Writer = Writer;

    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        self.created += 1;
        if self.fault == 8 {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        assert_ne!(self.fault, 9, "injected lookup create unwind");
        let mut layout =
            StoredPolynomialLayoutV1::new(self.context, self.next, field, basis, k, role)?;
        self.next += 1;
        match self.fault {
            1 => layout.field = other_field(field),
            2 => layout.k += 1,
            3 => layout.basis = StoredPolynomialBasisV1::Coefficient,
            4 => {
                layout.role = StoredPolynomialRoleV1::Advice {
                    column: 0,
                    phase: 0,
                }
            }
            5 => layout.proof_context = [8; 32],
            6 => layout.ordinal = 0,
            10 | 11 => {
                let index = if self.fault == 10 { 0 } else { 3 };
                let identity = Rc::clone(&self.backend.identities.borrow()[index]);
                let mut changed = identity.get();
                changed.proof_context = [8; 32];
                identity.set(changed);
            }
            12 => layout.proof_context = [0; 32],
            13 => {
                layout.role = StoredPolynomialRoleV1::LookupCompressed {
                    lookup: 2,
                    side: StoredLookupSideV1::Input,
                }
            }
            14 => {
                layout.role = StoredPolynomialRoleV1::LookupCompressed {
                    lookup: 3,
                    side: StoredLookupSideV1::Table,
                }
            }
            _ => {}
        }
        self.backend
            .live_writers
            .set(self.backend.live_writers.get() + 1);
        Ok(Writer {
            layout,
            backend: Rc::clone(&self.backend),
            values: Vec::new(),
            next_chunk: 0,
            observations: Cell::new(0),
            drift: self.fault == 7,
        })
    }
}

fn other_field(field: StoredPastaFieldV1) -> StoredPastaFieldV1 {
    match field {
        StoredPastaFieldV1::Fp => StoredPastaFieldV1::Fq,
        StoredPastaFieldV1::Fq => StoredPastaFieldV1::Fp,
    }
}

fn role(lookup: u32, side: StoredLookupSideV1) -> StoredPolynomialRoleV1 {
    StoredPolynomialRoleV1::LookupCompressed { lookup, side }
}

fn fixture<'params, C>(
    params: &'params ParamsIPA<C>,
    empty: bool,
) -> (
    CoefficientStoredAdviceV1<'params, C, Snapshot>,
    Provider,
    Expression<C::Scalar>,
)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let domain = EvaluationDomain::<C::Scalar>::new(3, params.k());
    let mut meta = ConstraintSystem::default();
    let mut expression = Expression::Constant(C::Scalar::from(17));
    if !empty {
        let column = meta.advice_column();
        meta.advice_column();
        let challenge = meta.challenge_usable_after(FirstPhase);
        meta.create_gate("lookup bridge source", |cells| {
            expression = cells.query_advice(column, Rotation::cur()) + challenge.expr();
            vec![expression.clone()]
        });
    }
    let plan = admit_stored_phase_plan_v1(params, &domain, &meta).unwrap();
    let columns = plan.columns;
    let usable = plan.usable_rows;
    let backend = Rc::new(Backend::default());
    let mut provider = Provider {
        backend,
        context: [9; 32],
        next: 11,
        fault: 0,
        created: 0,
    };
    let writers = (0..columns)
        .map(|column| {
            provider
                .create(
                    C::Scalar::STORED_FIELD,
                    StoredPolynomialBasisV1::Lagrange,
                    params.k(),
                    StoredPolynomialRoleV1::Advice {
                        column: column as u32,
                        phase: 0,
                    },
                )
                .unwrap()
        })
        .collect();
    let mut phase = StoredPhaseAssignmentsV1::<C, Writer>::begin(plan, writers).unwrap();
    for column in 0..columns {
        for row in 0..usable {
            phase
                .assign_discarding_value(
                    column,
                    row,
                    C::Scalar::from((row + column + 1) as u64).into(),
                )
                .unwrap();
        }
    }
    let mut rng = ChaCha20Rng::from_seed([37; 32]);
    let mut transcript = Blake2bWrite::<Vec<u8>, C, Challenge255<C>>::init(Vec::new());
    let complete = phase
        .finish(&mut rng)
        .unwrap()
        .absorb(&mut transcript)
        .unwrap()
        .into_complete()
        .unwrap();
    let owner = complete.stage_coefficients(&domain, &mut provider).unwrap();
    assert_eq!(provider.backend.live_snapshots.get(), 2 * columns);
    assert_eq!(provider.backend.live_writers.get(), 0);
    (owner, provider, expression)
}

struct Auxiliary {
    fail: bool,
}

impl<F: StoredAssignmentFieldV1> StoredAuxiliarySourceV1<F> for Auxiliary {
    fn validate(
        &mut self,
        expected: StoredExpressionContextV1<'_>,
    ) -> Result<(), StoredExpressionErrorV1> {
        if self.fail || expected.fixed_columns != 0 || expected.instance_columns != 0 {
            return Err(StoredExpressionErrorV1::Context);
        }
        Ok(())
    }
    fn fixed_value(&mut self, _: usize, _: usize) -> Result<F, StoredExpressionErrorV1> {
        Err(StoredExpressionErrorV1::Context)
    }
    fn instance_value(&mut self, _: usize, _: usize) -> Result<F, StoredExpressionErrorV1> {
        Err(StoredExpressionErrorV1::Context)
    }
}

fn context<'a>(
    layouts: &'a [StoredPolynomialLayoutV1],
    phases: &'a [u8],
) -> StoredExpressionContextV1<'a> {
    StoredExpressionContextV1 {
        domain: layouts[0],
        advice: layouts,
        fixed_columns: 0,
        instance_columns: 0,
        challenge_phases: phases,
    }
}

fn assert_dropped(backend: &Backend) {
    assert_eq!(backend.live_snapshots.get(), 0);
    assert_eq!(backend.live_writers.get(), 0);
}

fn successful_bridge<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    for k in [4, 9] {
        let params = ParamsIPA::<C>::new(k);
        let domain = EvaluationDomain::new(3, k);
        let (owner, mut provider, expression) = fixture(&params, false);
        let backend = Rc::clone(&provider.backend);
        assert!(std::ptr::eq(owner.params().unwrap(), &params));
        assert_eq!(owner.proof_context().unwrap(), Some([9; 32]));
        owner.validate_for_lookup(&domain).unwrap();
        let layouts = owner.layouts().unwrap().collect::<Vec<_>>();
        let challenges = owner.challenges().unwrap().collect::<Vec<_>>();
        assert_eq!(layouts.len(), 2);
        assert_eq!(challenges.len(), 1);
        let expected = owner.lagrange.session.as_ref().unwrap().columns[0]
            .snapshot
            .values
            .clone();
        let plan =
            prepare_stored_expression_v1(&expression, context(&layouts, &[0]), usize::MAX).unwrap();
        let mut current = owner;
        for start in (0..1_usize << k).step_by(STORED_SCALARS_PER_CHUNK_V1) {
            let len = STORED_SCALARS_PER_CHUNK_V1.min((1 << k) - start);
            let (next, ()) = current
                .with_expression_sources(
                    &plan,
                    StoredRowTileV1 { start, len },
                    &mut Auxiliary { fail: false },
                    &challenges,
                    |values| {
                        assert_eq!(values.len(), len);
                        for (offset, actual) in values.iter().enumerate() {
                            let original = Option::<C::Scalar>::from(C::Scalar::from_repr(
                                expected[start + offset],
                            ))
                            .unwrap();
                            assert_eq!(*actual, original + challenges[0]);
                        }
                        Ok(())
                    },
                )
                .unwrap();
            current = next;
        }
        let old = current.greatest_ordinal.unwrap();
        let reads = backend.reads.get();
        let (current, writer, captured) = current
            .create_output_writer(&mut provider, role(0, StoredLookupSideV1::Input))
            .unwrap();
        assert!(captured.ordinal() > old);
        assert_eq!(captured, writer.layout());
        assert_eq!(captured.role(), role(0, StoredLookupSideV1::Input));
        assert_eq!(captured.basis(), StoredPolynomialBasisV1::Lagrange);
        assert_eq!(backend.reads.get(), reads);
        assert_eq!(current.greatest_ordinal, Some(captured.ordinal()));
        current.validate_live_receipts().unwrap();
        drop(writer);
        drop(current);
        assert_dropped(&backend);
    }
}

#[test]
fn eq_fp_bridge_evaluates_actual_receipts_and_preserves_metadata() {
    successful_bridge::<EqAffine>();
}

#[test]
fn ep_fq_bridge_evaluates_actual_receipts_and_preserves_metadata() {
    successful_bridge::<EpAffine>();
}

fn malformed_sources<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let domain = EvaluationDomain::new(3, 4);
    for fault in 0..23 {
        let (mut owner, provider, _) = fixture(&params, false);
        let backend = Rc::clone(&provider.backend);
        let reads = backend.reads.get();
        let session = owner.lagrange.session.as_mut().unwrap();
        match fault {
            0 => session.plan.k += 1,
            1 => session.plan.field = other_field(session.plan.field),
            2 => session.next_phase = 0,
            3 => session.challenges.clear(),
            4 => session.challenges[0] = None,
            5 => session.plan.challenges += 1,
            6 => session.plan.phases[0].challenges.push(0),
            7 => {
                owner.coefficients.pop();
            }
            8 => session.columns[0].layout.role = role(0, StoredLookupSideV1::Input),
            9 => owner.coefficients[0].layout.role = role(0, StoredLookupSideV1::Input),
            10 => owner.coefficients[0].layout.basis = StoredPolynomialBasisV1::Lagrange,
            11 => owner.coefficients[0].layout.proof_context = [8; 32],
            12 => owner.coefficients[0].layout.ordinal = session.greatest_ordinal.unwrap(),
            13 => owner.proof_context = Some([8; 32]),
            14 => owner.greatest_ordinal = None,
            15 | 16 => {
                let index = if fault == 15 { 3 } else { 0 };
                let cell = Rc::clone(&backend.identities.borrow()[index]);
                let mut changed = cell.get();
                changed.ordinal += 100;
                cell.set(changed);
            }
            17 => session.greatest_ordinal = Some(0),
            18 => session.proof_context = None,
            19 => session.plan.phases[0].columns.reverse(),
            20 => {
                session.plan.phases[0].columns.pop();
            }
            21 => owner.greatest_ordinal = Some(0),
            22 => session.plan.usable_rows = usize::MAX,
            _ => unreachable!(),
        }
        assert!(owner.validate_for_lookup(&domain).is_err(), "fault {fault}");
        assert_eq!(backend.reads.get(), reads);
        drop(owner);
        assert_dropped(&backend);
    }
    let (owner, provider, _) = fixture(&params, false);
    assert!(
        owner
            .validate_for_lookup(&EvaluationDomain::new(3, 5))
            .is_err()
    );
    drop(owner);
    assert_dropped(&provider.backend);
}

#[test]
fn malformed_phase_geometry_and_every_receipt_identity_fail_before_reads_in_both_fields() {
    malformed_sources::<EqAffine>();
    malformed_sources::<EpAffine>();
}

fn malformed_outputs<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for fault in 1..15 {
        let (owner, mut provider, _) = fixture(&params, false);
        let backend = Rc::clone(&provider.backend);
        let reads = backend.reads.get();
        provider.fault = fault;
        let result = catch_unwind(AssertUnwindSafe(|| {
            owner.create_output_writer(&mut provider, role(2, StoredLookupSideV1::Table))
        }));
        if fault == 9 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err(), "fault {fault}");
        }
        assert_eq!(backend.reads.get(), reads);
        assert_dropped(&backend);
    }
    let (owner, mut provider, _) = fixture(&params, false);
    let before = provider.created;
    assert!(
        owner
            .create_output_writer(
                &mut provider,
                StoredPolynomialRoleV1::Advice {
                    column: 0,
                    phase: 0
                }
            )
            .is_err()
    );
    assert_eq!(provider.created, before);
    assert_dropped(&provider.backend);
}

#[test]
fn malformed_output_roles_geometry_context_ordinals_and_second_observation_drop_both_bases() {
    malformed_outputs::<EqAffine>();
    malformed_outputs::<EpAffine>();
}

fn zero_advice<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let (owner, mut provider, _) = fixture(&params, true);
    assert_eq!(owner.proof_context().unwrap(), None);
    assert_eq!(owner.lagrange.proof_context().unwrap(), None);
    assert_eq!(owner.greatest_ordinal, None);
    assert_eq!(provider.created, 0);
    assert_eq!(owner.layouts().unwrap().len(), 0);
    assert_eq!(owner.challenges().unwrap().count(), 0);
    provider.context = [7; 32];
    let (owner, first, captured) = owner
        .create_output_writer(&mut provider, role(0, StoredLookupSideV1::Input))
        .unwrap();
    assert_eq!(captured.proof_context, [7; 32]);
    assert_eq!(owner.proof_context().unwrap(), Some([7; 32]));
    assert_eq!(owner.lagrange.proof_context().unwrap(), None);
    assert_eq!(
        owner.lagrange.session.as_ref().unwrap().greatest_ordinal,
        None
    );
    owner
        .validate_for_lookup(&EvaluationDomain::new(3, 4))
        .unwrap();
    drop(first);
    let (owner, second, later) = owner
        .create_output_writer(&mut provider, role(0, StoredLookupSideV1::Table))
        .unwrap();
    assert!(later.ordinal() > captured.ordinal());
    assert_eq!(later.proof_context, captured.proof_context);
    drop(second);
    provider.context = [6; 32];
    assert!(
        owner
            .create_output_writer(&mut provider, role(1, StoredLookupSideV1::Input))
            .is_err()
    );
    assert_eq!(provider.backend.reads.get(), 0);
    assert_dropped(&provider.backend);
}

#[test]
fn zero_advice_uses_only_first_real_writer_context_without_rewriting_original_session() {
    zero_advice::<EqAffine>();
    zero_advice::<EpAffine>();
}

fn ordinal_reuse<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let (owner, mut provider, _) = fixture(&params, false);
    let coefficient_max = owner.greatest_ordinal.unwrap();
    let (owner, input, first) = owner
        .create_output_writer(&mut provider, role(0, StoredLookupSideV1::Input))
        .unwrap();
    drop(input);
    let (owner, table, second) = owner
        .create_output_writer(&mut provider, role(0, StoredLookupSideV1::Table))
        .unwrap();
    drop(table);
    assert!(first.ordinal() > coefficient_max);
    assert!(second.ordinal() > first.ordinal());
    provider.next = first.ordinal();
    assert!(
        owner
            .create_output_writer(&mut provider, role(1, StoredLookupSideV1::Input))
            .is_err()
    );
    assert_dropped(&provider.backend);
}

#[test]
fn lookup_output_ordinals_share_the_global_coefficient_high_water_mark_in_both_fields() {
    ordinal_reuse::<EqAffine>();
    ordinal_reuse::<EpAffine>();
}

fn evaluation_failures<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for fault in 0..9 {
        let (mut owner, provider, expression) = fixture(&params, false);
        let backend = Rc::clone(&provider.backend);
        let layouts = owner.layouts().unwrap().collect::<Vec<_>>();
        let mut challenges = owner.challenges().unwrap().collect::<Vec<_>>();
        let plan =
            prepare_stored_expression_v1(&expression, context(&layouts, &[0]), usize::MAX).unwrap();
        backend.fail_read.set(fault == 0);
        backend.panic_read.set(fault == 1);
        if fault == 6 {
            challenges[0] += C::Scalar::ONE;
        }
        if fault == 7 {
            owner.lagrange.session.take();
            assert!(owner.params().is_err());
            assert!(owner.layouts().is_err());
            assert!(owner.challenges().is_err());
            assert!(owner.proof_context().is_err());
        }
        if fault == 8 {
            owner.coefficients[0].layout.proof_context = [8; 32];
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            owner.with_expression_sources(
                &plan,
                StoredRowTileV1 {
                    start: 0,
                    len: if fault == 5 { 15 } else { 16 },
                },
                &mut Auxiliary { fail: fault == 4 },
                &challenges,
                |_| -> Result<(), StoredExpressionErrorV1> {
                    if fault == 2 {
                        return Err(StoredExpressionErrorV1::Consumer);
                    }
                    assert_ne!(fault, 3, "injected lookup consumer unwind");
                    Ok(())
                },
            )
        }));
        if fault == 1 || fault == 3 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err(), "fault {fault}");
        }
        assert_dropped(&backend);
    }
}

#[test]
fn evaluator_preflight_backend_consumer_errors_and_unwinds_drop_originals_and_coefficients() {
    evaluation_failures::<EqAffine>();
    evaluation_failures::<EpAffine>();
}

struct ConsumerValue(Rc<Cell<bool>>);
impl Drop for ConsumerValue {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

fn post_consumer_drift<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for changed_index in [1, 3] {
        let (owner, provider, expression) = fixture(&params, false);
        let backend = Rc::clone(&provider.backend);
        let layouts = owner.layouts().unwrap().collect::<Vec<_>>();
        let challenges = owner.challenges().unwrap().collect::<Vec<_>>();
        let plan =
            prepare_stored_expression_v1(&expression, context(&layouts, &[0]), usize::MAX).unwrap();
        let dropped = Rc::new(Cell::new(false));
        let value = ConsumerValue(Rc::clone(&dropped));
        let identity = Rc::clone(&backend.identities.borrow()[changed_index]);
        let result = owner.with_expression_sources(
            &plan,
            StoredRowTileV1 { start: 0, len: 16 },
            &mut Auxiliary { fail: false },
            &challenges,
            |_| {
                let mut changed = identity.get();
                changed.proof_context = [8; 32];
                identity.set(changed);
                Ok(value)
            },
        );
        assert!(matches!(
            result,
            Err(StoredExpressionErrorV1::Store(
                StoredPolynomialErrorV1::Context
            ))
        ));
        assert!(
            dropped.get(),
            "a successful consumer value must not escape failed final admission"
        );
        assert_dropped(&backend);
    }
}

#[test]
fn successful_consumer_cannot_restore_changed_unused_advice_or_coefficient_receipts() {
    post_consumer_drift::<EqAffine>();
    post_consumer_drift::<EpAffine>();
}

fn product_bridge_roundtrip<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    for k in [4, 8, 9] {
        let params = ParamsIPA::<C>::new(k);
        let (mut owner, mut provider, _) = fixture(&params, false);
        let backend = Rc::clone(&provider.backend);
        let original = owner
            .lagrange
            .session
            .as_ref()
            .unwrap()
            .columns
            .iter()
            .map(|column| {
                (
                    column.layout,
                    column.snapshot.values.clone(),
                    column.blind.0.0,
                )
            })
            .collect::<Vec<_>>();
        let original_ptr = owner.lagrange.session.as_ref().unwrap().columns.as_ptr();
        let coefficient_ptr = owner.coefficients.as_ptr();
        let challenges = owner.challenges().unwrap().collect::<Vec<_>>();
        let old = owner.product_ordinal_boundary(2).unwrap().unwrap();
        let reads = backend.reads.get();
        let mut copied = vec![C::Scalar::ZERO; STORED_SCALARS_PER_CHUNK_V1];
        let mut expected_reads = 0;
        for (column, (layout, encoded, _)) in original.iter().enumerate().rev() {
            for chunk in (0..layout.chunk_count() as u64).rev() {
                let count = layout.chunk_scalar_count(chunk).unwrap();
                owner = owner
                    .copy_lagrange_chunk_into(column as u32, chunk, &mut copied[..count])
                    .unwrap();
                let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
                for (actual, encoded) in copied[..count].iter().zip(&encoded[start..start + count])
                {
                    assert_eq!(actual.to_repr(), *encoded);
                }
                expected_reads += 1;
            }
        }
        assert_eq!(backend.reads.get(), reads + expected_reads);
        let (owner, copy, first) = owner.create_copy_product_writer(&mut provider, 7).unwrap();
        assert_eq!(
            first.role(),
            StoredPolynomialRoleV1::CopyPermutationProduct { set: 7 }
        );
        assert_eq!(first.basis(), StoredPolynomialBasisV1::Coefficient);
        assert_eq!(first.proof_context, [9; 32]);
        assert!(first.ordinal() > old);
        drop(copy);
        let (owner, lookup, second) = owner
            .create_lookup_product_writer(&mut provider, 3)
            .unwrap();
        assert_eq!(
            second.role(),
            StoredPolynomialRoleV1::LookupProduct { lookup: 3 }
        );
        assert_eq!(second.basis(), StoredPolynomialBasisV1::Coefficient);
        assert!(second.ordinal() > first.ordinal());
        assert_eq!(
            owner.product_ordinal_boundary(0).unwrap(),
            Some(second.ordinal())
        );
        assert_eq!(
            owner.lagrange.session.as_ref().unwrap().columns.as_ptr(),
            original_ptr
        );
        assert_eq!(owner.coefficients.as_ptr(), coefficient_ptr);
        assert_eq!(owner.challenges().unwrap().collect::<Vec<_>>(), challenges);
        for (column, (layout, values, blind)) in owner
            .lagrange
            .session
            .as_ref()
            .unwrap()
            .columns
            .iter()
            .zip(&original)
        {
            assert_eq!(column.layout, *layout);
            assert_eq!(column.snapshot.values, *values);
            assert_eq!(column.blind.0.0, *blind);
        }
        assert!(std::ptr::eq(owner.params().unwrap(), &params));
        assert_eq!(backend.reads.get(), reads + expected_reads);
        drop(lookup);
        drop(owner);
        assert_dropped(&backend);
    }
}

#[test]
fn product_bridge_copies_exact_original_chunks_and_shares_one_cursor_without_losing_blinds() {
    product_bridge_roundtrip::<EqAffine>();
    product_bridge_roundtrip::<EpAffine>();
}

fn product_bridge_refusals<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for is_lookup in [false, true] {
        // Provider fault 3 returns Coefficient: that is now the correct basis, so it is
        // deliberately a positive control rather than counted as a refused product writer.
        for fault in [1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14] {
            let (owner, mut provider, _) = fixture(&params, false);
            let backend = Rc::clone(&provider.backend);
            let reads = backend.reads.get();
            provider.fault = fault;
            let result = catch_unwind(AssertUnwindSafe(|| {
                if is_lookup {
                    owner.create_lookup_product_writer(&mut provider, 3)
                } else {
                    owner.create_copy_product_writer(&mut provider, 7)
                }
            }));
            if fault == 9 {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err(), "kind {is_lookup} fault {fault}");
            }
            assert_eq!(backend.reads.get(), reads);
            assert_dropped(&backend);
        }
        let (owner, mut provider, _) = fixture(&params, false);
        provider.fault = 3;
        let result = if is_lookup {
            owner.create_lookup_product_writer(&mut provider, 0)
        } else {
            owner.create_copy_product_writer(&mut provider, 0)
        };
        let (owner, writer, layout) = result.unwrap();
        assert_eq!(layout.basis(), StoredPolynomialBasisV1::Coefficient);
        drop(writer);
        drop(owner);
        assert_dropped(&provider.backend);
    }
    for fault in 0..10 {
        let (mut owner, provider, _) = fixture(&params, false);
        let backend = Rc::clone(&provider.backend);
        let reads = backend.reads.get();
        let mut output = [C::Scalar::from(23); 16];
        match fault {
            0 => backend.fail_read.set(true),
            1 => backend.panic_read.set(true),
            2 => {
                owner.lagrange.session.as_mut().unwrap().columns[0]
                    .snapshot
                    .values[0] = [0xff; 32]
            }
            3 => owner.coefficients[1].layout.proof_context = [8; 32],
            4 => {
                let identity = Rc::clone(&backend.identities.borrow()[3]);
                let mut layout = identity.get();
                layout.ordinal += 100;
                identity.set(layout);
            }
            5 => {
                owner.lagrange.session.take();
            }
            _ => (),
        }
        let column = if fault == 6 { 2 } else { 0 };
        let chunk = if fault == 7 { 1 } else { 0 };
        let len = if fault == 8 {
            15
        } else if fault == 9 {
            0
        } else {
            16
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            owner.copy_lagrange_chunk_into(column, chunk, &mut output[..len])
        }));
        if fault == 1 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err(), "copy fault {fault}");
        }
        if fault >= 3 {
            assert_eq!(backend.reads.get(), reads);
        }
        assert_dropped(&backend);
        // The caller owns this destination; it must supply a guard in the concrete product
        // stage. This bridge test does not claim erasure of caller-owned scratch on refusal.
    }
    for (last, outputs, success) in [
        (u64::MAX, 0, true),
        (u64::MAX, 1, false),
        (u64::MAX - 2, 1, true),
        (u64::MAX - 2, 2, false),
        (u64::MAX - 3, 2, true),
    ] {
        let (mut owner, provider, _) = fixture(&params, false);
        owner.greatest_ordinal = Some(last);
        let reads = provider.backend.reads.get();
        assert_eq!(owner.product_ordinal_boundary(outputs).is_ok(), success);
        assert_eq!(provider.backend.reads.get(), reads);
        drop(owner);
        assert_dropped(&provider.backend);
    }
    let (owner, mut provider, _) = fixture(&params, true);
    assert_eq!(owner.product_ordinal_boundary(2).unwrap(), None);
    let (owner, writer, first) = owner.create_copy_product_writer(&mut provider, 0).unwrap();
    drop(writer);
    provider.next = first.ordinal();
    assert!(
        owner
            .create_lookup_product_writer(&mut provider, 0)
            .is_err()
    );
    assert_dropped(&provider.backend);
}

#[test]
fn product_bridge_output_identity_bounds_backend_errors_and_unwinds_consume_every_basis() {
    product_bridge_refusals::<EqAffine>();
    product_bridge_refusals::<EpAffine>();
}

fn interleaved_product_fixture<'params, C>(
    params: &'params ParamsIPA<C>,
) -> (CoefficientStoredAdviceV1<'params, C, Snapshot>, Provider)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    use crate::plonk::{SecondPhase, ThirdPhase};
    let domain = EvaluationDomain::<C::Scalar>::new(3, params.k());
    let mut meta = ConstraintSystem::default();
    meta.advice_column();
    meta.advice_column_in(SecondPhase);
    meta.challenge_usable_after(SecondPhase);
    meta.advice_column();
    meta.challenge_usable_after(FirstPhase);
    meta.advice_column_in(ThirdPhase);
    meta.challenge_usable_after(ThirdPhase);
    meta.advice_column_in(SecondPhase);
    meta.challenge_usable_after(FirstPhase);
    let plan = admit_stored_phase_plan_v1(params, &domain, &meta).unwrap();
    let mut provider = Provider {
        backend: Rc::new(Backend::default()),
        context: [9; 32],
        next: 11,
        fault: 0,
        created: 0,
    };
    let mut phase_writers = plan
        .phases
        .iter()
        .enumerate()
        .map(|(phase, entry)| {
            entry
                .columns
                .iter()
                .map(|column| {
                    provider
                        .create(
                            C::Scalar::STORED_FIELD,
                            StoredPolynomialBasisV1::Lagrange,
                            params.k(),
                            StoredPolynomialRoleV1::Advice {
                                column: *column as u32,
                                phase: phase as u8,
                            },
                        )
                        .unwrap()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut active =
        StoredPhaseAssignmentsV1::<C, Writer>::begin(plan, std::mem::take(&mut phase_writers[0]))
            .unwrap();
    let mut rng = ChaCha20Rng::from_seed([81; 32]);
    let mut transcript = Blake2bWrite::<Vec<u8>, C, Challenge255<C>>::init(Vec::new());
    for phase in 0..3 {
        let state = active.active.as_ref().unwrap();
        let usable = state.session.plan.usable_rows;
        let columns = state.session.plan.phases[phase].columns.clone();
        for column in columns {
            for row in 0..usable {
                active
                    .assign_discarding_value(
                        column,
                        row,
                        C::Scalar::from((column + row + 1) as u64).into(),
                    )
                    .unwrap();
            }
        }
        let committed = active
            .finish(&mut rng)
            .unwrap()
            .absorb(&mut transcript)
            .unwrap();
        if phase == 2 {
            let owner = committed
                .into_complete()
                .unwrap()
                .stage_coefficients(&domain, &mut provider)
                .unwrap();
            return (owner, provider);
        }
        active = committed
            .begin_next(std::mem::take(&mut phase_writers[phase + 1]))
            .unwrap();
    }
    unreachable!("three phases include a final completion")
}

fn product_partition<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for fault in 0..12 {
        let (mut owner, provider) = interleaved_product_fixture(&params);
        let backend = Rc::clone(&provider.backend);
        let reads = backend.reads.get();
        let session = owner.lagrange.session.as_mut().unwrap();
        assert_eq!(session.plan.phases[0].columns, [0, 2]);
        assert_eq!(session.plan.phases[1].columns, [1, 4]);
        assert_eq!(session.plan.phases[2].columns, [3]);
        assert_eq!(session.plan.phases[0].challenges, [1, 3]);
        assert_eq!(session.plan.phases[1].challenges, [0]);
        assert_eq!(session.plan.phases[2].challenges, [2]);
        match fault {
            0 => (),
            1 => session.plan.phases[0].columns.reverse(),
            2 => session.plan.phases[0].columns[1] = 0,
            3 => session.plan.phases[1].columns[0] = 0,
            4 => {
                session.plan.phases[1].columns.pop();
            }
            5 => session.plan.phases[2].columns[0] = 5,
            6 => session.plan.phases[0].challenges.reverse(),
            7 => session.plan.phases[0].challenges[1] = 1,
            8 => session.plan.phases[1].challenges[0] = 1,
            9 => {
                session.plan.phases[2].challenges.clear();
            }
            10 => session.plan.phases[2].challenges[0] = 4,
            11 => session.plan.phases[1].columns.clear(),
            _ => unreachable!(),
        }
        let columns = session.columns.as_ptr();
        let coefficients = owner.coefficients.as_ptr();
        assert_eq!(
            owner.validate_live_receipts().is_ok(),
            fault == 0,
            "partition {fault}"
        );
        assert_eq!(
            owner.lagrange.session.as_ref().unwrap().columns.as_ptr(),
            columns
        );
        assert_eq!(owner.coefficients.as_ptr(), coefficients);
        assert_eq!(backend.reads.get(), reads);
        assert_eq!(backend.live_snapshots.get(), 10);
        drop(owner);
        assert_dropped(&backend);
    }
}

#[test]
fn interleaved_three_phase_partitions_preserve_owners_and_reject_duplicates_gaps_and_wrong_phase() {
    // Production now verifies sorted lists and their partition directly without temporary
    // membership Vecs. These tests exercise that grammar and owner pointers, not global RSS.
    product_partition::<EqAffine>();
    product_partition::<EpAffine>();
}
