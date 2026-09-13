//! Consuming coefficient staging against ordinary FFTs and original phase receipt owners.
//!
//! These plaintext recording fixtures test ownership and arithmetic only. They establish no
//! encrypted-backend, complete-proof, physical-device, process-memory or performance claim.

use super::*;
use crate::poly::stored_advice::StoredPolynomialProviderV1;

struct Provider {
    backend: Rc<Backend>,
    next: u64,
    forced: Option<(u32, u64)>,
    wrong_context: Option<u32>,
    wrong_role: Option<(u32, StoredPolynomialRoleV1)>,
    fail_create: Option<u32>,
    writer_drift: Option<(u32, usize, u64)>,
    created: Vec<StoredPolynomialLayoutV1>,
    reads_at_create: Vec<usize>,
}

impl Provider {
    fn new(backend: &Rc<Backend>, next: u64) -> Self {
        Self {
            backend: Rc::clone(backend),
            next,
            forced: None,
            wrong_context: None,
            wrong_role: None,
            fail_create: None,
            writer_drift: None,
            created: Vec::new(),
            reads_at_create: Vec::new(),
        }
    }
}

impl StoredPolynomialProviderV1 for Provider {
    type Writer = Writer;

    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Writer, StoredPolynomialErrorV1> {
        let StoredPolynomialRoleV1::Advice { column, .. } = role else {
            return Err(StoredPolynomialErrorV1::Context);
        };
        assert!(
            !self.backend.busy.get(),
            "provider creation inside a plaintext callback"
        );
        self.reads_at_create
            .push(self.backend.record.borrow().read_count);
        if self.fail_create == Some(column) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        let ordinal = self
            .forced
            .filter(|(at, _)| *at == column)
            .map_or(self.next, |(_, ordinal)| ordinal);
        self.next += 7;
        let context = if self.wrong_context == Some(column) {
            [8; 32]
        } else {
            [9; 32]
        };
        let role = self
            .wrong_role
            .filter(|(at, _)| *at == column)
            .map_or(role, |(_, replacement)| replacement);
        let layout = StoredPolynomialLayoutV1::new(context, ordinal, field, basis, k, role)?;
        self.created.push(layout);
        Ok(Writer {
            layout,
            backend: Rc::clone(&self.backend),
            values: Vec::new(),
            next: 0,
            layout_reads: Cell::new(0),
            change_ordinal_on_layout_read: self
                .writer_drift
                .filter(|(at, _, _)| *at == column)
                .map(|(_, read, ordinal)| (read, ordinal)),
        })
    }
}

fn cleanup(backend: &Rc<Backend>, outputs: usize, writers: usize) {
    let record = backend.record.borrow();
    assert_eq!(record.snapshot_drops, 5 + outputs);
    assert_eq!(record.writer_drops, 5 + writers);
    assert_eq!(record.sealed.len(), 5 + outputs);
    BLIND_DROPS.with(|counts| assert_eq!(counts.get(), (5, 0)));
    assert!(!backend.busy.get());
}

