//! Concrete stored lookup compression against ordinary theta and dense expression evaluation.
//!
//! The backend deliberately keeps plaintext oracle copies. These tests establish bounded-stage
//! arithmetic, ordering and owner teardown, not a complete lookup argument, proof or RSS bound.

use super::super::super::lookup::StoredLookupErrorV1;
use super::*;
use crate::poly::stored_advice::{
    StoredLookupSideV1, StoredPolynomialErrorV1, StoredPolynomialLayoutV1,
    StoredPolynomialProviderV1, StoredPolynomialRoleV1, StoredPolynomialSnapshotV1,
    StoredPolynomialWriterV1,
};

#[derive(Debug)]
struct OrdinaryThetaReached;

/// Runs the actual ordinary prover only through theta, before synthetic lookup membership
/// matters. A typed sentinel distinguishes the intended stop from any unrelated panic.
struct ThroughTheta<C: CurveAffine>
where
    C::Scalar: FromUniformBytes<64>,
{
    transcript: RecordingTranscript<C>,
    remaining: usize,
}

impl<C: CurveAffine> Transcript<C, Challenge255<C>> for ThroughTheta<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn squeeze_challenge(&mut self) -> Challenge255<C> {
        let challenge = self.transcript.squeeze_challenge();
        self.remaining = self.remaining.checked_sub(1).unwrap();
        if self.remaining == 0 {
            std::panic::panic_any(OrdinaryThetaReached);
        }
        challenge
    }

    fn common_point(&mut self, point: C) -> io::Result<()> {
        self.transcript.common_point(point)
    }

    fn common_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.transcript.common_scalar(scalar)
    }
}

impl<C: CurveAffine> TranscriptWrite<C, Challenge255<C>> for ThroughTheta<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn write_point(&mut self, point: C) -> io::Result<()> {
        self.transcript.write_point(point)
    }

    fn write_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.transcript.write_scalar(scalar)
    }
}

fn read_compressed<C: CurveAffine, S: StoredPolynomialSnapshotV1>(
    snapshot: &mut S,
    layout: StoredPolynomialLayoutV1,
) -> Vec<C::Scalar>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    assert_eq!(snapshot.layout(), layout);
    let mut decoded = Vec::new();
    for chunk in 0..layout.chunk_count() as u64 {
        snapshot
            .with_chunk(layout, chunk, |values| {
                decoded.extend(
                    values.iter().map(|value| {
                        Option::<C::Scalar>::from(C::Scalar::from_repr(*value)).unwrap()
                    }),
                );
                Ok(())
            })
            .unwrap();
    }
    assert_eq!(decoded.len(), layout.scalar_count());
    decoded
}

