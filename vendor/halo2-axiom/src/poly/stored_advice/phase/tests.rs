//! Seeded actual-IPA phase oracle and admission/failure tests.
//!
//! The recording backend intentionally keeps plaintext as an oracle. It proves neither spool
//! authentication nor secure scratch cleanup. Tests compare actual IPA points and transcript
//! bytes/challenges, not a replacement commitment algorithm or a complete proof.

use ff::FromUniformBytes;
use halo2curves::pasta::{EpAffine, EqAffine, Fp};
use rand_chacha::ChaCha20Rng;
use rand_core::{Error as RngError, SeedableRng};
use std::{
    cell::{Cell, RefCell},
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use super::*;
use crate::{
    plonk::{FirstPhase, SecondPhase, ThirdPhase},
    poly::commitment::ParamsProver,
    transcript::{Blake2bWrite, Challenge255, Transcript, TranscriptWriterBuffer},
};

mod completion;

thread_local! {
    // Observe initialized scalar storage immediately after the real SecretBlind destructor
    // wipes it; retain counts only, never the pre-wipe blind or a pointer to freed storage.
    static BLIND_DROPS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

/// Observe the initialized post-wipe blind slot from the parent module's real destructor.
pub(super) fn record_blind_drop<F: Field>(value: F) {
    BLIND_DROPS.with(|counts| {
        let (total, nonzero) = counts.get();
        counts.set((total + 1, nonzero + usize::from(value != F::ZERO)));
    });
}

#[derive(Default)]
struct Recording {
    fail_write: Option<(u32, u64)>,
    panic_write: Option<(u32, u64)>,
    fail_seal: Option<u32>,
    fail_read: Option<(u32, u64)>,
    panic_read: Option<(u32, u64)>,
    corrupt_read: Option<(u32, u64)>,
    short_read: Option<(u32, u64)>,
    change_after_read: Option<(u32, u64)>,
    writer_drops: usize,
    snapshot_drops: usize,
    read_count: usize,
    sealed: Vec<(StoredAdviceLayoutV1, Vec<[u8; 32]>)>,
    rng_draws: usize,
    seals_at_draw: Vec<usize>,
}

#[derive(Default)]
struct Backend {
    busy: Cell<bool>,
    record: RefCell<Recording>,
}
struct Window(Rc<Backend>);
impl Window {
    fn acquire(backend: &Rc<Backend>) -> Result<Self, StoredAdviceErrorV1> {
        if backend.busy.replace(true) {
            return Err(StoredAdviceErrorV1::Busy);
        }
        Ok(Self(Rc::clone(backend)))
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        self.0.busy.set(false);
    }
}

struct Writer {
    layout: StoredAdviceLayoutV1,
    backend: Rc<Backend>,
    values: Vec<[u8; 32]>,
    next: u64,
    layout_reads: Cell<usize>,
    change_ordinal_on_layout_read: Option<(usize, u64)>,
}
struct Snapshot {
    layout: StoredAdviceLayoutV1,
    backend: Rc<Backend>,
    values: Vec<[u8; 32]>,
    poisoned: bool,
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.backend.record.borrow_mut().writer_drops += 1;
    }
}
impl Drop for Snapshot {
    fn drop(&mut self) {
        self.backend.record.borrow_mut().snapshot_drops += 1;
    }
}

impl StoredAdviceWriterV1 for Writer {
    type Snapshot = Snapshot;
    fn layout(&self) -> StoredAdviceLayoutV1 {
        let read = self.layout_reads.get() + 1;
        self.layout_reads.set(read);
        let mut layout = self.layout;
        if let Some((change_at, ordinal)) = self.change_ordinal_on_layout_read {
            if read >= change_at {
                layout.ordinal = ordinal;
            }
        }
        layout
    }
    fn write_chunk(&mut self, chunk: u64, values: &[[u8; 32]]) -> Result<(), StoredAdviceErrorV1> {
        let _window = Window::acquire(&self.backend)?;
        let record = self.backend.record.borrow();
        let location = (self.layout.column(), chunk);
        assert_ne!(record.panic_write, Some(location), "injected write unwind");
        if record.fail_write == Some(location) {
            return Err(StoredAdviceErrorV1::Storage);
        }
        assert_eq!(self.next, chunk);
        assert_eq!(values.len(), self.layout.chunk_scalar_count(chunk).unwrap());
        assert!(
            values
                .iter()
                .all(|value| self.layout.field().is_canonical(value))
        );
        self.values.extend_from_slice(values);
        self.next += 1;
        Ok(())
    }
    fn seal(mut self) -> Result<Snapshot, StoredAdviceErrorV1> {
        let _window = Window::acquire(&self.backend)?;
        let mut record = self.backend.record.borrow_mut();
        if record.fail_seal == Some(self.layout.column()) {
            return Err(StoredAdviceErrorV1::Storage);
        }
        assert_eq!(self.next as usize, self.layout.chunk_count());
        let draws = record.rng_draws;
        record.seals_at_draw.push(draws);
        record.sealed.push((self.layout, self.values.clone()));
        Ok(Snapshot {
            layout: self.layout,
            backend: Rc::clone(&self.backend),
            values: std::mem::take(&mut self.values),
            poisoned: false,
        })
    }
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
        let _window = Window::acquire(&self.backend)?;
        self.poisoned = true;
        let location = (self.layout.column(), chunk);
        let (corrupt, short, change) = {
            let mut record = self.backend.record.borrow_mut();
            record.read_count += 1;
            assert_ne!(record.panic_read, Some(location), "injected read unwind");
            if record.fail_read == Some(location) {
                return Err(StoredAdviceErrorV1::Authentication);
            }
            (
                record.corrupt_read == Some(location),
                record.short_read == Some(location),
                record.change_after_read == Some(location),
            )
        };
        let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
        let count = self.layout.chunk_scalar_count(chunk)?;
        if corrupt {
            self.values[start] = [0xff; 32];
        }
        let value = consume(&self.values[start..start + count - usize::from(short)])?;
        if change {
            self.layout.column += 1;
        }
        self.poisoned = false;
        Ok(value)
    }
    fn with_column<R>(
        &mut self,
        _expected: StoredAdviceLayoutV1,
        _consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1> {
        panic!("phase commitment must not materialize an encoded whole column");
    }
}

struct CountingRng {
    inner: ChaCha20Rng,
    backend: Rc<Backend>,
}
impl CountingRng {
    fn new(backend: &Rc<Backend>) -> Self {
        Self {
            inner: ChaCha20Rng::from_seed([37; 32]),
            backend: Rc::clone(backend),
        }
    }
    fn draw(&self) {
        self.backend.record.borrow_mut().rng_draws += 1;
    }
}
impl RngCore for CountingRng {
    fn next_u32(&mut self) -> u32 {
        self.draw();
        self.inner.next_u32()
    }
    fn next_u64(&mut self) -> u64 {
        self.draw();
        self.inner.next_u64()
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.draw();
        self.inner.fill_bytes(dest);
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RngError> {
        self.draw();
        self.inner.try_fill_bytes(dest)
    }
}

struct CountingTranscript<C: CurveAffine>
where
    C::Scalar: FromUniformBytes<64>,
{
    inner: Blake2bWrite<Vec<u8>, C, Challenge255<C>>,
    writes: usize,
    squeezes: usize,
    fail_write: Option<usize>,
    events: Vec<bool>,
}
impl<C: CurveAffine> CountingTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn new() -> Self {
        Self {
            inner: Blake2bWrite::init(Vec::new()),
            writes: 0,
            squeezes: 0,
            fail_write: None,
            events: Vec::new(),
        }
    }
}
impl<C: CurveAffine> Transcript<C, Challenge255<C>> for CountingTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn squeeze_challenge(&mut self) -> Challenge255<C> {
        self.squeezes += 1;
        self.events.push(true);
        self.inner.squeeze_challenge()
    }
    fn common_point(&mut self, point: C) -> io::Result<()> {
        self.inner.common_point(point)
    }
    fn common_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.inner.common_scalar(scalar)
    }
}
impl<C: CurveAffine> TranscriptWrite<C, Challenge255<C>> for CountingTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn write_point(&mut self, point: C) -> io::Result<()> {
        let index = self.writes;
        self.writes += 1;
        self.events.push(false);
        if self.fail_write == Some(index) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.inner.write_point(point)
    }
    fn write_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.inner.write_scalar(scalar)
    }
}