fn coefficient_oracle<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    for k in [4, 9] {
        reset_blind_drops();
        let params = ParamsIPA::<C>::new(k);
        let domain = EvaluationDomain::<C::Scalar>::new(3, k);
        let backend = Rc::new(Backend::default());
        let (committed, mut rng, transcript) = absorbed_through(&params, &backend, 2);
        let complete = committed.into_complete().unwrap();
        let original = complete.session.as_ref().unwrap();
        let allocation = original.columns.as_ptr();
        let capacity = original.columns.capacity();
        let highwater = original.greatest_ordinal.unwrap();
        let expected: Vec<_> = original
            .columns
            .iter()
            .map(|column| {
                let decoded = column
                    .snapshot
                    .values
                    .iter()
                    .map(|bytes| Option::<C::Scalar>::from(C::Scalar::from_repr(*bytes)).unwrap())
                    .collect();
                let coefficients = domain.lagrange_to_coeff(domain.lagrange_from_vec(decoded));
                (
                    column.layout,
                    column.snapshot.values.as_ptr(),
                    column.snapshot.values.clone(),
                    column.blind.0.0,
                    coefficients
                        .iter()
                        .map(PrimeField::to_repr)
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let challenges = complete.challenges().unwrap().collect::<Vec<_>>();
        let reads = backend.record.borrow().read_count;
        let draws = backend.record.borrow().rng_draws;
        let transcript_state = (
            transcript.writes,
            transcript.squeezes,
            transcript.events.clone(),
            transcript.inner.clone().finalize(),
        );
        let mut next_rng = rng.inner.clone();
        let mut provider = Provider::new(&backend, highwater + 1);
        let staged = complete.stage_coefficients(&domain, &mut provider).unwrap();
        assert!(std::ptr::eq(staged.lagrange.params().unwrap(), &params));
        assert_eq!(staged.lagrange.proof_context().unwrap(), Some([9; 32]));
        assert_eq!(
            staged.lagrange.challenges().unwrap().collect::<Vec<_>>(),
            challenges
        );
        let retained = staged.lagrange.session.as_ref().unwrap();
        assert_eq!(retained.columns.as_ptr(), allocation);
        assert_eq!(retained.columns.capacity(), capacity);
        assert_eq!(retained.greatest_ordinal, Some(highwater));
        assert_eq!(staged.coefficients.len(), 5);
        assert_eq!(provider.created.len(), 5);
        let chunks = (1_usize << k).div_ceil(STORED_SCALARS_PER_CHUNK_V1);
        for (index, ((source, output), expected)) in retained
            .columns
            .iter()
            .zip(&staged.coefficients)
            .zip(&expected)
            .enumerate()
        {
            assert_eq!(source.layout, expected.0);
            assert_eq!(source.snapshot.layout, expected.0);
            assert_eq!(source.snapshot.values.as_ptr(), expected.1);
            assert_eq!(source.snapshot.values, expected.2);
            assert_eq!(source.blind.0.0, expected.3);
            assert!(!source.snapshot.poisoned);
            assert_eq!(output.layout, provider.created[index]);
            assert_eq!(output.snapshot.layout, output.layout);
            assert_eq!(output.snapshot.values, expected.4);
            assert!(!output.snapshot.poisoned);
            assert_eq!(output.layout.advice_coordinates().unwrap().0, index as u32);
            assert_eq!(
                output.layout.advice_coordinates().unwrap().1,
                expected.0.advice_coordinates().unwrap().1
            );
            assert_eq!(output.layout.role(), expected.0.role());
            assert_eq!(output.layout.field(), C::Scalar::STORED_FIELD);
            assert_eq!(output.layout.k(), k);
            assert_eq!(output.layout.proof_context, [9; 32]);
            assert_eq!(output.layout.basis(), StoredPolynomialBasisV1::Coefficient);
            assert_eq!(output.layout.ordinal(), highwater + 1 + index as u64 * 7);
            assert_eq!(provider.reads_at_create[index], reads + index * chunks);
        }
        assert_eq!(
            staged.greatest_ordinal,
            provider.created.last().map(|layout| layout.ordinal())
        );
        assert_eq!(backend.record.borrow().read_count, reads + 5 * chunks);
        assert_eq!(backend.record.borrow().rng_draws, draws);
        assert_eq!(
            (
                transcript.writes,
                transcript.squeezes,
                transcript.events.clone(),
                transcript.inner.clone().finalize()
            ),
            transcript_state
        );
        BLIND_DROPS.with(|counts| assert_eq!(counts.get(), (0, 0)));
        assert_eq!(backend.record.borrow().snapshot_drops, 0);
        let mut actual_next = [0; 64];
        let mut expected_next = [0; 64];
        rng.fill_bytes(&mut actual_next);
        next_rng.fill_bytes(&mut expected_next);
        assert_eq!(actual_next, expected_next);
        drop(staged);
        cleanup(&backend, 5, 5);
    }
}

#[test]
fn eq_fp_coefficients_match_ordinary_fft_and_preserve_exact_original_receipts() {
    coefficient_oracle::<EqAffine>();
}

#[test]
fn ep_fq_coefficients_match_ordinary_fft_and_preserve_exact_original_receipts() {
    coefficient_oracle::<EpAffine>();
}

fn empty_coefficients<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    reset_blind_drops();
    let params = ParamsIPA::<C>::new(4);
    let domain = EvaluationDomain::<C::Scalar>::new(3, 4);
    let plan = admit_stored_phase_plan_v1(&params, &domain, &ConstraintSystem::default()).unwrap();
    let backend = Rc::new(Backend::default());
    let mut rng = CountingRng::new(&backend);
    let mut transcript = CountingTranscript::<C>::new();
    let complete = StoredPhaseAssignmentsV1::<C, Writer>::begin(plan, vec![])
        .unwrap()
        .finish(&mut rng)
        .unwrap()
        .absorb(&mut transcript)
        .unwrap()
        .into_complete()
        .unwrap();
    let mut provider = Provider::new(&backend, 100);
    provider.fail_create = Some(0);
    let staged = complete.stage_coefficients(&domain, &mut provider).unwrap();
    assert!(staged.coefficients.is_empty());
    assert_eq!(staged.greatest_ordinal, None);
    assert_eq!(staged.lagrange.proof_context().unwrap(), None);
    assert_eq!(staged.lagrange.layouts().unwrap().len(), 0);
    assert_eq!(staged.lagrange.challenges().unwrap().count(), 0);
    assert!(std::ptr::eq(staged.lagrange.params().unwrap(), &params));
    assert!(provider.created.is_empty());
    assert!(provider.reads_at_create.is_empty());
    assert_eq!(backend.record.borrow().rng_draws, 0);
    assert_eq!(backend.record.borrow().read_count, 0);
    assert_eq!((transcript.writes, transcript.squeezes), (0, 0));
    drop(staged);
    assert_dropped(&backend, 0);
    assert_eq!(backend.record.borrow().writer_drops, 0);
}

fn malformed_empty<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let domain = EvaluationDomain::<C::Scalar>::new(3, 4);
    for fault in 0..6 {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let plan =
            admit_stored_phase_plan_v1(&params, &domain, &ConstraintSystem::default()).unwrap();
        let mut rng = CountingRng::new(&backend);
        let mut transcript = CountingTranscript::<C>::new();
        let mut complete = StoredPhaseAssignmentsV1::<C, Writer>::begin(plan, vec![])
            .unwrap()
            .finish(&mut rng)
            .unwrap()
            .absorb(&mut transcript)
            .unwrap()
            .into_complete()
            .unwrap();
        let session = complete.session.as_mut().unwrap();
        match fault {
            0 => session.proof_context = Some([9; 32]),
            1 => session.greatest_ordinal = Some(0),
            2 => session.plan.k = 5,
            3 => session.plan.challenges = 1,
            4 => session.next_phase = 0,
            5 => {
                session.plan.challenges = 1;
                session.plan.phases[0].challenges.push(0);
                session.challenges.push(Some(C::Scalar::ONE));
            }
            _ => unreachable!(),
        }
        let mut provider = Provider::new(&backend, 100);
        let result = complete
            .stage_coefficients(&domain, &mut provider)
            .map(|_| ());
        assert_eq!(result, Err(StoredPhaseErrorV1::Admission));
        assert!(provider.created.is_empty());
        assert!(provider.reads_at_create.is_empty());
        assert_eq!(backend.record.borrow().read_count, 0);
        assert_eq!(backend.record.borrow().rng_draws, 0);
        assert_eq!((transcript.writes, transcript.squeezes), (0, 0));
        assert_dropped(&backend, 0);
        assert_eq!(backend.record.borrow().writer_drops, 0);
    }
}