fn compressed_oracle<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    for k in [4, 9] {
        for selectors in [false, true] {
            let params = ParamsIPA::<C>::new(k);
            let pk = lookup_key(&params, selectors);
            let rows = 1_usize << k;
            let last = if k == 9 { 300 } else { 3 };
            let values = [
                vec![C::Scalar::from(17)],
                vec![],
                (0..last).map(|i| C::Scalar::from(i as u64 + 23)).collect(),
            ];
            let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
            let ordinary = Shared::<C>::new();
            let mut ordinary_transcript = ThroughTheta {
                transcript: RecordingTranscript::new(&ordinary),
                remaining: pk.vk.cs.num_challenges + 1,
            };
            let stop = catch_unwind(AssertUnwindSafe(|| {
                create_proof_consuming::<
                    IPACommitmentScheme<C>,
                    ProverIPA<'_, C, true, 6>,
                    Challenge255<C>,
                    _,
                    _,
                    _,
                >(
                    &params,
                    pk.clone(),
                    LookupProducer(Producer::new(&ordinary, last)),
                    &[&instances],
                    Rng(Arc::clone(&ordinary)),
                    &mut ordinary_transcript,
                )
            }));
            let panic = match stop {
                Err(panic) => panic,
                Ok(_) => panic!("ordinary prover did not reach the theta sentinel"),
            };
            assert!(panic.is::<OrdinaryThetaReached>());
            assert_eq!(ordinary_transcript.remaining, 0);
            let ordinary_events = ordinary.log.lock().unwrap().events.clone();
            let theta = match ordinary_events.last() {
                Some(Event::Challenge(value)) => *value,
                _ => panic!("ordinary theta was not the last transcript event"),
            };
            let mut ordinary_rng = ordinary.rng.lock().unwrap().clone();
            let shared = Shared::<C>::new();
            let prefix = prepare_single_phase_stored_ipa_prefix_v1::<
                C,
                _,
                _,
                _,
                Challenge255<C>,
                _,
                true,
                6,
            >(
                &params,
                pk,
                LookupProducer(Producer::new(&shared, last)),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap();
            let vk = prefix.pk.get_vk().to_bytes(crate::SerdeFormat::Processed);
            let fixed_allocation = prefix.pk.fixed_values.as_ptr();
            let challenges = prefix.advice.challenges().unwrap().collect::<Vec<_>>();
            let original_layouts = prefix.advice.layouts().unwrap().collect::<Vec<_>>();
            let dense_advice = shared
                .log
                .lock()
                .unwrap()
                .sealed
                .iter()
                .map(|(_, values)| {
                    prefix.pk.vk.domain.lagrange_from_vec(
                        values
                            .iter()
                            .map(|value| {
                                Option::<C::Scalar>::from(C::Scalar::from_repr(*value)).unwrap()
                            })
                            .collect(),
                    )
                })
                .collect::<Vec<_>>();
            let dense_instances = values
                .iter()
                .map(|values| {
                    let mut dense = prefix.pk.vk.domain.empty_lagrange();
                    dense[0..values.len()].copy_from_slice(values);
                    dense
                })
                .collect::<Vec<_>>();
            let mut expected = Vec::new();
            for lookup in &prefix.pk.vk.cs.lookups {
                for expressions in [&lookup.input_expressions, &lookup.table_expressions] {
                    let mut compressed = vec![C::Scalar::ZERO; rows];
                    for expression in expressions {
                        // Use the actual ordinary evaluator and explicit Horner order.
                        let evaluated = evaluate(
                            expression,
                            rows,
                            1,
                            &prefix.pk.fixed_values,
                            &dense_advice,
                            &dense_instances,
                            &challenges,
                        );
                        for (accumulator, value) in compressed.iter_mut().zip(evaluated.iter()) {
                            *accumulator = *accumulator * theta + value;
                        }
                    }
                    expected.push(compressed);
                }
            }
            let staged = prefix.stage_advice_coefficients().unwrap();
            let (events, draws, prior_sealed) = {
                let log = shared.log.lock().unwrap();
                (log.events.clone(), log.rng_calls, log.sealed.clone())
            };
            assert_eq!(prior_sealed.len(), 4);
            assert_eq!(events, ordinary_events[..ordinary_events.len() - 1]);
            let mut compressed = staged.compress_lookups(1 << 20).unwrap();
            assert_eq!(*compressed.theta, theta);
            assert_eq!(compressed.lookups.len(), 2);
            assert!(std::ptr::eq(compressed.inner.params, &params));
            assert!(std::ptr::eq(
                compressed.inner.instances.as_ptr(),
                instances.as_ptr()
            ));
            assert_eq!(compressed.inner.pk.fixed_values.as_ptr(), fixed_allocation);
            assert_eq!(
                compressed
                    .inner
                    .pk
                    .get_vk()
                    .to_bytes(crate::SerdeFormat::Processed),
                vk
            );
            assert!(Arc::ptr_eq(&compressed.inner.provider.shared, &shared));
            assert!(Arc::ptr_eq(&compressed.inner.rng.0, &shared));
            assert!(Arc::ptr_eq(&compressed.inner.transcript.shared, &shared));
            assert_eq!(
                compressed
                    .inner
                    .advice
                    .layouts()
                    .unwrap()
                    .collect::<Vec<_>>(),
                original_layouts
            );
            assert_eq!(
                compressed
                    .inner
                    .advice
                    .challenges()
                    .unwrap()
                    .collect::<Vec<_>>(),
                challenges
            );
            {
                let log = shared.log.lock().unwrap();
                assert_eq!(log.events, ordinary_events);
                assert_eq!(log.events.len(), events.len() + 1);
                assert_eq!(log.rng_calls, draws);
                assert_eq!(log.created, 8);
                assert_eq!(log.writer_drops, 8);
                assert_eq!(log.snapshot_drops, 0);
                assert_eq!(log.provider_drops, 0);
                assert_eq!(log.rng_drops, 0);
                assert_eq!(log.transcript_drops, 0);
                assert_eq!(log.sealed[..4], prior_sealed);
            }
            for (lookup_index, pair) in compressed.lookups.iter_mut().enumerate() {
                for (side_index, (side, column)) in [
                    (StoredLookupSideV1::Input, &mut pair.input),
                    (StoredLookupSideV1::Table, &mut pair.table),
                ]
                .into_iter()
                .enumerate()
                {
                    let index = 2 * lookup_index + side_index;
                    assert_eq!(
                        column.layout.role(),
                        StoredPolynomialRoleV1::LookupCompressed {
                            lookup: lookup_index as u32,
                            side,
                        }
                    );
                    assert_eq!(column.layout.basis(), StoredPolynomialBasisV1::Lagrange);
                    assert_eq!(column.layout.k(), k);
                    assert_eq!(column.layout.field(), C::Scalar::STORED_FIELD);
                    assert_eq!(column.layout.ordinal(), 4 + index as u64);
                    assert!(column.layout.same_proof_context(original_layouts[0]));
                    assert_eq!(
                        read_compressed::<C, _>(&mut column.snapshot, column.layout),
                        expected[index]
                    );
                }
            }
            // Theta/compression must leave the exact next proof randomness untouched.
            let (mut actual_next, mut expected_next) = ([0; 64], [0; 64]);
            compressed.inner.rng.fill_bytes(&mut actual_next);
            ordinary_rng.fill_bytes(&mut expected_next);
            assert_eq!(actual_next, expected_next);
            drop(compressed);
            assert_dropped(&shared);
            assert_eq!(shared.log.lock().unwrap().snapshot_drops, 8);
        }
    }
}