fn writers<C: CurveAffine>(
    plan: &StoredPhasePlanV1<'_, C>,
    phase: usize,
    first_ordinal: u64,
    backend: &Rc<Backend>,
) -> Vec<Writer> {
    plan.phases[phase]
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| Writer {
            layout: StoredAdviceLayoutV1::new(
                [9; 32],
                first_ordinal + index as u64,
                plan.field,
                StoredPolynomialBasisV1::Lagrange,
                plan.k,
                *column as u32,
                phase as u8,
            )
            .unwrap(),
            backend: Rc::clone(backend),
            values: Vec::new(),
            next: 0,
            layout_reads: Cell::new(0),
            change_ordinal_on_layout_read: None,
        })
        .collect()
}

fn configured<F: Field>() -> ConstraintSystem<F> {
    let mut meta = ConstraintSystem::default();
    meta.advice_column();
    meta.advice_column();
    meta.challenge_usable_after(FirstPhase);
    meta.challenge_usable_after(FirstPhase);
    meta.advice_column_in(SecondPhase);
    meta.challenge_usable_after(SecondPhase);
    meta
}

fn phase_inputs<F: Field>(column: usize, usable: usize, prior: F) -> Vec<(usize, Assigned<F>)> {
    vec![
        (0, Assigned::Rational(F::from(6), F::from(2))),
        (2, Assigned::Rational(F::ONE, F::ZERO)),
        (
            usable - 1,
            Assigned::Trivial(F::from(column as u64 + 19) + prior),
        ),
    ]
}