#[test]
fn both_fields_empty_coefficient_stage_preserves_no_context_and_uses_no_provider() {
    empty_coefficients::<EqAffine>();
    empty_coefficients::<EpAffine>();
    malformed_empty::<EqAffine>();
    malformed_empty::<EpAffine>();
}

fn malformed_sources<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for fault in 0..10 {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (committed, _rng, transcript) = absorbed_through(&params, &backend, 2);
        let mut complete = committed.into_complete().unwrap();
        let session = complete.session.as_mut().unwrap();
        let highwater = session.greatest_ordinal.unwrap();
        match fault {
            0 => (), // Wrong domain degree.
            1 => session.columns[4].snapshot.layout.ordinal += 1,
            2 => session.columns[4].layout.proof_context = [8; 32],
            3 => set_phase(&mut session.columns[4].layout, 0),
            4 => session.columns[4].layout.basis = StoredPolynomialBasisV1::Coefficient,
            5 => set_column(&mut session.columns[4].layout, 0),
            6 => session.greatest_ordinal = Some(highwater - 1),
            7 => session.plan.phases[1].challenges[0] = 1, // Cross-phase duplicate, count unchanged.
            8 => session.plan.phases[0].challenges.swap(0, 1),
            9 => session.plan.phases[1].columns.clear(),
            _ => unreachable!(),
        }
        let domain = EvaluationDomain::<C::Scalar>::new(3, if fault == 0 { 5 } else { 4 });
        let mut provider = Provider::new(&backend, highwater + 1);
        let reads = backend.record.borrow().read_count;
        let draws = backend.record.borrow().rng_draws;
        let transcript_bytes = transcript.inner.clone().finalize();
        assert!(complete.stage_coefficients(&domain, &mut provider).is_err());
        assert!(provider.created.is_empty());
        assert!(provider.reads_at_create.is_empty());
        assert_eq!(backend.record.borrow().read_count, reads);
        assert_eq!(backend.record.borrow().rng_draws, draws);
        assert_eq!(transcript.inner.clone().finalize(), transcript_bytes);
        cleanup(&backend, 0, 0);
    }
}