#[test]
fn eq_lookup_compression_matches_ordinary_theta_dense_horner_and_original_owners() {
    compressed_oracle::<EqAffine>();
}

#[test]
fn ep_lookup_compression_matches_ordinary_theta_dense_horner_and_original_owners() {
    compressed_oracle::<EpAffine>();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Injection {
    None,
    Create(u64),
    Write(u64, u64),
    Seal(u64),
    Read(u64, u64),
    PanicWrite(u64, u64),
    PanicSeal(u64),
    PanicRead(u64, u64),
    CreateContext,
    CreateRole,
    CreateOrdinal,
    WriterSecondObservation,
    WriterDrift,
    SealedDrift,
    EarlierDrift(u64),
}

struct InjectedProvider<C: CurveAffine> {
    inner: Provider<C>,
    injection: Arc<Mutex<Injection>>,
    window: Arc<std::sync::atomic::AtomicBool>,
}

struct InjectedWriter<C: CurveAffine> {
    inner: Writer<C>,
    injection: Arc<Mutex<Injection>>,
    observations: std::cell::Cell<usize>,
    window: Arc<std::sync::atomic::AtomicBool>,
}

struct InjectedSnapshot<C: CurveAffine> {
    inner: Snapshot<C>,
    injection: Arc<Mutex<Injection>>,
    window: Arc<std::sync::atomic::AtomicBool>,
}

struct ReadWindow(Arc<std::sync::atomic::AtomicBool>);

impl Drop for ReadWindow {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

fn different_context(layout: StoredPolynomialLayoutV1) -> StoredPolynomialLayoutV1 {
    StoredPolynomialLayoutV1::new(
        [41; 32],
        layout.ordinal(),
        layout.field(),
        layout.basis(),
        layout.k(),
        layout.role(),
    )
    .unwrap()
}

impl<C: CurveAffine> StoredPolynomialProviderV1 for InjectedProvider<C> {
    type Writer = InjectedWriter<C>;

    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        assert!(!self.window.load(std::sync::atomic::Ordering::SeqCst));
        let injection = *self.injection.lock().unwrap();
        if injection == Injection::Create(self.inner.ordinal) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        let mut inner = self.inner.create(field, basis, k, role)?;
        if inner.layout.ordinal() == 4 {
            inner.layout = match injection {
                Injection::CreateContext => different_context(inner.layout),
                Injection::CreateRole => StoredPolynomialLayoutV1::new(
                    [23; 32],
                    4,
                    field,
                    basis,
                    k,
                    StoredPolynomialRoleV1::LookupCompressed {
                        lookup: 0,
                        side: StoredLookupSideV1::Table,
                    },
                )?,
                Injection::CreateOrdinal => {
                    StoredPolynomialLayoutV1::new([23; 32], 3, field, basis, k, role)?
                }
                _ => inner.layout,
            };
        }
        Ok(InjectedWriter {
            inner,
            injection: Arc::clone(&self.injection),
            observations: std::cell::Cell::new(0),
            window: Arc::clone(&self.window),
        })
    }
}

impl<C: CurveAffine> StoredPolynomialWriterV1 for InjectedWriter<C> {
    type Snapshot = InjectedSnapshot<C>;

    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let observation = self.observations.get();
        self.observations.set(observation + 1);
        let injection = *self.injection.lock().unwrap();
        if self.inner.layout.ordinal() == 4
            && ((injection == Injection::WriterDrift && self.inner.next > 0)
                || (injection == Injection::WriterSecondObservation && observation > 0))
        {
            different_context(self.inner.layout)
        } else {
            self.inner.layout()
        }
    }

    fn write_chunk(
        &mut self,
        chunk: u64,
        values: &[[u8; 32]],
    ) -> Result<(), StoredPolynomialErrorV1> {
        assert!(!self.window.load(std::sync::atomic::Ordering::SeqCst));
        let injection = *self.injection.lock().unwrap();
        let ordinal = self.inner.layout.ordinal();
        assert_ne!(injection, Injection::PanicWrite(ordinal, chunk));
        if injection == Injection::Write(ordinal, chunk) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        self.inner.write_chunk(chunk, values)
    }

    fn seal(self) -> Result<Self::Snapshot, StoredPolynomialErrorV1> {
        assert!(!self.window.load(std::sync::atomic::Ordering::SeqCst));
        let injection = *self.injection.lock().unwrap();
        let ordinal = self.inner.layout.ordinal();
        assert_ne!(injection, Injection::PanicSeal(ordinal));
        if injection == Injection::Seal(ordinal) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        let inner = self.inner.seal()?;
        if ordinal == 7 {
            if let Injection::EarlierDrift(victim) = injection {
                let mut log = inner.shared.log.lock().unwrap();
                let original = log
                    .sealed
                    .iter()
                    .find(|(layout, _)| layout.ordinal() == victim)
                    .unwrap()
                    .0;
                log.snapshot_layout_override = Some((victim, different_context(original)));
            }
        }
        Ok(InjectedSnapshot {
            inner,
            injection: self.injection,
            window: self.window,
        })
    }
}