fn actual_oracle<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    for k in [4, 9] {
        let params = ParamsIPA::<C>::new(k);
        let domain = EvaluationDomain::<C::Scalar>::new(3, k);
        let mut meta = configured::<C::Scalar>();
        meta.advice_column_in(ThirdPhase);
        meta.challenge_usable_after(ThirdPhase);
        let plan = admit_stored_phase_plan_v1(&params, &domain, &meta).unwrap();
        let backend = Rc::new(Backend::default());
        let mut rng = CountingRng::new(&backend);
        let mut oracle_rng = ChaCha20Rng::from_seed([37; 32]);
        let mut transcript = CountingTranscript::<C>::new();
        let mut oracle_transcript = Blake2bWrite::<Vec<u8>, C, Challenge255<C>>::init(Vec::new());
        // The existing caller owns and supplies this prefix before the phase owner writes.
        transcript.common_scalar(C::Scalar::from(101)).unwrap();
        oracle_transcript
            .common_scalar(C::Scalar::from(101))
            .unwrap();
        let usable = plan.usable_rows;
        let phase0 = writers(&plan, 0, 11, &backend);
        let mut phase1 = Some(writers(&plan, 1, 19, &backend));
        let mut phase2 = Some(writers(&plan, 2, 23, &backend));
        let mut owner = StoredPhaseAssignmentsV1::<C, Writer>::begin(plan, phase0).unwrap();
        let mut oracle_challenges = vec![None; 4];
        for phase in 0..3 {
            let column_indices: &[usize] = match phase {
                0 => &[0, 1],
                1 => &[2],
                _ => &[3],
            };
            let challenge_indices: &[usize] = match phase {
                0 => &[0, 1],
                1 => &[2],
                _ => &[3],
            };
            let prior = oracle_challenges[0].unwrap_or(C::Scalar::ZERO);
            let mut oracle_polys = Vec::new();
            for column in column_indices {
                let mut poly = domain.empty_lagrange();
                for (row, value) in phase_inputs(*column, usable, prior) {
                    owner.assign_discarding_value(*column, row, value).unwrap();
                    poly[row] = value.evaluate();
                }
                oracle_polys.push(poly);
            }
            // Exact existing oracle: every tail first, then every blind.
            for poly in &mut oracle_polys {
                for value in &mut poly[usable..] {
                    *value = C::Scalar::random(&mut oracle_rng);
                }
            }
            let oracle_blinds: Vec<_> = oracle_polys
                .iter()
                .map(|_| Blind(C::Scalar::random(&mut oracle_rng)))
                .collect();
            let oracle_projective: Vec<_> = oracle_polys
                .iter()
                .zip(&oracle_blinds)
                .map(|(poly, blind)| params.commit_lagrange(poly, *blind))
                .collect();
            let mut oracle_points = vec![C::identity(); oracle_projective.len()];
            C::Curve::batch_normalize(&oracle_projective, &mut oracle_points);
            let prepared = owner.finish(&mut rng).unwrap();
            assert_eq!(prepared.commitments, oracle_points);
            for ((column, poly), blind) in prepared
                .columns
                .iter()
                .zip(&oracle_polys)
                .zip(&oracle_blinds)
            {
                assert_eq!(column.blind.0, *blind);
                assert_eq!(
                    column.snapshot.values,
                    poly.iter().map(|x| x.to_repr()).collect::<Vec<_>>()
                );
            }
            let before = transcript.events.len();
            for point in &oracle_points {
                oracle_transcript.write_point(*point).unwrap();
            }
            for index in challenge_indices {
                oracle_challenges[*index] =
                    Some(*oracle_transcript.squeeze_challenge_scalar::<()>());
            }
            let committed = prepared.absorb(&mut transcript).unwrap();
            let expected_events: Vec<_> = std::iter::repeat_n(false, column_indices.len())
                .chain(std::iter::repeat_n(true, challenge_indices.len()))
                .collect();
            assert_eq!(&transcript.events[before..], expected_events);
            for index in 0..5 {
                assert_eq!(
                    committed.challenge(index),
                    oracle_challenges.get(index).copied().flatten()
                );
            }
            if phase < 2 {
                assert!(!committed.is_complete());
                // Later phases consume this exact cursor; they cannot reset or replay phase 0.
                let writers = if phase == 0 {
                    phase1.take().unwrap()
                } else {
                    phase2.take().unwrap()
                };
                owner = committed.begin_next(writers).unwrap();
            } else {
                assert!(committed.is_complete());
                assert_eq!(committed.session.columns.len(), 4);
                drop(committed);
                break;
            }
        }
        let mut next = [0; 64];
        let mut oracle_next = [0; 64];
        rng.fill_bytes(&mut next);
        oracle_rng.fill_bytes(&mut oracle_next);
        assert_eq!(next, oracle_next);
        assert_eq!(transcript.inner.finalize(), oracle_transcript.finalize());
        assert!(!backend.busy.get());
        assert_eq!(backend.record.borrow().snapshot_drops, 4);
        let record = backend.record.borrow();
        assert_eq!(record.sealed.len(), 4);
        assert!(
            record
                .seals_at_draw
                .windows(2)
                .all(|draws| draws[0] < draws[1])
        );
    }
}