#[test]
fn both_fields_coefficient_preflights_reject_domain_and_late_source_identity_without_io() {
    malformed_sources::<EqAffine>();
    malformed_sources::<EpAffine>();
}

fn rejected_ordinals<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let domain = EvaluationDomain::<C::Scalar>::new(3, 9);
    // Each bad ordinal is greater than that source's ordinal. Checking only the
    // source/destination pair would therefore accept it and lose global uniqueness.
    for (column, ordinal) in [(0, 12), (0, 37), (1, 37), (1, 38)] {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (committed, _rng, _transcript) = absorbed_through(&params, &backend, 2);
        let complete = committed.into_complete().unwrap();
        assert_eq!(
            complete.session.as_ref().unwrap().greatest_ordinal,
            Some(37)
        );
        assert!(
            ordinal
                > complete.session.as_ref().unwrap().columns[column as usize]
                    .layout
                    .ordinal()
        );
        let reads = backend.record.borrow().read_count;
        let mut provider = Provider::new(&backend, 38);
        provider.forced = Some((column, ordinal));
        assert!(complete.stage_coefficients(&domain, &mut provider).is_err());
        assert_eq!(provider.created.len(), column as usize + 1);
        assert_eq!(
            backend.record.borrow().read_count,
            reads + column as usize * 2
        );
        cleanup(&backend, column as usize, column as usize + 1);
    }
}

fn drifting_writer<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let domain = EvaluationDomain::<C::Scalar>::new(3, 9);
    for (column, changed_ordinal) in [(0, 37), (1, 38)] {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (committed, _rng, _transcript) = absorbed_through(&params, &backend, 2);
        let complete = committed.into_complete().unwrap();
        let reads = backend.record.borrow().read_count;
        let mut provider = Provider::new(&backend, 38);
        // First layout read admits a fresh global ordinal. The converter's next observation
        // reports a different ordinal still greater than this source, but globally reused.
        provider.writer_drift = Some((column, 2, changed_ordinal));
        let result = complete
            .stage_coefficients(&domain, &mut provider)
            .map(|_| ());
        assert_eq!(
            result,
            Err(StoredPhaseErrorV1::Store(StoredPolynomialErrorV1::Context))
        );
        assert_eq!(provider.created.len(), column as usize + 1);
        assert_eq!(
            backend.record.borrow().read_count,
            reads + column as usize * 2
        );
        cleanup(&backend, column as usize, column as usize + 1);
    }
}

fn exhausted_ordinals<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let domain = EvaluationDomain::<C::Scalar>::new(3, 9);
    for source_already_max in [false, true] {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (committed, _rng, _transcript) = absorbed_through(&params, &backend, 2);
        let mut complete = committed.into_complete().unwrap();
        if source_already_max {
            let session = complete.session.as_mut().unwrap();
            // Final phase/global column3 is the actual last source ordinal, not column4.
            session.columns[3].layout.ordinal = u64::MAX;
            session.columns[3].snapshot.layout.ordinal = u64::MAX;
            session.greatest_ordinal = Some(u64::MAX);
        }
        let reads = backend.record.borrow().read_count;
        let mut provider = Provider::new(&backend, 38);
        // Force MAX without incrementing a MAX counter in this fixture. Either it equals the
        // source maximum immediately, or it becomes the maximum and the next output refuses.
        provider.forced = Some((0, u64::MAX));
        let result = complete
            .stage_coefficients(&domain, &mut provider)
            .map(|_| ());
        assert_eq!(
            result,
            Err(StoredPhaseErrorV1::Store(StoredPolynomialErrorV1::Context))
        );
        let outputs = if source_already_max { 0 } else { 1 };
        assert_eq!(provider.created.len(), outputs + 1);
        assert_eq!(backend.record.borrow().read_count, reads + outputs * 2);
        cleanup(&backend, outputs, outputs + 1);
    }
}