impl<C: CurveAffine> StoredPolynomialSnapshotV1 for InjectedSnapshot<C> {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        if *self.injection.lock().unwrap() == Injection::SealedDrift
            && self.inner.layout.ordinal() == 5
        {
            different_context(self.inner.layout)
        } else {
            self.inner.layout()
        }
    }

    fn with_chunk<V>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        let injection = *self.injection.lock().unwrap();
        let ordinal = self.inner.layout.ordinal();
        assert_ne!(injection, Injection::PanicRead(ordinal, chunk));
        if injection == Injection::Read(ordinal, chunk) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        assert!(!self.window.swap(true, std::sync::atomic::Ordering::SeqCst));
        let _window = ReadWindow(Arc::clone(&self.window));
        self.inner.with_chunk(expected, chunk, consume)
    }

    fn with_column<V>(
        &mut self,
        _: StoredPolynomialLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        panic!("lookup compression must never materialize a full stored column")
    }
}

fn failed_compression<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let pk = lookup_key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for injection in [
        Injection::Create(6),
        Injection::Write(5, 1),
        Injection::Seal(5),
        Injection::Read(0, 1),
        Injection::PanicWrite(5, 1),
        Injection::PanicSeal(5),
        Injection::PanicRead(0, 1),
    ] {
        let shared = Shared::<C>::new();
        let control = Arc::new(Mutex::new(Injection::None));
        let staged =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                pk.clone(),
                LookupProducer(Producer::new(&shared, 300)),
                &instances,
                InjectedProvider {
                    inner: Provider::new(&shared),
                    injection: Arc::clone(&control),
                    window: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                },
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap()
            .stage_advice_coefficients()
            .unwrap();
        let (events, draws) = {
            let log = shared.log.lock().unwrap();
            assert_eq!(log.sealed.len(), 4);
            (log.events.clone(), log.rng_calls)
        };
        *control.lock().unwrap() = injection;
        let result = catch_unwind(AssertUnwindSafe(|| {
            staged.compress_lookups(1 << 20).map(|_| ())
        }));
        if matches!(
            injection,
            Injection::PanicWrite(..) | Injection::PanicSeal(..) | Injection::PanicRead(..)
        ) {
            assert!(result.is_err(), "{injection:?}");
        } else {
            assert!(result.unwrap().is_err(), "{injection:?}");
        }
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events[..events.len()], events);
        assert_eq!(log.events.len(), events.len() + 1);
        assert!(matches!(log.events.last(), Some(Event::Challenge(_))));
        assert_eq!(log.rng_calls, draws);
        let sealed = match injection {
            Injection::Create(6) => 6,
            Injection::Write(..)
            | Injection::Seal(..)
            | Injection::PanicWrite(..)
            | Injection::PanicSeal(..) => 5,
            _ => 4,
        };
        assert_eq!(log.sealed.len(), sealed, "{injection:?}");
        assert_eq!(log.snapshot_drops, sealed, "{injection:?}");
    }
}