#[test]
fn eq_fp_seeded_phase_points_blinds_rng_bytes_and_challenges_match_actual_ipa() {
    actual_oracle::<EqAffine>();
}
#[test]
fn ep_fq_seeded_phase_points_blinds_rng_bytes_and_challenges_match_actual_ipa() {
    actual_oracle::<EpAffine>();
}

fn simple_setup(params: &ParamsIPA<EqAffine>) -> (StoredPhasePlanV1<'_, EqAffine>, Rc<Backend>) {
    let domain = EvaluationDomain::<Fp>::new(3, 4);
    let meta = configured();
    let plan = admit_stored_phase_plan_v1(params, &domain, &meta).unwrap();
    (plan, Rc::new(Backend::default()))
}

#[test]
fn phase_transitions_retain_the_original_admitted_parameter_owner() {
    let params = ParamsIPA::<EqAffine>::new(4);
    let mut replacement = params.clone();
    replacement.g_lagrange.swap(0, 1);
    let (plan, backend) = simple_setup(&params);
    assert!(std::ptr::eq(plan.params, &params));
    assert!(!std::ptr::eq(plan.params, &replacement));
    let debug = format!("{plan:?}");
    assert!(debug.contains("StoredPhasePlanV1"));
    assert!(!debug.contains("g_lagrange"));
    assert!(debug.len() < 512);
    let first = writers(&plan, 0, 7, &backend);
    let second = writers(&plan, 1, 11, &backend);
    let owner = StoredPhaseAssignmentsV1::<EqAffine, Writer>::begin(plan, first).unwrap();
    let mut rng = CountingRng::new(&backend);
    let mut transcript = CountingTranscript::<EqAffine>::new();
    let prepared = owner.finish(&mut rng).unwrap();
    assert!(std::ptr::eq(prepared.session.plan.params, &params));
    let committed = prepared.absorb(&mut transcript).unwrap();
    assert!(std::ptr::eq(committed.session.plan.params, &params));
    let second = committed.begin_next(second).unwrap();
    let prepared = second.finish(&mut rng).unwrap();
    assert!(std::ptr::eq(prepared.session.plan.params, &params));
    let committed = prepared.absorb(&mut transcript).unwrap();
    assert!(committed.is_complete());
    assert!(std::ptr::eq(committed.session.plan.params, &params));
    // finish has no params argument: a same-k replacement cannot be supplied. The shared
    // borrow in this owner also prevents safe mutation of params until this owner is dropped.
}

