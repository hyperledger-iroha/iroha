//! Independent original coefficient read/cursor bridges and consuming-error regressions.
//!
//! Plaintext receipts expose identity and allocation movement for tests only. These checks
//! measure neither encrypted storage nor whole-process memory and never copy a production blind.

use super::super::*;
use super::*;
use crate::{
    plonk::{ConstraintSystem, FirstPhase, SecondPhase, ThirdPhase},
    poly::{
        EvaluationDomain,
        stored_advice::{
            StoredLookupSideV1, StoredPastaFieldV1, StoredPolynomialProviderV1,
            StoredPolynomialWriterV1,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadFault {
    Error,
    Panic,
    Short,
    Long,
    Encoding,
    Drift(u64),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CreateFault {
    Error,
    Panic,
    Context,
    Field,
    Basis,
    K,
    Role,
    Ordinal,
    Exhausted,
    SecondLayout,
    FinalAdviceSweep,
    Drift(u64),
}
#[derive(Default)]
struct PhaseBank {
    live: Cell<usize>,
    writers: Cell<usize>,
    reads: Cell<usize>,
    busy: Cell<bool>,
    writer_checks: Cell<usize>,
    late_writer_drift: Cell<bool>,
    read_fault: Cell<Option<(u64, u64, ReadFault)>>,
    create_fault: Cell<Option<CreateFault>>,
    creates: Cell<usize>,
    identities: RefCell<Vec<Rc<Cell<StoredPolynomialLayoutV1>>>>,
    dropped: RefCell<Vec<u64>>,
    fault: Cell<Option<DropFault>>,
    layout_panic: Cell<Option<u64>>,
}
struct ReadWindow(Rc<PhaseBank>);
impl Drop for ReadWindow {
    fn drop(&mut self) {
        self.0.busy.set(false);
    }
}
fn phase_drift(bank: &PhaseBank, victim: u64) {
    let identity = bank
        .identities
        .borrow()
        .iter()
        .find(|v| v.get().ordinal() == victim)
        .unwrap()
        .clone();
    let mut layout = identity.get();
    layout.proof_context = [8; 32];
    identity.set(layout);
}
struct PhaseSnapshot {
    poisoned: bool,
    layout: Rc<Cell<StoredPolynomialLayoutV1>>,
    values: Vec<[u8; 32]>,
    bank: Rc<PhaseBank>,
}
impl StoredPolynomialSnapshotV1 for PhaseSnapshot {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let layout = self.layout.get();
        if self.bank.create_fault.get() == Some(CreateFault::FinalAdviceSweep)
            && self.bank.writer_checks.get() >= 2
        {
            self.bank.late_writer_drift.set(true);
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
        if self.poisoned {
            return Err(StoredPolynomialErrorV1::Poisoned);
        }
        if self.layout.get() != expected {
            return Err(StoredPolynomialErrorV1::Context);
        }
        assert!(
            !self.bank.busy.replace(true),
            "coefficient callbacks never overlap"
        );
        let _window = ReadWindow(Rc::clone(&self.bank));
        self.poisoned = true;
        self.bank.reads.set(self.bank.reads.get() + 1);
        let start = chunk as usize * 256;
        let mut encoded = self.values[start..start + expected.chunk_scalar_count(chunk)?].to_vec();
        let fault = self
            .bank
            .read_fault
            .get()
            .filter(|(ordinal, index, _)| *ordinal == expected.ordinal() && *index == chunk)
            .map(|(_, _, fault)| fault);
        match fault {
            Some(ReadFault::Error) => return Err(StoredPolynomialErrorV1::Storage),
            Some(ReadFault::Panic) => panic!("injected coefficient decoder window unwind"),
            Some(ReadFault::Short) => {
                encoded.pop();
            }
            Some(ReadFault::Long) => encoded.push([0; 32]),
            Some(ReadFault::Encoding) => {
                let last = encoded.len() - 1;
                encoded[last] = [255; 32];
            }
            _ => (),
        }
        let result = consume(&encoded)?;
        if let Some(ReadFault::Drift(victim)) = fault {
            phase_drift(&self.bank, victim);
        }
        self.poisoned = false;
        Ok(result)
    }
    fn with_column<V>(
        &mut self,
        _: StoredPolynomialLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        panic!("unbounded retirement test read")
    }
}
impl Drop for PhaseSnapshot {
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
struct PhaseWriter {
    observations: Cell<usize>,
    layout: StoredPolynomialLayoutV1,
    values: Vec<[u8; 32]>,
    bank: Rc<PhaseBank>,
}
impl Drop for PhaseWriter {
    fn drop(&mut self) {
        self.bank.writers.set(self.bank.writers.get() - 1);
    }
}
impl StoredPolynomialWriterV1 for PhaseWriter {
    type Snapshot = PhaseSnapshot;
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let count = self.observations.get() + 1;
        self.observations.set(count);
        let mut layout = self.layout;
        if self.bank.create_fault.get() == Some(CreateFault::FinalAdviceSweep) {
            self.bank
                .writer_checks
                .set(self.bank.writer_checks.get() + 1);
            if self.bank.late_writer_drift.get() {
                layout.proof_context = [8; 32];
            }
        }
        if self.bank.create_fault.get() == Some(CreateFault::SecondLayout) && count > 1 {
            layout.proof_context = [8; 32];
        }
        layout
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
        Ok(PhaseSnapshot {
            poisoned: false,
            layout,
            values: std::mem::take(&mut self.values),
            bank: Rc::clone(&self.bank),
        })
    }
}
struct PhaseProvider {
    bank: Rc<PhaseBank>,
    next: u64,
}
impl StoredPolynomialProviderV1 for PhaseProvider {
    type Writer = PhaseWriter;
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        self.bank.creates.set(self.bank.creates.get() + 1);
        let fault = self.bank.create_fault.get();
        if fault == Some(CreateFault::FinalAdviceSweep) {
            self.bank.writer_checks.set(0);
            self.bank.late_writer_drift.set(false);
        }
        if fault == Some(CreateFault::Error) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        assert_ne!(
            fault,
            Some(CreateFault::Panic),
            "injected quotient factory unwind"
        );
        let mut layout = StoredPolynomialLayoutV1::new([9; 32], self.next, field, basis, k, role)?;
        self.next = self
            .next
            .checked_add(1)
            .ok_or(StoredPolynomialErrorV1::Capacity)?;
        match fault {
            Some(CreateFault::Context) => layout.proof_context = [8; 32],
            Some(CreateFault::Field) => {
                layout.field = match field {
                    StoredPastaFieldV1::Fp => StoredPastaFieldV1::Fq,
                    StoredPastaFieldV1::Fq => StoredPastaFieldV1::Fp,
                }
            }
            Some(CreateFault::Basis) => layout.basis = StoredPolynomialBasisV1::Coefficient,
            Some(CreateFault::K) => layout.k += 1,
            Some(CreateFault::Role) => {
                layout.role = StoredPolynomialRoleV1::Instance { column: 99 }
            }
            Some(CreateFault::Ordinal) => layout.ordinal = 0,
            Some(CreateFault::Exhausted) => layout.ordinal = u64::MAX,
            Some(CreateFault::Drift(victim)) => phase_drift(&self.bank, victim),
            _ => (),
        }
        self.bank.writers.set(self.bank.writers.get() + 1);
        Ok(PhaseWriter {
            observations: Cell::new(0),
            layout,
            values: Vec::new(),
            bank: Rc::clone(&self.bank),
        })
    }
}
fn phase_fixture<'params, C>(
    params: &'params ParamsIPA<C>,
    empty: bool,
) -> (
    CoefficientStoredAdviceV1<'params, C, PhaseSnapshot>,
    PhaseProvider,
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
    let mut provider = PhaseProvider {
        bank: Rc::new(PhaseBank::default()),
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
    let mut active =
        StoredPhaseAssignmentsV1::<C, PhaseWriter>::begin(plan, std::mem::take(&mut writers[0]))
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
fn coefficient_fixture<'params, C>(
    params: &'params ParamsIPA<C>,
    empty: bool,
) -> (
    CoefficientOnlyStoredAdviceV1<'params, C, PhaseSnapshot>,
    PhaseProvider,
)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let (owner, provider) = phase_fixture(params, empty);
    let allocation = owner.prepare_coefficient_only_handoff().unwrap();
    (owner.into_coefficient_only(allocation).unwrap(), provider)
}
fn sentinel<C: CurveAffine>(provider: &mut PhaseProvider, k: u32) -> PhaseSnapshot
where
    C::Scalar: StoredAssignmentFieldV1,
{
    let mut writer = provider
        .create(
            C::Scalar::STORED_FIELD,
            StoredPolynomialBasisV1::Coefficient,
            k,
            StoredPolynomialRoleV1::Instance { column: 77 },
        )
        .unwrap();
    let layout = writer.layout();
    for chunk in 0..layout.chunk_count() as u64 {
        writer
            .write_chunk(
                chunk,
                &vec![C::Scalar::from(53).to_repr(); layout.chunk_scalar_count(chunk).unwrap()],
            )
            .unwrap();
    }
    writer.seal().unwrap()
}
fn assert_sentinel<C: CurveAffine>(snapshot: &mut PhaseSnapshot)
where
    C::Scalar: StoredAssignmentFieldV1,
{
    let layout = snapshot.layout();
    snapshot
        .with_chunk(layout, 0, |values| {
            assert!(values.iter().all(|v| *v == C::Scalar::from(53).to_repr()));
            Ok(())
        })
        .unwrap();
}

fn bridge_success<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    for k in [4, 8, 9] {
        let params = ParamsIPA::<C>::new(k);
        let (mut owner, mut provider) = coefficient_fixture(&params, false);
        let bank = Rc::clone(&provider.bank);
        let original_phases = owner.plan.phases.as_ptr();
        let original_challenges = owner.challenges.as_ptr();
        let values = owner
            .columns
            .iter()
            .map(|c| c.coefficient.snapshot.values.clone())
            .collect::<Vec<_>>();
        let layouts = owner.layouts().unwrap().collect::<Vec<_>>();
        let blinds = owner
            .columns
            .iter()
            .map(|c| (c.blind.0).0)
            .collect::<Vec<_>>();
        let challenges = owner.challenges().unwrap().collect::<Vec<_>>();
        assert_eq!(owner.plan.phases[0].columns, [0, 2]);
        assert_eq!(owner.plan.phases[1].columns, [1, 4]);
        assert_eq!(owner.plan.phases[2].columns, [3]);
        assert_eq!(challenges.len(), 4);
        let before = bank.reads.get();
        take_blind_clear_observations();
        for (column, layout) in layouts.iter().enumerate() {
            for chunk in 0..layout.chunk_count() as u64 {
                let len = layout.chunk_scalar_count(chunk).unwrap();
                let mut output = vec![C::Scalar::from(71); len];
                owner = owner
                    .copy_coefficient_chunk_into(column as u32, chunk, &mut output)
                    .unwrap();
                let start = chunk as usize * 256;
                assert_eq!(
                    output.iter().map(PrimeField::to_repr).collect::<Vec<_>>(),
                    values[column][start..start + len]
                );
                assert_eq!(owner.plan.phases.as_ptr(), original_phases);
                assert_eq!(owner.challenges.as_ptr(), original_challenges);
                assert_eq!(owner.challenges().unwrap().collect::<Vec<_>>(), challenges);
                assert_eq!(
                    owner
                        .columns
                        .iter()
                        .map(|c| (c.blind.0).0)
                        .collect::<Vec<_>>(),
                    blinds
                );
                assert_eq!(take_blind_clear_observations(), (0, true));
                assert!(!bank.busy.get());
            }
        }
        assert_eq!(bank.reads.get() - before, 5 * layouts[0].chunk_count());
        let old = owner.greatest_ordinal().unwrap();
        assert_eq!(owner.quotient_ordinal_boundary(0).unwrap(), old);
        assert_eq!(owner.quotient_ordinal_boundary(9).unwrap(), old);
        let reads = bank.reads.get();
        // Provider gaps advance the one original cursor; no identity is inferred from a live
        // snapshot maximum after writers have been dropped.
        provider.next += 7;
        for source in &layouts {
            let expected = provider.next;
            let (next, writer, layout) = owner
                .create_coset_writer(&mut provider, *source, 2, 3)
                .unwrap();
            owner = next;
            assert_eq!(layout.ordinal(), expected);
            assert_eq!(layout.role(), source.role());
            assert_eq!(layout.field(), source.field());
            assert_eq!(layout.k(), source.k());
            assert!(layout.same_proof_context(*source));
            assert_eq!(
                layout.basis(),
                StoredPolynomialBasisV1::CosetPart {
                    extension_log: 2,
                    part: 3
                }
            );
            assert_eq!(owner.greatest_ordinal().unwrap(), Some(expected));
            assert_eq!(writer.layout(), layout);
            drop(writer);
        }
        // These closed roles preserve labels only. The phase bridge deliberately cannot
        // authenticate a non-advice witness from metadata; the consuming outer owner must
        // validate its actual retained snapshot and logical index before calling this factory.
        for role in [
            StoredPolynomialRoleV1::Instance { column: 3 },
            StoredPolynomialRoleV1::CopyPermutationProduct { set: 2 },
            StoredPolynomialRoleV1::LookupPermuted {
                lookup: 1,
                side: StoredLookupSideV1::Input,
            },
            StoredPolynomialRoleV1::LookupPermuted {
                lookup: 1,
                side: StoredLookupSideV1::Table,
            },
            StoredPolynomialRoleV1::LookupProduct { lookup: 1 },
        ] {
            let source = StoredPolynomialLayoutV1::new(
                owner.proof_context().unwrap().unwrap(),
                owner.greatest_ordinal().unwrap().unwrap(),
                C::Scalar::STORED_FIELD,
                StoredPolynomialBasisV1::Coefficient,
                k,
                role,
            )
            .unwrap();
            let expected = provider.next;
            let (next, writer, layout) = owner
                .create_coset_writer(&mut provider, source, 2, 1)
                .unwrap();
            owner = next;
            assert_eq!(layout.role(), role);
            assert_eq!(layout.ordinal(), expected);
            assert_eq!(
                layout.basis(),
                StoredPolynomialBasisV1::CosetPart {
                    extension_log: 2,
                    part: 1
                }
            );
            assert!(layout.same_proof_context(source));
            assert_eq!(owner.greatest_ordinal().unwrap(), Some(expected));
            assert_eq!(writer.layout(), layout);
            drop(writer);
        }
        for part in 0..4 {
            let expected = provider.next;
            let (next, writer, layout) = owner
                .create_quotient_writer(&mut provider, 2, part)
                .unwrap();
            owner = next;
            assert_eq!(layout.role(), StoredPolynomialRoleV1::QuotientNumerator);
            assert_eq!(
                layout.basis(),
                StoredPolynomialBasisV1::CosetPart {
                    extension_log: 2,
                    part
                }
            );
            assert_eq!(layout.ordinal(), expected);
            assert_eq!(owner.greatest_ordinal().unwrap(), Some(expected));
            assert_eq!(writer.layout(), layout);
            drop(writer);
        }
        assert_eq!(
            bank.reads.get(),
            reads,
            "factories consume no source scalar"
        );
        assert_eq!(owner.layouts().unwrap().collect::<Vec<_>>(), layouts);
        assert_eq!(
            owner
                .columns
                .iter()
                .map(|c| (c.blind.0).0)
                .collect::<Vec<_>>(),
            blinds
        );
        assert!(std::ptr::eq(owner.params().unwrap(), &params));
        assert_eq!(bank.live.get(), 5);
        assert_eq!(bank.writers.get(), 0);
        assert_eq!(take_blind_clear_observations(), (0, true));
        drop(owner);
        assert_eq!(take_blind_clear_observations(), (5, true));
        assert_eq!(bank.live.get(), 0);
    }
}

#[test]
fn both_pasta_quotient_bridge_copies_original_coefficients_preserves_phases_blinds_and_global_cursor()
 {
    bridge_success::<EpAffine>();
    bridge_success::<EqAffine>();
}

fn bridge_read_failures<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    for column in 0..5 {
        for fault_index in 0..8 {
            let (owner, mut provider) = coefficient_fixture(&params, false);
            let bank = Rc::clone(&provider.bank);
            let mut survivor = sentinel::<C>(&mut provider, 9);
            let ordinal = owner.columns[column].coefficient.layout.ordinal();
            let victim = owner.columns[(column + 3) % 5].coefficient.layout.ordinal();
            let fault = match fault_index {
                0 => ReadFault::Error,
                1 => ReadFault::Panic,
                2 => ReadFault::Short,
                3 => ReadFault::Long,
                4 => ReadFault::Encoding,
                5 => ReadFault::Drift(ordinal),
                6 => ReadFault::Drift(victim),
                7 => ReadFault::Error,
                _ => unreachable!(),
            };
            bank.read_fault.set(Some((ordinal, 1, fault)));
            if fault_index == 7 {
                bank.fault.set(Some(DropFault::Panic(ordinal)));
            }
            take_blind_clear_observations();
            let mut output = vec![C::Scalar::from(79); 256];
            let before = bank.reads.get();
            let result = catch_unwind(AssertUnwindSafe(|| {
                owner.copy_coefficient_chunk_into(column as u32, 1, &mut output)
            }));
            if fault_index == 1 || fault_index == 7 {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(bank.reads.get(), before + 1);
            assert!(output.iter().all(|v| *v == C::Scalar::ZERO));
            assert_eq!(take_blind_clear_observations(), (5, true));
            assert_eq!(bank.live.get(), 1);
            assert_eq!(bank.writers.get(), 0);
            assert!(!bank.busy.get());
            assert_sentinel::<C>(&mut survivor);
            drop(survivor);
            assert_eq!(bank.live.get(), 0);
        }
    }
    for case in 0..10 {
        let (mut owner, mut provider) = coefficient_fixture(&params, false);
        let bank = Rc::clone(&provider.bank);
        let mut survivor = sentinel::<C>(&mut provider, 9);
        let mut column = 0;
        let mut chunk = 0;
        let mut output = vec![C::Scalar::from(83); 256];
        match case {
            0 => column = 5,
            1 => chunk = 2,
            2 => {
                output.pop();
            }
            3 => output.push(C::Scalar::ONE),
            4 => phase_drift(&bank, owner.columns[4].coefficient.layout.ordinal()),
            5 => owner.challenges[3] = None,
            6 => owner.plan.phases[2].columns[0] = 4,
            7 => owner.source_greatest = None,
            8 => owner.greatest_ordinal = None,
            9 => bank
                .layout_panic
                .set(Some(owner.columns[4].coefficient.layout.ordinal())),
            _ => unreachable!(),
        }
        let before = bank.reads.get();
        take_blind_clear_observations();
        let result = catch_unwind(AssertUnwindSafe(|| {
            owner.copy_coefficient_chunk_into(column, chunk, &mut output)
        }));
        if case == 9 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(bank.reads.get(), before);
        assert!(output.iter().all(|v| *v == C::Scalar::ZERO));
        assert_eq!(take_blind_clear_observations(), (5, true));
        assert_eq!(bank.live.get(), 1);
        assert_sentinel::<C>(&mut survivor);
    }
}

#[test]
fn both_pasta_quotient_coefficient_read_decode_validation_and_drop_failures_destroy_owner_and_clear_output()
 {
    bridge_read_failures::<EpAffine>();
    bridge_read_failures::<EqAffine>();
}

fn bridge_factories<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for numerator in [false, true] {
        for case in 0..12 {
            let (owner, mut provider) = coefficient_fixture(&params, false);
            let bank = Rc::clone(&provider.bank);
            let mut survivor = sentinel::<C>(&mut provider, 4);
            let source = owner.columns[0].coefficient.layout;
            let fault = match case {
                0 => CreateFault::Error,
                1 => CreateFault::Panic,
                2 => CreateFault::Context,
                3 => CreateFault::Field,
                4 => CreateFault::Basis,
                5 => CreateFault::K,
                6 => CreateFault::Role,
                7 => CreateFault::Ordinal,
                8 => CreateFault::Exhausted,
                9 => CreateFault::SecondLayout,
                10 => CreateFault::Drift(owner.columns[4].coefficient.layout.ordinal()),
                11 => CreateFault::FinalAdviceSweep,
                _ => unreachable!(),
            };
            bank.create_fault.set(Some(fault));
            let reads = bank.reads.get();
            let creates = bank.creates.get();
            take_blind_clear_observations();
            let result = catch_unwind(AssertUnwindSafe(|| {
                if numerator {
                    owner.create_quotient_writer(&mut provider, 2, 1)
                } else {
                    owner.create_coset_writer(&mut provider, source, 2, 1)
                }
            }));
            if case == 1 {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(bank.creates.get(), creates + 1);
            if case == 11 {
                assert!(
                    bank.late_writer_drift.get(),
                    "retained advice callback changes the already checked new writer"
                );
                assert!(
                    bank.writer_checks.get() >= 3,
                    "new writer is checked again after the final advice sweep"
                );
            }
            assert_eq!(bank.reads.get(), reads);
            assert_eq!(bank.writers.get(), 0);
            assert_eq!(bank.live.get(), 1);
            assert_eq!(take_blind_clear_observations(), (5, true));
            assert_sentinel::<C>(&mut survivor);
        }
    }
    for case in 0..13 {
        let (owner, mut provider) = coefficient_fixture(&params, false);
        let bank = Rc::clone(&provider.bank);
        let mut source = owner.columns[0].coefficient.layout;
        let mut extension = 2;
        let mut part = 1;
        match case {
            0 => extension = 0,
            1 => extension = 16,
            2 => part = 4,
            3 => source.proof_context = [8; 32],
            4 => source.basis = StoredPolynomialBasisV1::Lagrange,
            5 => source.k = 5,
            6 => {
                source.field = match source.field {
                    StoredPastaFieldV1::Fp => StoredPastaFieldV1::Fq,
                    StoredPastaFieldV1::Fq => StoredPastaFieldV1::Fp,
                }
            }
            7 => source.ordinal = provider.next + 3,
            8 => {
                source.role = StoredPolynomialRoleV1::Advice {
                    column: 1,
                    phase: 1,
                }
            }
            9 => source.role = StoredPolynomialRoleV1::VanishingRandom,
            10 => source.role = StoredPolynomialRoleV1::QuotientNumerator,
            11 => {
                source.role = StoredPolynomialRoleV1::LookupCompressed {
                    lookup: 0,
                    side: crate::poly::stored_advice::StoredLookupSideV1::Input,
                }
            }
            12 => {
                source.role = StoredPolynomialRoleV1::Advice {
                    column: 0,
                    phase: 2,
                }
            }
            _ => unreachable!(),
        }
        let creates = bank.creates.get();
        let reads = bank.reads.get();
        take_blind_clear_observations();
        assert!(
            owner
                .create_coset_writer(&mut provider, source, extension, part)
                .is_err()
        );
        assert_eq!(bank.creates.get(), creates);
        assert_eq!(bank.reads.get(), reads);
        assert_eq!(bank.live.get(), 0);
        assert_eq!(bank.writers.get(), 0);
        assert_eq!(take_blind_clear_observations(), (5, true));
    }
}

#[test]
fn both_pasta_quotient_factories_reject_source_and_provider_substitution_without_reusable_prefix() {
    bridge_factories::<EpAffine>();
    bridge_factories::<EqAffine>();
}

fn bridge_empty_and_ordinal<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for empty in [false, true] {
        for near_end in [false, true] {
            let (owner, mut provider) = coefficient_fixture(&params, empty);
            let bank = Rc::clone(&provider.bank);
            assert_eq!(
                owner.proof_context().unwrap(),
                if empty { None } else { Some([9; 32]) }
            );
            assert_eq!(
                owner.quotient_ordinal_boundary(0).unwrap(),
                owner.greatest_ordinal().unwrap()
            );
            assert!(owner.quotient_ordinal_boundary(usize::MAX).is_err() || empty);
            if near_end {
                provider.next = u64::MAX - 1;
            }
            let expected = provider.next;
            let (owner, writer, layout) =
                owner.create_quotient_writer(&mut provider, 2, 0).unwrap();
            assert_eq!(layout.ordinal(), expected);
            assert_eq!(owner.proof_context().unwrap(), Some([9; 32]));
            assert_eq!(owner.greatest_ordinal().unwrap(), Some(expected));
            assert!(layout.same_proof_context(writer.layout()));
            assert_eq!(owner.columns.len(), if empty { 0 } else { 5 });
            if near_end {
                assert!(owner.quotient_ordinal_boundary(1).is_err());
            } else {
                assert_eq!(owner.quotient_ordinal_boundary(1).unwrap(), Some(expected));
            }
            drop(writer);
            drop(owner);
            assert_eq!(bank.live.get(), 0);
            assert_eq!(bank.writers.get(), 0);
        }
    }
    for outputs in [1, 2, 7] {
        let (mut owner, provider) = coefficient_fixture(&params, false);
        owner.greatest_ordinal = Some(u64::MAX - outputs as u64);
        let before = provider.bank.creates.get();
        assert!(owner.quotient_ordinal_boundary(outputs).is_err());
        assert_eq!(provider.bank.creates.get(), before);
        owner.greatest_ordinal = Some(u64::MAX - outputs as u64 - 1);
        assert_eq!(
            owner.quotient_ordinal_boundary(outputs).unwrap(),
            owner.greatest_ordinal().unwrap()
        );
    }
}

#[test]
fn both_pasta_quotient_zero_advice_first_context_and_last_legal_ordinal_remain_original_owner_bound()
 {
    bridge_empty_and_ordinal::<EpAffine>();
    bridge_empty_and_ordinal::<EqAffine>();
}