#[test]
fn both_pasta_partial_lookup_backend_errors_and_unwinds_destroy_every_protocol_owner() {
    failed_compression::<EqAffine>();
    failed_compression::<EpAffine>();
}

fn metadata_compression<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let pk = lookup_key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for injection in [
        Injection::None,
        Injection::CreateContext,
        Injection::CreateRole,
        Injection::CreateOrdinal,
        Injection::WriterSecondObservation,
        Injection::WriterDrift,
        Injection::SealedDrift,
        Injection::EarlierDrift(0),
        Injection::EarlierDrift(2),
        Injection::EarlierDrift(4),
    ] {
        let shared = Shared::<C>::new();
        let control = Arc::new(Mutex::new(Injection::None));
        let staged =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                pk.clone(),
                LookupProducer(Producer::new(&shared, 300)),
                &instances,
                InjectedProvider {
                    inner: Provider::new(&shared),
                    injection: Arc::clone(&control),
                    window: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                },
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap()
            .stage_advice_coefficients()
            .unwrap();
        let (events, reads, writes, draws) = {
            let log = shared.log.lock().unwrap();
            (log.events.clone(), log.reads, log.writes, log.rng_calls)
        };
        *control.lock().unwrap() = injection;
        let result = staged.compress_lookups(1 << 20).map(|_| ());
        if injection == Injection::None {
            assert_eq!(result, Ok(()));
        } else {
            assert!(
                matches!(
                    result,
                    Err(StoredLookupErrorV1::Store(StoredPolynomialErrorV1::Context))
                        | Err(StoredLookupErrorV1::Phase(StoredPhaseErrorV1::Store(
                            StoredPolynomialErrorV1::Context
                        )))
                        | Err(StoredLookupErrorV1::Expression(
                            StoredExpressionErrorV1::Store(StoredPolynomialErrorV1::Context)
                        ))
                ),
                "unexpected metadata refusal for {injection:?}: {result:?}"
            );
        }
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.rng_calls, draws);
        assert_eq!(log.events[..events.len()], events);
        assert_eq!(log.events.len(), events.len() + 1);
        assert!(matches!(log.events.last(), Some(Event::Challenge(_))));
        if matches!(
            injection,
            Injection::CreateContext
                | Injection::CreateRole
                | Injection::CreateOrdinal
                | Injection::WriterSecondObservation
        ) {
            assert_eq!(
                log.reads, reads,
                "unadmitted destination reached witness reads"
            );
            assert_eq!(log.writes, writes, "unadmitted destination reached writes");
            assert_eq!(log.sealed.len(), 4);
        }
        if matches!(injection, Injection::EarlierDrift(_)) {
            assert_eq!(log.sealed.len(), 8);
            assert_eq!(log.snapshot_drops, 8);
        }
    }
}

#[test]
fn both_pasta_lookup_role_ordinal_and_live_receipt_drift_fail_closed() {
    metadata_compression::<EqAffine>();
    metadata_compression::<EpAffine>();
}

