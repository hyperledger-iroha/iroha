//! Independent actual phase ownership, sole-blind handoff and destructor-boundary tests.
//!
//! Plaintext receipts expose identity and allocation movement for tests only. These checks
//! measure neither encrypted storage nor whole-process memory and never copy a production blind.

use super::*;
use crate::{
    plonk::{ConstraintSystem, FirstPhase, SecondPhase, ThirdPhase},
    poly::{
        EvaluationDomain,
        commitment::ParamsProver as _,
        stored_advice::{
            StoredPastaFieldV1, StoredPolynomialProviderV1, StoredPolynomialWriterV1,
            phase::{StoredPhaseAssignmentsV1, admit_stored_phase_plan_v1},
        },
    },
    transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
};
use ff::{Field, FromUniformBytes, PrimeField, WithSmallOrderMulGroup};
use halo2curves::pasta::{EpAffine, EqAffine};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[derive(Clone, Copy)]
enum DropFault {
    Drift(u64, u64),
    Panic(u64),
}
#[derive(Default)]
struct RetirementBank {
    live: Cell<usize>,
    writers: Cell<usize>,
    reads: Cell<usize>,
    creates: Cell<usize>,
    identities: RefCell<Vec<Rc<Cell<StoredPolynomialLayoutV1>>>>,
    dropped: RefCell<Vec<u64>>,
    fault: Cell<Option<DropFault>>,
    layout_panic: Cell<Option<u64>>,
    layout_calls: Cell<usize>,
    fold_fault: Cell<Option<(usize, bool)>>,
}
struct RetirementSnapshot {
    layout: Rc<Cell<StoredPolynomialLayoutV1>>,
    values: Vec<[u8; 32]>,
    bank: Rc<RetirementBank>,
}
impl StoredPolynomialSnapshotV1 for RetirementSnapshot {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let mut layout = self.layout.get();
        let call = self.bank.layout_calls.get();
        self.bank.layout_calls.set(call + 1);
        if let Some((target, panic)) = self.bank.fold_fault.get() {
            if target == call {
                self.bank.fold_fault.set(None);
                if panic {
                    panic!("injected opening blind validation unwind");
                }
                layout.proof_context = [8; 32];
                self.layout.set(layout);
            }
        }
        if self.bank.layout_panic.get() == Some(layout.ordinal()) {
            self.bank.layout_panic.set(None);
            panic!("injected coefficient handoff layout unwind");
        }
        layout
    }
    fn with_chunk<V>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        assert_eq!(self.layout.get(), expected);
        self.bank.reads.set(self.bank.reads.get() + 1);
        let start = chunk as usize * 256;
        consume(&self.values[start..start + expected.chunk_scalar_count(chunk)?])
    }
    fn with_column<V>(
        &mut self,
        _: StoredPolynomialLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        panic!("unbounded retirement test read")
    }
}
impl Drop for RetirementSnapshot {
    fn drop(&mut self) {
        let ordinal = self.layout.get().ordinal();
        self.bank.live.set(self.bank.live.get() - 1);
        self.bank.dropped.borrow_mut().push(ordinal);
        match self.bank.fault.get() {
            Some(DropFault::Drift(trigger, victim)) if ordinal == trigger => {
                self.bank.fault.set(None);
                let target = Rc::clone(
                    self.bank
                        .identities
                        .borrow()
                        .iter()
                        .find(|l| l.get().ordinal() == victim)
                        .unwrap(),
                );
                let mut layout = target.get();
                layout.proof_context = [8; 32];
                target.set(layout);
            }
            Some(DropFault::Panic(trigger)) if ordinal == trigger => {
                self.bank.fault.set(None);
                panic!("injected original advice retirement unwind");
            }
            _ => (),
        }
    }
}
struct RetirementWriter {
    layout: StoredPolynomialLayoutV1,
    values: Vec<[u8; 32]>,
    bank: Rc<RetirementBank>,
}
impl Drop for RetirementWriter {
    fn drop(&mut self) {
        self.bank.writers.set(self.bank.writers.get() - 1);
    }
}
impl StoredPolynomialWriterV1 for RetirementWriter {
    type Snapshot = RetirementSnapshot;
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        self.layout
    }
    fn write_chunk(
        &mut self,
        chunk: u64,
        values: &[[u8; 32]],
    ) -> Result<(), StoredPolynomialErrorV1> {
        assert_eq!(self.values.len(), chunk as usize * 256);
        assert_eq!(values.len(), self.layout.chunk_scalar_count(chunk)?);
        assert!(values.iter().all(|v| self.layout.field().is_canonical(v)));
        self.values.extend_from_slice(values);
        Ok(())
    }
    fn seal(mut self) -> Result<Self::Snapshot, StoredPolynomialErrorV1> {
        assert_eq!(self.values.len(), 1 << self.layout.k());
        let layout = Rc::new(Cell::new(self.layout));
        self.bank.identities.borrow_mut().push(Rc::clone(&layout));
        self.bank.live.set(self.bank.live.get() + 1);
        Ok(RetirementSnapshot {
            layout,
            values: std::mem::take(&mut self.values),
            bank: Rc::clone(&self.bank),
        })
    }
}
struct RetirementProvider {
    bank: Rc<RetirementBank>,
    next: u64,
}
impl StoredPolynomialProviderV1 for RetirementProvider {
    type Writer = RetirementWriter;
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        let layout = StoredPolynomialLayoutV1::new([9; 32], self.next, field, basis, k, role)?;
        self.next += 1;
        self.bank.writers.set(self.bank.writers.get() + 1);
        self.bank.creates.set(self.bank.creates.get() + 1);
        Ok(RetirementWriter {
            layout,
            values: Vec::new(),
            bank: Rc::clone(&self.bank),
        })
    }
}
fn retirement_fixture<'params, C>(
    params: &'params ParamsIPA<C>,
    empty: bool,
) -> (
    CoefficientStoredAdviceV1<'params, C, RetirementSnapshot>,
    RetirementProvider,
)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let domain = EvaluationDomain::<C::Scalar>::new(3, params.k());
    let mut meta = ConstraintSystem::default();
    if !empty {
        meta.advice_column();
        meta.advice_column_in(SecondPhase);
        meta.challenge_usable_after(SecondPhase);
        meta.advice_column();
        meta.challenge_usable_after(FirstPhase);
        meta.advice_column_in(ThirdPhase);
        meta.challenge_usable_after(ThirdPhase);
        meta.advice_column_in(SecondPhase);
        meta.challenge_usable_after(FirstPhase);
    }
    let plan = admit_stored_phase_plan_v1(params, &domain, &meta).unwrap();
    let phases = plan.phases.len();
    let mut provider = RetirementProvider {
        bank: Rc::new(RetirementBank::default()),
        next: 11,
    };
    let mut writers = plan
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
    let mut active = StoredPhaseAssignmentsV1::<C, RetirementWriter>::begin(
        plan,
        std::mem::take(&mut writers[0]),
    )
    .unwrap();
    let mut rng = ChaCha20Rng::from_seed([81; 32]);
    let mut transcript = Blake2bWrite::<Vec<u8>, C, Challenge255<C>>::init(Vec::new());
    for phase in 0..phases {
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
        if phase + 1 == phases {
            return (
                committed
                    .into_complete()
                    .unwrap()
                    .stage_coefficients(&domain, &mut provider)
                    .unwrap(),
                provider,
            );
        }
        active = committed
            .begin_next(std::mem::take(&mut writers[phase + 1]))
            .unwrap();
    }
    unreachable!()
}
fn retirement_success<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    for k in [4, 8, 9] {
        let params = ParamsIPA::<C>::new(k);
        let (owner, provider) = retirement_fixture(&params, false);
        let bank = Rc::clone(&provider.bank);
        let session = owner.lagrange.session.as_ref().unwrap();
        assert_eq!(session.plan.phases[0].columns, [0, 2]);
        assert_eq!(session.plan.phases[1].columns, [1, 4]);
        assert_eq!(session.plan.phases[2].columns, [3]);
        let phase_ptr = session.plan.phases.as_ptr();
        let challenge_ptr = session.challenges.as_ptr();
        let challenges = owner.challenges().unwrap().collect::<Vec<_>>();
        let originals = session
            .columns
            .iter()
            .map(|c| (c.layout, (c.blind.0).0))
            .collect::<Vec<_>>();
        assert!(originals.iter().all(|(_, b)| *b != C::Scalar::ZERO));
        let coefficients = owner
            .coefficients
            .iter()
            .map(|c| {
                (
                    c.layout,
                    Rc::clone(&c.snapshot.layout),
                    c.snapshot.values.as_ptr(),
                    c.snapshot.values.clone(),
                )
            })
            .collect::<Vec<_>>();
        let reads = bank.reads.get();
        let creates = bank.creates.get();
        take_blind_clear_observations();
        let allocation = owner.prepare_coefficient_only_handoff().unwrap();
        assert!(allocation.columns.is_empty() && allocation.columns.capacity() >= originals.len());
        assert_eq!(
            allocation.payload_bytes().unwrap(),
            allocation.columns.capacity()
                * std::mem::size_of::<RetainedColumn<C, RetirementSnapshot>>()
                + std::mem::size_of_val(&allocation)
        );
        let allocation_ptr = allocation.columns.as_ptr();
        let capacity = allocation.columns.capacity();
        assert_eq!(take_blind_clear_observations(), (0, true));
        let retained = owner.into_coefficient_only(allocation).unwrap();
        assert_eq!(retained.columns.as_ptr(), allocation_ptr);
        assert_eq!(retained.columns.capacity(), capacity);
        assert_eq!(retained.plan.phases.as_ptr(), phase_ptr);
        assert_eq!(retained.challenges.as_ptr(), challenge_ptr);
        assert_eq!(
            retained.challenges().unwrap().collect::<Vec<_>>(),
            challenges
        );
        assert_eq!(retained.proof_context().unwrap(), Some([9; 32]));
        assert!(std::ptr::eq(retained.params().unwrap(), &params));
        assert_eq!(
            retained.greatest_ordinal().unwrap(),
            Some(provider.next - 1)
        );
        assert_eq!(
            (
                bank.reads.get(),
                bank.creates.get(),
                bank.live.get(),
                bank.writers.get()
            ),
            (reads, creates, 5, 0)
        );
        assert_eq!(
            *bank.dropped.borrow(),
            originals
                .iter()
                .map(|(l, _)| l.ordinal())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            take_blind_clear_observations(),
            (0, true),
            "a sole advice blind was destroyed during transfer"
        );
        for ((column, original), coefficient) in
            retained.columns.iter().zip(&originals).zip(&coefficients)
        {
            assert_eq!(column.original, original.0);
            assert_eq!((column.blind.0).0, original.1);
            assert_eq!(column.coefficient.layout, coefficient.0);
            assert!(Rc::ptr_eq(
                &column.coefficient.snapshot.layout,
                &coefficient.1
            ));
            assert_eq!(column.coefficient.snapshot.values.as_ptr(), coefficient.2);
            assert_eq!(column.coefficient.snapshot.values, coefficient.3);
        }
        for _ in 0..3 {
            retained.validate_live_receipts().unwrap();
        }
        assert_eq!(retained.columns.as_ptr(), allocation_ptr);
        assert_eq!(bank.reads.get(), reads);
        drop(retained);
        assert_eq!(bank.live.get(), 0);
        assert_eq!(take_blind_clear_observations(), (5, true));
    }
}
#[test]
fn both_pasta_three_phase_handoff_moves_sole_guards_and_original_allocations_without_reads() {
    retirement_success::<EqAffine>();
    retirement_success::<EpAffine>();
}