#[test]
fn both_fields_coefficient_destinations_must_exceed_all_original_and_prior_output_ordinals() {
    rejected_ordinals::<EqAffine>();
    rejected_ordinals::<EpAffine>();
    exhausted_ordinals::<EqAffine>();
    exhausted_ordinals::<EpAffine>();
    drifting_writer::<EqAffine>();
    drifting_writer::<EpAffine>();
}

fn partial_failures<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let domain = EvaluationDomain::<C::Scalar>::new(3, 9);
    for fault in 0..10 {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (committed, _rng, transcript) = absorbed_through(&params, &backend, 2);
        let complete = committed.into_complete().unwrap();
        let mut provider = Provider::new(&backend, 38);
        let reads = backend.record.borrow().read_count;
        let draws = backend.record.borrow().rng_draws;
        let transcript_bytes = transcript.inner.clone().finalize();
        // Fail global column 2 only after complete outputs for columns 0 and 1.
        {
            let mut record = backend.record.borrow_mut();
            match fault {
                0 => provider.fail_create = Some(2),
                1 => provider.wrong_context = Some(2),
                2 => record.fail_write = Some((2, 1)),
                3 => record.fail_seal = Some(2),
                4 => record.fail_read = Some((2, 1)),
                5 => record.panic_read = Some((2, 1)),
                6 => record.panic_write = Some((2, 1)),
                7 => record.corrupt_read = Some((2, 1)),
                8 => record.short_read = Some((2, 1)),
                9 => record.change_after_read = Some((2, 1)),
                _ => unreachable!(),
            }
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            complete.stage_coefficients(&domain, &mut provider)
        }));
        if fault == 5 || fault == 6 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        let created = if fault == 0 { 2 } else { 3 };
        assert_eq!(provider.created.len(), created);
        assert_eq!(provider.reads_at_create.len(), 3);
        assert_eq!(
            backend.record.borrow().read_count,
            reads + if fault <= 1 { 4 } else { 6 }
        );
        assert_eq!(backend.record.borrow().rng_draws, draws);
        assert_eq!(transcript.inner.clone().finalize(), transcript_bytes);
        cleanup(&backend, 2, created);
    }
}

// A shared backend can reveal substitution of an earlier receipt only after a later
// destination seals. This isolates final all-receipt validation from per-conversion checks.
struct LateSubstitution {
    seals: Cell<usize>,
    target_ordinal: u64,
    nonordinal_on_second_layout: bool,
}
struct LateSnapshot {
    inner: Snapshot,
    substitution: Rc<LateSubstitution>,
    layout_reads: Cell<usize>,
}
impl StoredPolynomialSnapshotV1 for LateSnapshot {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let mut layout = self.inner.layout();
        self.layout_reads.set(self.layout_reads.get() + 1);
        if layout.ordinal() == self.substitution.target_ordinal {
            if self.substitution.nonordinal_on_second_layout {
                // The raw converter's final snapshot check is first; the phase bridge's
                // recapture is second. Keep ordinal unchanged to test the whole identity.
                if self.layout_reads.get() >= 2 {
                    layout.basis = StoredPolynomialBasisV1::Lagrange;
                }
            } else if self.substitution.seals.get() == 5 {
                layout.ordinal += 1000;
            }
        }
        layout
    }
    fn with_chunk<R>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredPolynomialErrorV1>,
    ) -> Result<R, StoredPolynomialErrorV1> {
        self.inner.with_chunk(expected, chunk, consume)
    }
    fn with_column<R>(
        &mut self,
        _: StoredPolynomialLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredPolynomialErrorV1>,
    ) -> Result<R, StoredPolynomialErrorV1> {
        panic!("no backend full-column materialization")
    }
}
struct LateWriter {
    inner: Writer,
    substitution: Rc<LateSubstitution>,
}
impl StoredPolynomialWriterV1 for LateWriter {
    type Snapshot = LateSnapshot;
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        self.inner.layout()
    }
    fn write_chunk(
        &mut self,
        chunk: u64,
        values: &[[u8; 32]],
    ) -> Result<(), StoredPolynomialErrorV1> {
        self.inner.write_chunk(chunk, values)
    }
    fn seal(self) -> Result<LateSnapshot, StoredPolynomialErrorV1> {
        let inner = self.inner.seal()?;
        self.substitution
            .seals
            .set(self.substitution.seals.get() + 1);
        Ok(LateSnapshot {
            inner,
            substitution: self.substitution,
            layout_reads: Cell::new(0),
        })
    }
}
struct LateProvider {
    inner: Provider,
    substitution: Rc<LateSubstitution>,
}
impl StoredPolynomialProviderV1 for LateProvider {
    type Writer = LateWriter;
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<LateWriter, StoredPolynomialErrorV1> {
        Ok(LateWriter {
            inner: self.inner.create(field, basis, k, role)?,
            substitution: Rc::clone(&self.substitution),
        })
    }
}
fn late_receipt_substitution<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let domain = EvaluationDomain::<C::Scalar>::new(3, 4);
    for (target_ordinal, nonordinal_on_second_layout) in [(11, false), (38, false), (38, true)] {
        // Earlier original source, earlier output, then nonordinal output drift on recapture.
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (committed, _rng, _transcript) = absorbed_through(&params, &backend, 2);
        let mut complete = committed.into_complete().unwrap();
        let substitution = Rc::new(LateSubstitution {
            seals: Cell::new(0),
            target_ordinal,
            nonordinal_on_second_layout,
        });
        // Only the test replaces the backend type. Every actual source snapshot and original
        // SecretBlind guard moves once; no copied blind or fabricated witness is introduced.
        let Session {
            plan,
            next_phase,
            proof_context,
            greatest_ordinal,
            columns,
            challenges,
        } = complete.session.take().unwrap();
        let columns = columns
            .into_iter()
            .map(
                |StoredColumn {
                     layout,
                     snapshot,
                     blind,
                 }| StoredColumn {
                    layout,
                    snapshot: LateSnapshot {
                        inner: snapshot,
                        substitution: Rc::clone(&substitution),
                        layout_reads: Cell::new(0),
                    },
                    blind,
                },
            )
            .collect();
        let complete = CompleteStoredAdviceV1 {
            session: Some(Session {
                plan,
                next_phase,
                proof_context,
                greatest_ordinal,
                columns,
                challenges,
            }),
        };
        let mut provider = LateProvider {
            inner: Provider::new(&backend, 38),
            substitution,
        };
        let result = complete
            .stage_coefficients(&domain, &mut provider)
            .map(|_| ());
        assert_eq!(
            result,
            Err(StoredPhaseErrorV1::Store(StoredPolynomialErrorV1::Context))
        );
        let outputs = if nonordinal_on_second_layout { 1 } else { 5 };
        assert_eq!(provider.substitution.seals.get(), outputs);
        cleanup(&backend, outputs, outputs);
    }
}