fn preflight_compression<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let wrong_params = ParamsIPA::<C>::new(5);
    let pk = lookup_key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let oversized = vec![C::Scalar::ONE; 16];
    let mut oversized_instances = instances.clone();
    oversized_instances[1] = &oversized;
    let short_domain = crate::poly::EvaluationDomain::<C::Scalar>::new(3, 3);
    for fault in 0..7 {
        let shared = Shared::<C>::new();
        let mut staged =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                pk.clone(),
                LookupProducer(Producer::new(&shared, 3)),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap()
            .stage_advice_coefficients()
            .unwrap();
        let mut budget = 1 << 20;
        match fault {
            0 => budget = 0,
            1 => {
                staged.pk.fixed_values.pop();
            }
            2 => staged.instances = &[],
            3 => staged.params = &wrong_params,
            4 => staged.instances = &oversized_instances,
            5 => {
                staged.pk.fixed_values[0] = short_domain.lagrange_from_vec(vec![C::Scalar::ONE; 8]);
            }
            6 => {
                let mut log = shared.log.lock().unwrap();
                let layout = log.sealed[2].0;
                log.snapshot_layout_override = Some((layout.ordinal(), different_context(layout)));
            }
            _ => unreachable!(),
        }
        let (events, reads, writes, draws, created) = {
            let log = shared.log.lock().unwrap();
            (
                log.events.clone(),
                log.reads,
                log.writes,
                log.rng_calls,
                log.created,
            )
        };
        let result = staged.compress_lookups(budget).map(|_| ());
        if fault == 0 {
            assert_eq!(result, Err(StoredLookupErrorV1::ScratchLimit));
        } else {
            assert!(result.is_err(), "preflight {fault}");
        }
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events, "preflight {fault} squeezed theta");
        assert_eq!(log.reads, reads, "preflight {fault} read witness data");
        assert_eq!(log.writes, writes);
        assert_eq!(log.rng_calls, draws);
        assert_eq!(log.created, created);
        assert_eq!(log.snapshot_drops, 4);
    }
}

#[test]
fn both_pasta_lookup_preflights_reject_before_theta_witness_or_destination_creation() {
    preflight_compression::<EqAffine>();
    preflight_compression::<EpAffine>();
}

/// An actual zero-advice circuit, with either no lookup or an empty expression pair list.
struct SparseLookupProducer<C: CurveAffine, const NO_LOOKUPS: bool>(EmptyLookupProducer<C>);

impl<C: CurveAffine, const NO_LOOKUPS: bool> Circuit<C::Scalar>
    for SparseLookupProducer<C, NO_LOOKUPS>
{
    type Config = EmptyConfig;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self(self.0.without_witnesses())
    }

    fn configure(meta: &mut ConstraintSystem<C::Scalar>) -> EmptyConfig {
        let fixed = meta.fixed_column();
        let instance = meta.instance_column();
        if !NO_LOOKUPS {
            meta.lookup_any("empty expression pair list", |_| vec![]);
        }
        EmptyConfig { fixed, instance }
    }

    fn synthesize_for_measurement(
        &self,
        config: EmptyConfig,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.0.run(config, layouter, true)
    }

    fn synthesize(
        &self,
        config: EmptyConfig,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.0.run(config, layouter, false)
    }
}