#[test]
fn empty_phase_preserves_rng_transcript_and_cannot_repeat() {
    let params = ParamsIPA::<EqAffine>::new(4);
    let domain = EvaluationDomain::<Fp>::new(3, 4);
    let meta = ConstraintSystem::default();
    let plan = admit_stored_phase_plan_v1(&params, &domain, &meta).unwrap();
    assert_eq!(plan.phases.len(), 1);
    let backend = Rc::new(Backend::default());
    let mut rng = CountingRng::new(&backend);
    let mut transcript = CountingTranscript::<EqAffine>::new();
    let owner = StoredPhaseAssignmentsV1::<EqAffine, Writer>::begin(plan, vec![]).unwrap();
    let committed = owner
        .finish(&mut rng)
        .unwrap()
        .absorb(&mut transcript)
        .unwrap();
    assert!(committed.is_complete());
    assert_eq!(backend.record.borrow().rng_draws, 0);
    assert_eq!(transcript.writes, 0);
    assert_eq!(transcript.squeezes, 0);
    assert!(matches!(
        committed.begin_next::<Writer>(vec![]),
        Err(StoredPhaseErrorV1::Admission)
    ));
}

#[test]
fn cs_admission_rejects_malformed_maps_and_wrong_parameters_without_rng() {
    let params = ParamsIPA::<EqAffine>::new(4);
    let domain = EvaluationDomain::<Fp>::new(3, 4);
    let meta = configured::<Fp>();
    let mut malformed = meta.clone();
    malformed.num_advice_columns += 1;
    assert!(matches!(
        admit_stored_phase_plan_v1(&params, &domain, &malformed),
        Err(StoredPhaseErrorV1::Admission)
    ));
    let mut malformed = meta.clone();
    malformed.num_challenges += 1;
    assert!(matches!(
        admit_stored_phase_plan_v1(&params, &domain, &malformed),
        Err(StoredPhaseErrorV1::Admission)
    ));
    let wrong_domain = EvaluationDomain::<Fp>::new(3, 3);
    assert!(matches!(
        admit_stored_phase_plan_v1(&params, &wrong_domain, &meta),
        Err(StoredPhaseErrorV1::Admission)
    ));
    let mut truncated = params.clone();
    truncated.g_lagrange.pop();
    assert!(matches!(
        admit_stored_phase_plan_v1(&truncated, &domain, &meta),
        Err(StoredPhaseErrorV1::Admission)
    ));
    let tiny = ParamsIPA::<EqAffine>::new(2);
    let tiny_domain = EvaluationDomain::<Fp>::new(3, 2);
    assert!(matches!(
        admit_stored_phase_plan_v1(&tiny, &tiny_domain, &meta),
        Err(StoredPhaseErrorV1::Admission)
    ));
}