fn corrupt_plan<C: CurveAffine>(plan: &mut StoredPhasePlanV1<'_, C>, case: usize) {
    match case {
        0 => plan.phases[0].columns.reverse(),
        1 => plan.phases[0].columns[1] = 0,
        2 => plan.phases[1].columns[0] = 0,
        3 => {
            plan.phases[1].columns.pop();
        }
        4 => plan.phases[2].columns[0] = 5,
        5 => plan.phases[0].challenges.reverse(),
        6 => plan.phases[0].challenges[1] = 1,
        7 => plan.phases[1].challenges[0] = 1,
        8 => plan.phases[2].challenges.clear(),
        9 => plan.phases[2].challenges[0] = 4,
        10 => plan.phases[1].columns.clear(),
        11 => plan.usable_rows = usize::MAX,
        12 => plan.k += 1,
        _ => unreachable!(),
    }
}
fn retirement_refusals<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let alternate = ParamsIPA::<C>::new(4);
    // Check every partition both before the token allocation and on the final owner.
    for final_owner in [false, true] {
        for case in 0..13 {
            let (mut owner, provider) = retirement_fixture(&params, false);
            let reads = provider.bank.reads.get();
            take_blind_clear_observations();
            if final_owner {
                let allocation = owner.prepare_coefficient_only_handoff().unwrap();
                let mut retained = owner.into_coefficient_only(allocation).unwrap();
                corrupt_plan(&mut retained.plan, case);
                assert!(
                    retained.validate_live_receipts().is_err(),
                    "retained plan {case}"
                );
                drop(retained);
            } else {
                corrupt_plan(&mut owner.lagrange.session.as_mut().unwrap().plan, case);
                assert!(
                    owner.prepare_coefficient_only_handoff().is_err(),
                    "input plan {case}"
                );
                drop(owner);
            }
            assert_eq!(provider.bank.reads.get(), reads);
            assert_eq!(provider.bank.live.get(), 0);
            assert_eq!(take_blind_clear_observations(), (5, true));
        }
    }
    for case in 0..11 {
        let (mut owner, provider) = retirement_fixture(&params, false);
        let reads = provider.bank.reads.get();
        let mut allocation = owner.prepare_coefficient_only_handoff().unwrap();
        take_blind_clear_observations();
        match case {
            0 => allocation.params = &alternate,
            1 => allocation.k += 1,
            2 => allocation.count += 1,
            3 => allocation.context = Some([8; 32]),
            4 => allocation.columns = Vec::new(),
            5 => owner.lagrange.session.as_mut().unwrap().challenges[0] = None,
            6 => owner.greatest_ordinal = Some(0),
            7 => {
                let id = &owner.coefficients[4].snapshot.layout;
                let mut l = id.get();
                l.proof_context = [8; 32];
                id.set(l);
            }
            8 => {
                let id = &owner.lagrange.session.as_ref().unwrap().columns[4]
                    .snapshot
                    .layout;
                let mut l = id.get();
                l.ordinal += 1;
                id.set(l);
            }
            9 => owner.coefficients.swap(0, 4),
            10 => owner.proof_context = Some([8; 32]),
            _ => unreachable!(),
        }
        assert!(
            owner.into_coefficient_only(allocation).is_err(),
            "prepared token/identity {case}"
        );
        assert_eq!(provider.bank.reads.get(), reads);
        assert_eq!(provider.bank.live.get(), 0);
        assert_eq!(take_blind_clear_observations(), (5, true));
    }
}
#[test]
fn both_pasta_handoff_and_retained_owner_reject_token_substitution_and_every_phase_partition() {
    retirement_refusals::<EqAffine>();
    retirement_refusals::<EpAffine>();
}