fn sparse_compression<C, const NO_LOOKUPS: bool>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    for k in [4, 9] {
        let params = ParamsIPA::<C>::new(k);
        let key_shared = Shared::<C>::new();
        let producer = SparseLookupProducer::<C, NO_LOOKUPS>(EmptyLookupProducer(Producer::new(
            &key_shared,
            0,
        )));
        let vk = keygen_vk_custom(&params, &producer, true).unwrap();
        let pk = keygen_pk(&params, vk, &producer).unwrap();
        let values = vec![C::Scalar::from(17)];
        let instances: [&[C::Scalar]; 1] = [&values];
        let shared = Shared::<C>::new();
        let staged =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
                &params,
                pk,
                SparseLookupProducer::<C, NO_LOOKUPS>(EmptyLookupProducer(Producer::new(
                    &shared, 0,
                ))),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap()
            .stage_advice_coefficients()
            .unwrap();
        let events = shared.log.lock().unwrap().events.clone();
        assert_eq!(shared.log.lock().unwrap().created, 0);
        let mut expected_rng = shared.rng.lock().unwrap().clone();
        let mut compressed = staged
            .compress_lookups(if NO_LOOKUPS { 0 } else { 16 * 1024 })
            .unwrap();
        assert_eq!(compressed.lookups.len(), usize::from(!NO_LOOKUPS));
        assert_eq!(
            compressed.inner.advice.proof_context().unwrap(),
            if NO_LOOKUPS { None } else { Some([23; 32]) }
        );
        {
            let log = shared.log.lock().unwrap();
            assert_eq!(log.events[..events.len()], events);
            assert_eq!(log.events.len(), events.len() + 1);
            assert_eq!(
                log.events.last(),
                Some(&Event::Challenge(*compressed.theta))
            );
            assert_eq!(log.rng_calls, 0);
            assert_eq!(log.reads, 0);
            assert_eq!(log.created, if NO_LOOKUPS { 0 } else { 2 });
            assert_eq!(log.snapshot_drops, 0);
        }
        if !NO_LOOKUPS {
            let pair = &mut compressed.lookups[0];
            for (ordinal, (side, column)) in [
                (StoredLookupSideV1::Input, &mut pair.input),
                (StoredLookupSideV1::Table, &mut pair.table),
            ]
            .into_iter()
            .enumerate()
            {
                let expected_layout = StoredPolynomialLayoutV1::new(
                    [23; 32],
                    ordinal as u64,
                    C::Scalar::STORED_FIELD,
                    StoredPolynomialBasisV1::Lagrange,
                    k,
                    StoredPolynomialRoleV1::LookupCompressed { lookup: 0, side },
                )
                .unwrap();
                // The first real writer establishes the context; no public geometry label
                // substitutes for this authenticated provider context, even with zero advice.
                assert_eq!(column.layout, expected_layout);
                assert_eq!(
                    read_compressed::<C, _>(&mut column.snapshot, column.layout),
                    vec![C::Scalar::ZERO; 1 << k]
                );
            }
        }
        let (mut actual_next, mut expected_next) = ([0; 64], [0; 64]);
        compressed.inner.rng.fill_bytes(&mut actual_next);
        expected_rng.fill_bytes(&mut expected_next);
        assert_eq!(actual_next, expected_next);
        drop(compressed);
        assert_dropped(&shared);
    }
}

#[test]
fn both_pasta_zero_advice_empty_lists_write_real_zero_polynomials_in_provider_context() {
    sparse_compression::<EqAffine, false>();
    sparse_compression::<EpAffine, false>();
}

#[test]
fn both_pasta_zero_advice_no_lookups_squeeze_theta_with_zero_budget_and_no_store() {
    sparse_compression::<EqAffine, true>();
    sparse_compression::<EpAffine, true>();
}

fn zero_advice_values<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let key_shared = Shared::<C>::new();
    let producer = EmptyLookupProducer(Producer::new(&key_shared, 0));
    let vk = keygen_vk_custom(&params, &producer, true).unwrap();
    let pk = keygen_pk(&params, vk, &producer).unwrap();
    let values = vec![C::Scalar::from(17)];
    let instances: [&[C::Scalar]; 1] = [&values];
    let shared = Shared::<C>::new();
    let prefix =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
            &params,
            pk,
            EmptyLookupProducer(Producer::new(&shared, 0)),
            &instances,
            Provider::new(&shared),
            Rng(Arc::clone(&shared)),
            RecordingTranscript::new(&shared),
        )
        .unwrap();
    assert_eq!(prefix.advice.proof_context().unwrap(), None);
    let mut dense = prefix.pk.vk.domain.empty_lagrange();
    dense[0] = C::Scalar::from(17);
    let mut expected = Vec::new();
    for expression in [
        &prefix.pk.vk.cs.lookups[0].input_expressions[0],
        &prefix.pk.vk.cs.lookups[0].table_expressions[0],
    ] {
        expected.push(evaluate(
            expression,
            512,
            1,
            &prefix.pk.fixed_values,
            &[],
            &[dense.clone()],
            &[],
        ));
    }
    let mut compressed = prefix
        .stage_advice_coefficients()
        .unwrap()
        .compress_lookups(1 << 20)
        .unwrap();
    assert_eq!(compressed.lookups.len(), 1);
    let pair = &mut compressed.lookups[0];
    for (index, column) in [&mut pair.input, &mut pair.table].into_iter().enumerate() {
        assert_eq!(column.layout.ordinal(), index as u64);
        assert_eq!(
            read_compressed::<C, _>(&mut column.snapshot, column.layout),
            expected[index].to_vec()
        );
    }
    assert_eq!(shared.log.lock().unwrap().rng_calls, 0);
    assert_eq!(shared.log.lock().unwrap().created, 2);
    drop(compressed);
    assert_dropped(&shared);
}