#[test]
fn writer_admission_rejects_duplicates_omissions_reordering_and_wrong_coordinates() {
    for case in 0..11 {
        let params = ParamsIPA::<EqAffine>::new(4);
        let (plan, backend) = simple_setup(&params);
        let mut input = writers(&plan, 0, 7, &backend);
        match case {
            0 => {
                input.pop();
            }
            1 => input[1].layout.column = input[0].layout.column,
            2 => input.swap(0, 1),
            3 => input[1].layout.ordinal = input[0].layout.ordinal,
            4 => input[1].layout.proof_context = [10; 32],
            5 => input[0].layout.field = StoredPastaFieldV1::Fq,
            6 => input[0].layout.k += 1,
            7 => input[0].layout.phase = 1,
            8 => input[0].layout.basis = StoredPolynomialBasisV1::Coefficient,
            9 => input[1].layout.ordinal = input[0].layout.ordinal - 1,
            _ => input[0].layout.proof_context = [0; 32],
        }
        assert!(matches!(
            StoredPhaseAssignmentsV1::<EqAffine, Writer>::begin(plan, input),
            Err(StoredPhaseErrorV1::Admission)
        ));
        assert_eq!(backend.record.borrow().rng_draws, 0);
        assert_eq!(backend.record.borrow().writer_drops, 2);
    }
}

#[test]
fn writer_layout_substitution_during_admission_drops_every_writer() {
    for changed_read in [2, 3] {
        let params = ParamsIPA::<EqAffine>::new(4);
        let (plan, backend) = simple_setup(&params);
        let mut input = writers(&plan, 0, 7, &backend);
        input[1].change_ordinal_on_layout_read = Some((changed_read, input[0].layout.ordinal()));
        assert!(matches!(
            StoredPhaseAssignmentsV1::<EqAffine, Writer>::begin(plan, input),
            Err(StoredPhaseErrorV1::Admission)
                | Err(StoredPhaseErrorV1::Assignment(
                    StoredAssignmentErrorV1::Store(StoredAdviceErrorV1::Context)
                ))
        ));
        assert_eq!(backend.record.borrow().writer_drops, 2);
        assert_eq!(backend.record.borrow().rng_draws, 0);
        assert!(backend.record.borrow().sealed.is_empty());
    }
}

#[test]
fn cs_admission_rejects_phase_gaps_and_challenges_without_advice() {
    let params = ParamsIPA::<EqAffine>::new(4);
    let domain = EvaluationDomain::<Fp>::new(3, 4);
    let mut meta = ConstraintSystem::default();
    meta.advice_column();
    meta.advice_column_in(SecondPhase);
    meta.advice_column_in(ThirdPhase);
    for skipped in [0, 1] {
        let mut malformed = meta.clone();
        malformed.advice_column_phase[skipped] = malformed.advice_column_phase[2];
        assert!(matches!(
            admit_stored_phase_plan_v1(&params, &domain, &malformed),
            Err(StoredPhaseErrorV1::Admission)
        ));
    }
    let mut malformed = configured::<Fp>();
    malformed.challenge_phase[0] = meta.advice_column_phase[2];
    assert!(matches!(
        admit_stored_phase_plan_v1(&params, &domain, &malformed),
        Err(StoredPhaseErrorV1::Admission)
    ));
}

#[test]
fn refused_assignments_poison_the_whole_owner_even_if_caller_ignores_error() {
    for case in 0..6 {
        let params = ParamsIPA::<EqAffine>::new(4);
        let (plan, backend) = simple_setup(&params);
        let usable = plan.usable_rows;
        let input = writers(&plan, 0, 7, &backend);
        let mut owner = StoredPhaseAssignmentsV1::<EqAffine, Writer>::begin(plan, input).unwrap();
        owner
            .assign_discarding_value(0, 2, Assigned::Trivial(Fp::ONE))
            .unwrap();
        let error = match case {
            0 => owner.assign_discarding_value(0, 2, Assigned::Zero),
            1 => owner.assign_discarding_value(0, 1, Assigned::Zero),
            2 => owner.assign_discarding_value(0, usable, Assigned::Zero),
            3 => owner.assign_discarding_value(2, 0, Assigned::Zero),
            4 => owner.reject_reference_return(),
            _ => owner.reject_unknown_value(),
        };
        assert!(error.is_err());
        assert_eq!(
            owner.assign_discarding_value(1, 0, Assigned::Zero),
            Err(StoredPhaseErrorV1::Poisoned)
        );
        assert_eq!(
            owner.reject_reference_return(),
            Err(StoredPhaseErrorV1::Poisoned)
        );
        assert_eq!(
            owner.reject_unknown_value(),
            Err(StoredPhaseErrorV1::Poisoned)
        );
        let mut rng = CountingRng::new(&backend);
        assert!(matches!(
            owner.finish(&mut rng),
            Err(StoredPhaseErrorV1::Poisoned)
        ));
        assert_eq!(backend.record.borrow().rng_draws, 0);
        assert_eq!(backend.record.borrow().writer_drops, 2);
    }
}