fn retirement_destructors<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    // Each original destructor can alter an already moved or still pending coefficient;
    // earlier destructors additionally target every still-live original source.
    let mut cases = Vec::new();
    for drop_index in 0..5 {
        for coefficient in 0..5 {
            cases.push((drop_index, Some((true, coefficient)), false));
        }
        for source in drop_index + 1..5 {
            cases.push((drop_index, Some((false, source)), false));
        }
        cases.push((drop_index, None, true));
    }
    for (drop_index, victim, panic) in cases {
        let (owner, mut provider) = retirement_fixture(&params, false);
        let bank = Rc::clone(&provider.bank);
        let sources = owner
            .lagrange
            .session
            .as_ref()
            .unwrap()
            .columns
            .iter()
            .map(|c| c.layout.ordinal())
            .collect::<Vec<_>>();
        let coefficients = owner
            .coefficients
            .iter()
            .map(|c| c.layout.ordinal())
            .collect::<Vec<_>>();
        let mut writer = provider
            .create(
                C::Scalar::STORED_FIELD,
                StoredPolynomialBasisV1::Coefficient,
                4,
                StoredPolynomialRoleV1::Instance { column: 77 },
            )
            .unwrap();
        writer
            .write_chunk(0, &[C::Scalar::from(99).to_repr(); 16])
            .unwrap();
        let mut sentinel = writer.seal().unwrap();
        let sentinel_layout = sentinel.layout();
        let reads = bank.reads.get();
        let allocation = owner.prepare_coefficient_only_handoff().unwrap();
        take_blind_clear_observations();
        bank.fault
            .set(Some(if let Some((coefficient, index)) = victim {
                DropFault::Drift(
                    sources[drop_index],
                    if coefficient {
                        coefficients[index]
                    } else {
                        sources[index]
                    },
                )
            } else {
                DropFault::Panic(sources[drop_index])
            }));
        let result = catch_unwind(AssertUnwindSafe(|| {
            owner.into_coefficient_only(allocation).map(|_| ())
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(
            (bank.reads.get(), bank.live.get(), bank.writers.get()),
            (reads, 1, 0)
        );
        assert_eq!(take_blind_clear_observations(), (5, true));
        assert_eq!(sentinel.layout(), sentinel_layout);
        sentinel
            .with_chunk(sentinel_layout, 0, |values| {
                assert_eq!(values, &[C::Scalar::from(99).to_repr(); 16]);
                Ok(())
            })
            .unwrap();
        drop(sentinel);
        assert_eq!(bank.live.get(), 0);
    }
    for coefficient in [false, true] {
        for index in 0..5 {
            let (owner, provider) = retirement_fixture(&params, false);
            let allocation = owner.prepare_coefficient_only_handoff().unwrap();
            let ordinal = if coefficient {
                owner.coefficients[index].layout.ordinal()
            } else {
                owner.lagrange.session.as_ref().unwrap().columns[index]
                    .layout
                    .ordinal()
            };
            take_blind_clear_observations();
            provider.bank.layout_panic.set(Some(ordinal));
            assert!(
                catch_unwind(AssertUnwindSafe(|| owner
                    .into_coefficient_only(allocation)
                    .map(|_| ())))
                .is_err()
            );
            assert_eq!(provider.bank.live.get(), 0);
            assert_eq!(take_blind_clear_observations(), (5, true));
        }
    }
}
#[test]
fn both_pasta_every_last_use_drop_detects_remaining_or_moved_receipt_drift_and_unwind() {
    retirement_destructors::<EqAffine>();
    retirement_destructors::<EpAffine>();
}

fn empty_retirement<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for establish in [false, true] {
        let (owner, mut provider) = retirement_fixture(&params, true);
        assert_eq!(owner.proof_context().unwrap(), None);
        let allocation = owner.prepare_coefficient_only_handoff().unwrap();
        // Capacity is prepared before the zero-source proof's first actual writer.
        let owner = if establish {
            let (owner, writer, layout) = owner.create_vanishing_writer(&mut provider).unwrap();
            assert_eq!(layout.role(), StoredPolynomialRoleV1::VanishingRandom);
            assert_eq!(layout.ordinal(), 11);
            drop(writer);
            owner
        } else {
            owner
        };
        take_blind_clear_observations();
        let retained = owner.into_coefficient_only(allocation).unwrap();
        assert!(retained.layouts().unwrap().next().is_none());
        assert!(retained.challenges().unwrap().next().is_none());
        assert_eq!(
            retained.proof_context().unwrap(),
            establish.then_some([9; 32])
        );
        assert_eq!(
            retained.greatest_ordinal().unwrap(),
            establish.then_some(11)
        );
        assert_eq!(provider.bank.reads.get(), 0);
        assert_eq!(provider.bank.creates.get(), usize::from(establish));
        drop(retained);
        assert_eq!(take_blind_clear_observations(), (0, true));
        assert_eq!(provider.bank.live.get(), 0);
    }
}
#[test]
fn both_pasta_empty_handoff_preserves_absence_or_first_actual_writer_context_without_invention() {
    empty_retirement::<EqAffine>();
    empty_retirement::<EpAffine>();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InverseFactoryFault {
    Error,
    Panic,
    Field,
    K,
    Basis,
    Role,
    Context,
    Index,
    Extension,
    Ordinal,
    Exhausted,
    SecondLayout,
    FinalLayout,
    Drift,
}
struct InverseFactoryProvider {
    inner: RetirementProvider,
    fault: Option<InverseFactoryFault>,
    attempts: Rc<Cell<usize>>,
}
struct InverseFactoryWriter {
    inner: RetirementWriter,
    fault: Option<InverseFactoryFault>,
    observations: Cell<usize>,
}
impl StoredPolynomialWriterV1 for InverseFactoryWriter {
    type Snapshot = RetirementSnapshot;
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let count = self.observations.get() + 1;
        self.observations.set(count);
        let mut layout = self.inner.layout();
        if (self.fault == Some(InverseFactoryFault::SecondLayout) && count >= 2)
            || (self.fault == Some(InverseFactoryFault::FinalLayout) && count >= 3)
        {
            layout.proof_context = [8; 32];
        }
        layout
    }
    fn write_chunk(
        &mut self,
        chunk: u64,
        values: &[[u8; 32]],
    ) -> Result<(), StoredPolynomialErrorV1> {
        self.inner.write_chunk(chunk, values)
    }
    fn seal(self) -> Result<Self::Snapshot, StoredPolynomialErrorV1> {
        self.inner.seal()
    }
}
impl StoredPolynomialProviderV1 for InverseFactoryProvider {
    type Writer = InverseFactoryWriter;
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        self.attempts.set(self.attempts.get() + 1);
        if self.fault == Some(InverseFactoryFault::Error) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        assert_ne!(
            self.fault,
            Some(InverseFactoryFault::Panic),
            "injected inverse named factory unwind"
        );
        let mut inner = self.inner.create(field, basis, k, role)?;
        match self.fault {
            Some(InverseFactoryFault::Field) => {
                inner.layout.field = match field {
                    StoredPastaFieldV1::Fp => StoredPastaFieldV1::Fq,
                    StoredPastaFieldV1::Fq => StoredPastaFieldV1::Fp,
                }
            }
            Some(InverseFactoryFault::K) => inner.layout.k += 1,
            Some(InverseFactoryFault::Basis) => {
                inner.layout.basis = StoredPolynomialBasisV1::Lagrange
            }
            Some(InverseFactoryFault::Role) => {
                inner.layout.role = StoredPolynomialRoleV1::Instance { column: 77 }
            }
            Some(InverseFactoryFault::Context) => inner.layout.proof_context = [8; 32],
            Some(InverseFactoryFault::Index) => {
                inner.layout.role = match role {
                    StoredPolynomialRoleV1::QuotientAliasedPart {
                        part,
                        extension_log,
                    } => StoredPolynomialRoleV1::QuotientAliasedPart {
                        part: part ^ 1,
                        extension_log,
                    },
                    StoredPolynomialRoleV1::QuotientPiece { piece } => {
                        StoredPolynomialRoleV1::QuotientPiece { piece: piece + 1 }
                    }
                    _ => unreachable!(),
                }
            }
            Some(InverseFactoryFault::Extension) => {
                inner.layout.role = StoredPolynomialRoleV1::QuotientAliasedPart {
                    part: 0,
                    extension_log: 3,
                }
            }
            Some(InverseFactoryFault::Ordinal) => inner.layout.ordinal = 0,
            Some(InverseFactoryFault::Exhausted) => inner.layout.ordinal = u64::MAX,
            Some(InverseFactoryFault::Drift) => {
                let identity = Rc::clone(self.inner.bank.identities.borrow().last().unwrap());
                let mut layout = identity.get();
                layout.proof_context = [8; 32];
                identity.set(layout);
            }
            _ => (),
        }
        Ok(InverseFactoryWriter {
            inner,
            fault: self.fault,
            observations: Cell::new(0),
        })
    }
}
fn inverse_phase_fixture<'params, C>(
    params: &'params ParamsIPA<C>,
    empty: bool,
    establish: bool,
) -> (
    CoefficientOnlyStoredAdviceV1<'params, C, RetirementSnapshot>,
    RetirementProvider,
)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let (owner, mut provider) = retirement_fixture(params, empty);
    let allocation = owner.prepare_coefficient_only_handoff().unwrap();
    let owner = if establish && empty {
        let (owner, writer, _) = owner.create_vanishing_writer(&mut provider).unwrap();
        drop(writer);
        owner
    } else {
        owner
    };
    (owner.into_coefficient_only(allocation).unwrap(), provider)
}
fn inverse_named_factory_success<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    for k in [4, 8, 9] {
        let params = ParamsIPA::<C>::new(k);
        for empty in [false, true] {
            let (mut owner, mut provider) = inverse_phase_fixture(&params, empty, true);
            let layouts = owner.layouts().unwrap().collect::<Vec<_>>();
            let values = owner
                .columns
                .iter()
                .map(|column| column.coefficient.snapshot.values.clone())
                .collect::<Vec<_>>();
            let blinds = owner
                .columns
                .iter()
                .map(|column| (column.blind.0).0)
                .collect::<Vec<_>>();
            let challenges = owner.challenges().unwrap().collect::<Vec<_>>();
            let phase_pointer = owner.plan.phases.as_ptr();
            let challenge_pointer = owner.challenges.as_ptr();
            let reads = provider.bank.reads.get();
            provider.next += 17;
            take_blind_clear_observations();
            for extension in [1, 2, 19 - k] {
                for part in [0, (1_u32 << extension) - 1] {
                    let next = provider.next;
                    let (retained, writer, layout) = owner
                        .create_quotient_alias_writer(&mut provider, extension, part)
                        .unwrap();
                    owner = retained;
                    assert_eq!(
                        layout.role(),
                        StoredPolynomialRoleV1::QuotientAliasedPart {
                            part,
                            extension_log: extension
                        }
                    );
                    assert_eq!(layout.basis(), StoredPolynomialBasisV1::Coefficient);
                    assert_eq!(layout.ordinal(), next);
                    assert_eq!(writer.layout(), layout);
                    drop(writer);
                    assert_eq!(owner.greatest_ordinal().unwrap(), Some(next));
                }
            }
            for piece in [0, 1, (1_u32 << (19 - k)) - 1] {
                let next = provider.next;
                let (retained, writer, layout) = owner
                    .create_quotient_piece_writer(&mut provider, piece)
                    .unwrap();
                owner = retained;
                assert_eq!(
                    layout.role(),
                    StoredPolynomialRoleV1::QuotientPiece { piece }
                );
                assert_eq!(layout.basis(), StoredPolynomialBasisV1::Coefficient);
                assert_eq!(layout.ordinal(), next);
                assert_eq!(writer.layout(), layout);
                drop(writer);
                assert_eq!(owner.greatest_ordinal().unwrap(), Some(next));
            }
            assert_eq!(owner.layouts().unwrap().collect::<Vec<_>>(), layouts);
            assert_eq!(owner.challenges().unwrap().collect::<Vec<_>>(), challenges);
            assert_eq!(
                owner
                    .columns
                    .iter()
                    .map(|column| column.coefficient.snapshot.values.clone())
                    .collect::<Vec<_>>(),
                values
            );
            assert_eq!(
                owner
                    .columns
                    .iter()
                    .map(|column| (column.blind.0).0)
                    .collect::<Vec<_>>(),
                blinds
            );
            assert_eq!(owner.plan.phases.as_ptr(), phase_pointer);
            assert_eq!(owner.challenges.as_ptr(), challenge_pointer);
            assert!(std::ptr::eq(owner.params().unwrap(), &params));
            assert_eq!(provider.bank.reads.get(), reads);
            assert_eq!(provider.bank.writers.get(), 0);
            assert_eq!(take_blind_clear_observations(), (0, true));
            drop(owner);
            assert_eq!(
                take_blind_clear_observations(),
                (if empty { 0 } else { 5 }, true)
            );
            assert_eq!(provider.bank.live.get(), 0);
        }
    }
}
#[test]
fn both_pasta_inverse_named_alias_piece_factories_preserve_original_phases_challenges_blinds_and_global_cursor()
 {
    inverse_named_factory_success::<EqAffine>();
    inverse_named_factory_success::<EpAffine>();
}
fn inverse_named_factory_failures<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for alias in [false, true] {
        for fault in [
            InverseFactoryFault::Error,
            InverseFactoryFault::Panic,
            InverseFactoryFault::Field,
            InverseFactoryFault::K,
            InverseFactoryFault::Basis,
            InverseFactoryFault::Role,
            InverseFactoryFault::Context,
            InverseFactoryFault::Index,
            InverseFactoryFault::Extension,
            InverseFactoryFault::Ordinal,
            InverseFactoryFault::Exhausted,
            InverseFactoryFault::SecondLayout,
            InverseFactoryFault::FinalLayout,
            InverseFactoryFault::Drift,
        ] {
            let (owner, provider) = inverse_phase_fixture(&params, false, true);
            let bank = Rc::clone(&provider.bank);
            let reads = bank.reads.get();
            let attempts = Rc::new(Cell::new(0));
            let mut provider = InverseFactoryProvider {
                inner: provider,
                fault: Some(fault),
                attempts: Rc::clone(&attempts),
            };
            take_blind_clear_observations();
            let result = catch_unwind(AssertUnwindSafe(|| {
                if alias {
                    owner.create_quotient_alias_writer(&mut provider, 2, 1)
                } else {
                    owner.create_quotient_piece_writer(&mut provider, 1)
                }
            }));
            if fault == InverseFactoryFault::Panic {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err(), "accepted {fault:?}");
            }
            assert_eq!(attempts.get(), 1);
            assert_eq!(bank.reads.get(), reads);
            assert_eq!(bank.live.get(), 0);
            assert_eq!(bank.writers.get(), 0);
            assert_eq!(take_blind_clear_observations(), (5, true));
        }
    }
    for case in 0..7 {
        let (owner, mut provider) = inverse_phase_fixture(&params, case >= 5, case < 5);
        let creates = provider.bank.creates.get();
        let reads = provider.bank.reads.get();
        take_blind_clear_observations();
        let result = match case {
            0 => owner.create_quotient_alias_writer(&mut provider, 0, 0),
            1 => owner.create_quotient_alias_writer(&mut provider, 16, 0),
            2 => owner.create_quotient_alias_writer(&mut provider, 2, 4),
            3 => owner.create_quotient_piece_writer(&mut provider, 1 << 15),
            4 => owner.create_quotient_piece_writer(&mut provider, u32::MAX),
            5 => owner.create_quotient_alias_writer(&mut provider, 2, 0),
            6 => owner.create_quotient_piece_writer(&mut provider, 0),
            _ => unreachable!(),
        };
        assert!(result.is_err());
        assert_eq!(provider.bank.creates.get(), creates);
        assert_eq!(provider.bank.reads.get(), reads);
        assert_eq!(provider.bank.live.get(), 0);
        assert_eq!(provider.bank.writers.get(), 0);
        assert_eq!(
            take_blind_clear_observations(),
            (if case >= 5 { 0 } else { 5 }, true)
        );
    }
    for alias in [false, true] {
        let (owner, mut provider) = inverse_phase_fixture(&params, false, true);
        provider.next = u64::MAX - 1;
        let (owner, writer, layout) = if alias {
            owner
                .create_quotient_alias_writer(&mut provider, 2, 0)
                .unwrap()
        } else {
            owner
                .create_quotient_piece_writer(&mut provider, 0)
                .unwrap()
        };
        assert_eq!(layout.ordinal(), u64::MAX - 1);
        assert_eq!(provider.next, u64::MAX);
        drop(writer);
        let creates = provider.bank.creates.get();
        take_blind_clear_observations();
        let result = if alias {
            owner.create_quotient_alias_writer(&mut provider, 2, 1)
        } else {
            owner.create_quotient_piece_writer(&mut provider, 1)
        };
        assert!(result.is_err());
        assert_eq!(provider.bank.creates.get(), creates);
        assert_eq!(provider.bank.live.get(), 0);
        assert_eq!(provider.bank.writers.get(), 0);
        assert_eq!(take_blind_clear_observations(), (5, true));
    }
}
#[test]
fn both_pasta_inverse_named_factories_refuse_substitution_missing_context_invalid_coordinates_and_exhausted_cursor()
 {
    inverse_named_factory_failures::<EqAffine>();
    inverse_named_factory_failures::<EpAffine>();
}

#[path = "retirement/blind_tests.rs"]
mod blind_tests;