#[test]
fn both_pasta_zero_advice_lookup_compression_uses_actual_key_fixed_and_padded_instances() {
    zero_advice_values::<EqAffine>();
    zero_advice_values::<EpAffine>();
}

fn no_lookup_advice<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let pk = key(&params, true);
    assert!(pk.vk.cs.lookups.is_empty());
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let staged =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
            &params,
            pk,
            Producer::new(&shared, 3),
            &instances,
            Provider::new(&shared),
            Rng(Arc::clone(&shared)),
            RecordingTranscript::new(&shared),
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap();
    let (events, draws, reads, writes, sealed) = {
        let log = shared.log.lock().unwrap();
        (
            log.events.clone(),
            log.rng_calls,
            log.reads,
            log.writes,
            log.sealed.clone(),
        )
    };
    let mut expected_rng = shared.rng.lock().unwrap().clone();
    let mut compressed = staged.compress_lookups(0).unwrap();
    assert!(compressed.lookups.is_empty());
    assert_eq!(compressed.inner.advice.layouts().unwrap().len(), 2);
    assert_eq!(
        compressed.inner.advice.proof_context().unwrap(),
        Some([23; 32])
    );
    {
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events[..events.len()], events);
        assert_eq!(log.events.len(), events.len() + 1);
        assert_eq!(
            log.events.last(),
            Some(&Event::Challenge(*compressed.theta))
        );
        assert_eq!(log.rng_calls, draws);
        assert_eq!(log.reads, reads);
        assert_eq!(log.writes, writes);
        assert_eq!(log.sealed, sealed);
        assert_eq!(log.created, 4);
        assert_eq!(log.snapshot_drops, 0);
    }
    let (mut actual_next, mut expected_next) = ([0; 64], [0; 64]);
    compressed.inner.rng.fill_bytes(&mut actual_next);
    expected_rng.fill_bytes(&mut expected_next);
    assert_eq!(actual_next, expected_next);
    drop(compressed);
    assert_dropped(&shared);
}

#[test]
fn both_pasta_no_lookup_theta_retains_all_advice_and_coefficients_without_scratch() {
    no_lookup_advice::<EqAffine>();
    no_lookup_advice::<EpAffine>();
}

fn empty_list_budget<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let key_shared = Shared::<C>::new();
    let producer =
        SparseLookupProducer::<C, false>(EmptyLookupProducer(Producer::new(&key_shared, 0)));
    let vk = keygen_vk_custom(&params, &producer, true).unwrap();
    let pk = keygen_pk(&params, vk, &producer).unwrap();
    let values = [C::Scalar::from(17)];
    let instances: [&[C::Scalar]; 1] = [&values];
    let shared = Shared::<C>::new();
    let staged =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
            &params,
            pk,
            SparseLookupProducer::<C, false>(EmptyLookupProducer(Producer::new(&shared, 0))),
            &instances,
            Provider::new(&shared),
            Rng(Arc::clone(&shared)),
            RecordingTranscript::new(&shared),
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap();
    let events = shared.log.lock().unwrap().events.clone();
    assert_eq!(
        staged.compress_lookups(16 * 1024 - 1).map(|_| ()),
        Err(StoredLookupErrorV1::ScratchLimit)
    );
    assert_dropped(&shared);
    let log = shared.log.lock().unwrap();
    assert_eq!(log.events, events);
    assert_eq!(log.created, 0);
    assert_eq!(log.writes, 0);
    assert_eq!(log.reads, 0);
    assert_eq!(log.rng_calls, 0);
}

#[test]
fn both_pasta_empty_lookup_lists_require_the_complete_output_tile_budget_before_theta() {
    empty_list_budget::<EqAffine>();
    empty_list_budget::<EpAffine>();
}

#[path = "lookup_sort_tests.rs"]
mod sorting;

#[path = "lookup_membership_tests.rs"]
mod membership;