#[test]
fn storage_and_canonical_failures_drop_every_phase_handle_before_challenge() {
    for stage in 0..5 {
        let params = ParamsIPA::<EqAffine>::new(4);
        let (plan, backend) = simple_setup(&params);
        let input = writers(&plan, 0, 7, &backend);
        {
            let mut record = backend.record.borrow_mut();
            match stage {
                0 => record.fail_write = Some((1, 0)),
                1 => record.fail_seal = Some(1),
                2 => record.fail_read = Some((1, 0)),
                3 => record.corrupt_read = Some((1, 0)),
                _ => record.change_after_read = Some((1, 0)),
            }
        }
        let owner = StoredPhaseAssignmentsV1::<EqAffine, Writer>::begin(plan, input).unwrap();
        let mut rng = CountingRng::new(&backend);
        assert!(owner.finish(&mut rng).is_err());
        assert_eq!(backend.record.borrow().writer_drops, 2);
        assert_eq!(
            backend.record.borrow().snapshot_drops,
            if stage <= 1 { 1 } else { 2 }
        );
        assert!(!backend.busy.get());
    }
}

#[test]
fn caught_assignment_write_unwind_leaves_the_complete_owner_poisoned() {
    let params = ParamsIPA::<EqAffine>::new(9);
    let domain = EvaluationDomain::<Fp>::new(3, 9);
    let meta = configured::<Fp>();
    let plan = admit_stored_phase_plan_v1(&params, &domain, &meta).unwrap();
    let backend = Rc::new(Backend::default());
    let input = writers(&plan, 0, 7, &backend);
    backend.record.borrow_mut().panic_write = Some((1, 0));
    let mut owner = StoredPhaseAssignmentsV1::<EqAffine, Writer>::begin(plan, input).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            owner.assign_discarding_value(1, STORED_SCALARS_PER_CHUNK_V1 - 1, Assigned::Zero)
        }))
        .is_err()
    );
    assert_eq!(
        owner.assign_discarding_value(0, 0, Assigned::Zero),
        Err(StoredPhaseErrorV1::Poisoned)
    );
    let mut rng = CountingRng::new(&backend);
    assert!(matches!(
        owner.finish(&mut rng),
        Err(StoredPhaseErrorV1::Poisoned)
    ));
    assert_eq!(backend.record.borrow().writer_drops, 2);
    assert_eq!(backend.record.borrow().rng_draws, 0);
    assert!(backend.record.borrow().sealed.is_empty());
    assert!(!backend.busy.get());
}

#[test]
fn write_and_read_unwind_destroy_the_active_owner() {
    for writing in [true, false] {
        let params = ParamsIPA::<EqAffine>::new(4);
        let (plan, backend) = simple_setup(&params);
        let input = writers(&plan, 0, 7, &backend);
        if writing {
            backend.record.borrow_mut().panic_write = Some((1, 0));
        } else {
            backend.record.borrow_mut().panic_read = Some((1, 0));
        }
        let owner = StoredPhaseAssignmentsV1::<EqAffine, Writer>::begin(plan, input).unwrap();
        let mut rng = CountingRng::new(&backend);
        assert!(catch_unwind(AssertUnwindSafe(|| owner.finish(&mut rng))).is_err());
        assert!(!backend.busy.get());
        assert_eq!(backend.record.borrow().writer_drops, 2);
        assert_eq!(
            backend.record.borrow().snapshot_drops,
            if writing { 1 } else { 2 }
        );
    }
}