#[test]
fn both_fields_partial_coefficient_failures_and_unwinds_drop_sources_outputs_and_wipe_blinds() {
    partial_failures::<EqAffine>();
    partial_failures::<EpAffine>();
    late_receipt_substitution::<EqAffine>();
    late_receipt_substitution::<EpAffine>();
}

#[test]
fn coefficient_staging_rejects_lookup_source_and_destination_roles_before_reads() {
    let params = ParamsIPA::<EqAffine>::new(4);
    let domain = EvaluationDomain::<Fp>::new(3, 4);
    for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (committed, _, _) = absorbed_through(&params, &backend, 2);
        let mut complete = committed.into_complete().unwrap();
        let session = complete.session.as_mut().unwrap();
        session.columns[4].layout.role =
            StoredPolynomialRoleV1::LookupCompressed { lookup: 4, side };
        session.columns[4].snapshot.layout = session.columns[4].layout;
        let reads = backend.record.borrow().read_count;
        let mut provider = Provider::new(&backend, 38);
        assert!(matches!(
            complete.stage_coefficients(&domain, &mut provider),
            Err(StoredPhaseErrorV1::Admission)
        ));
        assert!(provider.created.is_empty());
        assert_eq!(backend.record.borrow().read_count, reads);
        cleanup(&backend, 0, 0);

        for at in [0, 2] {
            reset_blind_drops();
            let backend = Rc::new(Backend::default());
            let (committed, _, _) = absorbed_through(&params, &backend, 2);
            let complete = committed.into_complete().unwrap();
            let reads = backend.record.borrow().read_count;
            let mut provider = Provider::new(&backend, 38);
            provider.wrong_role = Some((
                at,
                StoredPolynomialRoleV1::LookupCompressed { lookup: at, side },
            ));
            assert!(matches!(
                complete.stage_coefficients(&domain, &mut provider),
                Err(StoredPhaseErrorV1::Store(StoredPolynomialErrorV1::Context))
            ));
            assert_eq!(provider.created.len(), at as usize + 1);
            assert_eq!(backend.record.borrow().read_count, reads + at as usize);
            cleanup(&backend, at as usize, at as usize + 1);
        }
    }
}