#[test]
fn transcript_failure_at_any_point_never_squeezes_a_phase_challenge() {
    for fail in [0, 1] {
        let params = ParamsIPA::<EqAffine>::new(4);
        let (plan, backend) = simple_setup(&params);
        let input = writers(&plan, 0, 7, &backend);
        let owner = StoredPhaseAssignmentsV1::<EqAffine, Writer>::begin(plan, input).unwrap();
        let mut rng = CountingRng::new(&backend);
        let mut transcript = CountingTranscript::<EqAffine>::new();
        transcript.fail_write = Some(fail);
        let prepared = owner.finish(&mut rng).unwrap();
        assert!(matches!(
            prepared.absorb(&mut transcript),
            Err(StoredPhaseErrorV1::Transcript)
        ));
        assert_eq!(transcript.writes, fail + 1);
        assert_eq!(transcript.squeezes, 0);
        assert_eq!(backend.record.borrow().snapshot_drops, 2);
    }
}

#[test]
fn later_phase_error_destroys_previous_receipts_and_rejects_ordinal_reuse() {
    for malformed in [true, false] {
        let params = ParamsIPA::<EqAffine>::new(4);
        let (plan, backend) = simple_setup(&params);
        let first = writers(&plan, 0, 7, &backend);
        let second = writers(&plan, 1, if malformed { 8 } else { 11 }, &backend);
        let owner = StoredPhaseAssignmentsV1::<EqAffine, Writer>::begin(plan, first).unwrap();
        let mut rng = CountingRng::new(&backend);
        let mut transcript = CountingTranscript::<EqAffine>::new();
        let committed = owner
            .finish(&mut rng)
            .unwrap()
            .absorb(&mut transcript)
            .unwrap();
        assert_eq!(transcript.squeezes, 2);
        if malformed {
            assert!(matches!(
                committed.begin_next(second),
                Err(StoredPhaseErrorV1::Admission)
            ));
        } else {
            let mut second = committed.begin_next(second).unwrap();
            assert!(second.reject_unknown_value().is_err());
            assert!(matches!(
                second.finish(&mut rng),
                Err(StoredPhaseErrorV1::Poisoned)
            ));
        }
        assert_eq!(transcript.squeezes, 2);
        assert_eq!(backend.record.borrow().snapshot_drops, 2);
        assert_eq!(backend.record.borrow().writer_drops, 3);
    }
}

#[test]
fn prepared_metadata_substitution_is_rejected_before_transcript_writes() {
    let params = ParamsIPA::<EqAffine>::new(4);
    let (plan, backend) = simple_setup(&params);
    let input = writers(&plan, 0, 7, &backend);
    let owner = StoredPhaseAssignmentsV1::<EqAffine, Writer>::begin(plan, input).unwrap();
    let mut rng = CountingRng::new(&backend);
    let mut transcript = CountingTranscript::<EqAffine>::new();
    let mut prepared = owner.finish(&mut rng).unwrap();
    prepared.columns[1].snapshot.layout.column += 1;
    assert!(matches!(
        prepared.absorb(&mut transcript),
        Err(StoredPhaseErrorV1::Admission)
    ));
    assert_eq!(transcript.writes, 0);
    assert_eq!(transcript.squeezes, 0);
    assert_eq!(backend.record.borrow().snapshot_drops, 2);
}

#[test]
fn guarded_polynomial_clear_preserves_allocation_and_clears_every_slot() {
    let mut polynomial = GuardedPolynomial::<Fp>::zeroed(513).unwrap();
    let pointer = polynomial.0.values.as_ptr();
    polynomial.0.values.fill(Fp::ONE);
    polynomial.clear();
    assert_eq!(polynomial.0.values.as_ptr(), pointer);
    assert!(polynomial.0.values.iter().all(|value| *value == Fp::ZERO));
}

#[path = "synthesis_tests.rs"]
mod synthesis;
